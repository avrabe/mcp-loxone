//! Advanced retry policy implementation with exponential backoff and jitter
//!
//! This module provides sophisticated retry strategies for handling transient
//! failures, with configurable backoff strategies, jitter, and circuit breaker
//! integration.

use crate::error::{LoxoneError, Result};
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Retry policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff strategy
    pub backoff_strategy: BackoffStrategy,
    /// Jitter configuration
    pub jitter: JitterConfig,
    /// Retry conditions
    pub retry_conditions: RetryConditions,
    /// Enable detailed logging
    pub detailed_logging: bool,
}

/// Backoff strategies for retry delays
#[derive(Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    /// Fixed delay between retries
    Fixed,
    /// Linear backoff (delay increases linearly)
    Linear { increment: Duration },
    /// Exponential backoff (delay doubles each time)
    Exponential { multiplier: f64 },
    /// Fibonacci backoff
    Fibonacci,
    /// Custom backoff function
    #[serde(skip)]
    Custom(Arc<dyn Fn(u32) -> Duration + Send + Sync>),
}

impl std::fmt::Debug for BackoffStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fixed => write!(f, "Fixed"),
            Self::Linear { increment } => f
                .debug_struct("Linear")
                .field("increment", increment)
                .finish(),
            Self::Exponential { multiplier } => f
                .debug_struct("Exponential")
                .field("multiplier", multiplier)
                .finish(),
            Self::Fibonacci => write!(f, "Fibonacci"),
            Self::Custom(_) => write!(f, "Custom(<function>)"),
        }
    }
}

/// Jitter configuration for retry delays
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitterConfig {
    /// Enable jitter
    pub enabled: bool,
    /// Jitter type
    pub jitter_type: JitterType,
    /// Jitter factor (0.0 to 1.0)
    pub jitter_factor: f64,
}

/// Types of jitter strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JitterType {
    /// Full jitter - delay = random(0, calculated_delay)
    Full,
    /// Equal jitter - delay = calculated_delay/2 + random(0, calculated_delay/2)
    Equal,
    /// Decorrelated jitter - uses previous delay to calculate next
    Decorrelated,
}

/// Conditions for retrying operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConditions {
    /// Retry on connection errors
    pub on_connection_error: bool,
    /// Retry on timeout errors
    pub on_timeout: bool,
    /// Retry on rate limit errors
    pub on_rate_limit: bool,
    /// Retry on service unavailable
    pub on_service_unavailable: bool,
    /// Custom error patterns to retry
    pub custom_patterns: Vec<String>,
    /// Error patterns to never retry
    pub never_retry_patterns: Vec<String>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::milliseconds(100),
            max_delay: Duration::seconds(30),
            backoff_strategy: BackoffStrategy::Exponential { multiplier: 2.0 },
            jitter: JitterConfig {
                enabled: true,
                jitter_type: JitterType::Equal,
                jitter_factor: 0.5,
            },
            retry_conditions: RetryConditions::default(),
            detailed_logging: true,
        }
    }
}

impl Default for RetryConditions {
    fn default() -> Self {
        Self {
            on_connection_error: true,
            on_timeout: true,
            on_rate_limit: true,
            on_service_unavailable: true,
            custom_patterns: Vec::new(),
            never_retry_patterns: vec![
                "authentication".to_string(),
                "invalid_input".to_string(),
                "permission_denied".to_string(),
            ],
        }
    }
}

impl RetryPolicy {
    /// Create a policy for critical operations (fewer retries, shorter delays)
    pub fn critical() -> Self {
        Self {
            max_attempts: 2,
            initial_delay: Duration::milliseconds(50),
            max_delay: Duration::seconds(5),
            backoff_strategy: BackoffStrategy::Linear {
                increment: Duration::milliseconds(100),
            },
            jitter: JitterConfig {
                enabled: true,
                jitter_type: JitterType::Full,
                jitter_factor: 0.3,
            },
            retry_conditions: RetryConditions {
                on_connection_error: true,
                on_timeout: true,
                on_rate_limit: false,
                on_service_unavailable: true,
                custom_patterns: Vec::new(),
                never_retry_patterns: vec!["authentication".to_string()],
            },
            detailed_logging: true,
        }
    }

