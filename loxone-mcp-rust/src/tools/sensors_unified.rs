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

/// Get all air quality sensor readings using unified value resolution
pub async fn get_air_quality_sensors_unified(context: ToolContext) -> ToolResponse {
    // Ensure we're connected
    if let Err(e) = context.ensure_connected().await {
        return ToolResponse::error(format!("Connection error: {}", e));
    }

    // Get all devices using context helper
    let all_devices = match context.get_devices(None).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(format!("Failed to get devices: {}", e)),
    };

    // Filter for air quality sensors
    let air_quality_sensors: Vec<_> = all_devices
        .into_iter()
        .filter(|device| {
            let name_lower = device.name.to_lowercase();
            let type_lower = device.device_type.to_lowercase();
            
            // Air quality sensor patterns
            name_lower.contains("air quality")
                || name_lower.contains("luftqualität")
                || name_lower.contains("co2")
                || name_lower.contains("voc")
                || name_lower.contains("pm2.5")
                || name_lower.contains("pm10")
                || name_lower.contains("humidity")
                || name_lower.contains("feuchtigkeit")
                || name_lower.contains("luftfeuchtigkeit")
                || type_lower.contains("airquality")
                || type_lower.contains("co2")
                || type_lower.contains("voc")
                || type_lower.contains("humidity")
                || (device.device_type == "InfoOnlyAnalog" && (
                    name_lower.contains("air") || 
                    name_lower.contains("co2") ||
                    name_lower.contains("humidity")
                ))
        })
        .collect();

    if air_quality_sensors.is_empty() {
        return ToolResponse::success(json!({
            "sensors": [],
            "summary": {
                "total_sensors": 0,
                "air_quality_status": "No Data",
                "average_co2": null,
                "average_humidity": null,
                "average_voc": null
            }
        }));
    }

    // Use unified value resolver for consistent value parsing
    let resolver = &context.value_resolver;
    let uuids: Vec<String> = air_quality_sensors.iter().map(|d| d.uuid.clone()).collect();
    
    // Batch resolve all sensor values efficiently
    let resolved_values = match resolver.resolve_batch_values(&uuids).await {
        Ok(values) => values,
        Err(e) => return ToolResponse::error(format!("Failed to resolve sensor values: {}", e)),
    };

    let mut sensor_data = Vec::new();
    let mut co2_total = 0.0;
    let mut co2_count = 0;
    let mut humidity_total = 0.0;
    let mut humidity_count = 0;
    let mut voc_total = 0.0;
    let mut voc_count = 0;
    let mut worst_air_quality = "Good";

    for device in &air_quality_sensors {
        if let Some(resolved) = resolved_values.get(&device.uuid) {
            let mut sensor_json = create_sensor_json_from_resolved(device, resolved);
            
            // Determine sensor type and calculate averages
            if let Some(unit) = &resolved.unit {
                match unit.as_str() {
                    "ppm" => {
                        if let Some(value) = resolved.numeric_value {
                            if device.name.to_lowercase().contains("co2") {
                                co2_total += value;
                                co2_count += 1;
                                sensor_json["sensor_type"] = json!("co2");
                                sensor_json["co2_ppm"] = json!(value);
                                
                                // Assess CO2 level
                                let quality = if value < 800.0 {
                                    "Good"
                                } else if value < 1000.0 {
                                    "Moderate"
                                } else if value < 1400.0 {
                                    "Poor"
                                } else {
                                    "Very Poor"
                                };
                                sensor_json["air_quality"] = json!(quality);
                                
                                // Update worst quality
                                if quality == "Very Poor" || (worst_air_quality != "Very Poor" && quality == "Poor") || (worst_air_quality == "Good" && quality == "Moderate") {
                                    worst_air_quality = quality;
                                }
                            } else if device.name.to_lowercase().contains("voc") {
                                voc_total += value;
                                voc_count += 1;
                                sensor_json["sensor_type"] = json!("voc");
                                sensor_json["voc_ppm"] = json!(value);
                            }
                        }
                    }
                    "%" => {
                        if let Some(value) = resolved.numeric_value {
                            if device.name.to_lowercase().contains("humid") || device.name.to_lowercase().contains("feucht") {
                                humidity_total += value;
                                humidity_count += 1;
                                sensor_json["sensor_type"] = json!("humidity");
                                sensor_json["humidity_percent"] = json!(value);
                                
                                // Assess humidity level
                                let comfort = if (30.0..=60.0).contains(&value) {
                                    "Comfortable"
                                } else if !(20.0..=70.0).contains(&value) {
                                    "Uncomfortable"
                                } else {
                                    "Acceptable"
                                };
                                sensor_json["comfort_level"] = json!(comfort);
                            }
                        }
                    }
                    "μg/m³" | "ug/m3" => {
                        if let Some(value) = resolved.numeric_value {
                            if device.name.to_lowercase().contains("pm2.5") || device.name.to_lowercase().contains("pm25") {
                                sensor_json["sensor_type"] = json!("pm2.5");
                                sensor_json["pm25_ugm3"] = json!(value);
                                
                                // WHO guidelines for PM2.5
                                let quality = if value < 15.0 {
                                    "Good"
                                } else if value < 35.0 {
                                    "Moderate"
                                } else if value < 55.0 {
                                    "Poor"
                                } else {
                                    "Very Poor"
                                };
                                sensor_json["air_quality"] = json!(quality);
                            } else if device.name.to_lowercase().contains("pm10") {
                                sensor_json["sensor_type"] = json!("pm10");
                                sensor_json["pm10_ugm3"] = json!(value);
                            }
                        }
                    }
                    _ => {
                        sensor_json["sensor_type"] = json!("unknown");
                    }
                }
            } else {
                // Try to infer from name if no unit
                let name_lower = device.name.to_lowercase();
                if name_lower.contains("co2") {
                    sensor_json["sensor_type"] = json!("co2");
                } else if name_lower.contains("humid") || name_lower.contains("feucht") {
                    sensor_json["sensor_type"] = json!("humidity");
                } else if name_lower.contains("voc") {
                    sensor_json["sensor_type"] = json!("voc");
                } else if name_lower.contains("pm") {
                    sensor_json["sensor_type"] = json!("particulate");
                } else {
                    sensor_json["sensor_type"] = json!("air_quality");
                }
            }
            
            sensor_data.push(sensor_json);
        } else {
            // Device found but no resolved value
            sensor_data.push(json!({
                "uuid": device.uuid,
                "name": device.name,
                "room": device.room.as_deref().unwrap_or("Unknown"),
                "value": null,
                "sensor_type": "unknown",
                "status": "No Data",
                "confidence": 0.0,
                "source": "Missing"
            }));
        }
    }

    let average_co2 = if co2_count > 0 {
        Some(co2_total / co2_count as f64)
    } else {
        None
    };

    let average_humidity = if humidity_count > 0 {
        Some(humidity_total / humidity_count as f64)
    } else {
        None
    };

    let average_voc = if voc_count > 0 {
        Some(voc_total / voc_count as f64)
    } else {
        None
    };

    ToolResponse::success(json!({
        "sensors": sensor_data,
        "summary": {
            "total_sensors": sensor_data.len(),
            "air_quality_status": worst_air_quality,
            "average_co2": average_co2,
            "average_humidity": average_humidity,
            "average_voc": average_voc,
            "co2_sensors": co2_count,
            "humidity_sensors": humidity_count,
            "voc_sensors": voc_count
        }
    }))
}

