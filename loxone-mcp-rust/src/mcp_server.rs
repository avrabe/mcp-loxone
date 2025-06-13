//! Proper MCP server implementation using 4t145/rmcp
//!
//! This module implements the MCP server using the correct API from the rmcp crate.

use crate::client::{create_client, ClientContext, LoxoneClient};
use crate::config::ServerConfig;
use crate::error::{LoxoneError, Result};

use rmcp::{
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;
use tokio::io::{stdin, stdout};
use tracing::{error, info, warn};

/// Device control request parameters
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeviceControlRequest {
    #[schemars(description = "Device UUID")]
    pub device_id: String,
    #[schemars(description = "Action to perform (on, off, up, down, stop)")]
    pub action: String,
}

/// Room control request parameters
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RoomControlRequest {
    #[schemars(description = "Room name")]
    pub room_name: String,
    #[schemars(description = "Action to perform")]
    pub action: String,
}

/// Temperature control request parameters
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TemperatureRequest {
    #[schemars(description = "Room name")]
    pub room_name: String,
    #[schemars(description = "Target temperature in Celsius")]
    pub temperature: f64,
}

/// Room devices request parameters
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RoomDevicesRequest {
    #[schemars(description = "Name of the room")]
    pub room_name: String,
}

/// Main MCP server for Loxone control
#[derive(Clone)]
pub struct LoxoneMcpServer {
    /// Server configuration
    #[allow(dead_code)]
    config: ServerConfig,

    /// Loxone client
    client: Arc<dyn LoxoneClient>,

    /// Client context for caching
    context: Arc<ClientContext>,
}

impl LoxoneMcpServer {
    /// Create new MCP server instance
    pub async fn new(mut config: ServerConfig) -> Result<Self> {
        info!("🚀 Initializing Loxone MCP server...");

        // Create credential manager with proper async initialization
        info!("📋 Initializing credential manager...");
        let credential_manager =
            match crate::config::credentials::create_best_credential_manager().await {
                Ok(manager) => {
                    info!("✅ Created multi-backend credential manager");
                    manager
                }
                Err(e) => {
                    error!(
                        "❌ Failed to create multi-backend credential manager: {}",
                        e
                    );
                    error!("");
                    error!("🚀 Quick Setup Guide:");
                    error!("");
                    error!("Option 1: Use environment variables (simplest):");
                    error!("  export LOXONE_USER=<your-username>");
                    error!("  export LOXONE_PASS=<your-password>");
                    error!("  export LOXONE_HOST=<miniserver-ip-or-hostname>");
                    error!("");
                    error!("Option 2: Use keychain (interactive setup):");
                    error!("  cargo run --bin setup");
                    error!("");
                    error!("Option 3: Use Infisical (for teams):");
                    error!("  export INFISICAL_PROJECT_ID=\"your-project-id\"");
                    error!("  export INFISICAL_CLIENT_ID=\"your-client-id\"");
                    error!("  export INFISICAL_CLIENT_SECRET=\"your-client-secret\"");
                    error!("");
                    error!("For more info: https://github.com/your-repo/loxone-mcp-rust#setup");
                    return Err(e);
                }
            };

        // Load credentials using the credential manager (handles Infisical → Env → Keychain priority)
        info!("🔄 Loading credentials from best available backend...");
        let credentials = match credential_manager.get_credentials().await {
            Ok(creds) => {
                info!("✅ Credentials loaded for user: {}", creds.username);
                creds
            }
            Err(e) => {
                error!("❌ Failed to load credentials: {}", e);
                error!("");
                error!("🔧 No credentials found! Please configure using one of these methods:");
                error!("");
                error!("1️⃣  Environment Variables (quickest for testing):");
                error!("    export LOXONE_USER=\"your-username\"");
                error!("    export LOXONE_PASS=\"your-password\"");
                error!("    export LOXONE_HOST=\"192.168.1.100\"  # your miniserver IP");
                error!("");
                error!("2️⃣  Interactive Setup (stores in system keychain):");
                error!("    cargo run --bin setup");
                error!("    # Follow the prompts to enter credentials");
                error!("");
                error!("3️⃣  Infisical (for teams - no extra installation needed!):");
                error!("    # Example setup (replace with your values):");
                error!("    export INFISICAL_PROJECT_ID=\"65f8e2c8a8b7d9001c4f2a3b\"");
                error!("    export INFISICAL_CLIENT_ID=\"6f4d8e91-3a2b-4c5d-9e7f-1a2b3c4d5e6f\"");
                error!("    export INFISICAL_CLIENT_SECRET=\"st.abc123...\"  # Token from https://app.infisical.com");
                error!("    export INFISICAL_ENVIRONMENT=\"dev\"");
                error!("");
                error!("    Guide: ./INFISICAL_SETUP.md or https://app.infisical.com/signup");
                error!("");
                error!(
                    "📚 Documentation: https://github.com/your-repo/loxone-mcp-rust#credentials"
                );
                return Err(e);
            }
        };

        // Handle host URL: environment variables take precedence over keychain
        if let Ok(host) = std::env::var("LOXONE_HOST") {
            if let Ok(url) = host.parse() {
                config.loxone.url = url;
                info!("📍 Using host URL from environment: {}", host);
            } else {
                warn!(
                    "⚠️ Failed to parse LOXONE_HOST environment variable: {}",
                    host
                );
            }
        } else {
            info!("💡 Using default URL: {}", config.loxone.url);
        }

        // Create Loxone client
        let mut client = create_client(&config.loxone, &credentials).await?;
        info!("✅ Loxone client created");

        // Test connection
        client.connect().await?;
        info!("✅ Connected to Loxone Miniserver");

        // Create client context
        let context = Arc::new(ClientContext::new());

        // Load structure data
        match client.get_structure().await {
            Ok(structure) => {
                context.update_structure(structure).await?;
                info!("✅ Structure data loaded and cached");
            }
            Err(e) => {
                warn!("⚠️ Failed to load structure data: {}", e);
                // Continue without structure data - tools will handle this gracefully
            }
        }

        // Initialize sensor state logger
        let log_file = std::path::PathBuf::from("sensor_state_log.json");
        if let Err(e) = context.initialize_sensor_logger(log_file).await {
            warn!("⚠️ Failed to initialize sensor state logger: {}", e);
        } else {
            info!("✅ Sensor state logger initialized");
        }

        Ok(Self {
            config,
            client: Arc::from(client),
            context,
        })
    }

    /// Run the server with stdio transport
    pub async fn run_stdio(&self) -> Result<()> {
        info!("🎉 Starting Loxone MCP server with stdio transport");

        let service = self
            .clone()
            .serve((stdin(), stdout()))
            .await
            .map_err(|e| LoxoneError::connection(format!("Failed to start server: {}", e)))?;

        info!("✅ MCP server started successfully");

        // Keep server running
        let quit_reason = service
            .waiting()
            .await
            .map_err(|e| LoxoneError::connection(format!("Server error: {}", e)))?;

        info!("🛑 Server stopped: {:?}", quit_reason);
        Ok(())
    }
}

impl LoxoneMcpServer {
    /// List all rooms in the Loxone system with device counts
    pub async fn list_rooms(&self) -> std::result::Result<CallToolResult, rmcp::Error> {
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
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
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
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
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
    pub async fn get_system_status(&self) -> std::result::Result<CallToolResult, rmcp::Error> {
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
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
        let devices = self.context.devices.read().await;
        let rooms = self.context.rooms.read().await;

        // Find room by name
        let room_uuid = rooms
            .iter()
            .find(|(_, room)| room.name.to_lowercase() == room_name.to_lowercase())
            .map(|(uuid, _)| uuid.clone());

        if room_uuid.is_none() {
            let content = format!("❌ Room '{}' not found", room_name);
            return Ok(CallToolResult::error(vec![Content::text(content)]));
        }

        let room_uuid = room_uuid.unwrap();
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
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
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
            Ok(CallToolResult::error(vec![Content::text(format!(
                "Device '{}' not found",
                device
            ))]))
        }
    }

    /// Control all rolladen in the system
    pub async fn control_all_rolladen(
        &self,
        action: String,
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
        let devices = self.context.devices.read().await;
        let rolladen_devices: Vec<_> = devices
            .values()
            .filter(|device| device.device_type == "Jalousie")
            .collect();

        if rolladen_devices.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "No rolladen/blinds found in the system".to_string(),
            )]));
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
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
        let devices = self.context.devices.read().await;
        let rooms = self.context.rooms.read().await;

        // Find room by name
        let room_uuid = rooms
            .iter()
            .find(|(_, room)| room.name.to_lowercase() == room_name.to_lowercase())
            .map(|(uuid, _)| uuid.clone());

        if room_uuid.is_none() {
            return Ok(CallToolResult::error(vec![Content::text(format!(
                "Room '{}' not found",
                room_name
            ))]));
        }

        let room_uuid = room_uuid.unwrap();
        let rolladen_devices: Vec<_> = devices
            .values()
            .filter(|device| {
                device.device_type == "Jalousie" && (device.room.as_ref() == Some(&room_uuid))
            })
            .collect();

        if rolladen_devices.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "No rolladen/blinds found in room '{}'",
                room_name
            ))]));
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
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
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
            return Ok(CallToolResult::success(vec![Content::text(
                "No lights found in the system".to_string(),
            )]));
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
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
        let devices = self.context.devices.read().await;
        let rooms = self.context.rooms.read().await;

        // Find room by name
        let room_uuid = rooms
            .iter()
            .find(|(_, room)| room.name.to_lowercase() == room_name.to_lowercase())
            .map(|(uuid, _)| uuid.clone());

        if room_uuid.is_none() {
            return Ok(CallToolResult::error(vec![Content::text(format!(
                "Room '{}' not found",
                room_name
            ))]));
        }

        let room_uuid = room_uuid.unwrap();
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
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "No lights found in room '{}'",
                room_name
            ))]));
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

    /// Discover all devices in the system
    pub async fn discover_all_devices(&self) -> std::result::Result<CallToolResult, rmcp::Error> {
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
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
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

    /// Get audio zones and their status
    pub async fn get_audio_zones(&self) -> std::result::Result<CallToolResult, rmcp::Error> {
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
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
        let context = crate::tools::ToolContext::new(self.client.clone(), self.context.clone());

        let result =
            crate::tools::audio::control_audio_zone(context, zone_name, action, value).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Get available audio sources
    pub async fn get_audio_sources(&self) -> std::result::Result<CallToolResult, rmcp::Error> {
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
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
        let context = crate::tools::ToolContext::new(self.client.clone(), self.context.clone());

        let result = crate::tools::audio::set_audio_volume(context, zone_name, volume).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Get sensor state history
    pub async fn get_sensor_state_history(
        &self,
        uuid: String,
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
        let context = crate::tools::ToolContext::new(self.client.clone(), self.context.clone());

        let logger = self.context.get_sensor_logger().await;
        let result = crate::tools::sensors::get_sensor_state_history(context, uuid, logger).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Get recent sensor changes
    pub async fn get_recent_sensor_changes(
        &self,
        limit: Option<usize>,
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
        let context = crate::tools::ToolContext::new(self.client.clone(), self.context.clone());

        let logger = self.context.get_sensor_logger().await;
        let result = crate::tools::sensors::get_recent_sensor_changes(context, limit, logger).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Get door/window activity
    pub async fn get_door_window_activity(
        &self,
        hours: Option<u32>,
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
        let context = crate::tools::ToolContext::new(self.client.clone(), self.context.clone());

        let logger = self.context.get_sensor_logger().await;
        let result = crate::tools::sensors::get_door_window_activity(context, hours, logger).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Get logging statistics
    pub async fn get_logging_statistics_tool(
        &self,
    ) -> std::result::Result<CallToolResult, rmcp::Error> {
        let context = crate::tools::ToolContext::new(self.client.clone(), self.context.clone());

        let logger = self.context.get_sensor_logger().await;
        let result = crate::tools::sensors::get_logging_statistics(context, logger).await;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Public method to call tools for HTTP transport
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, String> {
        match tool_name {
            "list_rooms" => match self.list_rooms().await {
                Ok(result) => self.convert_tool_result(result),
                Err(e) => Err(format!("Failed to list rooms: {}", e)),
            },
            "get_room_devices" => {
                let room_name = arguments
                    .get("room_name")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing room_name parameter")?;
                let device_type = arguments
                    .get("device_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                match self
                    .get_room_devices_enhanced(room_name.to_string(), device_type)
                    .await
                {
                    Ok(result) => self.convert_tool_result(result),
                    Err(e) => Err(format!("Failed to get room devices: {}", e)),
                }
            }
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
            "discover_all_devices" => match self.discover_all_devices().await {
                Ok(result) => self.convert_tool_result(result),
                Err(e) => Err(format!("Failed to discover devices: {}", e)),
            },
            "get_devices_by_type" => {
                let device_type = arguments
                    .get("device_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                match self.get_devices_by_type(device_type).await {
                    Ok(result) => self.convert_tool_result(result),
                    Err(e) => Err(format!("Failed to get devices by type: {}", e)),
                }
            }
            "get_system_status" => match self.get_system_status().await {
                Ok(result) => self.convert_tool_result(result),
                Err(e) => Err(format!("Failed to get system status: {}", e)),
            },
            "get_audio_zones" => match self.get_audio_zones().await {
                Ok(result) => self.convert_tool_result(result),
                Err(e) => Err(format!("Failed to get audio zones: {}", e)),
            },
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
            "get_audio_sources" => match self.get_audio_sources().await {
                Ok(result) => self.convert_tool_result(result),
                Err(e) => Err(format!("Failed to get audio sources: {}", e)),
            },
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
                match self.get_sensor_state_history(uuid.to_string()).await {
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
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
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
}

