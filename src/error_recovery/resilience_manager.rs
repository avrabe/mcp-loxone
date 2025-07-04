//! Resilience manager combining circuit breakers and retry policies
//!
//! This module provides a unified interface for applying multiple resilience
//! patterns to operations, ensuring maximum availability and graceful degradation.

use crate::error::{LoxoneError, Result};
use crate::error_recovery::{
    CircuitBreakerConfig, CircuitBreakerManager, RetryExecutor, RetryPolicy,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Resilience configuration for a service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilienceConfig {
    /// Circuit breaker configuration
    pub circuit_breaker: CircuitBreakerConfig,
    /// Retry policy configuration
    pub retry_policy: RetryPolicy,
    /// Fallback configuration
    pub fallback: FallbackConfig,
    /// Enable timeout protection
    pub timeout_protection: bool,
    /// Timeout duration
    pub timeout_duration: chrono::Duration,
}

/// Fallback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    /// Enable fallback
    pub enabled: bool,
    /// Fallback strategy
    pub strategy: FallbackStrategy,
    /// Cache duration for cached fallbacks
    pub cache_duration: chrono::Duration,
}

/// Fallback strategies
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FallbackStrategy {
    /// Return default value
    Default,
    /// Return cached value
    Cached,
    /// Return degraded response
    Degraded,
    /// Custom fallback
    Custom,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            circuit_breaker: CircuitBreakerConfig::default(),
            retry_policy: RetryPolicy::default(),
            fallback: FallbackConfig {
                enabled: true,
                strategy: FallbackStrategy::Default,
                cache_duration: chrono::Duration::minutes(5),
            },
            timeout_protection: true,
            timeout_duration: chrono::Duration::seconds(30),
        }
    }
}

impl ResilienceConfig {
    /// Create configuration for critical services
    pub fn critical_service() -> Self {
        Self {
            circuit_breaker: CircuitBreakerConfig::critical_service(),
            retry_policy: RetryPolicy::critical(),
            fallback: FallbackConfig {
                enabled: true,
                strategy: FallbackStrategy::Cached,
                cache_duration: chrono::Duration::minutes(1),
            },
            timeout_protection: true,
            timeout_duration: chrono::Duration::seconds(10),
        }
    }

    /// Create configuration for background services
    pub fn background_service() -> Self {
        Self {
            circuit_breaker: CircuitBreakerConfig::non_critical_service(),
            retry_policy: RetryPolicy::background(),
            fallback: FallbackConfig {
                enabled: true,
                strategy: FallbackStrategy::Default,
                cache_duration: chrono::Duration::minutes(10),
            },
            timeout_protection: true,
            timeout_duration: chrono::Duration::minutes(2),
        }
    }
}

/// Resilience manager for coordinating error recovery strategies
pub struct ResilienceManager {
    configs: Arc<RwLock<HashMap<String, ResilienceConfig>>>,
    circuit_breaker_manager: Arc<CircuitBreakerManager>,
    retry_executors: Arc<RwLock<HashMap<String, Arc<RetryExecutor>>>>,
    fallback_cache: Arc<RwLock<HashMap<String, CachedFallback>>>,
}

/// Cached fallback value
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct CachedFallback {
    value: serde_json::Value,
    cached_at: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
}

impl ResilienceManager {
    /// Create new resilience manager
    pub fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
            circuit_breaker_manager: Arc::new(CircuitBreakerManager::new(
                CircuitBreakerConfig::default(),
            )),
            retry_executors: Arc::new(RwLock::new(HashMap::new())),
            fallback_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register service with resilience configuration
    pub async fn register_service(&self, service_name: &str, config: ResilienceConfig) {
        let mut configs = self.configs.write().await;
        configs.insert(service_name.to_string(), config.clone());

        // Create retry executor
        let executor = Arc::new(RetryExecutor::new(config.retry_policy));
        let mut executors = self.retry_executors.write().await;
        executors.insert(service_name.to_string(), executor);

        info!(
            "Registered resilience configuration for service: {}",
            service_name
        );
    }