/// Get all presence detector readings using unified value resolution
pub async fn get_presence_detectors_unified(context: ToolContext) -> ToolResponse {
    // Ensure we're connected
    if let Err(e) = context.ensure_connected().await {
        return ToolResponse::error(format!("Connection error: {}", e));
    }

    // Get all devices using context helper
    let all_devices = match context.get_devices(None).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(format!("Failed to get devices: {}", e)),
    };

    // Filter for presence detectors
    let presence_detectors: Vec<_> = all_devices
        .into_iter()
        .filter(|device| {
            let name_lower = device.name.to_lowercase();
            let type_lower = device.device_type.to_lowercase();
            
            // Presence detector patterns
            name_lower.contains("presence")
                || name_lower.contains("präsenz")
                || name_lower.contains("anwesenheit")
                || name_lower.contains("occupancy")
                || name_lower.contains("belegung")
                || name_lower.contains("person")
                || name_lower.contains("people")
                || name_lower.contains("radar")
                || name_lower.contains("microwave")
                || name_lower.contains("ultrasonic")
                || name_lower.contains("ultraschall")
                || type_lower.contains("presence")
                || type_lower.contains("occupancy")
                || type_lower.contains("radar")
                || type_lower.contains("ultrasonic")
                || (type_lower.contains("digital") && (
                    name_lower.contains("presence") || 
                    name_lower.contains("occupancy") ||
                    name_lower.contains("person")
                ))
        })
        .collect();

    if presence_detectors.is_empty() {
        return ToolResponse::success(json!({
            "detectors": [],
            "summary": {
                "total_detectors": 0,
                "occupied_rooms": 0,
                "vacant_rooms": 0,
                "presence_detected": false,
                "occupancy_rate": 0.0
            }
        }));
    }

    // Use unified value resolver for consistent value parsing
    let resolver = &context.value_resolver;
    let uuids: Vec<String> = presence_detectors.iter().map(|d| d.uuid.clone()).collect();
    
    // Batch resolve all detector values efficiently
    let resolved_values = match resolver.resolve_batch_values(&uuids).await {
        Ok(values) => values,
        Err(e) => return ToolResponse::error(format!("Failed to resolve detector values: {}", e)),
    };

    let mut detector_data = Vec::new();
    let mut occupied_count = 0;
    let mut vacant_count = 0;
    let mut room_occupancy = std::collections::HashMap::new();

    for device in &presence_detectors {
        if let Some(resolved) = resolved_values.get(&device.uuid) {
            // Check if presence is detected based on resolved value
            let presence_detected = if let Some(numeric) = resolved.numeric_value {
                numeric > 0.0
            } else {
                // Check formatted value for presence status
                let formatted = resolved.formatted_value.to_lowercase();
                formatted.contains("present") || formatted.contains("occupied") || 
                formatted.contains("detected") || formatted.contains("on") || 
                formatted.contains("1") || formatted.contains("true") ||
                formatted.contains("anwesend") || formatted.contains("belegt")
            };

            if presence_detected {
                occupied_count += 1;
            } else {
                vacant_count += 1;
            }

            // Track room-level occupancy
            let room_name = device.room.as_deref().unwrap_or("Unknown");
            let current_occupancy = room_occupancy.get(room_name).unwrap_or(&false);
            room_occupancy.insert(room_name, *current_occupancy || presence_detected);

            let mut detector_json = create_sensor_json_from_resolved(device, resolved);
            
            // Add presence-specific fields
            detector_json["presence_detected"] = json!(presence_detected);
            detector_json["occupancy_status"] = json!(if presence_detected { "Occupied" } else { "Vacant" });
            detector_json["detector_type"] = json!(if device.name.to_lowercase().contains("radar") {
                "radar"
            } else if device.name.to_lowercase().contains("ultrasonic") || device.name.to_lowercase().contains("ultraschall") {
                "ultrasonic"
            } else if device.name.to_lowercase().contains("microwave") {
                "microwave"
            } else if device.name.to_lowercase().contains("pir") {
                "pir"
            } else {
                "presence"
            });
            
            // Add confidence level based on detector type and value
            if let Some(numeric) = resolved.numeric_value {
                let confidence = if numeric >= 0.8 {
                    "High"
                } else if numeric >= 0.5 {
                    "Medium"
                } else if numeric > 0.0 {
                    "Low"
                } else {
                    "None"
                };
                detector_json["detection_confidence"] = json!(confidence);
            }
            
            detector_data.push(detector_json);
        } else {
            // Device found but no resolved value
            detector_data.push(json!({
                "uuid": device.uuid,
                "name": device.name,
                "room": device.room.as_deref().unwrap_or("Unknown"),
                "presence_detected": false,
                "occupancy_status": "Unknown",
                "detector_type": "unknown",
                "status": "No Data",
                "confidence": 0.0,
                "source": "Missing"
            }));
        }
    }

    // Calculate room-level statistics
    let occupied_rooms = room_occupancy.values().filter(|&&occupied| occupied).count();
    let total_rooms = room_occupancy.len();
    let occupancy_rate = if total_rooms > 0 {
        occupied_rooms as f64 / total_rooms as f64
    } else {
        0.0
    };

    // Create room occupancy summary
    let room_summary: Vec<_> = room_occupancy
        .iter()
        .map(|(room, &occupied)| json!({
            "room": room,
            "occupied": occupied,
            "status": if occupied { "Occupied" } else { "Vacant" }
        }))
        .collect();

    ToolResponse::success(json!({
        "detectors": detector_data,
        "room_occupancy": room_summary,
        "summary": {
            "total_detectors": detector_data.len(),
            "occupied_detectors": occupied_count,
            "vacant_detectors": vacant_count,
            "occupied_rooms": occupied_rooms,
            "total_rooms": total_rooms,
            "presence_detected": occupied_count > 0,
            "occupancy_rate": (occupancy_rate * 100.0).round() / 100.0,
            "overall_status": if occupied_count > 0 { "Activity Detected" } else { "No Activity" }
        }
    }))
}

