//! Tool adapters for converting between legacy Loxone tools and MCP framework
//!
//! This module provides the bridge layer that allows existing Loxone tools
//! to work with the new MCP framework without requiring immediate tool rewrites.

use serde_json::Value;
use mcp_protocol::{Tool, Content, CallToolRequestParam};
use crate::{
    server::LoxoneMcpServer,
    tools::{ToolContext, ToolResponse},
    error::LoxoneError,
};

/// Helper to extract parameter from MCP request
pub fn extract_string_param(params: &Option<Value>, name: &str) -> Result<String, LoxoneError> {
    params
        .as_ref()
        .and_then(|p| p.get(name))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| LoxoneError::validation(format!("Missing required parameter: {}", name)))
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

    Content::text(serde_json::to_string_pretty(&result).unwrap_or_else(|_| {
        "Failed to serialize response".to_string()
    }))
}

/// Create tool context from Loxone server
pub fn create_tool_context(server: &LoxoneMcpServer) -> ToolContext {
    ToolContext {
        client: server.client.clone(),
        context: server.context.clone(), 
        value_resolver: server.value_resolver.clone(),
        state_manager: server.state_manager.clone(),
    }
}

/// Generate Tool definitions for all available Loxone tools
pub fn get_all_loxone_tools() -> Vec<Tool> {
    vec![
        // Room tools
        Tool {
            name: "list_rooms".to_string(),
            description: "List all available rooms in the Loxone system".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "get_room_devices".to_string(),
            description: "Get all devices in a specific room".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "room_name": {
                        "type": "string",
                        "description": "Name of the room to get devices for"
                    }
                },
                "required": ["room_name"]
            }),
        },
        Tool {
            name: "get_room_overview".to_string(),
            description: "Get comprehensive overview of a room including devices and status".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "room_name": {
                        "type": "string",
                        "description": "Name of the room to get overview for"
                    }
                },
                "required": ["room_name"]
            }),
        },
        
        // Device tools
        Tool {
            name: "discover_all_devices".to_string(),
            description: "Discover all devices in the Loxone system".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "control_device".to_string(),
            description: "Control a specific device (lights, blinds, etc.)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "device_id": {
                        "type": "string",
                        "description": "UUID of the device to control"
                    },
                    "action": {
                        "type": "string",
                        "description": "Action to perform (on, off, toggle, etc.)",
                        "enum": ["on", "off", "toggle", "up", "down", "stop"]
                    },
                    "value": {
                        "type": "number",
                        "description": "Optional value for the action (e.g., brightness level)",
                        "minimum": 0,
                        "maximum": 100
                    }
                },
                "required": ["device_id", "action"]
            }),
        },
        Tool {
            name: "get_devices_by_category".to_string(),
            description: "Get devices filtered by category (lights, blinds, climate, etc.)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "description": "Device category to filter by",
                        "enum": ["lights", "blinds", "climate", "sensors", "audio", "security"]
                    }
                },
                "required": ["category"]
            }),
        },
        
        // Rolladen/Blinds tools
        Tool {
            name: "control_rolladen_unified".to_string(),
            description: "Unified rolladen/blinds control with scope-based targeting".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "description": "Scope of rolladen/blinds control",
                        "enum": ["device", "room", "system", "all"]
                    },
                    "target": {
                        "type": "string",
                        "description": "Target device ID/name or room name (required for device/room scope)"
                    },
                    "action": {
                        "type": "string",
                        "description": "Rolladen/blinds action to perform",
                        "enum": ["up", "down", "stop", "position", "hoch", "runter", "stopp"]
                    },
                    "position": {
                        "type": "integer",
                        "description": "Position percentage (0-100) where 0=fully up, 100=fully down",
                        "minimum": 0,
                        "maximum": 100
                    }
                },
                "required": ["scope", "action"]
            }),
        },
        Tool {
            name: "discover_rolladen_capabilities".to_string(),
            description: "Discover all rolladen/blinds capabilities and devices in the system".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        Tool {
            name: "control_room_rolladen".to_string(),
            description: "Control all rolladen/blinds in a specific room (legacy compatibility)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "room": {
                        "type": "string",
                        "description": "Name of the room"
                    },
                    "action": {
                        "type": "string",
                        "description": "Action to perform: 'up', 'down', or 'stop'"
                    }
                },
                "required": ["room", "action"]
            }),
        },
        Tool {
            name: "control_all_rolladen".to_string(),
            description: "Control all rolladen/blinds in the entire system (legacy compatibility)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "Action to perform: 'up', 'down', or 'stop'"
                    }
                },
                "required": ["action"]
            }),
        },
        Tool {
            name: "control_room_lights".to_string(),
            description: "Control all lights in a specific room (legacy compatibility)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "room": {
                        "type": "string",
                        "description": "Name of the room"
                    },
                    "action": {
                        "type": "string",
                        "description": "Action to perform: 'on' or 'off'"
                    }
                },
                "required": ["room", "action"]
            }),
        },
        Tool {
            name: "control_all_lights".to_string(),
            description: "Control all lights in the entire system (legacy compatibility)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "Action to perform: 'on' or 'off'"
                    }
                },
                "required": ["action"]
            }),
        },
        Tool {
            name: "control_multiple_devices".to_string(),
            description: "Control multiple devices simultaneously (legacy compatibility)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "devices": {
                        "type": "array",
                        "description": "Array of device UUIDs to control",
                        "items": {"type": "string"}
                    },
                    "action": {
                        "type": "string",
                        "description": "Action to perform on all devices"
                    }
                },
                "required": ["devices", "action"]
            }),
        },
        
        // Audio tools
        Tool {
            name: "get_audio_zones".to_string(),
            description: "Get audio system information including zones, sources, and playback status".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        Tool {
            name: "control_audio_zone".to_string(),
            description: "Control an audio zone (play, stop, volume control)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "zone_name": {
                        "type": "string",
                        "description": "Name of the audio zone to control"
                    },
                    "action": {
                        "type": "string",
                        "description": "Action to perform",
                        "enum": ["play", "stop", "pause", "volume", "mute", "unmute", "next", "previous", "start"]
                    },
                    "value": {
                        "type": "number",
                        "description": "Value for volume actions (0-100)",
                        "minimum": 0,
                        "maximum": 100
                    }
                },
                "required": ["zone_name", "action"]
            }),
        },
        Tool {
            name: "get_audio_sources".to_string(),
            description: "Get available audio sources and their status".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        Tool {
            name: "set_audio_volume".to_string(),
            description: "Set volume for an audio zone".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "zone_name": {
                        "type": "string",
                        "description": "Name of the audio zone"
                    },
                    "volume": {
                        "type": "number",
                        "description": "Volume level (0-100)",
                        "minimum": 0,
                        "maximum": 100
                    }
                },
                "required": ["zone_name", "volume"]
            }),
        },
        
        // Lighting tools
        Tool {
            name: "control_lights_unified".to_string(),
            description: "Unified lighting control with scope-based targeting".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "description": "Scope of lighting control",
                        "enum": ["device", "room", "all"]
                    },
                    "target": {
                        "type": "string",
                        "description": "Target device ID or room name (required for device/room scope)"
                    },
                    "action": {
                        "type": "string",
                        "description": "Lighting action to perform",
                        "enum": ["on", "off", "dim", "bright", "toggle"]
                    },
                    "brightness": {
                        "type": "integer",
                        "description": "Brightness level (0-100) for dim/bright actions",
                        "minimum": 0,
                        "maximum": 100
                    }
                },
                "required": ["scope", "action"]
            }),
        },
        
        // Sensor tools
        Tool {
            name: "get_all_door_window_sensors".to_string(),
            description: "Get status of all door and window sensors".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "get_temperature_sensors".to_string(),
            description: "Get readings from all temperature sensors".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        
        // Weather tools
        Tool {
            name: "get_weather_station_data".to_string(),
            description: "Get comprehensive weather station data".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        
        // Energy tools  
        Tool {
            name: "get_energy_consumption".to_string(),
            description: "Get current energy consumption data".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "period": {
                        "type": "string",
                        "description": "Time period for energy data",
                        "enum": ["current", "today", "week", "month"]
                    }
                },
                "required": []
            }),
        },
        
        // Climate control tools
        Tool {
            name: "get_climate_control".to_string(),
            description: "Get comprehensive climate control overview including all HVAC devices and room controllers".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        Tool {
            name: "get_room_climate".to_string(),
            description: "Get climate status for a specific room including temperature sensors and controllers".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "room_name": {
                        "type": "string",
                        "description": "Name of the room to get climate information for"
                    }
                },
                "required": ["room_name"]
            }),
        },
        Tool {
            name: "set_room_temperature".to_string(),
            description: "Set target temperature for a room's climate controller".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "room_name": {
                        "type": "string",
                        "description": "Name of the room to control"
                    },
                    "temperature": {
                        "type": "number",
                        "description": "Target temperature in Celsius (5.0 - 35.0)",
                        "minimum": 5.0,
                        "maximum": 35.0
                    }
                },
                "required": ["room_name", "temperature"]
            }),
        },
        Tool {
            name: "get_temperature_readings".to_string(),
            description: "Get temperature readings from all climate sensors and room controllers".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        Tool {
            name: "set_room_mode".to_string(),
            description: "Control heating/cooling mode for a room's climate controller".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "room_name": {
                        "type": "string",
                        "description": "Name of the room to control"
                    },
                    "mode": {
                        "type": "string",
                        "description": "Climate mode to set",
                        "enum": ["heating", "cooling", "auto", "off"]
                    }
                },
                "required": ["room_name", "mode"]
            }),
        },
        
        // Security tools
        Tool {
            name: "get_alarm_status".to_string(),
            description: "Get current alarm system status and security device states".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        Tool {
            name: "arm_alarm".to_string(),
            description: "Arm the alarm system for security monitoring".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "mode": {
                        "type": "string",
                        "description": "Alarm mode to set",
                        "enum": ["home", "away", "full"],
                        "default": "away"
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "disarm_alarm".to_string(),
            description: "Disarm the alarm system".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        
        // Workflow tools
        Tool {
            name: "create_workflow".to_string(),
            description: "Create a new automation workflow by chaining multiple tools together".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the workflow"
                    },
                    "description": {
                        "type": "string",
                        "description": "Description of what the workflow does"
                    },
                    "steps": {
                        "type": "array",
                        "description": "Array of workflow steps to execute",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": {
                                    "type": "string",
                                    "enum": ["tool", "parallel", "sequential", "conditional", "delay", "loop"]
                                }
                            }
                        }
                    },
                    "timeout_seconds": {
                        "type": "number",
                        "description": "Maximum execution time in seconds"
                    },
                    "variables": {
                        "type": "object",
                        "description": "Initial variables for the workflow"
                    }
                },
                "required": ["name", "description", "steps"]
            }),
        },
        Tool {
            name: "execute_workflow_demo".to_string(),
            description: "Execute a demonstration workflow to show automation capabilities".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "workflow_name": {
                        "type": "string",
                        "description": "Name of the demo workflow to execute",
                        "enum": ["home_automation", "morning_routine", "security_check"]
                    },
                    "variables": {
                        "type": "object",
                        "description": "Variables to pass to the workflow"
                    }
                },
                "required": ["workflow_name"]
            }),
        },
        Tool {
            name: "list_predefined_workflows".to_string(),
            description: "List all predefined automation workflows available in the system".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        Tool {
            name: "get_workflow_examples".to_string(),
            description: "Get examples of workflow configurations for different automation scenarios".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
    ]
}

