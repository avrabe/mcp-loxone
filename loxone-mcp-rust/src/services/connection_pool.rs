//! High-performance connection pooling for ultra-fast dashboard loads
//!
//! This module provides optimized connection pooling and request batching
//! to minimize latency and achieve <100ms dashboard response times.

use crate::client::LoxoneClient;
use crate::error::{LoxoneError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock, Semaphore};
use tracing::{debug, info, warn};

/// High-performance connection pool with request batching
pub struct ConnectionPool {
    /// Active connections
    connections: Arc<RwLock<Vec<Arc<dyn LoxoneClient>>>>,
    /// Connection semaphore for limiting concurrent connections
    semaphore: Arc<Semaphore>,
    /// Request queue for batching
    request_queue: Arc<Mutex<RequestQueue>>,
    /// Pool configuration
    config: PoolConfig,
    /// Pool metrics
    metrics: Arc<RwLock<PoolMetrics>>,
    /// Background task handles
    _task_handles: Vec<tokio::task::JoinHandle<()>>,
}

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections
    pub max_connections: usize,
    /// Minimum number of idle connections to maintain
    pub min_idle: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Batch size for grouping requests
    pub batch_size: usize,
    /// Batch timeout for forcing execution
    pub batch_timeout: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_idle: 2,
            connection_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_millis(100), // 100ms timeout for dashboard
            batch_size: 20,
            batch_timeout: Duration::from_millis(10), // 10ms batch window
            health_check_interval: Duration::from_secs(30),
        }
    }
}

/// Pool performance metrics
#[derive(Debug, Clone, Default)]
pub struct PoolMetrics {
    /// Total requests processed
    pub total_requests: u64,
    /// Requests served from batch
    pub batched_requests: u64,
    /// Average request time in microseconds
    pub avg_request_time_us: u64,
    /// Connection pool utilization percentage
    pub pool_utilization_percent: f32,
    /// Active connections
    pub active_connections: usize,
    /// Idle connections
    pub idle_connections: usize,
    /// Failed requests
    pub failed_requests: u64,
    /// Batch efficiency (requests per batch)
    pub avg_batch_size: f32,
}

/// Request queue for batching operations
#[derive(Default)]
struct RequestQueue {
    /// Pending batch requests
    pending: Vec<BatchRequest>,
    /// Last batch execution time
    last_batch_at: Option<Instant>,
}

/// Batched request for efficient processing
#[derive(Debug)]
struct BatchRequest {
    /// Device UUIDs to query
    pub uuids: Vec<String>,
    /// Response sender
    pub response_tx: mpsc::UnboundedSender<Result<HashMap<String, serde_json::Value>>>,
    /// Request timestamp
    #[allow(dead_code)]
    pub created_at: Instant,
}

impl ConnectionPool {
    /// Create new high-performance connection pool
    pub async fn new(
        config: PoolConfig,
        connection_factory: impl Fn() -> Result<Arc<dyn LoxoneClient>> + Send + Sync + 'static,
    ) -> Result<Self> {
        let connections = Arc::new(RwLock::new(Vec::new()));
        let semaphore = Arc::new(Semaphore::new(config.max_connections));
        let request_queue = Arc::new(Mutex::new(RequestQueue::default()));
        let metrics = Arc::new(RwLock::new(PoolMetrics::default()));

        // Pre-populate with minimum idle connections
        {
            let mut conn_guard = connections.write().await;
            for _ in 0..config.min_idle {
                match connection_factory() {
                    Ok(conn) => conn_guard.push(conn),
                    Err(e) => {
                        warn!("Failed to create initial connection: {}", e);
                        break;
                    }
                }
            }
        }

        // Start background batch processor
        let batch_task = Self::start_batch_processor(
            request_queue.clone(),
            connections.clone(),
            config.clone(),
            metrics.clone(),
        );

        // Start health check task
        let health_task = Self::start_health_checker(
            connections.clone(),
            config.clone(),
            metrics.clone(),
        );

        Ok(Self {
            connections,
            semaphore,
            request_queue,
            config,
            metrics,
            _task_handles: vec![batch_task, health_task],
        })
    }

