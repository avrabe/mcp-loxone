//! Unified sensor tools using the value resolution service
//!
//! This module provides sensor tools that use the unified value resolver
//! for consistent value parsing across the system.

use crate::tools::{value_helpers::*, ToolContext, ToolResponse};
use serde_json::json;

/// Get all temperature sensor readings using unified value resolution
pub async fn get_temperature_sensors_unified(context: ToolContext) -> ToolResponse {
    // Ensure we're connected
    if let Err(e) = context.ensure_connected().await {
        return ToolResponse::error(format!("Connection error: {}", e));
    }

    // Get all devices
    let all_devices = match context.get_devices(None).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(format!("Failed to get devices: {}", e)),
    };

    // Filter for temperature sensors
    let temperature_sensors: Vec<_> = all_devices
        .into_iter()
        .filter(|device| {
            // Common temperature sensor patterns
            device.device_type.to_lowercase().contains("temperature")
                || device.device_type == "InfoOnlyAnalog"
                || device.name.to_lowercase().contains("temp")
                || device.name.to_lowercase().contains("temperatur")
                || device.category == "temperature"
        })
        .collect();

    if temperature_sensors.is_empty() {
        return ToolResponse::empty_with_context("No temperature sensors found in the system");
    }

    // Use unified resolver if available
    let sensors = if let Some(resolver) = &context.value_resolver {
        // Batch resolve all sensor values efficiently
        let resolved_values = resolve_batch_values_for_tools(resolver, &temperature_sensors).await;

        let mut sensor_data = Vec::new();
        let mut total_temperature = 0.0;
        let mut valid_readings = 0;

        for (uuid, value) in resolved_values {
            if let Some(device) = temperature_sensors.iter().find(|d| d.uuid == uuid) {
                if let Some(numeric) = value.numeric_value {
                    valid_readings += 1;
                    total_temperature += numeric;
                }

                let sensor_json = create_sensor_json(device, &value);
                sensor_data.push(sensor_json);
            }
        }

        let average_temperature = if valid_readings > 0 {
            Some(total_temperature / valid_readings as f64)
        } else {
            None
        };

        json!({
            "sensors": sensor_data,
            "summary": {
                "total_sensors": sensor_data.len(),
                "valid_readings": valid_readings,
                "average_temperature": average_temperature,
            }
        })
    } else {
        // Fallback to legacy parsing
        let mut sensors = Vec::new();
        let mut total_temperature = 0.0;
        let mut valid_readings = 0;

        for device in temperature_sensors {
            // Get current state
            let state_result = context
                .client
                .get_device_states(&[device.uuid.clone()])
                .await;

            match state_result {
                Ok(states) => {
                    if let Some(state_value) = states.get(&device.uuid) {
                        // Try to parse temperature value
                        let temperature_value = if let Some(num_val) = state_value.as_f64() {
                            valid_readings += 1;
                            total_temperature += num_val;
                            Some(num_val)
                        } else if let Some(str_val) = state_value.as_str() {
                            if let Ok(temp) = str_val.parse::<f64>() {
                                valid_readings += 1;
                                total_temperature += temp;
                                Some(temp)
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let sensor_info = json!({
                            "uuid": device.uuid,
                            "name": device.name,
                            "room": device.room.as_deref().unwrap_or("Unknown"),
                            "value": temperature_value,
                            "raw_value": state_value,
                            "unit": device.states.get("unit").and_then(|v| v.as_str()).unwrap_or("°C"),
                        });

                        sensors.push(sensor_info);
                    } else {
                        let sensor_info = json!({
                            "uuid": device.uuid,
                            "name": device.name,
                            "room": device.room.as_deref().unwrap_or("Unknown"),
                            "value": null,
                            "error": "No state data available",
                        });

                        sensors.push(sensor_info);
                    }
                }
                Err(_) => {
                    let sensor_info = json!({
                        "uuid": device.uuid,
                        "name": device.name,
                        "room": device.room.as_deref().unwrap_or("Unknown"),
                        "value": null,
                        "error": "Failed to fetch state",
                    });

                    sensors.push(sensor_info);
                }
            }
        }

        let average_temperature = if valid_readings > 0 {
            Some(total_temperature / valid_readings as f64)
        } else {
            None
        };

        json!({
            "sensors": sensors,
            "summary": {
                "total_sensors": sensors.len(),
                "valid_readings": valid_readings,
                "average_temperature": average_temperature,
            }
        })
    };

    ToolResponse::success(sensors)
}

/// Get all door/window sensor states using unified value resolution
pub async fn get_door_window_sensors_unified(context: ToolContext) -> ToolResponse {
    // Ensure we're connected
    if let Err(e) = context.ensure_connected().await {
        return ToolResponse::error(format!("Connection error: {}", e));
    }

    // Get all devices
    let all_devices = match context.get_devices(None).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(format!("Failed to get devices: {}", e)),
    };

    // Filter for door/window sensors
    let door_window_sensors: Vec<_> = all_devices
        .into_iter()
        .filter(|device| {
            device.device_type.to_lowercase().contains("door")
                || device.device_type.to_lowercase().contains("window")
                || device.device_type.to_lowercase().contains("contact")
                || device.name.to_lowercase().contains("door")
                || device.name.to_lowercase().contains("window")
                || device.name.to_lowercase().contains("fenster")
                || device.name.to_lowercase().contains("tür")
        })
        .collect();

    if door_window_sensors.is_empty() {
        return ToolResponse::empty_with_context("No door/window sensors found in the system");
    }

    // Use unified resolver if available
    let sensors = if let Some(resolver) = &context.value_resolver {
        // Batch resolve all sensor values efficiently
        let resolved_values = resolve_batch_values_for_tools(resolver, &door_window_sensors).await;

        let mut sensor_data = Vec::new();
        let mut open_count = 0;
        let mut closed_count = 0;

        for (uuid, value) in resolved_values {
            if let Some(device) = door_window_sensors.iter().find(|d| d.uuid == uuid) {
                if let Some(numeric) = value.numeric_value {
                    if numeric > 0.0 {
                        open_count += 1;
                    } else {
                        closed_count += 1;
                    }
                }

                let mut sensor_json = create_sensor_json(device, &value);
                // Add specific door/window status
                sensor_json["status"] = json!(format_sensor_value_display(&value));
                sensor_data.push(sensor_json);
            }
        }

        json!({
            "sensors": sensor_data,
            "summary": {
                "total_sensors": sensor_data.len(),
                "open": open_count,
                "closed": closed_count,
                "all_closed": open_count == 0,
            }
        })
    } else {
        // Fallback to legacy parsing
        let mut sensors = Vec::new();
        let mut open_count = 0;
        let mut closed_count = 0;

        for device in door_window_sensors {
            // Get current state
            let state_result = context
                .client
                .get_device_states(&[device.uuid.clone()])
                .await;

            match state_result {
                Ok(states) => {
                    if let Some(state_value) = states.get(&device.uuid) {
                        let is_open = match state_value {
                            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) > 0.0,
                            serde_json::Value::Bool(b) => *b,
                            _ => false,
                        };

                        if is_open {
                            open_count += 1;
                        } else {
                            closed_count += 1;
                        }

                        let sensor_info = json!({
                            "uuid": device.uuid,
                            "name": device.name,
                            "room": device.room.as_deref().unwrap_or("Unknown"),
                            "status": if is_open { "OPEN" } else { "CLOSED" },
                            "value": state_value,
                        });

                        sensors.push(sensor_info);
                    }
                }
                Err(_) => {
                    let sensor_info = json!({
                        "uuid": device.uuid,
                        "name": device.name,
                        "room": device.room.as_deref().unwrap_or("Unknown"),
                        "status": "UNKNOWN",
                        "error": "Failed to fetch state",
                    });

                    sensors.push(sensor_info);
                }
            }
        }

        json!({
            "sensors": sensors,
            "summary": {
                "total_sensors": sensors.len(),
                "open": open_count,
                "closed": closed_count,
                "all_closed": open_count == 0,
            }
        })
    };

    ToolResponse::success(sensors)
}
