//! Metrics collection and Prometheus export
//!
//! This module provides metrics collection with Prometheus-compatible exports
//! and integration with InfluxDB for historical storage.

use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};
use tokio::sync::RwLock;
use tracing::{debug, info};

#[cfg(feature = "influxdb")]
use super::influxdb::{InfluxManager, McpMetrics};

/// Metric type for Prometheus export
#[derive(Debug, Clone, PartialEq)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

/// Metric value
#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
    Summary { sum: f64, count: u64 },
}

/// Metric metadata
#[derive(Debug, Clone)]
pub struct MetricMetadata {
    pub name: String,
    pub help: String,
    pub metric_type: MetricType,
    pub labels: HashMap<String, String>,
}

/// Single metric with value and metadata
#[derive(Debug, Clone)]
pub struct Metric {
    pub metadata: MetricMetadata,
    pub value: MetricValue,
    pub timestamp: Instant,
}

/// Request timing information
#[derive(Debug, Clone)]
pub struct RequestTiming {
    pub endpoint: String,
    pub method: String,
    pub duration_ms: f64,
    pub status_code: u16,
}

/// System resource metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub memory_total_mb: f64,
    pub disk_usage_percent: f64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
}

/// Metrics collector
pub struct MetricsCollector {
    /// All metrics storage
    metrics: Arc<RwLock<HashMap<String, Metric>>>,
    /// Request timings for percentile calculation
    request_timings: Arc<RwLock<Vec<RequestTiming>>>,
    /// InfluxDB manager for historical storage
    #[cfg(feature = "influxdb")]
    influx_manager: Option<Arc<InfluxManager>>,
    /// Collection start time
    start_time: Instant,
    /// System info collector
    system: Arc<RwLock<System>>,
    /// Last system update time
    last_system_update: Arc<RwLock<Instant>>,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            request_timings: Arc::new(RwLock::new(Vec::new())),
            #[cfg(feature = "influxdb")]
            influx_manager: None,
            start_time: Instant::now(),
            system: Arc::new(RwLock::new(system)),
            last_system_update: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Create with InfluxDB integration
    #[cfg(feature = "influxdb")]
    pub fn with_influx(influx_manager: Arc<InfluxManager>) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            request_timings: Arc::new(RwLock::new(Vec::new())),
            influx_manager: Some(influx_manager),
            start_time: Instant::now(),
            system: Arc::new(RwLock::new(system)),
            last_system_update: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Register a counter metric
    pub async fn register_counter(&self, name: &str, help: &str, labels: HashMap<String, String>) {
        let metadata = MetricMetadata {
            name: name.to_string(),
            help: help.to_string(),
            metric_type: MetricType::Counter,
            labels,
        };

        let metric = Metric {
            metadata,
            value: MetricValue::Counter(0),
            timestamp: Instant::now(),
        };

        self.metrics.write().await.insert(name.to_string(), metric);
    }

    /// Register a gauge metric
    pub async fn register_gauge(&self, name: &str, help: &str, labels: HashMap<String, String>) {
        let metadata = MetricMetadata {
            name: name.to_string(),
            help: help.to_string(),
            metric_type: MetricType::Gauge,
            labels,
        };

        let metric = Metric {
            metadata,
            value: MetricValue::Gauge(0.0),
            timestamp: Instant::now(),
        };

        self.metrics.write().await.insert(name.to_string(), metric);
    }

    /// Increment counter
    pub async fn increment_counter(&self, name: &str, value: u64) {
        let mut metrics = self.metrics.write().await;
        if let Some(metric) = metrics.get_mut(name) {
            if let MetricValue::Counter(ref mut count) = metric.value {
                *count += value;
                metric.timestamp = Instant::now();
            }
        }
    }