    /// Execute operation with full resilience protection
    pub async fn execute_with_resilience<F, T, Fut>(
        &self,
        service_name: &str,
        operation: F,
        fallback_value: Option<T>,
    ) -> Result<T>
    where
        F: Fn() -> Fut + Clone,
        Fut: Future<Output = Result<T>>,
        T: serde::Serialize + serde::de::DeserializeOwned + Clone,
    {
        let configs = self.configs.read().await;
        let config = configs.get(service_name).cloned().unwrap_or_default();
        drop(configs);

        // Get circuit breaker
        let circuit_breaker = self.circuit_breaker_manager.get_breaker(service_name).await;

        // Check circuit breaker
        if !circuit_breaker.should_allow_request().await {
            warn!("Circuit breaker open for service: {}", service_name);

            if config.fallback.enabled {
                return self
                    .handle_fallback(service_name, &config.fallback, fallback_value)
                    .await;
            }

            return Err(LoxoneError::external_service_error(
                "Service temporarily unavailable - circuit breaker open",
            ));
        }

        // Get retry executor
        let executors = self.retry_executors.read().await;
        let executor = executors.get(service_name).cloned();
        drop(executors);

        // Execute with retry and timeout protection
        let result = if let Some(executor) = executor {
            if config.timeout_protection {
                self.execute_with_timeout(executor, operation, config.timeout_duration)
                    .await
            } else {
                executor.execute(operation).await
            }
        } else {
            // No retry executor, execute directly
            operation().await
        };

        // Handle result
        match result {
            Ok(value) => {
                circuit_breaker.record_success().await;

                // Cache successful result if caching is enabled
                if config.fallback.strategy == FallbackStrategy::Cached {
                    self.cache_fallback(service_name, &value, config.fallback.cache_duration)
                        .await;
                }

                Ok(value)
            }
            Err(error) => {
                circuit_breaker.record_failure(&error).await;

                if config.fallback.enabled {
                    self.handle_fallback(service_name, &config.fallback, fallback_value)
                        .await
                } else {
                    Err(error)
                }
            }
        }
    }

    /// Execute with timeout protection
    async fn execute_with_timeout<F, T, Fut>(
        &self,
        executor: Arc<RetryExecutor>,
        operation: F,
        timeout: chrono::Duration,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let timeout_duration = std::time::Duration::from_millis(timeout.num_milliseconds() as u64);

        match tokio::time::timeout(timeout_duration, executor.execute(operation)).await {
            Ok(result) => result,
            Err(_) => Err(LoxoneError::timeout("Operation timed out")),
        }
    }

    /// Handle fallback logic
    async fn handle_fallback<T>(
        &self,
        service_name: &str,
        fallback_config: &FallbackConfig,
        fallback_value: Option<T>,
    ) -> Result<T>
    where
        T: serde::Serialize + serde::de::DeserializeOwned + Clone,
    {
        match fallback_config.strategy {
            FallbackStrategy::Default => {
                if let Some(value) = fallback_value {
                    info!("Using default fallback for service: {}", service_name);
                    Ok(value)
                } else {
                    Err(LoxoneError::external_service_error(
                        "Service unavailable and no fallback provided",
                    ))
                }
            }
            FallbackStrategy::Cached => {
                let cache = self.fallback_cache.read().await;
                if let Some(cached) = cache.get(service_name) {
                    if cached.expires_at > chrono::Utc::now() {
                        info!("Using cached fallback for service: {}", service_name);
                        return serde_json::from_value(cached.value.clone()).map_err(|e| {
                            LoxoneError::internal(format!(
                                "Failed to deserialize cached value: {}",
                                e
                            ))
                        });
                    }
                }
                drop(cache);

                // Fall back to default if no cache
                if let Some(value) = fallback_value {
                    Ok(value)
                } else {
                    Err(LoxoneError::external_service_error(
                        "Service unavailable and no cached fallback available",
                    ))
                }
            }
            FallbackStrategy::Degraded => {
                info!("Using degraded response for service: {}", service_name);
                if let Some(value) = fallback_value {
                    Ok(value)
                } else {
                    Err(LoxoneError::external_service_error(
                        "Service unavailable - operating in degraded mode",
                    ))
                }
            }
            FallbackStrategy::Custom => {
                // Custom fallback logic would be implemented here
                if let Some(value) = fallback_value {
                    Ok(value)
                } else {
                    Err(LoxoneError::external_service_error(
                        "Service unavailable and custom fallback not implemented",
                    ))
                }
            }
        }
    }

