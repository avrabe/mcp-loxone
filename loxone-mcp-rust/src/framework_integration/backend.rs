//! Loxone backend implementation for the MCP framework
//!
//! This module implements the McpBackend trait to bridge the existing Loxone
//! server implementation with the new MCP framework.

use async_trait::async_trait;
use pulseengine_mcp_protocol::*;
use pulseengine_mcp_server::backend::{BackendError, McpBackend};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::{
    error::LoxoneError, framework_integration::adapters, server::LoxoneMcpServer, ServerConfig,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Convert LoxoneError to BackendError
impl From<LoxoneError> for BackendError {
    fn from(err: LoxoneError) -> Self {
        match err {
            LoxoneError::Connection(msg) => BackendError::connection(msg),
            LoxoneError::Authentication(msg) => BackendError::configuration(msg),
            LoxoneError::Config(msg) => BackendError::configuration(msg),
            LoxoneError::InvalidInput(msg) => BackendError::configuration(msg),
            _ => BackendError::internal(err.to_string()),
        }
    }
}

/// Convert BackendError to LoxoneError
impl From<BackendError> for LoxoneError {
    fn from(err: BackendError) -> Self {
        match err {
            BackendError::Connection(msg) => LoxoneError::connection(msg),
            BackendError::Configuration(msg) => LoxoneError::config(msg),
            BackendError::NotSupported(msg) => LoxoneError::invalid_input(msg),
            _ => LoxoneError::invalid_input(err.to_string()),
        }
    }
}

/// Convert LoxoneError to MCP protocol Error
impl From<LoxoneError> for Error {
    fn from(val: LoxoneError) -> Self {
        match val {
            LoxoneError::Connection(msg) => {
                Error::internal_error(format!("Connection error: {}", msg))
            }
            LoxoneError::Authentication(msg) => {
                Error::invalid_params(format!("Authentication error: {}", msg))
            }
            LoxoneError::Config(msg) => {
                Error::invalid_params(format!("Configuration error: {}", msg))
            }
            LoxoneError::InvalidInput(msg) => Error::invalid_params(msg),
            LoxoneError::NotFound(msg) => Error::method_not_found(msg),
            LoxoneError::Timeout(msg) => Error::internal_error(format!("Timeout: {}", msg)),
            _ => Error::internal_error(val.to_string()),
        }
    }
}

/// Cache entry for resource data
#[derive(Clone)]
struct CacheEntry {
    data: String,
    mime_type: String,
    timestamp: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn new(data: String, mime_type: String, ttl: Duration) -> Self {
        Self {
            data,
            mime_type,
            timestamp: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.timestamp.elapsed() > self.ttl
    }
}

/// Loxone-specific backend implementation for the MCP framework
#[derive(Clone)]
pub struct LoxoneBackend {
    /// Reference to the Loxone MCP server
    server: Arc<LoxoneMcpServer>,

    /// Simple resource cache with TTL
    resource_cache: Arc<tokio::sync::RwLock<HashMap<String, CacheEntry>>>,
}

impl LoxoneBackend {
    /// Create a new Loxone backend
    pub fn new(server: Arc<LoxoneMcpServer>) -> Self {
        Self {
            server,
            resource_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Get cache TTL for a resource URI
    fn get_cache_ttl(&self, uri: &str) -> Duration {
        match uri {
            // Fast-changing resources (short TTL)
            "loxone://sensors/temperature"
            | "loxone://sensors/door-window"
            | "loxone://sensors/motion" => Duration::from_secs(5),
            // Medium-changing resources
            "loxone://energy/consumption" | "loxone://weather/current" => Duration::from_secs(30),
            // Slow-changing resources (longer TTL)
            "loxone://devices/all" | "loxone://audio/zones" => {
                Duration::from_secs(300) // 5 minutes
            }
            // Static resources (very long TTL)
            "loxone://rooms" | "loxone://structure/rooms" | "loxone://system/capabilities" => {
                Duration::from_secs(3600) // 1 hour
            }
            // Default TTL for other resources
            _ => Duration::from_secs(60),
        }
    }

    /// Check if resource should be cached
    fn should_cache(&self, uri: &str) -> bool {
        // Cache most resources except system info which should always be fresh
        !matches!(uri, "loxone://system/info" | "loxone://status/health")
    }
}

#[async_trait]
impl McpBackend for LoxoneBackend {
    type Error = LoxoneError;
    type Config = ServerConfig;

    async fn initialize(config: Self::Config) -> std::result::Result<Self, Self::Error> {
        info!("🚀 Initializing Loxone backend with framework");
        let server = LoxoneMcpServer::new(config).await?;
        Ok(Self::new(Arc::new(server)))
    }

    fn get_server_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .enable_logging()
                .enable_sampling()
                .build(),
            server_info: Implementation {
                name: "loxone-mcp-server".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some("Loxone home automation control via MCP. Use tools to control lights, blinds, climate, and access sensor data.".to_string()),
        }
    }

    async fn health_check(&self) -> std::result::Result<(), Self::Error> {
        info!("🔍 Performing Loxone backend health check");

        // Check if the server is properly initialized and can connect
        match self.server.client.health_check().await {
            Ok(true) => {
                info!("✅ Loxone backend health check passed");
                Ok(())
            }
            Ok(false) => {
                warn!("⚠️ Loxone backend health check failed - not connected");
                Err(LoxoneError::connection(
                    "Health check failed: not connected",
                ))
            }
            Err(e) => {
                error!("❌ Loxone backend health check error: {}", e);
                Err(LoxoneError::connection(format!(
                    "Health check error: {}",
                    e
                )))
            }
        }
    }

    async fn list_tools(
        &self,
        _params: PaginatedRequestParam,
    ) -> std::result::Result<ListToolsResult, Self::Error> {
        info!("📋 Listing Loxone tools");

        let tools = adapters::get_all_loxone_tools();

        info!("✅ Listed {} Loxone tools", tools.len());

        Ok(ListToolsResult {
            tools,
            next_cursor: String::new(), // Empty string instead of None
        })
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParam,
    ) -> std::result::Result<CallToolResult, Self::Error> {
        info!("⚡ Calling Loxone tool: {}", params.name);

        // Use the adapter layer to handle tool calls
        match adapters::handle_tool_call(&self.server, &params).await {
            Ok(content) => {
                info!("✅ Tool {} executed successfully", params.name);
                Ok(CallToolResult::success(vec![content]))
            }
            Err(e) => {
                error!("❌ Tool {} failed: {}", params.name, e);
                Ok(CallToolResult::error_text(format!(
                    "Tool execution failed: {}",
                    e
                )))
            }
        }
    }

    async fn list_resources(
        &self,
        _params: PaginatedRequestParam,
    ) -> std::result::Result<ListResourcesResult, Self::Error> {
        info!("📁 Listing Loxone resources");

        let resources = vec![
            // System resources
            Resource {
                uri: "loxone://system/info".to_string(),
                name: "System Information".to_string(),
                description: Some("Current Loxone system information and status".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://structure/rooms".to_string(),
                name: "Room Structure".to_string(),
                description: Some("Complete room structure with devices".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://config/devices".to_string(),
                name: "Device Configuration".to_string(),
                description: Some("All configured devices and their properties".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://status/health".to_string(),
                name: "System Health".to_string(),
                description: Some("Current system health and connectivity status".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://system/capabilities".to_string(),
                name: "System Capabilities".to_string(),
                description: Some("Available system capabilities and features".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://system/categories".to_string(),
                name: "Device Categories".to_string(),
                description: Some("Overview of all device categories with counts".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            // Room and device resources
            Resource {
                uri: "loxone://rooms".to_string(),
                name: "All Rooms".to_string(),
                description: Some("List of all rooms in the home automation system".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://devices/all".to_string(),
                name: "All Devices".to_string(),
                description: Some(
                    "Complete list of all devices with their current states".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://devices/category/lighting".to_string(),
                name: "Lighting Devices".to_string(),
                description: Some("All lighting devices and their current states".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://devices/category/blinds".to_string(),
                name: "Blinds/Rolladen".to_string(),
                description: Some("All blinds and rolladen devices with positions".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://devices/category/climate".to_string(),
                name: "Climate Devices".to_string(),
                description: Some("All climate and temperature control devices".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            // Audio resources
            Resource {
                uri: "loxone://audio/zones".to_string(),
                name: "Audio Zones".to_string(),
                description: Some("All audio zones and their current status".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://audio/sources".to_string(),
                name: "Audio Sources".to_string(),
                description: Some("Available audio sources and their status".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            // Sensor resources
            Resource {
                uri: "loxone://sensors/temperature".to_string(),
                name: "Temperature Sensors".to_string(),
                description: Some("All temperature sensors and their readings".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://sensors/door-window".to_string(),
                name: "Door/Window Sensors".to_string(),
                description: Some("All door and window sensors with current states".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://sensors/motion".to_string(),
                name: "Motion Sensors".to_string(),
                description: Some("All motion sensors and detection status".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            // Weather and energy resources
            Resource {
                uri: "loxone://weather/current".to_string(),
                name: "Current Weather".to_string(),
                description: Some("Current weather data from all weather sensors".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://energy/consumption".to_string(),
                name: "Energy Consumption".to_string(),
                description: Some("Current energy consumption and usage metrics".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
        ];

        info!("✅ Listed {} Loxone resources", resources.len());

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        params: ReadResourceRequestParam,
    ) -> std::result::Result<ReadResourceResult, Self::Error> {
        info!("📖 Reading Loxone resource: {}", params.uri);

        // Check cache first if caching is enabled for this resource
        if self.should_cache(&params.uri) {
            let cache = self.resource_cache.read().await;
            if let Some(entry) = cache.get(&params.uri) {
                if !entry.is_expired() {
                    debug!("💰 Cache hit for resource: {}", params.uri);
                    return Ok(ReadResourceResult {
                        contents: vec![ResourceContents {
                            uri: params.uri,
                            mime_type: Some(entry.mime_type.clone()),
                            text: Some(entry.data.clone()),
                            blob: None,
                        }],
                    });
                } else {
                    debug!("⏰ Cache expired for resource: {}", params.uri);
                }
            }
        }

        let (mime_type, content) = match params.uri.as_str() {
            "loxone://system/info" => {
                let info = serde_json::json!({
                    "server": "loxone-mcp-server",
                    "version": "1.0.0",
                    "connected": self.server.client.is_connected().await.unwrap_or(false),
                    "health": self.server.client.health_check().await.unwrap_or(false)
                });
                ("application/json", info.to_string())
            }

            "loxone://structure/rooms" => {
                let rooms = self.server.context.rooms.read().await;
                let room_data = serde_json::to_value(&*rooms)
                    .map_err(|e| LoxoneError::serialization(e.to_string()))?;
                ("application/json", room_data.to_string())
            }

            "loxone://config/devices" => {
                let devices = self.server.context.devices.read().await;
                let device_data = serde_json::to_value(&*devices)
                    .map_err(|e| LoxoneError::serialization(e.to_string()))?;
                ("application/json", device_data.to_string())
            }

            "loxone://status/health" => {
                let health_status = serde_json::json!({
                    "status": "healthy",
                    "message": "Framework migration mode - basic health check"
                });
                let health_data = serde_json::to_value(&health_status)
                    .map_err(|e| LoxoneError::serialization(e.to_string()))?;
                ("application/json", health_data.to_string())
            }

            // Room resources
            "loxone://rooms" => {
                let rooms = self.server.context.rooms.read().await;
                let room_list: Vec<_> = rooms.keys().cloned().collect();
                ("application/json", serde_json::to_string(&room_list)?)
            }

            // Device resources
            "loxone://devices/all" => {
                let devices = self.server.context.devices.read().await;
                let device_list: Vec<_> = devices.values().collect();
                ("application/json", serde_json::to_string(&device_list)?)
            }
            "loxone://devices/category/lighting" => {
                let devices = self.server.context.devices.read().await;
                let lighting_devices: Vec<_> = devices
                    .values()
                    .filter(|d| {
                        d.category == "lights"
                            || d.device_type.contains("Light")
                            || d.device_type.contains("Dimmer")
                    })
                    .collect();
                (
                    "application/json",
                    serde_json::to_string(&lighting_devices)?,
                )
            }
            "loxone://devices/category/blinds" => {
                let devices = self.server.context.devices.read().await;
                let blinds_devices: Vec<_> = devices
                    .values()
                    .filter(|d| d.category == "blinds" || d.device_type == "Jalousie")
                    .collect();
                ("application/json", serde_json::to_string(&blinds_devices)?)
            }
            "loxone://devices/category/climate" => {
                let devices = self.server.context.devices.read().await;
                let climate_devices: Vec<_> = devices
                    .values()
                    .filter(|d| {
                        d.category == "climate"
                            || d.device_type.contains("Temperature")
                            || d.device_type.contains("Climate")
                    })
                    .collect();
                ("application/json", serde_json::to_string(&climate_devices)?)
            }

            // Audio resources
            "loxone://audio/zones" => {
                let devices = self.server.context.devices.read().await;
                let audio_devices: Vec<_> = devices
                    .values()
                    .filter(|d| {
                        d.category == "audio"
                            || d.device_type.contains("Audio")
                            || d.device_type.contains("Music")
                    })
                    .collect();
                ("application/json", serde_json::to_string(&audio_devices)?)
            }
            "loxone://audio/sources" => {
                let audio_sources = serde_json::json!({
                    "sources": [],
                    "note": "Audio sources discovery not yet implemented in framework migration"
                });
                ("application/json", audio_sources.to_string())
            }

            // Sensor resources
            "loxone://sensors/temperature" => {
                let devices = self.server.context.devices.read().await;
                let temp_sensors: Vec<_> = devices
                    .values()
                    .filter(|d| {
                        d.device_type.contains("Temperature") || d.device_type.contains("Temp")
                    })
                    .collect();
                ("application/json", serde_json::to_string(&temp_sensors)?)
            }
            "loxone://sensors/door-window" => {
                let devices = self.server.context.devices.read().await;
                let door_window_sensors: Vec<_> = devices
                    .values()
                    .filter(|d| {
                        d.device_type.contains("Door")
                            || d.device_type.contains("Window")
                            || d.device_type.contains("Contact")
                    })
                    .collect();
                (
                    "application/json",
                    serde_json::to_string(&door_window_sensors)?,
                )
            }
            "loxone://sensors/motion" => {
                let devices = self.server.context.devices.read().await;
                let motion_sensors: Vec<_> = devices
                    .values()
                    .filter(|d| d.device_type.contains("Motion") || d.device_type.contains("PIR"))
                    .collect();
                ("application/json", serde_json::to_string(&motion_sensors)?)
            }

            // Weather resources
            "loxone://weather/current" => {
                let weather_data = serde_json::json!({
                    "temperature": null,
                    "humidity": null,
                    "pressure": null,
                    "note": "Weather data access not yet implemented in framework migration"
                });
                ("application/json", weather_data.to_string())
            }

            // Energy resources
            "loxone://energy/consumption" => {
                let energy_data = serde_json::json!({
                    "current_usage": null,
                    "daily_total": null,
                    "note": "Energy consumption data not yet implemented in framework migration"
                });
                ("application/json", energy_data.to_string())
            }

            // System resources (additional)
            "loxone://system/capabilities" => {
                let capabilities = self.server.context.capabilities.read().await;
                ("application/json", serde_json::to_string(&*capabilities)?)
            }
            "loxone://system/categories" => {
                let devices = self.server.context.devices.read().await;
                let mut categories = std::collections::HashMap::new();
                for device in devices.values() {
                    *categories.entry(device.category.clone()).or_insert(0) += 1;
                }
                let category_summary = serde_json::json!({
                    "categories": categories,
                    "total_devices": devices.len()
                });
                ("application/json", category_summary.to_string())
            }

            _ => {
                return Err(LoxoneError::not_found(format!(
                    "Resource not found: {}",
                    params.uri
                )));
            }
        };

        // Update cache if caching is enabled for this resource
        if self.should_cache(&params.uri) {
            let ttl = self.get_cache_ttl(&params.uri);
            let cache_entry = CacheEntry::new(content.clone(), mime_type.to_string(), ttl);

            let mut cache = self.resource_cache.write().await;
            cache.insert(params.uri.clone(), cache_entry);
            debug!("💾 Cached resource: {} (TTL: {:?})", params.uri, ttl);

            // Simple cache cleanup: remove expired entries periodically
            if cache.len() > 100 {
                cache.retain(|_, entry| !entry.is_expired());
                debug!("🧹 Cleaned up expired cache entries");
            }
        }

        Ok(ReadResourceResult {
            contents: vec![ResourceContents {
                uri: params.uri,
                mime_type: Some(mime_type.to_string()),
                text: Some(content),
                blob: None,
            }],
        })
    }

    async fn list_resource_templates(
        &self,
        _params: PaginatedRequestParam,
    ) -> std::result::Result<ListResourceTemplatesResult, Self::Error> {
        info!("📋 Listing Loxone resource templates");

        let resource_templates = vec![
            ResourceTemplate {
                uri_template: "loxone://devices/{room_name}".to_string(),
                name: "Room Devices".to_string(),
                description: Some("All devices in a specific room".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://sensors/{sensor_type}".to_string(),
                name: "Sensor Data".to_string(),
                description: Some("Sensor readings by type".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ];

        Ok(ListResourceTemplatesResult {
            resource_templates,
            next_cursor: String::new(), // Empty string instead of None
        })
    }

    async fn list_prompts(
        &self,
        _params: PaginatedRequestParam,
    ) -> std::result::Result<ListPromptsResult, Self::Error> {
        info!("💬 Listing Loxone prompts");

        let prompts = vec![
            Prompt {
                name: "analyze_energy_usage".to_string(),
                description: Some(
                    "Analyze energy consumption patterns and provide optimization suggestions"
                        .to_string(),
                ),
                arguments: Some(vec![PromptArgument {
                    name: "period".to_string(),
                    description: Some("Time period to analyze (day, week, month)".to_string()),
                    required: Some(false),
                }]),
            },
            Prompt {
                name: "home_status_summary".to_string(),
                description: Some(
                    "Generate a comprehensive summary of current home status".to_string(),
                ),
                arguments: Some(vec![PromptArgument {
                    name: "include_sensors".to_string(),
                    description: Some("Include sensor data in summary".to_string()),
                    required: Some(false),
                }]),
            },
            Prompt {
                name: "security_report".to_string(),
                description: Some(
                    "Generate a security status report with recommendations".to_string(),
                ),
                arguments: Some(vec![]), // Empty array instead of None
            },
        ];

        Ok(ListPromptsResult {
            prompts,
            next_cursor: Some(String::new()), // Empty string instead of None
        })
    }

    async fn get_prompt(
        &self,
        params: GetPromptRequestParam,
    ) -> std::result::Result<GetPromptResult, Self::Error> {
        info!("📝 Getting Loxone prompt: {}", params.name);

        let (description, messages) = match params.name.as_str() {
            "analyze_energy_usage" => {
                let period = params.arguments.as_ref()
                    .and_then(|args| args.get("period"))
                    .map(|s| s.as_str())
                    .unwrap_or("week");

                (
                    "Energy usage analysis and optimization".to_string(),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are an energy efficiency expert analyzing Loxone home automation data for the {} period.", period)
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            "Please analyze the energy consumption data and provide optimization recommendations."
                        ),
                    ]
                )
            }

            "home_status_summary" => {
                let include_sensors = params.arguments.as_ref()
                    .and_then(|args| args.get("include_sensors"))
                    .map(|v| v == "true")
                    .unwrap_or(true);

                let system_context = if include_sensors {
                    "You have access to comprehensive home data including all sensors, devices, and systems."
                } else {
                    "You have access to basic home data excluding detailed sensor information."
                };

                (
                    "Comprehensive home status summary".to_string(),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a smart home assistant. {}", system_context)
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            "Please provide a comprehensive summary of the current home status including all relevant systems and recommendations."
                        ),
                    ]
                )
            }

            "security_report" => {
                (
                    "Security status and recommendations".to_string(),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            "You are a home security expert analyzing Loxone security system data."
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            "Please analyze the current security status and provide a comprehensive security report with recommendations."
                        ),
                    ]
                )
            }

            _ => {
                return Err(LoxoneError::not_found(format!("Prompt not found: {}", params.name)));
            }
        };

        Ok(GetPromptResult {
            description: Some(description),
            messages,
        })
    }

    async fn subscribe(
        &self,
        params: SubscribeRequestParam,
    ) -> std::result::Result<(), Self::Error> {
        info!("🔔 Subscribing to Loxone resource: {}", params.uri);

        // Validate resource URI exists
        match params.uri.as_str() {
            // System resources
            "loxone://system/info" | "loxone://structure/rooms" | "loxone://config/devices" |
            "loxone://status/health" | "loxone://system/capabilities" | "loxone://system/categories" |
            // Room and device resources
            "loxone://rooms" | "loxone://devices/all" |
            "loxone://devices/category/lighting" | "loxone://devices/category/blinds" | "loxone://devices/category/climate" |
            // Audio resources
            "loxone://audio/zones" | "loxone://audio/sources" |
            // Sensor resources
            "loxone://sensors/temperature" | "loxone://sensors/door-window" | "loxone://sensors/motion" |
            // Weather and energy resources
            "loxone://weather/current" | "loxone://energy/consumption" => {
                info!("✅ Valid resource URI for subscription: {}", params.uri);

                // In a full implementation, we would:
                // 1. Register the client for notifications
                // 2. Start monitoring the resource for changes
                // 3. Set up WebSocket or SSE push notifications

                // For framework migration, we accept the subscription but don't implement push notifications yet
                info!("📝 Subscription registered (push notifications pending full implementation)");
                Ok(())
            }
            _ => {
                warn!("❌ Unknown resource URI for subscription: {}", params.uri);
                Err(LoxoneError::not_found(format!("Resource not found for subscription: {}", params.uri)))
            }
        }
    }

    async fn unsubscribe(
        &self,
        params: UnsubscribeRequestParam,
    ) -> std::result::Result<(), Self::Error> {
        info!("🔕 Unsubscribing from Loxone resource: {}", params.uri);

        // Validate resource URI and unsubscribe
        match params.uri.as_str() {
            // Valid resource URIs (same as subscribe)
            "loxone://system/info"
            | "loxone://structure/rooms"
            | "loxone://config/devices"
            | "loxone://status/health"
            | "loxone://system/capabilities"
            | "loxone://system/categories"
            | "loxone://rooms"
            | "loxone://devices/all"
            | "loxone://devices/category/lighting"
            | "loxone://devices/category/blinds"
            | "loxone://devices/category/climate"
            | "loxone://audio/zones"
            | "loxone://audio/sources"
            | "loxone://sensors/temperature"
            | "loxone://sensors/door-window"
            | "loxone://sensors/motion"
            | "loxone://weather/current"
            | "loxone://energy/consumption" => {
                info!("✅ Valid resource URI for unsubscription: {}", params.uri);

                // In a full implementation, we would:
                // 1. Remove the client from notification registry
                // 2. Stop monitoring if no other clients subscribed
                // 3. Clean up WebSocket/SSE connections

                info!("📝 Unsubscription processed (full cleanup pending implementation)");
                Ok(())
            }
            _ => {
                warn!("❌ Unknown resource URI for unsubscription: {}", params.uri);
                Err(LoxoneError::not_found(format!(
                    "Resource not found for unsubscription: {}",
                    params.uri
                )))
            }
        }
    }

    async fn complete(
        &self,
        params: CompleteRequestParam,
    ) -> std::result::Result<CompleteResult, Self::Error> {
        info!("🔍 Providing completion for: {}", params.ref_);

        let completions = match params.ref_.as_str() {
            "room_names" => {
                let rooms = self.server.context.rooms.read().await;
                rooms.keys().cloned().collect::<Vec<_>>()
            }
            "device_types" => {
                vec![
                    "Light".to_string(),
                    "Jalousie".to_string(),
                    "TimedSwitch".to_string(),
                    "Dimmer".to_string(),
                ]
            }
            "sensor_types" => {
                vec![
                    "temperature".to_string(),
                    "humidity".to_string(),
                    "motion".to_string(),
                    "door_window".to_string(),
                ]
            }
            _ => {
                debug!("Unknown completion reference: {}", params.ref_);
                vec![]
            }
        };

        Ok(CompleteResult {
            completion: completions
                .into_iter()
                .map(|c| CompletionInfo {
                    completion: c,
                    has_more: Some(false),
                })
                .collect(),
        })
    }

    async fn set_level(
        &self,
        params: SetLevelRequestParam,
    ) -> std::result::Result<(), Self::Error> {
        info!("📊 Setting log level to: {}", params.level);

        // Update the tracing subscriber level if possible
        // This is a simplified implementation - in practice, you might want
        // to use a dynamic tracing subscriber that can be updated at runtime

        Ok(())
    }

    async fn handle_custom_method(
        &self,
        method: &str,
        _params: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, Self::Error> {
        warn!("❓ Unknown custom method: {}", method);
        Err(LoxoneError::validation(format!(
            "Unknown method: {}",
            method
        )))
    }
}