    /// Execute batch request with aggressive optimization
    pub async fn execute_batch(&self, uuids: Vec<String>) -> Result<HashMap<String, serde_json::Value>> {
        let start = Instant::now();
        
        // Create response channel
        let (response_tx, mut response_rx) = mpsc::unbounded_channel();
        
        // Add to batch queue
        {
            let mut queue = self.request_queue.lock().await;
            queue.pending.push(BatchRequest {
                uuids,
                response_tx,
                created_at: start,
            });
            
            // Force batch execution if queue is full
            if queue.pending.len() >= self.config.batch_size {
                self.execute_pending_batch(&mut queue).await;
            }
        }
        
        // Wait for response with timeout
        match tokio::time::timeout(self.config.request_timeout, response_rx.recv()).await {
            Ok(Some(result)) => {
                let elapsed = start.elapsed();
                self.update_metrics(elapsed, true).await;
                result
            }
            Ok(None) => {
                self.update_metrics(start.elapsed(), false).await;
                Err(LoxoneError::connection("Response channel closed"))
            }
            Err(_) => {
                self.update_metrics(start.elapsed(), false).await;
                Err(LoxoneError::timeout("Batch request timeout"))
            }
        }
    }

    /// Start background batch processor for maximum throughput
    fn start_batch_processor(
        request_queue: Arc<Mutex<RequestQueue>>,
        connections: Arc<RwLock<Vec<Arc<dyn LoxoneClient>>>>,
        config: PoolConfig,
        metrics: Arc<RwLock<PoolMetrics>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.batch_timeout);
            
            loop {
                interval.tick().await;
                
                let mut queue = request_queue.lock().await;
                
                // Execute batch if there are pending requests
                if !queue.pending.is_empty() {
                    // Check if batch timeout has elapsed
                    let should_execute = match queue.last_batch_at {
                        Some(last) => last.elapsed() >= config.batch_timeout,
                        None => true,
                    };
                    
                    if should_execute || queue.pending.len() >= config.batch_size {
                        Self::execute_pending_batch_static(&mut queue, &connections, &metrics).await;
                    }
                }
            }
        })
    }

    /// Execute pending batch requests
    async fn execute_pending_batch(&self, queue: &mut RequestQueue) {
        Self::execute_pending_batch_static(queue, &self.connections, &self.metrics).await;
    }

    /// Static version for background task
    async fn execute_pending_batch_static(
        queue: &mut RequestQueue,
        connections: &Arc<RwLock<Vec<Arc<dyn LoxoneClient>>>>,
        metrics: &Arc<RwLock<PoolMetrics>>,
    ) {
        if queue.pending.is_empty() {
            return;
        }

        debug!("Executing batch of {} requests", queue.pending.len());
        
        // Collect all UUIDs from pending requests
        let mut all_uuids = Vec::new();
        let mut response_channels = Vec::new();
        
        for request in queue.pending.drain(..) {
            all_uuids.extend(request.uuids);
            response_channels.push(request.response_tx);
        }
        
        // Remove duplicates
        all_uuids.sort();
        all_uuids.dedup();
        
        // Execute batch request
        let result = Self::execute_batch_request(&all_uuids, connections).await;
        
        // Send results to all waiting requests
        match &result {
            Ok(data) => {
                for tx in response_channels {
                    let _ = tx.send(Ok(data.clone()));
                }
            }
            Err(e) => {
                // Create a new error for each channel since LoxoneError doesn't implement Clone
                for tx in response_channels {
                    let error_msg = e.to_string();
                    let _ = tx.send(Err(crate::error::LoxoneError::connection(error_msg)));
                }
            }
        }
        
        // Update metrics
        {
            let mut metrics_guard = metrics.write().await;
            metrics_guard.batched_requests += queue.pending.len() as u64;
            metrics_guard.avg_batch_size = if metrics_guard.batched_requests > 0 {
                metrics_guard.total_requests as f32 / metrics_guard.batched_requests as f32
            } else {
                0.0
            };
        }
        
        queue.last_batch_at = Some(Instant::now());
    }

    /// Execute batch request against connection pool
    async fn execute_batch_request(
        uuids: &[String],
        connections: &Arc<RwLock<Vec<Arc<dyn LoxoneClient>>>>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let connections_guard = connections.read().await;
        
        if connections_guard.is_empty() {
            return Err(LoxoneError::connection("No available connections"));
        }
        
        // Use first available connection (could implement round-robin)
        let connection = connections_guard[0].clone();
        drop(connections_guard);
        
        // Execute batch request
        connection.get_device_states(uuids).await
    }

    /// Start health checker for connection maintenance
    fn start_health_checker(
        connections: Arc<RwLock<Vec<Arc<dyn LoxoneClient>>>>,
        config: PoolConfig,
        metrics: Arc<RwLock<PoolMetrics>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.health_check_interval);
            
            loop {
                interval.tick().await;
                
                let mut connections_guard = connections.write().await;
                let mut active_count = 0;
                
                // Health check all connections
                let mut healthy_connections = Vec::new();
                for connection in connections_guard.drain(..) {
                    match connection.health_check().await {
                        Ok(true) => {
                            healthy_connections.push(connection);
                            active_count += 1;
                        }
                        Ok(false) | Err(_) => {
                            debug!("Removing unhealthy connection from pool");
                        }
                    }
                }
                
                *connections_guard = healthy_connections;
                
                // Update metrics
                {
                    let mut metrics_guard = metrics.write().await;
                    metrics_guard.active_connections = active_count;
                    metrics_guard.idle_connections = active_count; // Simplified
                    metrics_guard.pool_utilization_percent = 
                        (active_count as f32 / config.max_connections as f32) * 100.0;
                }
                
                info!("Health check completed: {} healthy connections", active_count);
            }
        })
    }

    /// Update performance metrics
    async fn update_metrics(&self, elapsed: Duration, success: bool) {
        let mut metrics = self.metrics.write().await;
        
        metrics.total_requests += 1;
        
        if !success {
            metrics.failed_requests += 1;
        }
        
        let elapsed_us = elapsed.as_micros() as u64;
        
        // Update average request time
        if metrics.total_requests == 1 {
            metrics.avg_request_time_us = elapsed_us;
        } else {
            metrics.avg_request_time_us = 
                (metrics.avg_request_time_us * (metrics.total_requests - 1) + elapsed_us) / metrics.total_requests;
        }
    }

    /// Get pool metrics
    pub async fn get_metrics(&self) -> PoolMetrics {
        self.metrics.read().await.clone()
    }

    /// Get pool status
    pub async fn get_status(&self) -> PoolStatus {
        let connections_count = self.connections.read().await.len();
        let metrics = self.get_metrics().await;
        
        PoolStatus {
            total_connections: connections_count,
            available_permits: self.semaphore.available_permits(),
            avg_request_time_ms: metrics.avg_request_time_us as f32 / 1000.0,
            success_rate_percent: if metrics.total_requests > 0 {
                ((metrics.total_requests - metrics.failed_requests) as f32 / metrics.total_requests as f32) * 100.0
            } else {
                100.0
            },
            batch_efficiency: metrics.avg_batch_size,
        }
    }
}

