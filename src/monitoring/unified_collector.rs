//! Unified data collection service for dashboard
//!
//! This service replaces the fragmented data collection approach with a single
//! pipeline that feeds both real-time dashboard updates and historical storage.

use crate::client::LoxoneClient;
use crate::error::Result;
// Removed history import - module was unused
// Legacy http_transport disabled during framework migration
// use crate::http_transport::rate_limiting::RateLimitResult;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info};

/// Unified data collector for dashboard
pub struct UnifiedDataCollector {
    /// Loxone clients for different endpoints
    clients: HashMap<String, Arc<dyn LoxoneClient>>,

    /// Real-time data broadcast
    realtime_tx: broadcast::Sender<DashboardUpdate>,

    /// Operational metrics collector
    operational_metrics: Arc<RwLock<OperationalMetrics>>,

    /// Collection state
    state: Arc<RwLock<CollectorState>>,

    /// Configuration
    config: CollectorConfig,
}

/// Configuration for data collector
#[derive(Debug, Clone)]
pub struct CollectorConfig {
    /// Collection interval in seconds
    pub collection_interval_seconds: u64,

    /// Enable historical data storage
    pub enable_history: bool,

    /// Maximum clients to track
    pub max_clients: usize,

    /// Operational metrics retention (in minutes)
    pub metrics_retention_minutes: u64,
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            collection_interval_seconds: 5,
            enable_history: std::env::var("ENABLE_LOXONE_STATS").unwrap_or_default() == "1",
            max_clients: 10,
            metrics_retention_minutes: 60,
        }
    }
}

/// Current state of the data collector
#[derive(Debug, Default)]
struct CollectorState {
    /// Is the collector running
    running: bool,

    /// Last collection timestamp
    last_collection: Option<DateTime<Utc>>,

    /// Collection statistics
    stats: CollectionStats,

    /// Current dashboard data
    current_data: DashboardData,
}

/// Collection statistics
#[derive(Debug, Default)]
struct CollectionStats {
    total_collections: u64,
    successful_collections: u64,
    failed_collections: u64,
    average_collection_time_ms: f64,
    last_error: Option<String>,
}

/// Real-time dashboard update message
#[derive(Debug, Clone, Serialize)]
pub struct DashboardUpdate {
    /// Update type
    pub update_type: UpdateType,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Update payload
    pub data: serde_json::Value,
}

/// Types of dashboard updates
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum UpdateType {
    /// Device state change
    DeviceState { device_uuid: String, room: String },

    /// Sensor reading update
    SensorReading { sensor_uuid: String, room: String },

    /// System metric update
    SystemMetric { metric_name: String },

    /// Operational event
    Operational { category: String },

    /// Full data refresh
    FullRefresh,
}

/// Complete dashboard data structure
#[derive(Debug, Clone, Serialize, Default)]
pub struct DashboardData {
    /// Real-time monitoring data
    pub realtime: RealtimeData,

    /// Device and room overview
    pub devices: DeviceOverview,

    /// Operational metrics
    pub operational: OperationalMetrics,

    /// Quick historical summaries
    pub trends: TrendSummary,

    /// Metadata
    pub metadata: DashboardMetadata,
}

/// Real-time monitoring section
#[derive(Debug, Clone, Serialize, Default)]
pub struct RealtimeData {
    /// System health indicators
    pub system_health: SystemHealth,

    /// Active sensor readings
    pub active_sensors: Vec<SensorReading>,

    /// Recent activity (last 10 events)
    pub recent_activity: Vec<ActivityEvent>,
}

/// System health indicators
#[derive(Debug, Clone, Serialize)]
pub struct SystemHealth {
    /// Loxone connection status
    pub connection_status: ConnectionStatus,

    /// Last successful update
    pub last_update: DateTime<Utc>,

    /// Error rate (errors per minute)
    pub error_rate: f64,

    /// Response time (average over last 5 minutes)
    pub avg_response_time_ms: f64,
}

impl Default for SystemHealth {
    fn default() -> Self {
        Self {
            connection_status: ConnectionStatus::Disconnected,
            last_update: Utc::now(),
            error_rate: 0.0,
            avg_response_time_ms: 0.0,
        }
    }
}

/// Connection status enumeration
#[derive(Debug, Clone, Serialize)]
pub enum ConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Error(String),
}

/// Device and room overview section
#[derive(Debug, Clone, Serialize, Default)]
pub struct DeviceOverview {
    /// Room data with temperature controllers
    pub rooms: Vec<RoomData>,

    /// Device status matrix
    pub device_matrix: HashMap<String, Vec<DeviceStatus>>,

    /// Quick control devices
    pub quick_controls: Vec<QuickControl>,
}

/// Room data with climate information
#[derive(Debug, Clone, Serialize)]
pub struct RoomData {
    /// Room name
    pub name: String,

