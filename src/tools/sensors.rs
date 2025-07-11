//! Sensor monitoring and discovery MCP tools
//!
//! Tools for sensor discovery, state monitoring, and sensor configuration.

// Re-export unified sensor functions with shorter names for compatibility
pub use crate::tools::sensors_unified::{
    discover_sensor_types_unified as discover_sensor_types,
    get_air_quality_sensors_unified as get_air_quality_sensors,
    // get_door_window_sensors_unified as get_all_door_window_sensors, // REMOVED
    get_energy_meters_unified as get_energy_meters,
    get_motion_sensors_unified as get_motion_sensors,
    get_presence_detectors_unified as get_presence_detectors,
    // get_temperature_sensors_unified as get_temperature_sensors, // REMOVED
    get_weather_station_sensors_unified as get_weather_station_sensors,
};

use crate::tools::{ToolContext, ToolResponse};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// State change event for sensor history tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChangeEvent {
    /// Sensor UUID
    pub uuid: String,

    /// Timestamp of the state change
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Previous state value
    pub old_value: serde_json::Value,

    /// New state value
    pub new_value: serde_json::Value,

    /// Human-readable state description
    pub human_readable: String,

    /// Event type (state_change, first_seen, etc.)
    pub event_type: String,
}

/// Complete state history for a sensor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorStateHistory {
    /// Sensor UUID
    pub uuid: String,

    /// Sensor name (if known)
    pub name: Option<String>,

    /// When this sensor was first seen
    pub first_seen: chrono::DateTime<chrono::Utc>,

    /// Last update timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,

    /// Total number of state changes
    pub total_changes: usize,

    /// Current state value
    pub current_state: serde_json::Value,

    /// Ring buffer of recent state change events
    pub state_events: VecDeque<StateChangeEvent>,

    /// Maximum events to keep in history
    pub max_events: usize,

    /// Sensor type classification
    pub sensor_type: Option<SensorType>,

    /// Associated room
    pub room: Option<String>,
}

impl SensorStateHistory {
    /// Create new sensor history
    pub fn new(uuid: String, name: Option<String>, max_events: usize) -> Self {
        Self {
            uuid,
            name,
            first_seen: chrono::Utc::now(),
            last_updated: chrono::Utc::now(),
            total_changes: 0,
            current_state: serde_json::Value::Null,
            state_events: VecDeque::with_capacity(max_events),
            max_events,
            sensor_type: None,
            room: None,
        }
    }

    /// Add a state change event
    pub fn add_state_change(&mut self, old_value: serde_json::Value, new_value: serde_json::Value) {
        let event = StateChangeEvent {
            uuid: self.uuid.clone(),
            timestamp: chrono::Utc::now(),
            old_value: old_value.clone(),
            new_value: new_value.clone(),
            human_readable: human_readable_state(&new_value, self.sensor_type.as_ref()),
            event_type: "state_change".to_string(),
        };

        // Add to ring buffer
        if self.state_events.len() >= self.max_events {
            self.state_events.pop_front();
        }
        self.state_events.push_back(event);

        // Update metadata
        self.current_state = new_value;
        self.last_updated = chrono::Utc::now();
        self.total_changes += 1;
    }

    /// Get recent changes within time window
    pub fn get_recent_changes(&self, hours: u32) -> Vec<&StateChangeEvent> {
        let threshold = chrono::Utc::now() - chrono::Duration::hours(hours as i64);
        self.state_events
            .iter()
            .filter(|event| event.timestamp > threshold)
            .collect()
    }

    /// Get activity summary for door/window sensors
    pub fn get_door_window_activity(&self, hours: u32) -> DoorWindowActivity {
        let recent_events = self.get_recent_changes(hours);
        let mut opens = 0;
        let mut closes = 0;
        let mut last_open_time: Option<chrono::DateTime<chrono::Utc>> = None;
        let mut last_close_time: Option<chrono::DateTime<chrono::Utc>> = None;

        for event in recent_events {
            if is_open_state(&event.new_value) {
                opens += 1;
                last_open_time = Some(event.timestamp);
            } else {
                closes += 1;
                last_close_time = Some(event.timestamp);
            }
        }

        DoorWindowActivity {
            opens,
            closes,
            last_open_time,
            last_close_time,
            current_state: human_readable_state(&self.current_state, self.sensor_type.as_ref()),
        }
    }
}

/// Door/window activity summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoorWindowActivity {
    pub opens: usize,
    pub closes: usize,
    pub last_open_time: Option<chrono::DateTime<chrono::Utc>>,
    pub last_close_time: Option<chrono::DateTime<chrono::Utc>>,
    pub current_state: String,
}

/// Sensor state logger for tracking historical data
#[derive(Debug)]
pub struct SensorStateLogger {
    /// Path to the log file
    log_file: PathBuf,

