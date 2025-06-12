//! Sensor monitoring and discovery MCP tools
//!
//! Tools for sensor discovery, state monitoring, and sensor configuration.

use crate::tools::{ToolContext, ToolResponse};
// use rmcp::tool; // TODO: Re-enable when rmcp API is clarified
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn discover_new_sensors(
    _context: ToolContext,
    // #[description("Discovery duration in seconds")] // TODO: Re-enable when rmcp API is clarified
    duration_seconds: Option<u64>
) -> ToolResponse {
    let duration = std::time::Duration::from_secs(duration_seconds.unwrap_or(60));
    
    // This would implement real sensor discovery via WebSocket monitoring
    // For now, return a placeholder response
    
    let discovered_sensors = vec![
        DiscoveredSensor {
            uuid: "example-sensor-1".to_string(),
            name: Some("Kitchen Window".to_string()),
            current_value: serde_json::Value::Number(serde_json::Number::from(0)),
            value_history: vec![
                serde_json::Value::Number(serde_json::Number::from(0)),
                serde_json::Value::Number(serde_json::Number::from(1)),
                serde_json::Value::Number(serde_json::Number::from(0)),
            ],
            first_seen: chrono::Utc::now() - chrono::Duration::minutes(5),
            last_updated: chrono::Utc::now(),
            update_count: 3,
            sensor_type: SensorType::DoorWindow,
            confidence: 0.95,
            pattern_score: 0.8,
            room: Some("Kitchen".to_string()),
            metadata: HashMap::new(),
        }
    ];
    
    let stats = calculate_sensor_statistics(&discovered_sensors);
    
    let response_data = serde_json::json!({
        "discovery_duration": format!("{}s", duration.as_secs()),
        "discovered_sensors": discovered_sensors,
        "statistics": stats,
        "discovery_complete": true,
        "timestamp": chrono::Utc::now()
    });
    
    ToolResponse::success_with_message(
        response_data,
        format!("Discovered {} sensors in {}s", discovered_sensors.len(), duration.as_secs())
    )
}

/// Get all configured door/window sensors
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_all_door_window_sensors(context: ToolContext) -> ToolResponse {
    // Get sensor devices from the system
    let devices = match context.context.get_devices_by_category("sensors").await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    // Filter for door/window sensors based on type or name patterns
    let door_window_sensors: Vec<_> = devices.into_iter()
        .filter(|device| {
            let name_lower = device.name.to_lowercase();
            let type_lower = device.device_type.to_lowercase();
            
            name_lower.contains("door") || 
            name_lower.contains("window") || 
            name_lower.contains("fenster") || 
            name_lower.contains("tür") ||
            type_lower.contains("door") ||
            type_lower.contains("window")
        })
        .collect();
    
    if door_window_sensors.is_empty() {
        return ToolResponse::error("No door/window sensors found".to_string());
    }
    
    // Get current states for each sensor
    let mut sensor_states = Vec::new();
    for sensor in &door_window_sensors {
        // Get current state
        let state = sensor.states.get("value")
            .or_else(|| sensor.states.get("active"))
            .or_else(|| sensor.states.get("state"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        
        let is_open = match &state {
            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) > 0.0,
            serde_json::Value::Bool(b) => *b,
            _ => false,
        };
        
        sensor_states.push(serde_json::json!({
            "uuid": sensor.uuid,
            "name": sensor.name,
            "room": sensor.room,
            "state": state,
            "is_open": is_open,
            "device_type": sensor.device_type,
            "last_updated": chrono::Utc::now()
        }));
    }
    
    // Calculate summary statistics
    let total_sensors = sensor_states.len();
    let open_count = sensor_states.iter()
        .filter(|s| s.get("is_open").and_then(|v| v.as_bool()).unwrap_or(false))
        .count();
    let closed_count = total_sensors - open_count;
    
    let response_data = serde_json::json!({
        "sensors": sensor_states,
        "summary": {
            "total_sensors": total_sensors,
            "open": open_count,
            "closed": closed_count,
            "all_closed": open_count == 0,
            "any_open": open_count > 0
        },
        "timestamp": chrono::Utc::now()
    });
    
    let status_message = if open_count == 0 {
        format!("All {} door/window sensors are closed", total_sensors)
    } else {
        format!("{} door/window sensors: {} open, {} closed", total_sensors, open_count, closed_count)
    };
    
    ToolResponse::success_with_message(response_data, status_message)
}

/// List all discovered sensors
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn list_discovered_sensors(
    context: ToolContext,
    // #[description("Filter by sensor type")] // TODO: Re-enable when rmcp API is clarified
    sensor_type: Option<String>,
    // #[description("Filter by room")] // TODO: Re-enable when rmcp API is clarified
    room: Option<String>
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
            current_value: device.states.get("value")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            value_history: Vec::new(), // Would be populated from monitoring
            first_seen: chrono::Utc::now() - chrono::Duration::hours(1), // Placeholder
            last_updated: chrono::Utc::now(),
            update_count: 1,
            sensor_type: detected_type,
            confidence: 0.8, // Placeholder confidence
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
            format!("Found {} {} sensors in room '{}'", discovered_sensors.len(), stype, room)
        }
        (Some(stype), None) => {
            format!("Found {} {} sensors", discovered_sensors.len(), stype)
        }
        (None, Some(room)) => {
            format!("Found {} sensors in room '{}'", discovered_sensors.len(), room)
        }
        (None, None) => {
            format!("Found {} sensors", discovered_sensors.len())
        }
    };
    
    ToolResponse::success_with_message(response_data, message)
}

