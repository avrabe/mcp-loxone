//! Loxone MCP Server Example
//!
//! This demonstrates how to use the separated mcp-framework with a domain-specific
//! Loxone backend implementation for home automation control.

use loxone_backend::{LoxoneBackend, LoxoneConfig};
use mcp_server::{McpServer, ServerConfig, McpBackend};
use mcp_transport::TransportConfig;

use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("loxone_mcp=debug,mcp_server=debug,loxone_backend=debug"))
        )
        .init();

    info!("🏠 Starting Loxone MCP Server");

    // Load Loxone configuration
    let loxone_config = match LoxoneConfig::load().await {
        Ok(config) => {
            info!("✅ Loxone configuration loaded successfully");
            config
        }
        Err(e) => {
            eprintln!("❌ Failed to load Loxone configuration: {}", e);
            eprintln!("💡 Please ensure the following environment variables are set:");
            eprintln!("   LOXONE_HOST=<your-miniserver-ip>");
            eprintln!("   LOXONE_USER=<your-username>");
            eprintln!("   LOXONE_PASS=<your-password>");
            eprintln!("   Example: LOXONE_HOST=192.168.1.100 LOXONE_USER=admin LOXONE_PASS=your-password");
            std::process::exit(1);
        }
    };

    // Create Loxone backend
    let backend = LoxoneBackend::initialize(loxone_config).await?;
    info!("✅ Loxone backend initialized");

    // Create server configuration
    let mut auth_config = mcp_auth::default_config();
    auth_config.enabled = false; // Disable authentication for this example

    let server_config = ServerConfig {
        server_info: backend.get_server_info(),
        auth_config,
        transport_config: TransportConfig::Stdio, // Use stdio transport for MCP clients
        ..Default::default()
    };

    // Create and start server
    let mut server = McpServer::new(backend, server_config).await?;

    info!("✅ Loxone MCP Server started successfully");
    info!("🏠 Available tools:");
    info!("   • control_lights_unified - Control lighting devices and rooms");
    info!("   • list_rooms - List all rooms in the system");
    info!("   • get_room_details - Get detailed room information");
    info!("📦 Available resources:");
    info!("   • loxone://structure - Complete system structure");
    info!("   • loxone://rooms - All rooms");
    info!("   • loxone://devices - All devices");
    info!("🔗 Connect using any MCP client via stdio");
    info!("💡 Example usage in Claude Desktop:");
    info!("   - 'Turn on the living room lights'");
    info!("   - 'List all rooms'");
    info!("   - 'Show me details for the kitchen'");

    // Run server until shutdown
    server.run().await?;

    info!("👋 Loxone MCP Server stopped");
    Ok(())
}