    /// Set gauge value
    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write().await;
        if let Some(metric) = metrics.get_mut(name) {
            if let MetricValue::Gauge(ref mut gauge_value) = metric.value {
                *gauge_value = value;
                metric.timestamp = Instant::now();
            }
        }
    }

    /// Record request timing
    pub async fn record_request_timing(&self, timing: RequestTiming) {
        self.request_timings.write().await.push(timing.clone());

        // Update request counter
        self.increment_counter("mcp_requests_total", 1).await;

        // Update status counter
        let status_code_family = timing.status_code / 100;
        let status_family = format!("{status_code_family}xx");
        self.increment_counter(&format!("mcp_requests_by_status_{status_family}"), 1)
            .await;

        // Update response time histogram
        self.record_histogram("mcp_request_duration_ms", timing.duration_ms)
            .await;

        // Update endpoint-specific metrics
        self.increment_counter(
            &format!(
                "mcp_requests_by_endpoint_{}",
                timing.endpoint.replace('/', "_")
            ),
            1,
        )
        .await;
    }

    /// Record rate limit event
    pub async fn record_rate_limit_event(&self, limited: bool) {
        if limited {
            self.increment_counter("rate_limit_rejections_total", 1)
                .await;
        } else {
            self.increment_counter("rate_limit_allowed_total", 1).await;
        }
    }

    /// Record histogram value
    async fn record_histogram(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write().await;

        // Create histogram metric if it doesn't exist
        if !metrics.contains_key(name) {
            let metadata = MetricMetadata {
                name: name.to_string(),
                help: format!("Histogram for {name}"),
                metric_type: MetricType::Histogram,
                labels: HashMap::new(),
            };

            let metric = Metric {
                metadata,
                value: MetricValue::Histogram(vec![]),
                timestamp: Instant::now(),
            };

            metrics.insert(name.to_string(), metric);
        }

        if let Some(metric) = metrics.get_mut(name) {
            if let MetricValue::Histogram(ref mut values) = metric.value {
                values.push(value);
                metric.timestamp = Instant::now();

                // Keep only last 1000 values to prevent memory growth
                if values.len() > 1000 {
                    values.drain(0..values.len() - 1000);
                }
            }
        }
    }

    /// Collect and update system metrics
    pub async fn collect_system_metrics(&self) {
        let mut last_update = self.last_system_update.write().await;

        // Only update every 5 seconds to avoid excessive CPU usage
        if last_update.elapsed() < Duration::from_secs(5) {
            return;
        }

        let mut system = self.system.write().await;
        system.refresh_cpu_usage();
        system.refresh_memory();
        system.refresh_processes();

        // Calculate CPU usage
        let cpu_usage = system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>()
            / system.cpus().len() as f32;

        // Get memory info
        let used_memory = system.used_memory() as f64 / 1024.0 / 1024.0; // Convert to MB
        let total_memory = system.total_memory() as f64 / 1024.0 / 1024.0;

        // Get current process info
        let current_pid = std::process::id();
        let process_memory = if let Some(process) = system.process(Pid::from_u32(current_pid)) {
            process.memory() as f64 / 1024.0 / 1024.0
        } else {
            0.0
        };

        drop(system); // Release lock before updating metrics

        // Update metrics
        self.set_gauge("system_cpu_usage_percent", cpu_usage as f64)
            .await;
        self.set_gauge("system_memory_usage_mb", used_memory).await;
        self.set_gauge("system_memory_total_mb", total_memory).await;
        self.set_gauge("process_memory_usage_mb", process_memory)
            .await;

        *last_update = Instant::now();

        debug!(
            "System metrics updated - CPU: {:.1}%, Memory: {:.1}/{:.1} MB, Process: {:.1} MB",
            cpu_usage, used_memory, total_memory, process_memory
        );
    }

    /// Update system metrics (deprecated - use collect_system_metrics)
    pub async fn update_system_metrics(&self, system_metrics: SystemMetrics) {
        self.set_gauge("system_cpu_usage_percent", system_metrics.cpu_usage_percent)
            .await;
        self.set_gauge("system_memory_usage_mb", system_metrics.memory_usage_mb)
            .await;
        self.set_gauge("system_memory_total_mb", system_metrics.memory_total_mb)
            .await;
        self.set_gauge(
            "system_disk_usage_percent",
            system_metrics.disk_usage_percent,
        )
        .await;
        self.set_gauge(
            "system_network_rx_bytes",
            system_metrics.network_rx_bytes as f64,
        )
        .await;
        self.set_gauge(
            "system_network_tx_bytes",
            system_metrics.network_tx_bytes as f64,
        )
        .await;
    }

    /// Export metrics in Prometheus format
    pub async fn export_prometheus(&self) -> String {
        let metrics = self.metrics.read().await;
        let mut output = String::new();

        // Add process info
        output.push_str(&format!(
            "# HELP process_uptime_seconds Time since process start\n\
             # TYPE process_uptime_seconds gauge\n\
             process_uptime_seconds {}\n\n",
            self.start_time.elapsed().as_secs_f64()
        ));

        // Export all metrics
        for metric in metrics.values() {
            let metadata = &metric.metadata;

            // Write help and type
            output.push_str(&format!("# HELP {} {}\n", metadata.name, metadata.help));
            output.push_str(&format!(
                "# TYPE {} {}\n",
                metadata.name,
                match metadata.metric_type {
                    MetricType::Counter => "counter",
                    MetricType::Gauge => "gauge",
                    MetricType::Histogram => "histogram",
                    MetricType::Summary => "summary",
                }
            ));

            // Write metric value
            let labels_str = if metadata.labels.is_empty() {
                String::new()
            } else {
                let labels: Vec<String> = metadata
                    .labels
                    .iter()
                    .map(|(k, v)| format!("{k}=\"{v}\""))
                    .collect();
                let label_string = labels.join(",");
                format!("{{{label_string}}}")
            };

            match &metric.value {
                MetricValue::Counter(value) => {
                    output.push_str(&format!("{}{} {}\n", metadata.name, labels_str, value));
                }
                MetricValue::Gauge(value) => {
                    output.push_str(&format!("{}{} {}\n", metadata.name, labels_str, value));
                }
                MetricValue::Histogram(values) => {
                    if !values.is_empty() {
                        // Calculate percentiles
                        let mut sorted = values.clone();
                        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

                        let p50 = sorted[sorted.len() / 2];
                        let p90 = sorted[sorted.len() * 90 / 100];
                        let p99 = sorted[sorted.len() * 99 / 100];
                        let sum: f64 = sorted.iter().sum();

                        output.push_str(&format!(
                            "{}_bucket{{le=\"50\",{}}} {}\n",
                            metadata.name, labels_str, p50
                        ));
                        output.push_str(&format!(
                            "{}_bucket{{le=\"90\",{}}} {}\n",
                            metadata.name, labels_str, p90
                        ));
                        output.push_str(&format!(
                            "{}_bucket{{le=\"99\",{}}} {}\n",
                            metadata.name, labels_str, p99
                        ));
                        output.push_str(&format!("{}_sum{} {}\n", metadata.name, labels_str, sum));
                        output.push_str(&format!(
                            "{}_count{} {}\n",
                            metadata.name,
                            labels_str,
                            values.len()
                        ));
                    }
                }
                MetricValue::Summary { sum, count } => {
                    output.push_str(&format!("{}_sum{} {}\n", metadata.name, labels_str, sum));
                    output.push_str(&format!(
                        "{}_count{} {}\n",
                        metadata.name, labels_str, count
                    ));
                }
            }

            output.push('\n');
        }

        output
    }

    /// Push metrics to InfluxDB
    #[cfg(feature = "influxdb")]
    pub async fn push_to_influx(&self) -> Result<()> {
        if let Some(influx) = &self.influx_manager {
            let metrics = self.metrics.read().await;

            // Collect MCP metrics
            let total_requests = metrics
                .get("mcp_requests_total")
                .and_then(|m| match &m.value {
                    MetricValue::Counter(v) => Some(*v),
                    _ => None,
                })
                .unwrap_or(0);

            let failed_requests = metrics
                .get("mcp_requests_by_status_5xx")
                .and_then(|m| match &m.value {
                    MetricValue::Counter(v) => Some(*v),
                    _ => None,
                })
                .unwrap_or(0);

            let avg_response_time = self.calculate_avg_response_time().await;

            let cpu_usage = metrics
                .get("system_cpu_usage_percent")
                .and_then(|m| match &m.value {
                    MetricValue::Gauge(v) => Some(*v),
                    _ => None,
                })
                .unwrap_or(0.0);

            let memory_usage = metrics
                .get("system_memory_usage_mb")
                .and_then(|m| match &m.value {
                    MetricValue::Gauge(v) => Some(*v),
                    _ => None,
                })
                .unwrap_or(0.0);

            // Write MCP metrics
            influx
                .write_mcp_metrics(McpMetrics {
                    total_requests,
                    failed_requests,
                    active_connections: 0, // TODO: Track active connections
                    avg_response_time_ms: avg_response_time,
                    cpu_usage,
                    memory_usage_mb: memory_usage,
                    timestamp: Utc::now(),
                })
                .await?;

            info!("Pushed metrics to InfluxDB");
        }

        Ok(())
    }

    /// Calculate average response time (recent requests only)
    async fn calculate_avg_response_time(&self) -> f64 {
        let timings = self.request_timings.read().await;
        if timings.is_empty() {
            return 0.0;
        }

        // Only use last 10 requests for more recent average
        let recent_timings: Vec<&RequestTiming> = timings.iter().rev().take(10).collect();
        if recent_timings.is_empty() {
            return 0.0;
        }

        let sum: f64 = recent_timings.iter().map(|t| t.duration_ms).sum();
        sum / recent_timings.len() as f64
    }

    /// Clear old request timings to prevent memory growth
    pub async fn cleanup_old_timings(&self, _max_age: Duration) {
        let mut timings = self.request_timings.write().await;

        // Keep only recent timings
        // Note: RequestTiming doesn't have timestamp, so we just limit the size

        // Keep max 10000 entries
        if timings.len() > 10000 {
            let drain_count = timings.len() - 10000;
            timings.drain(0..drain_count);
        }
    }

    /// Initialize default metrics
    pub async fn init_default_metrics(&self) {
        // Request metrics
        self.register_counter(
            "mcp_requests_total",
            "Total number of MCP requests",
            HashMap::new(),
        )
        .await;

        // Status code metrics
        for status in ["2xx", "3xx", "4xx", "5xx"] {
            self.register_counter(
                &format!("mcp_requests_by_status_{status}"),
                &format!("Requests with {status} status"),
                HashMap::new(),
            )
            .await;
        }

        // Rate limiting metrics
        self.register_counter(
            "rate_limit_rejections_total",
            "Total number of rate limited requests",
            HashMap::new(),
        )
        .await;

        self.register_counter(
            "rate_limit_allowed_total",
            "Total number of requests allowed by rate limiter",
            HashMap::new(),
        )
        .await;

        // System metrics
        self.register_gauge(
            "system_cpu_usage_percent",
            "CPU usage percentage",
            HashMap::new(),
        )
        .await;

        self.register_gauge(
            "system_memory_usage_mb",
            "Memory usage in MB",
            HashMap::new(),
        )
        .await;

        self.register_gauge(
            "system_memory_total_mb",
            "Total memory in MB",
            HashMap::new(),
        )
        .await;

        self.register_gauge(
            "process_memory_usage_mb",
            "Process memory usage in MB",
            HashMap::new(),
        )
        .await;

        self.register_gauge(
            "system_disk_usage_percent",
            "Disk usage percentage",
            HashMap::new(),
        )
        .await;

        self.register_gauge(
            "system_network_rx_bytes",
            "Network received bytes",
            HashMap::new(),
        )
        .await;

        self.register_gauge(
            "system_network_tx_bytes",
            "Network transmitted bytes",
            HashMap::new(),
        )
        .await;

        debug!("Initialized default metrics");
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Global metrics instance (lazy initialized)
static METRICS: once_cell::sync::Lazy<Arc<MetricsCollector>> =
    once_cell::sync::Lazy::new(|| Arc::new(MetricsCollector::new()));

/// Get global metrics collector
pub fn get_metrics() -> Arc<MetricsCollector> {
    METRICS.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_counter_metric() {
        let collector = MetricsCollector::new();

        collector
            .register_counter("test_counter", "Test counter", HashMap::new())
            .await;
        collector.increment_counter("test_counter", 5).await;

        let metrics = collector.metrics.read().await;
        let metric = metrics.get("test_counter").unwrap();

        match &metric.value {
            MetricValue::Counter(v) => assert_eq!(*v, 5),
            _ => panic!("Wrong metric type"),
        }
    }

    #[tokio::test]
    async fn test_gauge_metric() {
        let collector = MetricsCollector::new();

        collector
            .register_gauge("test_gauge", "Test gauge", HashMap::new())
            .await;
        collector.set_gauge("test_gauge", 42.5).await;

        let metrics = collector.metrics.read().await;
        let metric = metrics.get("test_gauge").unwrap();

        match &metric.value {
            MetricValue::Gauge(v) => assert_eq!(*v, 42.5),
            _ => panic!("Wrong metric type"),
        }
    }

    #[tokio::test]
    async fn test_prometheus_export() {
        let collector = MetricsCollector::new();

        collector
            .register_counter("test_counter", "Test counter", HashMap::new())
            .await;
        collector.increment_counter("test_counter", 10).await;

        let export = collector.export_prometheus().await;

        assert!(export.contains("# HELP test_counter Test counter"));
        assert!(export.contains("# TYPE test_counter counter"));
        assert!(export.contains("test_counter 10"));
    }
}
