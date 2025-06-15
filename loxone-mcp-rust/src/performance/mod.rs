//! Performance monitoring and metrics collection system
//!
//! This module provides comprehensive performance monitoring capabilities including:
//! - Request latency tracking
//! - Bottleneck identification  
//! - Resource utilization monitoring
//! - Performance analytics and reporting

pub mod analyzer;
pub mod metrics;
pub mod middleware;
pub mod profiler;
pub mod reporter;

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Performance monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable performance monitoring
    pub enabled: bool,
    /// Metrics collection configuration
    pub metrics: metrics::MetricsConfig,
    /// Performance profiling configuration
    pub profiler: profiler::ProfilerConfig,
    /// Analysis configuration
    pub analyzer: analyzer::AnalyzerConfig,
    /// Reporting configuration
    pub reporter: reporter::ReporterConfig,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            metrics: metrics::MetricsConfig::default(),
            profiler: profiler::ProfilerConfig::default(),
            analyzer: analyzer::AnalyzerConfig::default(),
            reporter: reporter::ReporterConfig::default(),
        }
    }
}

impl PerformanceConfig {
    /// Create a production configuration with balanced monitoring
    pub fn production() -> Self {
        Self {
            enabled: true,
            metrics: metrics::MetricsConfig::production(),
            profiler: profiler::ProfilerConfig::production(),
            analyzer: analyzer::AnalyzerConfig::production(),
            reporter: reporter::ReporterConfig::production(),
        }
    }

    /// Create a development configuration with detailed monitoring
    pub fn development() -> Self {
        Self {
            enabled: true,
            metrics: metrics::MetricsConfig::development(),
            profiler: profiler::ProfilerConfig::development(),
            analyzer: analyzer::AnalyzerConfig::development(),
            reporter: reporter::ReporterConfig::development(),
        }
    }

    /// Create a minimal configuration for testing
    pub fn testing() -> Self {
        Self {
            enabled: false,
            metrics: metrics::MetricsConfig::minimal(),
            profiler: profiler::ProfilerConfig::disabled(),
            analyzer: analyzer::AnalyzerConfig::minimal(),
            reporter: reporter::ReporterConfig::disabled(),
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            self.metrics.validate()?;
            self.profiler.validate()?;
            self.analyzer.validate()?;
            self.reporter.validate()?;
        }
        Ok(())
    }
}

/// Performance timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTiming {
    /// Start timestamp (nanoseconds since epoch)
    #[serde(with = "instant_serde")]
    pub start_time: Instant,
    /// End timestamp (nanoseconds since epoch)
    #[serde(with = "option_instant_serde")]
    pub end_time: Option<Instant>,
    /// Total duration
    pub duration: Option<Duration>,
    /// Sub-timings for different phases
    pub phases: HashMap<String, Duration>,
    /// Performance tags
    pub tags: HashMap<String, String>,
}

mod instant_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(_instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert to duration since epoch for serialization
        let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        serializer.serialize_u64(duration.as_nanos() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
    where
        D: Deserializer<'de>,
    {
        let _nanos = u64::deserialize(deserializer)?;
        // This is approximate since we can't perfectly reconstruct Instant
        Ok(Instant::now())
    }
}

mod option_instant_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Instant;

    pub fn serialize<S>(opt_instant: &Option<Instant>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match opt_instant {
            Some(instant) => super::instant_serde::serialize(instant, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Instant>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<u64>::deserialize(deserializer).map(|opt| {
            opt.map(|_| Instant::now()) // Approximate reconstruction
        })
    }
}

impl PerformanceTiming {
    /// Create new performance timing
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            end_time: None,
            duration: None,
            phases: HashMap::new(),
            tags: HashMap::new(),
        }
    }

    /// Start timing a phase
    pub fn start_phase(&mut self, phase: &str) -> PhaseTimer {
        PhaseTimer::new(phase.to_string())
    }

    /// Record a phase duration
    pub fn record_phase(&mut self, phase: String, duration: Duration) {
        self.phases.insert(phase, duration);
    }

    /// Add a tag
    pub fn tag(&mut self, key: String, value: String) {
        self.tags.insert(key, value);
    }

    /// Finish timing
    pub fn finish(&mut self) {
        let now = Instant::now();
        self.end_time = Some(now);
        self.duration = Some(now.duration_since(self.start_time));
    }

    /// Get duration
    pub fn get_duration(&self) -> Option<Duration> {
        self.duration
            .or_else(|| self.end_time.map(|end| end.duration_since(self.start_time)))
    }

    /// Check if timing is complete
    pub fn is_complete(&self) -> bool {
        self.end_time.is_some()
    }
}

impl Default for PerformanceTiming {
    fn default() -> Self {
        Self::new()
    }
}

/// Phase timer for measuring sub-operations
pub struct PhaseTimer {
    phase_name: String,
    start_time: Instant,
}

impl PhaseTimer {
    /// Create new phase timer
    pub fn new(phase_name: String) -> Self {
        Self {
            phase_name,
            start_time: Instant::now(),
        }
    }

