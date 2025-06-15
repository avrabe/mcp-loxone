//! Dynamic dashboard that auto-discovers available data sources

use super::core::UnifiedHistoryStore;
use super::events::*;
use crate::client::ClientContext;
use crate::error::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, info};

/// Dynamic dashboard that adapts to available data
pub struct DynamicDashboard {
    history_store: Arc<UnifiedHistoryStore>,
    client_context: Arc<ClientContext>,
    discovery_cache: Arc<tokio::sync::RwLock<DiscoveryCache>>,
}

/// Cache of discovered data sources
#[derive(Debug, Default)]
struct DiscoveryCache {
    available_rooms: HashSet<String>,
    available_devices: HashMap<String, DeviceInfo>,
    available_sensors: HashMap<String, SensorInfo>,
    available_metrics: HashSet<String>,
    last_discovery: Option<DateTime<Utc>>,
    discovery_stats: DiscoveryStats,
}

#[derive(Debug, Default)]
struct DiscoveryStats {
    total_events_analyzed: u64,
    unique_sources_found: u64,
    last_activity: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
struct DeviceInfo {
    name: String,
    device_type: String,
    room: Option<String>,
    last_seen: DateTime<Utc>,
    state_count: u32,
    is_active: bool,
}

#[derive(Debug, Clone, Serialize)]
struct SensorInfo {
    name: String,
    sensor_type: String,
    unit: String,
    room: Option<String>,
    last_reading: DateTime<Utc>,
    reading_count: u32,
    min_value: f64,
    max_value: f64,
    avg_value: f64,
}

/// Dynamic dashboard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicDashboardConfig {
    /// Auto-discovery interval in seconds
    pub discovery_interval_seconds: u64,

    /// Minimum events needed to show a data source
    pub min_events_threshold: u32,

    /// Time window for "recent" data (hours)
    pub recent_window_hours: u32,

    /// Maximum widgets to show per category
    pub max_widgets_per_category: usize,

    /// Enable real-time updates
    pub enable_real_time: bool,
}

impl Default for DynamicDashboardConfig {
    fn default() -> Self {
        Self {
            discovery_interval_seconds: 300, // 5 minutes
            min_events_threshold: 5,
            recent_window_hours: 24,
            max_widgets_per_category: 10,
            enable_real_time: true,
        }
    }
}

impl DynamicDashboard {
    /// Create new dynamic dashboard
    pub fn new(
        history_store: Arc<UnifiedHistoryStore>,
        client_context: Arc<ClientContext>,
    ) -> Self {
        Self {
            history_store,
            client_context,
            discovery_cache: Arc::new(tokio::sync::RwLock::new(DiscoveryCache::default())),
        }
    }

    /// Discover available data sources
    pub async fn discover_data_sources(&self) -> Result<()> {
        info!("Starting dynamic dashboard data discovery");
        let start_time = std::time::Instant::now();

        let now = Utc::now();
        let analysis_window = now - Duration::hours(24);

        // Query recent events for analysis
        let events = self
            .history_store
            .query()
            .since(analysis_window)
            .execute()
            .await?
            .events;

        let mut cache = self.discovery_cache.write().await;
        cache.available_rooms.clear();
        cache.available_devices.clear();
        cache.available_sensors.clear();
        cache.available_metrics.clear();

        debug!("Analyzing {} events for data sources", events.len());

        // Analyze events to discover data sources
        for event in &events {
            match &event.category {
                EventCategory::DeviceState(state) => {
                    self.process_device_event(state, &event.timestamp, &mut cache)
                        .await;
                }
                EventCategory::SensorReading(data) => {
                    self.process_sensor_event(data, &event.timestamp, &mut cache)
                        .await;
                }
                EventCategory::SystemMetric(metric) => {
                    cache.available_metrics.insert(metric.metric_name.clone());
                }
                _ => {} // Skip other categories for now
            }
        }

        // Also discover from current system state
        self.discover_from_system_state(&mut cache).await?;

        cache.last_discovery = Some(now);
        cache.discovery_stats.total_events_analyzed = events.len() as u64;
        cache.discovery_stats.unique_sources_found =
            cache.available_devices.len() as u64 + cache.available_sensors.len() as u64;
        cache.discovery_stats.last_activity = Some(now);

        let discovery_time = start_time.elapsed();
        info!(
            "Discovery completed: {} devices, {} sensors, {} rooms, {} metrics in {}ms",
            cache.available_devices.len(),
            cache.available_sensors.len(),
            cache.available_rooms.len(),
            cache.available_metrics.len(),
            discovery_time.as_millis()
        );

        Ok(())
    }

