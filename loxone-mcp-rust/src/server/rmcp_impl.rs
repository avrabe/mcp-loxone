//! ServerHandler trait implementation for LoxoneMcpServer
//!
//! This module contains the rmcp ServerHandler trait implementation that handles
//! MCP protocol requests including server info, tool listing, and tool execution.

use super::{
    rate_limiter::RateLimitResult,
    request_context::{RequestContext as McpRequestContext, RequestTracker},
    response_cache::create_cache_key,
    LoxoneMcpServer,
};
use crate::logging::{
    metrics::get_metrics,
    structured::{StructuredContext, StructuredLogger},
};
use rmcp::{
    model::{
        CallToolResult, Content, ListToolsResult, ProtocolVersion, ServerCapabilities, ServerInfo,
        Tool,
    },
    service::RequestContext,
    Error, RoleServer, ServerHandler,
};
use std::sync::Arc;
use tracing::{debug, warn};

impl ServerHandler for LoxoneMcpServer {
    async fn ping(&self, _context: RequestContext<RoleServer>) -> Result<(), Error> {
        debug!("Ping request received");
        Ok(())
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: rmcp::model::Implementation {
                name: "Loxone MCP Server".into(),
                version: "1.0.0".into(),
            },
            instructions: Some(
                "Controls Loxone Generation 1 home automation systems. \
                 Provides room and device control, temperature management, \
                 and system monitoring capabilities."
                    .into(),
            ),
        }
    }

    async fn list_tools(
        &self,
        _request: rmcp::model::PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, Error> {
        let tools = vec![
            Tool {
                name: "list_rooms".into(),
                description: "Get list of all rooms with device counts and information".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_room_devices".into(), 
                description: "Get all devices in a specific room with detailed information".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "room_name": {
                            "type": "string",
                            "description": "Name of the room"
                        },
                        "device_type": {
                            "type": "string",
                            "description": "Optional filter by device type (e.g., 'Switch', 'Jalousie')"
                        }
                    },
                    "required": ["room_name"]
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "control_device".into(),
                description: "Control a single Loxone device by UUID or name".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object", 
                    "properties": {
                        "device": {
                            "type": "string",
                            "description": "Device UUID or name"
                        },
                        "action": {
                            "type": "string",
                            "description": "Action to perform (on, off, up, down, stop)"
                        },
                        "room": {
                            "type": "string",
                            "description": "Optional room name to help identify the device"
                        }
                    },
                    "required": ["device", "action"]
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "control_multiple_devices".into(),
                description: "Control multiple devices simultaneously with the same action".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "devices": {
                            "type": "array",
                            "description": "List of device names or UUIDs to control",
                            "items": {
                                "type": "string"
                            }
                        },
                        "action": {
                            "type": "string",
                            "description": "Action to perform on all devices (on, off, up, down, stop)"
                        }
                    },
                    "required": ["devices", "action"]
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "control_all_rolladen".into(),
                description: "Control all rolladen/blinds in the entire system simultaneously".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "description": "Action to perform: 'up', 'down', or 'stop'"
                        }
                    },
                    "required": ["action"]
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "control_room_rolladen".into(),
                description: "Control all rolladen/blinds in a specific room".into(),
                input_schema: Arc::new(serde_json::json!({
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
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "control_all_lights".into(),
                description: "Control all lights in the entire system simultaneously".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "description": "Action to perform: 'on' or 'off'"
                        }
                    },
                    "required": ["action"]
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "control_room_lights".into(),
                description: "Control all lights in a specific room".into(),
                input_schema: Arc::new(serde_json::json!({
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
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "discover_all_devices".into(),
                description: "Discover and list all devices in the system with detailed information".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_devices_by_type".into(),
                description: "Get all devices filtered by type (e.g., Switch, Jalousie, Dimmer)".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "device_type": {
                            "type": "string",
                            "description": "Type of devices to filter (optional, shows all types if not specified)"
                        }
                    },
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_devices_by_category".into(),
                description: "Get all devices filtered by category with pagination support".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "category": {
                            "type": "string",
                            "description": "Device category (lighting, blinds, climate, sensors, etc.)"
                        },
                        "limit": {
                            "type": "number",
                            "description": "Maximum number of devices to return (optional)"
                        },
                        "include_state": {
                            "type": "boolean",
                            "description": "Include current device state (optional, default: false)"
                        }
                    },
                    "required": ["category"]
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_available_capabilities".into(),
                description: "Get available system capabilities based on your Loxone configuration".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_all_categories_overview".into(),
                description: "Get overview of all device categories with counts and examples".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_system_status".into(),
                description: "Get overall system status and health information".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_audio_zones".into(),
                description: "Get audio zones and their current playback status".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "control_audio_zone".into(),
                description: "Control an audio zone (play, stop, volume control)".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "zone_name": {
                            "type": "string",
                            "description": "Name of the audio zone"
                        },
                        "action": {
                            "type": "string",
                            "description": "Action to perform (play, stop, pause, volume, mute, unmute, next, previous)"
                        },
                        "value": {
                            "type": "number",
                            "description": "Optional value for actions like volume (0-100)"
                        }
                    },
                    "required": ["zone_name", "action"]
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_audio_sources".into(),
                description: "Get available audio sources and their status".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "set_audio_volume".into(),
                description: "Set volume for an audio zone".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "zone_name": {
                            "type": "string",
                            "description": "Name of the audio zone"
                        },
                        "volume": {
                            "type": "number",
                            "description": "Volume level (0-100)"
                        }
                    },
                    "required": ["zone_name", "volume"]
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_health_check".into(),
                description: "Perform comprehensive health check of the Loxone system and MCP server".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_health_status".into(),
                description: "Get basic health status (lightweight check)".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_all_door_window_sensors".into(),
                description: "Get status of all door and window sensors".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_temperature_sensors".into(),
                description: "Get all temperature sensors and their current readings".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "discover_new_sensors".into(),
                description: "Discover sensors by monitoring WebSocket traffic or analyzing structure".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "duration_seconds": {
                            "type": "number",
                            "description": "Discovery duration in seconds (default: 60)"
                        }
                    },
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "list_discovered_sensors".into(),
                description: "List all discovered sensors with optional filtering".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "sensor_type": {
                            "type": "string",
                            "description": "Filter by sensor type (door_window, motion, temperature, etc.)"
                        },
                        "room": {
                            "type": "string",
                            "description": "Filter by room name"
                        }
                    },
                    "required": []
                }).as_object().unwrap().clone()),
            },
            // Workflow Tools
            Tool {
                name: "create_workflow".into(),
                description: "Create a new workflow that chains multiple tools together".into(),
                input_schema: Arc::new(serde_json::json!({
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
                                    },
                                    "name": {
                                        "type": "string",
                                        "description": "Tool name for 'tool' type steps"
                                    },
                                    "params": {
                                        "type": "object",
                                        "description": "Parameters for tool execution"
                                    }
                                }
                            }
                        },
                        "timeout_seconds": {
                            "type": "number",
                            "description": "Optional global timeout in seconds"
                        },
                        "variables": {
                            "type": "object",
                            "description": "Variables that can be used in the workflow"
                        }
                    },
                    "required": ["name", "description", "steps"]
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "execute_workflow_demo".into(),
                description: "Execute a predefined demo workflow to show workflow capabilities".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "workflow_name": {
                            "type": "string",
                            "description": "Name of the predefined workflow to execute",
                            "enum": ["morning_routine", "parallel_demo", "conditional_demo", "security_check", "evening_routine"]
                        },
                        "variables": {
                            "type": "object",
                            "description": "Optional variables to pass to the workflow"
                        }
                    },
                    "required": ["workflow_name"]
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "list_predefined_workflows".into(),
                description: "List all available predefined workflow templates".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
            Tool {
                name: "get_workflow_examples".into(),
                description: "Get detailed examples and documentation for creating workflows".into(),
                input_schema: Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }).as_object().unwrap().clone()),
            },
        ];

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, Error> {
        // Create request context for tracking
        let req_ctx = McpRequestContext::new(request.name.to_string());
        let _span = RequestTracker::create_span(&req_ctx);

        // Create structured context for enhanced observability
        let structured_ctx = StructuredContext::new(request.name.to_string()).with_client_context(
            format!("mcp-client-{}", request.name),
            None, // user_agent - could be extracted from context if available
            None, // session_id - could be added later
        );
        let _structured_span = StructuredLogger::create_span(&structured_ctx);

        // Record request start in metrics
        get_metrics().record_request_start(&request.name).await;

        // Check resource availability
        let _resource_permit = match self.resource_monitor.check_resources(&request.name).await {
            Ok(permit) => permit,
            Err(e) => {
                warn!(
                    tool_name = %request.name,
                    error = %e,
                    "Resource limit exceeded"
                );

                get_metrics()
                    .record_error(&request.name, &req_ctx.id, &e, req_ctx.elapsed())
                    .await;

                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Resource limit exceeded: {}",
                    e
                ))]));
            }
        };

        // Check rate limits - using tool name as client ID for basic limiting
        let client_id = format!("mcp-client-{}", request.name);
        let rate_limit_result = self.rate_limiter.check_composite(&client_id, None).await;

        match rate_limit_result {
            RateLimitResult::Limited { reset_at: _ } => {
                warn!(
                    client_id = %client_id,
                    tool_name = %request.name,
                    "Request rate limited"
                );

                // Record rate limit hit in metrics
                get_metrics().record_rate_limit_hit().await;

                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Rate limit exceeded for tool '{}'. Please try again in a few seconds.",
                    request.name
                ))]));
            }
            RateLimitResult::AllowedBurst => {
                warn!(
                    client_id = %client_id,
                    tool_name = %request.name,
                    "Request allowed using burst capacity"
                );
            }
            RateLimitResult::Allowed => {
                // Normal operation, no logging needed
            }
        }

        // Log request start
        let args_value = serde_json::Value::Object(request.arguments.clone().unwrap_or_default());
        RequestTracker::log_request_start(&req_ctx, &args_value);

        // Enhanced structured logging
        StructuredLogger::log_request_start(&structured_ctx, &args_value);

        // Check cache for read-only tools
        let cache_key = create_cache_key(&request.name, &args_value);
        let is_read_only_tool = is_read_only_tool(&request.name);

        if is_read_only_tool {
            if let Some(cached_result) = self.response_cache.get(&cache_key).await {
                debug!("Cache hit for tool: {}", request.name);

                // Record cache hit metrics
                get_metrics()
                    .record_request_end(&request.name, req_ctx.elapsed(), true)
                    .await;

                // Convert cached JSON back to CallToolResult
                return Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&cached_result)
                        .unwrap_or_else(|_| "{}".to_string()),
                )]));
            }
        }

        // Validate parameters using schema validator
        let validation_result = self
            .schema_validator
            .validate_tool_parameters(&request.name, &args_value);

        // Record schema validation metrics
        get_metrics()
            .record_schema_validation(validation_result.is_ok())
            .await;

        if let Err(validation_error) = validation_result {
            warn!(
                tool_name = %request.name,
                error = %validation_error,
                "Schema validation failed"
            );

            // Record the validation error
            let duration = req_ctx.elapsed();
            get_metrics()
                .record_error(
                    &request.name,
                    &req_ctx.id,
                    &crate::error::LoxoneError::invalid_input(validation_error.to_string()),
                    duration,
                )
                .await;

            return Ok(CallToolResult::error(vec![Content::text(format!(
                "Invalid parameters for tool '{}': {}",
                request.name, validation_error
            ))]));
        }

        let result = match request.name.as_ref() {
            "list_rooms" => self
                .list_rooms()
                .await
                .map_err(|_| Error::invalid_params("Failed to list rooms", None)),

            "get_room_devices" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments", None))?;
                let room_name = args
                    .get("room_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing room_name parameter", None))?;
                let device_type = args
                    .get("device_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                self.get_room_devices_enhanced(room_name, device_type)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to get room devices", None))
            }

            "control_device" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments", None))?;
                let device = args
                    .get("device")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing device parameter", None))?;
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing action parameter", None))?;
                let room = args
                    .get("room")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                self.control_device_enhanced(device, action, room)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to control device", None))
            }

            "control_multiple_devices" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments", None))?;
                let devices = args
                    .get("devices")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<String>>()
                    })
                    .ok_or_else(|| {
                        Error::invalid_params("Missing or invalid devices parameter", None)
                    })?;
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing action parameter", None))?;
                self.control_multiple_devices(devices, action)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to control multiple devices", None))
            }

            "control_all_rolladen" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments", None))?;
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing action parameter", None))?;
                self.control_all_rolladen(action)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to control all rolladen", None))
            }

            "control_room_rolladen" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments", None))?;
                let room = args
                    .get("room")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing room parameter", None))?;
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing action parameter", None))?;
                self.control_room_rolladen(room, action)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to control room rolladen", None))
            }

            "control_all_lights" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments", None))?;
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing action parameter", None))?;
                self.control_all_lights(action)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to control all lights", None))
            }

            "control_room_lights" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments", None))?;
                let room = args
                    .get("room")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing room parameter", None))?;
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing action parameter", None))?;
                self.control_room_lights(room, action)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to control room lights", None))
            }

            "discover_all_devices" => self
                .discover_all_devices()
                .await
                .map_err(|_| Error::invalid_params("Failed to discover devices", None)),

            "get_devices_by_type" => {
                let args = request.arguments.unwrap_or_default();
                let device_type = args
                    .get("device_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                self.get_devices_by_type(device_type)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to get devices by type", None))
            }

            "get_devices_by_category" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments", None))?;
                let category = args
                    .get("category")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing category parameter", None))?;
                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let include_state = args
                    .get("include_state")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                self.get_devices_by_category(category, limit, include_state)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to get devices by category", None))
            }

            "get_available_capabilities" => self
                .get_available_capabilities()
                .await
                .map_err(|_| Error::invalid_params("Failed to get available capabilities", None)),

            "get_all_categories_overview" => self
                .get_all_categories_overview()
                .await
                .map_err(|_| Error::invalid_params("Failed to get categories overview", None)),

            "get_system_status" => self
                .get_system_status()
                .await
                .map_err(|_| Error::invalid_params("Failed to get system status", None)),

            "get_audio_zones" => self
                .get_audio_zones()
                .await
                .map_err(|_| Error::invalid_params("Failed to get audio zones", None)),

            "control_audio_zone" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments", None))?;
                let zone_name = args
                    .get("zone_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing zone_name parameter", None))?;
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing action parameter", None))?;
                let value = args.get("value").and_then(|v| v.as_f64());
                self.control_audio_zone(zone_name, action, value)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to control audio zone", None))
            }

            "get_audio_sources" => self
                .get_audio_sources()
                .await
                .map_err(|_| Error::invalid_params("Failed to get audio sources", None)),

            "set_audio_volume" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments", None))?;
                let zone_name = args
                    .get("zone_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing zone_name parameter", None))?;
                let volume = args
                    .get("volume")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| Error::invalid_params("Missing volume parameter", None))?;
                self.set_audio_volume(zone_name, volume)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to set audio volume", None))
            }

            "get_health_check" => self
                .get_health_check()
                .await
                .map_err(|_| Error::invalid_params("Failed to perform health check", None)),

            "get_health_status" => self
                .get_health_status()
                .await
                .map_err(|_| Error::invalid_params("Failed to get health status", None)),

            "get_all_door_window_sensors" => self
                .get_all_door_window_sensors()
                .await
                .map_err(|_| Error::invalid_params("Failed to get door/window sensors", None)),

            "get_temperature_sensors" => self
                .get_temperature_sensors()
                .await
                .map_err(|_| Error::invalid_params("Failed to get temperature sensors", None)),

            "discover_new_sensors" => {
                let args = request.arguments.unwrap_or_default();
                let duration_seconds = args.get("duration_seconds").and_then(|v| v.as_u64());
                self.discover_new_sensors(duration_seconds)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to discover new sensors", None))
            }

            "list_discovered_sensors" => {
                let args = request.arguments.unwrap_or_default();
                let sensor_type = args
                    .get("sensor_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let room = args
                    .get("room")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                self.list_discovered_sensors(sensor_type, room)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to list discovered sensors", None))
            }

            // Workflow Tools
            "create_workflow" => self
                .create_workflow(request.arguments.unwrap_or_default().into())
                .await
                .map_err(|_| Error::invalid_params("Failed to create workflow", None)),

            "execute_workflow_demo" => self
                .execute_workflow_demo(request.arguments.unwrap_or_default().into())
                .await
                .map_err(|_| Error::invalid_params("Failed to execute workflow demo", None)),

            "list_predefined_workflows" => self
                .list_predefined_workflows()
                .await
                .map_err(|_| Error::invalid_params("Failed to list predefined workflows", None)),

            "get_workflow_examples" => self
                .get_workflow_examples()
                .await
                .map_err(|_| Error::invalid_params("Failed to get workflow examples", None)),

            _ => Err(Error::invalid_params("Unknown tool", None)),
        };

        // Log request completion with enhanced observability
        let duration = req_ctx.elapsed();

        match &result {
            Ok(_call_result) => {
                RequestTracker::log_request_end(&req_ctx, true, None);
                RequestTracker::log_if_slow(&req_ctx, 1000); // Warn if > 1 second

                // Enhanced structured logging for success
                StructuredLogger::log_request_end(&structured_ctx, true, None, None);
                StructuredLogger::log_slow_request(&structured_ctx, 1000);

                // Record successful request metrics
                get_metrics()
                    .record_request_end(&request.name, duration, true)
                    .await;
            }
            Err(e) => {
                // Convert rmcp Error to LoxoneError for both old and new logging
                let loxone_error = crate::error::LoxoneError::invalid_input(e.to_string());

                RequestTracker::log_request_end(&req_ctx, false, Some(&loxone_error));

                // Enhanced structured logging for errors
                StructuredLogger::log_request_end(
                    &structured_ctx,
                    false,
                    Some(&loxone_error),
                    None,
                );

                // Record error metrics
                get_metrics()
                    .record_error(&request.name, &req_ctx.id, &loxone_error, duration)
                    .await;
                get_metrics()
                    .record_request_end(&request.name, duration, false)
                    .await;
            }
        }

        // Store result in cache for read-only tools
        if is_read_only_tool {
            if let Ok(ref call_result) = result {
                // Extract the response data from CallToolResult
                if let Some(content) = call_result.content.first() {
                    if let Some(text_content) = content.as_text() {
                        // Try to parse the response as JSON for caching
                        if let Ok(json_value) =
                            serde_json::from_str::<serde_json::Value>(&text_content.text)
                        {
                            // Cache with appropriate TTL based on tool type
                            let ttl = get_cache_ttl(&request.name);
                            self.response_cache
                                .put_with_ttl(cache_key, json_value, ttl)
                                .await;
                            debug!("Cached result for tool: {} (TTL: {:?})", request.name, ttl);
                        }
                    }
                }
            }
        }

        result
    }
}