    /// Finish phase and return duration
    pub fn finish(self) -> (String, Duration) {
        (self.phase_name, self.start_time.elapsed())
    }
}

/// Performance measurement context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceContext {
    /// Request/operation identifier
    pub operation_id: String,
    /// Operation type
    pub operation_type: String,
    /// Client identifier
    pub client_id: Option<String>,
    /// Additional context data
    pub context_data: HashMap<String, String>,
}

impl PerformanceContext {
    /// Create new performance context
    pub fn new(operation_id: String, operation_type: String) -> Self {
        Self {
            operation_id,
            operation_type,
            client_id: None,
            context_data: HashMap::new(),
        }
    }

    /// Set client ID
    pub fn with_client_id(mut self, client_id: String) -> Self {
        self.client_id = Some(client_id);
        self
    }

    /// Add context data
    pub fn with_context(mut self, key: String, value: String) -> Self {
        self.context_data.insert(key, value);
        self
    }
}

/// Performance measurement result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMeasurement {
    /// Performance context
    pub context: PerformanceContext,
    /// Timing information
    pub timing: PerformanceTiming,
    /// Resource usage
    pub resource_usage: ResourceUsage,
    /// Performance metrics
    pub metrics: HashMap<String, f64>,
    /// Errors or warnings
    pub issues: Vec<PerformanceIssue>,
}

/// Resource usage information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceUsage {
    /// CPU usage percentage
    pub cpu_usage: Option<f64>,
    /// Memory usage in bytes
    pub memory_usage: Option<u64>,
    /// Network bytes sent
    pub network_tx: Option<u64>,
    /// Network bytes received
    pub network_rx: Option<u64>,
    /// Disk read bytes
    pub disk_read: Option<u64>,
    /// Disk write bytes
    pub disk_write: Option<u64>,
}

/// Performance issue or warning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceIssue {
    /// Issue severity
    pub severity: PerformanceIssueSeverity,
    /// Issue type
    pub issue_type: PerformanceIssueType,
    /// Issue description
    pub description: String,
    /// Recommended action
    pub recommendation: Option<String>,
    /// Metric value that triggered the issue
    pub metric_value: Option<f64>,
    /// Threshold that was exceeded
    pub threshold: Option<f64>,
}

/// Performance issue severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceIssueSeverity {
    /// Informational
    Info,
    /// Warning - performance degradation
    Warning,
    /// Critical - significant performance impact
    Critical,
}

/// Performance issue type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceIssueType {
    /// High latency
    HighLatency,
    /// High CPU usage
    HighCpuUsage,
    /// High memory usage
    HighMemoryUsage,
    /// Slow database query
    SlowQuery,
    /// Network timeout
    NetworkTimeout,
    /// Resource contention
    ResourceContention,
    /// Memory leak
    MemoryLeak,
    /// Inefficient algorithm
    InefficientAlgorithm,
}

/// Performance monitoring service
pub struct PerformanceMonitor {
    config: PerformanceConfig,
    metrics_collector: metrics::MetricsCollector,
    profiler: profiler::PerformanceProfiler,
    analyzer: analyzer::PerformanceAnalyzer,
    reporter: reporter::PerformanceReporter,
}

impl PerformanceMonitor {
    /// Create new performance monitor
    pub fn new(config: PerformanceConfig) -> Result<Self> {
        config.validate()?;

        let metrics_collector = metrics::MetricsCollector::new(config.metrics.clone())?;
        let profiler = profiler::PerformanceProfiler::new(config.profiler.clone())?;
        let analyzer = analyzer::PerformanceAnalyzer::new(config.analyzer.clone())?;
        let reporter = reporter::PerformanceReporter::new(config.reporter.clone())?;

        Ok(Self {
            config,
            metrics_collector,
            profiler,
            analyzer,
            reporter,
        })
    }

    /// Start measuring performance for an operation
    pub async fn start_measurement(
        &self,
        context: PerformanceContext,
    ) -> Result<PerformanceMeasurement> {
        if !self.config.enabled {
            return Ok(PerformanceMeasurement {
                context,
                timing: PerformanceTiming::new(),
                resource_usage: ResourceUsage::default(),
                metrics: HashMap::new(),
                issues: Vec::new(),
            });
        }

        let timing = PerformanceTiming::new();
        let resource_usage = self.metrics_collector.collect_resource_usage().await?;

        Ok(PerformanceMeasurement {
            context,
            timing,
            resource_usage,
            metrics: HashMap::new(),
            issues: Vec::new(),
        })
    }

    /// Finish measuring performance for an operation
    pub async fn finish_measurement(
        &self,
        mut measurement: PerformanceMeasurement,
    ) -> Result<PerformanceMeasurement> {
        if !self.config.enabled {
            return Ok(measurement);
        }

        // Finish timing
        measurement.timing.finish();

        // Collect final resource usage
        measurement.resource_usage = self.metrics_collector.collect_resource_usage().await?;

        // Collect custom metrics
        measurement.metrics = self
            .metrics_collector
            .collect_metrics(&measurement.context)
            .await?;

        // Analyze performance and identify issues
        measurement.issues = self.analyzer.analyze_performance(&measurement).await?;

        // Report measurement
        self.reporter.report_measurement(&measurement).await?;

        Ok(measurement)
    }