    /// Maximum events per sensor
    max_events_per_sensor: usize,

    /// Maximum number of sensors to track
    max_sensors: usize,

    /// Sync interval for writing to disk
    sync_interval: std::time::Duration,

    /// Sensor histories (UUID -> History)
    sensor_histories: Arc<RwLock<HashMap<String, SensorStateHistory>>>,

    /// Session start time
    session_start: chrono::DateTime<chrono::Utc>,

    /// Pending changes counter
    pending_changes: Arc<RwLock<usize>>,

    /// Last sync time
    last_sync: Arc<RwLock<chrono::DateTime<chrono::Utc>>>,
}

impl SensorStateLogger {
    /// Create new sensor state logger
    pub fn new(log_file: PathBuf) -> Self {
        Self {
            log_file,
            max_events_per_sensor: 100,
            max_sensors: 1000,
            sync_interval: std::time::Duration::from_secs(600), // 10 minutes
            sensor_histories: Arc::new(RwLock::new(HashMap::new())),
            session_start: chrono::Utc::now(),
            pending_changes: Arc::new(RwLock::new(0)),
            last_sync: Arc::new(RwLock::new(chrono::Utc::now())),
        }
    }

    /// Load existing history from disk
    pub async fn load_from_disk(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.log_file.exists() {
            info!("No existing sensor history file found, starting fresh");
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.log_file).await?;
        let loaded_histories: HashMap<String, SensorStateHistory> = serde_json::from_str(&content)?;

        let mut histories = self.sensor_histories.write().await;
        *histories = loaded_histories;

        let sensor_count = histories.len();
        let total_events: usize = histories.values().map(|h| h.state_events.len()).sum();

        info!(
            "Loaded sensor history: {} sensors, {} total events",
            sensor_count, total_events
        );
        Ok(())
    }

    /// Save current history to disk
    pub async fn save_to_disk(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let histories = self.sensor_histories.read().await;
        let content = serde_json::to_string_pretty(&*histories)?;

        // Write to temporary file first, then rename for atomic operation
        let temp_file = self.log_file.with_extension("tmp");
        tokio::fs::write(&temp_file, content).await?;
        tokio::fs::rename(&temp_file, &self.log_file).await?;

        // Update sync time and reset pending changes
        *self.last_sync.write().await = chrono::Utc::now();
        *self.pending_changes.write().await = 0;

        debug!("Sensor history saved to disk: {} sensors", histories.len());
        Ok(())
    }

    /// Log a state change
    pub async fn log_state_change(
        &self,
        uuid: String,
        old_value: serde_json::Value,
        new_value: serde_json::Value,
        sensor_name: Option<String>,
        sensor_type: Option<SensorType>,
        room: Option<String>,
    ) {
        // Skip if values are the same
        if old_value == new_value {
            return;
        }

        let mut histories = self.sensor_histories.write().await;

        // Enforce max sensors limit with LRU eviction
        if histories.len() >= self.max_sensors && !histories.contains_key(&uuid) {
            // Find least recently updated sensor and remove it
            let oldest_uuid = histories
                .iter()
                .min_by_key(|(_, history)| history.last_updated)
                .map(|(uuid, _)| uuid.clone());

            if let Some(oldest) = oldest_uuid {
                histories.remove(&oldest);
                warn!(
                    "Removed oldest sensor {} to make room for new sensor {}",
                    oldest, uuid
                );
            }
        }

        // Get or create sensor history
        let history = histories.entry(uuid.clone()).or_insert_with(|| {
            let mut new_history = SensorStateHistory::new(
                uuid.clone(),
                sensor_name.clone(),
                self.max_events_per_sensor,
            );
            new_history.sensor_type = sensor_type;
            new_history.room = room;
            new_history
        });

        // Update history
        history.add_state_change(old_value, new_value);

        // Update pending changes
        let mut pending = self.pending_changes.write().await;
        *pending += 1;

        debug!(
            "Logged state change for sensor {}: {} pending changes",
            uuid, *pending
        );
    }

    /// Get history for a specific sensor
    pub async fn get_sensor_history(&self, uuid: &str) -> Option<SensorStateHistory> {
        let histories = self.sensor_histories.read().await;
        histories.get(uuid).cloned()
    }

    /// Get recent changes across all sensors
    pub async fn get_recent_changes(&self, limit: usize) -> Vec<StateChangeEvent> {
        let histories = self.sensor_histories.read().await;
        let mut all_events = Vec::new();

        for history in histories.values() {
            all_events.extend(history.state_events.iter().cloned());
        }

        // Sort by timestamp descending (newest first)
        all_events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        all_events.truncate(limit);

        all_events
    }