    /// Current temperature
    pub current_temp: Option<f64>,

    /// Target temperature
    pub target_temp: Option<f64>,

    /// Climate controller UUID
    pub controller_uuid: Option<String>,

    /// Device count in room
    pub device_count: usize,

    /// Active devices in room
    pub active_devices: usize,
}

/// Device status in matrix view
#[derive(Debug, Clone, Serialize)]
pub struct DeviceStatus {
    /// Device UUID
    pub uuid: String,

    /// Device name
    pub name: String,

    /// Device type
    pub device_type: String,

    /// Current state
    pub state: serde_json::Value,

    /// Status indicator
    pub status: StatusIndicator,

    /// Last update
    pub last_update: DateTime<Utc>,
}

/// Status indicator colors
#[derive(Debug, Clone, Serialize)]
pub enum StatusIndicator {
    Active,   // Green
    Inactive, // Gray
    Warning,  // Yellow
    Error,    // Red
    Unknown,  // Blue
}

/// Quick control widget
#[derive(Debug, Clone, Serialize)]
pub struct QuickControl {
    /// Device UUID
    pub uuid: String,

    /// Display name
    pub name: String,

    /// Control type
    pub control_type: ControlType,

    /// Current value
    pub current_value: serde_json::Value,

    /// Available commands
    pub commands: Vec<String>,
}

/// Types of quick controls
#[derive(Debug, Clone, Serialize)]
pub enum ControlType {
    Switch,      // On/Off
    Dimmer,      // 0-100%
    Blinds,      // Up/Down/Position
    Temperature, // Set temperature
}

/// Operational metrics section
#[derive(Debug, Clone, Serialize, Default)]
pub struct OperationalMetrics {
    /// API performance metrics
    pub api_performance: ApiPerformanceMetrics,

    /// Rate limiter status
    pub rate_limiter: RateLimiterMetrics,

    /// Security events
    pub security_events: SecurityMetrics,

    /// Resource utilization
    pub resources: ResourceMetrics,
}

/// API performance tracking
#[derive(Debug, Clone, Serialize, Default)]
pub struct ApiPerformanceMetrics {
    /// Requests per minute
    pub requests_per_minute: f64,

    /// Average response time (ms)
    pub avg_response_time_ms: f64,

    /// Error rate percentage
    pub error_rate_percent: f64,

    /// Slowest endpoints
    pub slow_endpoints: Vec<EndpointMetric>,

    /// Recent performance history (last hour)
    pub performance_history: Vec<PerformanceDataPoint>,
}

/// Individual endpoint metrics
#[derive(Debug, Clone, Serialize)]
pub struct EndpointMetric {
    pub path: String,
    pub avg_response_time_ms: f64,
    pub request_count: u64,
    pub error_count: u64,
}

/// Performance data point for trending
#[derive(Debug, Clone, Serialize)]
pub struct PerformanceDataPoint {
    pub timestamp: DateTime<Utc>,
    pub response_time_ms: f64,
    pub request_count: u64,
    pub error_count: u64,
}

/// Rate limiter metrics
#[derive(Debug, Clone, Serialize, Default)]
pub struct RateLimiterMetrics {
    /// Current active clients
    pub active_clients: u32,

    /// Rate limit hits in last hour
    pub recent_hits: u32,

    /// Blocked requests
    pub blocked_requests: u32,

    /// Top offending IPs
    pub top_offenders: Vec<IpActivity>,

    /// Rate limit efficiency
    pub efficiency_percent: f64,
}

/// IP activity tracking
#[derive(Debug, Clone, Serialize)]
pub struct IpActivity {
    pub ip: String,
    pub request_count: u32,
    pub blocked_count: u32,
    pub last_activity: DateTime<Utc>,
}

/// Security metrics
#[derive(Debug, Clone, Serialize, Default)]
pub struct SecurityMetrics {
    /// Failed authentication attempts
    pub auth_failures: u32,

    /// Suspicious activity count
    pub suspicious_activity: u32,

    /// Recent security events
    pub recent_events: Vec<SecurityEvent>,

    /// Security score (0-100)
    pub security_score: u8,
}

/// Security event entry
#[derive(Debug, Clone, Serialize)]
pub struct SecurityEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub source_ip: String,
    pub description: String,
    pub severity: SecuritySeverity,
}

/// Security event severity
#[derive(Debug, Clone, Serialize)]
pub enum SecuritySeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Resource utilization metrics
#[derive(Debug, Clone, Serialize, Default)]
pub struct ResourceMetrics {
    /// WebSocket connection count
    pub websocket_connections: u32,

    /// Memory usage (MB)
    pub memory_usage_mb: f64,

    /// CPU usage percentage
    pub cpu_usage_percent: f64,

