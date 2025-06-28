//! Loxone MCP Server - Main Entry Point
//!
//! This is the main binary entry point for the Loxone MCP server.
//! It supports both native and WASM32-WASIP2 compilation targets.

use loxone_mcp_rust::{LoxoneBackend, LoxoneError, Result, ServerConfig};
use mcp_server::{backend::McpBackend, GenericServerHandler};
use mcp_transport::{http::HttpTransport, stdio::StdioTransport, Transport};

use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

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

            // Load configuration (stdio mode with graceful fallback)
            let config = match ServerConfig::from_env() {
                Ok(config) => config,
                Err(e) => {
                    error!("Failed to load configuration: {}", e);
                    error!("💡 Credential issues detected, but server will continue running");
                    error!("💡 Run credential setup first or check environment variables");
                    warn!("🚀 Starting server in offline mode - MCP tools will be available but Loxone functionality limited");

                    // Create a fallback config that allows server to run
                    ServerConfig::offline_mode()
                }
            };

            run_stdio_server(config).await?;
        }
        Commands::Http {
            port,
            api_key: _,
            show_access_url,
            dev_mode,
        } => {
            info!(
                "🌐 Starting Loxone MCP Server with HTTP/SSE transport on port {}",
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
                        error!("💡 Credential issues detected, but server will continue running");
                        error!("💡 Run credential setup first or check environment variables");
                        error!("💡 Or use --dev-mode to bypass Loxone connection");
                        warn!("🚀 Starting server in offline mode - MCP tools will be available but Loxone functionality limited");

                        // Create a fallback config that allows server to run
                        ServerConfig::offline_mode()
                    }
                }
            };

            run_http_server(config, port, show_access_url, dev_mode).await?;
        }
    }

    Ok(())
}

/// Run server with stdio transport (for Claude Desktop)
async fn run_stdio_server(config: ServerConfig) -> Result<()> {
    use mcp_auth::AuthenticationManager as FrameworkAuthManager;
    use mcp_server::middleware::MiddlewareStack;
    use std::sync::Arc;

    info!("🚀 Starting Loxone MCP Server with new framework (stdio transport)");

    // Create framework backend using initialize method
    let backend = Arc::new(LoxoneBackend::initialize(config).await?);
    info!("✅ Loxone backend initialized successfully");

    // Create authentication manager (minimal for stdio)
    let auth_config = mcp_auth::AuthConfig::default();
    let auth_manager = Arc::new(
        FrameworkAuthManager::new(auth_config)
            .await
            .map_err(|e| LoxoneError::config(e.to_string()))?,
    );

    // Create middleware stack
    let middleware = MiddlewareStack::new();

    // Create generic handler
    let handler = GenericServerHandler::new(backend, auth_manager, middleware);

    // Create stdio transport
    let mut transport = StdioTransport::new();

    // Start the transport with the handler
    transport
        .start(Box::new(move |req| {
            let handler = handler.clone();
            Box::pin(async move {
                handler.handle_request(req).await.unwrap_or_else(|e| {
                    tracing::error!("Request handling error: {}", e);
                    mcp_protocol::Response {
                        jsonrpc: "2.0".to_string(),
                        id: serde_json::Value::Null,
                        result: None,
                        error: Some(mcp_protocol::Error::internal_error(e.to_string())),
                    }
                })
            })
        }))
        .await
        .map_err(|e| LoxoneError::connection(e.to_string()))?;

    Ok(())
}

/// Run server with HTTP/SSE transport (for n8n and web clients)
async fn run_http_server(
    config: ServerConfig,
    port: u16,
    _show_access_url: bool,
    _dev_mode: bool,
) -> Result<()> {
    use mcp_auth::AuthenticationManager as FrameworkAuthManager;
    use mcp_server::middleware::MiddlewareStack;
    use std::sync::Arc;

    info!("🚀 Starting Loxone MCP Server with framework-based HTTP/SSE transport");

    // Create framework backend using initialize method (same as stdio)
    let backend = Arc::new(LoxoneBackend::initialize(config).await?);
    info!("✅ Loxone backend initialized successfully");

    // Create authentication manager
    let auth_config = mcp_auth::AuthConfig::default();
    let auth_manager = Arc::new(
        FrameworkAuthManager::new(auth_config)
            .await
            .map_err(|e| LoxoneError::config(e.to_string()))?,
    );

    // Create middleware stack
    let middleware = MiddlewareStack::new();

    // Create generic handler (same as stdio)
    let handler = GenericServerHandler::new(backend, auth_manager, middleware);

    // Create HTTP transport with SSE support (compatible with MCP Inspector)
    let mut transport = HttpTransport::new(port);

    // Start the transport with the handler (same pattern as stdio)
    transport
        .start(Box::new(move |req| {
            let handler = handler.clone();
            Box::pin(async move {
                handler.handle_request(req).await.unwrap_or_else(|e| {
                    tracing::error!("Request handling error: {}", e);
                    mcp_protocol::Response {
                        jsonrpc: "2.0".to_string(),
                        id: serde_json::Value::Null,
                        result: None,
                        error: Some(mcp_protocol::Error::internal_error(e.to_string())),
                    }
                })
            })
        }))
        .await
        .map_err(|e| LoxoneError::connection(e.to_string()))?;

    // Keep the server running indefinitely
    info!("🌐 HTTP server is running. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await.map_err(|e| {
        LoxoneError::connection(format!("Failed to listen for shutdown signal: {}", e))
    })?;
    info!("👋 Shutdown signal received, stopping server...");

    // Gracefully stop the transport
    transport
        .stop()
        .await
        .map_err(|e| LoxoneError::connection(e.to_string()))?;

    Ok(())
}
