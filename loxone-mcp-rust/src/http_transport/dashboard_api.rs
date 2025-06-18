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
    // TODO: Fix when sensor_readings is available in ClientContext
    // let sensor_readings = context.sensor_readings.read().await;
    let sensor_readings: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();

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
        //     if sensor.sensor_type == crate::tools::sensors::SensorType::Temperature {
        //         current_temp = Some(sensor.value);
        //     } else if sensor.sensor_type == crate::tools::sensors::SensorType::Humidity {
        //         current_humidity = Some(sensor.value);
        //     }
        // }

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
pub async fn get_sensors_json(State(_context): State<Arc<ClientContext>>) -> Json<Value> {
    // TODO: Fix when sensor_readings is available in ClientContext
    // let sensor_readings = context.sensor_readings.read().await;
    let sensor_readings: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();

    let sensor_data: Vec<_> = sensor_readings
        .keys()
        .map(|uuid| {
            json!({
                "uuid": uuid,
                "name": "Unknown", // reading.name,
                "type": "unknown", // reading.sensor_type,
                "value": 0.0, // reading.value,
                "unit": "unknown", // reading.unit,
                "location": "unknown", // reading.location,
                "timestamp": chrono::Utc::now(), // reading.timestamp,
                "status": "unknown", // reading.status,
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
    // TODO: Fix when sensor_readings is available in ClientContext
    // let sensor_readings = context.sensor_readings.read().await;
    let sensor_readings: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    let capabilities = context.capabilities.read().await;

    // Process rooms with full data
    let mut room_data = Vec::new();
    for (_uuid, room) in rooms.iter() {
        let room_devices: Vec<_> = devices
            .values()
            .filter(|device| device.room.as_ref() == Some(&room.name))
            .collect();

        let room_sensors: Vec<_> = sensor_readings
            .iter()
            .filter(|(_, _reading)| {
                // TODO: Fix when sensor structure is available
                false // reading.location.as_ref() == Some(&room.name) || reading.name.contains(&room.name)
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
                // .filter(|(_, r)| r.status == crate::tools::sensors::SensorStatus::Online)
                .filter(|_| true) // TODO: Fix when SensorStatus is available
                .count(),
        }
    }))
}
