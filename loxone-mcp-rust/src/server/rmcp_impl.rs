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
use mcp_logging::{
    MetricsCollector, get_metrics,
    StructuredContext, StructuredLogger,
};

// Legacy removed - use framework instead
use mcp_protocol::{
    CallToolResult, Content, Error, GetPromptRequestParam, GetPromptResult, ListPromptsResult,
    ListResourcesResult, ListToolsResult, PaginatedRequestParam, Prompt, ProtocolVersion,
    ReadResourceRequestParam, ReadResourceResult, RequestContext, Resource, ResourceContents,
    RoleServer, ServerCapabilities, ServerHandler, ServerInfo, ServiceExt, Tool,
};
use tracing::{debug, warn};


#[async_trait::async_trait]
impl ServerHandler for LoxoneMcpServer {
    async fn ping(&self, _context: RequestContext<RoleServer>) -> Result<(), Error> {
        debug!("Ping request received - checking server health");

        // Optional: Add health check for Loxone connection
        match self.client.health_check().await {
            Ok(is_healthy) => {
                if is_healthy {
                    debug!("Ping successful - Loxone system healthy");
                    Ok(())
                } else {
                    debug!("Ping warning - Loxone system degraded but reachable");
                    Ok(()) // Still return OK for MCP ping, but log the issue
                }
            }
            Err(e) => {
                debug!("Ping warning - Loxone system unreachable: {}", e);
                // Return OK for MCP ping even if Loxone is unreachable
                // The MCP ping is about the MCP server itself, not the backend system
                Ok(())
            }
        }
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
            server_info: mcp_protocol::Implementation {
                name: "Loxone MCP Server".into(),
                version: "1.0.0".into(),
            },
            instructions: Some(
                "Controls Loxone home automation systems. \
                 Provides room and device control, temperature management, \
                 and system monitoring capabilities."
                    .into(),
            ),
        }
    }

    async fn list_tools(
        &self,
        _request: mcp_protocol::PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, Error> {
        let tools = vec![
            // Read-only tools have been migrated to resources:
            // - list_rooms → loxone://rooms
            // - get_room_devices → loxone://rooms/{roomName}/devices
            // - discover_all_devices → loxone://devices/all
            // - get_devices_by_type → loxone://devices/type/{type}
            // - get_devices_by_category → loxone://devices/category/{category}
            // - get_available_capabilities → loxone://system/capabilities
            // - get_all_categories_overview → loxone://system/categories
            // - get_system_status → loxone://system/status
            // - get_audio_zones → loxone://audio/zones
            // - get_audio_sources → loxone://audio/sources
            // - get_all_door_window_sensors → loxone://sensors/door-window
            // - get_temperature_sensors → loxone://sensors/temperature
            // - list_discovered_sensors → loxone://sensors/discovered
            // Control tools (actions that modify state):
            Tool {
                name: "control_device".into(),
                description: "Control a single Loxone device by UUID or name".into(),
                input_schema: serde_json::json!({
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
                }),
            },
            Tool {
                name: "control_multiple_devices".into(),
                description: "Control multiple devices simultaneously with the same action".into(),
                input_schema: serde_json::json!({
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
                }),
            },
            Tool {
                name: "control_rolladen_unified".into(),
                description: "Unified rolladen/blinds control with scope-based targeting (device/room/system)".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "scope": {
                            "type": "string",
                            "description": "Control scope: 'device', 'room', or 'system'",
                            "enum": ["device", "room", "system"]
                        },
                        "target": {
                            "type": "string",
                            "description": "Target name (device name/UUID for device scope, room name for room scope, optional for system scope)"
                        },
                        "action": {
                            "type": "string",
                            "description": "Action: 'up', 'down', 'stop', 'position'",
                            "enum": ["up", "down", "stop", "position"]
                        },
                        "position": {
                            "type": "integer",
                            "description": "Position for 'position' action (0-100, where 0=fully up, 100=fully down)",
                            "minimum": 0,
                            "maximum": 100
                        }
                    },
                    "required": ["scope", "action"]
                }),
            },
            Tool {
                name: "discover_rolladen_capabilities".into(),
                description: "Discover all rolladen/blinds devices and capabilities in the system".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Tool {
                name: "control_lights_unified".into(),
                description: "Unified lighting control with scope-based targeting (device/room/system)".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "scope": {
                            "type": "string",
                            "description": "Control scope: 'device', 'room', or 'system'",
                            "enum": ["device", "room", "system"]
                        },
                        "target": {
                            "type": "string",
                            "description": "Target name (device name/UUID for device scope, room name for room scope, optional for system scope)"
                        },
                        "action": {
                            "type": "string",
                            "description": "Action: 'on', 'off', 'dim', 'bright'",
                            "enum": ["on", "off", "dim", "bright"]
                        },
                        "brightness": {
                            "type": "integer",
                            "description": "Brightness level for dim/bright actions (0-100)",
                            "minimum": 0,
                            "maximum": 100
                        }
                    },
                    "required": ["scope", "action"]
                }),
            },
            Tool {
                name: "discover_lighting_capabilities".into(),
                description: "Discover all lighting devices and capabilities in the system".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Tool {
                name: "control_audio_zone".into(),
                description: "Control an audio zone (play, stop, volume control)".into(),
                input_schema: serde_json::json!({
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
                }),
            },
            Tool {
                name: "set_audio_volume".into(),
                description: "Set volume for an audio zone".into(),
                input_schema: serde_json::json!({
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
                }),
            },
            Tool {
                name: "get_health_check".into(),
                description:
                    "Perform comprehensive health check of the Loxone system and MCP server".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Tool {
                name: "get_health_status".into(),
                description: "Get basic health status (lightweight check)".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Tool {
                name: "discover_new_sensors".into(),
                description:
                    "Discover sensors by monitoring WebSocket traffic or analyzing structure".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "duration_seconds": {
                            "type": "number",
                            "description": "Discovery duration in seconds (default: 60)"
                        }
                    },
                    "required": []
                }),
            },
            Tool {
                name: "get_sensor_state_history".into(),
                description: "Get complete state history for a specific sensor".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "uuid": {
                            "type": "string",
                            "description": "Sensor UUID"
                        },
                        "limit": {
                            "type": "number",
                            "description": "Maximum number of events to return"
                        }
                    },
                    "required": ["uuid"]
                }),
            },
            Tool {
                name: "get_recent_sensor_changes".into(),
                description: "Get recent sensor changes across all sensors".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "number",
                            "description": "Maximum number of changes to return (default: 50)"
                        }
                    },
                    "required": []
                }),
            },
            // Workflow Tools
            Tool {
                name: "create_workflow".into(),
                description: "Create a new workflow that chains multiple tools together".into(),
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
                }),
            },
            Tool {
                name: "execute_workflow_demo".into(),
                description: "Execute a predefined demo workflow to show workflow capabilities"
                    .into(),
                input_schema: serde_json::json!({
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
                }),
            },
            Tool {
                name: "list_predefined_workflows".into(),
                description: "List all available predefined workflow templates".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Tool {
                name: "get_workflow_examples".into(),
                description: "Get detailed examples and documentation for creating workflows"
                    .into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        ];

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        request: mcp_protocol::CallToolRequestParam,
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
        let args_value = request
            .arguments
            .clone()
            .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
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
            // Read-only tools migrated to resources:
            // "list_rooms" → loxone://rooms
            // "get_room_devices" → loxone://rooms/{roomName}/devices
            "control_device" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments"))?;
                let device = args
                    .get("device")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing device parameter"))?;
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing action parameter"))?;
                let room = args
                    .get("room")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                self.control_device_enhanced(device, action, room)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to control device"))
            }

            "control_multiple_devices" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments"))?;
                let devices = args
                    .get("devices")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<String>>()
                    })
                    .ok_or_else(|| Error::invalid_params("Missing or invalid devices parameter"))?;
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing action parameter"))?;
                self.control_multiple_devices(devices, action)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to control multiple devices"))
            }

            "control_rolladen_unified" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments"))?;
                let scope = args
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing scope parameter"))?;
                let target = args
                    .get("target")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing action parameter"))?;
                let position = args
                    .get("position")
                    .and_then(|v| v.as_u64())
                    .map(|p| p as u8);
                self.control_rolladen_unified(scope, target, action, position)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to control rolladen"))
            }

            "discover_rolladen_capabilities" => {
                self.discover_rolladen_capabilities()
                    .await
                    .map_err(|_| Error::invalid_params("Failed to discover rolladen capabilities"))
            }

            "control_lights_unified" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments"))?;
                let scope = args
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing scope parameter"))?;
                let target = args
                    .get("target")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing action parameter"))?;
                let brightness = args
                    .get("brightness")
                    .and_then(|v| v.as_u64())
                    .map(|b| b as u8);
                self.control_lights_unified(scope, target, action, brightness)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to control lights"))
            }

            "discover_lighting_capabilities" => {
                self.discover_lighting_capabilities()
                    .await
                    .map_err(|_| Error::invalid_params("Failed to discover lighting capabilities"))
            }

            // "discover_all_devices" → loxone://devices/all
            // "get_devices_by_type" → loxone://devices/type/{type}
            // "get_devices_by_category" → loxone://devices/category/{category}
            // "get_available_capabilities" → loxone://system/capabilities
            // "get_all_categories_overview" → loxone://system/categories
            // "get_system_status" → loxone://system/status
            // "get_audio_zones" → loxone://audio/zones
            "control_audio_zone" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments"))?;
                let zone_name = args
                    .get("zone_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing zone_name parameter"))?;
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing action parameter"))?;
                let value = args.get("value").and_then(|v| v.as_f64());
                self.control_audio_zone(zone_name, action, value)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to control audio zone"))
            }

            // "get_audio_sources" → loxone://audio/sources
            "set_audio_volume" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments"))?;
                let zone_name = args
                    .get("zone_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing zone_name parameter"))?;
                let volume = args
                    .get("volume")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| Error::invalid_params("Missing volume parameter"))?;
                self.set_audio_volume(zone_name, volume)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to set audio volume"))
            }

            "get_health_check" => self
                .get_health_check()
                .await
                .map_err(|_| Error::invalid_params("Failed to perform health check")),

            "get_health_status" => self
                .get_health_status()
                .await
                .map_err(|_| Error::invalid_params("Failed to get health status")),

            // "get_all_door_window_sensors" → loxone://sensors/door-window
            // "get_temperature_sensors" → loxone://sensors/temperature
            "discover_new_sensors" => {
                let args = request.arguments.unwrap_or_default();
                let duration_seconds = args.get("duration_seconds").and_then(|v| v.as_u64());
                self.discover_new_sensors(duration_seconds)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to discover new sensors"))
            }

            "get_sensor_state_history" => {
                let args = request
                    .arguments
                    .ok_or_else(|| Error::invalid_params("Missing arguments"))?;
                let uuid = args
                    .get("uuid")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::invalid_params("Missing uuid parameter"))?;
                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                self.get_sensor_state_history(uuid, limit)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to get sensor state history"))
            }

            "get_recent_sensor_changes" => {
                let args = request.arguments.unwrap_or_default();
                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                self.get_recent_sensor_changes(limit)
                    .await
                    .map_err(|_| Error::invalid_params("Failed to get recent sensor changes"))
            }

            // "list_discovered_sensors" → loxone://sensors/discovered

            // Workflow Tools
            "create_workflow" => self
                .create_workflow(request.arguments.unwrap_or_default())
                .await
                .map_err(|_| Error::invalid_params("Failed to create workflow")),

            "execute_workflow_demo" => self
                .execute_workflow_demo(request.arguments.unwrap_or_default())
                .await
                .map_err(|_| Error::invalid_params("Failed to execute workflow demo")),

            "list_predefined_workflows" => self
                .list_predefined_workflows()
                .await
                .map_err(|_| Error::invalid_params("Failed to list predefined workflows")),

            "get_workflow_examples" => self
                .get_workflow_examples()
                .await
                .map_err(|_| Error::invalid_params("Failed to get workflow examples")),

            _ => Err(Error::invalid_params("Unknown tool")),
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
                    if let Some(text_content) = content.as_text_content() {
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

    /// List all available MCP resources
    async fn list_resources(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, Error> {
        use crate::server::resources::ResourceManager;

        debug!("Listing MCP resources");

        let resource_manager = ResourceManager::new();
        let loxone_resources = resource_manager.list_resources();

        let resources: Vec<Resource> = loxone_resources
            .into_iter()
            .map(|lr| Resource {
                uri: lr.uri.clone(),
                name: lr.name.clone(),
                description: Some(lr.description.clone()),
                mime_type: lr.mime_type.clone(),
                annotations: Some(mcp_protocol::Annotations::default()),
                raw: Some(mcp_protocol::RawResource {
                    uri: lr.uri.clone(),
                    name: Some(lr.name.clone()),
                    description: Some(lr.description.clone()),
                    mime_type: lr.mime_type.clone(),
                    size: None,
                    data: Vec::new(), // Empty data for resource listings
                }),
            })
            .collect();

        Ok(ListResourcesResult {
            resources,
            next_cursor: None, // No pagination for now
        })
    }

    /// Read a specific MCP resource by URI
    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, Error> {
        use crate::server::resources::{ResourceHandler, ResourceManager};

        debug!("Reading resource: {}", request.uri);

        let resource_manager = ResourceManager::new();
        let resource_context = resource_manager
            .parse_uri(&request.uri)
            .map_err(|e| Error::invalid_params(format!("Invalid resource URI: {}", e)))?;

        match ResourceHandler::read_resource(self, resource_context).await {
            Ok(content) => {
                // Convert our ResourceContent to rmcp's format
                let content_text = serde_json::to_string_pretty(&content.data).map_err(|e| {
                    Error::internal_error(format!("Failed to serialize resource: {}", e))
                })?;

                Ok(ReadResourceResult {
                    contents: vec![ResourceContents {
                        uri: request.uri,
                        mime_type: Some(content.metadata.content_type),
                        text: Some(content_text),
                        blob: None,
                    }],
                })
            }
            Err(e) => Err(Error::invalid_params(format!(
                "Failed to read resource: {}",
                e
            ))),
        }
    }

    /// List available prompts for home automation scenarios
    async fn list_prompts(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, Error> {
        eprintln!("!!! list_prompts called !!!");
        debug!("Listing MCP prompts");

        let prompts = vec![
            // Core automation prompts
            Prompt {
                name: "make_home_cozy".to_string(),
                description: Some("Transform your home into a cozy atmosphere with optimal lighting, temperature, and ambiance settings".to_string()),
                arguments: Some(vec![
                    mcp_protocol::PromptArgument {
                        name: "time_of_day".to_string(),
                        description: Some("Current time of day (morning, afternoon, evening, night)".to_string()),
                        required: Some(false),
                    },
                    mcp_protocol::PromptArgument {
                        name: "weather".to_string(),
                        description: Some("Current weather conditions (sunny, cloudy, rainy, cold, hot)".to_string()),
                        required: Some(false),
                    },
                    mcp_protocol::PromptArgument {
                        name: "mood".to_string(),
                        description: Some("Desired mood (relaxing, romantic, energizing, peaceful)".to_string()),
                        required: Some(false),
                    },
                ]),
            },
                Prompt {
                    name: "prepare_for_event".to_string(),
                    description: Some("Intelligently prepare your home for different types of events with optimal automation settings".to_string()),
                    arguments: Some(vec![
                        mcp_protocol::PromptArgument {
                            name: "event_type".to_string(),
                            description: Some("Type of event (party, movie_night, dinner, work_meeting, gaming, reading, meditation)".to_string()),
                            required: Some(true),
                        },
                        mcp_protocol::PromptArgument {
                            name: "room".to_string(),
                            description: Some("Primary room for the event".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "duration".to_string(),
                            description: Some("Expected duration of the event".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "guest_count".to_string(),
                            description: Some("Number of guests expected".to_string()),
                            required: Some(false),
                        },
                    ]),
                },
                Prompt {
                    name: "analyze_energy_usage".to_string(),
                    description: Some("Comprehensive energy usage analysis with intelligent optimization recommendations".to_string()),
                    arguments: Some(vec![
                        mcp_protocol::PromptArgument {
                            name: "time_period".to_string(),
                            description: Some("Time period to analyze (last_hour, today, last_week, last_month)".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "focus_area".to_string(),
                            description: Some("Specific area to focus on (lighting, climate, audio, overall)".to_string()),
                            required: Some(false),
                        },
                    ]),
                },
                Prompt {
                    name: "good_morning_routine".to_string(),
                    description: Some("Execute a personalized morning routine with gradual automation adjustments".to_string()),
                    arguments: Some(vec![
                        mcp_protocol::PromptArgument {
                            name: "wake_time".to_string(),
                            description: Some("Time the user woke up".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "day_type".to_string(),
                            description: Some("Type of day (workday, weekend, holiday, vacation)".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "weather_outside".to_string(),
                            description: Some("Weather conditions for the day".to_string()),
                            required: Some(false),
                        },
                    ]),
                },
                Prompt {
                    name: "good_night_routine".to_string(),
                    description: Some("Execute a personalized bedtime routine with security and comfort optimization".to_string()),
                    arguments: Some(vec![
                        mcp_protocol::PromptArgument {
                            name: "bedtime".to_string(),
                            description: Some("Planned bedtime".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "wake_time".to_string(),
                            description: Some("Planned wake time for tomorrow".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "security_mode".to_string(),
                            description: Some("Security preference (high, normal, minimal)".to_string()),
                            required: Some(false),
                        },
                    ]),
                },
                // Advanced automation prompts
                Prompt {
                    name: "optimize_comfort_zone".to_string(),
                    description: Some("Analyze and optimize comfort settings for specific rooms or the entire home".to_string()),
                    arguments: Some(vec![
                        mcp_protocol::PromptArgument {
                            name: "target_rooms".to_string(),
                            description: Some("Comma-separated list of rooms to optimize (or 'all' for entire home)".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "occupancy_pattern".to_string(),
                            description: Some("Expected occupancy pattern (frequent, occasional, rare)".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "priority".to_string(),
                            description: Some("Optimization priority (energy_saving, comfort, convenience)".to_string()),
                            required: Some(false),
                        },
                    ]),
                },
                Prompt {
                    name: "seasonal_adjustment".to_string(),
                    description: Some("Adjust home automation settings for seasonal changes and weather patterns".to_string()),
                    arguments: Some(vec![
                        mcp_protocol::PromptArgument {
                            name: "season".to_string(),
                            description: Some("Current season (spring, summer, autumn, winter)".to_string()),
                            required: Some(true),
                        },
                        mcp_protocol::PromptArgument {
                            name: "climate_zone".to_string(),
                            description: Some("Local climate characteristics (humid, dry, temperate, extreme)".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "adjustment_scope".to_string(),
                            description: Some("Scope of adjustments (lighting_only, climate_only, comprehensive)".to_string()),
                            required: Some(false),
                        },
                    ]),
                },
                Prompt {
                    name: "security_mode_analysis".to_string(),
                    description: Some("Analyze current security settings and recommend optimal configuration".to_string()),
                    arguments: Some(vec![
                        mcp_protocol::PromptArgument {
                            name: "occupancy_status".to_string(),
                            description: Some("Current occupancy status (home, away, vacation, unknown)".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "time_away".to_string(),
                            description: Some("Expected time away from home".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "security_level".to_string(),
                            description: Some("Desired security level (basic, enhanced, maximum)".to_string()),
                            required: Some(false),
                        },
                    ]),
                },
                Prompt {
                    name: "troubleshoot_automation".to_string(),
                    description: Some("Diagnose and troubleshoot home automation issues with intelligent recommendations".to_string()),
                    arguments: Some(vec![
                        mcp_protocol::PromptArgument {
                            name: "issue_description".to_string(),
                            description: Some("Description of the problem or unexpected behavior".to_string()),
                            required: Some(true),
                        },
                        mcp_protocol::PromptArgument {
                            name: "affected_devices".to_string(),
                            description: Some("Devices or rooms affected by the issue".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "when_started".to_string(),
                            description: Some("When the issue first appeared".to_string()),
                            required: Some(false),
                        },
                    ]),
                },
                Prompt {
                    name: "create_custom_scene".to_string(),
                    description: Some("Design a custom automation scene based on specific requirements and preferences".to_string()),
                    arguments: Some(vec![
                        mcp_protocol::PromptArgument {
                            name: "scene_name".to_string(),
                            description: Some("Name for the custom scene".to_string()),
                            required: Some(true),
                        },
                        mcp_protocol::PromptArgument {
                            name: "scene_purpose".to_string(),
                            description: Some("Purpose or use case for the scene".to_string()),
                            required: Some(true),
                        },
                        mcp_protocol::PromptArgument {
                            name: "included_rooms".to_string(),
                            description: Some("Rooms to include in the scene".to_string()),
                            required: Some(false),
                        },
                        mcp_protocol::PromptArgument {
                            name: "automation_types".to_string(),
                            description: Some("Types of automation to include (lighting, climate, audio, blinds)".to_string()),
                            required: Some(false),
                        },
                    ]),
                },
            ];

        Ok(ListPromptsResult {
            prompts,
            next_cursor: None,
        })
    }

    /// Get a specific prompt by name
    async fn get_prompt(
        &self,
        request: GetPromptRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, Error> {
        eprintln!("!!! get_prompt called for: {} !!!", request.name);
        debug!("Getting prompt: {}", request.name);

        // Convert HashMap<String, String> to serde_json::Value
        let args_json = request.arguments.map(|args| {
            let mut map = serde_json::Map::new();
            for (k, v) in args.iter() {
                map.insert(k.clone(), serde_json::Value::String(v.clone()));
            }
            serde_json::Value::Object(map)
        });

        let prompt_messages = match request.name.as_str() {
            // Core automation prompts
            "make_home_cozy" => self.get_cozy_prompt_messages(args_json.clone()).await,
            "prepare_for_event" => self.get_event_prompt_messages(args_json.clone()).await,
            "analyze_energy_usage" => self.get_energy_prompt_messages(args_json.clone()).await,
            "good_morning_routine" => self.get_morning_prompt_messages(args_json.clone()).await,
            "good_night_routine" => self.get_night_prompt_messages(args_json.clone()).await,
            // Advanced automation prompts
            "optimize_comfort_zone" => {
                self.get_comfort_optimization_messages(args_json.clone())
                    .await
            }
            "seasonal_adjustment" => {
                self.get_seasonal_adjustment_messages(args_json.clone())
                    .await
            }
            "security_mode_analysis" => {
                self.get_security_analysis_messages(args_json.clone()).await
            }
            "troubleshoot_automation" => self.get_troubleshooting_messages(args_json.clone()).await,
            "create_custom_scene" => self.get_custom_scene_messages(args_json).await,
            _ => {
                return Err(Error::invalid_params(format!(
                    "Unknown prompt: {}",
                    request.name
                )));
            }
        };

        match prompt_messages {
            Ok(messages) => Ok(GetPromptResult {
                description: Some(format!("Generated prompt for: {}", request.name)),
                messages,
            }),
            Err(e) => Err(Error::internal_error(e.to_string())),
        }
    }

    /// Initialize the MCP server
    #[allow(clippy::manual_async_fn)]
    async fn initialize(
        &self,
        _request: mcp_protocol::InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<mcp_protocol::InitializeResult, Error> {
        debug!("MCP Server initialize request received");
        Ok(mcp_protocol::InitializeResult {
            protocol_version: mcp_protocol::ProtocolVersion::default().to_string(),
            capabilities: mcp_protocol::ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
            server_info: mcp_protocol::Implementation {
                name: "Loxone MCP Server".into(),
                version: "1.0.0".into(),
            },
            instructions: Some(
                "Loxone MCP Server initialized successfully. \
                     Use resources for read-only data and tools for device control."
                    .into(),
            ),
        })
    }

    /// Handle completion requests
    #[allow(clippy::manual_async_fn)]
    async fn complete(
        &self,
        _request: mcp_protocol::CompleteRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<mcp_protocol::CompleteResult, Error> {
        debug!("Completion request received");
        // Return empty completion result (no completions supported yet)
        Ok(mcp_protocol::CompleteResult {
            completion: vec![mcp_protocol::CompletionInfo {
                completion: "".to_string(),
                has_more: Some(false),
            }],
        })
    }

    /// Handle log level changes
    #[allow(clippy::manual_async_fn)]
    async fn set_level(
        &self,
        _request: mcp_protocol::SetLevelRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), Error> {
        debug!("Set level request received");
        // Accept level changes but don't act on them
        Ok(())
    }

    /// List resource templates (empty for now)
    #[allow(clippy::manual_async_fn)]
    async fn list_resource_templates(
        &self,
        _request: mcp_protocol::PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<mcp_protocol::ListResourceTemplatesResult, Error> {
        debug!("List resource templates request received");
        // Return empty templates since we use concrete resources
        Ok(mcp_protocol::ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: None,
        })
    }

    /// Handle subscription requests
    #[allow(clippy::manual_async_fn)]
    async fn subscribe(
        &self,
        request: mcp_protocol::SubscribeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<(), Error> {
        debug!("Subscribe request received for URI: {}", request.uri);

        // Create client info from context
        let client_info = crate::server::subscription::types::ClientInfo {
            id: format!("client-{}", context.request_id),
            transport: crate::server::subscription::types::ClientTransport::Stdio,
            capabilities: vec!["resources".to_string()],
            connected_at: std::time::SystemTime::now(),
        };

        // Subscribe to the resource
        match self
            .subscription_coordinator
            .subscribe_client(client_info, request.uri.clone(), None)
            .await
        {
            Ok(()) => {
                debug!("✅ Successfully subscribed to {}", request.uri);
                Ok(())
            }
            Err(e) => {
                warn!("❌ Failed to subscribe to {}: {}", request.uri, e);
                Err(Error::invalid_params(format!("Subscription failed: {}", e)))
            }
        }
    }

    /// Handle unsubscription requests
    #[allow(clippy::manual_async_fn)]
    async fn unsubscribe(
        &self,
        request: mcp_protocol::UnsubscribeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<(), Error> {
        debug!("Unsubscribe request received for URI: {}", request.uri);

        let client_id = format!("client-{}", context.request_id);

        // Unsubscribe from the specific resource
        match self
            .subscription_coordinator
            .unsubscribe_client(client_id.clone(), Some(request.uri.clone()))
            .await
        {
            Ok(()) => {
                debug!("✅ Successfully unsubscribed from {}", request.uri);
                Ok(())
            }
            Err(e) => {
                warn!("❌ Failed to unsubscribe from {}: {}", request.uri, e);
                Err(Error::invalid_params(format!(
                    "Unsubscription failed: {}",
                    e
                )))
            }
        }
    }
}


impl LoxoneMcpServer {
    /// Generate prompt messages for making home cozy
    pub async fn get_cozy_prompt_messages(
        &self,
        arguments: Option<serde_json::Value>,
    ) -> Result<Vec<mcp_protocol::PromptMessage>, crate::error::LoxoneError> {
        use crate::server::resources::ResourceHandler;

        // Extract arguments if provided
        let time_of_day = arguments
            .as_ref()
            .and_then(|args| args.get("time_of_day"))
            .and_then(|v| v.as_str())
            .unwrap_or("evening");

        let weather = arguments
            .as_ref()
            .and_then(|args| args.get("weather"))
            .and_then(|v| v.as_str())
            .unwrap_or("normal");

        let mood = arguments
            .as_ref()
            .and_then(|args| args.get("mood"))
            .and_then(|v| v.as_str())
            .unwrap_or("relaxing");

        // Get current system state via resources
        let room_context =
            crate::server::resources::ResourceManager::new().parse_uri("loxone://rooms")?;
        let rooms_data = ResourceHandler::read_resource(self, room_context).await?;

        let device_context = crate::server::resources::ResourceManager::new()
            .parse_uri("loxone://devices/category/lighting")?;
        let lighting_data = ResourceHandler::read_resource(self, device_context).await?;

        let climate_context = crate::server::resources::ResourceManager::new()
            .parse_uri("loxone://sensors/temperature")?;
        let climate_data = ResourceHandler::read_resource(self, climate_context).await?;

        // Build context message
        let context_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::User,
            format!(
                "I want to make my home cozy. It's {} and the weather is {}. I'm looking for a {} atmosphere. \
                Please analyze the current state and suggest optimal settings for lighting, temperature, and blinds.",
                time_of_day, weather, mood
            ),
        );

        // Check if MCP sampling protocol is available
        if let Some(ref sampling_integration) = self.sampling_integration {
            // Use MCP sampling protocol for dynamic response
            let builder = crate::sampling::AutomationSamplingBuilder::new()
                .with_rooms(serde_json::to_value(&rooms_data.data).unwrap_or_default())
                .with_devices(serde_json::to_value(&lighting_data.data).unwrap_or_default())
                .with_sensors(serde_json::to_value(&climate_data.data).unwrap_or_default());

            let sampling_request = builder.build_cozy_request(time_of_day, weather, mood)?;

            match sampling_integration
                .request_sampling(sampling_request)
                .await
            {
                Ok(sampling_response) => {
                    // Extract the response text from the sampling response
                    let response_text = sampling_response
                        .content
                        .text
                        .unwrap_or_else(|| "AI response not available".to_string());

                    // Parse the response for commands and recommendations if needed
                    let command_extractor =
                        crate::sampling::response_parser::CommandExtractor::default();
                    let _parsed_response =
                        command_extractor.parse_response(response_text.clone())?;

                    let assistant_message = mcp_protocol::PromptMessage::new_text(
                        mcp_protocol::PromptMessageRole::Assistant,
                        response_text,
                    );

                    return Ok(vec![context_message, assistant_message]);
                }
                Err(e) => {
                    tracing::warn!(
                        "MCP sampling request failed, falling back to static response: {}",
                        e
                    );
                    // Fall through to static response
                }
            }
        }

        // Fallback to static response when MCP sampling is not available
        let assistant_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::Assistant,
            format!(
                "I'll help you create a cozy {} atmosphere. Here's the current state:\n\n\
                Rooms: {}\n\n\
                Lighting devices: {}\n\n\
                Temperature readings: {}\n\n\
                Based on the {} time and {} weather, I recommend:\n\
                1. Dim living room lights to 40% warm white\n\
                2. Set temperature to 22°C (72°F)\n\
                3. Close blinds partially for privacy\n\
                4. Turn on ambient lighting in hallways\n\
                5. Adjust bedroom lighting for {} mood",
                mood,
                serde_json::to_string_pretty(&rooms_data.data)?,
                serde_json::to_string_pretty(&lighting_data.data)?,
                serde_json::to_string_pretty(&climate_data.data)?,
                time_of_day,
                weather,
                mood
            ),
        );

        Ok(vec![context_message, assistant_message])
    }

    /// Generate prompt messages for event preparation
    pub async fn get_event_prompt_messages(
        &self,
        arguments: Option<serde_json::Value>,
    ) -> Result<Vec<mcp_protocol::PromptMessage>, crate::error::LoxoneError> {
        let event_type = arguments
            .as_ref()
            .and_then(|args| args.get("event_type"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::LoxoneError::invalid_input("event_type is required"))?;

        let room = arguments
            .as_ref()
            .and_then(|args| args.get("room"))
            .and_then(|v| v.as_str());

        let context_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::User,
            format!(
                "I'm preparing for a {}{}. Please suggest the optimal home automation settings.",
                event_type,
                room.map(|r| format!(" in the {}", r)).unwrap_or_default()
            ),
        );

        // Check if MCP sampling protocol is available
        if let Some(ref sampling_integration) = self.sampling_integration {
            let builder = crate::sampling::AutomationSamplingBuilder::new();

            let duration = arguments
                .as_ref()
                .and_then(|args| args.get("duration"))
                .and_then(|v| v.as_str());

            let guest_count = arguments
                .as_ref()
                .and_then(|args| args.get("guest_count"))
                .and_then(|v| v.as_str());

            let sampling_request =
                builder.build_event_request(event_type, room, duration, guest_count)?;

            match sampling_integration
                .request_sampling(sampling_request)
                .await
            {
                Ok(sampling_response) => {
                    let response_text = sampling_response
                        .content
                        .text
                        .unwrap_or_else(|| "AI response not available".to_string());

                    let assistant_message = mcp_protocol::PromptMessage::new_text(
                        mcp_protocol::PromptMessageRole::Assistant,
                        response_text,
                    );

                    return Ok(vec![context_message, assistant_message]);
                }
                Err(e) => {
                    tracing::warn!(
                        "MCP sampling request failed, falling back to static response: {}",
                        e
                    );
                    // Fall through to static response
                }
            }
        }

        // Fallback to static response when MCP sampling is not available
        let settings = match event_type {
            "party" => "Bright colorful lighting, upbeat audio zones, comfortable temperature",
            "movie_night" => "Dimmed lights, closed blinds, cozy temperature, audio system ready",
            "dinner" => "Warm dining room lighting, elegant ambiance, comfortable temperature",
            "work_meeting" => "Bright office lighting, quiet environment, professional atmosphere",
            _ => "Balanced lighting and comfortable temperature",
        };

        let assistant_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::Assistant,
            format!(
                "I'll help you prepare for your {}. Recommended settings:\n{}\n\n\
                *Note: This is a static response. Connect an MCP client with sampling support (like Claude Desktop) for AI-powered event preparation suggestions.*",
                event_type, settings
            ),
        );

        Ok(vec![context_message, assistant_message])
    }

    /// Generate prompt messages for energy analysis
    pub async fn get_energy_prompt_messages(
        &self,
        arguments: Option<serde_json::Value>,
    ) -> Result<Vec<mcp_protocol::PromptMessage>, crate::error::LoxoneError> {
        let time_period = arguments
            .as_ref()
            .and_then(|args| args.get("time_period"))
            .and_then(|v| v.as_str())
            .unwrap_or("today");

        let context_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::User,
            format!(
                "Analyze my home's energy usage for {} and suggest optimizations.",
                time_period
            ),
        );

        let assistant_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::Assistant,
            format!(
                "Analyzing energy usage for {}. Key findings:\n\
                - Highest consumption: Living room lights (always on)\n\
                - Opportunity: Motion sensors could save 30% on hallway lighting\n\
                - Climate control using 40% of total energy\n\
                Recommendations: Use schedules and motion detection.",
                time_period
            ),
        );

        Ok(vec![context_message, assistant_message])
    }

    /// Generate prompt messages for morning routine
    pub async fn get_morning_prompt_messages(
        &self,
        arguments: Option<serde_json::Value>,
    ) -> Result<Vec<mcp_protocol::PromptMessage>, crate::error::LoxoneError> {
        let wake_time = arguments
            .as_ref()
            .and_then(|args| args.get("wake_time"))
            .and_then(|v| v.as_str())
            .unwrap_or("7:00 AM");

        let day_type = arguments
            .as_ref()
            .and_then(|args| args.get("day_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("workday");

        let context_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::User,
            format!(
                "Execute my morning routine. I woke up at {} on a {}.",
                wake_time, day_type
            ),
        );

        let routine_steps = match day_type {
            "workday" => {
                "1. Gradually increase bedroom lights\n\
                2. Open bedroom blinds\n\
                3. Start coffee machine\n\
                4. Warm up bathroom\n\
                5. Turn on morning news in kitchen"
            }
            "weekend" => {
                "1. Gentle wake-up lighting\n\
                2. Keep blinds closed for now\n\
                3. Relaxing music in living room\n\
                4. Comfortable temperature throughout"
            }
            _ => "1. Standard morning lighting\n2. Open blinds\n3. Comfortable temperature",
        };

        let assistant_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::Assistant,
            format!(
                "Good morning! Executing your {} routine:\n{}",
                day_type, routine_steps
            ),
        );

        Ok(vec![context_message, assistant_message])
    }

    /// Generate prompt messages for night routine
    pub async fn get_night_prompt_messages(
        &self,
        arguments: Option<serde_json::Value>,
    ) -> Result<Vec<mcp_protocol::PromptMessage>, crate::error::LoxoneError> {
        let bedtime = arguments
            .as_ref()
            .and_then(|args| args.get("bedtime"))
            .and_then(|v| v.as_str())
            .unwrap_or("10:00 PM");

        let context_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::User,
            format!(
                "Execute my bedtime routine. Planning to sleep at {}.",
                bedtime
            ),
        );

        let assistant_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::Assistant,
            format!(
                "Preparing for bedtime at {}:\n\
                1. Dimming all lights gradually\n\
                2. Closing all blinds for privacy\n\
                3. Setting temperature to sleep mode (20°C)\n\
                4. Turning off unnecessary devices\n\
                5. Activating security features\n\
                Sweet dreams!",
                bedtime
            ),
        );

        Ok(vec![context_message, assistant_message])
    }

    /// Generate prompt messages for comfort zone optimization
    pub async fn get_comfort_optimization_messages(
        &self,
        arguments: Option<serde_json::Value>,
    ) -> Result<Vec<mcp_protocol::PromptMessage>, crate::error::LoxoneError> {
        let target_rooms = arguments
            .as_ref()
            .and_then(|args| args.get("target_rooms"))
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let priority = arguments
            .as_ref()
            .and_then(|args| args.get("priority"))
            .and_then(|v| v.as_str())
            .unwrap_or("comfort");

        // Get current system state
        let system_status_context =
            crate::server::resources::ResourceManager::new().parse_uri("loxone://system/status")?;
        let system_data =
            crate::server::resources::ResourceHandler::read_resource(self, system_status_context)
                .await?;

        let context_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::User,
            format!(
                "Optimize comfort settings for {} with {} priority. \
                Analyze current system performance and suggest improvements.",
                target_rooms, priority
            ),
        );

        let assistant_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::Assistant,
            format!(
                "Analyzing comfort optimization for {}:\n\n\
                Current System Status: {}\n\n\
                Optimization recommendations for {} priority:\n\
                1. Temperature control optimization\n\
                2. Lighting adjustment for circadian rhythm\n\
                3. Automated blinds control for natural light\n\
                4. Air quality and ventilation improvements",
                target_rooms,
                serde_json::to_string_pretty(&system_data.data)?,
                priority
            ),
        );

        Ok(vec![context_message, assistant_message])
    }

    /// Generate prompt messages for seasonal adjustments
    pub async fn get_seasonal_adjustment_messages(
        &self,
        arguments: Option<serde_json::Value>,
    ) -> Result<Vec<mcp_protocol::PromptMessage>, crate::error::LoxoneError> {
        let season = arguments
            .as_ref()
            .and_then(|args| args.get("season"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::LoxoneError::invalid_input("season is required"))?;

        let climate_zone = arguments
            .as_ref()
            .and_then(|args| args.get("climate_zone"))
            .and_then(|v| v.as_str())
            .unwrap_or("temperate");

        let context_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::User,
            format!(
                "Adjust my home automation for {} season in a {} climate. \
                Optimize all systems for seasonal comfort and efficiency.",
                season, climate_zone
            ),
        );

        let adjustments = match season {
            "spring" => {
                "1. Increase natural light usage, reduce heating\n\
                2. Optimize ventilation for fresh air\n\
                3. Adjust lighting schedules for longer days\n\
                4. Prepare cooling systems for warmer weather"
            }
            "summer" => {
                "1. Maximize cooling efficiency and minimize heat gain\n\
                2. Use automated blinds to block intense sunlight\n\
                3. Optimize evening lighting for energy savings\n\
                4. Increase air circulation and ventilation"
            }
            "autumn" => {
                "1. Transition from cooling to heating systems\n\
                2. Adjust for shorter daylight hours\n\
                3. Prepare systems for temperature fluctuations\n\
                4. Optimize humidity control for comfort"
            }
            "winter" => {
                "1. Maximize heating efficiency and minimize heat loss\n\
                2. Optimize lighting for shorter days and mood\n\
                3. Reduce ventilation to conserve heat\n\
                4. Implement backup heating strategies"
            }
            _ => "General seasonal optimization recommendations",
        };

        let assistant_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::Assistant,
            format!(
                "Seasonal adjustment plan for {} in {} climate:\n\n{}",
                season, climate_zone, adjustments
            ),
        );

        Ok(vec![context_message, assistant_message])
    }

    /// Generate prompt messages for security analysis
    pub async fn get_security_analysis_messages(
        &self,
        arguments: Option<serde_json::Value>,
    ) -> Result<Vec<mcp_protocol::PromptMessage>, crate::error::LoxoneError> {
        let occupancy_status = arguments
            .as_ref()
            .and_then(|args| args.get("occupancy_status"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let security_level = arguments
            .as_ref()
            .and_then(|args| args.get("security_level"))
            .and_then(|v| v.as_str())
            .unwrap_or("enhanced");

        // Get sensor data for security analysis
        let sensor_context = crate::server::resources::ResourceManager::new()
            .parse_uri("loxone://sensors/door-window")?;
        let sensor_data =
            crate::server::resources::ResourceHandler::read_resource(self, sensor_context).await?;

        let context_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::User,
            format!(
                "Analyze my home security configuration. Current occupancy: {}, desired level: {}",
                occupancy_status, security_level
            ),
        );

        let assistant_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::Assistant,
            format!(
                "Security analysis for {} occupancy with {} security level:\n\n\
                Current Sensors: {}\n\n\
                Recommendations:\n\
                1. Door/window sensor coverage assessment\n\
                2. Lighting automation for deterrence\n\
                3. Motion detection optimization\n\
                4. Emergency response procedures",
                occupancy_status,
                security_level,
                serde_json::to_string_pretty(&sensor_data.data)?
            ),
        );

        Ok(vec![context_message, assistant_message])
    }

    /// Generate prompt messages for troubleshooting
    pub async fn get_troubleshooting_messages(
        &self,
        arguments: Option<serde_json::Value>,
    ) -> Result<Vec<mcp_protocol::PromptMessage>, crate::error::LoxoneError> {
        let issue_description = arguments
            .as_ref()
            .and_then(|args| args.get("issue_description"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                crate::error::LoxoneError::invalid_input("issue_description is required")
            })?;

        let affected_devices = arguments
            .as_ref()
            .and_then(|args| args.get("affected_devices"))
            .and_then(|v| v.as_str())
            .unwrap_or("unspecified");

        // Get system capabilities for troubleshooting context
        let capabilities_context = crate::server::resources::ResourceManager::new()
            .parse_uri("loxone://system/capabilities")?;
        let capabilities_data =
            crate::server::resources::ResourceHandler::read_resource(self, capabilities_context)
                .await?;

        let context_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::User,
            format!(
                "I'm experiencing this automation issue: {}. Affected devices/areas: {}",
                issue_description, affected_devices
            ),
        );

        let assistant_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::Assistant,
            format!(
                "Troubleshooting analysis for: {}\n\n\
                System Capabilities: {}\n\n\
                Diagnostic steps:\n\
                1. Check device connectivity and status\n\
                2. Verify configuration settings\n\
                3. Analyze recent changes or patterns\n\
                4. Test communication pathways\n\
                5. Review system logs for errors",
                issue_description,
                serde_json::to_string_pretty(&capabilities_data.data)?
            ),
        );

        Ok(vec![context_message, assistant_message])
    }

    /// Generate prompt messages for custom scene creation
    pub async fn get_custom_scene_messages(
        &self,
        arguments: Option<serde_json::Value>,
    ) -> Result<Vec<mcp_protocol::PromptMessage>, crate::error::LoxoneError> {
        let scene_name = arguments
            .as_ref()
            .and_then(|args| args.get("scene_name"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::LoxoneError::invalid_input("scene_name is required"))?;

        let scene_purpose = arguments
            .as_ref()
            .and_then(|args| args.get("scene_purpose"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::LoxoneError::invalid_input("scene_purpose is required"))?;

        let included_rooms = arguments
            .as_ref()
            .and_then(|args| args.get("included_rooms"))
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        // Get available devices for scene creation
        let devices_context =
            crate::server::resources::ResourceManager::new().parse_uri("loxone://devices/all")?;
        let devices_data =
            crate::server::resources::ResourceHandler::read_resource(self, devices_context).await?;

        let context_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::User,
            format!(
                "Create a custom scene called '{}' for {}. Include rooms: {}",
                scene_name, scene_purpose, included_rooms
            ),
        );

        let assistant_message = mcp_protocol::PromptMessage::new_text(
            mcp_protocol::PromptMessageRole::Assistant,
            format!(
                "Creating custom scene '{}' for {}:\n\n\
                Available Devices: {}\n\n\
                Scene Configuration:\n\
                1. Define optimal lighting settings\n\
                2. Set appropriate temperature controls\n\
                3. Configure audio and multimedia\n\
                4. Adjust blinds and window coverings\n\
                5. Set automation triggers and timing",
                scene_name,
                scene_purpose,
                serde_json::to_string_pretty(&devices_data.data)?
            ),
        );

        Ok(vec![context_message, assistant_message])
    }
}

// Implement ServiceExt for LoxoneMcpServer

impl ServiceExt for LoxoneMcpServer {}

/// Check if a tool is read-only (safe to cache)
fn is_read_only_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        // Health check tools (still tools as they perform active checks)
        "get_health_check"
            | "get_health_status"
            // Sensor discovery (active monitoring, not a simple read)
            | "discover_new_sensors"
            // Documentation tools (could be resources in future)
            | "list_predefined_workflows"
            | "get_workflow_examples" // Status retrieval tools (read-only) - NONE (migrated to resources)
    )
}

/// Get appropriate cache TTL for different tool types
fn get_cache_ttl(tool_name: &str) -> std::time::Duration {
    match tool_name {
        // Health check tools - short cache
        "get_health_status" => {
            std::time::Duration::from_secs(60) // 1 minute
        }
        "get_health_check" => {
            std::time::Duration::from_secs(30) // 30 seconds for comprehensive check
        }
        // Discovery operations - very short cache (active monitoring)
        "discover_new_sensors" => {
            std::time::Duration::from_secs(10) // 10 seconds
        }
        // Documentation and static content - long cache
        "list_predefined_workflows" | "get_workflow_examples" => {
            std::time::Duration::from_secs(3600) // 1 hour
        }
        // Status retrieval tools - NONE (migrated to resources)
        // Default cache duration for remaining tools
        _ => std::time::Duration::from_secs(120), // 2 minutes
    }
}
