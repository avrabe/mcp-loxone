//! Unified rolladen/blinds control tools
//!
//! This module consolidates all rolladen/blinds-related MCP tools into a single,
//! comprehensive implementation that eliminates code duplication and provides
//! consistent blinds control across the entire system.

use crate::tools::{ToolContext, ToolResponse};
use serde_json::json;

/// Unified rolladen/blinds control with scope-based targeting
pub async fn control_rolladen_unified(
    context: ToolContext,
    scope: String,
    target: Option<String>,
    action: String,
    position: Option<u8>,
) -> ToolResponse {
    // Ensure we're connected
    if let Err(e) = context.ensure_connected().await {
        return ToolResponse::error(format!("Connection error: {}", e));
    }

    // Validate action using ActionAliases for multi-language support
    let normalized_action = match action.to_lowercase().as_str() {
        "up" | "hoch" | "rauf" | "öffnen" | "open" => "up",
        "down" | "runter" | "zu" | "schließen" | "close" => "down", 
        "stop" | "stopp" | "halt" => "stop",
        "position" | "pos" => "position",
        _ => {
            return ToolResponse::error(format!(
                "Invalid action '{}'. Supported actions: up, down, stop, position", 
                action
            ));
        }
    };

    // Validate position for position actions
    if normalized_action == "position" {
        if let Some(pos) = position {
            if pos > 100 {
                return ToolResponse::error("Position must be between 0-100 (0=fully up, 100=fully down)".to_string());
            }
        } else {
            return ToolResponse::error("Position action requires a position value (0-100)".to_string());
        }
    }

    // Get target rolladen devices based on scope
    let rolladen_devices = match scope.to_lowercase().as_str() {
        "device" => {
            let device_id = target.as_deref().unwrap_or("");
            if device_id.is_empty() {
                return ToolResponse::error("Device scope requires a target device name or UUID".to_string());
            }
            match get_rolladen_device_by_id(&context, device_id).await {
                Ok(devices) => devices,
                Err(e) => return ToolResponse::error(format!("Failed to get rolladen device: {}", e)),
            }
        }
        "room" => {
            let room_name = target.as_deref().unwrap_or("");
            if room_name.is_empty() {
                return ToolResponse::error("Room scope requires a target room name".to_string());
            }
            match get_room_rolladen_devices(&context, room_name).await {
                Ok(devices) => devices,
                Err(e) => return ToolResponse::error(format!("Failed to get room rolladen devices: {}", e)),
            }
        }
        "system" | "all" => {
            match get_all_rolladen_devices(&context).await {
                Ok(devices) => devices,
                Err(e) => return ToolResponse::error(format!("Failed to get all rolladen devices: {}", e)),
            }
        }
        _ => {
            return ToolResponse::error(format!(
                "Invalid scope '{}'. Supported scopes: device, room, system", 
                scope
            ));
        }
    };

    if rolladen_devices.is_empty() {
        return ToolResponse::success(json!({
            "message": format!("No rolladen/blinds devices found for scope '{}' and target '{}' ", 
                              scope, target.unwrap_or_default()),
            "devices_controlled": 0,
            "devices": []
        }));
    }

    // Execute rolladen commands in parallel for performance
    let results = execute_rolladen_commands(&context, &rolladen_devices, normalized_action, position).await;

    // Process results and generate response
    let mut successful_devices = Vec::new();
    let mut failed_devices = Vec::new();
    let mut room_stats = std::collections::HashMap::new();

    for (device, result) in rolladen_devices.iter().zip(results.iter()) {
        let room = device.room.as_deref().unwrap_or("Unknown");
        let room_entry = room_stats.entry(room.to_string()).or_insert_with(|| json!({
            "room": room,
            "total": 0,
            "successful": 0,
            "failed": 0
        }));

        room_entry["total"] = (room_entry["total"].as_u64().unwrap_or(0) + 1).into();

        match result {
            Ok(_) => {
                successful_devices.push(json!({
                    "uuid": device.uuid,
                    "name": device.name,
                    "room": room,
                    "type": device.device_type,
                    "action": normalized_action,
                    "position": position,
                    "status": "success"
                }));
                room_entry["successful"] = (room_entry["successful"].as_u64().unwrap_or(0) + 1).into();
            }
            Err(e) => {
                failed_devices.push(json!({
                    "uuid": device.uuid,
                    "name": device.name,
                    "room": room,
                    "type": device.device_type,
                    "action": normalized_action,
                    "error": e.to_string(),
                    "status": "failed"
                }));
                room_entry["failed"] = (room_entry["failed"].as_u64().unwrap_or(0) + 1).into();
            }
        }
    }

    ToolResponse::success(json!({
        "action": normalized_action,
        "scope": scope,
        "target": target,
        "position": position,
        "summary": {
            "total_devices": rolladen_devices.len(),
            "successful": successful_devices.len(),
            "failed": failed_devices.len(),
            "success_rate": format!("{:.1}%", 
                (successful_devices.len() as f64 / rolladen_devices.len() as f64) * 100.0)
        },
        "room_statistics": room_stats.values().collect::<Vec<_>>(),
        "devices": {
            "successful": successful_devices,
            "failed": failed_devices
        }
    }))
}

