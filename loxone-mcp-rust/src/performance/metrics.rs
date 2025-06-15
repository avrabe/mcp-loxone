//! Metrics collection and aggregation system

use crate::error::{LoxoneError, Result};
use crate::performance::{PerformanceContext, ResourceUsage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Metrics collection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,
    /// Collection interval
    pub collection_interval: Duration,
    /// Retention period for metrics
    pub retention_period: Duration,
    /// Maximum number of metrics to store in memory
    pub max_memory_metrics: usize,
    /// Metrics to collect
    pub collected_metrics: Vec<MetricType>,
    /// Resource monitoring configuration
    pub resource_monitoring: ResourceMonitoringConfig,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collection_interval: Duration::from_secs(5),
            retention_period: Duration::from_secs(3600), // 1 hour
            max_memory_metrics: 10000,
            collected_metrics: vec![
                MetricType::RequestLatency,
                MetricType::ThroughputRps,
                MetricType::ErrorRate,
                MetricType::CpuUsage,
                MetricType::MemoryUsage,
            ],
            resource_monitoring: ResourceMonitoringConfig::default(),
        }
    }
}

impl MetricsConfig {
    /// Production configuration with optimized settings
    pub fn production() -> Self {
        Self {
            enabled: true,
            collection_interval: Duration::from_secs(10),
            retention_period: Duration::from_secs(7200), // 2 hours
            max_memory_metrics: 5000,
            collected_metrics: vec![
                MetricType::RequestLatency,
                MetricType::ThroughputRps,
                MetricType::ErrorRate,
                MetricType::CpuUsage,
                MetricType::MemoryUsage,
            ],
            resource_monitoring: ResourceMonitoringConfig::production(),
        }
    }

    /// Development configuration with detailed monitoring
    pub fn development() -> Self {
        Self {
            enabled: true,
            collection_interval: Duration::from_secs(1),
            retention_period: Duration::from_secs(1800), // 30 minutes
            max_memory_metrics: 20000,
            collected_metrics: vec![
                MetricType::RequestLatency,
                MetricType::ThroughputRps,
                MetricType::ErrorRate,
                MetricType::CpuUsage,
                MetricType::MemoryUsage,
                MetricType::NetworkTraffic,
                MetricType::DiskIo,
                MetricType::ActiveConnections,
                MetricType::QueueDepth,
            ],
            resource_monitoring: ResourceMonitoringConfig::development(),
        }
    }

    /// Minimal configuration for testing
    pub fn minimal() -> Self {
        Self {
            enabled: true,
            collection_interval: Duration::from_secs(60),
            retention_period: Duration::from_secs(300), // 5 minutes
            max_memory_metrics: 100,
            collected_metrics: vec![MetricType::RequestLatency],
            resource_monitoring: ResourceMonitoringConfig::minimal(),
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.collection_interval.is_zero() {
            return Err(LoxoneError::invalid_input(
                "Collection interval cannot be zero",
            ));
        }

        if self.retention_period.is_zero() {
            return Err(LoxoneError::invalid_input(
                "Retention period cannot be zero",
            ));
        }

        if self.max_memory_metrics == 0 {
            return Err(LoxoneError::invalid_input(
                "Max memory metrics cannot be zero",
            ));
        }

        self.resource_monitoring.validate()?;
        Ok(())
    }
}

/// Resource monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMonitoringConfig {
    /// Enable CPU monitoring
    pub monitor_cpu: bool,
    /// Enable memory monitoring
    pub monitor_memory: bool,
    /// Enable network monitoring
    pub monitor_network: bool,
    /// Enable disk I/O monitoring
    pub monitor_disk: bool,
    /// Process-specific monitoring
    pub monitor_process: bool,
}

impl Default for ResourceMonitoringConfig {
    fn default() -> Self {
        Self {
            monitor_cpu: true,
            monitor_memory: true,
            monitor_network: false,
            monitor_disk: false,
            monitor_process: true,
        }
    }
}

impl ResourceMonitoringConfig {
    /// Production configuration
    pub fn production() -> Self {
        Self {
            monitor_cpu: true,
            monitor_memory: true,
            monitor_network: false,
            monitor_disk: false,
            monitor_process: true,
        }
    }

    /// Development configuration
    pub fn development() -> Self {
        Self {
            monitor_cpu: true,
            monitor_memory: true,
            monitor_network: true,
            monitor_disk: true,
            monitor_process: true,
        }
    }

