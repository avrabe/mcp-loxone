//! Room management MCP tools
//!
//! Tools for listing rooms, getting room devices, and room-based operations.

use crate::tools::{DeviceStats, ToolContext, ToolResponse};
// use rmcp::tool; // TODO: Re-enable when rmcp API is clarified
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Room information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    /// Room UUID
    pub uuid: String,

    /// Room name
    pub name: String,

    /// Number of devices in room
    pub device_count: usize,

    /// Device breakdown by category
    pub devices_by_category: HashMap<String, usize>,

    /// Sample device names (first 5)
    pub sample_devices: Vec<String>,
}

/// List all rooms with device counts
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn list_rooms(context: ToolContext) -> ToolResponse {
    let rooms = context.context.rooms.read().await;
    let devices = context.context.devices.read().await;

    let mut room_infos = Vec::new();

    for (uuid, room) in rooms.iter() {
        // Count devices in this room
        let room_devices: Vec<_> = devices
            .values()
            .filter(|device| device.room.as_ref() == Some(&room.name))
            .collect();

        // Group by category
        let mut devices_by_category = HashMap::new();
        for device in &room_devices {
            *devices_by_category
                .entry(device.category.clone())
                .or_insert(0) += 1;
        }

        // Get sample device names
        let sample_devices: Vec<String> = room_devices
            .iter()
            .take(5)
            .map(|device| device.name.clone())
            .collect();

        room_infos.push(RoomInfo {
            uuid: uuid.clone(),
            name: room.name.clone(),
            device_count: room_devices.len(),
            devices_by_category,
            sample_devices,
        });
    }

    // Sort by name
    room_infos.sort_by(|a, b| a.name.cmp(&b.name));

    ToolResponse::success_with_message(
        serde_json::to_value(room_infos).unwrap(),
        format!("Found {} rooms", rooms.len()),
    )
}

/// Get devices in a specific room
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_room_devices(
    context: ToolContext,
    room_name: String,
    // #[description("Optional filter by device category")] // TODO: Re-enable when rmcp API is clarified
    category: Option<String>,
    // #[description("Maximum number of devices to return")] // TODO: Re-enable when rmcp API is clarified
    limit: Option<usize>,
) -> ToolResponse {
    let devices = match context.context.get_devices_by_room(&room_name).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    if devices.is_empty() {
        return ToolResponse::not_found(&room_name, Some("Use list_rooms to see available rooms"));
    }

    // Apply category filter if specified
    let mut filtered_devices = devices;
    if let Some(ref cat) = category {
        filtered_devices.retain(|device| device.category == *cat);
    }

    // Apply limit
    if let Some(limit) = limit {
        filtered_devices.truncate(limit);
    }

    let stats = DeviceStats::from_devices(&filtered_devices);

    let response_data = serde_json::json!({
        "room": room_name,
        "devices": filtered_devices,
        "stats": stats,
        "total_found": filtered_devices.len()
    });

    let message = if let Some(cat) = category {
        format!(
            "Found {} {} devices in room '{}'",
            filtered_devices.len(),
            cat,
            room_name
        )
    } else {
        format!(
            "Found {} devices in room '{}'",
            filtered_devices.len(),
            room_name
        )
    };

    ToolResponse::success_with_message(response_data, message)
}

/// Get room overview with statistics
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_room_overview(context: ToolContext, room_name: String) -> ToolResponse {
    let devices = match context.context.get_devices_by_room(&room_name).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    if devices.is_empty() {
        return ToolResponse::not_found(&room_name, Some("Use list_rooms to see available rooms"));
    }

    let stats = DeviceStats::from_devices(&devices);

    // Get room info
    let rooms = context.context.rooms.read().await;
    let room_info = rooms.values().find(|room| room.name == room_name).cloned();

    // Categorize devices for overview
    let mut device_categories = HashMap::new();
    for device in &devices {
        let category_devices = device_categories
            .entry(device.category.clone())
            .or_insert_with(Vec::new);
        category_devices.push(serde_json::json!({
            "name": device.name,
            "type": device.device_type,
            "uuid": device.uuid
        }));
    }

    let response_data = serde_json::json!({
        "room": room_info,
        "statistics": stats,
        "device_categories": device_categories,
        "capabilities": {
            "has_lighting": stats.by_category.contains_key("lighting"),
            "has_blinds": stats.by_category.contains_key("blinds"),
            "has_climate": stats.by_category.contains_key("climate"),
            "has_sensors": stats.by_category.contains_key("sensors")
        }
    });

    ToolResponse::success_with_message(
        response_data,
        format!(
            "Room '{}' overview with {} devices",
            room_name,
            devices.len()
        ),
    )
}

