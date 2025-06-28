//! Metrics collection for observability and monitoring
//!
//! This module provides comprehensive metrics collection for:
//! - Request performance monitoring
//! - System health metrics
//! - Business logic metrics
//! - Error tracking and classification

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Global metrics collector instance
pub struct MetricsCollector {
    /// Request metrics
    request_metrics: Arc<RwLock<RequestMetrics>>,

    /// System health metrics
    health_metrics: Arc<RwLock<HealthMetrics>>,

    /// Business metrics
    business_metrics: Arc<RwLock<BusinessMetrics>>,

    /// Error metrics
    error_metrics: Arc<RwLock<ErrorMetrics>>,

    /// Start time for uptime calculation
    start_time: Instant,
}

/// Request performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetrics {
    /// Total number of requests
    pub total_requests: u64,

    /// Successful requests
    pub successful_requests: u64,

    /// Failed requests
    pub failed_requests: u64,

    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,

    /// 95th percentile response time
    pub p95_response_time_ms: f64,

    /// 99th percentile response time
    pub p99_response_time_ms: f64,

    /// Current active requests
    pub active_requests: u64,

    /// Requests per tool
    pub requests_by_tool: HashMap<String, u64>,

    /// Response times by tool
    pub response_times_by_tool: HashMap<String, Vec<f64>>,

    /// Rate limiting hits
    pub rate_limit_hits: u64,

    /// Request throughput (requests per second)
    pub requests_per_second: f64,

    /// Last update timestamp
    pub last_updated: u64,
}

/// System health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    /// CPU usage percentage
    pub cpu_usage_percent: Option<f64>,

    /// Memory usage in MB
    pub memory_usage_mb: Option<f64>,

    /// Memory usage percentage
    pub memory_usage_percent: Option<f64>,

    /// Disk usage percentage
    pub disk_usage_percent: Option<f64>,

    /// Network latency to Loxone server
    pub loxone_latency_ms: Option<f64>,

    /// Connection pool statistics
    pub connection_pool_active: Option<u32>,
    pub connection_pool_idle: Option<u32>,
    pub connection_pool_max: Option<u32>,

    /// System uptime in seconds
    pub uptime_seconds: u64,

    /// Garbage collection metrics (if available)
    pub gc_collections: Option<u64>,
    pub gc_time_ms: Option<f64>,

    /// Last health check status
    pub last_health_check_success: bool,
    pub last_health_check_time: u64,
}

/// Business logic metrics specific to Loxone MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessMetrics {
    /// Device operations
    pub device_operations_total: u64,
    pub device_operations_success: u64,
    pub device_operations_failed: u64,

    /// Operations by device type
    pub operations_by_device_type: HashMap<String, u64>,

    /// Operations by room
    pub operations_by_room: HashMap<String, u64>,

    /// Loxone API calls
    pub loxone_api_calls_total: u64,
    pub loxone_api_calls_success: u64,
    pub loxone_api_calls_failed: u64,

    /// Structure refresh operations
    pub structure_refreshes: u64,
    pub last_structure_refresh: u64,

    /// Authentication operations
    pub auth_attempts: u64,
    pub auth_successes: u64,
    pub auth_failures: u64,

    /// Cache hit/miss ratios
    pub cache_hits: u64,
    pub cache_misses: u64,

    /// Schema validation metrics
    pub schema_validations_total: u64,
    pub schema_validations_failed: u64,

    /// Request coalescing effectiveness
    pub coalesced_requests: u64,
    pub coalescing_time_saved_ms: f64,
}

/// Error classification and tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMetrics {
    /// Total errors
    pub total_errors: u64,

    /// Errors by classification
    pub client_errors: u64,
    pub server_errors: u64,
    pub network_errors: u64,
    pub auth_errors: u64,
    pub business_errors: u64,

    /// Errors by tool
    pub errors_by_tool: HashMap<String, u64>,

    /// Error rates
    pub error_rate_5min: f64,
    pub error_rate_1hour: f64,
    pub error_rate_24hour: f64,

    /// Most recent errors (last 10)
    pub recent_errors: Vec<ErrorRecord>,

    /// Error patterns
    pub timeout_errors: u64,
    pub connection_errors: u64,
    pub validation_errors: u64,
    pub device_control_errors: u64,
}