/// Get all weather station sensor readings using unified value resolution
pub async fn get_weather_station_sensors_unified(context: ToolContext) -> ToolResponse {
    // Ensure we're connected
    if let Err(e) = context.ensure_connected().await {
        return ToolResponse::error(format!("Connection error: {}", e));
    }

    // Get all devices using context helper
    let all_devices = match context.get_devices(None).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(format!("Failed to get devices: {}", e)),
    };

    // Filter for weather station sensors
    let weather_sensors: Vec<_> = all_devices
        .into_iter()
        .filter(|device| {
            let name_lower = device.name.to_lowercase();
            let type_lower = device.device_type.to_lowercase();
            
            // Weather station patterns
            name_lower.contains("weather")
                || name_lower.contains("wetter")
                || name_lower.contains("wind")
                || name_lower.contains("rain")
                || name_lower.contains("regen")
                || name_lower.contains("humidity")
                || name_lower.contains("feuchtigkeit")
                || name_lower.contains("pressure")
                || name_lower.contains("druck")
                || name_lower.contains("barometric")
                || name_lower.contains("solar")
                || name_lower.contains("uv")
                || name_lower.contains("brightness")
                || name_lower.contains("helligkeit")
                || name_lower.contains("lux")
                || name_lower.contains("outdoor")
                || name_lower.contains("außen")
                || name_lower.contains("external")
                || type_lower.contains("weather")
                || type_lower.contains("windspeed")
                || type_lower.contains("rainfall")
                || (device.device_type == "InfoOnlyAnalog" && (
                    name_lower.contains("wind") || 
                    name_lower.contains("rain") ||
                    name_lower.contains("pressure") ||
                    name_lower.contains("outdoor") ||
                    name_lower.contains("solar")
                ))
        })
        .collect();

    if weather_sensors.is_empty() {
        return ToolResponse::success(json!({
            "sensors": [],
            "summary": {
                "total_sensors": 0,
                "weather_status": "No Data",
                "outdoor_temperature": null,
                "wind_speed": null,
                "rainfall": null,
                "atmospheric_pressure": null,
                "humidity": null,
                "solar_radiation": null
            }
        }));
    }

    // Use unified value resolver for consistent value parsing
    let resolver = &context.value_resolver;
    let uuids: Vec<String> = weather_sensors.iter().map(|d| d.uuid.clone()).collect();
    
    // Batch resolve all sensor values efficiently
    let resolved_values = match resolver.resolve_batch_values(&uuids).await {
        Ok(values) => values,
        Err(e) => return ToolResponse::error(format!("Failed to resolve sensor values: {}", e)),
    };

    let mut sensor_data = Vec::new();
    let mut outdoor_temp: Option<f64> = None;
    let mut wind_speed: Option<f64> = None;
    let mut rainfall: Option<f64> = None;
    let mut pressure: Option<f64> = None;
    let mut humidity: Option<f64> = None;
    let mut solar_radiation: Option<f64> = None;
    let mut uv_index: Option<f64> = None;
    let mut brightness: Option<f64> = None;

    for device in &weather_sensors {
        if let Some(resolved) = resolved_values.get(&device.uuid) {
            let mut sensor_json = create_sensor_json_from_resolved(device, resolved);
            
            // Determine sensor type based on name and unit
            let name_lower = device.name.to_lowercase();
            let sensor_type = if name_lower.contains("temp") && (name_lower.contains("outdoor") || name_lower.contains("außen")) {
                "outdoor_temperature"
            } else if name_lower.contains("wind") {
                "wind_speed"
            } else if name_lower.contains("rain") || name_lower.contains("regen") {
                "rainfall"
            } else if name_lower.contains("pressure") || name_lower.contains("druck") {
                "atmospheric_pressure"
            } else if name_lower.contains("humid") && (name_lower.contains("outdoor") || name_lower.contains("außen")) {
                "outdoor_humidity"
            } else if name_lower.contains("solar") {
                "solar_radiation"
            } else if name_lower.contains("uv") {
                "uv_index"
            } else if name_lower.contains("brightness") || name_lower.contains("helligkeit") || name_lower.contains("lux") {
                "brightness"
            } else {
                "weather_sensor"
            };

            sensor_json["sensor_type"] = json!(sensor_type);

            // Extract values for summary based on type and unit
            if let Some(value) = resolved.numeric_value {
                match sensor_type {
                    "outdoor_temperature" => {
                        outdoor_temp = Some(value);
                        sensor_json["temperature_celsius"] = json!(value);
                    }
                    "wind_speed" => {
                        wind_speed = Some(value);
                        sensor_json["wind_speed_ms"] = json!(value);
                        
                        // Add wind condition assessment
                        let condition = if value < 1.0 {
                            "Calm"
                        } else if value < 5.0 {
                            "Light breeze"
                        } else if value < 10.0 {
                            "Moderate breeze"
                        } else if value < 15.0 {
                            "Strong breeze"
                        } else {
                            "High wind"
                        };
                        sensor_json["wind_condition"] = json!(condition);
                    }
                    "rainfall" => {
                        rainfall = Some(value);
                        sensor_json["rainfall_mm"] = json!(value);
                        
                        // Add rain intensity
                        let intensity = if value == 0.0 {
                            "No rain"
                        } else if value < 2.0 {
                            "Light rain"
                        } else if value < 10.0 {
                            "Moderate rain"
                        } else {
                            "Heavy rain"
                        };
                        sensor_json["rain_intensity"] = json!(intensity);
                    }
                    "atmospheric_pressure" => {
                        pressure = Some(value);
                        sensor_json["pressure_hpa"] = json!(value);
                        
                        // Add pressure trend indication
                        let trend = if value > 1020.0 {
                            "High pressure"
                        } else if value > 1000.0 {
                            "Normal pressure"
                        } else {
                            "Low pressure"
                        };
                        sensor_json["pressure_trend"] = json!(trend);
                    }
                    "outdoor_humidity" => {
                        humidity = Some(value);
                        sensor_json["humidity_percent"] = json!(value);
                    }
                    "solar_radiation" => {
                        solar_radiation = Some(value);
                        sensor_json["solar_radiation_wm2"] = json!(value);
                    }
                    "uv_index" => {
                        uv_index = Some(value);
                        sensor_json["uv_index"] = json!(value);
                        
                        // Add UV risk level
                        let risk = if value < 3.0 {
                            "Low"
                        } else if value < 6.0 {
                            "Moderate"
                        } else if value < 8.0 {
                            "High"
                        } else if value < 11.0 {
                            "Very High"
                        } else {
                            "Extreme"
                        };
                        sensor_json["uv_risk"] = json!(risk);
                    }
                    "brightness" => {
                        brightness = Some(value);
                        sensor_json["brightness_lux"] = json!(value);
                    }
                    _ => {}
                }
            }
            
            sensor_data.push(sensor_json);
        } else {
            // Device found but no resolved value
            sensor_data.push(json!({
                "uuid": device.uuid,
                "name": device.name,
                "room": device.room.as_deref().unwrap_or("Outdoor"),
                "value": null,
                "sensor_type": "unknown",
                "status": "No Data",
                "confidence": 0.0,
                "source": "Missing"
            }));
        }
    }

    // Determine overall weather status
    let weather_status = if outdoor_temp.is_some() || wind_speed.is_some() || rainfall.is_some() {
        let temp_status = outdoor_temp.map(|t| {
            if t < 0.0 { "Freezing" }
            else if t < 10.0 { "Cold" }
            else if t < 20.0 { "Cool" }
            else if t < 30.0 { "Warm" }
            else { "Hot" }
        }).unwrap_or("Unknown");

        let rain_status = rainfall.map(|r| {
            if r > 0.0 { "Rainy" } else { "Dry" }
        }).unwrap_or("Unknown");

        let wind_status = wind_speed.map(|w| {
            if w > 10.0 { "Windy" } else { "Calm" }
        }).unwrap_or("Unknown");

        format!("{}, {}, {}", temp_status, rain_status, wind_status)
    } else {
        "No Data".to_string()
    };

    ToolResponse::success(json!({
        "sensors": sensor_data,
        "summary": {
            "total_sensors": sensor_data.len(),
            "weather_status": weather_status,
            "outdoor_temperature": outdoor_temp,
            "wind_speed": wind_speed,
            "rainfall": rainfall,
            "atmospheric_pressure": pressure,
            "humidity": humidity,
            "solar_radiation": solar_radiation,
            "uv_index": uv_index,
            "brightness": brightness
        }
    }))
}

