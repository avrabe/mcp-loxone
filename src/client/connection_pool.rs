//! Connection pool management for Loxone clients
//!
//! This module provides connection pooling and resource management to prevent
//! exhaustion of system resources and ensure efficient connection reuse.

use crate::error::{LoxoneError, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tracing::{debug, info, warn};

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of concurrent connections
    pub max_connections: usize,

    /// Maximum idle time before connection is closed
    pub idle_timeout: Duration,

    /// Connection timeout
    pub connection_timeout: Duration,

    /// Maximum lifetime of a connection
    pub max_lifetime: Duration,

    /// Minimum number of idle connections to maintain
    pub min_idle: usize,

    /// Maximum number of pending connection requests
    pub max_pending: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            idle_timeout: Duration::from_secs(300), // 5 minutes
            connection_timeout: Duration::from_secs(30),
            max_lifetime: Duration::from_secs(3600), // 1 hour
            min_idle: 1,
            max_pending: 50,
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    /// Total connections created
    pub total_created: u64,

    /// Current active connections
    pub active_connections: usize,

    /// Current idle connections
    pub idle_connections: usize,

    /// Total requests served
    pub requests_served: u64,

    /// Connection wait time (average)
    pub avg_wait_time_ms: u64,

    /// Number of timeouts
    pub timeouts: u64,

    /// Number of connection errors
    pub errors: u64,
}

// Add compatibility methods
impl PoolStats {
    #[inline]
    pub fn active(&self) -> usize {
        self.active_connections
    }

    #[inline]
    pub fn idle(&self) -> usize {
        self.idle_connections
    }
}

/// Connection metadata
#[derive(Debug)]
struct ConnectionMeta {
    /// When the connection was created
    created_at: Instant,

    /// Last time the connection was used
    last_used: Instant,

    /// Number of requests served by this connection
    requests_served: u64,

    /// Whether the connection is currently active
    active: bool,
}

/// Connection pool for managing HTTP connections
pub struct ConnectionPool {
    /// Pool configuration
    config: PoolConfig,

    /// Semaphore for limiting concurrent connections
    connection_semaphore: Arc<Semaphore>,

    /// Pool statistics
    stats: Arc<Mutex<PoolStats>>,

    /// Connection metadata
    connections: Arc<Mutex<Vec<ConnectionMeta>>>,

    /// Pending request queue size
    pending_requests: Arc<Mutex<usize>>,
}

/// Internal connection pool state (shared)
#[derive(Clone)]
struct PoolState {
    stats: Arc<Mutex<PoolStats>>,
    connections: Arc<Mutex<Vec<ConnectionMeta>>>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(config: PoolConfig) -> Self {
        let connection_semaphore = Arc::new(Semaphore::new(config.max_connections));

        Self {
            config,
            connection_semaphore,
            stats: Arc::new(Mutex::new(PoolStats::default())),
            connections: Arc::new(Mutex::new(Vec::new())),
            pending_requests: Arc::new(Mutex::new(0)),
        }
    }

    /// Acquire a connection permit
    pub async fn acquire(&self) -> Result<ConnectionPermit> {
        // Check pending queue limit
        {
            let mut pending = self.pending_requests.lock().await;
            if *pending >= self.config.max_pending {
                return Err(LoxoneError::connection("Connection pool queue is full"));
            }
            *pending += 1;
        }

        let start = Instant::now();

        // Try to acquire permit with timeout
        let permit = match tokio::time::timeout(
            self.config.connection_timeout,
            self.connection_semaphore.clone().acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                self.decrement_pending().await;
                return Err(LoxoneError::connection(
                    "Failed to acquire connection permit",
                ));
            }
            Err(_) => {
                self.decrement_pending().await;
                self.increment_timeouts().await;
                return Err(LoxoneError::connection("Connection pool timeout"));
            }
        };

        // Decrement pending
        self.decrement_pending().await;

        // Update statistics
        let wait_time = start.elapsed();
        self.update_wait_time(wait_time).await;

        // Clean up old connections
        self.cleanup_connections().await;

        // Create new connection metadata
        let meta = ConnectionMeta {
            created_at: Instant::now(),
            last_used: Instant::now(),
            requests_served: 0,
            active: true,
        };

