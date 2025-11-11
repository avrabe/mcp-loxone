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

// Use framework types instead of legacy mcp_foundation
use pulseengine_mcp_protocol::{CallToolResult, Content};
use std::sync::Arc;
use tracing::warn;


impl LoxoneMcpServer {
    /// List all rooms in the Loxone system with device counts
    pub async fn list_rooms(&self) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
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
        Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
    }

    /// Get devices in a specific room
    pub async fn get_room_devices(
        &self,
        room_name: String,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        let devices = self.context.devices.read().await;
        let room_devices: Vec<String> = devices
            .values()
            .filter(|device| device.room.as_ref() == Some(&room_name))
            .map(|device| format!("{} ({})", device.name, device.device_type))
            .collect();

        let content =
            serde_json::to_string_pretty(&room_devices).unwrap_or_else(|_| "[]".to_string());
        Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
    }

    /// Control a specific device
    pub async fn control_device(
        &self,
        device_id: String,
        action: String,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        match self.client.send_command(&device_id, &action).await {
            Ok(_) => Ok(CallToolResult {
                content: vec![Content::text(format!(
                    "Successfully executed {} on device {}",
                    action, device_id
                ))],
                is_error: Some(false),
                structured_content: None,
                _meta: None,  // v0.13.0 new field
            }),
            Err(e) => Ok(CallToolResult {
                content: vec![Content::text(format!(
                    "Failed to control device: {}",
                    e
                ))],
                is_error: Some(true),
                structured_content: None,
                _meta: None,  // v0.13.0 new field
            }),
        }
    }

    /// Get overall system status
    pub async fn get_system_status(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
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
                Ok(CallToolResult {
                    content: vec![Content::text(content)],
                    is_error: Some(false),
                    structured_content: None,
                    _meta: None,  // v0.13.0 new field
                })
            }
            Ok(false) => Ok(CallToolResult {
                content: vec![Content::text(
                    "⚠️ Loxone system is online but may have issues".to_string(),
                )],
                is_error: Some(false),
                structured_content: None,
                _meta: None,  // v0.13.0 new field
            }),
            Err(e) => Ok(CallToolResult {
                content: vec![Content::text(format!(
                    "❌ System check failed: {}",
                    e
                ))],
                is_error: Some(true),
                structured_content: None,
                _meta: None,  // v0.13.0 new field
            }),
        }
    }

    /// Enhanced get_room_devices with device type filtering
    pub async fn get_room_devices_enhanced(
        &self,
        room_name: String,
        device_type_filter: Option<String>,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
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
        Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
    }

    /// Enhanced control_device that accepts device name or UUID
    pub async fn control_device_enhanced(
        &self,
        device: String,
        action: String,
        room_hint: Option<String>,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
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
                    Ok(CallToolResult {
                        content: vec![Content::text(content)],
                        is_error: Some(false),
                        structured_content: None,
                        _meta: None,  // v0.13.0 new field
                    })
                }
                Err(e) => Ok(CallToolResult {
                    content: vec![Content::text(format!(
                        "Failed to control device {}: {}",
                        device_obj.name, e
                    ))],
                    is_error: Some(true),
                    structured_content: None,
                    _meta: None,  // v0.13.0 new field
                }),
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
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{devices::control_multiple_devices, ToolContext};

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let response = control_multiple_devices(tool_context, devices, action).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult {
                content: vec![Content::text(content)],
                is_error: Some(false),
                structured_content: None,
                _meta: None,  // v0.13.0 new field
            })
        } else {
            Ok(CallToolResult {
                content: vec![Content::text(format!(
                    "Error: {}",
                    response
                        .message
                        .unwrap_or_else(|| "Unknown error".to_string())
                ))],
                is_error: Some(true),
                structured_content: None,
                _meta: None,  // v0.13.0 new field
            })
        }
    }

    /// Unified rolladen/blinds control with scope-based targeting
    pub async fn control_rolladen_unified(
        &self,
        scope: String,
        target: Option<String>,
        action: String,
        position: Option<u8>,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        // Create tool context
        let tool_context = crate::tools::ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        // Call the unified rolladen control function
        let tool_response = crate::tools::rolladen::control_rolladen_unified(
            tool_context,
            scope,
            target,
            action,
            position,
        ).await;

        // Convert ToolResponse to CallToolResult
        match tool_response.status.as_str() {
            "success" => {
                let content = serde_json::to_string_pretty(&tool_response.data)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult {
                    content: vec![Content::text(content)],
                    is_error: Some(false),
                    structured_content: None,
                    _meta: None,  // v0.13.0 new field
                })
            }
            _ => {
                let error_msg = tool_response.message.unwrap_or_else(|| "Unknown error".to_string());
                Err(pulseengine_mcp_protocol::Error::invalid_params(error_msg))
            }
        }
    }

    /// Discover all rolladen/blinds capabilities in the system
    pub async fn discover_rolladen_capabilities(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        // Create tool context
        let tool_context = crate::tools::ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        // Call the discovery function
        let tool_response = crate::tools::rolladen::discover_rolladen_capabilities(tool_context).await;

        // Convert ToolResponse to CallToolResult
        match tool_response.status.as_str() {
            "success" => {
                let content = serde_json::to_string_pretty(&tool_response.data)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult {
                    content: vec![Content::text(content)],
                    is_error: Some(false),
                    structured_content: None,
                    _meta: None,  // v0.13.0 new field
                })
            }
            _ => {
                let error_msg = tool_response.message.unwrap_or_else(|| "Unknown error".to_string());
                Err(pulseengine_mcp_protocol::Error::invalid_params(error_msg))
            }
        }
    }

    /// Unified lighting control with scope-based targeting
    pub async fn control_lights_unified(
        &self,
        scope: String,
        target: Option<String>,
        action: String,
        brightness: Option<u8>,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        // Create tool context
        let tool_context = crate::tools::ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        // Call the unified lighting control function
        let tool_response = crate::tools::lighting::control_lights_unified(
            tool_context,
            scope,
            target,
            action,
            brightness,
        ).await;

        // Convert ToolResponse to CallToolResult
        match tool_response.status.as_str() {
            "success" => {
                let content = serde_json::to_string_pretty(&tool_response.data)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult {
                    content: vec![Content::text(content)],
                    is_error: Some(false),
                    structured_content: None,
                    _meta: None,  // v0.13.0 new field
                })
            }
            _ => {
                let error_msg = tool_response.message.unwrap_or_else(|| "Unknown error".to_string());
                Err(pulseengine_mcp_protocol::Error::invalid_params(error_msg))
            }
        }
    }

    /// Discover all lighting capabilities in the system
    pub async fn discover_lighting_capabilities(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        // Create tool context
        let tool_context = crate::tools::ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        // Call the discovery function
        let tool_response = crate::tools::lighting::discover_lighting_capabilities(tool_context).await;

        // Convert ToolResponse to CallToolResult
        match tool_response.status.as_str() {
            "success" => {
                let content = serde_json::to_string_pretty(&tool_response.data)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult {
                    content: vec![Content::text(content)],
                    is_error: Some(false),
                    structured_content: None,
                    _meta: None,  // v0.13.0 new field
                })
            }
            _ => {
                let error_msg = tool_response.message.unwrap_or_else(|| "Unknown error".to_string());
                Err(pulseengine_mcp_protocol::Error::invalid_params(error_msg))
            }
        }
    }

    /// Get state history for a specific sensor
    pub async fn get_sensor_state_history(
        &self,
        uuid: String,
        _limit: Option<usize>,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{sensors, ToolContext};

        // Create a sensor state logger (normally this would be persistent)
        let logger = Arc::new(SensorStateLogger::new(std::path::PathBuf::from(
            "sensor_history.json",
        )));

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let response = sensors::get_sensor_state_history(tool_context, uuid, Some(logger)).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult {
                content: vec![Content::text(content)],
                is_error: Some(false),
                structured_content: None,
                _meta: None,  // v0.13.0 new field
            })
        } else {
            Ok(CallToolResult {
                content: vec![Content::text(format!(
                    "Error: {}",
                    response
                        .message
                        .unwrap_or_else(|| "Unknown error".to_string())
                ))],
                is_error: Some(true),
                structured_content: None,
                _meta: None,  // v0.13.0 new field
            })
        }
    }

    /// Get recent sensor changes across all sensors
    pub async fn get_recent_sensor_changes(
        &self,
        limit: Option<usize>,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{sensors, ToolContext};

        // Create a sensor state logger (normally this would be persistent)
        let logger = Arc::new(SensorStateLogger::new(std::path::PathBuf::from(
            "sensor_history.json",
        )));

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let response = sensors::get_recent_sensor_changes(tool_context, limit, Some(logger)).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult {
                content: vec![Content::text(content)],
                is_error: Some(false),
                structured_content: None,
                _meta: None,  // v0.13.0 new field
            })
        } else {
            Ok(CallToolResult {
                content: vec![Content::text(format!(
                    "Error: {}",
                    response
                        .message
                        .unwrap_or_else(|| "Unknown error".to_string())
                ))],
                is_error: Some(true),
                structured_content: None,
                _meta: None,  // v0.13.0 new field
            })
        }
    }

    /// Discover all devices in the system
    pub async fn discover_all_devices(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
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
        Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
    }

    /// Get devices filtered by type
    pub async fn get_devices_by_type(
        &self,
        device_type_filter: Option<String>,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
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
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
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
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        }
    }

    /// Get devices filtered by category with pagination
    pub async fn get_devices_by_category(
        &self,
        category: String,
        limit: Option<usize>,
        _include_state: bool,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{devices::get_devices_by_category, ToolContext};

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let response = get_devices_by_category(tool_context, category, limit).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        } else {
            Ok(CallToolResult {
            content: vec![Content::text(format!(
                "Error: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))],
            is_error: Some(true),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        }
    }

    /// Get available system capabilities
    pub async fn get_available_capabilities(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{devices::get_available_capabilities, ToolContext};

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let response = get_available_capabilities(tool_context).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        } else {
            Ok(CallToolResult {
            content: vec![Content::text(format!(
                "Error: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))],
            is_error: Some(true),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        }
    }

    /// Get all categories overview
    pub async fn get_all_categories_overview(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{devices::get_all_categories_overview, ToolContext};

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let response = get_all_categories_overview(tool_context).await;

        let content =
            serde_json::to_string_pretty(&response.data).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        } else {
            Ok(CallToolResult {
            content: vec![Content::text(format!(
                "Error: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))],
            is_error: Some(true),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        }
    }

    /// Get audio zones and their status
    pub async fn get_audio_zones(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        let context = crate::tools::ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let result = crate::tools::audio::get_audio_zones(context).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
    }

    /// Control an audio zone
    pub async fn control_audio_zone(
        &self,
        zone_name: String,
        action: String,
        value: Option<f64>,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        let context = crate::tools::ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let result =
            crate::tools::audio::control_audio_zone(context, zone_name, action, value).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
    }

    /// Get available audio sources
    pub async fn get_audio_sources(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        let context = crate::tools::ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let result = crate::tools::audio::get_audio_sources(context).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
    }

    /// Set audio zone volume
    pub async fn set_audio_volume(
        &self,
        zone_name: String,
        volume: f64,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        let context = crate::tools::ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let result = crate::tools::audio::set_audio_volume(context, zone_name, volume).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
    }

    /// Get door/window activity
    pub async fn get_door_window_activity(
        &self,
        hours: Option<u32>,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        let context = crate::tools::ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let logger = self.context.get_sensor_logger().await;
        let result = crate::tools::sensors::get_door_window_activity(context, hours, logger).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
    }

    /// Get logging statistics
    pub async fn get_logging_statistics_tool(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        let context = crate::tools::ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let logger = self.context.get_sensor_logger().await;
        let result = crate::tools::sensors::get_logging_statistics(context, logger).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
    }

    /// Perform comprehensive health check
    pub async fn get_health_check(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
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
                Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
            }
            Err(e) => Ok(CallToolResult {
            content: vec![Content::text(format!(
                "Health check failed: {}",
                e
            ))],
            is_error: Some(true),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        }),
        }
    }

    /// Get basic health status (lightweight)
    pub async fn get_health_status(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
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
                Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
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
                Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
            }
        }
    }

    /// Get all door/window sensors status
    pub async fn get_all_door_window_sensors(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{sensors, ToolContext};

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let response = sensors::get_all_door_window_sensors(tool_context).await;
        let content = serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        } else {
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(true),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        }
    }

    /// Get all temperature sensors and readings
    pub async fn get_temperature_sensors(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{sensors, ToolContext};

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let response = sensors::get_temperature_sensors(tool_context).await;
        let content = serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        } else {
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(true),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        }
    }

    /// Discover new sensors dynamically
    pub async fn discover_new_sensors(
        &self,
        duration_seconds: Option<u64>,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{sensors, ToolContext};

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let response = sensors::discover_new_sensors(tool_context, duration_seconds).await;
        let content = serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        } else {
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(true),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        }
    }

    /// List discovered sensors with optional filtering
    pub async fn list_discovered_sensors(
        &self,
        sensor_type: Option<String>,
        room: Option<String>,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{sensors, ToolContext};

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let response = sensors::list_discovered_sensors(tool_context, sensor_type, room).await;
        let content = serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string());

        if response.status == "success" {
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
        } else {
            Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(true),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
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
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{workflows, ToolContext};

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let params: workflows::CreateWorkflowParams =
            serde_json::from_value(arguments).map_err(|e| {
                pulseengine_mcp_protocol::Error::invalid_params(format!("Invalid parameters: {}", e))
            })?;

        let response = workflows::create_workflow(tool_context, params).await;

        match response {
            Ok(data) => {
                let content =
                    serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
            }
            Err(e) => {
                let error_content = serde_json::json!({"error": e.to_string()});
                let content = serde_json::to_string_pretty(&error_content)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(true),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
            }
        }
    }

    /// Execute a demo workflow
    pub async fn execute_workflow_demo(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{workflows, ToolContext};

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let params: workflows::ExecuteWorkflowParams =
            serde_json::from_value(arguments).map_err(|e| {
                pulseengine_mcp_protocol::Error::invalid_params(format!("Invalid parameters: {}", e))
            })?;

        let response = workflows::execute_workflow_demo(tool_context, params).await;

        match response {
            Ok(data) => {
                let content =
                    serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
            }
            Err(e) => {
                let error_content = serde_json::json!({"error": e.to_string()});
                let content = serde_json::to_string_pretty(&error_content)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(true),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
            }
        }
    }

    /// List predefined workflows
    pub async fn list_predefined_workflows(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{workflows, ToolContext};

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );
        let params = workflows::ListPredefinedWorkflowsParams {};

        let response = workflows::list_predefined_workflows(tool_context, params).await;

        match response {
            Ok(data) => {
                let content =
                    serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
            }
            Err(e) => {
                let error_content = serde_json::json!({"error": e.to_string()});
                let content = serde_json::to_string_pretty(&error_content)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(true),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
            }
        }
    }

    /// Get workflow examples
    pub async fn get_workflow_examples(
        &self,
    ) -> std::result::Result<CallToolResult, pulseengine_mcp_protocol::Error> {
        use crate::tools::{workflows, ToolContext};

        let tool_context = ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        );

        let response = workflows::get_workflow_examples(tool_context).await;

        match response {
            Ok(data) => {
                let content =
                    serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(false),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
            }
            Err(e) => {
                let error_content = serde_json::json!({"error": e.to_string()});
                let content = serde_json::to_string_pretty(&error_content)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: Some(true),
            structured_content: None,
            _meta: None,  // v0.13.0 new field
        })
            }
        }
    }
}