    /// Cache fallback value
    async fn cache_fallback<T>(
        &self,
        service_name: &str,
        value: &T,
        cache_duration: chrono::Duration,
    ) where
        T: serde::Serialize,
    {
        let cached_value = match serde_json::to_value(value) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to serialize value for caching: {}", e);
                return;
            }
        };

        let now = chrono::Utc::now();
        let cached_fallback = CachedFallback {
            value: cached_value,
            cached_at: now,
            expires_at: now + cache_duration,
        };

        let mut cache = self.fallback_cache.write().await;
        cache.insert(service_name.to_string(), cached_fallback);

        debug!("Cached fallback value for service: {}", service_name);
    }

    /// Get resilience statistics for a service
    pub async fn get_stats(&self, service_name: &str) -> ResilienceStats {
        let circuit_stats = self
            .circuit_breaker_manager
            .get_breaker(service_name)
            .await
            .get_stats()
            .await;

        let retry_stats = {
            let executors = self.retry_executors.read().await;
            if let Some(executor) = executors.get(service_name) {
                Some(executor.get_stats().await)
            } else {
                None
            }
        };

        let fallback_stats = {
            let cache = self.fallback_cache.read().await;
            let cached = cache.get(service_name).is_some();
            FallbackStats {
                cached_value_available: cached,
            }
        };

        ResilienceStats {
            circuit_breaker: circuit_stats,
            retry: retry_stats,
            fallback: fallback_stats,
        }
    }

    /// Reset all resilience components for a service
    pub async fn reset_service(&self, service_name: &str) {
        // Reset circuit breaker
        let breaker = self.circuit_breaker_manager.get_breaker(service_name).await;
        breaker.reset().await;

        // Reset retry stats
        let executors = self.retry_executors.read().await;
        if let Some(executor) = executors.get(service_name) {
            executor.reset_stats().await;
        }

        // Clear fallback cache
        let mut cache = self.fallback_cache.write().await;
        cache.remove(service_name);

        info!("Reset resilience components for service: {}", service_name);
    }

    /// Clean up expired cache entries
    pub async fn cleanup_cache(&self) {
        let mut cache = self.fallback_cache.write().await;
        let now = chrono::Utc::now();

        cache.retain(|_, cached| cached.expires_at > now);

        debug!("Cleaned up expired fallback cache entries");
    }
}

/// Resilience statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilienceStats {
    /// Circuit breaker statistics
    pub circuit_breaker: crate::error_recovery::CircuitBreakerStats,
    /// Retry statistics
    pub retry: Option<crate::error_recovery::RetryStats>,
    /// Fallback statistics
    pub fallback: FallbackStats,
}

/// Fallback statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackStats {
    /// Whether cached value is available
    pub cached_value_available: bool,
}

/// Resilience builder for fluent API
pub struct ResilienceBuilder {
    manager: Arc<ResilienceManager>,
    service_name: String,
    config: ResilienceConfig,
}

impl ResilienceBuilder {
    /// Create new resilience builder
    pub fn new(manager: Arc<ResilienceManager>, service_name: String) -> Self {
        Self {
            manager,
            service_name,
            config: ResilienceConfig::default(),
        }
    }

    /// Set circuit breaker configuration
    pub fn circuit_breaker(mut self, config: CircuitBreakerConfig) -> Self {
        self.config.circuit_breaker = config;
        self
    }

    /// Set retry policy
    pub fn retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.config.retry_policy = policy;
        self
    }

    /// Enable fallback with strategy
    pub fn fallback(
        mut self,
        strategy: FallbackStrategy,
        cache_duration: chrono::Duration,
    ) -> Self {
        self.config.fallback = FallbackConfig {
            enabled: true,
            strategy,
            cache_duration,
        };
        self
    }

    /// Set timeout protection
    pub fn timeout(mut self, duration: chrono::Duration) -> Self {
        self.config.timeout_protection = true;
        self.config.timeout_duration = duration;
        self
    }

    /// Build and register the resilience configuration
    pub async fn build(self) {
        self.manager
            .register_service(&self.service_name, self.config)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resilience_manager() {
        let manager = Arc::new(ResilienceManager::new());

        // Register service with resilience
        ResilienceBuilder::new(manager.clone(), "test-service".to_string())
            .circuit_breaker(CircuitBreakerConfig {
                failure_threshold: 2,
                timeout_duration: chrono::Duration::milliseconds(100),
                ..Default::default()
            })
            .retry_policy(RetryPolicy {
                max_attempts: 2,
                initial_delay: chrono::Duration::milliseconds(10),
                ..Default::default()
            })
            .fallback(FallbackStrategy::Default, chrono::Duration::minutes(5))
            .build()
            .await;

        // Test successful operation
        let result = manager
            .execute_with_resilience("test-service", || async { Ok("success") }, Some("fallback"))
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        // Test operation with fallback
        let failure_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let failure_count_clone = failure_count.clone();
        let result = manager
            .execute_with_resilience(
                "test-service",
                move || {
                    let failure_count = failure_count_clone.clone();
                    async move {
                        failure_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        Err::<&str, _>(LoxoneError::connection("test failure"))
                    }
                },
                Some("fallback"),
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "fallback");
    }
}
