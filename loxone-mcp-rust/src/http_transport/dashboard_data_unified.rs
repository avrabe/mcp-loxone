//! Unified dashboard data using the new value resolution service
//!
//! This module replaces the complex fallback logic in dashboard_data.rs
//! with a clean implementation using the UnifiedValueResolver.

use crate::server::LoxoneMcpServer;
use crate::services::sensor_registry::SensorType;
use crate::services::value_resolution::ResolvedValue;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Get dashboard data using unified value resolution (replaces get_dashboard_data_from_server)
pub async fn get_unified_dashboard_data(server: &LoxoneMcpServer) -> Value {
    let resolver = server.get_value_resolver();
    let context = &server.context;

    // Get connection status
    let connection_status = if *context.connected.read().await {
        "Connected"
    } else {
        "Disconnected"
    };

    // Get all devices and rooms
    let devices = context.devices.read().await;
    let rooms = context.rooms.read().await;

    // Get all device UUIDs
    let all_device_uuids: Vec<String> = devices.keys().cloned().collect();

    // Resolve all values efficiently in batch
    let resolved_values = match resolver.resolve_batch_values(&all_device_uuids).await {
        Ok(values) => values,
        Err(e) => {
            tracing::error!("Failed to resolve device values: {}", e);
            HashMap::new()
        }
    };

    tracing::info!(
        "Dashboard: Resolved {} values from {} devices",
        resolved_values.len(),
        all_device_uuids.len()
    );

    // Build rooms data with real-time sensor integration
    let mut rooms_data = Vec::new();
    for (room_uuid, room) in rooms.iter() {
        let room_devices: Vec<_> = devices
            .values()
            .filter(|device| device.room.as_ref() == Some(&room.name))
            .collect();

        // Get sensor readings for this room
        let mut room_temp: Option<f64> = None;
        let mut room_humidity: Option<f64> = None;
        let mut active_sensors = 0;
        let mut active_devices = 0;

        for device in &room_devices {
            if let Some(resolved) = resolved_values.get(&device.uuid) {
                // Count active devices (non-sensors)
                if resolved.numeric_value.unwrap_or(0.0) > 0.0 {
                    active_devices += 1;
                }

                // Extract sensor values
                match &resolved.sensor_type {
                    Some(SensorType::Temperature { .. }) => {
                        if let Some(value) = resolved.numeric_value {
                            room_temp = Some(value);
                            active_sensors += 1;
                        }
                    }
                    Some(SensorType::Humidity { .. }) => {
                        if let Some(value) = resolved.numeric_value {
                            room_humidity = Some(value);
                            active_sensors += 1;
                        }
                    }
                    Some(_) => {
                        if resolved.numeric_value.is_some() {
                            active_sensors += 1;
                        }
                    }
                    None => {}
                }
            }
        }

        rooms_data.push(json!({
            "name": room.name,
            "uuid": room_uuid,
            "device_count": room_devices.len(),
            "active_devices": active_devices,
            "active_sensors": active_sensors,
            "current_temp": room_temp,
            "current_humidity": room_humidity,
        }));
    }

    // Build devices data by category with resolved values
    let mut lights_data = Vec::new();
    let mut blinds_data = Vec::new();
    let mut climate_data = Vec::new();
    let mut other_data = Vec::new();

    for device in devices.values() {
        let resolved = resolved_values.get(&device.uuid);
        let device_json = build_device_json(device, resolved);

        match device.category.as_str() {
            "lights" => lights_data.push(device_json),
            "shading" => blinds_data.push(device_json),
            "climate" => climate_data.push(device_json),
            _ => {
                // Check if it's a sensor based on resolved type
                if let Some(resolved) = resolved {
                    if resolved.sensor_type.is_some() {
                        climate_data.push(device_json);
                    } else {
                        other_data.push(device_json);
                    }
                } else {
                    other_data.push(device_json);
                }
            }
        }
    }

    // Build device matrix for dashboard
    let mut device_matrix = Vec::new();
    for room in &rooms_data {
        if let Some(room_name) = room.get("name").and_then(|n| n.as_str()) {
            let mut all_room_devices = Vec::new();

            // Collect all devices in this room
            for device_list in [&lights_data, &blinds_data, &climate_data, &other_data] {
                all_room_devices.extend(
                    device_list
                        .iter()
                        .filter(|d| d.get("room").and_then(|r| r.as_str()) == Some(room_name))
                        .cloned(),
                );
            }

            if !all_room_devices.is_empty() {
                device_matrix.push(json!({
                    "room_name": room_name,
                    "devices": all_room_devices
                }));
            }
        }
    }

    // Count active sensors
    let active_sensor_count = resolved_values
        .values()
        .filter(|v| v.sensor_type.is_some() && v.numeric_value.is_some())
        .count();

    // Count active devices
    let active_device_count = resolved_values
        .values()
        .filter(|v| v.numeric_value.unwrap_or(0.0) > 0.0)
        .count();

    // Get real server metrics
    let metrics_collector = server.get_metrics_collector();
    let server_metrics = metrics_collector.get_metrics().await;

    // Build final response
    json!({
        "realtime": {
            "system_health": {
                "connection_status": connection_status,
                "last_update": chrono::Utc::now().to_rfc3339(),
                "resolved_values": resolved_values.len(),
                "sensors_active": active_sensor_count,
                "error_rate": server_metrics.errors.error_rate_percent,
                "avg_response_time_ms": server_metrics.network.average_response_time_ms,
                "total_requests": server_metrics.network.total_requests,
                "active_connections": server_metrics.network.active_connections,
                "uptime_seconds": server_metrics.uptime.uptime_seconds,
                "memory_usage_mb": server_metrics.performance.memory_usage_mb,
                "cpu_usage_percent": server_metrics.performance.cpu_usage_percent
            },
            "active_sensors": [],  // Could populate with actual sensor data
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
                "active_devices": active_device_count,
                "active_sensors": active_sensor_count,
                "rooms": rooms.len()
            }
        },
        "operational": {
            "performance": {
                "cpu_usage": server_metrics.performance.cpu_usage_percent,
                "memory_usage": server_metrics.performance.memory_usage_percent,
                "disk_usage": server_metrics.performance.disk_usage_percent
            },
            "network": {
                "requests_per_minute": server_metrics.network.requests_per_minute,
                "response_time": server_metrics.network.average_response_time_ms,
                "error_rate": server_metrics.errors.error_rate_percent
            },
            "mcp": {
                "tools_executed": server_metrics.mcp.tools_executed,
                "resources_accessed": server_metrics.mcp.resources_accessed,
                "prompts_processed": server_metrics.mcp.prompts_processed,
                "average_tool_time_ms": server_metrics.mcp.average_tool_execution_ms,
                "active_sessions": server_metrics.mcp.active_mcp_sessions,
                "most_used_tool": server_metrics.mcp.most_used_tool
            },
            "cache": {
                "hit_rate_percent": server_metrics.cache.hit_rate_percent,
                "miss_rate_percent": server_metrics.cache.miss_rate_percent,
                "total_entries": server_metrics.cache.total_cache_entries,
                "memory_mb": server_metrics.cache.cache_memory_mb
            },
            "uptime": {
                "uptime_seconds": server_metrics.uptime.uptime_seconds,
                "uptime_formatted": server_metrics.uptime.uptime_formatted,
                "start_time": server_metrics.uptime.start_time,
                "availability_percent": server_metrics.uptime.availability_percent
            },
            "statistics": {
                "total_rooms": rooms.len(),
                "total_devices": devices.len(),
                "device_states_fetched": resolved_values.len(),
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
            "version": "2.0.0-unified",
            "data_source": "unified_value_resolver"
        }
    })
}