    /// Get door/window activity summary
    pub async fn get_door_window_activity(&self, hours: u32) -> Vec<(String, DoorWindowActivity)> {
        let histories = self.sensor_histories.read().await;
        let mut activity = Vec::new();

        for (uuid, history) in histories.iter() {
            if matches!(history.sensor_type, Some(SensorType::DoorWindow)) {
                let sensor_activity = history.get_door_window_activity(hours);
                activity.push((uuid.clone(), sensor_activity));
            }
        }

        activity
    }

    /// Get logging statistics
    pub async fn get_logging_statistics(&self) -> LoggingStatistics {
        let histories = self.sensor_histories.read().await;
        let pending = *self.pending_changes.read().await;
        let last_sync = *self.last_sync.read().await;

        let total_sensors = histories.len();
        let total_events: usize = histories.values().map(|h| h.state_events.len()).sum();
        let total_changes: usize = histories.values().map(|h| h.total_changes).sum();

        // Calculate active sensors (updated in last hour)
        let threshold = chrono::Utc::now() - chrono::Duration::hours(1);
        let active_sensors = histories
            .values()
            .filter(|h| h.last_updated > threshold)
            .count();

        LoggingStatistics {
            session_start: self.session_start,
            total_sensors,
            active_sensors,
            total_events,
            total_changes,
            pending_changes: pending,
            last_sync_time: last_sync,
            uptime_hours: (chrono::Utc::now() - self.session_start).num_hours(),
        }
    }

    /// Start periodic sync task
    pub fn start_periodic_sync(&self) -> tokio::task::JoinHandle<()> {
        let logger = SensorStateLogger {
            log_file: self.log_file.clone(),
            max_events_per_sensor: self.max_events_per_sensor,
            max_sensors: self.max_sensors,
            sync_interval: self.sync_interval,
            sensor_histories: self.sensor_histories.clone(),
            session_start: self.session_start,
            pending_changes: self.pending_changes.clone(),
            last_sync: self.last_sync.clone(),
        };

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(logger.sync_interval);

            loop {
                interval.tick().await;

                let pending = *logger.pending_changes.read().await;
                if pending > 0 {
                    if let Err(e) = logger.save_to_disk().await {
                        error!("Failed to sync sensor history to disk: {}", e);
                    } else {
                        info!("Synced {} pending sensor changes to disk", pending);
                    }
                }
            }
        })
    }
}

/// Logging statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingStatistics {
    pub session_start: chrono::DateTime<chrono::Utc>,
    pub total_sensors: usize,
    pub active_sensors: usize,
    pub total_events: usize,
    pub total_changes: usize,
    pub pending_changes: usize,
    pub last_sync_time: chrono::DateTime<chrono::Utc>,
    pub uptime_hours: i64,
}

/// Convert sensor value to human-readable string
fn human_readable_state(value: &serde_json::Value, sensor_type: Option<&SensorType>) -> String {
    match (value, sensor_type) {
        (serde_json::Value::Number(n), Some(SensorType::DoorWindow)) => {
            if n.as_f64().unwrap_or(0.0) > 0.0 {
                "OPEN".to_string()
            } else {
                "CLOSED".to_string()
            }
        }
        (serde_json::Value::Bool(b), Some(SensorType::DoorWindow)) => {
            if *b {
                "OPEN".to_string()
            } else {
                "CLOSED".to_string()
            }
        }
        (serde_json::Value::Number(n), Some(SensorType::Motion)) => {
            if n.as_f64().unwrap_or(0.0) > 0.0 {
                "MOTION".to_string()
            } else {
                "NO_MOTION".to_string()
            }
        }
        (serde_json::Value::Bool(b), Some(SensorType::Motion)) => {
            if *b {
                "MOTION".to_string()
            } else {
                "NO_MOTION".to_string()
            }
        }
        _ => format!("{value}"),
    }
}

/// Check if a value represents an "open" state
fn is_open_state(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) > 0.0,
        serde_json::Value::Bool(b) => *b,
        _ => false,
    }
}

/// Discovered sensor information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredSensor {
    /// Sensor UUID
    pub uuid: String,

    /// Sensor name (if available)
    pub name: Option<String>,

    /// Current value
    pub current_value: serde_json::Value,

    /// Value history for pattern analysis
    pub value_history: Vec<serde_json::Value>,

    /// First discovery timestamp
    pub first_seen: chrono::DateTime<chrono::Utc>,

    /// Last update timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,

    /// Number of updates received
    pub update_count: usize,

    /// Detected sensor type
    pub sensor_type: SensorType,

    /// Detection confidence (0.0 - 1.0)
    pub confidence: f64,

    /// Pattern analysis score
    pub pattern_score: f64,

    /// Associated room (if detected)
    pub room: Option<String>,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Sensor type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SensorType {
    /// Door/window sensor (binary)
    DoorWindow,

    /// Motion sensor (binary)
    Motion,

    /// Analog sensor (continuous values)
    Analog,

    /// Temperature sensor
    Temperature,

    /// Light sensor
    Light,

    /// Noise/chatty sensor (frequent updates)
    Noisy,

    /// Humidity sensor
    Humidity,

    /// Air quality sensor (CO2, VOC, etc.)
    AirQuality,

    /// Energy meter sensor
    Energy,

    /// Unknown/unclassified
    Unknown,
}