/// Get all rolladen/blinds devices in the system
async fn get_all_rolladen_devices(context: &ToolContext) -> Result<Vec<crate::client::LoxoneDevice>, crate::error::LoxoneError> {
    let all_devices = context.get_devices(None).await?;
    
    Ok(all_devices
        .into_iter()
        .filter(is_rolladen_device)
        .collect())
}

/// Get rolladen/blinds devices in a specific room
async fn get_room_rolladen_devices(
    context: &ToolContext,
    room_name: &str,
) -> Result<Vec<crate::client::LoxoneDevice>, crate::error::LoxoneError> {
    let all_devices = context.get_devices(None).await?;
    
    Ok(all_devices
        .into_iter()
        .filter(|device| {
            is_rolladen_device(device) && 
            device.room.as_deref().unwrap_or("").to_lowercase() == room_name.to_lowercase()
        })
        .collect())
}

/// Get a specific rolladen/blinds device by name or UUID
async fn get_rolladen_device_by_id(
    context: &ToolContext,
    device_id: &str,
) -> Result<Vec<crate::client::LoxoneDevice>, crate::error::LoxoneError> {
    let all_devices = context.get_devices(None).await?;
    
    // Look for exact UUID match first
    if let Some(device) = all_devices.iter().find(|d| d.uuid == device_id) {
        if is_rolladen_device(device) {
            return Ok(vec![device.clone()]);
        } else {
            return Err(crate::error::LoxoneError::invalid_input(format!(
                "Device '{}' exists but is not a rolladen/blinds device", device_id
            )));
        }
    }
    
    // Look for name match
    if let Some(device) = all_devices.iter().find(|d| d.name.to_lowercase() == device_id.to_lowercase()) {
        if is_rolladen_device(device) {
            return Ok(vec![device.clone()]);
        } else {
            return Err(crate::error::LoxoneError::invalid_input(format!(
                "Device '{}' exists but is not a rolladen/blinds device", device_id
            )));
        }
    }
    
    Err(crate::error::LoxoneError::not_found(format!(
        "Rolladen/blinds device '{}' not found", device_id
    )))
}

/// Check if a device is a rolladen/blinds device
fn is_rolladen_device(device: &crate::client::LoxoneDevice) -> bool {
    // Check category first (most reliable)
    if device.category == "blinds" || device.category == "rolladen" {
        return true;
    }
    
    // Check device type patterns
    match device.device_type.as_str() {
        "Jalousie" | "Blind" | "RolladenController" => true,
        _ => {
            // Check for rolladen/blinds-related keywords in type or name
            let type_lower = device.device_type.to_lowercase();
            let name_lower = device.name.to_lowercase();
            
            type_lower.contains("jalousie") || 
            type_lower.contains("rolladen") ||
            type_lower.contains("blind") ||
            name_lower.contains("rolladen") ||
            name_lower.contains("jalousie") ||
            name_lower.contains("blind")
        }
    }
}

