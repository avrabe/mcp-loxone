//! Device control and discovery MCP tools
//!
//! Tools for device discovery, individual control, and batch operations.

use crate::tools::{ToolContext, ToolResponse, DeviceFilter, DeviceStats, ActionAliases};
// use rmcp::tool; // TODO: Re-enable when rmcp API is clarified
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Device control result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceControlResult {
    /// Device name
    pub device: String,
    
    /// Device UUID
    pub uuid: String,
    
    /// Action performed
    pub action: String,
    
    /// Success status
    pub success: bool,
    
    /// Response code
    pub code: Option<i32>,
    
    /// Error message if failed
    pub error: Option<String>,
    
    /// Response value
    pub response: Option<serde_json::Value>,
}

/// Discover all devices in the system
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn discover_all_devices(
    context: ToolContext,
    // #[description("Optional filter by category")] // TODO: Re-enable when rmcp API is clarified
    category: Option<String>,
    // #[description("Optional filter by device type")] // TODO: Re-enable when rmcp API is clarified
    device_type: Option<String>,
    // #[description("Maximum number of devices to return")] // TODO: Re-enable when rmcp API is clarified
    limit: Option<usize>
) -> ToolResponse {
    let filter = if category.is_some() || device_type.is_some() || limit.is_some() {
        Some(DeviceFilter {
            category,
            device_type,
            room: None,
            limit,
        })
    } else {
        None
    };
    
    let devices = match context.get_devices(filter).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    let stats = DeviceStats::from_devices(&devices);
    
    let response_data = serde_json::json!({
        "devices": devices,
        "statistics": stats,
        "total_found": devices.len()
    });
    
    ToolResponse::success_with_message(
        response_data,
        format!("Discovered {} devices", devices.len())
    )
}

