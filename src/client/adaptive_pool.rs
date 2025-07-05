//! Adaptive connection pool with authentication negotiation
//!
//! This module provides a connection pool that can handle multiple authentication
//! types dynamically, supporting mixed client types and automatic fallback.

use crate::client::load_balancer::{LoadBalancer, LoadBalancingStrategy};
use crate::client::{
    client_factory::{AdaptiveClientFactory, ClientFactory, ServerCapabilities},
    LoxoneClient,
};
use crate::config::{credentials::LoxoneCredentials, AuthMethod, LoxoneConfig};
use crate::error::{LoxoneError, Result};
use crate::error_recovery::{CircuitBreaker, CircuitBreakerConfig};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Adaptive connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptivePoolConfig {
    /// Minimum number of connections to maintain
    pub min_connections: usize,
    /// Maximum number of connections allowed
    pub max_connections: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Idle timeout before connection is closed
    pub idle_timeout: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Enable automatic authentication fallback
    pub auto_fallback: bool,
    /// Enable connection warming
    pub warm_connections: bool,
    /// Enable per-connection circuit breakers
    pub circuit_breakers: bool,
    /// Load balancing strategy
    pub load_balancing_strategy: LoadBalancingStrategy,
    /// Health monitoring configuration
    pub health_monitoring: Option<crate::client::pool_health_monitor::HealthMonitorConfig>,
}

impl Default for AdaptivePoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 2,
            max_connections: 10,
            connection_timeout: Duration::seconds(30),
            idle_timeout: Duration::minutes(5),
            health_check_interval: Duration::seconds(30),
            auto_fallback: true,
            warm_connections: true,
            circuit_breakers: true,
            load_balancing_strategy: LoadBalancingStrategy::default(),
            health_monitoring: Some(
                crate::client::pool_health_monitor::HealthMonitorConfig::default(),
            ),
        }
    }
}

/// Adaptive connection wrapper
pub struct AdaptiveConnection {
    /// Unique connection ID
    pub id: String,
    /// The actual client
    pub client: Box<dyn LoxoneClient>,
    /// Authentication method used
    pub auth_method: AuthMethod,
    /// Server capabilities at connection time
    pub capabilities: ServerCapabilities,
    /// Connection metadata
    pub metadata: ConnectionMetadata,
    /// Circuit breaker (if enabled)
    pub circuit_breaker: Option<Arc<CircuitBreaker>>,
}

/// Connection metadata
#[derive(Debug, Clone)]
pub struct ConnectionMetadata {
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Last used timestamp
    pub last_used: Arc<RwLock<DateTime<Utc>>>,
    /// Last health check
    pub last_health_check: Arc<RwLock<DateTime<Utc>>>,
    /// Health status
    pub is_healthy: Arc<RwLock<bool>>,
    /// Total requests handled
    pub total_requests: Arc<AtomicU64>,
    /// Failed requests
    pub failed_requests: Arc<AtomicU64>,
    /// Active requests
    pub active_requests: Arc<AtomicUsize>,
}

impl ConnectionMetadata {
    fn new() -> Self {
        Self {
            created_at: Utc::now(),
            last_used: Arc::new(RwLock::new(Utc::now())),
            last_health_check: Arc::new(RwLock::new(Utc::now())),
            is_healthy: Arc::new(RwLock::new(true)),
            total_requests: Arc::new(AtomicU64::new(0)),
            failed_requests: Arc::new(AtomicU64::new(0)),
            active_requests: Arc::new(AtomicUsize::new(0)),
        }
    }

    async fn mark_used(&self) {
        *self.last_used.write().await = Utc::now();
        self.active_requests.fetch_add(1, Ordering::Relaxed);
    }