    /// Process device event for discovery
    async fn process_device_event(
        &self,
        state: &DeviceStateChange,
        timestamp: &DateTime<Utc>,
        cache: &mut DiscoveryCache,
    ) {
        // Add room if present
        if let Some(ref room) = state.room {
            cache.available_rooms.insert(room.clone());
        }

        // Update or add device info
        let device_info = cache
            .available_devices
            .entry(state.device_uuid.clone())
            .or_insert_with(|| DeviceInfo {
                name: state.device_name.clone(),
                device_type: state.device_type.clone(),
                room: state.room.clone(),
                last_seen: *timestamp,
                state_count: 0,
                is_active: false,
            });

        device_info.last_seen = device_info.last_seen.max(*timestamp);
        device_info.state_count += 1;

        // Determine if device is currently active based on latest state
        device_info.is_active = match state.new_state.as_str() {
            Some(s) => s == "on" || s == "true" || s == "1",
            None => state.new_state.as_bool().unwrap_or(false),
        };
    }

    /// Process sensor event for discovery
    async fn process_sensor_event(
        &self,
        data: &SensorData,
        timestamp: &DateTime<Utc>,
        cache: &mut DiscoveryCache,
    ) {
        // Add room if present
        if let Some(ref room) = data.room {
            cache.available_rooms.insert(room.clone());
        }

        // Update or add sensor info
        let sensor_info = cache
            .available_sensors
            .entry(data.sensor_uuid.clone())
            .or_insert_with(|| SensorInfo {
                name: data.sensor_name.clone(),
                sensor_type: data.sensor_type.clone(),
                unit: data.unit.clone(),
                room: data.room.clone(),
                last_reading: *timestamp,
                reading_count: 0,
                min_value: data.value,
                max_value: data.value,
                avg_value: data.value,
            });

        sensor_info.last_reading = sensor_info.last_reading.max(*timestamp);
        sensor_info.reading_count += 1;
        sensor_info.min_value = sensor_info.min_value.min(data.value);
        sensor_info.max_value = sensor_info.max_value.max(data.value);

        // Update running average
        let old_avg = sensor_info.avg_value;
        let count = sensor_info.reading_count as f64;
        sensor_info.avg_value = (old_avg * (count - 1.0) + data.value) / count;
    }

    /// Discover from current system state
    async fn discover_from_system_state(&self, cache: &mut DiscoveryCache) -> Result<()> {
        // Get current system state from client context
        let rooms = self.client_context.rooms.read().await;
        let devices = self.client_context.devices.read().await;

        // Add all known rooms
        for room_name in rooms.keys() {
            cache.available_rooms.insert(room_name.clone());
        }

        // Add all known devices (even if no recent activity)
        for (uuid, device) in devices.iter() {
            if !cache.available_devices.contains_key(uuid) {
                cache.available_devices.insert(
                    uuid.clone(),
                    DeviceInfo {
                        name: device.name.clone(),
                        device_type: device.device_type.clone(),
                        room: device.room.clone(),
                        last_seen: Utc::now(), // Use current time as fallback
                        state_count: 0,
                        is_active: false, // Will be determined by states
                    },
                );
            }
        }

        Ok(())
    }

    /// Generate dynamic dashboard layout
    pub async fn generate_dashboard_layout(
        &self,
        config: &DynamicDashboardConfig,
    ) -> Result<DynamicDashboardLayout> {
        // Ensure we have recent discovery data
        {
            let cache = self.discovery_cache.read().await;
            if cache.last_discovery.is_none()
                || cache.last_discovery.unwrap()
                    < Utc::now() - Duration::seconds(config.discovery_interval_seconds as i64)
            {
                drop(cache);
                self.discover_data_sources().await?;
            }
        }

        let cache = self.discovery_cache.read().await;
        let mut layout = DynamicDashboardLayout {
            generated_at: Utc::now(),
            widgets: Vec::new(),
            available_filters: self.generate_available_filters(&cache).await,
            discovery_info: DiscoveryInfo {
                last_discovery: cache.last_discovery,
                total_devices: cache.available_devices.len(),
                total_sensors: cache.available_sensors.len(),
                total_rooms: cache.available_rooms.len(),
                total_metrics: cache.available_metrics.len(),
            },
        };

        // Generate widgets based on discovered data

        // 1. Room overview widget (if we have rooms)
        if !cache.available_rooms.is_empty() {
            layout
                .widgets
                .push(self.create_rooms_overview_widget(&cache, config).await?);
        }

        // 2. Active devices widget
        if !cache.available_devices.is_empty() {
            layout
                .widgets
                .push(self.create_devices_widget(&cache, config).await?);
        }

        // 3. Sensor widgets (group by type)
        if !cache.available_sensors.is_empty() {
            layout
                .widgets
                .extend(self.create_sensor_widgets(&cache, config).await?);
        }

        // 4. System metrics widget
        if !cache.available_metrics.is_empty() {
            layout
                .widgets
                .push(self.create_metrics_widget(&cache, config).await?);
        }

        // 5. Activity timeline widget
        layout
            .widgets
            .push(self.create_activity_timeline_widget(config).await?);

        Ok(layout)
    }