/// Execute rolladen/blinds commands in parallel for better performance
async fn execute_rolladen_commands(
    context: &ToolContext,
    devices: &[crate::client::LoxoneDevice],
    action: &str,
    position: Option<u8>,
) -> Vec<Result<crate::client::LoxoneResponse, crate::error::LoxoneError>> {
    use futures::future::join_all;

    let command_futures = devices.iter().map(|device| {
        async move {
            let command = match action {
                "up" => "FullUp".to_string(),
                "down" => "FullDown".to_string(),
                "stop" => "Stop".to_string(),
                "position" => {
                    if let Some(pos) = position {
                        // Convert percentage to 0.0-1.0 range (0=up, 1=down)
                        format!("{}", pos as f64 / 100.0)
                    } else {
                        "0.5".to_string() // Default to 50% if no position specified
                    }
                }
                _ => "Stop".to_string(), // Fallback
            };

            context.client.send_command(&device.uuid, &command).await
        }
    });

    join_all(command_futures).await
}

/// Discover all rolladen/blinds capabilities in the system
pub async fn discover_rolladen_capabilities(context: ToolContext) -> ToolResponse {
    // Ensure we're connected
    if let Err(e) = context.ensure_connected().await {
        return ToolResponse::error(format!("Connection error: {}", e));
    }

    let rolladen_devices = match get_all_rolladen_devices(&context).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(format!("Failed to get rolladen devices: {}", e)),
    };

    // Group devices by room and type
    let mut room_summary = std::collections::HashMap::new();
    let mut type_summary = std::collections::HashMap::new();
    let mut capabilities = std::collections::HashSet::new();

    for device in &rolladen_devices {
        let room = device.room.as_deref().unwrap_or("Unknown").to_string();
        let device_type = device.device_type.clone();

        // Room statistics
        let room_entry = room_summary.entry(room.clone()).or_insert_with(|| json!({
            "room": room,
            "device_count": 0,
            "device_types": std::collections::HashSet::<String>::new()
        }));
        room_entry["device_count"] = (room_entry["device_count"].as_u64().unwrap_or(0) + 1).into();

        // Type statistics  
        *type_summary.entry(device_type.clone()).or_insert(0) += 1;

        // Determine capabilities based on device type
        match device_type.as_str() {
            "Jalousie" => {
                capabilities.insert("up_down_control");
                capabilities.insert("position_control");
                capabilities.insert("stop_control");
                capabilities.insert("tilt_control");
            }
            "Blind" | "RolladenController" => {
                capabilities.insert("up_down_control");
                capabilities.insert("position_control");
                capabilities.insert("stop_control");
            }
            _ => {
                capabilities.insert("basic_control");
            }
        }
    }

    ToolResponse::success(json!({
        "summary": {
            "total_rolladen_devices": rolladen_devices.len(),
            "rooms_with_rolladen": room_summary.len(),
            "device_types": type_summary.len()
        },
        "capabilities": capabilities.into_iter().collect::<Vec<_>>(),
        "supported_actions": ["up", "down", "stop", "position"],
        "supported_scopes": ["device", "room", "system"],
        "room_breakdown": room_summary.values().collect::<Vec<_>>(),
        "type_breakdown": type_summary,
        "devices": rolladen_devices.iter().map(|device| json!({
            "uuid": device.uuid,
            "name": device.name,
            "type": device.device_type,
            "room": device.room.as_deref().unwrap_or("Unknown"),
            "category": device.category
        })).collect::<Vec<_>>()
    }))
}