/// Individual error record for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub timestamp: u64,
    pub error_type: String,
    pub error_message: String,
    pub tool_name: String,
    pub request_id: String,
    pub duration_ms: u64,
}

impl Default for RequestMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            avg_response_time_ms: 0.0,
            p95_response_time_ms: 0.0,
            p99_response_time_ms: 0.0,
            active_requests: 0,
            requests_by_tool: HashMap::new(),
            response_times_by_tool: HashMap::new(),
            rate_limit_hits: 0,
            requests_per_second: 0.0,
            last_updated: current_timestamp(),
        }
    }
}

impl Default for HealthMetrics {
    fn default() -> Self {
        Self {
            cpu_usage_percent: None,
            memory_usage_mb: None,
            memory_usage_percent: None,
            disk_usage_percent: None,
            loxone_latency_ms: None,
            connection_pool_active: None,
            connection_pool_idle: None,
            connection_pool_max: None,
            uptime_seconds: 0,
            gc_collections: None,
            gc_time_ms: None,
            last_health_check_success: false,
            last_health_check_time: current_timestamp(),
        }
    }
}

impl Default for BusinessMetrics {
    fn default() -> Self {
        Self {
            device_operations_total: 0,
            device_operations_success: 0,
            device_operations_failed: 0,
            operations_by_device_type: HashMap::new(),
            operations_by_room: HashMap::new(),
            loxone_api_calls_total: 0,
            loxone_api_calls_success: 0,
            loxone_api_calls_failed: 0,
            structure_refreshes: 0,
            last_structure_refresh: 0,
            auth_attempts: 0,
            auth_successes: 0,
            auth_failures: 0,
            cache_hits: 0,
            cache_misses: 0,
            schema_validations_total: 0,
            schema_validations_failed: 0,
            coalesced_requests: 0,
            coalescing_time_saved_ms: 0.0,
        }
    }
}

impl Default for ErrorMetrics {
    fn default() -> Self {
        Self {
            total_errors: 0,
            client_errors: 0,
            server_errors: 0,
            network_errors: 0,
            auth_errors: 0,
            business_errors: 0,
            errors_by_tool: HashMap::new(),
            error_rate_5min: 0.0,
            error_rate_1hour: 0.0,
            error_rate_24hour: 0.0,
            recent_errors: Vec::new(),
            timeout_errors: 0,
            connection_errors: 0,
            validation_errors: 0,
            device_control_errors: 0,
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            request_metrics: Arc::new(RwLock::new(RequestMetrics::default())),
            health_metrics: Arc::new(RwLock::new(HealthMetrics::default())),
            business_metrics: Arc::new(RwLock::new(BusinessMetrics::default())),
            error_metrics: Arc::new(RwLock::new(ErrorMetrics::default())),
            start_time: Instant::now(),
        }
    }

    /// Record a request start
    pub async fn record_request_start(&self, tool_name: &str) {
        let mut metrics = self.request_metrics.write().await;
        metrics.total_requests += 1;
        metrics.active_requests += 1;
        *metrics
            .requests_by_tool
            .entry(tool_name.to_string())
            .or_insert(0) += 1;
        metrics.last_updated = current_timestamp();
    }

    /// Record a request completion
    pub async fn record_request_end(&self, tool_name: &str, duration: Duration, success: bool) {
        let duration_ms = duration.as_millis() as f64;
        let mut metrics = self.request_metrics.write().await;

        metrics.active_requests = metrics.active_requests.saturating_sub(1);

        if success {
            metrics.successful_requests += 1;
        } else {
            metrics.failed_requests += 1;
        }

        // Update response times
        metrics
            .response_times_by_tool
            .entry(tool_name.to_string())
            .or_insert_with(Vec::new)
            .push(duration_ms);

        // Keep only last 1000 response times per tool for memory efficiency
        if let Some(times) = metrics.response_times_by_tool.get_mut(tool_name) {
            if times.len() > 1000 {
                times.drain(..times.len() - 1000);
            }
        }

        // Recalculate averages and percentiles
        self.update_response_time_statistics(&mut metrics).await;
        metrics.last_updated = current_timestamp();
    }

