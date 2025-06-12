//! Room management MCP tools
//!
//! Tools for listing rooms, getting room devices, and room-based operations.

use crate::tools::{ToolContext, ToolResponse, DeviceStats};
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
        let room_devices: Vec<_> = devices.values()
            .filter(|device| device.room.as_ref() == Some(&room.name))
            .collect();
        
        // Group by category
        let mut devices_by_category = HashMap::new();
        for device in &room_devices {
            *devices_by_category.entry(device.category.clone()).or_insert(0) += 1;
        }
        
        // Get sample device names
        let sample_devices: Vec<String> = room_devices.iter()
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
        format!("Found {} rooms", rooms.len())
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
    limit: Option<usize>
) -> ToolResponse {
    let devices = match context.context.get_devices_by_room(&room_name).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    if devices.is_empty() {
        return ToolResponse::error(format!("Room '{}' not found or has no devices", room_name));
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
        format!("Found {} {} devices in room '{}'", filtered_devices.len(), cat, room_name)
    } else {
        format!("Found {} devices in room '{}'", filtered_devices.len(), room_name)
    };
    
    ToolResponse::success_with_message(response_data, message)
}

/// Get room overview with statistics
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_room_overview(
    context: ToolContext,
    room_name: String
) -> ToolResponse {
    let devices = match context.context.get_devices_by_room(&room_name).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    if devices.is_empty() {
        return ToolResponse::error(format!("Room '{}' not found", room_name));
    }
    
    let stats = DeviceStats::from_devices(&devices);
    
    // Get room info
    let rooms = context.context.rooms.read().await;
    let room_info = rooms.values()
        .find(|room| room.name == room_name)
        .cloned();
    
    // Categorize devices for overview
    let mut device_categories = HashMap::new();
    for device in &devices {
        let category_devices = device_categories.entry(device.category.clone())
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
        format!("Room '{}' overview with {} devices", room_name, devices.len())
    )
}

/// Control all lights in a room
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn control_room_lights(
    context: ToolContext,
    room_name: String,
    // #[description("Action: on, off, dim, bright")] // TODO: Re-enable when rmcp API is clarified
    action: String
) -> ToolResponse {
    // Get lighting devices in the room
    let devices = match context.context.get_devices_by_room(&room_name).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    let light_devices: Vec<_> = devices.into_iter()
        .filter(|device| device.category == "lighting")
        .collect();
    
    if light_devices.is_empty() {
        return ToolResponse::error(format!("No lights found in room '{}'", room_name));
    }
    
    // Normalize action
    let normalized_action = crate::tools::ActionAliases::normalize_action(&action);
    
    // Build command list
    let commands: Vec<(String, String)> = light_devices.iter()
        .map(|device| (device.uuid.clone(), normalized_action.clone()))
        .collect();
    
    // Execute commands in parallel
    let results = match context.send_parallel_commands(commands).await {
        Ok(results) => results,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    // Process results
    let mut successful = 0;
    let mut failed = 0;
    let mut errors = Vec::new();
    
    for (device, result) in light_devices.iter().zip(results.iter()) {
        match result {
            Ok(response) if response.code == 200 => successful += 1,
            Ok(response) => {
                failed += 1;
                errors.push(format!("{}: Code {}", device.name, response.code));
            }
            Err(e) => {
                failed += 1;
                errors.push(format!("{}: {}", device.name, e));
            }
        }
    }
    
    let response_data = serde_json::json!({
        "room": room_name,
        "action": normalized_action,
        "total_devices": light_devices.len(),
        "successful": successful,
        "failed": failed,
        "errors": errors,
        "devices": light_devices.iter().map(|d| serde_json::json!({
            "name": d.name,
            "uuid": d.uuid,
            "type": d.device_type
        })).collect::<Vec<_>>()
    });
    
    let message = if failed == 0 {
        format!("Successfully controlled {} lights in room '{}'", successful, room_name)
    } else {
        format!("Controlled {}/{} lights in room '{}' ({} failed)", 
                successful, light_devices.len(), room_name, failed)
    };
    
    ToolResponse::success_with_message(response_data, message)
}

/// Control all blinds/rolladen in a room
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn control_room_rolladen(
    context: ToolContext,
    room_name: String,
    // #[description("Action: up, down, stop")] // TODO: Re-enable when rmcp API is clarified
    action: String
) -> ToolResponse {
    // Get blind devices in the room
    let devices = match context.context.get_devices_by_room(&room_name).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    let blind_devices: Vec<_> = devices.into_iter()
        .filter(|device| {
            device.category == "blinds" || 
            device.device_type.to_lowercase().contains("jalousie")
        })
        .collect();
    
    if blind_devices.is_empty() {
        return ToolResponse::error(format!("No blinds/rolladen found in room '{}'", room_name));
    }
    
    // Normalize action
    let normalized_action = crate::tools::ActionAliases::normalize_action(&action);
    
    // Validate action for blinds
    if !["up", "down", "stop"].contains(&normalized_action.as_str()) {
        return ToolResponse::error(format!("Invalid action '{}' for blinds. Use: up, down, stop", action));
    }
    
    // Build command list
    let commands: Vec<(String, String)> = blind_devices.iter()
        .map(|device| (device.uuid.clone(), normalized_action.clone()))
        .collect();
    
    // Execute commands in parallel
    let results = match context.send_parallel_commands(commands).await {
        Ok(results) => results,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    // Process results
    let mut successful = 0;
    let mut failed = 0;
    let mut errors = Vec::new();
    
    for (device, result) in blind_devices.iter().zip(results.iter()) {
        match result {
            Ok(response) if response.code == 200 => successful += 1,
            Ok(response) => {
                failed += 1;
                errors.push(format!("{}: Code {}", device.name, response.code));
            }
            Err(e) => {
                failed += 1;
                errors.push(format!("{}: {}", device.name, e));
            }
        }
    }
    
    let response_data = serde_json::json!({
        "room": room_name,
        "action": normalized_action,
        "total_devices": blind_devices.len(),
        "successful": successful,
        "failed": failed,
        "errors": errors,
        "devices": blind_devices.iter().map(|d| serde_json::json!({
            "name": d.name,
            "uuid": d.uuid,
            "type": d.device_type
        })).collect::<Vec<_>>()
    });
    
    let message = if failed == 0 {
        format!("Successfully controlled {} blinds in room '{}'", successful, room_name)
    } else {
        format!("Controlled {}/{} blinds in room '{}' ({} failed)", 
                successful, blind_devices.len(), room_name, failed)
    };
    
    ToolResponse::success_with_message(response_data, message)
}