    /// Record a custom metric
    pub async fn record_metric(
        &self,
        name: String,
        value: f64,
        tags: HashMap<String, String>,
    ) -> Result<()> {
        if self.config.enabled {
            self.metrics_collector
                .record_metric(name, value, tags)
                .await?;
        }
        Ok(())
    }

    /// Get performance statistics
    pub async fn get_statistics(&self) -> Result<PerformanceStatistics> {
        if !self.config.enabled {
            return Ok(PerformanceStatistics::default());
        }

        self.analyzer.get_statistics().await
    }

    /// Get performance configuration
    pub fn get_config(&self) -> &PerformanceConfig {
        &self.config
    }
}

/// Performance statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceStatistics {
    /// Request statistics
    pub request_stats: RequestStatistics,
    /// Resource usage statistics
    pub resource_stats: ResourceStatistics,
    /// Error statistics
    pub error_stats: ErrorStatistics,
    /// Performance trends
    pub trends: PerformanceTrends,
}

/// Request performance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestStatistics {
    /// Total requests processed
    pub total_requests: u64,
    /// Average response time
    pub avg_response_time: Duration,
    /// 50th percentile response time
    pub p50_response_time: Duration,
    /// 95th percentile response time
    pub p95_response_time: Duration,
    /// 99th percentile response time
    pub p99_response_time: Duration,
    /// Requests per second
    pub requests_per_second: f64,
    /// Success rate percentage
    pub success_rate: f64,
}

impl Default for RequestStatistics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            avg_response_time: Duration::from_millis(0),
            p50_response_time: Duration::from_millis(0),
            p95_response_time: Duration::from_millis(0),
            p99_response_time: Duration::from_millis(0),
            requests_per_second: 0.0,
            success_rate: 100.0,
        }
    }
}

/// Resource usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStatistics {
    /// Average CPU usage
    pub avg_cpu_usage: f64,
    /// Peak CPU usage
    pub peak_cpu_usage: f64,
    /// Average memory usage
    pub avg_memory_usage: u64,
    /// Peak memory usage
    pub peak_memory_usage: u64,
    /// Total network traffic
    pub total_network_traffic: u64,
    /// Total disk I/O
    pub total_disk_io: u64,
}

impl Default for ResourceStatistics {
    fn default() -> Self {
        Self {
            avg_cpu_usage: 0.0,
            peak_cpu_usage: 0.0,
            avg_memory_usage: 0,
            peak_memory_usage: 0,
            total_network_traffic: 0,
            total_disk_io: 0,
        }
    }
}

/// Error statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStatistics {
    /// Total errors
    pub total_errors: u64,
    /// Error rate percentage
    pub error_rate: f64,
    /// Errors by type
    pub errors_by_type: HashMap<String, u64>,
    /// Most common error
    pub most_common_error: Option<String>,
}

impl Default for ErrorStatistics {
    fn default() -> Self {
        Self {
            total_errors: 0,
            error_rate: 0.0,
            errors_by_type: HashMap::new(),
            most_common_error: None,
        }
    }
}

/// Performance trends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTrends {
    /// Response time trend (positive = improving, negative = degrading)
    pub response_time_trend: f64,
    /// Throughput trend
    pub throughput_trend: f64,
    /// Error rate trend
    pub error_rate_trend: f64,
    /// Resource usage trend
    pub resource_usage_trend: f64,
}

impl Default for PerformanceTrends {
    fn default() -> Self {
        Self {
            response_time_trend: 0.0,
            throughput_trend: 0.0,
            error_rate_trend: 0.0,
            resource_usage_trend: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_config_validation() {
        let config = PerformanceConfig::production();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_performance_timing() {
        let mut timing = PerformanceTiming::new();

        // Simulate some work
        std::thread::sleep(Duration::from_millis(10));

        timing.finish();
        assert!(timing.is_complete());
        assert!(timing.get_duration().unwrap().as_millis() >= 10);
    }

    #[test]
    fn test_phase_timer() {
        let timer = PhaseTimer::new("test_phase".to_string());
        std::thread::sleep(Duration::from_millis(5));
        let (phase, duration) = timer.finish();

        assert_eq!(phase, "test_phase");
        assert!(duration.as_millis() >= 5);
    }

    #[test]
    fn test_performance_context() {
        let context = PerformanceContext::new("test_op".to_string(), "test_type".to_string())
            .with_client_id("test_client".to_string())
            .with_context("key".to_string(), "value".to_string());

        assert_eq!(context.operation_id, "test_op");
        assert_eq!(context.operation_type, "test_type");
        assert_eq!(context.client_id, Some("test_client".to_string()));
        assert_eq!(context.context_data.get("key"), Some(&"value".to_string()));
    }

    #[tokio::test]
    async fn test_performance_monitor_creation() {
        let config = PerformanceConfig::testing();
        let monitor = PerformanceMonitor::new(config);
        assert!(monitor.is_ok());
    }
}
