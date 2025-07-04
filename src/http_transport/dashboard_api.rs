//! Dashboard API endpoints for direct data access
//!
//! This module provides HTTP endpoints that return Loxone data in JSON format
//! suitable for dashboard consumption.

use crate::client::ClientContext;
use axum::{extract::State, response::Json};
use serde_json::{json, Value};
use std::sync::Arc;

/// Get all rooms with device counts and sensor data
pub async fn get_rooms_json(State(context): State<Arc<ClientContext>>) -> Json<Value> {
    let rooms = context.rooms.read().await;
    let devices = context.devices.read().await;
    // Get sensor data from devices that have sensor capabilities
    let mut sensor_readings: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    
    // Extract sensor data from device states
    for device in devices.values() {
        if device.device_type.contains("Sensor") || device.device_type.contains("Weather") {
            sensor_readings.insert(device.uuid.clone(), serde_json::json!({
                "name": device.name,
                "type": device.device_type,
                "room": device.room,
                "states": device.states
            }));
        }
    }

    let mut room_data = Vec::new();

    for (_uuid, room) in rooms.iter() {
        // Count devices in this room
        let room_devices: Vec<_> = devices
            .values()
            .filter(|device| device.room.as_ref() == Some(&room.name))
            .collect();

        // Get sensor data for this room
        let room_sensors: Vec<_> = sensor_readings
            .iter()
            .filter(|(_, reading)| {
                reading
                    .get("room")
                    .and_then(|r| r.as_str())
                    .map(|room_name| room_name == room.name)
                    .unwrap_or(false)
            })
            .collect();

        // Extract temperature and humidity if available
        let mut current_temp: Option<f64> = None;
        let mut current_humidity: Option<f64> = None;

        // Extract sensor values from room devices
        for device in &room_devices {
            if device.device_type.contains("Temperature") || device.device_type.contains("Weather") {
                if let Some(temp_value) = device.states.get("value").and_then(|v| v.as_f64()) {
                    current_temp = Some(temp_value);
                }
            }
            if device.device_type.contains("Humidity") || device.device_type.contains("Weather") {
                if let Some(humidity_value) = device.states.get("humidity").and_then(|v| v.as_f64()) {
                    current_humidity = Some(humidity_value);
                }
            }
        }

        // Count active devices (simplified - consider "on" state)
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

        room_data.push(json!({
            "name": room.name,
            "uuid": room.uuid,
            "device_count": room_devices.len(),
            "active_devices": active_count,
            "current_temp": current_temp,
            "current_humidity": current_humidity,
            "sensors": room_sensors.len(),
        }));
    }

    Json(json!({
        "rooms": room_data,
        "total_rooms": room_data.len(),
    }))
}

/// Get all devices with their current states
pub async fn get_devices_json(State(context): State<Arc<ClientContext>>) -> Json<Value> {
    let devices = context.devices.read().await;

    let device_data: Vec<_> = devices
        .values()
        .map(|device| {
            json!({
                "uuid": device.uuid,
                "name": device.name,
                "type": device.device_type,
                "category": device.category,
                "room": device.room,
                "states": device.states,
                // "control_uuid": device.control_uuid, // Not available in current LoxoneDevice
            })
        })
        .collect();

    Json(json!({
        "devices": device_data,
        "total_devices": device_data.len(),
    }))
}

/// Get current sensor readings
pub async fn get_sensors_json(State(context): State<Arc<ClientContext>>) -> Json<Value> {
    let devices = context.devices.read().await;
    
    // Filter devices to get sensor-related ones
    let sensor_data: Vec<_> = devices
        .values()
        .filter(|device| {
            device.device_type.contains("Sensor") || 
            device.device_type.contains("Weather") ||
            device.device_type.contains("Temperature") ||
            device.device_type.contains("Humidity") ||
            device.device_type.contains("Motion") ||
            device.device_type.contains("Light")
        })
        .map(|device| {
            // Extract primary value from device states
            let value = device.states.get("value")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            
            // Determine unit based on device type
            let unit = if device.device_type.contains("Temperature") {
                "°C"
            } else if device.device_type.contains("Humidity") {
                "%"
            } else if device.device_type.contains("Light") {
                "lux"
            } else {
                "unknown"
            };
            
            // Determine status based on connection state
            let status = if device.states.contains_key("active") {
                "online"
            } else {
                "unknown"
            };
            
            json!({
                "uuid": device.uuid,
                "name": device.name,
                "type": device.device_type,
                "value": value,
                "unit": unit,
                "location": device.room,
                "timestamp": chrono::Utc::now(),
                "status": status,
            })
        })
        .collect();

    Json(json!({
        "sensors": sensor_data,
        "total_sensors": sensor_data.len(),
        "active_sensors": sensor_data.iter()
            .filter(|s| s["status"] == "online")
            .count(),
    }))
}

/// Get combined dashboard data
pub async fn get_dashboard_json(State(context): State<Arc<ClientContext>>) -> Json<Value> {
    let rooms = context.rooms.read().await;
    let devices = context.devices.read().await;
    let capabilities = context.capabilities.read().await;
    
    // Extract sensor data from devices
    let sensor_readings: std::collections::HashMap<String, serde_json::Value> = devices
        .values()
        .filter(|device| {
            device.device_type.contains("Sensor") || 
            device.device_type.contains("Weather") ||
            device.device_type.contains("Temperature") ||
            device.device_type.contains("Humidity")
        })
        .map(|device| {
            (device.uuid.clone(), serde_json::json!({
                "name": device.name,
                "type": device.device_type,
                "room": device.room,
                "states": device.states
            }))
        })
        .collect();

    // Process rooms with full data
    let mut room_data = Vec::new();
    for (_uuid, room) in rooms.iter() {
        let room_devices: Vec<_> = devices
            .values()
            .filter(|device| device.room.as_ref() == Some(&room.name))
            .collect();

        let room_sensors: Vec<_> = sensor_readings
            .iter()
            .filter(|(_, reading)| {
                reading
                    .get("room")
                    .and_then(|r| r.as_str())
                    .map(|room_name| room_name == room.name)
                    .unwrap_or(false)
            })
            .collect();

        room_data.push(json!({
            "name": room.name,
            "uuid": room.uuid,
            "devices": room_devices.len(),
            "sensors": room_sensors.len(),
        }));
    }

    Json(json!({
        "connection": {
            "status": if *context.connected.read().await { "Connected" } else { "Disconnected" },
            "last_update": chrono::Utc::now().format("%H:%M:%S").to_string(),
        },
        "capabilities": {
            "lights": capabilities.light_count,
            "blinds": capabilities.blind_count,
            "climate": capabilities.climate_count,
            "sensors": capabilities.sensor_count,
        },
        "rooms": room_data,
        "devices": {
            "total": devices.len(),
            "by_category": {
                "lights": devices.values().filter(|d| d.category == "lights").count(),
                "blinds": devices.values().filter(|d| d.category == "shading").count(),
                "climate": devices.values().filter(|d| d.category == "climate").count(),
                "sensors": devices.values().filter(|d| d.category == "sensors").count(),
            }
        },
        "sensors": {
            "total": sensor_readings.len(),
            "active": sensor_readings.iter()
                .filter(|(_, reading)| {
                    reading
                        .get("states")
                        .and_then(|states| states.get("active"))
                        .and_then(|active| active.as_bool())
                        .unwrap_or(true) // Assume active if no status info
                })
                .count(),
        }
    }))
}