        // Add to connections list
        {
            let mut connections = self.connections.lock().await;
            connections.push(meta);
        }

        // Update stats
        {
            let mut stats = self.stats.lock().await;
            stats.total_created += 1;
            stats.active_connections += 1;
        }

        Ok(ConnectionPermit {
            pool_state: PoolState {
                stats: self.stats.clone(),
                connections: self.connections.clone(),
            },
            _permit: permit,
            acquired_at: Instant::now(),
        })
    }

    /// Get current pool statistics
    pub async fn stats(&self) -> PoolStats {
        self.stats.lock().await.clone()
    }

    /// Check pool health
    pub async fn health_check(&self) -> PoolHealth {
        let stats = self.stats.lock().await.clone();
        let pending = *self.pending_requests.lock().await;

        let utilization = if self.config.max_connections > 0 {
            (stats.active_connections as f64 / self.config.max_connections as f64) * 100.0
        } else {
            0.0
        };

        let queue_pressure = if self.config.max_pending > 0 {
            (pending as f64 / self.config.max_pending as f64) * 100.0
        } else {
            0.0
        };

        let error_rate = if stats.requests_served > 0 {
            (stats.errors as f64 / stats.requests_served as f64) * 100.0
        } else {
            0.0
        };

        PoolHealth {
            healthy: utilization < 90.0 && queue_pressure < 80.0 && error_rate < 5.0,
            utilization,
            queue_pressure,
            error_rate,
            active_connections: stats.active_connections,
            idle_connections: stats.idle_connections,
            pending_requests: pending,
        }
    }

    /// Cleanup old connections
    async fn cleanup_connections(&self) {
        let mut connections = self.connections.lock().await;
        let now = Instant::now();

        connections.retain(|conn| {
            let age = now.duration_since(conn.created_at);
            let idle_time = now.duration_since(conn.last_used);

            // Remove if too old or idle too long
            if age > self.config.max_lifetime
                || (!conn.active && idle_time > self.config.idle_timeout)
            {
                debug!("Removing connection: age={:?}, idle={:?}", age, idle_time);
                false
            } else {
                true
            }
        });
    }

    async fn decrement_pending(&self) {
        let mut pending = self.pending_requests.lock().await;
        *pending = pending.saturating_sub(1);
    }

    async fn increment_timeouts(&self) {
        let mut stats = self.stats.lock().await;
        stats.timeouts += 1;
    }

    async fn update_wait_time(&self, duration: Duration) {
        let mut stats = self.stats.lock().await;
        stats.requests_served += 1;

        // Update average wait time (simple moving average)
        let new_wait_ms = duration.as_millis() as u64;
        if stats.avg_wait_time_ms == 0 {
            stats.avg_wait_time_ms = new_wait_ms;
        } else {
            stats.avg_wait_time_ms = (stats.avg_wait_time_ms * 9 + new_wait_ms) / 10;
        }
    }

    /// Release a connection back to the pool
    #[allow(dead_code)]
    async fn release(&self) {
        let mut stats = self.stats.lock().await;
        if stats.active_connections > 0 {
            stats.active_connections -= 1;
            stats.idle_connections += 1;
        }

        // Update connection metadata
        let mut connections = self.connections.lock().await;
        if let Some(conn) = connections.iter_mut().find(|c| c.active) {
            conn.active = false;
            conn.last_used = Instant::now();
            conn.requests_served += 1;
        }
    }

    /// Record an error
    pub async fn record_error(&self) {
        let mut stats = self.stats.lock().await;
        stats.errors += 1;
    }
}

/// Pool health information
#[derive(Debug, Clone)]
pub struct PoolHealth {
    /// Whether the pool is healthy
    pub healthy: bool,

    /// Connection utilization percentage
    pub utilization: f64,

    /// Queue pressure percentage
    pub queue_pressure: f64,

    /// Error rate percentage
    pub error_rate: f64,

    /// Active connections
    pub active_connections: usize,

    /// Idle connections
    pub idle_connections: usize,

    /// Pending requests
    pub pending_requests: usize,
}

/// Connection permit that must be held while using a connection
pub struct ConnectionPermit {
    /// Pool state for cleanup
    pool_state: PoolState,

    /// Semaphore permit
    _permit: tokio::sync::OwnedSemaphorePermit,

    /// When this permit was acquired
    acquired_at: Instant,
}