/// Pool status information
#[derive(Debug, Clone)]
pub struct PoolStatus {
    pub total_connections: usize,
    pub available_permits: usize,
    pub avg_request_time_ms: f32,
    pub success_rate_percent: f32,
    pub batch_efficiency: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    // use crate::client::http_client::LoxoneHttpClient;
    // use crate::config::{LoxoneConfig, credentials::LoxoneCredentials};

    #[tokio::test]
    async fn test_connection_pool_creation() {
        let config = PoolConfig::default();
        
        let pool_result = ConnectionPool::new(config, || {
            Err(LoxoneError::connection("Test error"))
        }).await;
        
        assert!(pool_result.is_ok());
    }

    #[tokio::test]
    async fn test_batch_request_timeout() {
        let config = PoolConfig {
            request_timeout: Duration::from_millis(10),
            ..Default::default()
        };
        
        let pool = ConnectionPool::new(config, || {
            Err(LoxoneError::connection("No connections"))
        }).await.unwrap();
        
        let result = pool.execute_batch(vec!["test-uuid".to_string()]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_metrics_update() {
        let config = PoolConfig::default();
        let pool = ConnectionPool::new(config, || {
            Err(LoxoneError::connection("Test"))
        }).await.unwrap();
        
        pool.update_metrics(Duration::from_millis(50), true).await;
        pool.update_metrics(Duration::from_millis(100), false).await;
        
        let metrics = pool.get_metrics().await;
        assert_eq!(metrics.total_requests, 2);
        assert_eq!(metrics.failed_requests, 1);
        assert!(metrics.avg_request_time_us > 0);
    }
}