/// Sensor statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorStatistics {
    /// Total discovered sensors
    pub total_sensors: usize,

    /// Sensors by type
    pub by_type: HashMap<String, usize>,

    /// Sensors by room
    pub by_room: HashMap<String, usize>,

    /// Active sensors (updated recently)
    pub active_count: usize,

    /// Binary sensors
    pub binary_count: usize,

    /// Analog sensors
    pub analog_count: usize,
}

/// Discover new sensors by monitoring WebSocket traffic
pub async fn discover_new_sensors(
    context: ToolContext,
    duration_seconds: Option<u64>,
) -> ToolResponse {
    let duration = std::time::Duration::from_secs(duration_seconds.unwrap_or(60));

    // Ensure WebSocket connection is available
    #[cfg(feature = "websocket")]
    {
        use crate::client::websocket_client::{
            EventFilter, LoxoneEventType, LoxoneWebSocketClient,
        };
        use std::collections::HashSet;

        // Check if the client supports WebSocket
        let ws_client = context
            .client
            .as_any()
            .downcast_ref::<LoxoneWebSocketClient>();

        if let Some(ws_client) = ws_client {
            info!(
                "Starting sensor discovery via WebSocket for {}s",
                duration.as_secs()
            );

            // Create filter for sensor events
            let mut event_types = HashSet::new();
            event_types.insert(LoxoneEventType::State);
            event_types.insert(LoxoneEventType::Sensor);

            let filter = EventFilter {
                event_types,
                min_interval: Some(std::time::Duration::from_millis(50)), // Faster for discovery
                ..Default::default()
            };

            // Subscribe to filtered events
            let mut event_receiver = ws_client.subscribe_with_filter(filter).await;

            // Track discovered sensors
            let mut discovered = HashMap::new();
            let discovery_start = chrono::Utc::now();
            let deadline = tokio::time::Instant::now() + duration;

            // Monitor events until deadline
            while tokio::time::Instant::now() < deadline {
                tokio::select! {
                    Some(update) = event_receiver.recv() => {
                        let sensor_entry = discovered.entry(update.uuid.clone())
                            .or_insert_with(|| DiscoveredSensor {
                                uuid: update.uuid.clone(),
                                name: update.device_name.clone(),
                                current_value: update.value.clone(),
                                value_history: Vec::new(),
                                first_seen: discovery_start,
                                last_updated: chrono::Utc::now(),
                                update_count: 0,
                                sensor_type: SensorType::Unknown,
                                confidence: 0.0,
                                pattern_score: 0.0,
                                room: update.room.clone(),
                                metadata: HashMap::new(),
                            });

                        // Update sensor data
                        sensor_entry.value_history.push(update.value.clone());
                        sensor_entry.current_value = update.value;
                        sensor_entry.last_updated = chrono::Utc::now();
                        sensor_entry.update_count += 1;

                        // Keep value history limited
                        if sensor_entry.value_history.len() > 100 {
                            sensor_entry.value_history.remove(0);
                        }

                        // Analyze pattern after sufficient samples
                        if sensor_entry.update_count >= 3 {
                            analyze_sensor_pattern(sensor_entry);
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                        // Check timeout periodically
                    }
                }
            }

            // Final analysis pass
            for sensor in discovered.values_mut() {
                if sensor.sensor_type == SensorType::Unknown {
                    analyze_sensor_pattern(sensor);
                }
            }

            let discovered_sensors: Vec<DiscoveredSensor> = discovered.into_values().collect();
            let stats = calculate_sensor_statistics(&discovered_sensors);

            let response_data = serde_json::json!({
                "discovery_duration": format!("{}s", duration.as_secs()),
                "discovered_sensors": discovered_sensors,
                "statistics": stats,
                "discovery_complete": true,
                "timestamp": chrono::Utc::now()
            });

            return ToolResponse::success_with_message(
                response_data,
                format!(
                    "Discovered {} sensors in {}s",
                    discovered_sensors.len(),
                    duration.as_secs()
                ),
            );
        }
    }

    // Fallback: Discover sensors from structure (not real-time)
    info!("WebSocket not available, discovering sensors from structure");

    let devices = match context.context.get_devices_by_category("sensors").await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(format!("Failed to get sensor devices: {e}")),
    };

    let mut discovered_sensors = Vec::new();
    for device in devices {
        let sensor_type = classify_sensor_type(&device);

        let sensor = DiscoveredSensor {
            uuid: device.uuid.clone(),
            name: Some(device.name.clone()),
            current_value: device
                .states
                .get("value")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            value_history: Vec::new(),
            first_seen: chrono::Utc::now(),
            last_updated: chrono::Utc::now(),
            update_count: 1,
            sensor_type,
            confidence: 0.7, // Lower confidence without real-time data
            pattern_score: 0.5,
            room: device.room.clone(),
            metadata: device.states.clone(),
        };

        discovered_sensors.push(sensor);
    }

    let stats = calculate_sensor_statistics(&discovered_sensors);

    let response_data = serde_json::json!({
        "discovery_method": "structure_analysis",
        "discovered_sensors": discovered_sensors,
        "statistics": stats,
        "discovery_complete": true,
        "timestamp": chrono::Utc::now(),
        "note": "Real-time discovery requires WebSocket connection"
    });

    ToolResponse::success_with_message(
        response_data,
        format!(
            "Found {} sensors from structure analysis",
            discovered_sensors.len()
        ),
    )
}