    /// Create rooms overview widget
    async fn create_rooms_overview_widget(
        &self,
        cache: &DiscoveryCache,
        _config: &DynamicDashboardConfig,
    ) -> Result<DashboardWidget> {
        let mut room_stats = HashMap::new();

        // Count devices per room
        for device in cache.available_devices.values() {
            if let Some(ref room) = device.room {
                let stats = room_stats.entry(room.clone()).or_insert_with(|| RoomStats {
                    device_count: 0,
                    active_devices: 0,
                    sensor_count: 0,
                });
                stats.device_count += 1;
                if device.is_active {
                    stats.active_devices += 1;
                }
            }
        }

        // Count sensors per room
        for sensor in cache.available_sensors.values() {
            if let Some(ref room) = sensor.room {
                room_stats
                    .entry(room.clone())
                    .or_insert_with(|| RoomStats {
                        device_count: 0,
                        active_devices: 0,
                        sensor_count: 0,
                    })
                    .sensor_count += 1;
            }
        }

        Ok(DashboardWidget {
            id: "rooms_overview".to_string(),
            title: "Rooms Overview".to_string(),
            widget_type: "room_grid".to_string(),
            data: serde_json::to_value(room_stats)?,
            config: serde_json::json!({
                "show_activity": true,
                "show_device_count": true,
                "show_sensor_count": true
            }),
            size: WidgetSize::Medium,
            position: WidgetPosition { x: 0, y: 0 },
            refresh_interval: Some(30),
        })
    }

    /// Create devices widget
    async fn create_devices_widget(
        &self,
        cache: &DiscoveryCache,
        config: &DynamicDashboardConfig,
    ) -> Result<DashboardWidget> {
        // Filter to most active devices
        let mut active_devices: Vec<_> = cache
            .available_devices
            .values()
            .filter(|d| d.state_count >= config.min_events_threshold)
            .cloned()
            .collect();

        active_devices.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        active_devices.truncate(config.max_widgets_per_category);

        Ok(DashboardWidget {
            id: "active_devices".to_string(),
            title: format!("Active Devices ({})", active_devices.len()),
            widget_type: "device_list".to_string(),
            data: serde_json::to_value(active_devices)?,
            config: serde_json::json!({
                "show_last_activity": true,
                "show_room": true,
                "show_state": true
            }),
            size: WidgetSize::Large,
            position: WidgetPosition { x: 1, y: 0 },
            refresh_interval: Some(10),
        })
    }

    /// Create sensor widgets (grouped by type)
    async fn create_sensor_widgets(
        &self,
        cache: &DiscoveryCache,
        config: &DynamicDashboardConfig,
    ) -> Result<Vec<DashboardWidget>> {
        let mut widgets = Vec::new();

        // Group sensors by type
        let mut sensors_by_type: HashMap<String, Vec<&SensorInfo>> = HashMap::new();
        for sensor in cache.available_sensors.values() {
            if sensor.reading_count >= config.min_events_threshold {
                sensors_by_type
                    .entry(sensor.sensor_type.clone())
                    .or_default()
                    .push(sensor);
            }
        }

        let mut y_position = 1;

        for (sensor_type, sensors) in sensors_by_type {
            if sensors.is_empty() {
                continue;
            }

            let widget_type = match sensor_type.as_str() {
                "temperature" => "temperature_chart",
                "door_window" => "door_window_status",
                "motion" => "motion_activity",
                _ => "generic_sensor_chart",
            };

            widgets.push(DashboardWidget {
                id: format!("sensors_{}", sensor_type),
                title: format!(
                    "{} Sensors ({})",
                    sensor_type.replace('_', " ").to_title_case(),
                    sensors.len()
                ),
                widget_type: widget_type.to_string(),
                data: serde_json::to_value(sensors)?,
                config: serde_json::json!({
                    "sensor_type": sensor_type,
                    "show_trend": true,
                    "show_min_max": true
                }),
                size: WidgetSize::Medium,
                position: WidgetPosition {
                    x: 0,
                    y: y_position,
                },
                refresh_interval: Some(60),
            });

            y_position += 1;
        }

        Ok(widgets)
    }

