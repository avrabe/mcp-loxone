//! Configuration for the unified history system

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration for the history store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    /// Hot storage configuration
    pub hot_storage: HotStorageConfig,

    /// Cold storage configuration
    pub cold_storage: ColdStorageConfig,

    /// Data retention policies
    pub retention: RetentionConfig,

    /// Performance tuning
    pub performance: PerformanceConfig,

    /// Event streaming configuration
    pub streaming: StreamingConfig,
}

/// Hot storage (in-memory) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotStorageConfig {
    /// Maximum device state events per device
    pub device_events_limit: usize,

    /// Sensor data retention in seconds
    pub sensor_retention_seconds: u64,

    /// System metrics retention in seconds
    pub metrics_retention_seconds: u64,

    /// Maximum audit events
    pub audit_events_limit: usize,

    /// Response cache TTL in seconds
    pub response_cache_ttl_seconds: u64,
}

/// Cold storage (persistent) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStorageConfig {
    /// Base directory for cold storage
    pub data_dir: PathBuf,

    /// Compression algorithm
    pub compression: CompressionType,

    /// Maximum storage size in bytes
    pub max_size_bytes: u64,

    /// Index cache size in MB
    pub index_cache_size_mb: usize,
}

/// Data retention policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// Device state retention in days
    pub device_states_days: u32,

    /// Sensor data retention in days
    pub sensor_data_days: u32,

    /// System metrics retention in days
    pub system_metrics_days: u32,

    /// Audit events retention in days
    pub audit_events_days: u32,

    /// Discovery cache retention in days
    pub discovery_cache_days: u32,
}

/// Performance tuning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Write buffer size
    pub write_buffer_size: usize,

    /// Flush interval in seconds
    pub flush_interval_seconds: u64,

    /// Query result cache size in MB
    pub query_cache_size_mb: usize,

    /// Tiering interval in seconds
    pub tiering_interval_seconds: u64,

    /// Background cleanup interval in seconds
    pub cleanup_interval_seconds: u64,
}

/// Event streaming configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    /// Maximum subscribers
    pub max_subscribers: usize,

    /// Event buffer size per subscriber
    pub buffer_size: usize,

    /// Slow subscriber timeout in seconds
    pub slow_subscriber_timeout_seconds: u64,
}

/// Compression types for cold storage
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionType {
    None,
    Gzip,
    Zstd,
    Lz4,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            hot_storage: HotStorageConfig {
                device_events_limit: 100,
                sensor_retention_seconds: 3600, // 1 hour
                metrics_retention_seconds: 900, // 15 minutes
                audit_events_limit: 1000,
                response_cache_ttl_seconds: 300, // 5 minutes
            },
            cold_storage: ColdStorageConfig {
                data_dir: PathBuf::from("/var/lib/loxone-mcp/history"),
                compression: CompressionType::Zstd,
                max_size_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
                index_cache_size_mb: 100,
            },
            retention: RetentionConfig {
                device_states_days: 30,
                sensor_data_days: 90,
                system_metrics_days: 7,
                audit_events_days: 365,
                discovery_cache_days: 0, // Never expire
            },
            performance: PerformanceConfig {
                write_buffer_size: 1000,
                flush_interval_seconds: 10,
                query_cache_size_mb: 100,
                tiering_interval_seconds: 300,  // 5 minutes
                cleanup_interval_seconds: 3600, // 1 hour
            },
            streaming: StreamingConfig {
                max_subscribers: 100,
                buffer_size: 1000,
                slow_subscriber_timeout_seconds: 30,
            },
        }
    }
}

impl HistoryConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Override with environment variables if present
        if let Ok(val) = std::env::var("HISTORY_HOT_DEVICE_EVENTS_LIMIT") {
            if let Ok(limit) = val.parse() {
                config.hot_storage.device_events_limit = limit;
            }
        }

        if let Ok(val) = std::env::var("HISTORY_COLD_DATA_DIR") {
            config.cold_storage.data_dir = PathBuf::from(val);
        }

        if let Ok(val) = std::env::var("HISTORY_RETENTION_DEVICE_DAYS") {
            if let Ok(days) = val.parse() {
                config.retention.device_states_days = days;
            }
        }

        // Add more environment variable overrides as needed

        config
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.hot_storage.device_events_limit == 0 {
            return Err("Device events limit must be greater than 0".to_string());
        }

        if self.cold_storage.max_size_bytes < 1024 * 1024 {
            return Err("Cold storage size must be at least 1 MB".to_string());
        }

        if self.performance.write_buffer_size == 0 {
            return Err("Write buffer size must be greater than 0".to_string());
        }

        Ok(())
    }
}