    /// Minimal configuration
    pub fn minimal() -> Self {
        Self {
            monitor_cpu: false,
            monitor_memory: true,
            monitor_network: false,
            monitor_disk: false,
            monitor_process: false,
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // At least one monitoring type should be enabled
        if !self.monitor_cpu
            && !self.monitor_memory
            && !self.monitor_network
            && !self.monitor_disk
            && !self.monitor_process
        {
            return Err(LoxoneError::invalid_input(
                "At least one monitoring type must be enabled",
            ));
        }
        Ok(())
    }
}

/// Metric type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MetricType {
    /// Request latency in milliseconds
    RequestLatency,
    /// Throughput in requests per second
    ThroughputRps,
    /// Error rate percentage
    ErrorRate,
    /// CPU usage percentage
    CpuUsage,
    /// Memory usage in bytes
    MemoryUsage,
    /// Network traffic in bytes
    NetworkTraffic,
    /// Disk I/O in bytes
    DiskIo,
    /// Active connections count
    ActiveConnections,
    /// Queue depth
    QueueDepth,
    /// Custom metric
    Custom(String),
}

/// Metric data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDataPoint {
    /// Metric type
    pub metric_type: MetricType,
    /// Metric value
    pub value: f64,
    /// Timestamp
    pub timestamp: u64,
    /// Tags for grouping/filtering
    pub tags: HashMap<String, String>,
}

impl MetricDataPoint {
    /// Create new metric data point
    pub fn new(metric_type: MetricType, value: f64) -> Self {
        Self {
            metric_type,
            value,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            tags: HashMap::new(),
        }
    }

    /// Create metric with tags
    pub fn with_tags(metric_type: MetricType, value: f64, tags: HashMap<String, String>) -> Self {
        Self {
            metric_type,
            value,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            tags,
        }
    }
}

/// Aggregated metric statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStats {
    /// Metric type
    pub metric_type: MetricType,
    /// Count of data points
    pub count: u64,
    /// Sum of values
    pub sum: f64,
    /// Average value
    pub avg: f64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// 50th percentile
    pub p50: f64,
    /// 95th percentile
    pub p95: f64,
    /// 99th percentile
    pub p99: f64,
    /// Standard deviation
    pub std_dev: f64,
    /// Time range
    pub time_range: Duration,
}

/// Metrics collector service
pub struct MetricsCollector {
    config: MetricsConfig,
    metrics_storage: Arc<RwLock<MetricsStorage>>,
    start_time: Instant,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new(config: MetricsConfig) -> Result<Self> {
        config.validate()?;

        let storage = MetricsStorage::new(config.max_memory_metrics, config.retention_period);

        Ok(Self {
            config,
            metrics_storage: Arc::new(RwLock::new(storage)),
            start_time: Instant::now(),
        })
    }

    /// Collect resource usage metrics
    pub async fn collect_resource_usage(&self) -> Result<ResourceUsage> {
        if !self.config.enabled {
            return Ok(ResourceUsage::default());
        }

        let mut usage = ResourceUsage::default();

        // Collect CPU usage
        if self.config.resource_monitoring.monitor_cpu {
            usage.cpu_usage = self.get_cpu_usage().await.ok();
        }

        // Collect memory usage
        if self.config.resource_monitoring.monitor_memory {
            usage.memory_usage = self.get_memory_usage().await.ok();
        }

        // Collect network usage
        if self.config.resource_monitoring.monitor_network {
            let (tx, rx) = self.get_network_usage().await.unwrap_or((0, 0));
            usage.network_tx = Some(tx);
            usage.network_rx = Some(rx);
        }

        // Collect disk I/O
        if self.config.resource_monitoring.monitor_disk {
            let (read, write) = self.get_disk_usage().await.unwrap_or((0, 0));
            usage.disk_read = Some(read);
            usage.disk_write = Some(write);
        }

        Ok(usage)
    }

    /// Collect metrics for a specific context
    pub async fn collect_metrics(
        &self,
        context: &PerformanceContext,
    ) -> Result<HashMap<String, f64>> {
        if !self.config.enabled {
            return Ok(HashMap::new());
        }

        let mut metrics = HashMap::new();

        // Collect enabled metrics
        for metric_type in &self.config.collected_metrics {
            if let Ok(value) = self.get_metric_value(metric_type, context).await {
                let key = match metric_type {
                    MetricType::RequestLatency => "request_latency_ms",
                    MetricType::ThroughputRps => "throughput_rps",
                    MetricType::ErrorRate => "error_rate_percent",
                    MetricType::CpuUsage => "cpu_usage_percent",
                    MetricType::MemoryUsage => "memory_usage_bytes",
                    MetricType::NetworkTraffic => "network_traffic_bytes",
                    MetricType::DiskIo => "disk_io_bytes",
                    MetricType::ActiveConnections => "active_connections",
                    MetricType::QueueDepth => "queue_depth",
                    MetricType::Custom(name) => name,
                };
                metrics.insert(key.to_string(), value);
            }
        }

        Ok(metrics)
    }