/// Check if a tool is read-only (safe to cache)
fn is_read_only_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "list_rooms"
            | "get_room_devices"
            | "discover_all_devices"
            | "get_devices_by_type"
            | "get_devices_by_category"
            | "get_available_capabilities"
            | "get_system_status"
            | "get_audio_zones"
            | "get_audio_sources"
            | "get_health_check"
            | "get_health_status"
            | "get_all_door_window_sensors"
            | "get_temperature_sensors"
            | "discover_new_sensors"
            | "list_discovered_sensors"
            | "list_predefined_workflows"
            | "get_workflow_examples"
    )
}

/// Get appropriate cache TTL for different tool types
fn get_cache_ttl(tool_name: &str) -> std::time::Duration {
    match tool_name {
        // Static structure data - cache longer
        "list_rooms"
        | "discover_all_devices"
        | "get_devices_by_type"
        | "get_devices_by_category"
        | "get_available_capabilities" => {
            std::time::Duration::from_secs(600) // 10 minutes
        }
        // System status - medium cache
        "get_system_status" | "get_health_status" => {
            std::time::Duration::from_secs(60) // 1 minute
        }
        // Audio and sensor data - shorter cache
        "get_audio_zones"
        | "get_audio_sources"
        | "get_all_door_window_sensors"
        | "get_temperature_sensors" => {
            std::time::Duration::from_secs(30) // 30 seconds
        }
        // Discovery operations - very short cache
        "discover_new_sensors" | "list_discovered_sensors" => {
            std::time::Duration::from_secs(10) // 10 seconds
        }
        // Documentation and static content - long cache
        "list_predefined_workflows" | "get_workflow_examples" => {
            std::time::Duration::from_secs(3600) // 1 hour
        }
        // Default cache duration
        _ => std::time::Duration::from_secs(120), // 2 minutes
    }
}
