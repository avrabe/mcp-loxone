//! Dashboard data helper for HTTP transport

use crate::client::ClientContext;
use crate::server::LoxoneMcpServer;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// Get complete dashboard data from server context (legacy)
pub async fn get_dashboard_data_from_context(context: &Arc<ClientContext>) -> Value {
    get_dashboard_data_from_context_legacy(context).await
}

/// Get complete dashboard data with real-time states from MCP server
pub async fn get_dashboard_data_from_server(server: &LoxoneMcpServer) -> Value {
    let context = &server.context;

    // Get connection status
    let connection_status = if *context.connected.read().await {
        "Connected"
    } else {
        "Disconnected"
    };

    // Connection status logging removed

    // Get data from context
    let rooms = context.rooms.read().await;
    let devices = context.devices.read().await;
    let _capabilities = context.capabilities.read().await;

    // Room/device count logging removed

    // Get all device UUIDs for fetching real-time states
    let all_device_uuids: Vec<String> = devices.keys().cloned().collect();

    // Fetch real-time device states
    let device_states = server
        .client
        .get_device_states(&all_device_uuids)
        .await
        .unwrap_or_default();

    // Log device state fetch results for debugging
    if device_states.is_empty() {
        tracing::warn!(
            "No device states returned from get_device_states for {} devices",
            all_device_uuids.len()
        );
    } else {
        tracing::debug!(
            "Fetched {} device states from {} requested",
            device_states.len(),
            all_device_uuids.len()
        );
    }

    // Fetch MCP sensor data to supplement device information
    let sensor_data = fetch_mcp_sensor_data(server).await;
    tracing::info!(
        "Dashboard: Fetched {} MCP sensors, {} structure devices",
        sensor_data.len(),
        devices.len()
    );

    // Build rooms data with device counts and real-time status
    let mut rooms_data = Vec::new();
    for (_uuid, room) in rooms.iter() {
        // Count devices in this room
        let room_devices: Vec<_> = devices
            .values()
            .filter(|device| device.room.as_ref() == Some(&room.name))
            .collect();

        // Count active devices using real-time states
        let active_count = room_devices
            .iter()
            .filter(|device| {
                device_states
                    .get(&device.uuid)
                    .and_then(|v| v.as_f64())
                    .map(|v| v > 0.0)
                    .unwrap_or(false)
            })
            .count();

        rooms_data.push(json!({
            "name": room.name,
            "uuid": room.uuid,
            "device_count": room_devices.len(),
            "active_devices": active_count,
            "current_temp": null, // TODO: Extract from sensors
            "current_humidity": null, // TODO: Extract from sensors
            "sensors": 0, // TODO: Count sensors
        }));
    }

    // Build devices data by category with real-time states
    let mut lights_data = Vec::new();
    let mut blinds_data = Vec::new();
    let mut climate_data = Vec::new();
    let mut other_data = Vec::new();

    // First, add actual sensors from MCP data as devices
    for (sensor_uuid, (sensor_value, sensor_name)) in &sensor_data {
        // Determine which room this sensor belongs to by name matching
        let sensor_room = determine_sensor_room(sensor_name, &rooms);

        let device_json = json!({
            "uuid": sensor_uuid,
            "name": sensor_name,
            "device_type": "Sensor",
            "type": "Sensor",
            "room": sensor_room,
            "status": "Active",
            "status_color": "green",
            "state_display": format!("{:.1}", sensor_value),
            "states": {
                "active": sensor_value,
                "value": sensor_value
            },
            "raw_state": json!(sensor_value),
            "cached_states": json!({}),
        });

        // Categorize sensors as climate-related
        climate_data.push(device_json);
    }

    // Then process existing devices from structure
    for device in devices.values() {
        // Get real-time state for this device
        let device_state = device_states.get(&device.uuid);
        let state_value = device_state.and_then(|v| v.as_f64()).unwrap_or(0.0);

        // Try to get state from cached device states as fallback
        let cached_active = device
            .states
            .get("active")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        // For sensor devices, try to extract meaningful values from device states
        let mut meaningful_value = 0.0;
        let mut has_meaningful_value = false;

        // Check if this is a sensor device and try to extract meaningful values
        if device.device_type.to_lowercase().contains("analog")
            || device.name.to_lowercase().contains("temperatur")
            || device.name.to_lowercase().contains("luftfeuchte")
            || device.name.to_lowercase().contains("helligkeit")
        {
            // For sensors, the real-time device state is the most reliable source
            // Extract from the fetched state data if it contains meaningful values
            if let Some(state_data) = device_state {
                // Check if it's a nested LL object with a value field
                if let Some(ll_obj) = state_data.get("LL").and_then(|v| v.as_object()) {
                    if let Some(state_value_str) = ll_obj.get("value").and_then(|v| v.as_str()) {
                        // Try to parse temperature strings like "27.0°", humidity "52%", or light "6Lx"
                        if let Some(parsed_val) = extract_numeric_value(state_value_str) {
                            meaningful_value = parsed_val;
                            has_meaningful_value = true;
                            tracing::debug!(
                                "Extracted sensor value from LL.value for {}: {} -> {}",
                                device.name,
                                state_value_str,
                                parsed_val
                            );
                        }
                    }
                }
                // Fallback: check if it's a direct numeric value
                else if let Some(direct_value) = state_data.as_f64() {
                    meaningful_value = direct_value;
                    has_meaningful_value = true;
                    tracing::debug!(
                        "Found direct sensor numeric value for {}: {}",
                        device.name,
                        direct_value
                    );
                }
                // Fallback: check if it's a direct string value to parse
                else if let Some(direct_str) = state_data.as_str() {
                    if let Some(parsed_val) = extract_numeric_value(direct_str) {
                        meaningful_value = parsed_val;
                        has_meaningful_value = true;
                        tracing::debug!(
                            "Parsed direct sensor string for {}: {} -> {}",
                            device.name,
                            direct_str,
                            parsed_val
                        );
                    }
                }
            }

            // If still no value, try the legacy UUID reference approach
            if !has_meaningful_value {
                if let Some(value_ref) = device.states.get("value") {
                    if let Some(uuid_str) = value_ref.as_str() {
                        // This is a UUID reference - look it up in device_states
                        if let Some(referenced_state) = device_states.get(uuid_str) {
                            if let Some(ref_value) = referenced_state.as_f64() {
                                meaningful_value = ref_value;
                                has_meaningful_value = true;
                                tracing::debug!(
                                    "Found sensor value via UUID reference for {}: {} -> {}",
                                    device.name,
                                    uuid_str,
                                    ref_value
                                );
                            } else if let Some(ref_str) = referenced_state.as_str() {
                                if let Some(parsed_val) = extract_numeric_value(ref_str) {
                                    meaningful_value = parsed_val;
                                    has_meaningful_value = true;
                                    tracing::debug!(
                                        "Parsed sensor string value for {}: {} -> {}",
                                        device.name,
                                        ref_str,
                                        parsed_val
                                    );
                                }
                            }
                        }
                    } else if let Some(direct_value) = value_ref.as_f64() {
                        meaningful_value = direct_value;
                        has_meaningful_value = true;
                        tracing::debug!(
                            "Found direct sensor value in states for {}: {}",
                            device.name,
                            direct_value
                        );
                    }
                }
            }

            // Log sensor device info for debugging (only if still no value found)
            if !has_meaningful_value {
                tracing::debug!(
                    "No meaningful value found for sensor '{}' - states: {:?}, real-time: {:?}",
                    device.name,
                    device.states,
                    device_state
                );
            }
        }

        // Use meaningful value if found, otherwise fall back to effective_state logic
        let effective_state = if has_meaningful_value {
            meaningful_value
        } else if state_value > 0.0 {
            state_value
        } else {
            cached_active
        };

        let (status, status_color, state_display, is_active) = match device.category.as_str() {
            "lights" => {
                if effective_state > 0.0 {
                    let brightness = (effective_state * 100.0).round() as i32;
                    (
                        "On".to_string(),
                        "green".to_string(),
                        format!("On ({}%)", brightness),
                        true,
                    )
                } else {
                    (
                        "Off".to_string(),
                        "gray".to_string(),
                        "Off".to_string(),
                        false,
                    )
                }
            }
            "shading" => {
                let position = (effective_state * 100.0).round() as i32;
                if position > 0 {
                    (
                        "Closed".to_string(),
                        "blue".to_string(),
                        format!("{}%", position),
                        true,
                    )
                } else {
                    (
                        "Open".to_string(),
                        "gray".to_string(),
                        "Open".to_string(),
                        false,
                    )
                }
            }
            _ => {
                // For sensors and other devices
                if has_meaningful_value {
                    // Show actual sensor readings
                    let display_val = if meaningful_value > 100.0 {
                        format!("{:.0}", meaningful_value) // Large values like temperatures
                    } else {
                        format!("{:.1}", meaningful_value) // Small values like percentages
                    };
                    ("Active".to_string(), "green".to_string(), display_val, true)
                } else if effective_state > 0.0 {
                    (
                        "Active".to_string(),
                        "green".to_string(),
                        format!("{:.1}", effective_state),
                        true,
                    )
                } else {
                    // Check if device has any non-zero cached states
                    let has_activity = device
                        .states
                        .values()
                        .any(|v| v.as_f64().unwrap_or(0.0) > 0.0);
                    if has_activity {
                        (
                            "Active".to_string(),
                            "yellow".to_string(),
                            "Active".to_string(),
                            true,
                        )
                    } else {
                        (
                            "Idle".to_string(),
                            "gray".to_string(),
                            "Idle".to_string(),
                            false,
                        )
                    }
                }
            }
        };

        let device_json = json!({
            "uuid": device.uuid,
            "name": device.name,
            "device_type": device.device_type,
            "type": device.device_type,
            "room": device.room,
            "status": status,
            "status_color": status_color,
            "state_display": state_display,
            "states": {
                "active": if is_active { effective_state } else { 0.0 },
                "value": effective_state
            },
            "raw_state": device_state,
            "cached_states": device.states,
        });

        match device.category.as_str() {
            "lights" => lights_data.push(device_json),
            "shading" => blinds_data.push(device_json),
            "climate" => climate_data.push(device_json),
            _ => other_data.push(device_json),
        }
    }

    // Collect all devices into a single vector for device_matrix
    let mut all_devices = Vec::new();
    all_devices.extend(lights_data.clone());
    all_devices.extend(blinds_data.clone());
    all_devices.extend(climate_data.clone());
    all_devices.extend(other_data.clone());

    // Build device matrix for dashboard
    let mut device_matrix = Vec::new();
    for room in &rooms_data {
        if let Some(room_name) = room.get("name").and_then(|n| n.as_str()) {
            let room_devices: Vec<_> = all_devices
                .iter()
                .filter(|d| d.get("room").and_then(|r| r.as_str()) == Some(room_name))
                .cloned()
                .collect();

            if !room_devices.is_empty() {
                device_matrix.push(json!({
                    "room_name": room_name,
                    "devices": room_devices
                }));
            }
        }
    }

    // Build final response in the format expected by unified dashboard
    json!({
        "realtime": {
            "system_health": {
                "connection_status": connection_status,
                "last_update": chrono::Utc::now().to_rfc3339(),
                "error_rate": 0.0,
                "avg_response_time_ms": 50.0
            },
            "active_sensors": [],
            "recent_activity": []
        },
        "devices": {
            "device_matrix": device_matrix,
            "rooms": rooms_data,
            "lights": lights_data,
            "blinds": blinds_data,
            "climate": climate_data,
            "other": other_data,
            "summary": {
                "total_devices": devices.len(),
                "active_devices": device_states.values().filter(|v| v.as_f64().unwrap_or(0.0) > 0.0).count(),
                "rooms": rooms.len()
            }
        },
        "operational": {
            "performance": {
                "cpu_usage": 25.0,
                "memory_usage": 45.0,
                "disk_usage": 60.0
            },
            "network": {
                "requests_per_minute": 150,
                "response_time": 45.0,
                "error_rate": 0.1
            },
            "statistics": {
                "total_rooms": rooms.len(),
                "total_devices": devices.len(),
                "device_states_fetched": device_states.len(),
                "connection_status": connection_status
            }
        },
        "trends": {
            "daily_activity": [],
            "device_usage": [],
            "performance_trends": []
        },
        "metadata": {
            "last_update": chrono::Utc::now().to_rfc3339(),
            "data_age_seconds": 0,
            "cache_status": "live",
            "version": "1.0.0"
        }
    })
}