    fn mark_request_complete(&self, success: bool) {
        self.active_requests.fetch_sub(1, Ordering::Relaxed);
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if !success {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn get_success_rate(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Relaxed);
        if total == 0 {
            1.0
        } else {
            let failed = self.failed_requests.load(Ordering::Relaxed);
            ((total - failed) as f64) / (total as f64)
        }
    }
}

/// Adaptive connection pool
pub struct AdaptiveConnectionPool {
    /// Configuration
    config: AdaptivePoolConfig,
    /// Loxone configuration
    loxone_config: LoxoneConfig,
    /// Credentials
    credentials: LoxoneCredentials,
    /// Client factory
    client_factory: Arc<dyn ClientFactory>,
    /// Active connections
    connections: Arc<RwLock<Vec<Arc<AdaptiveConnection>>>>,
    /// Connection semaphore for limiting concurrent connections
    semaphore: Arc<Semaphore>,
    /// Pool statistics
    stats: Arc<RwLock<PoolStatistics>>,
    /// Load balancer for connection selection
    load_balancer: Arc<LoadBalancer>,
    /// Health monitor for metrics and alerting
    health_monitor: Arc<RwLock<Option<Arc<crate::client::pool_health_monitor::PoolHealthMonitor>>>>,
    /// Shutdown flag
    shutdown: Arc<RwLock<bool>>,
}

/// Pool statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PoolStatistics {
    /// Total connections created
    pub total_created: u64,
    /// Current active connections
    pub active_connections: usize,
    /// Authentication method distribution
    pub auth_distribution: HashMap<String, usize>,
    /// Total requests served
    pub total_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Average success rate
    pub success_rate: f64,
}

impl AdaptiveConnectionPool {
    /// Create new adaptive connection pool
    pub async fn new(
        config: AdaptivePoolConfig,
        loxone_config: LoxoneConfig,
        credentials: LoxoneCredentials,
        client_factory: Option<Arc<dyn ClientFactory>>,
    ) -> Result<Arc<Self>> {
        let factory = client_factory.unwrap_or_else(|| Arc::new(AdaptiveClientFactory::new()));
        let semaphore = Arc::new(Semaphore::new(config.max_connections));
        let load_balancer = Arc::new(LoadBalancer::new(config.load_balancing_strategy.clone()));

        let pool = Self {
            config: config.clone(),
            loxone_config,
            credentials,
            client_factory: factory,
            connections: Arc::new(RwLock::new(Vec::new())),
            semaphore,
            stats: Arc::new(RwLock::new(PoolStatistics::default())),
            load_balancer,
            health_monitor: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(false)),
        };

        // Warm up connections if enabled
        if config.warm_connections {
            pool.warm_up_connections().await?;
        }

        // Create Arc wrapper for the pool first
        let pool_arc = Arc::new(pool);

        // Initialize health monitoring if configured
        if let Some(health_config) = &config.health_monitoring {
            let health_monitor =
                Arc::new(crate::client::pool_health_monitor::PoolHealthMonitor::new(
                    pool_arc.clone(),
                    health_config.clone(),
                ));
            *pool_arc.health_monitor.write().await = Some(health_monitor.clone());

            if let Err(e) = health_monitor.start().await {
                warn!("Failed to start health monitoring: {}", e);
            } else {
                info!("Health monitoring started");
            }
        }

        // Start background tasks
        pool_arc.start_health_monitor();
        pool_arc.start_cleanup_task();