/// Control a specific device
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn control_device(
    context: ToolContext,
    // #[description("Device name or UUID")] // TODO: Re-enable when rmcp API is clarified
    device: String,
    // #[description("Action to perform (on/off/up/down/stop/dim)")] // TODO: Re-enable when rmcp API is clarified
    action: String
) -> ToolResponse {
    // Find the device
    let device_info = match context.find_device(&device).await {
        Ok(device) => device,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    // Normalize action
    let normalized_action = ActionAliases::normalize_action(&action);
    
    // Validate action for device type
    let valid_actions = ActionAliases::get_valid_actions(&device_info.device_type);
    if !valid_actions.contains(&normalized_action.as_str()) {
        return ToolResponse::error(format!(
            "Invalid action '{}' for device type '{}'. Valid actions: {}",
            action,
            device_info.device_type,
            valid_actions.join(", ")
        ));
    }
    
    // Send command
    let result = match context.send_device_command(&device_info.uuid, &normalized_action).await {
        Ok(response) => DeviceControlResult {
            device: device_info.name.clone(),
            uuid: device_info.uuid.clone(),
            action: normalized_action,
            success: response.code == 200,
            code: Some(response.code),
            error: if response.code != 200 { 
                Some(format!("Command failed with code {}", response.code))
            } else { 
                None 
            },
            response: Some(response.value),
        },
        Err(e) => DeviceControlResult {
            device: device_info.name.clone(),
            uuid: device_info.uuid.clone(),
            action: normalized_action,
            success: false,
            code: None,
            error: Some(e.to_string()),
            response: None,
        },
    };
    
    let message = if result.success {
        format!("Successfully controlled device '{}' with action '{}'", 
                result.device, result.action)
    } else {
        format!("Failed to control device '{}': {}", 
                result.device, 
                result.error.as_deref().unwrap_or("Unknown error"))
    };
    
    ToolResponse::success_with_message(serde_json::to_value(result).unwrap(), message)
}

/// Control multiple devices with the same action
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn control_multiple_devices(
    context: ToolContext,
    // #[description("List of device names or UUIDs")] // TODO: Re-enable when rmcp API is clarified
    devices: Vec<String>,
    // #[description("Action to perform on all devices")] // TODO: Re-enable when rmcp API is clarified
    action: String
) -> ToolResponse {
    if devices.is_empty() {
        return ToolResponse::error("No devices specified".to_string());
    }
    
    // Normalize action
    let normalized_action = ActionAliases::normalize_action(&action);
    
    // Find all devices and validate
    let mut device_infos = Vec::new();
    let mut not_found = Vec::new();
    
    for device_id in &devices {
        match context.find_device(device_id).await {
            Ok(device) => device_infos.push(device),
            Err(_) => not_found.push(device_id.clone()),
        }
    }
    
    if !not_found.is_empty() {
        return ToolResponse::error(format!("Devices not found: {}", not_found.join(", ")));
    }
    
    // Build command list
    let commands: Vec<(String, String)> = device_infos.iter()
        .map(|device| (device.uuid.clone(), normalized_action.clone()))
        .collect();
    
    // Execute commands in parallel
    let results = match context.send_parallel_commands(commands).await {
        Ok(results) => results,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    // Process results
    let mut control_results = Vec::new();
    let mut successful = 0;
    let mut failed = 0;
    
    for (device, result) in device_infos.iter().zip(results.iter()) {
        let control_result = match result {
            Ok(response) => {
                let success = response.code == 200;
                if success { successful += 1; } else { failed += 1; }
                
                DeviceControlResult {
                    device: device.name.clone(),
                    uuid: device.uuid.clone(),
                    action: normalized_action.clone(),
                    success,
                    code: Some(response.code),
                    error: if success { None } else { 
                        Some(format!("Command failed with code {}", response.code))
                    },
                    response: Some(response.value.clone()),
                }
            }
            Err(e) => {
                failed += 1;
                DeviceControlResult {
                    device: device.name.clone(),
                    uuid: device.uuid.clone(),
                    action: normalized_action.clone(),
                    success: false,
                    code: None,
                    error: Some(e.to_string()),
                    response: None,
                }
            }
        };
        
        control_results.push(control_result);
    }
    
    let response_data = serde_json::json!({
        "action": normalized_action,
        "total_devices": device_infos.len(),
        "successful": successful,
        "failed": failed,
        "results": control_results
    });
    
    let message = if failed == 0 {
        format!("Successfully controlled {} devices with action '{}'", successful, normalized_action)
    } else {
        format!("Controlled {}/{} devices with action '{}' ({} failed)", 
                successful, device_infos.len(), normalized_action, failed)
    };
    
    ToolResponse::success_with_message(response_data, message)
}

/// Control all lights in the system
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn control_all_lights(
    context: ToolContext,
    // #[description("Action: on, off, dim, bright")] // TODO: Re-enable when rmcp API is clarified
    action: String
) -> ToolResponse {
    // Get all lighting devices
    let devices = match context.context.get_devices_by_category("lighting").await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    if devices.is_empty() {
        return ToolResponse::error("No lighting devices found in the system".to_string());
    }
    
    // Normalize action
    let normalized_action = ActionAliases::normalize_action(&action);
    
    // Validate action for lights
    if !["on", "off", "dim", "bright"].contains(&normalized_action.as_str()) {
        return ToolResponse::error(format!(
            "Invalid action '{}' for lights. Use: on, off, dim, bright", action
        ));
    }
    
    // Build command list
    let commands: Vec<(String, String)> = devices.iter()
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
    let mut room_stats = HashMap::new();
    
    for (device, result) in devices.iter().zip(results.iter()) {
        // Update room statistics
        if let Some(ref room) = device.room {
            let stats = room_stats.entry(room.clone()).or_insert((0, 0));
            match result {
                Ok(response) if response.code == 200 => {
                    successful += 1;
                    stats.0 += 1;
                }
                _ => {
                    failed += 1;
                    stats.1 += 1;
                }
            }
        }
        
        // Collect errors
        if let Err(e) = result {
            errors.push(format!("{}: {}", device.name, e));
        } else if let Ok(response) = result {
            if response.code != 200 {
                errors.push(format!("{}: Code {}", device.name, response.code));
            }
        }
    }
    
    let response_data = serde_json::json!({
        "action": normalized_action,
        "total_devices": devices.len(),
        "successful": successful,
        "failed": failed,
        "room_statistics": room_stats,
        "errors": errors.into_iter().take(10).collect::<Vec<_>>(), // Limit errors shown
        "device_summary": devices.iter().map(|d| serde_json::json!({
            "name": d.name,
            "room": d.room,
            "type": d.device_type
        })).collect::<Vec<_>>()
    });
    
    let message = if failed == 0 {
        format!("Successfully controlled all {} lights with action '{}'", successful, normalized_action)
    } else {
        format!("Controlled {}/{} lights with action '{}' ({} failed)", 
                successful, devices.len(), normalized_action, failed)
    };
    
    ToolResponse::success_with_message(response_data, message)
}

/// Control all blinds/rolladen in the system
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn control_all_rolladen(
    context: ToolContext,
    // #[description("Action: up, down, stop")] // TODO: Re-enable when rmcp API is clarified
    action: String
) -> ToolResponse {
    // Get all blind devices
    let devices = match context.context.get_devices_by_category("blinds").await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    if devices.is_empty() {
        return ToolResponse::error("No blinds/rolladen found in the system".to_string());
    }
    
    // Normalize action
    let normalized_action = ActionAliases::normalize_action(&action);
    
    // Validate action for blinds
    if !["up", "down", "stop"].contains(&normalized_action.as_str()) {
        return ToolResponse::error(format!(
            "Invalid action '{}' for blinds. Use: up, down, stop", action
        ));
    }
    
    // Build command list
    let commands: Vec<(String, String)> = devices.iter()
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
    let mut room_stats = HashMap::new();
    
    for (device, result) in devices.iter().zip(results.iter()) {
        // Update room statistics
        if let Some(ref room) = device.room {
            let stats = room_stats.entry(room.clone()).or_insert((0, 0));
            match result {
                Ok(response) if response.code == 200 => {
                    successful += 1;
                    stats.0 += 1;
                }
                _ => {
                    failed += 1;
                    stats.1 += 1;
                }
            }
        }
        
        // Collect errors
        if let Err(e) = result {
            errors.push(format!("{}: {}", device.name, e));
        } else if let Ok(response) = result {
            if response.code != 200 {
                errors.push(format!("{}: Code {}", device.name, response.code));
            }
        }
    }
    
    let response_data = serde_json::json!({
        "action": normalized_action,
        "total_devices": devices.len(),
        "successful": successful,
        "failed": failed,
        "room_statistics": room_stats,
        "errors": errors.into_iter().take(10).collect::<Vec<_>>(),
        "device_summary": devices.iter().map(|d| serde_json::json!({
            "name": d.name,
            "room": d.room,
            "type": d.device_type
        })).collect::<Vec<_>>()
    });
    
    let message = if failed == 0 {
        format!("Successfully controlled all {} blinds with action '{}'", successful, normalized_action)
    } else {
        format!("Controlled {}/{} blinds with action '{}' ({} failed)", 
                successful, devices.len(), normalized_action, failed)
    };
    
    ToolResponse::success_with_message(response_data, message)
}

/// Get devices by category
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_devices_by_category(
    context: ToolContext,
    // #[description("Device category (lighting, blinds, climate, sensors, etc.)")] // TODO: Re-enable when rmcp API is clarified
    category: String,
    // #[description("Maximum number of devices to return")] // TODO: Re-enable when rmcp API is clarified
    limit: Option<usize>
) -> ToolResponse {
    let mut devices = match context.context.get_devices_by_category(&category).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    if devices.is_empty() {
        return ToolResponse::error(format!("No devices found in category '{}'", category));
    }
    
    // Apply limit
    if let Some(limit) = limit {
        devices.truncate(limit);
    }
    
    let stats = DeviceStats::from_devices(&devices);
    
    let response_data = serde_json::json!({
        "category": category,
        "devices": devices,
        "statistics": stats,
        "total_found": devices.len()
    });
    
    ToolResponse::success_with_message(
        response_data,
        format!("Found {} devices in category '{}'", devices.len(), category)
    )
}

/// Get devices by type
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_devices_by_type(
    context: ToolContext,
    // #[description("Device type (e.g., LightController, Jalousie)")] // TODO: Re-enable when rmcp API is clarified
    device_type: String,
    // #[description("Maximum number of devices to return")] // TODO: Re-enable when rmcp API is clarified
    limit: Option<usize>
) -> ToolResponse {
    let devices = match context.get_devices(Some(DeviceFilter {
        device_type: Some(device_type.clone()),
        category: None,
        room: None,
        limit,
    })).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };
    
    if devices.is_empty() {
        return ToolResponse::error(format!("No devices found of type '{}'", device_type));
    }
    
    let stats = DeviceStats::from_devices(&devices);
    
    let response_data = serde_json::json!({
        "device_type": device_type,
        "devices": devices,
        "statistics": stats,
        "total_found": devices.len()
    });
    
    ToolResponse::success_with_message(
        response_data,
        format!("Found {} devices of type '{}'", devices.len(), device_type)
    )
}