    /// Record a rate limit hit
    pub async fn record_rate_limit_hit(&self) {
        let mut metrics = self.request_metrics.write().await;
        metrics.rate_limit_hits += 1;
        metrics.last_updated = current_timestamp();
    }

    /// Record an error
    pub async fn record_error(
        &self,
        tool_name: &str,
        request_id: &str,
        error: &crate::error::LoxoneError,
        duration: Duration,
    ) {
        let mut metrics = self.error_metrics.write().await;
        metrics.total_errors += 1;

        // Classify error
        match error {
            crate::error::LoxoneError::InvalidInput(_) => metrics.client_errors += 1,
            crate::error::LoxoneError::Authentication(_) => {
                metrics.auth_errors += 1;
                let mut business = self.business_metrics.write().await;
                business.auth_failures += 1;
            }
            crate::error::LoxoneError::Connection(_) => {
                metrics.network_errors += 1;
                metrics.connection_errors += 1;
            }
            crate::error::LoxoneError::Timeout(_) => {
                metrics.network_errors += 1;
                metrics.timeout_errors += 1;
            }
            crate::error::LoxoneError::DeviceControl(_) => {
                metrics.business_errors += 1;
                metrics.device_control_errors += 1;
            }
            _ => metrics.server_errors += 1,
        }

        // Track by tool
        *metrics
            .errors_by_tool
            .entry(tool_name.to_string())
            .or_insert(0) += 1;

        // Add to recent errors (keep last 10)
        let error_record = ErrorRecord {
            timestamp: current_timestamp(),
            error_type: format!("{error:?}"),
            error_message: error.to_string(),
            tool_name: tool_name.to_string(),
            request_id: request_id.to_string(),
            duration_ms: duration.as_millis() as u64,
        };

        metrics.recent_errors.push(error_record);
        if metrics.recent_errors.len() > 10 {
            metrics.recent_errors.remove(0);
        }
    }

    /// Record a device operation
    pub async fn record_device_operation(
        &self,
        device_type: Option<&str>,
        room_name: Option<&str>,
        success: bool,
    ) {
        let mut metrics = self.business_metrics.write().await;
        metrics.device_operations_total += 1;

        if success {
            metrics.device_operations_success += 1;
        } else {
            metrics.device_operations_failed += 1;
        }

        if let Some(dev_type) = device_type {
            *metrics
                .operations_by_device_type
                .entry(dev_type.to_string())
                .or_insert(0) += 1;
        }

        if let Some(room) = room_name {
            *metrics
                .operations_by_room
                .entry(room.to_string())
                .or_insert(0) += 1;
        }
    }

    /// Record a Loxone API call
    pub async fn record_loxone_api_call(&self, success: bool) {
        let mut metrics = self.business_metrics.write().await;
        metrics.loxone_api_calls_total += 1;

        if success {
            metrics.loxone_api_calls_success += 1;
        } else {
            metrics.loxone_api_calls_failed += 1;
        }
    }

    /// Record schema validation
    pub async fn record_schema_validation(&self, success: bool) {
        let mut metrics = self.business_metrics.write().await;
        metrics.schema_validations_total += 1;

        if !success {
            metrics.schema_validations_failed += 1;
        }
    }

