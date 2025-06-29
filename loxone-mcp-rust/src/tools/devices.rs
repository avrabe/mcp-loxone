//! Device control MCP tools
//!
//! Tools for device control and batch operations.
//! For read-only device data, use resources:
//! - loxone://devices/all - All devices
//! - loxone://devices/category/{category} - Devices by category
//! - loxone://system/capabilities - System capabilities

use crate::tools::{ActionAliases, DeviceFilter, DeviceStats, ToolContext, ToolResponse};
// use crate::validation::ToolParameterValidator; // Temporarily disabled
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

// READ-ONLY TOOL REMOVED:
// discover_all_devices() → Use resource: loxone://devices/all
// This function provided read-only data access and violated MCP patterns.

/// Control a specific device
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn control_device(
    context: ToolContext,
    // #[description("Device name or UUID")] // TODO: Re-enable when rmcp API is clarified
    device: String,
    // #[description("Action to perform (on/off/up/down/stop/dim)")] // TODO: Re-enable when rmcp API is clarified
    action: String,
) -> ToolResponse {
    // Temporarily disabled validation
    // if let Err(e) = ToolParameterValidator::validate_device_control(&device, &action) {
    //     return ToolResponse::error(e.to_string());
    // }

    // Basic validation instead
    if device.is_empty() {
        return ToolResponse::error("Device cannot be empty".to_string());
    }
    if action.is_empty() {
        return ToolResponse::error("Action cannot be empty".to_string());
    }

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
    let result = match context
        .send_device_command(&device_info.uuid, &normalized_action)
        .await
    {
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
        format!(
            "Successfully controlled device '{}' with action '{}'",
            result.device, result.action
        )
    } else {
        format!(
            "Failed to control device '{}': {}",
            result.device,
            result.error.as_deref().unwrap_or("Unknown error")
        )
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
    action: String,
) -> ToolResponse {
    // use crate::validation::InputValidator; // Temporarily disabled

    if devices.is_empty() {
        return ToolResponse::error("No devices specified".to_string());
    }

    // Temporarily disabled validation
    // if let Err(e) = InputValidator::validate_batch_size(devices.len()) {
    //     return ToolResponse::error(e.to_string());
    // }

    // Validate action
    // if let Err(e) = InputValidator::validate_action(&action) {
    //     return ToolResponse::error(e.to_string());
    // }

    // Basic validation instead
    if devices.len() > 100 {
        return ToolResponse::error("Too many devices (max 100)".to_string());
    }

    if action.is_empty() {
        return ToolResponse::error("Action cannot be empty".to_string());
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
        return ToolResponse::not_found(
            &not_found.join(", "),
            Some("Use discover_all_devices to find available devices"),
        );
    }

    // Build command list
    let commands: Vec<(String, String)> = device_infos
        .iter()
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
                if success {
                    successful += 1;
                } else {
                    failed += 1;
                }

                DeviceControlResult {
                    device: device.name.clone(),
                    uuid: device.uuid.clone(),
                    action: normalized_action.clone(),
                    success,
                    code: Some(response.code),
                    error: if success {
                        None
                    } else {
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
        format!("Successfully controlled {successful} devices with action '{normalized_action}'")
    } else {
        let total_devices = device_infos.len();
        format!(
            "Controlled {successful}/{total_devices} devices with action '{normalized_action}' ({failed} failed)"
        )
    };

    ToolResponse::success_with_message(response_data, message)
}

/// Get devices by category
// #[tool] // TODO: Re-enable when rmcp API is clarified
// READ-ONLY TOOL REMOVED:
// get_devices_by_category() → Use resource: loxone://devices/category/{category}
// This function provided read-only data access and violated MCP patterns.

// READ-ONLY TOOL REMOVED:
// get_available_capabilities() → Use resource: loxone://system/capabilities
// This function provided read-only data access and violated MCP patterns.

#[allow(dead_code)]
async fn _removed_get_available_capabilities(context: ToolContext) -> ToolResponse {
    let capabilities = context.context.capabilities.read().await;
    let devices = context.context.devices.read().await;

    let mut available_features = serde_json::Map::new();

    // Lighting capability
    if capabilities.has_lighting {
        available_features.insert(
            "lighting".to_string(),
            serde_json::json!({
                "available": true,
                "device_count": capabilities.light_count,
                "tools": [
                    "control_device",
                    "control_multiple_devices",
                    "control_all_lights",
                    "control_room_lights",
                    "get_devices_by_type (with 'LightController')",
                    "get_devices_by_category (with 'lighting')"
                ],
                "description": "Control lights, dimmers, and switches"
            }),
        );
    }

    // Blinds/Rolladen capability
    if capabilities.has_blinds {
        available_features.insert(
            "blinds_rolladen".to_string(),
            serde_json::json!({
                "available": true,
                "device_count": capabilities.blind_count,
                "tools": [
                    "control_device",
                    "control_multiple_devices",
                    "control_all_rolladen",
                    "control_room_rolladen",
                    "get_devices_by_type (with 'Jalousie')",
                    "get_devices_by_category (with 'blinds')"
                ],
                "description": "Control blinds, shutters, and rolladen (up/down/stop)"
            }),
        );
    }

    // Weather capability
    if capabilities.has_weather {
        available_features.insert(
            "weather".to_string(),
            serde_json::json!({
                "available": true,
                "device_count": devices.values()
                    .filter(|d| d.category == "weather")
                    .count(),
                "tools": ["get_weather_data", "get_outdoor_conditions"],
                "description": "Weather stations and environmental sensors"
            }),
        );
    }

    // Security capability
    if capabilities.has_security {
        available_features.insert(
            "security".to_string(),
            serde_json::json!({
                "available": true,
                "device_count": devices.values()
                    .filter(|d| d.category == "security")
                    .count(),
                "tools": ["get_security_status", "get_all_door_window_sensors"],
                "description": "Security system, alarms, and access control"
            }),
        );
    }

    // Energy capability
    if capabilities.has_energy {
        available_features.insert(
            "energy".to_string(),
            serde_json::json!({
                "available": true,
                "device_count": devices.values()
                    .filter(|d| d.category == "energy")
                    .count(),
                "tools": ["get_energy_consumption"],
                "description": "Power monitoring and energy management"
            }),
        );
    }

    // Audio capability
    if capabilities.has_audio {
        available_features.insert(
            "audio".to_string(),
            serde_json::json!({
                "available": true,
                "zone_count": devices.values()
                    .filter(|d| d.category == "audio")
                    .count(),
                "tools": [
                    "get_audio_zones",
                    "control_audio_zone",
                    "get_audio_sources",
                    "set_audio_volume"
                ],
                "description": "Multiroom audio system control"
            }),
        );
    }

    // Climate capability
    if capabilities.has_climate {
        available_features.insert(
            "climate".to_string(),
            serde_json::json!({
                "available": true,
                "device_count": capabilities.climate_count,
                "tools": ["get_climate_control", "get_temperature_sensors"],
                "description": "HVAC, heating, and climate control"
            }),
        );
    }

    // Sensors capability
    if capabilities.has_sensors {
        available_features.insert(
            "sensors".to_string(),
            serde_json::json!({
                "available": true,
                "sensor_count": capabilities.sensor_count,
                "tools": [
                    "get_all_door_window_sensors",
                    "get_temperature_sensors",
                    "list_discovered_sensors",
                    "get_sensor_details",
                    "get_recent_sensor_changes"
                ],
                "description": "Various sensors including motion, temperature, door/window"
            }),
        );
    }

    // Room management (always available)
    available_features.insert(
        "room_management".to_string(),
        serde_json::json!({
            "available": true,
            "room_count": context.context.rooms.read().await.len(),
            "tools": [
                "list_rooms",
                "get_room_devices",
                "control_room_lights",
                "control_room_rolladen"
            ],
            "description": "Room-based device organization and control"
        }),
    );

    // Device discovery (always available)
    available_features.insert(
        "device_discovery".to_string(),
        serde_json::json!({
            "available": true,
            "total_devices": devices.len(),
            "tools": [
                "discover_all_devices",
                "get_devices_by_type",
                "get_devices_by_category",
                "get_all_categories_overview"
            ],
            "description": "Device discovery and categorization"
        }),
    );

    let response_data = serde_json::json!({
        "system_capabilities": available_features,
        "note": "Available features depend on your Loxone system configuration"
    });

    ToolResponse::success_with_message(
        response_data,
        format!("Found {} available capabilities", available_features.len()),
    )
}

// READ-ONLY TOOL REMOVED:
// get_all_categories_overview() → Use resource: loxone://system/categories
// This function provided read-only data access and violated MCP patterns.

// READ-ONLY TOOL REMOVED:
// get_devices_by_type() → Use resource: loxone://devices/type/{type}
// This function provided read-only data access and violated MCP patterns.

// Note: get_all_blinds_status has been migrated to a resource: loxone://devices/category/blinds