impl ConnectionPermit {
    /// Get the time this permit has been held
    pub fn held_duration(&self) -> Duration {
        self.acquired_at.elapsed()
    }
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        // Log if held too long
        let held_duration = self.held_duration();
        if held_duration > Duration::from_secs(30) {
            warn!(
                "Connection held for {:?}, consider optimization",
                held_duration
            );
        }

        // Release the connection back to the pool
        let stats = self.pool_state.stats.clone();
        let connections = self.pool_state.connections.clone();

        tokio::spawn(async move {
            // Update statistics
            let mut stats = stats.lock().await;
            if stats.active_connections > 0 {
                stats.active_connections -= 1;
                stats.idle_connections += 1;
            }

            // Update connection metadata
            let mut connections = connections.lock().await;
            if let Some(conn) = connections.iter_mut().find(|c| c.active) {
                conn.active = false;
                conn.last_used = Instant::now();
                conn.requests_served += 1;
            }
        });
    }
}

/// Connection pool builder
pub struct PoolBuilder {
    config: PoolConfig,
}

impl PoolBuilder {
    /// Create a new pool builder
    pub fn new() -> Self {
        Self {
            config: PoolConfig::default(),
        }
    }

    /// Set maximum connections
    pub fn max_connections(mut self, max: usize) -> Self {
        self.config.max_connections = max;
        self
    }

    /// Set idle timeout
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.config.idle_timeout = timeout;
        self
    }

    /// Set connection timeout
    pub fn connection_timeout(mut self, timeout: Duration) -> Self {
        self.config.connection_timeout = timeout;
        self
    }

    /// Set maximum lifetime
    pub fn max_lifetime(mut self, lifetime: Duration) -> Self {
        self.config.max_lifetime = lifetime;
        self
    }

    /// Set minimum idle connections
    pub fn min_idle(mut self, min: usize) -> Self {
        self.config.min_idle = min;
        self
    }

    /// Set maximum pending requests
    pub fn max_pending(mut self, max: usize) -> Self {
        self.config.max_pending = max;
        self
    }

    /// Build the connection pool
    pub fn build(self) -> ConnectionPool {
        info!("Creating connection pool with config: {:?}", self.config);
        ConnectionPool::new(self.config)
    }
}

impl Default for PoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_pool_basic() {
        let pool = PoolBuilder::new().max_connections(2).build();

        // Acquire first connection
        let permit1 = pool.acquire().await.unwrap();
        let stats = pool.stats().await;
        assert_eq!(stats.active_connections, 1);
        assert_eq!(stats.total_created, 1);

        // Acquire second connection
        let _permit2 = pool.acquire().await.unwrap();
        let stats = pool.stats().await;
        assert_eq!(stats.active_connections, 2);

        // Drop first permit
        drop(permit1);
        tokio::time::sleep(Duration::from_millis(10)).await;

        let stats = pool.stats().await;
        assert_eq!(stats.active_connections, 1);
        assert_eq!(stats.idle_connections, 1);

        // Can acquire another
        let _permit3 = pool.acquire().await.unwrap();
        let stats = pool.stats().await;
        assert_eq!(stats.active_connections, 2);
    }

    #[tokio::test]
    async fn test_pool_limits() {
        let pool = PoolBuilder::new()
            .max_connections(1)
            .connection_timeout(Duration::from_millis(100))
            .build();

        // Acquire the only connection
        let _permit = pool.acquire().await.unwrap();

        // Try to acquire another (should timeout)
        let start = Instant::now();
        let result = pool.acquire().await;
        assert!(result.is_err());
        assert!(start.elapsed() >= Duration::from_millis(100));

        let stats = pool.stats().await;
        assert_eq!(stats.timeouts, 1);
    }

    #[tokio::test]
    async fn test_pool_health() {
        let pool = PoolBuilder::new().max_connections(10).build();

        let health = pool.health_check().await;
        assert!(health.healthy);
        assert_eq!(health.utilization, 0.0);

        // Acquire some connections
        let _permits: Vec<_> = futures::future::try_join_all((0..5).map(|_| pool.acquire()))
            .await
            .unwrap();

        let health = pool.health_check().await;
        assert!(health.healthy);
        assert_eq!(health.utilization, 50.0);
    }
}
