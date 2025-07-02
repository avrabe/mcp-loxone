//! InfluxDB integration for time series data storage
//!
//! This module provides comprehensive InfluxDB integration for storing:
//! - Loxone sensor data and device states
//! - MCP server metrics and performance data
//! - Authentication and rate limiting statistics
//! - Historical data for trend analysis

use crate::error::{LoxoneError, Result};
use chrono::{DateTime, Utc};
use futures::stream;
use influxdb2::api::write::TimestampPrecision;
use influxdb2::models::DataPoint;
use influxdb2::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

/// InfluxDB configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfluxConfig {
    /// InfluxDB URL (e.g., http://localhost:8086)
    pub url: String,
    /// Organization name
    pub org: String,
    /// API token for authentication
    pub token: String,
    /// Default bucket for metrics
    pub bucket: String,
    /// Whether to create bucket if it doesn't exist
    pub create_bucket: bool,
    /// Retention policy in seconds (0 = infinite)
    pub retention_seconds: u64,
    /// Batch size for writes
    pub batch_size: usize,
    /// Flush interval in seconds
    pub flush_interval_seconds: u64,
}

impl Default for InfluxConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8086".to_string(),
            org: "loxone-mcp".to_string(),
            token: std::env::var("INFLUXDB_TOKEN").unwrap_or_default(),
            bucket: "loxone_metrics".to_string(),
            create_bucket: true,
            retention_seconds: 30 * 24 * 3600, // 30 days default
            batch_size: 1000,
            flush_interval_seconds: 10,
        }
    }
}