    /// Create a policy for background operations (more retries, longer delays)
    pub fn background() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::seconds(1),
            max_delay: Duration::minutes(2),
            backoff_strategy: BackoffStrategy::Exponential { multiplier: 3.0 },
            jitter: JitterConfig {
                enabled: true,
                jitter_type: JitterType::Decorrelated,
                jitter_factor: 0.7,
            },
            retry_conditions: RetryConditions::default(),
            detailed_logging: false,
        }
    }

    /// Calculate delay for given attempt
    pub fn calculate_delay(&self, attempt: u32, previous_delay: Option<Duration>) -> Duration {
        let base_delay = match &self.backoff_strategy {
            BackoffStrategy::Fixed => self.initial_delay,
            BackoffStrategy::Linear { increment } => {
                self.initial_delay + (*increment * (attempt - 1) as i32)
            }
            BackoffStrategy::Exponential { multiplier } => {
                let ms = self.initial_delay.num_milliseconds() as f64
                    * multiplier.powi((attempt - 1) as i32);
                Duration::milliseconds(ms as i64)
            }
            BackoffStrategy::Fibonacci => {
                let fib = fibonacci(attempt);
                self.initial_delay * fib as i32
            }
            BackoffStrategy::Custom(f) => f(attempt),
        };

        // Cap at maximum delay
        let capped_delay = if base_delay > self.max_delay {
            self.max_delay
        } else {
            base_delay
        };

        // Apply jitter
        if self.jitter.enabled {
            self.apply_jitter(capped_delay, previous_delay)
        } else {
            capped_delay
        }
    }

    /// Apply jitter to delay
    fn apply_jitter(&self, delay: Duration, previous_delay: Option<Duration>) -> Duration {
        let mut rng = rand::thread_rng();
        let delay_ms = delay.num_milliseconds() as f64;

        let jittered_ms = match self.jitter.jitter_type {
            JitterType::Full => rng.gen_range(0.0..=delay_ms * self.jitter.jitter_factor),
            JitterType::Equal => {
                let half = delay_ms / 2.0;
                half + rng.gen_range(0.0..=half * self.jitter.jitter_factor)
            }
            JitterType::Decorrelated => {
                let prev_ms = previous_delay
                    .map(|d| d.num_milliseconds() as f64)
                    .unwrap_or(delay_ms);
                let min = self.initial_delay.num_milliseconds() as f64;
                let max = (delay_ms * 3.0).min(self.max_delay.num_milliseconds() as f64);
                rng.gen_range(min..=max).min(prev_ms * 3.0)
            }
        };

        Duration::milliseconds(jittered_ms as i64)
    }

    /// Check if error should be retried
    pub fn should_retry(&self, error: &LoxoneError) -> bool {
        let error_str = error.to_string().to_lowercase();

        // Check never retry patterns first
        for pattern in &self.retry_conditions.never_retry_patterns {
            if error_str.contains(&pattern.to_lowercase()) {
                return false;
            }
        }

        // Check standard retry conditions
        if self.retry_conditions.on_connection_error && error_str.contains("connection") {
            return true;
        }
        if self.retry_conditions.on_timeout && error_str.contains("timeout") {
            return true;
        }
        if self.retry_conditions.on_rate_limit && error_str.contains("rate") {
            return true;
        }
        if self.retry_conditions.on_service_unavailable && error_str.contains("unavailable") {
            return true;
        }

        // Check custom patterns
        for pattern in &self.retry_conditions.custom_patterns {
            if error_str.contains(&pattern.to_lowercase()) {
                return true;
            }
        }

        false
    }
}

/// Retry executor
pub struct RetryExecutor {
    policy: RetryPolicy,
    stats: Arc<RwLock<RetryStats>>,
}

/// Retry statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetryStats {
    /// Total operations attempted
    pub total_operations: u64,
    /// Successful operations (no retry needed)
    pub successful_first_attempt: u64,
    /// Successful operations (after retry)
    pub successful_after_retry: u64,
    /// Failed operations (all retries exhausted)
    pub failed_after_retries: u64,
    /// Total retry attempts
    pub total_retry_attempts: u64,
    /// Average retries per operation
    pub average_retries: f64,
    /// Last retry timestamp
    pub last_retry: Option<DateTime<Utc>>,
}

impl RetryExecutor {
    /// Create new retry executor
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            policy,
            stats: Arc::new(RwLock::new(RetryStats::default())),
        }
    }

    /// Execute operation with retry policy
    pub async fn execute<F, T, Fut>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        drop(stats);

        let mut attempt = 0;
        let mut previous_delay = None;
        let mut last_error;

        loop {
            attempt += 1;

            if self.policy.detailed_logging && attempt > 1 {
                debug!("Retry attempt {} of {}", attempt, self.policy.max_attempts);
            }

            match operation().await {
                Ok(result) => {
                    let mut stats = self.stats.write().await;
                    if attempt == 1 {
                        stats.successful_first_attempt += 1;
                    } else {
                        stats.successful_after_retry += 1;
                    }
                    stats.average_retries =
                        stats.total_retry_attempts as f64 / stats.total_operations as f64;
                    drop(stats);

                    if self.policy.detailed_logging && attempt > 1 {
                        info!("Operation succeeded after {} attempts", attempt);
                    }

                    return Ok(result);
                }
                Err(error) => {
                    last_error = Some(error);

                    if attempt >= self.policy.max_attempts {
                        let mut stats = self.stats.write().await;
                        stats.failed_after_retries += 1;
                        drop(stats);

                        warn!(
                            "Operation failed after {} attempts: {}",
                            attempt,
                            last_error.as_ref().unwrap()
                        );

                        return Err(last_error.unwrap());
                    }

                    if !self.policy.should_retry(last_error.as_ref().unwrap()) {
                        if self.policy.detailed_logging {
                            debug!("Error not retryable: {}", last_error.as_ref().unwrap());
                        }
                        return Err(last_error.unwrap());
                    }

                    let delay = self.policy.calculate_delay(attempt, previous_delay);
                    previous_delay = Some(delay);

                    let mut stats = self.stats.write().await;
                    stats.total_retry_attempts += 1;
                    stats.last_retry = Some(Utc::now());
                    drop(stats);

                    if self.policy.detailed_logging {
                        debug!(
                            "Retrying after {:?} (attempt {}/{})",
                            delay, attempt, self.policy.max_attempts
                        );
                    }

                    sleep(std::time::Duration::from_millis(
                        delay.num_milliseconds() as u64
                    ))
                    .await;
                }
            }
        }
    }

    /// Get retry statistics
    pub async fn get_stats(&self) -> RetryStats {
        self.stats.read().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        *self.stats.write().await = RetryStats::default();
    }
}