/// List all discovered sensors
pub async fn list_discovered_sensors(
    context: ToolContext,
    sensor_type: Option<String>,
    room: Option<String>,
) -> ToolResponse {
    // This would normally load from the sensor discovery cache
    // For now, return sensor devices from the structure

    let devices = match context.context.get_devices_by_category("sensors").await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    let mut discovered_sensors = Vec::new();

    for device in devices {
        // Convert device to discovered sensor format
        let detected_type = classify_sensor_type(&device);

        // Apply filters
        if let Some(ref filter_type) = sensor_type {
            let type_match = match detected_type {
                SensorType::DoorWindow => filter_type == "door_window",
                SensorType::Motion => filter_type == "motion",
                SensorType::Analog => filter_type == "analog",
                SensorType::Temperature => filter_type == "temperature",
                SensorType::Light => filter_type == "light",
                SensorType::Noisy => filter_type == "noisy",
                SensorType::Humidity => filter_type == "humidity",
                SensorType::AirQuality => filter_type == "air_quality",
                SensorType::Energy => filter_type == "energy",
                SensorType::Unknown => filter_type == "unknown",
            };
            if !type_match {
                continue;
            }
        }

        if let Some(ref filter_room) = room {
            if device.room.as_ref() != Some(filter_room) {
                continue;
            }
        }

        let sensor = DiscoveredSensor {
            uuid: device.uuid.clone(),
            name: Some(device.name.clone()),
            current_value: device
                .states
                .get("value")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            value_history: Vec::new(), // Would be populated from monitoring
            first_seen: chrono::Utc::now() - chrono::Duration::hours(1), // Placeholder
            last_updated: chrono::Utc::now(),
            update_count: 1,
            sensor_type: detected_type,
            confidence: 0.8,    // Placeholder confidence
            pattern_score: 0.7, // Placeholder score
            room: device.room.clone(),
            metadata: device.states.clone(),
        };

        discovered_sensors.push(sensor);
    }

    let stats = calculate_sensor_statistics(&discovered_sensors);

    let response_data = serde_json::json!({
        "sensors": discovered_sensors,
        "statistics": stats,
        "filters": {
            "sensor_type": sensor_type,
            "room": room
        },
        "timestamp": chrono::Utc::now()
    });

    let message = match (sensor_type.as_deref(), room.as_deref()) {
        (Some(stype), Some(room)) => {
            format!(
                "Found {} {} sensors in room '{}'",
                discovered_sensors.len(),
                stype,
                room
            )
        }
        (Some(stype), None) => {
            format!("Found {} {stype} sensors", discovered_sensors.len())
        }
        (None, Some(room)) => {
            format!(
                "Found {} sensors in room '{}'",
                discovered_sensors.len(),
                room
            )
        }
        (None, None) => {
            format!("Found {} sensors", discovered_sensors.len())
        }
    };

    ToolResponse::success_with_message(response_data, message)
}

