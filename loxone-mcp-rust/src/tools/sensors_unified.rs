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

/// Get all energy meter readings using unified value resolution
pub async fn get_energy_meters_unified(context: ToolContext) -> ToolResponse {
    // Ensure we're connected
    if let Err(e) = context.ensure_connected().await {
        return ToolResponse::error(format!("Connection error: {}", e));
    }

    // Get all devices using context helper
    let all_devices = match context.get_devices(None).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(format!("Failed to get devices: {}", e)),
    };

    // Filter for energy meters and power sensors
    let energy_meters: Vec<_> = all_devices
        .into_iter()
        .filter(|device| {
            let name_lower = device.name.to_lowercase();
            let type_lower = device.device_type.to_lowercase();
            
            // Energy meter patterns
            type_lower.contains("powermeter")
                || type_lower.contains("energymeter")
                || type_lower.contains("meter")
                || name_lower.contains("power")
                || name_lower.contains("energy")
                || name_lower.contains("energie")
                || name_lower.contains("leistung")
                || name_lower.contains("verbrauch")
                || name_lower.contains("stromzähler")
                || name_lower.contains("kwh")
                || name_lower.contains("watt")
                || device.device_type == "InfoOnlyAnalog" && (
                    name_lower.contains("power") || 
                    name_lower.contains("energy") ||
                    name_lower.contains("consumption")
                )
        })
        .collect();

    if energy_meters.is_empty() {
        return ToolResponse::success(json!({
            "meters": [],
            "summary": {
                "total_meters": 0,
                "total_power": 0.0,
                "total_energy": 0.0,
                "unit_power": "W",
                "unit_energy": "kWh"
            }
        }));
    }

    // Use unified value resolver for consistent value parsing
    let resolver = &context.value_resolver;
    let uuids: Vec<String> = energy_meters.iter().map(|d| d.uuid.clone()).collect();
    
    // Batch resolve all meter values efficiently
    let resolved_values = match resolver.resolve_batch_values(&uuids).await {
        Ok(values) => values,
        Err(e) => return ToolResponse::error(format!("Failed to resolve meter values: {}", e)),
    };

    let mut meter_data = Vec::new();
    let mut total_power = 0.0;
    let mut total_energy = 0.0;
    let mut power_count = 0;
    let mut energy_count = 0;

    for device in &energy_meters {
        if let Some(resolved) = resolved_values.get(&device.uuid) {
            let mut meter_json = create_sensor_json_from_resolved(device, resolved);
            
            // Determine meter type and accumulate totals
            if let Some(unit) = &resolved.unit {
                match unit.as_str() {
                    "W" | "kW" => {
                        if let Some(value) = resolved.numeric_value {
                            let watts = if unit == "kW" { value * 1000.0 } else { value };
                            total_power += watts;
                            power_count += 1;
                            meter_json["meter_type"] = json!("power");
                            meter_json["power_watts"] = json!(watts);
                        }
                    }
                    "kWh" | "Wh" | "MWh" => {
                        if let Some(value) = resolved.numeric_value {
                            let kwh = match unit.as_str() {
                                "Wh" => value / 1000.0,
                                "MWh" => value * 1000.0,
                                _ => value,
                            };
                            total_energy += kwh;
                            energy_count += 1;
                            meter_json["meter_type"] = json!("energy");
                            meter_json["energy_kwh"] = json!(kwh);
                        }
                    }
                    "A" => {
                        meter_json["meter_type"] = json!("current");
                        if let Some(value) = resolved.numeric_value {
                            meter_json["current_amps"] = json!(value);
                        }
                    }
                    "V" => {
                        meter_json["meter_type"] = json!("voltage");
                        if let Some(value) = resolved.numeric_value {
                            meter_json["voltage_volts"] = json!(value);
                        }
                    }
                    _ => {
                        meter_json["meter_type"] = json!("unknown");
                    }
                }
            } else {
                // Try to infer from name if no unit
                let name_lower = device.name.to_lowercase();
                if name_lower.contains("power") || name_lower.contains("leistung") {
                    meter_json["meter_type"] = json!("power");
                } else if name_lower.contains("energy") || name_lower.contains("energie") || name_lower.contains("kwh") {
                    meter_json["meter_type"] = json!("energy");
                } else {
                    meter_json["meter_type"] = json!("unknown");
                }
            }
            
            meter_data.push(meter_json);
        } else {
            // Device found but no resolved value
            meter_data.push(json!({
                "uuid": device.uuid,
                "name": device.name,
                "room": device.room.as_deref().unwrap_or("Unknown"),
                "value": null,
                "meter_type": "unknown",
                "status": "No Data",
                "confidence": 0.0,
                "source": "Missing"
            }));
        }
    }

    ToolResponse::success(json!({
        "meters": meter_data,
        "summary": {
            "total_meters": meter_data.len(),
            "power_meters": power_count,
            "energy_meters": energy_count,
            "total_power": total_power,
            "total_energy": total_energy,
            "unit_power": "W",
            "unit_energy": "kWh"
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

/// Get all motion sensor statuses using unified value resolution
pub async fn get_motion_sensors_unified(context: ToolContext) -> ToolResponse {
    // Ensure we're connected
    if let Err(e) = context.ensure_connected().await {
        return ToolResponse::error(format!("Connection error: {}", e));
    }

    // Get all devices using context helper
    let all_devices = match context.get_devices(None).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(format!("Failed to get devices: {}", e)),
    };

    // Filter for motion sensors
    let motion_sensors: Vec<_> = all_devices
        .into_iter()
        .filter(|device| {
            let name_lower = device.name.to_lowercase();
            let type_lower = device.device_type.to_lowercase();
            
            // Motion sensor patterns
            name_lower.contains("motion")
                || name_lower.contains("bewegung")
                || name_lower.contains("pir")
                || name_lower.contains("bewegungsmelder")
                || name_lower.contains("präsenz")
                || name_lower.contains("presence")
                || type_lower.contains("motion")
                || type_lower.contains("bewegung")
                || type_lower.contains("pir")
                || type_lower.contains("presence")
                || (type_lower.contains("digital") && name_lower.contains("bewegung"))
        })
        .collect();

    if motion_sensors.is_empty() {
        return ToolResponse::success(json!({
            "sensors": [],
            "summary": {
                "total": 0,
                "active": 0,
                "inactive": 0,
                "motion_detected": false,
            }
        }));
    }

    // Use unified value resolver for consistent value parsing
    let resolver = &context.value_resolver;
    let uuids: Vec<String> = motion_sensors.iter().map(|d| d.uuid.clone()).collect();
    
    // Batch resolve all sensor values efficiently
    let resolved_values = match resolver.resolve_batch_values(&uuids).await {
        Ok(values) => values,
        Err(e) => return ToolResponse::error(format!("Failed to resolve sensor values: {}", e)),
    };

    let mut sensors = Vec::new();
    let mut active_count = 0;
    let mut inactive_count = 0;

    for device in &motion_sensors {
        if let Some(resolved) = resolved_values.get(&device.uuid) {
            // Check if motion is detected based on resolved value
            let motion_detected = if let Some(numeric) = resolved.numeric_value {
                numeric > 0.0
            } else {
                // Check formatted value for motion status
                let formatted = resolved.formatted_value.to_lowercase();
                formatted.contains("detected") || formatted.contains("on") || formatted.contains("1") || formatted.contains("active")
            };

            if motion_detected {
                active_count += 1;
            } else {
                inactive_count += 1;
            }

            let mut sensor_json = create_sensor_json_from_resolved(device, resolved);
            // Add motion-specific status
            sensor_json["motion_detected"] = json!(motion_detected);
            sensor_json["status"] = json!(if motion_detected { "Motion Detected" } else { "No Motion" });
            
            sensors.push(sensor_json);
        } else {
            // Device found but no resolved value
            sensors.push(json!({
                "uuid": device.uuid,
                "name": device.name,
                "room": device.room.as_deref().unwrap_or("Unknown"),
                "motion_detected": false,
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
            "active": active_count,
            "inactive": inactive_count,
            "motion_detected": active_count > 0,
        }
    }))
}