        Ok(pool_arc)
    }

    /// Warm up minimum connections
    async fn warm_up_connections(&self) -> Result<()> {
        info!("Warming up {} connections", self.config.min_connections);

        for i in 0..self.config.min_connections {
            match self.create_connection(None).await {
                Ok(conn) => {
                    let mut connections = self.connections.write().await;
                    connections.push(conn);
                }
                Err(e) => {
                    warn!("Failed to create connection {}: {}", i, e);
                    // Continue with other connections
                }
            }
        }

        Ok(())
    }

    /// Create a new connection
    async fn create_connection(
        &self,
        preferred_method: Option<AuthMethod>,
    ) -> Result<Arc<AdaptiveConnection>> {
        // Discover capabilities and create client
        let (client, auth_method) = self
            .client_factory
            .create_client(&self.loxone_config, &self.credentials, preferred_method)
            .await?;

        let capabilities = self
            .client_factory
            .get_cached_capabilities()
            .unwrap_or_else(|| ServerCapabilities {
                supports_basic_auth: true,
                supports_token_auth: false,
                supports_websocket: false,
                server_version: None,
                encryption_level: crate::client::client_factory::EncryptionLevel::None,
                discovered_at: Utc::now(),
            });

        // Create circuit breaker if enabled
        let circuit_breaker = if self.config.circuit_breakers {
            Some(Arc::new(CircuitBreaker::new(
                CircuitBreakerConfig::default(),
            )))
        } else {
            None
        };

        let connection = Arc::new(AdaptiveConnection {
            id: format!(
                "conn-{:?}-{}",
                auth_method,
                Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ),
            client,
            auth_method,
            capabilities,
            metadata: ConnectionMetadata::new(),
            circuit_breaker,
        });

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_created += 1;
        stats.active_connections += 1;
        *stats
            .auth_distribution
            .entry(format!("{auth_method:?}"))
            .or_insert(0) += 1;

        info!(
            "Created connection {} with auth method {:?}",
            connection.id, auth_method
        );

        Ok(connection)
    }

    /// Get a connection from the pool
    pub async fn get_connection(&self) -> Result<AdaptiveConnectionGuard> {
        self.get_connection_with_session(None).await
    }

    /// Get a connection from the pool with optional session key for sticky sessions
    pub async fn get_connection_with_session(
        &self,
        session_key: Option<&str>,
    ) -> Result<AdaptiveConnectionGuard> {
        // Acquire semaphore permit
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| LoxoneError::internal("Failed to acquire connection permit"))?;

        // Get available healthy connections
        let available_connections = {
            let connections = self.connections.read().await;
            let mut available = Vec::new();

            for conn in connections.iter() {
                let is_healthy = *conn.metadata.is_healthy.read().await;

                if is_healthy {
                    // Check circuit breaker if present
                    let can_use = if let Some(ref cb) = conn.circuit_breaker {
                        cb.should_allow_request().await
                    } else {
                        true
                    };

                    if can_use {
                        available.push(conn.clone());
                    }
                }
            }

            available
        };

        // Use load balancer to select the best connection
        let connection = self
            .load_balancer
            .select_connection(&available_connections, session_key)
            .await;

        let connection = if let Some(conn) = connection {
            conn
        } else {
            // No healthy connection available, create a new one if possible
            let connections = self.connections.read().await;
            if connections.len() >= self.config.max_connections {
                return Err(LoxoneError::resource_exhausted("Connection pool exhausted"));
            }
            drop(connections);

            // Create new connection
            let new_conn = self.create_connection(None).await?;
            let mut connections = self.connections.write().await;
            connections.push(new_conn.clone());
            new_conn
        };

        // Mark connection as used
        connection.metadata.mark_used().await;

        Ok(AdaptiveConnectionGuard {
            connection,
            _permit: permit,
            pool_load_balancer: self.load_balancer.clone(),
            start_time: std::time::Instant::now(),
        })
    }

    /// Start health monitoring task
    fn start_health_monitor(&self) {
        let connections = self.connections.clone();
        let interval_duration = self.config.health_check_interval;
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            let mut interval = interval(std::time::Duration::from_millis(
                interval_duration.num_milliseconds() as u64,
            ));

            loop {
                interval.tick().await;

                if *shutdown.read().await {
                    break;
                }

                let conns = connections.read().await.clone();
                for conn in conns {
                    Self::check_connection_health(&conn).await;
                }
            }
        });
    }

    /// Check connection health
    async fn check_connection_health(connection: &Arc<AdaptiveConnection>) {
        let start = Utc::now();

        match connection.client.health_check().await {
            Ok(healthy) => {
                *connection.metadata.is_healthy.write().await = healthy;
                *connection.metadata.last_health_check.write().await = Utc::now();

                if !healthy {
                    warn!("Connection {} health check failed", connection.id);
                }
            }
            Err(e) => {
                error!("Health check error for connection {}: {}", connection.id, e);
                *connection.metadata.is_healthy.write().await = false;
                *connection.metadata.last_health_check.write().await = Utc::now();
            }
        }

        debug!(
            "Health check for {} took {:?}",
            connection.id,
            Utc::now() - start
        );
    }

    /// Start cleanup task for idle connections
    fn start_cleanup_task(&self) {
        let connections = self.connections.clone();
        let config = self.config.clone();
        let stats = self.stats.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            let mut interval = interval(std::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                if *shutdown.read().await {
                    break;
                }

                let mut conns = connections.write().await;
                let now = Utc::now();
                let min_connections = config.min_connections;

                // Remove idle connections over the minimum
                if conns.len() > min_connections {
                    let min_connections = config.min_connections;
                    let current_len = conns.len();
                    conns.retain(|conn| {
                        let should_keep = tokio::task::block_in_place(|| {
                            let last_used = *conn.metadata.last_used.blocking_read();
                            let is_idle = now - last_used > config.idle_timeout;
                            let active = conn.metadata.active_requests.load(Ordering::Relaxed);

                            // Keep if active or not idle, or if we're at minimum
                            active > 0 || !is_idle || current_len <= min_connections
                        });

                        if !should_keep {
                            info!("Removing idle connection: {}", conn.id);
                            let mut stats = stats.blocking_write();
                            stats.active_connections -= 1;
                            if let Some(count) = stats
                                .auth_distribution
                                .get_mut(&format!("{:?}", conn.auth_method))
                            {
                                *count = count.saturating_sub(1);
                            }
                        }

                        should_keep
                    });
                }
            }
        });
    }

    /// Get pool statistics
    pub async fn get_stats(&self) -> PoolStatistics {
        let mut stats = self.stats.read().await.clone();

        // Calculate current success rate
        let connections = self.connections.read().await;
        let total_success_rate: f64 = connections
            .iter()
            .map(|c| c.metadata.get_success_rate())
            .sum::<f64>()
            / connections.len().max(1) as f64;

        stats.success_rate = total_success_rate;
        stats.active_connections = connections.len();

        stats
    }

    /// Get load balancing statistics
    pub async fn get_load_balancing_stats(
        &self,
    ) -> crate::client::load_balancer::LoadBalancingStatistics {
        self.load_balancer.get_statistics().await
    }

    /// Record performance metrics for load balancing
    pub async fn record_performance(
        &self,
        connection_id: &str,
        response_time_ms: u64,
        success: bool,
    ) {
        self.load_balancer
            .record_performance(connection_id, response_time_ms, success)
            .await;
    }

    /// Get health monitor (if enabled)
    pub async fn get_health_monitor(
        &self,
    ) -> Option<Arc<crate::client::pool_health_monitor::PoolHealthMonitor>> {
        self.health_monitor.read().await.clone()
    }

    /// Get current health metrics
    pub async fn get_health_metrics(
        &self,
    ) -> Option<crate::client::pool_health_monitor::HealthMetrics> {
        if let Some(monitor) = self.get_health_monitor().await {
            monitor.get_current_metrics().await.ok()
        } else {
            None
        }
    }

    /// Subscribe to health alerts
    pub async fn subscribe_to_health_alerts(
        &self,
    ) -> Option<tokio::sync::broadcast::Receiver<crate::client::pool_health_monitor::HealthAlert>>
    {
        self.get_health_monitor()
            .await
            .map(|monitor| monitor.subscribe_to_alerts())
    }

    /// Generate health report
    pub async fn generate_health_report(&self) -> Option<String> {
        if let Some(monitor) = self.get_health_monitor().await {
            monitor.generate_health_report().await.ok()
        } else {
            None
        }
    }

    /// Shutdown the pool
    pub async fn shutdown(&self) {
        info!("Shutting down adaptive connection pool");
        *self.shutdown.write().await = true;

        // Stop health monitor if running
        if let Some(monitor) = self.get_health_monitor().await {
            monitor.stop().await;
        }

        // Clear all connections
        let mut connections = self.connections.write().await;
        connections.clear();
    }
}