    /// Disk usage percentage
    pub disk_usage_percent: f64,

    /// Network activity
    pub network_activity: NetworkActivity,
}

/// Network activity metrics
#[derive(Debug, Clone, Serialize, Default)]
pub struct NetworkActivity {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub active_connections: u32,
}

/// Trend summary section
#[derive(Debug, Clone, Serialize, Default)]
pub struct TrendSummary {
    /// Temperature trends for last 24h
    pub temperature_trends: Vec<TemperatureTrend>,

    /// Device usage patterns
    pub device_usage: Vec<DeviceUsagePattern>,

    /// System performance trends
    pub performance_trends: Vec<PerformanceDataPoint>,
}

/// Temperature trend for a room
#[derive(Debug, Clone, Serialize)]
pub struct TemperatureTrend {
    pub room_name: String,
    pub data_points: Vec<TemperatureDataPoint>,
    pub min_temp: f64,
    pub max_temp: f64,
    pub avg_temp: f64,
}

/// Temperature data point
#[derive(Debug, Clone, Serialize)]
pub struct TemperatureDataPoint {
    pub timestamp: DateTime<Utc>,
    pub temperature: f64,
    pub target: Option<f64>,
}

/// Device usage pattern
#[derive(Debug, Clone, Serialize)]
pub struct DeviceUsagePattern {
    pub device_name: String,
    pub device_type: String,
    pub activation_count: u32,
    pub total_runtime_minutes: u32,
    pub peak_usage_hour: u8,
}

/// Dashboard metadata
#[derive(Debug, Clone, Serialize)]
pub struct DashboardMetadata {
    /// Last full update
    pub last_update: DateTime<Utc>,

    /// Data freshness (seconds since last Loxone poll)
    pub data_age_seconds: u64,

    /// Collection statistics
    pub collection_stats: CollectionStatsPublic,

    /// Dashboard version
    pub version: String,
}

impl Default for DashboardMetadata {
    fn default() -> Self {
        Self {
            last_update: Utc::now(),
            data_age_seconds: 0,
            collection_stats: CollectionStatsPublic::default(),
            version: "1.0.0".to_string(),
        }
    }
}

/// Public collection statistics
#[derive(Debug, Clone, Serialize, Default)]
pub struct CollectionStatsPublic {
    pub total_collections: u64,
    pub success_rate_percent: f64,
    pub avg_collection_time_ms: f64,
    pub last_error: Option<String>,
}

/// Sensor reading for dashboard
#[derive(Debug, Clone, Serialize)]
pub struct SensorReading {
    pub uuid: String,
    pub name: String,
    pub room: String,
    pub value: serde_json::Value,
    pub unit: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub status: StatusIndicator,
}

/// Activity event for recent activity
#[derive(Debug, Clone, Serialize)]
pub struct ActivityEvent {
    pub timestamp: DateTime<Utc>,
    pub device_name: String,
    pub room: String,
    pub action: String,
    pub details: String,
}

impl UnifiedDataCollector {
    /// Create new unified data collector
    pub fn new(clients: HashMap<String, Arc<dyn LoxoneClient>>, config: CollectorConfig) -> Self {
        let (realtime_tx, _) = broadcast::channel(1000);

        Self {
            clients,
            realtime_tx,
            operational_metrics: Arc::new(RwLock::new(OperationalMetrics::default())),
            state: Arc::new(RwLock::new(CollectorState::default())),
            config,
        }
    }

    /// Start the data collection service
    pub async fn start(&self) -> Result<()> {
        info!(
            "Starting unified data collector (interval: {}s)",
            self.config.collection_interval_seconds
        );

        {
            let mut state = self.state.write().await;
            state.running = true;
        }

        // Start collection loop
        self.start_collection_loop().await;

        // Start metrics cleanup task
        self.start_metrics_cleanup().await;

        Ok(())
    }

    /// Stop the data collection service
    pub async fn stop(&self) {
        info!("Stopping unified data collector");
        let mut state = self.state.write().await;
        state.running = false;
    }

    /// Get current dashboard data
    pub async fn get_dashboard_data(&self) -> DashboardData {
        let state = self.state.read().await;
        state.current_data.clone()
    }

    /// Subscribe to real-time updates
    pub fn subscribe_updates(&self) -> broadcast::Receiver<DashboardUpdate> {
        self.realtime_tx.subscribe()
    }

    // Legacy rate limiter event recording - disabled during framework migration
    // Use framework middleware instead
    // pub async fn record_rate_limit_event(&self, result: RateLimitResult, client_ip: String) {
    //     // Disabled - use framework rate limiting instead
    // }

