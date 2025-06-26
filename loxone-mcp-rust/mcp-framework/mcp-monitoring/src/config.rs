//! Monitoring configuration

use serde::{Deserialize, Serialize};

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable metrics collection
    pub enabled: bool,
    /// Metrics collection interval in seconds
    pub collection_interval_secs: u64,
    /// Enable performance monitoring
    pub performance_monitoring: bool,
    /// Enable health checks
    pub health_checks: bool,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collection_interval_secs: 60,
            performance_monitoring: true,
            health_checks: true,
        }
    }
}