/// Intelligently discover and classify all sensors using behavioral analysis
pub async fn discover_sensor_types_unified(context: ToolContext) -> ToolResponse {
    // Ensure we're connected
    if let Err(e) = context.ensure_connected().await {
        return ToolResponse::error(format!("Connection error: {}", e));
    }

    // Get all devices using context helper
    let all_devices = match context.get_devices(None).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(format!("Failed to get devices: {}", e)),
    };

    // Filter for potential sensor devices
    let potential_sensors: Vec<_> = all_devices
        .into_iter()
        .filter(|device| {
            let name_lower = device.name.to_lowercase();
            let type_lower = device.device_type.to_lowercase();
            
            // Include devices that might be sensors based on type or name patterns
            type_lower.contains("analog") ||
            type_lower.contains("digital") ||
            type_lower.contains("sensor") ||
            type_lower.contains("temp") ||
            type_lower.contains("gate") ||
            type_lower.contains("contact") ||
            type_lower.contains("motion") ||
            type_lower.contains("presence") ||
            type_lower.contains("weather") ||
            name_lower.contains("sensor") ||
            name_lower.contains("temp") ||
            name_lower.contains("humid") ||
            name_lower.contains("pressure") ||
            name_lower.contains("wind") ||
            name_lower.contains("rain") ||
            name_lower.contains("motion") ||
            name_lower.contains("door") ||
            name_lower.contains("window") ||
            name_lower.contains("presence") ||
            name_lower.contains("co2") ||
            name_lower.contains("air") ||
            name_lower.contains("energy") ||
            name_lower.contains("power") ||
            name_lower.contains("meter")
        })
        .collect();

    if potential_sensors.is_empty() {
        return ToolResponse::success(json!({
            "discovered_sensors": [],
            "classification_summary": {
                "total_analyzed": 0,
                "classified_sensors": 0,
                "unknown_sensors": 0,
                "confidence_distribution": {}
            }
        }));
    }

    // Use unified value resolver for consistent value parsing
    let resolver = &context.value_resolver;
    let uuids: Vec<String> = potential_sensors.iter().map(|d| d.uuid.clone()).collect();
    
    // Batch resolve all sensor values efficiently
    let resolved_values = match resolver.resolve_batch_values(&uuids).await {
        Ok(values) => values,
        Err(e) => return ToolResponse::error(format!("Failed to resolve sensor values: {}", e)),
    };

    let mut discovered_sensors = Vec::new();
    let mut classification_counts = std::collections::HashMap::new();
    let mut confidence_distribution = std::collections::HashMap::new();

    for device in &potential_sensors {
        let mut sensor_classification = json!({
            "uuid": device.uuid,
            "name": device.name,
            "room": device.room.as_deref().unwrap_or("Unknown"),
            "device_type": device.device_type,
            "category": device.category
        });

        if let Some(resolved) = resolved_values.get(&device.uuid) {
            // Behavioral analysis based on value patterns and characteristics
            let (sensor_type, confidence, characteristics) = classify_sensor_behavior(device, resolved);
            
            sensor_classification["detected_sensor_type"] = json!(sensor_type);
            sensor_classification["confidence"] = json!(confidence);
            sensor_classification["behavioral_characteristics"] = json!(characteristics);
            sensor_classification["raw_value"] = json!(resolved.numeric_value);
            sensor_classification["formatted_value"] = json!(resolved.formatted_value);
            sensor_classification["unit"] = json!(resolved.unit);
            
            // Track classification statistics
            *classification_counts.entry(sensor_type.clone()).or_insert(0) += 1;
            let confidence_bucket = match confidence {
                conf if conf >= 0.8 => "high",
                conf if conf >= 0.6 => "medium", 
                conf if conf >= 0.4 => "low",
                _ => "very_low"
            };
            *confidence_distribution.entry(confidence_bucket.to_string()).or_insert(0) += 1;
        } else {
            // No value available - classify based on name/type only
            let (sensor_type, confidence, characteristics) = classify_sensor_by_metadata(device);
            
            sensor_classification["detected_sensor_type"] = json!(sensor_type);
            sensor_classification["confidence"] = json!(confidence);
            sensor_classification["behavioral_characteristics"] = json!(characteristics);
            sensor_classification["raw_value"] = json!(null);
            sensor_classification["status"] = json!("No Data Available");
            
            *classification_counts.entry(sensor_type.clone()).or_insert(0) += 1;
            *confidence_distribution.entry("metadata_only".to_string()).or_insert(0) += 1;
        }
        
        discovered_sensors.push(sensor_classification);
    }

    let classified_sensors = classification_counts.values().sum::<i32>();
    let unknown_sensors = classification_counts.get("unknown").unwrap_or(&0);

    ToolResponse::success(json!({
        "discovered_sensors": discovered_sensors,
        "classification_summary": {
            "total_analyzed": potential_sensors.len(),
            "classified_sensors": classified_sensors,
            "unknown_sensors": unknown_sensors,
            "sensor_type_counts": classification_counts,
            "confidence_distribution": confidence_distribution
        },
        "recommendations": generate_sensor_recommendations(&classification_counts, &discovered_sensors)
    }))
}