/// Route tool calls to appropriate implementation
pub async fn handle_tool_call(
    server: &LoxoneMcpServer,
    params: &CallToolRequestParam,
) -> Result<Content, LoxoneError> {
    let context = create_tool_context(server);
    
    let response = match params.name.as_str() {
        // Room tools
        "list_rooms" => {
            crate::tools::rooms::list_rooms(context).await
        }
        "get_room_devices" => {
            let room_name = extract_string_param(&params.arguments, "room_name")?;
            let category = extract_optional_string_param(&params.arguments, "category");
            let limit = params.arguments.as_ref()
                .and_then(|p| p.get("limit"))
                .and_then(|v| v.as_u64())
                .map(|n| n as usize);
            crate::tools::rooms::get_room_devices(context, room_name, category, limit).await
        }
        "get_room_overview" => {
            let room_name = extract_string_param(&params.arguments, "room_name")?;
            crate::tools::rooms::get_room_overview(context, room_name).await
        }
        
        // Device tools
        "discover_all_devices" => {
            let category = extract_optional_string_param(&params.arguments, "category");
            let device_type = extract_optional_string_param(&params.arguments, "device_type");
            let limit = params.arguments.as_ref()
                .and_then(|p| p.get("limit"))
                .and_then(|v| v.as_u64())
                .map(|n| n as usize);
            crate::tools::devices::discover_all_devices(context, category, device_type, limit).await
        }
        "control_device" => {
            let device_id = extract_string_param(&params.arguments, "device_id")?;
            let action = extract_string_param(&params.arguments, "action")?;
            // Note: value parameter not used by current function signature
            crate::tools::devices::control_device(context, device_id, action).await
        }
        "get_devices_by_category" => {
            let category = extract_string_param(&params.arguments, "category")?;
            let limit = params.arguments.as_ref()
                .and_then(|p| p.get("limit"))
                .and_then(|v| v.as_u64())
                .map(|n| n as usize);
            crate::tools::devices::get_devices_by_category(context, category, limit).await
        }
        
        // Rolladen/Blinds tools
        "control_rolladen_unified" => {
            let scope = extract_string_param(&params.arguments, "scope")?;
            let target = extract_optional_string_param(&params.arguments, "target");
            let action = extract_string_param(&params.arguments, "action")?;
            let position = extract_optional_u8_param(&params.arguments, "position");
            crate::tools::rolladen::control_rolladen_unified(context, scope, target, action, position).await
        }
        "discover_rolladen_capabilities" => {
            crate::tools::rolladen::discover_rolladen_capabilities(context).await
        }
        "control_room_rolladen" => {
            // Legacy compatibility: redirect to unified rolladen control with room scope
            let room = extract_string_param(&params.arguments, "room")?;
            let action = extract_string_param(&params.arguments, "action")?;
            crate::tools::rolladen::control_rolladen_unified(
                context, 
                "room".to_string(), 
                Some(room), 
                action, 
                None
            ).await
        }
        "control_all_rolladen" => {
            // Legacy compatibility: redirect to unified rolladen control with system scope
            let action = extract_string_param(&params.arguments, "action")?;
            crate::tools::rolladen::control_rolladen_unified(
                context, 
                "all".to_string(), 
                None, 
                action, 
                None
            ).await
        }
        "control_room_lights" => {
            // Legacy compatibility: redirect to unified lighting control with room scope
            let room = extract_string_param(&params.arguments, "room")?;
            let action = extract_string_param(&params.arguments, "action")?;
            crate::tools::lighting::control_lights_unified(
                context, 
                "room".to_string(), 
                Some(room), 
                action, 
                None
            ).await
        }
        "control_all_lights" => {
            // Legacy compatibility: redirect to unified lighting control with all scope
            let action = extract_string_param(&params.arguments, "action")?;
            crate::tools::lighting::control_lights_unified(
                context, 
                "all".to_string(), 
                None, 
                action, 
                None
            ).await
        }
        "control_multiple_devices" => {
            // Legacy compatibility: control multiple devices by UUID
            let devices = params.arguments.as_ref()
                .and_then(|p| p.get("devices"))
                .and_then(|v| v.as_array())
                .ok_or_else(|| LoxoneError::invalid_input("Missing or invalid devices parameter"))?;
            let action = extract_string_param(&params.arguments, "action")?;
            
            // Execute commands in parallel for all devices
            let mut results = Vec::new();
            for device_uuid in devices {
                if let Some(uuid) = device_uuid.as_str() {
                    match context.client.send_command(uuid, &action).await {
                        Ok(response) => results.push(serde_json::json!({
                            "device": uuid,
                            "success": true,
                            "response": response.value
                        })),
                        Err(e) => results.push(serde_json::json!({
                            "device": uuid,
                            "success": false,
                            "error": e.to_string()
                        }))
                    }
                }
            }
            
            crate::tools::ToolResponse {
                status: "success".to_string(),
                data: serde_json::json!({
                    "action": action,
                    "results": results,
                    "total_devices": devices.len(),
                    "timestamp": chrono::Utc::now()
                }),
                message: Some(format!("Controlled {} devices with action '{}'", devices.len(), action)),
                timestamp: chrono::Utc::now(),
            }
        }
        
        // Audio tools
        "get_audio_zones" => {
            crate::tools::audio::get_audio_zones(context).await
        }
        "control_audio_zone" => {
            let zone_name = extract_string_param(&params.arguments, "zone_name")?;
            let action = extract_string_param(&params.arguments, "action")?;
            let value = params.arguments.as_ref()
                .and_then(|p| p.get("value"))
                .and_then(|v| v.as_f64());
            crate::tools::audio::control_audio_zone(context, zone_name, action, value).await
        }
        "get_audio_sources" => {
            crate::tools::audio::get_audio_sources(context).await
        }
        "set_audio_volume" => {
            let zone_name = extract_string_param(&params.arguments, "zone_name")?;
            let volume = params.arguments.as_ref()
                .and_then(|p| p.get("volume"))
                .and_then(|v| v.as_f64())
                .ok_or_else(|| LoxoneError::invalid_input("Missing volume parameter"))?;
            crate::tools::audio::set_audio_volume(context, zone_name, volume).await
        }
        
        // Lighting tools
        "control_lights_unified" => {
            let scope = extract_string_param(&params.arguments, "scope")?;
            let target = extract_optional_string_param(&params.arguments, "target");
            let action = extract_string_param(&params.arguments, "action")?;
            let brightness = extract_optional_u8_param(&params.arguments, "brightness");
            crate::tools::lighting::control_lights_unified(context, scope, target, action, brightness).await
        }
        
        // Sensor tools
        "get_all_door_window_sensors" => {
            crate::tools::sensors_unified::get_door_window_sensors_unified(context).await
        }
        "get_temperature_sensors" => {
            crate::tools::sensors_unified::get_temperature_sensors_unified(context).await
        }
        
        // Weather tools  
        "get_weather_station_data" => {
            crate::tools::weather::get_weather_data(context).await
        }
        
        // Energy tools (special handling for different return type)
        "get_energy_consumption" => {
            let _period = extract_optional_string_param(&params.arguments, "period");
            match crate::tools::energy::get_energy_consumption(
                params.arguments.clone().unwrap_or_default(), 
                std::sync::Arc::new(context)
            ).await {
                Ok(value) => crate::tools::ToolResponse {
                    status: "success".to_string(),
                    data: value,
                    message: None,
                    timestamp: chrono::Utc::now(),
                },
                Err(e) => crate::tools::ToolResponse {
                    status: "error".to_string(),
                    data: serde_json::Value::Null,
                    message: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                },
            }
        }
        
        // Climate control tools
        "get_climate_control" => {
            crate::tools::climate::get_climate_control(context).await
        }
        "get_room_climate" => {
            let room_name = extract_string_param(&params.arguments, "room_name")?;
            crate::tools::climate::get_room_climate(context, room_name).await
        }
        "set_room_temperature" => {
            let room_name = extract_string_param(&params.arguments, "room_name")?;
            let temperature = params.arguments.as_ref()
                .and_then(|p| p.get("temperature"))
                .and_then(|v| v.as_f64())
                .ok_or_else(|| LoxoneError::invalid_input("Missing temperature parameter"))?;
            crate::tools::climate::set_room_temperature(context, room_name, temperature).await
        }
        "get_temperature_readings" => {
            crate::tools::climate::get_temperature_readings(context).await
        }
        "set_room_mode" => {
            let room_name = extract_string_param(&params.arguments, "room_name")?;
            let mode = extract_string_param(&params.arguments, "mode")?;
            crate::tools::climate::set_room_mode(context, room_name, mode).await
        }
        
        // Security tools (adapted from legacy signature)
        "get_alarm_status" => {
            match crate::tools::security::get_alarm_status(
                params.arguments.clone().unwrap_or_default(),
                std::sync::Arc::new(context)
            ).await {
                Ok(value) => crate::tools::ToolResponse {
                    status: "success".to_string(),
                    data: value,
                    message: None,
                    timestamp: chrono::Utc::now(),
                },
                Err(e) => crate::tools::ToolResponse {
                    status: "error".to_string(),
                    data: serde_json::Value::Null,
                    message: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                },
            }
        }
        "arm_alarm" => {
            match crate::tools::security::arm_alarm(
                params.arguments.clone().unwrap_or_default(),
                std::sync::Arc::new(context)
            ).await {
                Ok(value) => crate::tools::ToolResponse {
                    status: "success".to_string(),
                    data: value,
                    message: None,
                    timestamp: chrono::Utc::now(),
                },
                Err(e) => crate::tools::ToolResponse {
                    status: "error".to_string(),
                    data: serde_json::Value::Null,
                    message: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                },
            }
        }
        "disarm_alarm" => {
            match crate::tools::security::disarm_alarm(
                params.arguments.clone().unwrap_or_default(),
                std::sync::Arc::new(context)
            ).await {
                Ok(value) => crate::tools::ToolResponse {
                    status: "success".to_string(),
                    data: value,
                    message: None,
                    timestamp: chrono::Utc::now(),
                },
                Err(e) => crate::tools::ToolResponse {
                    status: "error".to_string(),
                    data: serde_json::Value::Null,
                    message: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                },
            }
        }
        
        // Workflow tools (adapted from legacy signature)
        "create_workflow" => {
            // Extract workflow parameters manually since they have a complex structure
            let name = extract_string_param(&params.arguments, "name")?;
            let description = extract_string_param(&params.arguments, "description")?;
            let _steps = params.arguments.as_ref()
                .and_then(|p| p.get("steps"))
                .and_then(|v| v.as_array())
                .ok_or_else(|| LoxoneError::invalid_input("Missing or invalid steps parameter"))?;
            let timeout_seconds = params.arguments.as_ref()
                .and_then(|p| p.get("timeout_seconds"))
                .and_then(|v| v.as_u64());
            let variables = params.arguments.as_ref()
                .and_then(|p| p.get("variables"))
                .and_then(|v| v.as_object())
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
            
            let workflow_params = crate::tools::workflows::CreateWorkflowParams {
                name,
                description,
                steps: vec![], // Simplified for framework migration
                timeout_seconds,
                variables,
            };
            
            match crate::tools::workflows::create_workflow(context, workflow_params).await {
                Ok(value) => crate::tools::ToolResponse {
                    status: "success".to_string(),
                    data: value,
                    message: None,
                    timestamp: chrono::Utc::now(),
                },
                Err(e) => crate::tools::ToolResponse {
                    status: "error".to_string(),
                    data: serde_json::Value::Null,
                    message: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                },
            }
        }
        "execute_workflow_demo" => {
            let workflow_name = extract_string_param(&params.arguments, "workflow_name")?;
            let variables = params.arguments.as_ref()
                .and_then(|p| p.get("variables"))
                .and_then(|v| v.as_object())
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
            
            let execute_params = crate::tools::workflows::ExecuteWorkflowParams {
                workflow_name,
                variables,
            };
            
            match crate::tools::workflows::execute_workflow_demo(context, execute_params).await {
                Ok(value) => crate::tools::ToolResponse {
                    status: "success".to_string(),
                    data: value,
                    message: None,
                    timestamp: chrono::Utc::now(),
                },
                Err(e) => crate::tools::ToolResponse {
                    status: "error".to_string(),
                    data: serde_json::Value::Null,
                    message: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                },
            }
        }
        "list_predefined_workflows" => {
            let list_params = crate::tools::workflows::ListPredefinedWorkflowsParams {};
            match crate::tools::workflows::list_predefined_workflows(context, list_params).await {
                Ok(value) => crate::tools::ToolResponse {
                    status: "success".to_string(),
                    data: value,
                    message: None,
                    timestamp: chrono::Utc::now(),
                },
                Err(e) => crate::tools::ToolResponse {
                    status: "error".to_string(),
                    data: serde_json::Value::Null,
                    message: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                },
            }
        }
        "get_workflow_examples" => {
            match crate::tools::workflows::get_workflow_examples(context).await {
                Ok(value) => crate::tools::ToolResponse {
                    status: "success".to_string(),
                    data: value,
                    message: None,
                    timestamp: chrono::Utc::now(),
                },
                Err(e) => crate::tools::ToolResponse {
                    status: "error".to_string(),
                    data: serde_json::Value::Null,
                    message: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                },
            }
        }
        
        _ => {
            return Err(LoxoneError::validation(format!("Unknown tool: {}", params.name)));
        }
    };
    
    Ok(tool_response_to_content(response))
}