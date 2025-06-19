//! Loxone MCP Server - Main Entry Point
//!
//! This is the main binary entry point for the Loxone MCP server.
//! It supports both native and WASM32-WASIP2 compilation targets.

use loxone_mcp_rust::{
    http_transport::{HttpServerConfig, HttpTransportServer},
    security::SecurityConfig,
    server::LoxoneMcpServer,
    Result, ServerConfig,
};

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

    // Load configuration
    let config = match ServerConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            error!("💡 Run credential setup first or check environment variables");
            std::process::exit(1);
        }
    };

    // Handle commands
    match cli.command {
        Commands::Stdio => {
            info!("🚀 Starting Loxone MCP Server with stdio transport (Claude Desktop mode)");
            run_stdio_server(config).await?;
        }
        Commands::Http { port, api_key: _ } => {
            info!(
                "🌐 Starting Loxone MCP Server with HTTP/SSE transport (n8n mode) on port {}",
                port
            );

            run_http_server(config, port).await?;
        }
    }

    Ok(())
}

/// Run server with stdio transport (for Claude Desktop)
async fn run_stdio_server(config: ServerConfig) -> Result<()> {
    let server = LoxoneMcpServer::new(config).await?;
    info!("✅ MCP server initialized successfully");
    server.run().await
}


/// Run server with HTTP/SSE transport (for n8n and web clients)
async fn run_http_server(config: ServerConfig, port: u16) -> Result<()> {
    // Create MCP server
    let mcp_server = LoxoneMcpServer::new(config).await?;
    info!("✅ MCP server initialized successfully");

    // Authentication is always enabled for HTTP mode
    info!("🔐 Authentication ENABLED");
    info!("   Use 'loxone-mcp-auth' CLI to manage API keys");
    info!("   Or visit http://localhost:{}/admin/keys", port);

    // Create HTTP server configuration with security based on environment
    let security_config = if std::env::var("PRODUCTION").is_ok() {
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

    let http_config = HttpServerConfig {
        port,
        security_config,
        performance_config,
        #[cfg(feature = "influxdb")]
        influx_config: None,
    };

    // Create and start HTTP transport server
    let http_server = HttpTransportServer::new(mcp_server, http_config).await?;
    http_server.start().await
}