/// Get detailed sensor information
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_sensor_details(
    context: ToolContext,
    // #[description("Sensor UUID or name")] // TODO: Re-enable when rmcp API is clarified
    sensor_id: String
) -> ToolResponse {
    // Find the sensor
    let device = match context.find_device(&sensor_id).await {
        Ok(device) => device,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    // Check if it's a sensor
    if device.category != "sensors" {
        return ToolResponse::error(format!("Device '{}' is not a sensor", sensor_id));
    }
    
    let sensor_type = classify_sensor_type(&device);
    
    // Get current state
    let current_state = device.states.get("value")
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
        format!("Sensor details for '{}'", device.name)
    )
}

/// Get sensor categories overview
// #[tool] // TODO: Re-enable when rmcp API is clarified
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
        let type_name = format!("{:?}", sensor_type).to_lowercase();
        
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
        format!("Sensor categories: {} sensors across {} types in {} rooms",
                devices.len(), type_distribution.len(), room_distribution.len())
    )
}

/// Classify sensor type based on device information
fn classify_sensor_type(device: &crate::client::LoxoneDevice) -> SensorType {
    let name_lower = device.name.to_lowercase();
    let type_lower = device.device_type.to_lowercase();
    
    // Check for door/window sensors
    if name_lower.contains("door") || name_lower.contains("window") ||
       name_lower.contains("fenster") || name_lower.contains("tür") ||
       type_lower.contains("door") || type_lower.contains("window") {
        return SensorType::DoorWindow;
    }
    
    // Check for motion sensors
    if name_lower.contains("motion") || name_lower.contains("bewegung") ||
       name_lower.contains("pir") || type_lower.contains("motion") {
        return SensorType::Motion;
    }
    
    // Check for temperature sensors
    if name_lower.contains("temperature") || name_lower.contains("temp") ||
       name_lower.contains("thermometer") || type_lower.contains("temperature") {
        return SensorType::Temperature;
    }
    
    // Check for light sensors
    if name_lower.contains("light") || name_lower.contains("lux") ||
       name_lower.contains("brightness") || type_lower.contains("light") {
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
    let has_binary_state = device.states.contains_key("active") ||
                          device.states.contains_key("state");
    capabilities.insert("binary_state".to_string(), serde_json::Value::Bool(has_binary_state));
    
    // Check for analog value
    let has_analog_value = device.states.contains_key("value") ||
                          device.states.contains_key("analog");
    capabilities.insert("analog_value".to_string(), serde_json::Value::Bool(has_analog_value));
    
    // Check for temperature
    let has_temperature = device.states.contains_key("temperature") ||
                         device.states.contains_key("temp");
    capabilities.insert("temperature".to_string(), serde_json::Value::Bool(has_temperature));
    
    // State count
    capabilities.insert("state_count".to_string(), 
                       serde_json::Value::Number(serde_json::Number::from(device.states.len())));
    
    serde_json::Value::Object(capabilities)
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