/// Legacy function using cached states only
async fn get_dashboard_data_from_context_legacy(context: &Arc<ClientContext>) -> Value {
    // Get connection status
    let connection_status = if *context.connected.read().await {
        "Connected"
    } else {
        "Disconnected"
    };

    // Get data from context
    let rooms = context.rooms.read().await;
    let devices = context.devices.read().await;
    // TODO: Fix when sensor_readings is available in ClientContext
    // let sensor_readings = context.sensor_readings.read().await;
    let sensor_readings: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    let capabilities = context.capabilities.read().await;

    // Build rooms data with device counts and sensor info
    let mut rooms_data = Vec::new();
    for (_uuid, room) in rooms.iter() {
        // Count devices in this room
        let room_devices: Vec<_> = devices
            .values()
            .filter(|device| device.room.as_ref() == Some(&room.name))
            .collect();

        // Get sensor data for this room
        let room_sensors: Vec<_> = sensor_readings
            .iter()
            .filter(|(_, _reading)| {
                // TODO: Fix when sensor structure is available
                false // reading.location.as_ref() == Some(&room.name) || reading.name.contains(&room.name)
            })
            .collect();

        // Extract temperature and humidity if available
        let current_temp: Option<f64> = None;
        let current_humidity: Option<f64> = None;

        // TODO: Fix when sensor readings are available
        // for (_, sensor) in &room_sensors {
        //     match sensor.sensor_type {
        //         crate::tools::sensors::SensorType::Temperature => {
        //             current_temp = Some(sensor.value);
        //         }
        //         crate::tools::sensors::SensorType::Humidity => {
        //             current_humidity = Some(sensor.value);
        //         }
        //         _ => {}
        //     }
        // }

        // Count active devices
        let active_count = room_devices
            .iter()
            .filter(|device| {
                device
                    .states
                    .get("active")
                    .and_then(|v| v.as_f64())
                    .map(|v| v > 0.0)
                    .unwrap_or(false)
            })
            .count();

        rooms_data.push(json!({
            "name": room.name,
            "uuid": room.uuid,
            "device_count": room_devices.len(),
            "active_devices": active_count,
            "current_temp": current_temp,
            "current_humidity": current_humidity,
            "sensors": room_sensors.len(),
        }));
    }

    // Build devices data by category
    let mut lights_data = Vec::new();
    let mut blinds_data = Vec::new();
    let mut climate_data = Vec::new();
    let mut other_data = Vec::new();

    for device in devices.values() {
        let device_json = json!({
            "uuid": device.uuid,
            "name": device.name,
            "type": device.device_type,
            "room": device.room,
            "states": device.states,
        });

        match device.category.as_str() {
            "lights" => lights_data.push(device_json),
            "shading" => blinds_data.push(device_json),
            "climate" => climate_data.push(device_json),
            _ => other_data.push(device_json),
        }
    }

    // Build sensors data
    let _sensors_data: Vec<_> = sensor_readings
        .keys()
        .map(|uuid| {
            json!({
                "uuid": uuid,
                "name": "Unknown", // reading.name,
                "type": "unknown", // reading.sensor_type,
                "value": 0.0, // reading.value,
                "unit": "unknown", // reading.unit,
                "location": "unknown", // reading.location,
                "status": "unknown", // reading.status,
                "timestamp": chrono::Utc::now(), // reading.timestamp,
            })
        })
        .collect();

    // Count active sensors
    let active_sensors = sensor_readings
        .iter()
        // .filter(|(_, r)| r.status == crate::tools::sensors::SensorStatus::Online)
        .filter(|_| true) // TODO: Fix when SensorStatus is available
        .count();

    // Collect all devices into a single vector for device_matrix
    let mut all_devices = Vec::new();
    all_devices.extend(lights_data.clone());
    all_devices.extend(blinds_data.clone());
    all_devices.extend(climate_data.clone());
    all_devices.extend(other_data.clone());

    // Build device matrix for dashboard
    let mut device_matrix = Vec::new();
    for room in &rooms_data {
        if let Some(room_name) = room.get("name").and_then(|n| n.as_str()) {
            let room_devices: Vec<_> = all_devices
                .iter()
                .filter(|d| d.get("room").and_then(|r| r.as_str()) == Some(room_name))
                .cloned()
                .collect();

            if !room_devices.is_empty() {
                device_matrix.push(json!({
                    "room_name": room_name,
                    "devices": room_devices
                }));
            }
        }
    }

    // Build final dashboard data
    json!({
        "realtime": {
            "connection_status": connection_status,
            "last_update": chrono::Utc::now().format("%H:%M:%S").to_string(),
            "error_rate": 0.0,
            "response_time_ms": 50,
            "active_sensors": active_sensors,
            "recent_activity": [
                {
                    "timestamp": chrono::Utc::now(),
                    "type": "status_check",
                    "description": "Dashboard - Data Refresh"
                }
            ]
        },
        "devices": {
            "rooms": rooms_data,
            "lights": lights_data,
            "blinds": blinds_data,
            "climate": climate_data,
            "other": other_data,
            "device_matrix": device_matrix,
            "categories": {
                "lights": lights_data.len(),
                "blinds": blinds_data.len(),
                "climate": climate_data.len(),
                "other": other_data.len()
            }
        },
        "operational": {
            "api_performance": {
                "request_rate": 0.0,
                "avg_response_time_ms": 50,
                "error_rate": 0.0
            },
            "rate_limiter": {
                "total_requests": 0,
                "blocked_requests": 0,
                "current_window_requests": 0
            },
            "resources": {
                "websocket_connections": 0,
                "memory_usage_mb": 0,
                "cpu_usage_percent": 0.0
            }
        },
        "historical": {
            "enabled": false,
            "data_points": 0,
            "retention_days": 0
        },
        "metadata": {
            "timestamp": chrono::Utc::now(),
            "last_update": chrono::Utc::now(),
            "data_age_seconds": 0,
            "collection_stats": {
                "total_collections": 0,
                "success_rate_percent": 100.0,
                "avg_collection_time_ms": 0.0,
                "last_error": null
            },
            "version": "1.0.0",
            "capabilities": {
                "lights": capabilities.light_count,
                "blinds": capabilities.blind_count,
                "climate": capabilities.climate_count,
                "sensors": capabilities.sensor_count,
            }
        }
    })
}