    /// Record API performance data
    pub async fn record_api_performance(
        &self,
        _endpoint: String,
        response_time_ms: f64,
        was_error: bool,
    ) {
        let mut metrics = self.operational_metrics.write().await;

        // Update overall metrics
        metrics.api_performance.requests_per_minute += 1.0 / 60.0; // Approximate

        // Update response time (simple moving average)
        let current_avg = metrics.api_performance.avg_response_time_ms;
        metrics.api_performance.avg_response_time_ms =
            (current_avg * 0.9) + (response_time_ms * 0.1);

        // Update error rate
        if was_error {
            metrics.api_performance.error_rate_percent =
                (metrics.api_performance.error_rate_percent * 0.95) + 5.0;
        } else {
            metrics.api_performance.error_rate_percent *= 0.95;
        }

        // Add to performance history
        metrics
            .api_performance
            .performance_history
            .push(PerformanceDataPoint {
                timestamp: Utc::now(),
                response_time_ms,
                request_count: 1,
                error_count: if was_error { 1 } else { 0 },
            });

        // Keep only last hour of data
        let cutoff = Utc::now() - chrono::Duration::hours(1);
        metrics
            .api_performance
            .performance_history
            .retain(|point| point.timestamp > cutoff);
    }

    /// Start collection loop
    async fn start_collection_loop(&self) {
        let state = self.state.clone();
        let clients = self.clients.clone();
        let realtime_tx = self.realtime_tx.clone();
        let operational_metrics = self.operational_metrics.clone();
        let interval_secs = self.config.collection_interval_seconds;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                {
                    let state_guard = state.read().await;
                    if !state_guard.running {
                        break;
                    }
                }

                let start_time = Instant::now();

                match Self::collect_data(&clients, &realtime_tx, &operational_metrics).await {
                    Ok(dashboard_data) => {
                        let collection_time = start_time.elapsed().as_millis() as f64;

                        let mut state_guard = state.write().await;
                        state_guard.last_collection = Some(Utc::now());
                        state_guard.stats.total_collections += 1;
                        state_guard.stats.successful_collections += 1;

                        // Update average collection time
                        let current_avg = state_guard.stats.average_collection_time_ms;
                        state_guard.stats.average_collection_time_ms =
                            (current_avg * 0.9) + (collection_time * 0.1);

                        state_guard.current_data = dashboard_data;

                        debug!("Data collection completed in {:.2}ms", collection_time);
                    }
                    Err(e) => {
                        error!("Data collection failed: {}", e);

                        let mut state_guard = state.write().await;
                        state_guard.stats.total_collections += 1;
                        state_guard.stats.failed_collections += 1;
                        state_guard.stats.last_error = Some(e.to_string());
                    }
                }
            }

            info!("Data collection loop stopped");
        });
    }

    /// Collect data from all sources
    async fn collect_data(
        _clients: &HashMap<String, Arc<dyn LoxoneClient>>,
        realtime_tx: &broadcast::Sender<DashboardUpdate>,
        _operational_metrics: &Arc<RwLock<OperationalMetrics>>,
    ) -> Result<DashboardData> {
        // This is a placeholder implementation
        // In the real implementation, this would:
        // 1. Poll all Loxone clients for current state
        // 2. Process device and sensor data
        // 3. Update operational metrics
        // 4. Broadcast real-time updates
        // 5. Return consolidated dashboard data

        let dashboard_data = DashboardData {
            metadata: DashboardMetadata {
                last_update: Utc::now(),
                data_age_seconds: 0,
                collection_stats: CollectionStatsPublic::default(),
                version: "1.0.0".to_string(),
            },
            ..Default::default()
        };

        // Broadcast full refresh
        let _ = realtime_tx.send(DashboardUpdate {
            update_type: UpdateType::FullRefresh,
            timestamp: Utc::now(),
            data: serde_json::to_value(&dashboard_data).unwrap_or_default(),
        });

        Ok(dashboard_data)
    }

    /// Start metrics cleanup task
    async fn start_metrics_cleanup(&self) {
        let operational_metrics = self.operational_metrics.clone();
        let retention_minutes = self.config.metrics_retention_minutes;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // Every 5 minutes

            loop {
                interval.tick().await;

                let cutoff = Utc::now() - chrono::Duration::minutes(retention_minutes as i64);
                let mut metrics = operational_metrics.write().await;

                // Clean old performance data
                metrics
                    .api_performance
                    .performance_history
                    .retain(|point| point.timestamp > cutoff);

                // Clean old IP activities
                metrics
                    .rate_limiter
                    .top_offenders
                    .retain(|ip| ip.last_activity > cutoff);

                // Clean old security events
                metrics
                    .security_events
                    .recent_events
                    .retain(|event| event.timestamp > cutoff);

                debug!(
                    "Cleaned operational metrics older than {} minutes",
                    retention_minutes
                );
            }
        });
    }
}