/// Connection guard that returns connection to pool when dropped
pub struct AdaptiveConnectionGuard {
    connection: Arc<AdaptiveConnection>,
    _permit: tokio::sync::OwnedSemaphorePermit,
    pool_load_balancer: Arc<LoadBalancer>,
    start_time: std::time::Instant,
}

impl AdaptiveConnectionGuard {
    /// Get the client
    pub fn client(&self) -> &dyn LoxoneClient {
        self.connection.client.as_ref()
    }

    /// Get authentication method used
    pub fn auth_method(&self) -> AuthMethod {
        self.connection.auth_method
    }

    /// Get connection ID
    pub fn connection_id(&self) -> &str {
        &self.connection.id
    }

    /// Get server capabilities
    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.connection.capabilities
    }

    /// Record operation result
    pub async fn record_result(&self, success: bool) {
        self.connection.metadata.mark_request_complete(success);

        // Record performance metrics for load balancing
        let response_time_ms = self.start_time.elapsed().as_millis() as u64;
        self.pool_load_balancer
            .record_performance(&self.connection.id, response_time_ms, success)
            .await;

        // Update circuit breaker if present
        if let Some(ref cb) = self.connection.circuit_breaker {
            if success {
                cb.record_success().await;
            } else {
                cb.record_failure(&LoxoneError::internal("Operation failed"))
                    .await;
            }
        }
    }
}