/// Classify sensor behavior based on value characteristics and patterns
fn classify_sensor_behavior(device: &crate::client::LoxoneDevice, resolved: &crate::services::ResolvedValue) -> (String, f64, serde_json::Value) {
    let name_lower = device.name.to_lowercase();
    let type_lower = device.device_type.to_lowercase();
    
    let mut characteristics = json!({});
    #[allow(unused_assignments)]
    let mut confidence = 0.5; // Base confidence
    
    // Temperature sensors
    if name_lower.contains("temp") || name_lower.contains("temperatur") {
        if let Some(value) = resolved.numeric_value {
            if value > -40.0 && value < 80.0 {
                confidence = 0.9;
                characteristics["value_range"] = json!("typical_temperature");
                characteristics["likely_unit"] = json!("celsius");
                return ("temperature".to_string(), confidence, characteristics);
            }
        }
        confidence = 0.7;
        return ("temperature".to_string(), confidence, characteristics);
    }
    
    // Door/Window sensors (binary)
    if name_lower.contains("door") || name_lower.contains("window") || name_lower.contains("tür") || name_lower.contains("fenster") || type_lower.contains("gate") {
        if let Some(value) = resolved.numeric_value {
            if value == 0.0 || value == 1.0 {
                confidence = 0.95;
                characteristics["value_type"] = json!("binary");
                characteristics["current_state"] = json!(if value > 0.0 { "open" } else { "closed" });
                return ("door_window_contact".to_string(), confidence, characteristics);
            }
        }
        confidence = 0.8;
        return ("door_window_contact".to_string(), confidence, characteristics);
    }
    
    // Motion sensors (binary or presence detection)
    if name_lower.contains("motion") || name_lower.contains("bewegung") || name_lower.contains("pir") {
        if let Some(value) = resolved.numeric_value {
            if value == 0.0 || value == 1.0 {
                confidence = 0.9;
                characteristics["value_type"] = json!("binary");
                characteristics["current_state"] = json!(if value > 0.0 { "motion_detected" } else { "no_motion" });
                return ("motion_sensor".to_string(), confidence, characteristics);
            }
        }
        confidence = 0.8;
        return ("motion_sensor".to_string(), confidence, characteristics);
    }
    
    // Energy/Power meters
    if name_lower.contains("power") || name_lower.contains("energy") || name_lower.contains("watt") || name_lower.contains("kwh") {
        if let Some(value) = resolved.numeric_value {
            if (0.0..100000.0).contains(&value) {
                confidence = 0.85;
                characteristics["value_range"] = json!("typical_power_consumption");
                if let Some(unit) = &resolved.unit {
                    characteristics["unit"] = json!(unit);
                    if unit.contains("W") || unit.contains("kW") {
                        characteristics["meter_type"] = json!("power");
                    } else if unit.contains("Wh") || unit.contains("kWh") {
                        characteristics["meter_type"] = json!("energy");
                    }
                }
                return ("energy_meter".to_string(), confidence, characteristics);
            }
        }
        confidence = 0.7;
        return ("energy_meter".to_string(), confidence, characteristics);
    }
    
    // Humidity sensors
    if name_lower.contains("humid") || name_lower.contains("feucht") {
        if let Some(value) = resolved.numeric_value {
            if (0.0..=100.0).contains(&value) {
                confidence = 0.9;
                characteristics["value_range"] = json!("percentage");
                characteristics["likely_unit"] = json!("percent");
                return ("humidity_sensor".to_string(), confidence, characteristics);
            }
        }
        confidence = 0.7;
        return ("humidity_sensor".to_string(), confidence, characteristics);
    }
    
    // CO2 sensors
    if name_lower.contains("co2") {
        if let Some(value) = resolved.numeric_value {
            if (300.0..=5000.0).contains(&value) {
                confidence = 0.9;
                characteristics["value_range"] = json!("typical_co2_ppm");
                characteristics["likely_unit"] = json!("ppm");
                return ("co2_sensor".to_string(), confidence, characteristics);
            }
        }
        confidence = 0.8;
        return ("co2_sensor".to_string(), confidence, characteristics);
    }
    
    // Pressure sensors
    if name_lower.contains("pressure") || name_lower.contains("druck") {
        if let Some(value) = resolved.numeric_value {
            if (900.0..=1100.0).contains(&value) {
                confidence = 0.9;
                characteristics["value_range"] = json!("atmospheric_pressure_hpa");
                characteristics["likely_unit"] = json!("hPa");
                return ("pressure_sensor".to_string(), confidence, characteristics);
            }
        }
        confidence = 0.7;
        return ("pressure_sensor".to_string(), confidence, characteristics);
    }
    
    // Wind sensors
    if name_lower.contains("wind") {
        if let Some(value) = resolved.numeric_value {
            if (0.0..=50.0).contains(&value) {
                confidence = 0.85;
                characteristics["value_range"] = json!("wind_speed_ms");
                characteristics["likely_unit"] = json!("m/s");
                return ("wind_sensor".to_string(), confidence, characteristics);
            }
        }
        confidence = 0.7;
        return ("wind_sensor".to_string(), confidence, characteristics);
    }
    
    // Brightness/Light sensors
    if name_lower.contains("brightness") || name_lower.contains("light") || name_lower.contains("lux") {
        if let Some(value) = resolved.numeric_value {
            if (0.0..=100000.0).contains(&value) {
                confidence = 0.8;
                characteristics["value_range"] = json!("brightness_lux");
                characteristics["likely_unit"] = json!("lux");
                return ("brightness_sensor".to_string(), confidence, characteristics);
            }
        }
        confidence = 0.6;
        return ("brightness_sensor".to_string(), confidence, characteristics);
    }
    
    // Generic analog classification based on value patterns
    if type_lower.contains("analog") || type_lower.contains("infoonly") {
        if let Some(value) = resolved.numeric_value {
            characteristics["numeric_value"] = json!(value);
            
            // Binary-like values
            if value == 0.0 || value == 1.0 {
                confidence = 0.6;
                characteristics["value_pattern"] = json!("binary");
                return ("binary_sensor".to_string(), confidence, characteristics);
            }
            
            // Percentage-like values
            if (0.0..=100.0).contains(&value) && value.fract() != 0.0 {
                confidence = 0.5;
                characteristics["value_pattern"] = json!("percentage_like");
                return ("analog_sensor".to_string(), confidence, characteristics);
            }
            
            // Large integer values (could be counters)
            if value > 1000.0 && value.fract() == 0.0 {
                confidence = 0.4;
                characteristics["value_pattern"] = json!("counter_like");
                return ("counter_sensor".to_string(), confidence, characteristics);
            }
        }
        
        confidence = 0.3;
        characteristics["classification"] = json!("generic_analog");
        return ("analog_sensor".to_string(), confidence, characteristics);
    }
    
    // Digital classification
    if type_lower.contains("digital") {
        confidence = 0.4;
        characteristics["classification"] = json!("generic_digital");
        return ("digital_sensor".to_string(), confidence, characteristics);
    }
    
    // Fallback classification
    confidence = 0.1;
    characteristics["classification"] = json!("unclassified");
    ("unknown".to_string(), confidence, characteristics)
}