/// Fetch sensor data from MCP tools to supplement device information
async fn fetch_mcp_sensor_data(server: &LoxoneMcpServer) -> HashMap<String, (f64, String)> {
    let mut sensor_values = HashMap::new();

    // Try to get temperature sensors from MCP tools
    tracing::debug!("Attempting to fetch MCP temperature sensors...");
    match server.get_temperature_sensors().await {
        Ok(sensor_result) => {
            tracing::debug!(
                "Successfully got sensor result with {} content items",
                sensor_result.content.len()
            );
            // Extract content from CallToolResult
            if let Some(content) = sensor_result.content.first() {
                let content_text = match content {
                    mcp_foundation::Content::Text { text } => text.clone(),
                    _ => return sensor_values,
                };
                tracing::debug!("Sensor result content length: {}", content_text.len());
                if let Ok(sensors_value) = serde_json::from_str::<Value>(&content_text) {
                    if let Some(sensors_array) =
                        sensors_value.get("sensors").and_then(|s| s.as_array())
                    {
                        tracing::debug!("Found {} sensors in MCP result", sensors_array.len());
                        for sensor in sensors_array {
                            if let (Some(uuid), Some(name), Some(raw_value)) = (
                                sensor.get("uuid").and_then(|u| u.as_str()),
                                sensor.get("name").and_then(|n| n.as_str()),
                                sensor.get("raw_value").and_then(|r| r.as_object()),
                            ) {
                                if let Some(ll_data) =
                                    raw_value.get("LL").and_then(|ll| ll.as_object())
                                {
                                    if let Some(value_str) =
                                        ll_data.get("value").and_then(|v| v.as_str())
                                    {
                                        // Parse temperature values like "24.9°" or humidity like "53%"
                                        if let Some(numeric_part) = extract_numeric_value(value_str)
                                        {
                                            sensor_values.insert(
                                                uuid.to_string(),
                                                (numeric_part, name.to_string()),
                                            );
                                            tracing::debug!(
                                                "Added sensor: {} = {} ({})",
                                                name,
                                                numeric_part,
                                                value_str
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        tracing::warn!("No 'sensors' array found in MCP result");
                    }
                } else {
                    tracing::warn!("Failed to parse sensor result as JSON");
                }
            } else {
                tracing::warn!("No content found in sensor result");
            }
        }
        Err(e) => {
            tracing::error!("Failed to fetch temperature sensors: {}", e);
        }
    }

    tracing::info!(
        "Fetched {} sensor values from MCP tools",
        sensor_values.len()
    );
    sensor_values
}

/// Extract numeric value from sensor strings like "24.9°", "53%", "6Lx"
fn extract_numeric_value(value_str: &str) -> Option<f64> {
    // Remove common units and parse the numeric part
    let cleaned = value_str
        .replace("°", "")
        .replace("%", "")
        .replace("Lx", "")
        .trim()
        .to_string();

    cleaned.parse::<f64>().ok()
}

/// Determine which room a sensor belongs to based on its name
fn determine_sensor_room(
    sensor_name: &str,
    rooms: &std::collections::HashMap<String, crate::client::LoxoneRoom>,
) -> Option<String> {
    // First try exact room name matches in sensor name
    for (_uuid, room) in rooms.iter() {
        if sensor_name.contains(&room.name) {
            return Some(room.name.clone());
        }
    }

    // Then try common room name patterns
    let lower_name = sensor_name.to_lowercase();
    if lower_name.contains("wohnzimmer") {
        return Some("Wohnzimmer".to_string());
    }
    if lower_name.contains("schlafzimmer") {
        return Some("Schlafzimmer".to_string());
    }
    if lower_name.contains("küche") {
        return Some("Küche".to_string());
    }
    if lower_name.contains("bad") {
        return Some("Bad".to_string());
    }
    if lower_name.contains("flur") {
        return Some("Flur".to_string());
    }
    if lower_name.contains("arbeitszimmer") {
        return Some("Arbeitszimmer".to_string());
    }
    if lower_name.contains("terrasse") {
        return Some("Terrasse Li.".to_string());
    }
    if lower_name.contains("zimmer") && lower_name.contains("og") {
        return Some("Zimmer OG".to_string());
    }
    if lower_name.contains("treppe") {
        return Some("Flur".to_string());
    }

    None
}
