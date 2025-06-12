//! MCP server implementation using the Rust MCP SDK
//!
//! This module implements the main MCP server that registers all tools
//! and handles the connection lifecycle with Loxone systems.

use crate::client::{create_client, ClientContext, LoxoneClient};
use crate::config::{ServerConfig, credentials::CredentialManager};
use crate::error::{LoxoneError, Result};
// use crate::tools::{ToolContext, ToolResponse}; // TODO: Re-enable after fixing tool syntax
// use rmcp::{
//     server::{Server, ServerHandler},
//     transport::Transport,
//     tool,
// };
use rmcp::tool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error, warn, debug};

/// Main MCP server for Loxone control
pub struct LoxoneMcpServer {
    /// Server configuration
    config: ServerConfig,
    
    /// Loxone client
    client: Arc<dyn LoxoneClient>,
    
    /// Client context for caching
    context: Arc<ClientContext>,
    
    /// Tool context for MCP tools
    // tool_context: ToolContext, // TODO: Re-enable when tools are fixed
    
    /// MCP server instance
    // mcp_server: Arc<RwLock<Option<Server>>>, // TODO: Re-enable when rmcp API is clarified
}

impl LoxoneMcpServer {
    /// Create new MCP server instance
    pub async fn new(config: ServerConfig) -> Result<Self> {
        info!("🚀 Initializing Loxone MCP Server (Rust)");
        
        // Validate configuration
        config.validate()?;
        
        // Get credentials
        let credential_manager = CredentialManager::new(config.credentials.clone());
        let credentials = credential_manager.get_credentials().await?;
        
        // Create Loxone client
        let client = create_client(&config.loxone, &credentials).await?;
        let client = Arc::new(client);
        
        // Create client context
        let context = Arc::new(ClientContext::new());
        
        // Create tool context
        let tool_context = ToolContext::new(client.clone(), context.clone());
        
        info!("✅ Server components initialized");
        
        Ok(Self {
            config,
            client,
            context,
            tool_context,
            mcp_server: Arc::new(RwLock::new(None)),
        })
    }
    
    /// Start the MCP server
    pub async fn run(&self) -> Result<()> {
        info!("🌐 Starting Loxone MCP Server");
        
        // Connect to Loxone
        self.connect_to_loxone().await?;
        
        // Create MCP server with tools
        let server = self.create_mcp_server().await?;
        
        // Store server instance
        *self.mcp_server.write().await = Some(server);
        
        // Start server based on transport configuration
        match self.config.mcp.transport.transport_type.as_str() {
            "stdio" => {
                info!("📡 Starting MCP server with stdio transport");
                self.run_stdio().await
            }
            "http" => {
                info!("🌐 Starting MCP server with HTTP transport");
                self.run_http().await
            }
            _ => {
                Err(LoxoneError::config(format!(
                    "Unsupported transport type: {}", 
                    self.config.mcp.transport.transport_type
                )))
            }
        }
    }
    
    /// Connect to Loxone Miniserver
    async fn connect_to_loxone(&self) -> Result<()> {
        info!("🔌 Connecting to Loxone Miniserver at {}", self.config.loxone.url);
        
        // Get mutable reference to client for connection
        // Note: This is a simplified approach. In practice, you might need
        // to use Arc<Mutex<>> for mutable operations
        
        // For now, we'll assume the client connects during creation
        // and implement a reconnection strategy separately
        
        // Check if already connected
        if self.client.is_connected().await? {
            info!("✅ Already connected to Loxone");
            return Ok(());
        }
        
        // Attempt connection with retries
        let mut attempts = 0;
        let max_attempts = self.config.loxone.max_retries;
        
        while attempts < max_attempts {
            attempts += 1;
            
            match self.client.health_check().await {
                Ok(true) => {
                    info!("✅ Connected to Loxone Miniserver");
                    
                    // Load system capabilities
                    self.load_system_capabilities().await?;
                    
                    return Ok(());
                }
                Ok(false) => {
                    warn!("❌ Loxone health check failed (attempt {}/{})", attempts, max_attempts);
                }
                Err(e) => {
                    error!("❌ Connection error (attempt {}/{}): {}", attempts, max_attempts, e);
                }
            }
            
            if attempts < max_attempts {
                let delay = std::time::Duration::from_secs(2_u64.pow(attempts.min(5)));
                debug!("⏳ Retrying connection in {:?}", delay);
                tokio::time::sleep(delay).await;
            }
        }
        
        Err(LoxoneError::connection(format!(
            "Failed to connect to Loxone after {} attempts", max_attempts
        )))
    }
    
    /// Load system capabilities and structure
    async fn load_system_capabilities(&self) -> Result<()> {
        info!("📋 Loading Loxone system structure and capabilities");
        
        match self.client.get_structure().await {
            Ok(structure) => {
                self.context.update_structure(structure).await?;
                
                let capabilities = self.context.capabilities.read().await;
                info!("🏠 System capabilities loaded:");
                info!("   💡 Lighting: {} devices", capabilities.light_count);
                info!("   🏗️ Blinds: {} devices", capabilities.blind_count);
                info!("   🌡️ Climate: {} devices", capabilities.climate_count);
                info!("   📡 Sensors: {} devices", capabilities.sensor_count);
                
                let rooms = self.context.rooms.read().await;
                info!("   🏠 Rooms: {}", rooms.len());
                
                Ok(())
            }
            Err(e) => {
                warn!("⚠️ Failed to load structure: {}", e);
                warn!("   Server will start but some features may be limited");
                Ok(()) // Don't fail startup for structure loading issues
            }
        }
    }
    