/// Classify sensor based on metadata only (when no value is available)
fn classify_sensor_by_metadata(device: &crate::client::LoxoneDevice) -> (String, f64, serde_json::Value) {
    let name_lower = device.name.to_lowercase();
    let type_lower = device.device_type.to_lowercase();
    
    let characteristics = json!({
        "classification_method": "metadata_only",
        "device_type": device.device_type,
        "category": device.category
    });
    
    // High confidence name-based classification
    if name_lower.contains("temp") { return ("temperature".to_string(), 0.7, characteristics); }
    if name_lower.contains("door") || name_lower.contains("window") { return ("door_window_contact".to_string(), 0.7, characteristics); }
    if name_lower.contains("motion") || name_lower.contains("bewegung") { return ("motion_sensor".to_string(), 0.7, characteristics); }
    if name_lower.contains("humid") { return ("humidity_sensor".to_string(), 0.7, characteristics); }
    if name_lower.contains("co2") { return ("co2_sensor".to_string(), 0.7, characteristics); }
    if name_lower.contains("pressure") { return ("pressure_sensor".to_string(), 0.6, characteristics); }
    if name_lower.contains("power") || name_lower.contains("energy") { return ("energy_meter".to_string(), 0.6, characteristics); }
    if name_lower.contains("wind") { return ("wind_sensor".to_string(), 0.6, characteristics); }
    if name_lower.contains("rain") { return ("rain_sensor".to_string(), 0.6, characteristics); }
    
    // Type-based classification
    if type_lower.contains("gate") { return ("door_window_contact".to_string(), 0.6, characteristics); }
    if type_lower.contains("analog") { return ("analog_sensor".to_string(), 0.3, characteristics); }
    if type_lower.contains("digital") { return ("digital_sensor".to_string(), 0.3, characteristics); }
    
    ("unknown".to_string(), 0.1, characteristics)
}

