//! Tool adapters for converting between legacy Loxone tools and MCP framework
//!
//! This module provides the bridge layer that allows existing Loxone tools
//! to work with the new MCP framework without requiring immediate tool rewrites.

use crate::{
    error::LoxoneError,
    tools::{ToolContext, ToolResponse},
};
use pulseengine_mcp_protocol::{CallToolRequestParam, Content, Tool};
use serde_json::Value;
use std::sync::Arc;

/// Helper to extract parameter from MCP request
pub fn extract_string_param(params: &Option<Value>, name: &str) -> Result<String, LoxoneError> {
    params
        .as_ref()
        .and_then(|p| p.get(name))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| LoxoneError::validation(format!("Missing required parameter: {name}")))
}

/// Helper to extract optional parameter from MCP request
pub fn extract_optional_string_param(params: &Option<Value>, name: &str) -> Option<String> {
    params
        .as_ref()
        .and_then(|p| p.get(name))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Helper to extract optional number parameter from MCP request
pub fn extract_optional_u8_param(params: &Option<Value>, name: &str) -> Option<u8> {
    params
        .as_ref()
        .and_then(|p| p.get(name))
        .and_then(|v| v.as_u64())
        .and_then(|n| if n <= 255 { Some(n as u8) } else { None })
}

/// Convert ToolResponse to Content for MCP framework
pub fn tool_response_to_content(response: ToolResponse) -> Content {
    if response.status == "error" {
        return Content::text(format!(
            "Error: {}",
            response.message.unwrap_or("Unknown error".to_string())
        ));
    }

    let mut result = serde_json::json!({
        "status": response.status,
        "data": response.data,
        "timestamp": response.timestamp
    });

    if let Some(message) = response.message {
        result["message"] = Value::String(message);
    }

    Content::text(
        serde_json::to_string_pretty(&result)
            .unwrap_or_else(|_| "Failed to serialize response".to_string()),
    )
}

/// Legacy function - use create_tool_context_direct instead
/// This function is deprecated and will be removed
#[deprecated(note = "Use create_tool_context_direct instead - framework migration complete")]
pub fn create_tool_context(_: &()) -> ToolContext {
    panic!("Legacy create_tool_context called - use create_tool_context_direct instead");
}

/// Create tool context directly from dependencies (no server wrapper)
pub fn create_tool_context_direct(
    client: &Arc<dyn crate::client::LoxoneClient>,
    context: &Arc<crate::client::ClientContext>,
) -> ToolContext {
    let sensor_registry = Arc::new(crate::services::SensorTypeRegistry::default());
    ToolContext {
        client: client.clone(),
        context: context.clone(),
        value_resolver: Arc::new(crate::services::UnifiedValueResolver::new(
            client.clone(),
            sensor_registry,
        )),
        state_manager: None, // Simplified for framework migration
    }
}

/// Route tool calls to appropriate implementation (framework migration version)
pub async fn handle_tool_call_direct(
    client: &Arc<dyn crate::client::LoxoneClient>,
    context: &Arc<crate::client::ClientContext>,
    params: &CallToolRequestParam,
) -> Result<Content, LoxoneError> {
    let tool_context = create_tool_context_direct(client, context);

    let response = match params.name.as_str() {
        // Control tools
        "control_lighting" => {
            // Parse lighting control parameters
            #[derive(serde::Deserialize)]
            struct LightingParams {
                room: Option<String>,
                action: String,
                brightness: Option<u8>,
            }
            let lighting_params: LightingParams = serde_json::from_value(
                params.arguments.clone().unwrap_or_default(),
            )
            .map_err(|e| LoxoneError::validation(format!("Invalid lighting parameters: {e}")))?;

            let scope = lighting_params.room.unwrap_or_else(|| "all".to_string());
            Ok(crate::tools::lighting::control_lights_unified(
                tool_context,
                scope,
                None, // target - room is used as scope instead
                lighting_params.action,
                lighting_params.brightness,
            )
            .await)
        }
        "control_blinds" => {
            // Parse blinds control parameters
            #[derive(serde::Deserialize)]
            struct BlindsParams {
                room: Option<String>,
                action: String,
                position: Option<u8>,
            }
            let blinds_params: BlindsParams = serde_json::from_value(
                params.arguments.clone().unwrap_or_default(),
            )
            .map_err(|e| LoxoneError::validation(format!("Invalid blinds parameters: {e}")))?;

            let (scope, target) = if let Some(room) = blinds_params.room {
                ("room".to_string(), Some(room))
            } else {
                ("all".to_string(), None)
            };
            Ok(crate::tools::rolladen::control_rolladen_unified(
                tool_context,
                scope,
                target,
                blinds_params.action,
                blinds_params.position,
            )
            .await)
        }
        "control_climate" => {
            // Parse climate control parameters
            #[derive(serde::Deserialize)]
            struct ClimateParams {
                room: Option<String>,
                action: String,
                temperature: Option<f64>,
                mode: Option<String>,
            }
            let climate_params: ClimateParams = serde_json::from_value(
                params.arguments.clone().unwrap_or_default(),
            )
            .map_err(|e| LoxoneError::validation(format!("Invalid climate parameters: {e}")))?;

            match climate_params.action.as_str() {
                "set_temperature" => {
                    let room_name = climate_params.room.ok_or_else(|| {
                        LoxoneError::validation(
                            "room parameter required for set_temperature action".to_string(),
                        )
                    })?;
                    let temperature = climate_params.temperature.ok_or_else(|| {
                        LoxoneError::validation(
                            "temperature parameter required for set_temperature action".to_string(),
                        )
                    })?;
                    Ok(crate::tools::climate::set_room_temperature(
                        tool_context,
                        room_name,
                        temperature,
                    )
                    .await)
                }
                _ => Err(LoxoneError::validation(format!(
                    "Unsupported climate action: {}",
                    climate_params.action
                ))),
            }
        }
        "control_audio" => {
            // Parse audio control parameters
            #[derive(serde::Deserialize)]
            struct AudioParams {
                zone: String,
                action: String,
                volume: Option<f64>,
            }
            let audio_params: AudioParams = serde_json::from_value(
                params.arguments.clone().unwrap_or_default(),
            )
            .map_err(|e| LoxoneError::validation(format!("Invalid audio parameters: {e}")))?;

            Ok(crate::tools::audio::control_audio_zone(
                tool_context,
                audio_params.zone,
                audio_params.action,
                audio_params.volume,
            )
            .await)
        }

        // Monitoring tools
        "get_sensor_data" => {
            // Parse sensor parameters
            #[derive(serde::Deserialize)]
            struct SensorParams {
                sensor_type: Option<String>,
                room: Option<String>,
            }
            let sensor_params: SensorParams = serde_json::from_value(
                params.arguments.clone().unwrap_or_default(),
            )
            .map_err(|e| LoxoneError::validation(format!("Invalid sensor parameters: {e}")))?;

            // For now, use a default sensor ID - ideally this should query by type/room
            let sensor_id = format!(
                "{}_{}",
                sensor_params
                    .sensor_type
                    .unwrap_or_else(|| "all".to_string()),
                sensor_params.room.unwrap_or_else(|| "all".to_string())
            );
            Ok(crate::tools::sensors::get_sensor_details(tool_context, sensor_id).await)
        }
        "monitor_energy" => {
            // Energy optimization takes raw JSON input
            Ok(crate::tools::energy::optimize_energy_usage(
                params.arguments.clone().unwrap_or_default(),
                Arc::new(tool_context),
            )
            .await
            .map(ToolResponse::success)
            .unwrap_or_else(|e| ToolResponse::error(e.to_string())))
        }
        "get_weather_data" => {
            // Weather data now available via resources
            Err(LoxoneError::validation(
                "get_weather_data tool has been deprecated. Use resource: loxone://weather instead"
                    .to_string(),
            ))
        }
        "get_system_status" => {
            // System status now available via resources
            Err(LoxoneError::validation(
                "get_system_status tool has been deprecated. Use resource: loxone://system/status instead".to_string()
            ))
        }
        "test_connection" => {
            // Connection test now available via resources
            Err(LoxoneError::validation(
                "test_connection tool has been deprecated. Use resource: loxone://system/health instead".to_string()
            ))
        }

        // Device management
        "get_device_status" => {
            // Parse device status parameters
            #[derive(serde::Deserialize)]
            struct DeviceParams {
                device_name: Option<String>,
                device_category: Option<String>,
            }
            let device_params: DeviceParams = serde_json::from_value(
                params.arguments.clone().unwrap_or_default(),
            )
            .map_err(|e| LoxoneError::validation(format!("Invalid device parameters: {e}")))?;

            let device = device_params
                .device_name
                .unwrap_or_else(|| "all".to_string());
            Ok(crate::tools::devices::control_device(
                tool_context,
                device,
                "status".to_string(), // Use status action to get device status
            )
            .await)
        }
        "send_custom_command" => {
            // Parse custom command parameters
            #[derive(serde::Deserialize)]
            struct CustomCommandParams {
                device_uuid: String,
                command: String,
            }
            let command_params: CustomCommandParams =
                serde_json::from_value(params.arguments.clone().unwrap_or_default()).map_err(
                    |e| LoxoneError::validation(format!("Invalid custom command parameters: {e}")),
                )?;

            // Use control_device with custom command
            Ok(crate::tools::devices::control_device(
                tool_context,
                command_params.device_uuid,
                command_params.command,
            )
            .await)
        }

        // System tools removed - functionality not available in current modules
        // "get_system_status" => { ... }
        // "test_connection" => { ... }

        // Security tools
        "get_security_status" => {
            // Security functions take raw JSON input
            Ok(crate::tools::security::arm_alarm(
                params.arguments.clone().unwrap_or_default(),
                Arc::new(tool_context),
            )
            .await
            .map(ToolResponse::success)
            .unwrap_or_else(|e| ToolResponse::error(e.to_string())))
        }

        // Workflow tools
        "execute_scene" => {
            // Parse workflow execution parameters
            use crate::tools::workflows::ExecuteWorkflowParams;
            let workflow_params: ExecuteWorkflowParams = serde_json::from_value(
                params.arguments.clone().unwrap_or_default(),
            )
            .map_err(|e| LoxoneError::validation(format!("Invalid workflow parameters: {e}")))?;

            Ok(
                crate::tools::workflows::execute_workflow_demo(tool_context, workflow_params)
                    .await
                    .map(ToolResponse::success)
                    .unwrap_or_else(|e| ToolResponse::error(e.to_string())),
            )
        }
        "create_custom_scene" => {
            // Parse workflow creation parameters
            use crate::tools::workflows::CreateWorkflowParams;
            let workflow_params: CreateWorkflowParams = serde_json::from_value(
                params.arguments.clone().unwrap_or_default(),
            )
            .map_err(|e| {
                LoxoneError::validation(format!("Invalid workflow creation parameters: {e}"))
            })?;

            Ok(
                crate::tools::workflows::create_workflow(tool_context, workflow_params)
                    .await
                    .map(ToolResponse::success)
                    .unwrap_or_else(|e| ToolResponse::error(e.to_string())),
            )
        }

        _ => {
            return Err(LoxoneError::validation(format!(
                "Unknown tool: {}",
                params.name
            )));
        }
    };

    match response {
        Ok(tool_response) => Ok(tool_response_to_content(tool_response)),
        Err(e) => Err(e),
    }
}

/// Generate Tool definitions for all available Loxone tools
pub fn get_all_loxone_tools() -> Vec<Tool> {
    vec![
        // Device Control Tools
        Tool {
            name: "control_lighting".to_string(),
            description: "Control lighting devices in rooms or globally".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "room": {
                        "type": "string",
                        "description": "Room name to control lights in",
                        "x-completion-ref": "room_names_with_lights"
                    },
                    "action": {
                        "type": "string",
                        "enum": ["on", "off", "toggle", "dim"],
                        "description": "Action to perform"
                    },
                    "brightness": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 100,
                        "description": "Brightness level (0-100) for dimming"
                    }
                },
                "required": ["action"]
            }),
        },
        Tool {
            name: "control_blinds".to_string(),
            description: "Control blinds/rolladen in rooms".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "room": {
                        "type": "string",
                        "description": "Room name to control blinds in",
                        "x-completion-ref": "room_names_with_blinds"
                    },
                    "action": {
                        "type": "string",
                        "enum": ["up", "down", "stop", "position"],
                        "description": "Blind control action"
                    },
                    "position": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 100,
                        "description": "Position percentage (0=closed, 100=open)"
                    }
                },
                "required": ["action"]
            }),
        },
        Tool {
            name: "control_climate".to_string(),
            description: "Control climate/heating in rooms".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "room": {
                        "type": "string",
                        "description": "Room name to control climate in",
                        "x-completion-ref": "room_names_with_climate"
                    },
                    "action": {
                        "type": "string",
                        "enum": ["set_temperature", "mode", "boost"],
                        "description": "Climate control action"
                    },
                    "temperature": {
                        "type": "number",
                        "minimum": 10,
                        "maximum": 30,
                        "description": "Target temperature in Celsius"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["auto", "heat", "off"],
                        "description": "Climate control mode"
                    }
                },
                "required": ["action"]
            }),
        },
        Tool {
            name: "control_audio".to_string(),
            description: "Control audio zones and playback".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "zone": {
                        "type": "string",
                        "description": "Audio zone name",
                        "x-completion-ref": "audio_zone_names"
                    },
                    "action": {
                        "type": "string",
                        "enum": ["play", "pause", "stop", "volume", "next", "previous"],
                        "description": "Audio control action"
                    },
                    "volume": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 100,
                        "description": "Volume level (0-100)"
                    }
                },
                "required": ["action"]
            }),
        },
        // Monitoring Tools
        Tool {
            name: "get_sensor_data".to_string(),
            description: "Get current sensor readings from the system".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "sensor_type": {
                        "type": "string",
                        "enum": ["temperature", "humidity", "motion", "door_window", "all"],
                        "description": "Type of sensors to query"
                    },
                    "room": {
                        "type": "string",
                        "description": "Filter by specific room",
                        "x-completion-ref": "room_names"
                    }
                }
            }),
        },
        Tool {
            name: "monitor_energy".to_string(),
            description: "Monitor energy consumption and power usage".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "enum": ["total", "room", "device"],
                        "description": "Scope of energy monitoring"
                    },
                    "room": {
                        "type": "string",
                        "description": "Room name for room-specific monitoring",
                        "x-completion-ref": "room_names"
                    }
                }
            }),
        },
        // Weather tool removed - use resources: loxone://weather instead

        // Device Management Tools
        Tool {
            name: "get_device_status".to_string(),
            description: "Get detailed status of specific devices".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "device_name": {
                        "type": "string",
                        "description": "Name of device to query",
                        "x-completion-ref": "device_names"
                    },
                    "device_category": {
                        "type": "string",
                        "enum": ["lights", "blinds", "climate", "audio", "security"],
                        "description": "Category of devices to query"
                    }
                }
            }),
        },
        Tool {
            name: "send_custom_command".to_string(),
            description: "Send custom commands to Loxone devices".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "device_uuid": {
                        "type": "string",
                        "description": "UUID of target device"
                    },
                    "command": {
                        "type": "string",
                        "description": "Command to send"
                    }
                },
                "required": ["device_uuid", "command"]
            }),
        },
        // System tools removed - functionality not available in current modules
        // get_system_status, test_connection - use resources: loxone://system/status instead

        // Security Tools
        Tool {
            name: "get_security_status".to_string(),
            description: "Get current security system status".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "include_history": {
                        "type": "boolean",
                        "description": "Include recent security events"
                    }
                }
            }),
        },
        // Workflow Tools
        Tool {
            name: "execute_scene".to_string(),
            description: "Execute predefined automation scenes".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "scene_name": {
                        "type": "string",
                        "description": "Name of scene to execute",
                        "x-completion-ref": "scene_names"
                    }
                },
                "required": ["scene_name"]
            }),
        },
        Tool {
            name: "create_custom_scene".to_string(),
            description: "Create custom automation scenes".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "scene_name": {
                        "type": "string",
                        "description": "Name for the new scene"
                    },
                    "actions": {
                        "type": "array",
                        "description": "List of actions to include in scene"
                    }
                },
                "required": ["scene_name", "actions"]
            }),
        },
        // READ-ONLY TOOLS REMOVED: list_rooms, get_room_devices, get_room_overview
        // → Use resources: loxone://rooms, loxone://rooms/{room}/devices, loxone://rooms/{room}/overview
    ]
}

// Legacy function disabled - framework migration complete
// All tool handling now goes through handle_tool_call_direct