/// Build device JSON with resolved value
fn build_device_json(
    device: &crate::client::LoxoneDevice,
    resolved: Option<&ResolvedValue>,
) -> Value {
    let (status, status_color, state_display, numeric_value) = match resolved {
        Some(resolved) => {
            match &resolved.sensor_type {
                Some(sensor_type) => {
                    // Handle different sensor types appropriately
                    match sensor_type {
                        crate::services::sensor_registry::SensorType::Temperature { .. }
                        | crate::services::sensor_registry::SensorType::Humidity { .. }
                        | crate::services::sensor_registry::SensorType::Illuminance { .. }
                        | crate::services::sensor_registry::SensorType::AirPressure { .. }
                        | crate::services::sensor_registry::SensorType::AirQuality { .. } => {
                            // Environmental sensors - show readings
                            if resolved.numeric_value.is_some() {
                                (
                                    "Active".to_string(),
                                    "green".to_string(),
                                    resolved.formatted_value.clone(),
                                    resolved.numeric_value.unwrap_or(0.0),
                                )
                            } else {
                                (
                                    "Offline".to_string(),
                                    "gray".to_string(),
                                    "No Data".to_string(),
                                    0.0,
                                )
                            }
                        }
                        crate::services::sensor_registry::SensorType::BlindPosition { .. }
                        | crate::services::sensor_registry::SensorType::WindowPosition { .. } => {
                            // Position sensors - show position
                            if let Some(numeric) = resolved.numeric_value {
                                if numeric > 0.0 {
                                    (
                                        "Positioned".to_string(),
                                        "blue".to_string(),
                                        resolved.formatted_value.clone(),
                                        numeric,
                                    )
                                } else {
                                    (
                                        "Closed".to_string(),
                                        "gray".to_string(),
                                        "Closed".to_string(),
                                        0.0,
                                    )
                                }
                            } else {
                                (
                                    "Unknown".to_string(),
                                    "gray".to_string(),
                                    "Unknown".to_string(),
                                    0.0,
                                )
                            }
                        }
                        crate::services::sensor_registry::SensorType::MotionDetector
                        | crate::services::sensor_registry::SensorType::DoorWindowContact => {
                            // Binary sensors - show state
                            if let Some(numeric) = resolved.numeric_value {
                                if numeric > 0.0 {
                                    (
                                        "Triggered".to_string(),
                                        "orange".to_string(),
                                        resolved.formatted_value.clone(),
                                        numeric,
                                    )
                                } else {
                                    (
                                        "Ready".to_string(),
                                        "green".to_string(),
                                        resolved.formatted_value.clone(),
                                        0.0,
                                    )
                                }
                            } else {
                                (
                                    "Standby".to_string(),
                                    "blue".to_string(),
                                    resolved.formatted_value.clone(),
                                    0.0,
                                )
                            }
                        }
                        crate::services::sensor_registry::SensorType::PowerMeter { .. }
                        | crate::services::sensor_registry::SensorType::EnergyConsumption {
                            ..
                        } => {
                            // Power sensors - show consumption
                            if let Some(numeric) = resolved.numeric_value {
                                if numeric > 0.0 {
                                    (
                                        "Consuming".to_string(),
                                        "orange".to_string(),
                                        resolved.formatted_value.clone(),
                                        numeric,
                                    )
                                } else {
                                    (
                                        "Idle".to_string(),
                                        "gray".to_string(),
                                        "0W".to_string(),
                                        0.0,
                                    )
                                }
                            } else {
                                (
                                    "Offline".to_string(),
                                    "red".to_string(),
                                    "No Data".to_string(),
                                    0.0,
                                )
                            }
                        }
                        crate::services::sensor_registry::SensorType::SoundLevel { .. } => {
                            // Audio devices
                            if let Some(numeric) = resolved.numeric_value {
                                if numeric > 0.0 {
                                    (
                                        "Playing".to_string(),
                                        "green".to_string(),
                                        resolved.formatted_value.clone(),
                                        numeric,
                                    )
                                } else {
                                    (
                                        "Muted".to_string(),
                                        "gray".to_string(),
                                        "Muted".to_string(),
                                        0.0,
                                    )
                                }
                            } else {
                                (
                                    "Ready".to_string(),
                                    "blue".to_string(),
                                    resolved.formatted_value.clone(),
                                    0.0,
                                )
                            }
                        }
                        _ => {
                            // Default sensor handling
                            if resolved.numeric_value.is_some() {
                                (
                                    "Active".to_string(),
                                    "green".to_string(),
                                    resolved.formatted_value.clone(),
                                    resolved.numeric_value.unwrap_or(0.0),
                                )
                            } else {
                                (
                                    "Ready".to_string(),
                                    "blue".to_string(),
                                    resolved.formatted_value.clone(),
                                    0.0,
                                )
                            }
                        }
                    }
                }
                None => {
                    // Regular device - use device-specific logic
                    match device.category.as_str() {
                        "lights" => {
                            if let Some(numeric) = resolved.numeric_value {
                                if numeric > 0.0 {
                                    let brightness = (numeric * 100.0).round() as i32;
                                    (
                                        "On".to_string(),
                                        "green".to_string(),
                                        format!("On ({}%)", brightness),
                                        numeric,
                                    )
                                } else {
                                    (
                                        "Off".to_string(),
                                        "gray".to_string(),
                                        "Off".to_string(),
                                        0.0,
                                    )
                                }
                            } else {
                                (
                                    "Unknown".to_string(),
                                    "gray".to_string(),
                                    "Unknown".to_string(),
                                    0.0,
                                )
                            }
                        }
                        "shading" => {
                            if let Some(numeric) = resolved.numeric_value {
                                let position = (numeric * 100.0).round() as i32;
                                if position > 0 {
                                    (
                                        "Closed".to_string(),
                                        "blue".to_string(),
                                        format!("{}%", position),
                                        numeric,
                                    )
                                } else {
                                    (
                                        "Open".to_string(),
                                        "gray".to_string(),
                                        "Open".to_string(),
                                        0.0,
                                    )
                                }
                            } else {
                                (
                                    "Unknown".to_string(),
                                    "gray".to_string(),
                                    "Unknown".to_string(),
                                    0.0,
                                )
                            }
                        }
                        "switches" | "controls" => {
                            // Handle switches and control devices
                            if let Some(numeric) = resolved.numeric_value {
                                if numeric > 0.0 {
                                    (
                                        "On".to_string(),
                                        "green".to_string(),
                                        "On".to_string(),
                                        numeric,
                                    )
                                } else {
                                    (
                                        "Off".to_string(),
                                        "gray".to_string(),
                                        "Off".to_string(),
                                        0.0,
                                    )
                                }
                            } else {
                                // Check the formatted value for text states
                                let state_text = &resolved.formatted_value;
                                if state_text.to_lowercase().contains("on")
                                    || state_text.to_lowercase().contains("active")
                                {
                                    (
                                        "On".to_string(),
                                        "green".to_string(),
                                        state_text.clone(),
                                        1.0,
                                    )
                                } else if state_text.to_lowercase().contains("off")
                                    || state_text.to_lowercase().contains("inactive")
                                {
                                    (
                                        "Off".to_string(),
                                        "gray".to_string(),
                                        state_text.clone(),
                                        0.0,
                                    )
                                } else {
                                    (
                                        "Unknown".to_string(),
                                        "gray".to_string(),
                                        state_text.clone(),
                                        0.0,
                                    )
                                }
                            }
                        }
                        "security" => {
                            // Handle security devices
                            if let Some(numeric) = resolved.numeric_value {
                                if numeric > 0.0 {
                                    (
                                        "Armed".to_string(),
                                        "red".to_string(),
                                        "Armed".to_string(),
                                        numeric,
                                    )
                                } else {
                                    (
                                        "Disarmed".to_string(),
                                        "green".to_string(),
                                        "Disarmed".to_string(),
                                        0.0,
                                    )
                                }
                            } else {
                                let state_text = &resolved.formatted_value;
                                if state_text.to_lowercase().contains("armed")
                                    || state_text.to_lowercase().contains("active")
                                {
                                    (
                                        "Armed".to_string(),
                                        "red".to_string(),
                                        state_text.clone(),
                                        1.0,
                                    )
                                } else {
                                    (
                                        "Disarmed".to_string(),
                                        "green".to_string(),
                                        state_text.clone(),
                                        0.0,
                                    )
                                }
                            }
                        }
                        _ => {
                            // Enhanced generic device handling
                            if let Some(numeric) = resolved.numeric_value {
                                if numeric > 0.0 {
                                    // Check if it's a percentage value
                                    if resolved.unit.as_ref().is_some_and(|u| u.contains('%')) {
                                        (
                                            "Active".to_string(),
                                            "blue".to_string(),
                                            resolved.formatted_value.clone(),
                                            numeric,
                                        )
                                    } else if numeric == 1.0 {
                                        // Binary on/off device
                                        (
                                            "On".to_string(),
                                            "green".to_string(),
                                            "On".to_string(),
                                            numeric,
                                        )
                                    } else {
                                        // Show actual value
                                        (
                                            "Active".to_string(),
                                            "green".to_string(),
                                            resolved.formatted_value.clone(),
                                            numeric,
                                        )
                                    }
                                } else {
                                    // Check formatted value for meaningful states
                                    let state_text = &resolved.formatted_value;
                                    if state_text.to_lowercase() == "closed" {
                                        (
                                            "Closed".to_string(),
                                            "blue".to_string(),
                                            "Closed".to_string(),
                                            0.0,
                                        )
                                    } else if state_text.to_lowercase() == "open" {
                                        (
                                            "Open".to_string(),
                                            "green".to_string(),
                                            "Open".to_string(),
                                            0.0,
                                        )
                                    } else if state_text.to_lowercase() != "idle"
                                        && !state_text.is_empty()
                                    {
                                        // Show the actual formatted state instead of "Idle"
                                        (
                                            "Ready".to_string(),
                                            "blue".to_string(),
                                            state_text.clone(),
                                            0.0,
                                        )
                                    } else {
                                        (
                                            "Off".to_string(),
                                            "gray".to_string(),
                                            "Off".to_string(),
                                            0.0,
                                        )
                                    }
                                }
                            } else {
                                // No numeric value - check formatted value
                                let state_text = &resolved.formatted_value;
                                if !state_text.is_empty() && state_text.to_lowercase() != "idle" {
                                    // Check for common state patterns
                                    if state_text.to_lowercase().contains("closed") {
                                        (
                                            "Closed".to_string(),
                                            "blue".to_string(),
                                            state_text.clone(),
                                            0.0,
                                        )
                                    } else if state_text.to_lowercase().contains("open") {
                                        (
                                            "Open".to_string(),
                                            "green".to_string(),
                                            state_text.clone(),
                                            0.0,
                                        )
                                    } else if state_text.to_lowercase().contains("on") {
                                        (
                                            "On".to_string(),
                                            "green".to_string(),
                                            state_text.clone(),
                                            1.0,
                                        )
                                    } else if state_text.to_lowercase().contains("off") {
                                        (
                                            "Off".to_string(),
                                            "gray".to_string(),
                                            state_text.clone(),
                                            0.0,
                                        )
                                    } else {
                                        // Show actual state instead of "Idle"
                                        (
                                            "Ready".to_string(),
                                            "blue".to_string(),
                                            state_text.clone(),
                                            0.0,
                                        )
                                    }
                                } else {
                                    (
                                        "Standby".to_string(),
                                        "gray".to_string(),
                                        "Standby".to_string(),
                                        0.0,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
        None => (
            "Unknown".to_string(),
            "red".to_string(),
            "No Data".to_string(),
            0.0,
        ),
    };

    json!({
        "uuid": device.uuid,
        "name": device.name,
        "device_type": device.device_type,
        "type": device.device_type,
        "sensor_type": resolved.and_then(|r| r.sensor_type.as_ref()).map(|t| {
            let type_str = format!("{:?}", t);
            type_str.split('{').next().unwrap_or("Unknown").to_string()
        }),
        "room": device.room,
        "status": status,
        "status_color": status_color,
        "state_display": state_display,
        "confidence": resolved.map(|r| r.confidence).unwrap_or(0.0),
        "validation_status": resolved.map(|r| format!("{:?}", r.validation_status)).unwrap_or_else(|| "Unknown".to_string()),
        "source": resolved.map(|r| format!("{:?}", r.source)).unwrap_or_else(|| "Unknown".to_string()),
        "states": {
            "active": if numeric_value > 0.0 { numeric_value } else { 0.0 },
            "value": numeric_value
        },
        "resolved_value": resolved.map(|r| json!({
            "numeric": r.numeric_value,
            "formatted": r.formatted_value,
            "unit": r.unit,
            "timestamp": r.timestamp,
        })),
        "cached_states": device.states,
        "raw_state": resolved.map(|r| &r.raw_value),
    })
}
