//! Loxone MCP Server - Main Entry Point
//!
//! This is the main binary entry point for the Loxone MCP server.
//! It supports both native and WASM32-WASIP2 compilation targets.

use loxone_mcp_rust::{
    auth::AuthenticationManager,
    http_transport::{HttpServerConfig, HttpTransportServer},
    security::SecurityConfig,
    server::LoxoneMcpServer,
    Result, ServerConfig,
};
#[cfg(feature = "influxdb")]
use loxone_mcp_rust::monitoring;

use clap::{Parser, Subcommand};
use tracing::{error, info};

/// Command line arguments
#[derive(Parser)]
#[command(name = "loxone-mcp-server")]
#[command(about = "Loxone MCP Server in Rust")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run server with stdio transport (for Claude Desktop)
    Stdio,
    /// Run server with HTTP/SSE transport (for n8n and web clients)
    Http {
        /// Port to bind to
        #[arg(short, long, default_value = "3001")]
        port: u16,
        /// API key for authentication
        #[arg(long)]
        api_key: Option<String>,
        /// Show admin access URL with API key on startup
        #[arg(long)]
        show_access_url: bool,
        /// Enable development mode (bypasses authentication)
        #[arg(long)]
        dev_mode: bool,
    },
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize enhanced logging
    let log_config = loxone_mcp_rust::logging::LogConfig::from_env();
    if let Err(e) = loxone_mcp_rust::logging::init_logging(log_config) {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }

    // Handle commands
    match cli.command {
        Commands::Stdio => {
            info!("🚀 Starting Loxone MCP Server with stdio transport (Claude Desktop mode)");
            
            // Load configuration (required for stdio mode)
            let config = match ServerConfig::from_env() {
                Ok(config) => config,
                Err(e) => {
                    error!("Failed to load configuration: {}", e);
                    error!("💡 Run credential setup first or check environment variables");
                    std::process::exit(1);
                }
            };
            
            run_stdio_server(config).await?;
        }
        Commands::Http { port, api_key: _, show_access_url, dev_mode } => {
            info!(
                "🌐 Starting Loxone MCP Server with HTTP/SSE transport (n8n mode) on port {}",
                port
            );

            // In dev mode or when showing access URL, we might not need full Loxone credentials
            let config = if dev_mode {
                info!("🚧 Development mode - using minimal configuration");
                // Set environment variables for auth middleware and dummy credentials
                std::env::set_var("DEV_MODE", "1");
                std::env::set_var("LOXONE_USERNAME", "dev");
                std::env::set_var("LOXONE_PASSWORD", "dev");
                std::env::set_var("LOXONE_HOST", "localhost:8080");
                // Create a minimal config for dev mode
                ServerConfig::dev_mode()
            } else {
                match ServerConfig::from_env() {
                    Ok(config) => config,
                    Err(e) => {
                        error!("Failed to load configuration: {}", e);
                        error!("💡 Run credential setup first or check environment variables");
                        error!("💡 Or use --dev-mode to bypass Loxone connection");
                        std::process::exit(1);
                    }
                }
            };

            run_http_server(config, port, show_access_url, dev_mode).await?;
        }
    }

    Ok(())
}

/// Create authentication manager for first-run setup
async fn create_auth_manager() -> Result<AuthenticationManager> {
    AuthenticationManager::new().await
}

/// Run server with stdio transport (for Claude Desktop)
async fn run_stdio_server(config: ServerConfig) -> Result<()> {
    let server = LoxoneMcpServer::new(config).await?;
    info!("✅ MCP server initialized successfully");
    server.run().await
}


/// Run server with HTTP/SSE transport (for n8n and web clients)
async fn run_http_server(config: ServerConfig, port: u16, show_access_url: bool, dev_mode: bool) -> Result<()> {
    // Create MCP server
    let mcp_server = LoxoneMcpServer::new(config).await?;
    info!("✅ MCP server initialized successfully");

    // Handle first-run setup and access URL display
    if dev_mode {
        info!("🚧 Development mode ENABLED - Authentication BYPASSED");
        info!("   ⚠️  WARNING: This mode should NOT be used in production!");
        info!("   🌐 Access server: http://localhost:{}/admin", port);
    } else {
        info!("🔐 Authentication ENABLED");
        
        // Check if we need first-run setup
        let auth_manager = create_auth_manager().await?;
        let existing_keys = auth_manager.list_keys().await;
        
        if existing_keys.is_empty() {
            info!("🚀 First run detected! Setting up admin access...");
            
            // Create initial admin key
            let admin_key = auth_manager.create_key(
                "Auto-generated Admin Key".to_string(),
                loxone_mcp_rust::auth::models::Role::Admin,
                "first-run-setup".to_string(),
                None, // No expiration
            ).await?;
            
            info!("✅ Admin key created successfully!");
            info!("🔑 API Key: {}", admin_key.secret);
            info!("🌐 Admin URL: http://localhost:{}/admin?api_key={}", port, admin_key.secret);
            info!("");
            info!("⚠️  IMPORTANT: Save this API key - it won't be shown again!");
            info!("   Use 'loxone-mcp-auth list' to manage keys later");
        } else if show_access_url {
            // Find an admin key and display URL
            if let Some(admin_key) = existing_keys.iter().find(|k| matches!(k.role, loxone_mcp_rust::auth::models::Role::Admin)) {
                info!("🌐 Admin Access URL: http://localhost:{}/admin?api_key={}", port, admin_key.secret);
                info!("🔑 Admin Key ID: {}", admin_key.id);
            } else {
                info!("⚠️  No admin keys found. Create one with:");
                info!("   loxone-mcp-auth create --name \"Admin Key\" --role admin");
            }
        } else {
            info!("   Use 'loxone-mcp-auth' CLI to manage API keys");
            info!("   Or visit http://localhost:{}/admin/keys with valid API key", port);
            info!("   💡 Use --show-access-url flag to display ready-to-use URL");
        }
    }

    // Create HTTP server configuration with security based on environment and dev mode
    let security_config = if dev_mode {
        None // Bypass security in dev mode
    } else if std::env::var("PRODUCTION").is_ok() {
        Some(SecurityConfig::production())
    } else if std::env::var("DISABLE_SECURITY").is_ok() {
        None
    } else {
        Some(SecurityConfig::development())
    };

    // Configure performance monitoring based on environment
    let performance_config = if std::env::var("DISABLE_PERFORMANCE").is_ok() {
        None
    } else if std::env::var("PRODUCTION").is_ok() {
        Some(loxone_mcp_rust::performance::PerformanceConfig::production())
    } else {
        Some(loxone_mcp_rust::performance::PerformanceConfig::development())
    };

    // Configure InfluxDB if enabled
    #[cfg(feature = "influxdb")]
    let influx_config = if std::env::var("ENABLE_INFLUXDB").is_ok() || std::env::var("INFLUXDB_TOKEN").is_ok() {
        info!("📊 InfluxDB integration enabled");
        Some(crate::monitoring::influxdb::InfluxConfig::default())
    } else {
        info!("📊 InfluxDB integration disabled (set ENABLE_INFLUXDB=1 or INFLUXDB_TOKEN to enable)");
        None
    };

    let http_config = HttpServerConfig {
        port,
        security_config,
        performance_config,
        dev_mode,
        #[cfg(feature = "influxdb")]
        influx_config,
    };

    // Create and start HTTP transport server
    let http_server = HttpTransportServer::new(mcp_server, http_config).await?;
    http_server.start().await
}