    /// Record a custom metric
    pub async fn record_metric(
        &self,
        name: String,
        value: f64,
        tags: HashMap<String, String>,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let metric = MetricDataPoint::with_tags(MetricType::Custom(name), value, tags);

        let mut storage = self.metrics_storage.write().await;
        storage.add_metric(metric);

        Ok(())
    }

    /// Record a standard metric
    pub async fn record_standard_metric(
        &self,
        metric_type: MetricType,
        value: f64,
        tags: HashMap<String, String>,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let metric = MetricDataPoint::with_tags(metric_type, value, tags);

        let mut storage = self.metrics_storage.write().await;
        storage.add_metric(metric);

        Ok(())
    }

    /// Get metric statistics
    pub async fn get_metric_stats(
        &self,
        metric_type: &MetricType,
        time_range: Duration,
    ) -> Result<Option<MetricStats>> {
        let storage = self.metrics_storage.read().await;
        Ok(storage.get_stats(metric_type, time_range))
    }

    /// Get all metrics within a time range
    pub async fn get_metrics(&self, time_range: Duration) -> Result<Vec<MetricDataPoint>> {
        let storage = self.metrics_storage.read().await;
        Ok(storage.get_metrics_in_range(time_range))
    }

    /// Clean up old metrics
    pub async fn cleanup_metrics(&self) -> Result<()> {
        let mut storage = self.metrics_storage.write().await;
        storage.cleanup(self.config.retention_period);
        Ok(())
    }

    // Private helper methods for collecting system metrics

    async fn get_cpu_usage(&self) -> Result<f64> {
        // Simplified CPU usage calculation
        // In production, this would use proper system metrics
        use std::fs;

        if let Ok(loadavg) = fs::read_to_string("/proc/loadavg") {
            if let Some(load) = loadavg.split_whitespace().next() {
                if let Ok(load_value) = load.parse::<f64>() {
                    return Ok((load_value * 100.0).min(100.0)); // Convert to percentage
                }
            }
        }

        // Fallback: simulate some CPU usage
        Ok(0.0)
    }

    async fn get_memory_usage(&self) -> Result<u64> {
        // Simplified memory usage calculation
        // In production, this would use proper system metrics
        use std::fs;

        if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
            let mut total_kb = 0u64;
            let mut available_kb = 0u64;

            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        total_kb = value.parse().unwrap_or(0);
                    }
                } else if line.starts_with("MemAvailable:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        available_kb = value.parse().unwrap_or(0);
                    }
                }
            }

            if total_kb > 0 {
                let used_kb = total_kb.saturating_sub(available_kb);
                return Ok(used_kb * 1024); // Convert to bytes
            }
        }

        // Fallback: return process memory usage estimate
        Ok(1024 * 1024 * 10) // 10MB
    }

    async fn get_network_usage(&self) -> Result<(u64, u64)> {
        // Simplified network usage - would need proper system integration
        Ok((0, 0))
    }

    async fn get_disk_usage(&self) -> Result<(u64, u64)> {
        // Simplified disk usage - would need proper system integration
        Ok((0, 0))
    }

    async fn get_metric_value(
        &self,
        metric_type: &MetricType,
        _context: &PerformanceContext,
    ) -> Result<f64> {
        match metric_type {
            MetricType::RequestLatency => {
                // Would calculate based on recent request timings
                Ok(50.0) // 50ms average
            }
            MetricType::ThroughputRps => {
                // Would calculate based on recent request rate
                Ok(10.0) // 10 requests per second
            }
            MetricType::ErrorRate => {
                // Would calculate based on recent error ratio
                Ok(0.5) // 0.5% error rate
            }
            MetricType::CpuUsage => self.get_cpu_usage().await,
            MetricType::MemoryUsage => Ok(self.get_memory_usage().await? as f64),
            MetricType::NetworkTraffic => {
                let (tx, rx) = self.get_network_usage().await?;
                Ok((tx + rx) as f64)
            }
            MetricType::DiskIo => {
                let (read, write) = self.get_disk_usage().await?;
                Ok((read + write) as f64)
            }
            MetricType::ActiveConnections => Ok(5.0), // Would track actual connections
            MetricType::QueueDepth => Ok(0.0),        // Would track actual queue depth
            MetricType::Custom(_) => Ok(0.0),         // Custom metrics would be tracked separately
        }
    }
}

