//! Monitoring, metrics, and observability for MCP servers

pub mod config;
pub mod collector;
pub mod metrics;

pub use config::MonitoringConfig;
pub use collector::MetricsCollector;
pub use metrics::ServerMetrics;

/// Default monitoring configuration
pub fn default_config() -> MonitoringConfig {
    MonitoringConfig::default()
}