    /// Update health metrics
    pub async fn update_health_metrics(
        &self,
        cpu_usage: Option<f64>,
        memory_usage_mb: Option<f64>,
        loxone_latency_ms: Option<f64>,
        health_check_success: bool,
    ) {
        let mut metrics = self.health_metrics.write().await;
        metrics.cpu_usage_percent = cpu_usage;
        metrics.memory_usage_mb = memory_usage_mb;
        metrics.loxone_latency_ms = loxone_latency_ms;
        metrics.uptime_seconds = self.start_time.elapsed().as_secs();
        metrics.last_health_check_success = health_check_success;
        metrics.last_health_check_time = current_timestamp();
    }

    /// Get comprehensive metrics snapshot
    pub async fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        let request_metrics = self.request_metrics.read().await.clone();
        let health_metrics = self.health_metrics.read().await.clone();
        let business_metrics = self.business_metrics.read().await.clone();
        let error_metrics = self.error_metrics.read().await.clone();

        MetricsSnapshot {
            request_metrics,
            health_metrics,
            business_metrics,
            error_metrics,
            snapshot_timestamp: current_timestamp(),
        }
    }

    /// Update response time statistics
    async fn update_response_time_statistics(&self, metrics: &mut RequestMetrics) {
        let mut all_times = Vec::new();
        for times in metrics.response_times_by_tool.values() {
            all_times.extend(times);
        }

        if !all_times.is_empty() {
            all_times
                .sort_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            // Calculate average
            metrics.avg_response_time_ms = all_times.iter().sum::<f64>() / all_times.len() as f64;

            // Calculate percentiles
            if all_times.len() >= 20 {
                let p95_idx = (all_times.len() as f64 * 0.95) as usize;
                let p99_idx = (all_times.len() as f64 * 0.99) as usize;
                metrics.p95_response_time_ms = all_times[p95_idx.min(all_times.len() - 1)];
                metrics.p99_response_time_ms = all_times[p99_idx.min(all_times.len() - 1)];
            }
        }
    }
}

/// Complete metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub request_metrics: RequestMetrics,
    pub health_metrics: HealthMetrics,
    pub business_metrics: BusinessMetrics,
    pub error_metrics: ErrorMetrics,
    pub snapshot_timestamp: u64,
}

impl MetricsSnapshot {
    /// Calculate error rate
    pub fn error_rate(&self) -> f64 {
        if self.request_metrics.total_requests == 0 {
            0.0
        } else {
            self.request_metrics.failed_requests as f64 / self.request_metrics.total_requests as f64
        }
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        1.0 - self.error_rate()
    }

    /// Get availability percentage
    pub fn availability_percentage(&self) -> f64 {
        if self.health_metrics.last_health_check_success {
            99.9 // Assume high availability if last check was successful
        } else {
            95.0 // Degraded if health check failed
        }
    }
}

/// Get current timestamp in seconds since Unix epoch
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Global metrics collector instance
static METRICS: once_cell::sync::Lazy<MetricsCollector> =
    once_cell::sync::Lazy::new(MetricsCollector::new);

/// Get the global metrics collector
pub fn get_metrics() -> &'static MetricsCollector {
    &METRICS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_metrics_collection() {
        let collector = MetricsCollector::new();

        // Record some operations
        collector.record_request_start("test_tool").await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        collector
            .record_request_end("test_tool", Duration::from_millis(10), true)
            .await;

        let snapshot = collector.get_metrics_snapshot().await;
        assert_eq!(snapshot.request_metrics.total_requests, 1);
        assert_eq!(snapshot.request_metrics.successful_requests, 1);
        assert!(snapshot.request_metrics.avg_response_time_ms > 0.0);
    }

    #[tokio::test]
    async fn test_error_recording() {
        let collector = MetricsCollector::new();
        let error = crate::error::LoxoneError::authentication("Test auth error");

        collector
            .record_error("test_tool", "req-123", &error, Duration::from_millis(100))
            .await;

        let snapshot = collector.get_metrics_snapshot().await;
        assert_eq!(snapshot.error_metrics.total_errors, 1);
        assert_eq!(snapshot.error_metrics.auth_errors, 1);
        assert_eq!(snapshot.error_metrics.recent_errors.len(), 1);
    }
}
