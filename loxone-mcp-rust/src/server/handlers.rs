//! Handler methods for the Loxone MCP server
//!
//! This module contains all the handler methods that implement the actual
//! functionality for interacting with the Loxone system. These methods handle
//! operations like listing rooms, controlling devices, managing lights and blinds,
//! and accessing various system features.

use super::{
    rate_limiter::RateLimitResult,
    request_context::{RequestContext as McpRequestContext, RequestTracker},
    response_optimization::OptimizedResponses,
    LoxoneMcpServer,
};
use crate::tools::sensors::SensorStateLogger;
use mcp_foundation::{CallToolResult, Content};
use std::sync::Arc;
use tracing::warn;

impl LoxoneMcpServer {
    /// List all rooms in the Loxone system with device counts
    pub async fn list_rooms(&self) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let rooms = self.context.rooms.read().await;

        let mut rooms_with_info = Vec::new();
        for (uuid, room) in rooms.iter() {
            rooms_with_info.push(serde_json::json!({
                "uuid": uuid,
                "name": room.name,
                "device_count": room.device_count
            }));
        }

        let result = serde_json::json!({
            "total_rooms": rooms.len(),
            "rooms": rooms_with_info,
            "note": "Use get_room_devices(room_name) for detailed device information"
        });

        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Get devices in a specific room
    pub async fn get_room_devices(
        &self,
        room_name: String,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let devices = self.context.devices.read().await;
        let room_devices: Vec<String> = devices
            .values()
            .filter(|device| device.room.as_ref() == Some(&room_name))
            .map(|device| format!("{} ({})", device.name, device.device_type))
            .collect();

        let content =
            serde_json::to_string_pretty(&room_devices).unwrap_or_else(|_| "[]".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Control a specific device
    pub async fn control_device(
        &self,
        device_id: String,
        action: String,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        match self.client.send_command(&device_id, &action).await {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Successfully executed {} on device {}",
                action, device_id
            ))])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Failed to control device: {}",
                e
            ))])),
        }
    }

    /// Get overall system status
    pub async fn get_system_status(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        match self.client.health_check().await {
            Ok(true) => {
                let capabilities = self.context.capabilities.read().await;
                let rooms = self.context.rooms.read().await;
                let devices = self.context.devices.read().await;

                let status = serde_json::json!({
                    "system_status": "✅ Online and responsive",
                    "health": "healthy",
                    "statistics": {
                        "total_rooms": rooms.len(),
                        "total_devices": devices.len(),
                        "lighting_devices": capabilities.light_count,
                        "blind_devices": capabilities.blind_count,
                        "sensor_devices": capabilities.sensor_count
                    },
                    "capabilities": {
                        "has_lighting": capabilities.has_lighting,
                        "has_blinds": capabilities.has_blinds,
                        "has_sensors": capabilities.has_sensors,
                        "has_climate": capabilities.has_climate
                    }
                });

                let content =
                    serde_json::to_string_pretty(&status).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Ok(false) => Ok(CallToolResult::success(vec![Content::text(
                "⚠️ Loxone system is online but may have issues".to_string(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "❌ System check failed: {}",
                e
            ))])),
        }
    }

    /// Enhanced get_room_devices with device type filtering
    pub async fn get_room_devices_enhanced(
        &self,
        room_name: String,
        device_type_filter: Option<String>,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let devices = self.context.devices.read().await;
        let rooms = self.context.rooms.read().await;

        // Find room by name
        let room_uuid = rooms
            .iter()
            .find(|(_, room)| room.name.to_lowercase() == room_name.to_lowercase())
            .map(|(uuid, _)| uuid.clone());

        let room_uuid = match room_uuid {
            Some(uuid) => uuid,
            None => return Ok(OptimizedResponses::room_not_found(&room_name)),
        };
        let mut room_devices: Vec<_> = devices
            .values()
            .filter(|device| device.room.as_ref() == Some(&room_uuid))
            .collect();

        // Apply device type filter if specified
        if let Some(filter_type) = device_type_filter {
            room_devices.retain(|device| {
                device
                    .device_type
                    .to_lowercase()
                    .contains(&filter_type.to_lowercase())
            });
        }

        let device_info: Vec<_> = room_devices
            .iter()
            .map(|device| {
                serde_json::json!({
                    "uuid": device.uuid,
                    "name": device.name,
                    "type": device.device_type,
                    "category": device.category,
                    "room": room_name
                })
            })
            .collect();

        let result = serde_json::json!({
            "room": room_name,
            "device_count": device_info.len(),
            "devices": device_info
        });

        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Enhanced control_device that accepts device name or UUID
    pub async fn control_device_enhanced(
        &self,
        device: String,
        action: String,
        room_hint: Option<String>,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let devices = self.context.devices.read().await;

        // Try to find device by UUID first, then by name
        let device_entry = devices.get(&device).cloned().or_else(|| {
            devices
                .values()
                .find(|d| {
                    let name_match = d.name.to_lowercase() == device.to_lowercase();
                    if let Some(ref room) = room_hint {
                        // If room hint provided, prefer devices in that room
                        name_match
                            && d.room.as_ref().is_some_and(|r| {
                                // Check if room matches by name
                                let rooms = self.context.rooms.try_read();
                                if let Ok(rooms) = rooms {
                                    rooms.get(r).is_some_and(|room_obj| {
                                        room_obj.name.to_lowercase() == room.to_lowercase()
                                    })
                                } else {
                                    false
                                }
                            })
                    } else {
                        name_match
                    }
                })
                .cloned()
        });

        if let Some(device_obj) = device_entry {
            match self.client.send_command(&device_obj.uuid, &action).await {
                Ok(response) => {
                    let result = serde_json::json!({
                        "device": device_obj.name,
                        "uuid": device_obj.uuid,
                        "action": action,
                        "result": "success",
                        "response": response.value
                    });
                    let content =
                        serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
                    Ok(CallToolResult::success(vec![Content::text(content)]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Failed to control device {}: {}",
                    device_obj.name, e
                ))])),
            }
        } else {
            Ok(OptimizedResponses::device_not_found(&device))
        }
    }

    /// Control multiple devices simultaneously
    pub async fn control_multiple_devices(
        &self,
        devices: Vec<String>,
        action: String,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{devices::control_multiple_devices, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let response = control_multiple_devices(tool_context, devices, action).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(format!(
                "Error: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))]))
        }
    }

    /// Control all rolladen in the system
    pub async fn control_all_rolladen(
        &self,
        action: String,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let devices = self.context.devices.read().await;
        let rolladen_devices: Vec<_> = devices
            .values()
            .filter(|device| device.device_type == "Jalousie")
            .collect();

        if rolladen_devices.is_empty() {
            return Ok(OptimizedResponses::empty_blinds(Some("system")));
        }

        let mut results = Vec::new();
        let mut success_count = 0;
        let mut error_count = 0;

        for device in &rolladen_devices {
            match self.client.send_command(&device.uuid, &action).await {
                Ok(_) => {
                    results.push(format!("✅ {}: {}", device.name, action));
                    success_count += 1;
                }
                Err(e) => {
                    results.push(format!("❌ {}: failed ({})", device.name, e));
                    error_count += 1;
                }
            }
        }

        let summary = format!(
            "Controlled {} rolladen/blinds - {} successful, {} failed\n\nDetails:\n{}",
            rolladen_devices.len(),
            success_count,
            error_count,
            results.join("\n")
        );

        Ok(CallToolResult::success(vec![Content::text(summary)]))
    }

    /// Control rolladen in a specific room
    pub async fn control_room_rolladen(
        &self,
        room_name: String,
        action: String,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let devices = self.context.devices.read().await;
        let rooms = self.context.rooms.read().await;

        // Find room by name
        let room_uuid = rooms
            .iter()
            .find(|(_, room)| room.name.to_lowercase() == room_name.to_lowercase())
            .map(|(uuid, _)| uuid.clone());

        let room_uuid = match room_uuid {
            Some(uuid) => uuid,
            None => return Ok(OptimizedResponses::room_not_found(&room_name)),
        };
        let rolladen_devices: Vec<_> = devices
            .values()
            .filter(|device| {
                device.device_type == "Jalousie" && (device.room.as_ref() == Some(&room_uuid))
            })
            .collect();

        if rolladen_devices.is_empty() {
            return Ok(OptimizedResponses::empty_blinds(Some(&room_name)));
        }

        let mut results = Vec::new();
        let mut success_count = 0;
        let mut error_count = 0;

        for device in &rolladen_devices {
            match self.client.send_command(&device.uuid, &action).await {
                Ok(_) => {
                    results.push(format!("✅ {}: {}", device.name, action));
                    success_count += 1;
                }
                Err(e) => {
                    results.push(format!("❌ {}: failed ({})", device.name, e));
                    error_count += 1;
                }
            }
        }

        let summary = format!(
            "Controlled {} rolladen/blinds in '{}' - {} successful, {} failed\n\nDetails:\n{}",
            rolladen_devices.len(),
            room_name,
            success_count,
            error_count,
            results.join("\n")
        );

        Ok(CallToolResult::success(vec![Content::text(summary)]))
    }

    /// Control all lights in the system
    pub async fn control_all_lights(
        &self,
        action: String,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let devices = self.context.devices.read().await;
        let light_devices: Vec<_> = devices
            .values()
            .filter(|device| {
                device.category == "lighting"
                    || device.device_type == "Switch"
                    || device.device_type == "Dimmer"
            })
            .collect();

        if light_devices.is_empty() {
            return Ok(OptimizedResponses::empty_lights(Some("system")));
        }

        let mut results = Vec::new();
        let mut success_count = 0;
        let mut error_count = 0;

        for device in &light_devices {
            match self.client.send_command(&device.uuid, &action).await {
                Ok(_) => {
                    results.push(format!("✅ {}: {}", device.name, action));
                    success_count += 1;
                }
                Err(e) => {
                    results.push(format!("❌ {}: failed ({})", device.name, e));
                    error_count += 1;
                }
            }
        }

        let summary = format!(
            "Controlled {} lights - {} successful, {} failed\n\nDetails:\n{}",
            light_devices.len(),
            success_count,
            error_count,
            results.join("\n")
        );

        Ok(CallToolResult::success(vec![Content::text(summary)]))
    }

    /// Control lights in a specific room
    pub async fn control_room_lights(
        &self,
        room_name: String,
        action: String,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let devices = self.context.devices.read().await;
        let rooms = self.context.rooms.read().await;

        // Find room by name
        let room_uuid = rooms
            .iter()
            .find(|(_, room)| room.name.to_lowercase() == room_name.to_lowercase())
            .map(|(uuid, _)| uuid.clone());

        let room_uuid = match room_uuid {
            Some(uuid) => uuid,
            None => return Ok(OptimizedResponses::room_not_found(&room_name)),
        };
        let light_devices: Vec<_> = devices
            .values()
            .filter(|device| {
                (device.category == "lighting"
                    || device.device_type == "Switch"
                    || device.device_type == "Dimmer")
                    && (device.room.as_ref() == Some(&room_uuid))
            })
            .collect();

        if light_devices.is_empty() {
            return Ok(OptimizedResponses::empty_lights(Some(&room_name)));
        }

        let mut results = Vec::new();
        let mut success_count = 0;
        let mut error_count = 0;

        for device in &light_devices {
            match self.client.send_command(&device.uuid, &action).await {
                Ok(_) => {
                    results.push(format!("✅ {}: {}", device.name, action));
                    success_count += 1;
                }
                Err(e) => {
                    results.push(format!("❌ {}: failed ({})", device.name, e));
                    error_count += 1;
                }
            }
        }

        let summary = format!(
            "Controlled {} lights in '{}' - {} successful, {} failed\n\nDetails:\n{}",
            light_devices.len(),
            room_name,
            success_count,
            error_count,
            results.join("\n")
        );

        Ok(CallToolResult::success(vec![Content::text(summary)]))
    }

    /// Get state history for a specific sensor
    pub async fn get_sensor_state_history(
        &self,
        uuid: String,
        _limit: Option<usize>,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{sensors, ToolContext};

        // Create a sensor state logger (normally this would be persistent)
        let logger = Arc::new(SensorStateLogger::new(std::path::PathBuf::from(
            "sensor_history.json",
        )));

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let response = sensors::get_sensor_state_history(tool_context, uuid, Some(logger)).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(format!(
                "Error: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))]))
        }
    }

    /// Get recent sensor changes across all sensors
    pub async fn get_recent_sensor_changes(
        &self,
        limit: Option<usize>,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{sensors, ToolContext};

        // Create a sensor state logger (normally this would be persistent)
        let logger = Arc::new(SensorStateLogger::new(std::path::PathBuf::from(
            "sensor_history.json",
        )));

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let response = sensors::get_recent_sensor_changes(tool_context, limit, Some(logger)).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(format!(
                "Error: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))]))
        }
    }

    /// Discover all devices in the system
    pub async fn discover_all_devices(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let devices = self.context.devices.read().await;
        let rooms = self.context.rooms.read().await;
        let capabilities = self.context.capabilities.read().await;

        let device_list: Vec<_> = devices
            .values()
            .map(|device| {
                let room_name = device
                    .room
                    .as_ref()
                    .and_then(|room_uuid| rooms.get(room_uuid))
                    .map(|room| room.name.clone())
                    .unwrap_or_else(|| "No Room".to_string());

                serde_json::json!({
                    "uuid": device.uuid,
                    "name": device.name,
                    "type": device.device_type,
                    "category": device.category,
                    "room": room_name
                })
            })
            .collect();

        let result = serde_json::json!({
            "total_devices": devices.len(),
            "system_capabilities": {
                "lighting": capabilities.light_count,
                "blinds": capabilities.blind_count,
                "sensors": capabilities.sensor_count,
                "climate": capabilities.climate_count
            },
            "devices": device_list
        });

        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Get devices filtered by type
    pub async fn get_devices_by_type(
        &self,
        device_type_filter: Option<String>,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let devices = self.context.devices.read().await;
        let rooms = self.context.rooms.read().await;

        if let Some(filter_type) = device_type_filter {
            let filtered_devices: Vec<_> = devices
                .values()
                .filter(|device| {
                    device
                        .device_type
                        .to_lowercase()
                        .contains(&filter_type.to_lowercase())
                })
                .map(|device| {
                    let room_name = device
                        .room
                        .as_ref()
                        .and_then(|room_uuid| rooms.get(room_uuid))
                        .map(|room| room.name.clone())
                        .unwrap_or_else(|| "No Room".to_string());

                    serde_json::json!({
                        "uuid": device.uuid,
                        "name": device.name,
                        "type": device.device_type,
                        "category": device.category,
                        "room": room_name
                    })
                })
                .collect();

            let result = serde_json::json!({
                "filter": filter_type,
                "count": filtered_devices.len(),
                "devices": filtered_devices
            });

            let content =
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            // Show available device types
            let mut device_types = std::collections::HashMap::new();
            for device in devices.values() {
                *device_types.entry(device.device_type.clone()).or_insert(0) += 1;
            }

            let mut type_list: Vec<_> = device_types.into_iter().collect();
            type_list.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending

            let result = serde_json::json!({
                "available_types": type_list,
                "note": "Use device_type parameter to filter by specific type"
            });

            let content =
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
            Ok(CallToolResult::success(vec![Content::text(content)]))
        }
    }

    /// Get devices filtered by category with pagination
    pub async fn get_devices_by_category(
        &self,
        category: String,
        limit: Option<usize>,
        _include_state: bool,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{devices::get_devices_by_category, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let response = get_devices_by_category(tool_context, category, limit).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(format!(
                "Error: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))]))
        }
    }

    /// Get current status/positions of all blinds/rolladen
    pub async fn get_all_blinds_status(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{devices::get_all_blinds_status, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let response = get_all_blinds_status(tool_context).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(format!(
                "Error: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))]))
        }
    }

    /// Get available system capabilities
    pub async fn get_available_capabilities(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{devices::get_available_capabilities, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let response = get_available_capabilities(tool_context).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(format!(
                "Error: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))]))
        }
    }

    /// Get all categories overview
    pub async fn get_all_categories_overview(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{devices::get_all_categories_overview, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let response = get_all_categories_overview(tool_context).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(format!(
                "Error: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))]))
        }
    }

    /// Get audio zones and their status
    pub async fn get_audio_zones(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let context = crate::tools::ToolContext::new(self.client.clone(), self.context.clone());

        let result = crate::tools::audio::get_audio_zones(context).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Control an audio zone
    pub async fn control_audio_zone(
        &self,
        zone_name: String,
        action: String,
        value: Option<f64>,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let context = crate::tools::ToolContext::new(self.client.clone(), self.context.clone());

        let result =
            crate::tools::audio::control_audio_zone(context, zone_name, action, value).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Get available audio sources
    pub async fn get_audio_sources(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let context = crate::tools::ToolContext::new(self.client.clone(), self.context.clone());

        let result = crate::tools::audio::get_audio_sources(context).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Set audio zone volume
    pub async fn set_audio_volume(
        &self,
        zone_name: String,
        volume: f64,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let context = crate::tools::ToolContext::new(self.client.clone(), self.context.clone());

        let result = crate::tools::audio::set_audio_volume(context, zone_name, volume).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Get door/window activity
    pub async fn get_door_window_activity(
        &self,
        hours: Option<u32>,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let context = crate::tools::ToolContext::new(self.client.clone(), self.context.clone());

        let logger = self.context.get_sensor_logger().await;
        let result = crate::tools::sensors::get_door_window_activity(context, hours, logger).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Get logging statistics
    pub async fn get_logging_statistics_tool(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        let context = crate::tools::ToolContext::new(self.client.clone(), self.context.clone());

        let logger = self.context.get_sensor_logger().await;
        let result = crate::tools::sensors::get_logging_statistics(context, logger).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Perform comprehensive health check
    pub async fn get_health_check(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        match self.health_checker.check_health().await {
            Ok(report) => {
                // Add resource health information
                let resource_health = self.resource_monitor.health_check().await;
                let resource_usage = self.resource_monitor.get_usage().await;

                // Add resource health to the report as additional info
                let resource_info = serde_json::json!({
                    "resource_health": {
                        "healthy": resource_health.healthy,
                        "memory_percent": resource_health.memory_percent,
                        "cpu_percent": resource_health.cpu_percent,
                        "request_utilization": resource_health.request_utilization,
                        "warnings": resource_health.warnings
                    },
                    "resource_usage": {
                        "memory_mb": resource_usage.memory_bytes / (1024 * 1024),
                        "active_requests": resource_usage.active_requests,
                        "total_requests": resource_usage.total_requests,
                        "limit_hits": resource_usage.limit_hits,
                        "avg_request_duration_ms": resource_usage.avg_request_duration.as_millis()
                    }
                });

                // Convert report to JSON, add resource info, then back to report
                let mut report_json =
                    serde_json::to_value(&report).unwrap_or(serde_json::Value::Null);
                if let serde_json::Value::Object(ref mut map) = report_json {
                    map.insert("resources".to_string(), resource_info);
                }

                let content =
                    serde_json::to_string_pretty(&report_json).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Health check failed: {}",
                e
            ))])),
        }
    }

    /// Get basic health status (lightweight)
    pub async fn get_health_status(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        // Quick health check using basic connectivity
        match self.client.health_check().await {
            Ok(is_healthy) => {
                let status = if is_healthy { "healthy" } else { "unhealthy" };

                // Get connection pool health if using HTTP client
                let pool_health = if let Some(http_client) =
                    self.client
                        .as_any()
                        .downcast_ref::<crate::client::http_client::LoxoneHttpClient>()
                {
                    let health = http_client.pool_health().await;
                    Some(serde_json::json!({
                        "healthy": health.healthy,
                        "utilization": health.utilization,
                        "queue_pressure": health.queue_pressure,
                        "error_rate": health.error_rate,
                        "active_connections": health.active_connections,
                        "idle_connections": health.idle_connections
                    }))
                } else {
                    None
                };

                // Get resource monitor status
                let resource_health = self.resource_monitor.health_check().await;

                let result = serde_json::json!({
                    "status": status,
                    "is_operational": is_healthy,
                    "timestamp": chrono::Utc::now(),
                    "basic_check": true,
                    "connection_pool": pool_health,
                    "resources": {
                        "healthy": resource_health.healthy,
                        "memory_percent": resource_health.memory_percent,
                        "cpu_percent": resource_health.cpu_percent,
                        "request_utilization": resource_health.request_utilization
                    }
                });

                let content =
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Err(e) => {
                let result = serde_json::json!({
                    "status": "critical",
                    "is_operational": false,
                    "error": e.to_string(),
                    "timestamp": chrono::Utc::now(),
                    "basic_check": true
                });

                let content =
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
        }
    }

    /// Get all door/window sensors status
    pub async fn get_all_door_window_sensors(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{sensors, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let response = sensors::get_all_door_window_sensors(tool_context).await;
        let content = serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(content)]))
        }
    }

    /// Get all temperature sensors and readings
    pub async fn get_temperature_sensors(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{sensors, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let response = sensors::get_temperature_sensors(tool_context).await;
        let content = serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(content)]))
        }
    }

    /// Discover new sensors dynamically
    pub async fn discover_new_sensors(
        &self,
        duration_seconds: Option<u64>,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{sensors, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let response = sensors::discover_new_sensors(tool_context, duration_seconds).await;
        let content = serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(content)]))
        }
    }

    /// List discovered sensors with optional filtering
    pub async fn list_discovered_sensors(
        &self,
        sensor_type: Option<String>,
        room: Option<String>,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{sensors, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let response = sensors::list_discovered_sensors(tool_context, sensor_type, room).await;
        let content = serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(content)]))
        }
    }

    /// Public method to call tools for HTTP transport
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, String> {
        // Create request context for tracking
        let req_ctx = McpRequestContext::new(tool_name.to_string());
        let _span = RequestTracker::create_span(&req_ctx);

        // Check rate limits - using tool name as client ID for HTTP transport
        let client_id = format!("http-client-{}", tool_name);
        let rate_limit_result = self.rate_limiter.check_composite(&client_id, None).await;

        match rate_limit_result {
            RateLimitResult::Limited { reset_at: _ } => {
                warn!(
                    client_id = client_id,
                    tool_name = tool_name,
                    "HTTP request rate limited"
                );
                return Err(format!(
                    "Rate limit exceeded for tool '{}'. Please try again in a few seconds.",
                    tool_name
                ));
            }
            RateLimitResult::AllowedBurst => {
                warn!(
                    client_id = client_id,
                    tool_name = tool_name,
                    "HTTP request allowed using burst capacity"
                );
            }
            RateLimitResult::Allowed => {
                // Normal operation, no logging needed
            }
        }

        // Log request start
        RequestTracker::log_request_start(&req_ctx, &arguments);

        let result = match tool_name {
            // Read-only tools migrated to resources:
            // "list_rooms" → loxone://rooms (use resources/read instead)
            // "get_room_devices" → loxone://rooms/{roomName}/devices (use resources/read instead)
            "control_device" => {
                let device = arguments
                    .get("device")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing device parameter")?;
                let action = arguments
                    .get("action")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing action parameter")?;
                let room = arguments
                    .get("room")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                match self
                    .control_device_enhanced(device.to_string(), action.to_string(), room)
                    .await
                {
                    Ok(result) => self.convert_tool_result(result),
                    Err(e) => Err(format!("Failed to control device: {}", e)),
                }
            }
            "control_all_rolladen" => {
                let action = arguments
                    .get("action")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing action parameter")?;
                match self.control_all_rolladen(action.to_string()).await {
                    Ok(result) => self.convert_tool_result(result),
                    Err(e) => Err(format!("Failed to control all rolladen: {}", e)),
                }
            }
            "control_room_rolladen" => {
                let room = arguments
                    .get("room")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing room parameter")?;
                let action = arguments
                    .get("action")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing action parameter")?;
                match self
                    .control_room_rolladen(room.to_string(), action.to_string())
                    .await
                {
                    Ok(result) => self.convert_tool_result(result),
                    Err(e) => Err(format!("Failed to control room rolladen: {}", e)),
                }
            }
            "control_all_lights" => {
                let action = arguments
                    .get("action")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing action parameter")?;
                match self.control_all_lights(action.to_string()).await {
                    Ok(result) => self.convert_tool_result(result),
                    Err(e) => Err(format!("Failed to control all lights: {}", e)),
                }
            }
            "control_room_lights" => {
                let room = arguments
                    .get("room")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing room parameter")?;
                let action = arguments
                    .get("action")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing action parameter")?;
                match self
                    .control_room_lights(room.to_string(), action.to_string())
                    .await
                {
                    Ok(result) => self.convert_tool_result(result),
                    Err(e) => Err(format!("Failed to control room lights: {}", e)),
                }
            }
            // "discover_all_devices" → loxone://devices/all
            // "get_devices_by_type" → loxone://devices/type/{type}
            // "get_system_status" → loxone://system/status
            // "get_audio_zones" → loxone://audio/zones
            "control_audio_zone" => {
                let zone_name = arguments
                    .get("zone_name")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing zone_name parameter")?;
                let action = arguments
                    .get("action")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing action parameter")?;
                let value = arguments.get("value").and_then(|v| v.as_f64());
                match self
                    .control_audio_zone(zone_name.to_string(), action.to_string(), value)
                    .await
                {
                    Ok(result) => self.convert_tool_result(result),
                    Err(e) => Err(format!("Failed to control audio zone: {}", e)),
                }
            }
            // "get_audio_sources" → loxone://audio/sources
            "set_audio_volume" => {
                let zone_name = arguments
                    .get("zone_name")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing zone_name parameter")?;
                let volume = arguments
                    .get("volume")
                    .and_then(|v| v.as_f64())
                    .ok_or("Missing volume parameter")?;
                match self.set_audio_volume(zone_name.to_string(), volume).await {
                    Ok(result) => self.convert_tool_result(result),
                    Err(e) => Err(format!("Failed to set audio volume: {}", e)),
                }
            }
            "get_sensor_state_history" => {
                let uuid = arguments
                    .get("uuid")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing uuid parameter")?;
                match self.get_sensor_state_history(uuid.to_string(), None).await {
                    Ok(result) => self.convert_tool_result(result),
                    Err(e) => Err(format!("Failed to get sensor state history: {}", e)),
                }
            }
            "get_recent_sensor_changes" => {
                let limit = arguments
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                match self.get_recent_sensor_changes(limit).await {
                    Ok(result) => self.convert_tool_result(result),
                    Err(e) => Err(format!("Failed to get recent sensor changes: {}", e)),
                }
            }
            "get_door_window_activity" => {
                let hours = arguments
                    .get("hours")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32);
                match self.get_door_window_activity(hours).await {
                    Ok(result) => self.convert_tool_result(result),
                    Err(e) => Err(format!("Failed to get door/window activity: {}", e)),
                }
            }
            "get_logging_statistics" => match self.get_logging_statistics_tool().await {
                Ok(result) => self.convert_tool_result(result),
                Err(e) => Err(format!("Failed to get logging statistics: {}", e)),
            },
            "get_health_check" => match self.get_health_check().await {
                Ok(result) => self.convert_tool_result(result),
                Err(e) => Err(format!("Failed to perform health check: {}", e)),
            },
            "get_health_status" => match self.get_health_status().await {
                Ok(result) => self.convert_tool_result(result),
                Err(e) => Err(format!("Failed to get health status: {}", e)),
            },
            _ => Err(format!("Unknown tool: {}", tool_name)),
        };

        // Log request completion
        match &result {
            Ok(_) => {
                RequestTracker::log_request_end(&req_ctx, true, None);
                RequestTracker::log_if_slow(&req_ctx, 1000); // Warn if > 1 second
            }
            Err(e) => {
                // Convert string error to LoxoneError for logging
                let loxone_error = crate::error::LoxoneError::invalid_input(e.to_string());
                RequestTracker::log_request_end(&req_ctx, false, Some(&loxone_error));
            }
        }

        result
    }

    /// Convert CallToolResult to JSON
    fn convert_tool_result(
        &self,
        result: CallToolResult,
    ) -> std::result::Result<serde_json::Value, String> {
        // Content is an opaque type, we need to serialize it
        // For now, we'll extract text by converting to JSON and parsing
        let content_json = serde_json::to_value(&result.content)
            .map_err(|e| format!("Failed to serialize content: {}", e))?;

        Ok(serde_json::json!({
            "content": content_json,
            "is_error": result.is_error
        }))
    }

    // Workflow Tool Handlers

    /// Create a new workflow
    pub async fn create_workflow(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{workflows, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let params: workflows::CreateWorkflowParams =
            serde_json::from_value(arguments).map_err(|e| {
                mcp_foundation::Error::invalid_params(format!("Invalid parameters: {}", e))
            })?;

        let response = workflows::create_workflow(tool_context, params).await;

        match response {
            Ok(data) => {
                let content =
                    serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Err(e) => {
                let error_content = serde_json::json!({"error": e.to_string()});
                let content = serde_json::to_string_pretty(&error_content)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::error(vec![Content::text(content)]))
            }
        }
    }

    /// Execute a demo workflow
    pub async fn execute_workflow_demo(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{workflows, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let params: workflows::ExecuteWorkflowParams =
            serde_json::from_value(arguments).map_err(|e| {
                mcp_foundation::Error::invalid_params(format!("Invalid parameters: {}", e))
            })?;

        let response = workflows::execute_workflow_demo(tool_context, params).await;

        match response {
            Ok(data) => {
                let content =
                    serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Err(e) => {
                let error_content = serde_json::json!({"error": e.to_string()});
                let content = serde_json::to_string_pretty(&error_content)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::error(vec![Content::text(content)]))
            }
        }
    }

    /// List predefined workflows
    pub async fn list_predefined_workflows(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{workflows, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());
        let params = workflows::ListPredefinedWorkflowsParams {};

        let response = workflows::list_predefined_workflows(tool_context, params).await;

        match response {
            Ok(data) => {
                let content =
                    serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Err(e) => {
                let error_content = serde_json::json!({"error": e.to_string()});
                let content = serde_json::to_string_pretty(&error_content)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::error(vec![Content::text(content)]))
            }
        }
    }

    /// Get workflow examples
    pub async fn get_workflow_examples(
        &self,
    ) -> std::result::Result<CallToolResult, mcp_foundation::Error> {
        use crate::tools::{workflows, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());

        let response = workflows::get_workflow_examples(tool_context).await;

        match response {
            Ok(data) => {
                let content =
                    serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Err(e) => {
                let error_content = serde_json::json!({"error": e.to_string()});
                let content = serde_json::to_string_pretty(&error_content)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::error(vec![Content::text(content)]))
            }
        }
    }
}