impl Drop for AdaptiveConnectionGuard {
    fn drop(&mut self) {
        self.connection
            .metadata
            .active_requests
            .fetch_sub(1, Ordering::Relaxed);
    }
}

/// Builder for adaptive connection pool
pub struct AdaptivePoolBuilder {
    config: AdaptivePoolConfig,
    loxone_config: Option<LoxoneConfig>,
    credentials: Option<LoxoneCredentials>,
    client_factory: Option<Arc<dyn ClientFactory>>,
}

impl Default for AdaptivePoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptivePoolBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            config: AdaptivePoolConfig::default(),
            loxone_config: None,
            credentials: None,
            client_factory: None,
        }
    }

    /// Set Loxone configuration
    pub fn loxone_config(mut self, config: LoxoneConfig) -> Self {
        self.loxone_config = Some(config);
        self
    }

    /// Set credentials
    pub fn credentials(mut self, credentials: LoxoneCredentials) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Set custom client factory
    pub fn client_factory(mut self, factory: Arc<dyn ClientFactory>) -> Self {
        self.client_factory = Some(factory);
        self
    }

    /// Set minimum connections
    pub fn min_connections(mut self, min: usize) -> Self {
        self.config.min_connections = min;
        self
    }

    /// Set maximum connections
    pub fn max_connections(mut self, max: usize) -> Self {
        self.config.max_connections = max;
        self
    }

    /// Enable/disable auto fallback
    pub fn auto_fallback(mut self, enabled: bool) -> Self {
        self.config.auto_fallback = enabled;
        self
    }

    /// Set load balancing strategy
    pub fn load_balancing_strategy(mut self, strategy: LoadBalancingStrategy) -> Self {
        self.config.load_balancing_strategy = strategy;
        self
    }

    /// Enable health monitoring with configuration
    pub fn health_monitoring(
        mut self,
        config: crate::client::pool_health_monitor::HealthMonitorConfig,
    ) -> Self {
        self.config.health_monitoring = Some(config);
        self
    }

    /// Disable health monitoring
    pub fn disable_health_monitoring(mut self) -> Self {
        self.config.health_monitoring = None;
        self
    }

    /// Build the pool
    pub async fn build(self) -> Result<Arc<AdaptiveConnectionPool>> {
        let loxone_config = self
            .loxone_config
            .ok_or_else(|| LoxoneError::config("Loxone configuration required"))?;
        let credentials = self
            .credentials
            .ok_or_else(|| LoxoneError::config("Credentials required"))?;

        let pool = AdaptiveConnectionPool::new(
            self.config,
            loxone_config,
            credentials,
            self.client_factory,
        )
        .await?;

        Ok(pool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adaptive_pool_builder() {
        let config = LoxoneConfig {
            url: "http://localhost"
                .parse()
                .expect("Test URL should be valid"),
            ..Default::default()
        };
        let credentials = LoxoneCredentials {
            username: "test".to_string(),
            password: "test".to_string(),
            api_key: None,
            #[cfg(feature = "crypto-openssl")]
            public_key: None,
        };

        let builder = AdaptivePoolBuilder::new()
            .loxone_config(config)
            .credentials(credentials)
            .min_connections(0)
            .max_connections(5)
            .load_balancing_strategy(LoadBalancingStrategy::RoundRobin);

        // Just test that builder configuration works
        assert_eq!(builder.config.min_connections, 0);
        assert_eq!(builder.config.max_connections, 5);
        matches!(
            builder.config.load_balancing_strategy,
            LoadBalancingStrategy::RoundRobin
        );
    }
}