    /// Create MCP server with all tools registered
    async fn create_mcp_server(&self) -> Result<Server> {
        info!("🔧 Creating MCP server with tool registration");
        
        let mut server_builder = Server::new(&self.config.mcp.name, &self.config.mcp.version);
        
        // Register all tools
        self.register_tools(&mut server_builder).await?;
        
        let server = server_builder.build()
            .map_err(|e| LoxoneError::Mcp(format!("Failed to build MCP server: {}", e)))?;
        
        info!("✅ MCP server created with all tools registered");
        Ok(server)
    }
    
    /// Register all MCP tools
    async fn register_tools(&self, _server: &mut Server) -> Result<()> {
        // TODO: Re-implement when tools module is fixed
        /*
        let tool_context = self.tool_context.clone();
        
        // Room management tools
        server.add_tool("list_rooms", |_| async move {
            crate::tools::rooms::list_rooms(tool_context.clone()).await
        });
        
        server.add_tool("get_room_devices", |params| async move {
            let room_name = params.get("room_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let category = params.get("category")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let limit = params.get("limit")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize);
            
            crate::tools::rooms::get_room_devices(tool_context.clone(), room_name, category, limit).await
        });
        
        server.add_tool("control_room_lights", |params| async move {
            let room_name = params.get("room_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let action = params.get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("on")
                .to_string();
            
            crate::tools::rooms::control_room_lights(tool_context.clone(), room_name, action).await
        });
        
        // Device control tools
        server.add_tool("discover_all_devices", |params| async move {
            let category = params.get("category")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let device_type = params.get("device_type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let limit = params.get("limit")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize);
            
            crate::tools::devices::discover_all_devices(tool_context.clone(), category, device_type, limit).await
        });
        
        server.add_tool("control_device", |params| async move {
            let device = params.get("device")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let action = params.get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("on")
                .to_string();
            
            crate::tools::devices::control_device(tool_context.clone(), device, action).await
        });
        
        server.add_tool("control_all_lights", |params| async move {
            let action = params.get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("on")
                .to_string();
            
            crate::tools::devices::control_all_lights(tool_context.clone(), action).await
        });
        
        server.add_tool("control_all_rolladen", |params| async move {
            let action = params.get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("up")
                .to_string();
            
            crate::tools::devices::control_all_rolladen(tool_context.clone(), action).await
        });
        
        // Climate control tools
        server.add_tool("get_climate_control", |_| async move {
            crate::tools::climate::get_climate_control(tool_context.clone()).await
        });
        
        server.add_tool("get_room_climate", |params| async move {
            let room_name = params.get("room_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            
            crate::tools::climate::get_room_climate(tool_context.clone(), room_name).await
        });
        
        server.add_tool("set_room_temperature", |params| async move {
            let room_name = params.get("room_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let temperature = params.get("temperature")
                .and_then(|v| v.as_f64())
                .unwrap_or(20.0);
            
            crate::tools::climate::set_room_temperature(tool_context.clone(), room_name, temperature).await
        });
        
        // Sensor tools
        server.add_tool("get_all_door_window_sensors", |_| async move {
            crate::tools::sensors::get_all_door_window_sensors(tool_context.clone()).await
        });
        
        server.add_tool("list_discovered_sensors", |params| async move {
            let sensor_type = params.get("sensor_type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let room = params.get("room")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            
            crate::tools::sensors::list_discovered_sensors(tool_context.clone(), sensor_type, room).await
        });
        
        // Weather tools (if enabled)
        if self.config.tools.enable_weather {
            server.add_tool("get_weather_data", |_| async move {
                crate::tools::weather::get_weather_data(tool_context.clone()).await
            });
        }
        
        // info!("🔧 Registered {} MCP tools", server.tool_count());
        */
        Ok(())
    }
    
    /// Run server with stdio transport
    async fn run_stdio(&self) -> Result<()> {
        let server = self.mcp_server.read().await;
        let server = server.as_ref()
            .ok_or_else(|| LoxoneError::Mcp("Server not initialized".to_string()))?;
        
        // Create stdio transport
        let transport = Transport::stdio();
        
        info!("📡 MCP server listening on stdio");
        
        // Run server
        server.run(transport).await
            .map_err(|e| LoxoneError::Mcp(format!("Server error: {}", e)))?;
        
        Ok(())
    }
    
    /// Run server with HTTP transport
    async fn run_http(&self) -> Result<()> {
        let port = self.config.mcp.transport.port.unwrap_or(8080);
        let host = self.config.mcp.transport.host.as_deref().unwrap_or("127.0.0.1");
        
        let server = self.mcp_server.read().await;
        let server = server.as_ref()
            .ok_or_else(|| LoxoneError::Mcp("Server not initialized".to_string()))?;
        
        // Create HTTP transport
        let transport = Transport::http(format!("{}:{}", host, port));
        
        info!("🌐 MCP server listening on http://{}:{}", host, port);
        
        // Run server
        server.run(transport).await
            .map_err(|e| LoxoneError::Mcp(format!("Server error: {}", e)))?;
        
        Ok(())
    }
    
    /// Get server statistics
    pub async fn get_statistics(&self) -> serde_json::Value {
        let capabilities = self.context.capabilities.read().await;
        let devices = self.context.devices.read().await;
        let rooms = self.context.rooms.read().await;
        
        serde_json::json!({
            "server": {
                "name": self.config.mcp.name,
                "version": self.config.mcp.version,
                "transport": self.config.mcp.transport.transport_type,
                "connected": self.client.is_connected().await.unwrap_or(false)
            },
            "loxone": {
                "url": self.config.loxone.url,
                "total_devices": devices.len(),
                "total_rooms": rooms.len()
            },
            "capabilities": capabilities.clone(),
            "timestamp": chrono::Utc::now()
        })
    }
}