/// In-memory metrics storage
struct MetricsStorage {
    metrics: Vec<MetricDataPoint>,
    max_metrics: usize,
    retention_period: Duration,
}

impl MetricsStorage {
    fn new(max_metrics: usize, retention_period: Duration) -> Self {
        Self {
            metrics: Vec::with_capacity(max_metrics),
            max_metrics,
            retention_period,
        }
    }

    fn add_metric(&mut self, metric: MetricDataPoint) {
        self.metrics.push(metric);

        // Cleanup if we exceed max metrics
        if self.metrics.len() > self.max_metrics {
            self.cleanup(self.retention_period);
        }
    }

    fn cleanup(&mut self, retention_period: Duration) {
        let cutoff_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .saturating_sub(retention_period.as_secs());

        self.metrics
            .retain(|metric| metric.timestamp >= cutoff_time);

        // If still too many metrics, remove oldest
        if self.metrics.len() > self.max_metrics {
            let excess = self.metrics.len() - self.max_metrics;
            self.metrics.drain(0..excess);
        }
    }

    fn get_metrics_in_range(&self, time_range: Duration) -> Vec<MetricDataPoint> {
        let cutoff_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .saturating_sub(time_range.as_secs());

        self.metrics
            .iter()
            .filter(|metric| metric.timestamp >= cutoff_time)
            .cloned()
            .collect()
    }

    fn get_stats(&self, metric_type: &MetricType, time_range: Duration) -> Option<MetricStats> {
        let metrics = self.get_metrics_in_range(time_range);
        let filtered: Vec<_> = metrics
            .iter()
            .filter(|m| &m.metric_type == metric_type)
            .collect();

        if filtered.is_empty() {
            return None;
        }

        let values: Vec<f64> = filtered.iter().map(|m| m.value).collect();
        let count = values.len() as u64;
        let sum: f64 = values.iter().sum();
        let avg = sum / count as f64;

        let mut sorted_values = values.clone();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let min = sorted_values[0];
        let max = sorted_values[sorted_values.len() - 1];
        let p50 = percentile(&sorted_values, 50.0);
        let p95 = percentile(&sorted_values, 95.0);
        let p99 = percentile(&sorted_values, 99.0);

        // Calculate standard deviation
        let variance: f64 = values.iter().map(|x| (x - avg).powi(2)).sum::<f64>() / count as f64;
        let std_dev = variance.sqrt();

        Some(MetricStats {
            metric_type: metric_type.clone(),
            count,
            sum,
            avg,
            min,
            max,
            p50,
            p95,
            p99,
            std_dev,
            time_range,
        })
    }
}

/// Calculate percentile from sorted values
fn percentile(sorted_values: &[f64], percentile: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }

    let index = (percentile / 100.0) * (sorted_values.len() - 1) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;

    if lower == upper {
        sorted_values[lower]
    } else {
        let weight = index - lower as f64;
        sorted_values[lower] * (1.0 - weight) + sorted_values[upper] * weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_config_validation() {
        let config = MetricsConfig::production();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_metric_data_point() {
        let metric = MetricDataPoint::new(MetricType::RequestLatency, 100.0);
        assert_eq!(metric.value, 100.0);
        assert!(metric.timestamp > 0);
    }

    #[tokio::test]
    async fn test_metrics_collector() {
        let config = MetricsConfig::minimal();
        let collector = MetricsCollector::new(config).unwrap();

        let mut tags = HashMap::new();
        tags.insert("endpoint".to_string(), "/api/test".to_string());

        collector
            .record_metric("test_metric".to_string(), 42.0, tags)
            .await
            .unwrap();
    }

    #[test]
    fn test_metrics_storage() {
        let mut storage = MetricsStorage::new(100, Duration::from_secs(60));

        let metric = MetricDataPoint::new(MetricType::RequestLatency, 50.0);
        storage.add_metric(metric);

        let metrics = storage.get_metrics_in_range(Duration::from_secs(60));
        assert_eq!(metrics.len(), 1);
    }

    #[test]
    fn test_percentile_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(&values, 50.0), 3.0);
        assert_eq!(percentile(&values, 95.0), 4.8);
    }
}
