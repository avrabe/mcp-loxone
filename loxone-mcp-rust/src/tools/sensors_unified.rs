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

    // Get all devices using context helper
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
                || device.category == "sensors"
        })
        .collect();

    if temperature_sensors.is_empty() {
        return ToolResponse::success(json!({
            "sensors": [],
            "summary": {
                "total_sensors": 0,
                "valid_readings": 0,
                "average_temperature": null,
            }
        }));
    }

    // Use unified value resolver for consistent value parsing
    let resolver = &context.value_resolver;
    let uuids: Vec<String> = temperature_sensors.iter().map(|d| d.uuid.clone()).collect();
    
    // Batch resolve all sensor values efficiently
    let resolved_values = match resolver.resolve_batch_values(&uuids).await {
        Ok(values) => values,
        Err(e) => return ToolResponse::error(format!("Failed to resolve sensor values: {}", e)),
    };

    let mut sensor_data = Vec::new();
    let mut total_temperature = 0.0;
    let mut valid_readings = 0;

    for device in &temperature_sensors {
        if let Some(resolved) = resolved_values.get(&device.uuid) {
            let sensor_json = create_sensor_json_from_resolved(device, resolved);
            
            // Track temperature for averaging
            if let Some(temp) = resolved.numeric_value {
                total_temperature += temp;
                valid_readings += 1;
            }
            
            sensor_data.push(sensor_json);
        } else {
            // Device found but no resolved value
            sensor_data.push(json!({
                "uuid": device.uuid,
                "name": device.name,
                "room": device.room.as_deref().unwrap_or("Unknown"),
                "temperature": null,
                "status": "No Data",
                "confidence": 0.0,
                "source": "Missing"
            }));
        }
    }

    let average_temperature = if valid_readings > 0 {
        Some(total_temperature / valid_readings as f64)
    } else {
        None
    };

    ToolResponse::success(json!({
        "sensors": sensor_data,
        "summary": {
            "total_sensors": sensor_data.len(),
            "valid_readings": valid_readings,
            "average_temperature": average_temperature,
        }
    }))
}

/// Get all door/window sensor statuses using unified value resolution
pub async fn get_door_window_sensors_unified(context: ToolContext) -> ToolResponse {
    // Ensure we're connected
    if let Err(e) = context.ensure_connected().await {
        return ToolResponse::error(format!("Connection error: {}", e));
    }

    // Get all devices using context helper
    let all_devices = match context.get_devices(None).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(format!("Failed to get devices: {}", e)),
    };

    // Filter for door/window sensors
    let door_window_sensors: Vec<_> = all_devices
        .into_iter()
        .filter(|device| {
            device.device_type.to_lowercase().contains("gate")
                || device.device_type == "Gate"
                || device.name.to_lowercase().contains("door")
                || device.name.to_lowercase().contains("window")
                || device.name.to_lowercase().contains("tür")
                || device.name.to_lowercase().contains("fenster")
                || device.device_type.contains("Contact")
        })
        .collect();

    if door_window_sensors.is_empty() {
        return ToolResponse::success(json!({
            "sensors": [],
            "summary": {
                "total": 0,
                "open": 0,
                "closed": 0,
                "all_closed": true,
            }
        }));
    }

    // Use unified value resolver for consistent value parsing
    let resolver = &context.value_resolver;
    let uuids: Vec<String> = door_window_sensors.iter().map(|d| d.uuid.clone()).collect();
    
    // Batch resolve all sensor values efficiently
    let resolved_values = match resolver.resolve_batch_values(&uuids).await {
        Ok(values) => values,
        Err(e) => return ToolResponse::error(format!("Failed to resolve sensor values: {}", e)),
    };

    let mut sensors = Vec::new();
    let mut open_count = 0;
    let mut closed_count = 0;

    for device in &door_window_sensors {
        if let Some(resolved) = resolved_values.get(&device.uuid) {
            // Check if door/window is open based on resolved value
            let is_open = if let Some(numeric) = resolved.numeric_value {
                numeric > 0.0
            } else {
                // Check formatted value for open/closed status
                let formatted = resolved.formatted_value.to_lowercase();
                formatted.contains("open") || formatted.contains("on") || formatted.contains("1")
            };

            if is_open {
                open_count += 1;
            } else {
                closed_count += 1;
            }

            let mut sensor_json = create_sensor_json_from_resolved(device, resolved);
            // Add door-specific status
            sensor_json["status"] = json!(if is_open { "OPEN" } else { "CLOSED" });
            
            sensors.push(sensor_json);
        } else {
            // Device found but no resolved value
            sensors.push(json!({
                "uuid": device.uuid,
                "name": device.name,
                "room": device.room.as_deref().unwrap_or("Unknown"),
                "status": "UNKNOWN",
                "confidence": 0.0,
                "source": "Missing"
            }));
        }
    }

    ToolResponse::success(json!({
        "sensors": sensors,
        "summary": {
            "total": sensors.len(),
            "open": open_count,
            "closed": closed_count,
            "all_closed": open_count == 0,
        }
    }))
}