/// Get detailed sensor information
pub async fn get_sensor_details(context: ToolContext, sensor_id: String) -> ToolResponse {
    // Find the sensor
    let device = match context.find_device(&sensor_id).await {
        Ok(device) => device,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    // Check if it's a sensor
    if device.category != "sensors" {
        return ToolResponse::error(format!("Device '{sensor_id}' is not a sensor"));
    }

    let sensor_type = classify_sensor_type(&device);

    // Get current state
    let current_state = device
        .states
        .get("value")
        .or_else(|| device.states.get("active"))
        .or_else(|| device.states.get("state"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let sensor_details = serde_json::json!({
        "uuid": device.uuid,
        "name": device.name,
        "device_type": device.device_type,
        "room": device.room,
        "category": device.category,
        "sensor_type": sensor_type,
        "current_state": current_state,
        "all_states": device.states,
        "sub_controls": device.sub_controls,
        "capabilities": analyze_sensor_capabilities(&device),
        "timestamp": chrono::Utc::now()
    });

    ToolResponse::success_with_message(
        sensor_details,
        format!("Sensor details for '{}'", device.name),
    )
}

/// Get sensor categories overview
pub async fn get_sensor_categories(context: ToolContext) -> ToolResponse {
    let devices = match context.context.get_devices_by_category("sensors").await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    if devices.is_empty() {
        return ToolResponse::error("No sensors found in the system".to_string());
    }

    // Categorize sensors
    let mut categories = HashMap::new();
    let mut type_distribution = HashMap::new();
    let mut room_distribution = HashMap::new();

    for device in &devices {
        let sensor_type = classify_sensor_type(device);
        let type_name = format!("{sensor_type:?}").to_lowercase();

        // Update type distribution
        *type_distribution.entry(type_name.clone()).or_insert(0) += 1;

        // Update room distribution
        if let Some(ref room) = device.room {
            *room_distribution.entry(room.clone()).or_insert(0) += 1;
        }

        // Group sensors by detected type
        let category_sensors = categories.entry(type_name).or_insert_with(Vec::new);
        category_sensors.push(serde_json::json!({
            "uuid": device.uuid,
            "name": device.name,
            "room": device.room,
            "device_type": device.device_type
        }));
    }

    let response_data = serde_json::json!({
        "categories": categories,
        "type_distribution": type_distribution,
        "room_distribution": room_distribution,
        "total_sensors": devices.len(),
        "total_types": type_distribution.len(),
        "total_rooms": room_distribution.len(),
        "timestamp": chrono::Utc::now()
    });

    ToolResponse::success_with_message(
        response_data,
        format!(
            "Sensor categories: {} sensors across {} types in {} rooms",
            devices.len(),
            type_distribution.len(),
            room_distribution.len()
        ),
    )
}

/// Classify sensor type based on device information
fn classify_sensor_type(device: &crate::client::LoxoneDevice) -> SensorType {
    let name_lower = device.name.to_lowercase();
    let type_lower = device.device_type.to_lowercase();

    // Check for door/window sensors
    if name_lower.contains("door")
        || name_lower.contains("window")
        || name_lower.contains("fenster")
        || name_lower.contains("tür")
        || type_lower.contains("door")
        || type_lower.contains("window")
    {
        return SensorType::DoorWindow;
    }

    // Check for motion sensors
    if name_lower.contains("motion")
        || name_lower.contains("bewegung")
        || name_lower.contains("pir")
        || type_lower.contains("motion")
    {
        return SensorType::Motion;
    }

    // Check for temperature sensors
    if name_lower.contains("temperature")
        || name_lower.contains("temp")
        || name_lower.contains("thermometer")
        || type_lower.contains("temperature")
    {
        return SensorType::Temperature;
    }

    // Check for light sensors
    if name_lower.contains("light")
        || name_lower.contains("lux")
        || name_lower.contains("brightness")
        || type_lower.contains("light")
    {
        return SensorType::Light;
    }

    // Check if it's analog based on device type
    if type_lower.contains("analog") || type_lower.contains("sensor") {
        return SensorType::Analog;
    }

    // Default to unknown
    SensorType::Unknown
}

/// Analyze sensor capabilities
fn analyze_sensor_capabilities(device: &crate::client::LoxoneDevice) -> serde_json::Value {
    let mut capabilities = serde_json::Map::new();

    // Check for binary state
    let has_binary_state =
        device.states.contains_key("active") || device.states.contains_key("state");
    capabilities.insert(
        "binary_state".to_string(),
        serde_json::Value::Bool(has_binary_state),
    );

    // Check for analog value
    let has_analog_value =
        device.states.contains_key("value") || device.states.contains_key("analog");
    capabilities.insert(
        "analog_value".to_string(),
        serde_json::Value::Bool(has_analog_value),
    );

    // Check for temperature
    let has_temperature =
        device.states.contains_key("temperature") || device.states.contains_key("temp");
    capabilities.insert(
        "temperature".to_string(),
        serde_json::Value::Bool(has_temperature),
    );

    // State count
    capabilities.insert(
        "state_count".to_string(),
        serde_json::Value::Number(serde_json::Number::from(device.states.len())),
    );

    serde_json::Value::Object(capabilities)
}

/// Analyze sensor pattern to determine type and characteristics
fn analyze_sensor_pattern(sensor: &mut DiscoveredSensor) {
    if sensor.value_history.is_empty() {
        return;
    }

    // Analyze value patterns
    let mut distinct_values = HashSet::new();
    let mut numeric_values = Vec::new();
    let mut binary_pattern = true;

    for value in &sensor.value_history {
        distinct_values.insert(value.to_string());

        match value {
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    numeric_values.push(f);
                    // Check if it's binary (0/1 or close to it)
                    if f != 0.0 && f != 1.0 && !(-0.1..=1.1).contains(&f) {
                        binary_pattern = false;
                    }
                }
            }
            serde_json::Value::Bool(_) => {
                // Boolean values are binary by nature
            }
            _ => {
                binary_pattern = false;
            }
        }
    }

    // Calculate confidence based on sample count
    sensor.confidence = (sensor.update_count as f64 / 10.0).min(1.0);

    // Determine sensor type based on patterns
    if binary_pattern && distinct_values.len() <= 2 {
        // Binary sensor detected
        sensor.pattern_score = 0.9;

        // Try to determine specific binary type based on name
        if let Some(ref name) = sensor.name {
            let name_lower = name.to_lowercase();
            if name_lower.contains("door")
                || name_lower.contains("window")
                || name_lower.contains("fenster")
                || name_lower.contains("tür")
            {
                sensor.sensor_type = SensorType::DoorWindow;
            } else if name_lower.contains("motion")
                || name_lower.contains("bewegung")
                || name_lower.contains("pir")
            {
                sensor.sensor_type = SensorType::Motion;
            } else {
                sensor.sensor_type = SensorType::DoorWindow; // Default binary to door/window
            }
        } else {
            sensor.sensor_type = SensorType::DoorWindow; // Default binary
        }
    } else if !numeric_values.is_empty() {
        // Analog sensor detected
        let avg = numeric_values.iter().sum::<f64>() / numeric_values.len() as f64;
        let variance = numeric_values
            .iter()
            .map(|v| (v - avg).powi(2))
            .sum::<f64>()
            / numeric_values.len() as f64;

        sensor.pattern_score = 1.0 - (variance / 1000.0).min(1.0); // Lower variance = higher score

        // Determine analog type based on value range and name
        if let Some(ref name) = sensor.name {
            let name_lower = name.to_lowercase();
            if name_lower.contains("temp") || name_lower.contains("thermometer") {
                sensor.sensor_type = SensorType::Temperature;
            } else if name_lower.contains("light")
                || name_lower.contains("lux")
                || name_lower.contains("brightness")
            {
                sensor.sensor_type = SensorType::Light;
            } else {
                sensor.sensor_type = SensorType::Analog;
            }
        } else {
            // Check value range for temperature
            if numeric_values.iter().all(|&v| (-50.0..=100.0).contains(&v))
                && numeric_values.iter().any(|&v| (10.0..=40.0).contains(&v))
            {
                sensor.sensor_type = SensorType::Temperature;
            } else {
                sensor.sensor_type = SensorType::Analog;
            }
        }
    } else {
        // Unknown or noisy sensor
        sensor.sensor_type = SensorType::Unknown;
        sensor.pattern_score = 0.3;
    }

    // Check for noisy sensors (too many updates)
    let update_rate = sensor.update_count as f64
        / (chrono::Utc::now() - sensor.first_seen)
            .num_seconds()
            .max(1) as f64;
    if update_rate > 1.0 {
        // More than 1 update per second
        sensor.sensor_type = SensorType::Noisy;
        sensor.pattern_score *= 0.5; // Reduce confidence for noisy sensors
    }
}

/// Calculate sensor statistics
fn calculate_sensor_statistics(sensors: &[DiscoveredSensor]) -> SensorStatistics {
    let mut by_type = HashMap::new();
    let mut by_room = HashMap::new();
    let mut binary_count = 0;
    let mut analog_count = 0;
    let mut active_count = 0;

    let now = chrono::Utc::now();
    let recent_threshold = now - chrono::Duration::minutes(10);

    for sensor in sensors {
        // Count by type
        let type_name = format!("{:?}", sensor.sensor_type).to_lowercase();
        *by_type.entry(type_name).or_insert(0) += 1;

        // Count by room
        if let Some(ref room) = sensor.room {
            *by_room.entry(room.clone()).or_insert(0) += 1;
        }

        // Count binary vs analog
        match sensor.sensor_type {
            SensorType::DoorWindow | SensorType::Motion => binary_count += 1,
            SensorType::Analog | SensorType::Temperature | SensorType::Light => analog_count += 1,
            _ => {}
        }

        // Count active sensors
        if sensor.last_updated > recent_threshold {
            active_count += 1;
        }
    }

    SensorStatistics {
        total_sensors: sensors.len(),
        by_type,
        by_room,
        active_count,
        binary_count,
        analog_count,
    }
}

// === MCP Tools for Sensor History ===

/// Get complete state history for a specific sensor
pub async fn get_sensor_state_history(
    _context: ToolContext,
    uuid: String,
    logger: Option<Arc<SensorStateLogger>>,
) -> ToolResponse {
    let logger = match logger {
        Some(logger) => logger,
        None => return ToolResponse::error("Sensor state logger not available".to_string()),
    };

    match logger.get_sensor_history(&uuid).await {
        Some(history) => {
            let response_data = serde_json::json!({
                "uuid": history.uuid,
                "name": history.name,
                "first_seen": history.first_seen,
                "last_updated": history.last_updated,
                "total_changes": history.total_changes,
                "current_state": history.current_state,
                "sensor_type": history.sensor_type,
                "room": history.room,
                "events": history.state_events,
                "event_count": history.state_events.len(),
                "max_events": history.max_events,
                "timestamp": chrono::Utc::now()
            });

            ToolResponse::success_with_message(
                response_data,
                format!(
                    "History for sensor '{}': {} events, {} total changes",
                    history.name.as_deref().unwrap_or(&uuid),
                    history.state_events.len(),
                    history.total_changes
                ),
            )
        }
        None => ToolResponse::error(format!("No history found for sensor '{uuid}'")),
    }
}

/// Get recent sensor changes across all sensors
pub async fn get_recent_sensor_changes(
    _context: ToolContext,
    limit: Option<usize>,
    logger: Option<Arc<SensorStateLogger>>,
) -> ToolResponse {
    let logger = match logger {
        Some(logger) => logger,
        None => return ToolResponse::error("Sensor state logger not available".to_string()),
    };

    let limit = limit.unwrap_or(50);
    let recent_changes = logger.get_recent_changes(limit).await;

    let response_data = serde_json::json!({
        "recent_changes": recent_changes,
        "count": recent_changes.len(),
        "limit": limit,
        "timestamp": chrono::Utc::now()
    });

    ToolResponse::success_with_message(
        response_data,
        format!("Retrieved {} recent sensor changes", recent_changes.len()),
    )
}

/// Get door/window activity summary
pub async fn get_door_window_activity(
    _context: ToolContext,
    hours: Option<u32>,
    logger: Option<Arc<SensorStateLogger>>,
) -> ToolResponse {
    let logger = match logger {
        Some(logger) => logger,
        None => return ToolResponse::error("Sensor state logger not available".to_string()),
    };

    let hours = hours.unwrap_or(24);
    let activity = logger.get_door_window_activity(hours).await;

    // Calculate totals
    let total_opens: usize = activity.iter().map(|(_, a)| a.opens).sum();
    let total_closes: usize = activity.iter().map(|(_, a)| a.closes).sum();
    let sensors_with_activity = activity
        .iter()
        .filter(|(_, a)| a.opens > 0 || a.closes > 0)
        .count();

    // Group by sensor name for better readability
    let activities: Vec<_> = activity
        .into_iter()
        .map(|(uuid, activity)| {
            serde_json::json!({
                "uuid": uuid,
                "opens": activity.opens,
                "closes": activity.closes,
                "last_open_time": activity.last_open_time,
                "last_close_time": activity.last_close_time,
                "current_state": activity.current_state
            })
        })
        .collect();

    let response_data = serde_json::json!({
        "time_window_hours": hours,
        "door_window_activities": activities,
        "summary": {
            "total_sensors": activities.len(),
            "sensors_with_activity": sensors_with_activity,
            "total_opens": total_opens,
            "total_closes": total_closes,
            "total_events": total_opens + total_closes
        },
        "timestamp": chrono::Utc::now()
    });

    ToolResponse::success_with_message(
        response_data,
        format!(
            "Door/window activity ({}h): {} opens, {} closes across {} sensors",
            hours,
            total_opens,
            total_closes,
            activities.len()
        ),
    )
}

/// Get logging system statistics
pub async fn get_logging_statistics(
    _context: ToolContext,
    logger: Option<Arc<SensorStateLogger>>,
) -> ToolResponse {
    let logger = match logger {
        Some(logger) => logger,
        None => return ToolResponse::error("Sensor state logger not available".to_string()),
    };

    let stats = logger.get_logging_statistics().await;

    let response_data = serde_json::json!({
        "logging_statistics": stats,
        "timestamp": chrono::Utc::now()
    });

    ToolResponse::success_with_message(
        response_data,
        format!(
            "Logging stats: {} sensors, {} events, {}h uptime",
            stats.total_sensors, stats.total_events, stats.uptime_hours
        ),
    )
}