/// Calculate fibonacci number
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => {
            let mut a = 0;
            let mut b = 1;
            for _ in 2..=n {
                let temp = a + b;
                a = b;
                b = temp;
            }
            b
        }
    }
}

/// Retry builder for fluent API
pub struct RetryBuilder {
    policy: RetryPolicy,
}

impl RetryBuilder {
    /// Create new retry builder
    pub fn new() -> Self {
        Self {
            policy: RetryPolicy::default(),
        }
    }

    /// Set maximum attempts
    pub fn max_attempts(mut self, attempts: u32) -> Self {
        self.policy.max_attempts = attempts;
        self
    }

    /// Set initial delay
    pub fn initial_delay(mut self, delay: Duration) -> Self {
        self.policy.initial_delay = delay;
        self
    }

    /// Set backoff strategy
    pub fn backoff(mut self, strategy: BackoffStrategy) -> Self {
        self.policy.backoff_strategy = strategy;
        self
    }

    /// Enable jitter
    pub fn with_jitter(mut self, jitter_type: JitterType, factor: f64) -> Self {
        self.policy.jitter = JitterConfig {
            enabled: true,
            jitter_type,
            jitter_factor: factor,
        };
        self
    }

    /// Build retry executor
    pub fn build(self) -> RetryExecutor {
        RetryExecutor::new(self.policy)
    }
}

/// Helper macro for retrying operations
#[macro_export]
macro_rules! retry {
    ($policy:expr, $operation:expr) => {{
        let executor = RetryExecutor::new($policy);
        executor.execute(|| async { $operation }).await
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        let policy = RetryPolicy {
            initial_delay: Duration::milliseconds(100),
            backoff_strategy: BackoffStrategy::Exponential { multiplier: 2.0 },
            jitter: JitterConfig {
                enabled: false,
                jitter_type: JitterType::Full,
                jitter_factor: 0.0,
            },
            ..Default::default()
        };

        assert_eq!(policy.calculate_delay(1, None), Duration::milliseconds(100));
        assert_eq!(policy.calculate_delay(2, None), Duration::milliseconds(200));
        assert_eq!(policy.calculate_delay(3, None), Duration::milliseconds(400));
    }

    #[test]
    fn test_fibonacci_backoff() {
        let policy = RetryPolicy {
            initial_delay: Duration::milliseconds(100),
            backoff_strategy: BackoffStrategy::Fibonacci,
            jitter: JitterConfig {
                enabled: false,
                jitter_type: JitterType::Full,
                jitter_factor: 0.0,
            },
            ..Default::default()
        };

        assert_eq!(policy.calculate_delay(1, None), Duration::milliseconds(100));
        assert_eq!(policy.calculate_delay(2, None), Duration::milliseconds(100));
        assert_eq!(policy.calculate_delay(3, None), Duration::milliseconds(200));
        assert_eq!(policy.calculate_delay(4, None), Duration::milliseconds(300));
    }

    #[tokio::test]
    async fn test_retry_executor() {
        let attempt_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let executor = RetryBuilder::new()
            .max_attempts(3)
            .initial_delay(Duration::milliseconds(10))
            .build();

        let attempt_count_clone = attempt_count.clone();
        let result = executor
            .execute(move || {
                let count = attempt_count_clone.clone();
                async move {
                    let current = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    if current < 3 {
                        Err(LoxoneError::connection("Simulated failure"))
                    } else {
                        Ok("Success")
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Success");

        let stats = executor.get_stats().await;
        assert_eq!(stats.total_operations, 1);
        assert_eq!(stats.successful_after_retry, 1);
        assert_eq!(stats.total_retry_attempts, 2);
    }
}