    /// Create system metrics widget
    async fn create_metrics_widget(
        &self,
        cache: &DiscoveryCache,
        _config: &DynamicDashboardConfig,
    ) -> Result<DashboardWidget> {
        Ok(DashboardWidget {
            id: "system_metrics".to_string(),
            title: format!("System Metrics ({})", cache.available_metrics.len()),
            widget_type: "metrics_chart".to_string(),
            data: serde_json::to_value(&cache.available_metrics)?,
            config: serde_json::json!({
                "show_health_score": true,
                "show_error_rate": true
            }),
            size: WidgetSize::Medium,
            position: WidgetPosition { x: 1, y: 1 },
            refresh_interval: Some(30),
        })
    }

    /// Create activity timeline widget
    async fn create_activity_timeline_widget(
        &self,
        config: &DynamicDashboardConfig,
    ) -> Result<DashboardWidget> {
        let since = Utc::now() - Duration::hours(config.recent_window_hours as i64);
        let recent_events = self
            .history_store
            .query()
            .since(since)
            .limit(50)
            .execute()
            .await?
            .events;

        Ok(DashboardWidget {
            id: "activity_timeline".to_string(),
            title: "Recent Activity".to_string(),
            widget_type: "activity_timeline".to_string(),
            data: serde_json::to_value(recent_events)?,
            config: serde_json::json!({
                "show_categories": ["device_state", "sensor_reading", "audit_event"],
                "group_by_time": true
            }),
            size: WidgetSize::Large,
            position: WidgetPosition { x: 0, y: 2 },
            refresh_interval: Some(15),
        })
    }

    /// Generate available filters
    async fn generate_available_filters(&self, cache: &DiscoveryCache) -> Vec<DashboardFilter> {
        let mut filters = Vec::new();

        // Room filter
        if !cache.available_rooms.is_empty() {
            filters.push(DashboardFilter {
                name: "room".to_string(),
                label: "Room".to_string(),
                filter_type: "select".to_string(),
                options: cache.available_rooms.iter().cloned().collect(),
            });
        }

        // Device type filter
        let device_types: HashSet<String> = cache
            .available_devices
            .values()
            .map(|d| d.device_type.clone())
            .collect();
        if !device_types.is_empty() {
            filters.push(DashboardFilter {
                name: "device_type".to_string(),
                label: "Device Type".to_string(),
                filter_type: "select".to_string(),
                options: device_types.into_iter().collect(),
            });
        }

        // Sensor type filter
        let sensor_types: HashSet<String> = cache
            .available_sensors
            .values()
            .map(|s| s.sensor_type.clone())
            .collect();
        if !sensor_types.is_empty() {
            filters.push(DashboardFilter {
                name: "sensor_type".to_string(),
                label: "Sensor Type".to_string(),
                filter_type: "select".to_string(),
                options: sensor_types.into_iter().collect(),
            });
        }

        // Time range filter
        filters.push(DashboardFilter {
            name: "time_range".to_string(),
            label: "Time Range".to_string(),
            filter_type: "time_range".to_string(),
            options: vec![
                "1h".to_string(),
                "6h".to_string(),
                "24h".to_string(),
                "7d".to_string(),
            ],
        });

        filters
    }
}

// Data structures for dynamic dashboard

#[derive(Debug, Clone, Serialize)]
pub struct DynamicDashboardLayout {
    pub generated_at: DateTime<Utc>,
    pub widgets: Vec<DashboardWidget>,
    pub available_filters: Vec<DashboardFilter>,
    pub discovery_info: DiscoveryInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardWidget {
    pub id: String,
    pub title: String,
    pub widget_type: String,
    pub data: serde_json::Value,
    pub config: serde_json::Value,
    pub size: WidgetSize,
    pub position: WidgetPosition,
    pub refresh_interval: Option<u32>, // seconds
}

#[derive(Debug, Clone, Serialize)]
pub enum WidgetSize {
    Small,  // 1x1
    Medium, // 2x1
    Large,  // 2x2
    Wide,   // 3x1
}

#[derive(Debug, Clone, Serialize)]
pub struct WidgetPosition {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardFilter {
    pub name: String,
    pub label: String,
    pub filter_type: String, // "select", "time_range", "text"
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryInfo {
    pub last_discovery: Option<DateTime<Utc>>,
    pub total_devices: usize,
    pub total_sensors: usize,
    pub total_rooms: usize,
    pub total_metrics: usize,
}

#[derive(Debug, Serialize)]
struct RoomStats {
    device_count: u32,
    active_devices: u32,
    sensor_count: u32,
}

// Helper trait for string formatting
trait ToTitleCase {
    fn to_title_case(&self) -> String;
}

impl ToTitleCase for str {
    fn to_title_case(&self) -> String {
        self.split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}