/// Loxone sensor data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneSensorData {
    /// Sensor UUID
    pub uuid: String,
    /// Sensor name
    pub name: String,
    /// Sensor type (temperature, humidity, etc.)
    pub sensor_type: String,
    /// Room assignment
    pub room: Option<String>,
    /// Sensor value
    pub value: f64,
    /// Unit of measurement
    pub unit: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Device state data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStateData {
    /// Device UUID
    pub uuid: String,
    /// Device name
    pub name: String,
    /// Device type
    pub device_type: String,
    /// Room assignment
    pub room: Option<String>,
    /// State value (on/off, position, etc.)
    pub state: String,
    /// Numeric value if applicable
    pub value: Option<f64>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// MCP server metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMetrics {
    /// Total requests
    pub total_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Active connections
    pub active_connections: u32,
    /// Average response time in ms
    pub avg_response_time_ms: f64,
    /// CPU usage percentage
    pub cpu_usage: f64,
    /// Memory usage in MB
    pub memory_usage_mb: f64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Authentication metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMetrics {
    /// Total authentication attempts
    pub total_attempts: u64,
    /// Successful authentications
    pub successful_auths: u64,
    /// Failed authentications
    pub failed_auths: u64,
    /// Active sessions
    pub active_sessions: u32,
    /// API keys in use
    pub active_api_keys: u32,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Rate limiting metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitMetrics {
    /// Total rate limit checks
    pub total_checks: u64,
    /// Requests allowed
    pub requests_allowed: u64,
    /// Requests limited
    pub requests_limited: u64,
    /// Clients in penalty
    pub penalized_clients: u32,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Batch write buffer
#[derive(Debug)]
struct WriteBuffer {
    /// Buffered data points
    points: Vec<DataPoint>,
    /// Last flush time
    last_flush: DateTime<Utc>,
}

impl WriteBuffer {
    fn new() -> Self {
        Self {
            points: Vec::new(),
            last_flush: Utc::now(),
        }
    }

    fn add_point(&mut self, point: DataPoint) {
        self.points.push(point);
    }

    fn should_flush(&self, batch_size: usize, flush_interval: Duration) -> bool {
        self.points.len() >= batch_size
            || Utc::now()
                .signed_duration_since(self.last_flush)
                .num_seconds() as u64
                >= flush_interval.as_secs()
    }

    fn take_points(&mut self) -> Vec<DataPoint> {
        self.last_flush = Utc::now();
        std::mem::take(&mut self.points)
    }
}

/// InfluxDB client manager
pub struct InfluxManager {
    /// InfluxDB client
    client: Client,
    /// Configuration
    config: InfluxConfig,
    /// Write buffer
    buffer: Arc<RwLock<WriteBuffer>>,
    /// Running flag
    running: Arc<RwLock<bool>>,
}

impl InfluxManager {
    /// Create new InfluxDB manager
    pub async fn new(config: InfluxConfig) -> Result<Self> {
        let client = Client::new(&config.url, &config.org, &config.token);

        // Verify connection and create bucket if needed
        if config.create_bucket {
            match Self::ensure_bucket(&client, &config).await {
                Ok(_) => info!("InfluxDB bucket '{}' is ready", config.bucket),
                Err(e) => {
                    error!("Failed to ensure bucket: {}", e);
                    return Err(LoxoneError::connection(format!(
                        "InfluxDB bucket setup failed: {e}"
                    )));
                }
            }
        }

        let manager = Self {
            client,
            config,
            buffer: Arc::new(RwLock::new(WriteBuffer::new())),
            running: Arc::new(RwLock::new(true)),
        };

        // Start background flush task
        manager.start_flush_task();

        Ok(manager)
    }

    /// Ensure bucket exists
    async fn ensure_bucket(client: &Client, config: &InfluxConfig) -> Result<()> {
        // Check if bucket exists by trying to get bucket info
        // Note: influxdb2-client doesn't have a direct list_buckets method,
        // so we'll try to write a test point to check if bucket exists
        let test_point = DataPoint::builder("test")
            .field("value", 1)
            .build()
            .map_err(|e| LoxoneError::config(format!("Failed to build test point: {e}")))?;

        match client
            .write(&config.bucket, stream::once(async { test_point }))
            .await
        {
            Ok(_) => {
                debug!("Bucket '{}' exists and is writable", config.bucket);
                return Ok(());
            }
            Err(e) => {
                warn!("Bucket '{}' might not exist: {}", config.bucket, e);
            }
        }

        // Note: Bucket creation via API requires admin permissions
        // For now, we'll assume the bucket exists or needs to be created manually
        info!(
            "Please ensure InfluxDB bucket '{}' exists with appropriate retention policy",
            config.bucket
        );
        Ok(())
    }

    /// Start background flush task
    fn start_flush_task(&self) {
        let buffer = self.buffer.clone();
        let client = self.client.clone();
        let config = self.config.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut flush_interval = interval(Duration::from_secs(config.flush_interval_seconds));
            flush_interval.tick().await; // Skip first immediate tick

            while *running.read().await {
                flush_interval.tick().await;

                let mut write_buffer = buffer.write().await;
                if write_buffer.should_flush(
                    config.batch_size,
                    Duration::from_secs(config.flush_interval_seconds),
                ) {
                    let points = write_buffer.take_points();
                    drop(write_buffer); // Release lock before writing

                    if !points.is_empty() {
                        debug!("Flushing {} data points to InfluxDB", points.len());
                        if let Err(e) = client
                            .write_with_precision(
                                &config.bucket,
                                stream::iter(points),
                                TimestampPrecision::Milliseconds,
                            )
                            .await
                        {
                            error!("Failed to write to InfluxDB: {}", e);
                        }
                    }
                }
            }
        });
    }

    /// Write sensor data
    pub async fn write_sensor_data(&self, data: LoxoneSensorData) -> Result<()> {
        let sensor_name = &data.sensor_type;
        let mut point = DataPoint::builder(format!("sensor_{sensor_name}"))
            .tag("uuid", data.uuid)
            .tag("name", data.name)
            .tag("type", data.sensor_type)
            .field("value", data.value)
            .field("unit", data.unit);

        if let Some(room) = data.room {
            point = point.tag("room", room);
        }

        let point = point
            .timestamp(data.timestamp.timestamp_millis())
            .build()
            .map_err(|e| LoxoneError::invalid_input(format!("Failed to build data point: {e}")))?;

        self.buffer.write().await.add_point(point);
        Ok(())
    }

    /// Write device state
    pub async fn write_device_state(&self, data: DeviceStateData) -> Result<()> {
        let mut point = DataPoint::builder("device_state")
            .tag("uuid", data.uuid)
            .tag("name", data.name)
            .tag("type", data.device_type)
            .field("state", data.state);

        if let Some(room) = data.room {
            point = point.tag("room", room);
        }

        if let Some(value) = data.value {
            point = point.field("value", value);
        }

        let point = point
            .timestamp(data.timestamp.timestamp_millis())
            .build()
            .map_err(|e| LoxoneError::invalid_input(format!("Failed to build data point: {e}")))?;

        self.buffer.write().await.add_point(point);
        Ok(())
    }

    /// Write MCP metrics
    pub async fn write_mcp_metrics(&self, metrics: McpMetrics) -> Result<()> {
        let point = DataPoint::builder("mcp_metrics")
            .field("total_requests", metrics.total_requests as i64)
            .field("failed_requests", metrics.failed_requests as i64)
            .field("active_connections", metrics.active_connections as i64)
            .field("avg_response_time_ms", metrics.avg_response_time_ms)
            .field("cpu_usage", metrics.cpu_usage)
            .field("memory_usage_mb", metrics.memory_usage_mb)
            .timestamp(metrics.timestamp.timestamp_millis())
            .build()
            .map_err(|e| LoxoneError::invalid_input(format!("Failed to build data point: {e}")))?;

        self.buffer.write().await.add_point(point);
        Ok(())
    }

    /// Write authentication metrics
    pub async fn write_auth_metrics(&self, metrics: AuthMetrics) -> Result<()> {
        let point = DataPoint::builder("auth_metrics")
            .field("total_attempts", metrics.total_attempts as i64)
            .field("successful_auths", metrics.successful_auths as i64)
            .field("failed_auths", metrics.failed_auths as i64)
            .field("active_sessions", metrics.active_sessions as i64)
            .field("active_api_keys", metrics.active_api_keys as i64)
            .timestamp(metrics.timestamp.timestamp_millis())
            .build()
            .map_err(|e| LoxoneError::invalid_input(format!("Failed to build data point: {e}")))?;

        self.buffer.write().await.add_point(point);
        Ok(())
    }

    /// Write rate limit metrics
    pub async fn write_rate_limit_metrics(&self, metrics: RateLimitMetrics) -> Result<()> {
        let point = DataPoint::builder("rate_limit_metrics")
            .field("total_checks", metrics.total_checks as i64)
            .field("requests_allowed", metrics.requests_allowed as i64)
            .field("requests_limited", metrics.requests_limited as i64)
            .field("penalized_clients", metrics.penalized_clients as i64)
            .timestamp(metrics.timestamp.timestamp_millis())
            .build()
            .map_err(|e| LoxoneError::invalid_input(format!("Failed to build data point: {e}")))?;

        self.buffer.write().await.add_point(point);
        Ok(())
    }

    /// Force flush all buffered data
    pub async fn flush(&self) -> Result<()> {
        let points = self.buffer.write().await.take_points();

        if !points.is_empty() {
            info!("Force flushing {} data points to InfluxDB", points.len());
            self.client
                .write_with_precision(
                    &self.config.bucket,
                    stream::iter(points),
                    TimestampPrecision::Milliseconds,
                )
                .await
                .map_err(|e| LoxoneError::invalid_input(format!("InfluxDB write failed: {e}")))?;
        }

        Ok(())
    }

    /// Query recent sensor data
    pub async fn query_sensor_history(
        &self,
        sensor_uuid: &str,
        duration: &str,
    ) -> Result<Vec<(DateTime<Utc>, f64)>> {
        let _query = format!(
            r#"
            from(bucket: "{}")
                |> range(start: -{})
                |> filter(fn: (r) => r["_measurement"] =~ /^sensor_/ and r["uuid"] == "{}")
                |> filter(fn: (r) => r["_field"] == "value")
                |> sort(columns: ["_time"])
            "#,
            self.config.bucket, duration, sensor_uuid
        );

        // For now, return empty data - query API needs proper type implementation
        // TODO: Implement proper InfluxDB query result parsing
        let data = Vec::new();

        Ok(data)
    }

    /// Query device state history
    pub async fn query_device_history(
        &self,
        device_uuid: &str,
        duration: &str,
    ) -> Result<Vec<(DateTime<Utc>, String, Option<f64>)>> {
        let _query = format!(
            r#"
            from(bucket: "{}")
                |> range(start: -{})
                |> filter(fn: (r) => r["_measurement"] == "device_state" and r["uuid"] == "{}")
                |> filter(fn: (r) => r["_field"] == "state" or r["_field"] == "value")
                |> pivot(rowKey:["_time"], columnKey: ["_field"], valueColumn: "_value")
                |> sort(columns: ["_time"])
            "#,
            self.config.bucket, duration, device_uuid
        );

        // For now, return empty data - query API needs proper type implementation
        // TODO: Implement proper InfluxDB query result parsing
        let data = Vec::new();

        Ok(data)
    }

    /// Get aggregated metrics for dashboard
    pub async fn get_dashboard_metrics(
        &self,
        _duration: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let metrics = HashMap::new();

        // TODO: Implement dashboard metrics queries
        // This would include:
        // - Average sensor values by room
        // - MCP server performance metrics
        // - System resource usage over time

        Ok(metrics)
    }

    /// Shutdown manager
    pub async fn shutdown(&self) {
        *self.running.write().await = false;

        // Final flush
        if let Err(e) = self.flush().await {
            error!("Failed to flush data during shutdown: {}", e);
        }

        info!("InfluxDB manager shut down");
    }
}

impl Drop for InfluxManager {
    fn drop(&mut self) {
        // Note: Can't do async operations in drop, so shutdown should be called explicitly
        if *self.running.blocking_read() {
            warn!("InfluxManager dropped without calling shutdown()");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_influx_config_default() {
        let config = InfluxConfig::default();
        assert_eq!(config.url, "http://localhost:8086");
        assert_eq!(config.org, "loxone-mcp");
        assert_eq!(config.bucket, "loxone_metrics");
        assert_eq!(config.retention_seconds, 30 * 24 * 3600);
    }

    #[tokio::test]
    async fn test_write_buffer() {
        let mut buffer = WriteBuffer::new();

        let point = DataPoint::builder("test")
            .field("value", 42.0)
            .build()
            .unwrap();

        buffer.add_point(point);
        assert_eq!(buffer.points.len(), 1);

        let points = buffer.take_points();
        assert_eq!(points.len(), 1);
        assert_eq!(buffer.points.len(), 0);
    }
}