/// Generate recommendations for improving sensor classification
fn generate_sensor_recommendations(classification_counts: &std::collections::HashMap<String, i32>, discovered_sensors: &[serde_json::Value]) -> serde_json::Value {
    let mut recommendations = Vec::new();
    
    let unknown_count = classification_counts.get("unknown").unwrap_or(&0);
    let total_sensors = classification_counts.values().sum::<i32>();
    
    if *unknown_count > 0 {
        recommendations.push(json!({
            "type": "improve_naming",
            "message": format!("{} sensors could not be classified. Consider using descriptive names like 'Living Room Temperature' or 'Front Door Contact'.", unknown_count),
            "priority": "medium"
        }));
    }
    
    // Check for sensors with low confidence
    let low_confidence_sensors: Vec<_> = discovered_sensors
        .iter()
        .filter(|sensor| {
            sensor.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0) < 0.5
        })
        .collect();
    
    if !low_confidence_sensors.is_empty() {
        recommendations.push(json!({
            "type": "verify_classification",
            "message": format!("{} sensors have low classification confidence. Manual verification recommended.", low_confidence_sensors.len()),
            "priority": "low"
        }));
    }
    
    // Suggest sensor groups for better organization
    let sensor_types: Vec<String> = classification_counts.keys().cloned().collect();
    if sensor_types.len() > 5 {
        recommendations.push(json!({
            "type": "organize_sensors",
            "message": format!("Detected {} different sensor types. Consider organizing them into logical groups or rooms for better management.", sensor_types.len()),
            "priority": "low"
        }));
    }
    
    json!({
        "recommendations": recommendations,
        "classification_accuracy": {
            "total_sensors": total_sensors,
            "classified_sensors": total_sensors - unknown_count,
            "accuracy_percentage": if total_sensors > 0 { 
                ((total_sensors - unknown_count) as f64 / total_sensors as f64 * 100.0).round() 
            } else { 
                0.0 
            }
        }
    })
}