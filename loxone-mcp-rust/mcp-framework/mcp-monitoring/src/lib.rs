//! Monitoring, metrics, and observability for MCP servers
//!
//! This crate provides comprehensive monitoring capabilities for MCP servers including:
//! - Real-time metrics collection and reporting
//! - Health checks and system monitoring
//! - Performance profiling and optimization insights
//! - InfluxDB integration for time-series data
//! - Prometheus-compatible metrics export
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use pulseengine_mcp_monitoring::{MetricsCollector, MonitoringConfig, ServerMetrics};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create monitoring configuration
//!     let config = MonitoringConfig {
//!         enable_metrics: true,
//!         metrics_port: 9090,
//!         health_check_interval: Duration::from_secs(30),
//!         influxdb_url: Some("http://localhost:8086".to_string()),
//!         influxdb_database: Some("mcp_metrics".to_string()),
//!         ..Default::default()
//!     };
//!
//!     // Create metrics collector
//!     let mut collector = MetricsCollector::new(config);
//!     collector.start().await?;
//!
//!     // Record metrics in your application
//!     collector.record_request_duration(Duration::from_millis(50)).await;
//!     collector.increment_request_count("tool_call").await;
//!     collector.record_error("connection_timeout").await;
//!
//!     // Get current metrics snapshot
//!     let metrics = collector.get_metrics().await;
//!     println!("Total requests: {}", metrics.request_count);
//!     println!("Average response time: {:?}", metrics.avg_response_time);
//!
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! - **Real-time metrics**: Live request/response time tracking
//! - **Health monitoring**: System resource and connectivity checks
//! - **Time-series storage**: InfluxDB integration for historical data
//! - **Prometheus export**: Industry-standard metrics format
//! - **Performance profiling**: Identify bottlenecks and optimization opportunities
//! - **Production ready**: Low overhead, highly optimized collection

pub mod collector;
pub mod config;
pub mod metrics;

pub use collector::MetricsCollector;
pub use config::MonitoringConfig;
pub use metrics::ServerMetrics;

/// Default monitoring configuration
pub fn default_config() -> MonitoringConfig {
    MonitoringConfig::default()
}
