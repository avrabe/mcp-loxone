//! Loxone MCP Server - Main Entry Point
//!
//! This is the main binary entry point for the Loxone MCP server.
//! It supports both native and WASM32-WASIP2 compilation targets.

use loxone_mcp_rust::{
    config::credentials::{create_credentials, LoxoneCredentials},
    http_transport::{AuthConfig, HttpTransportServer},
    server::LoxoneMcpServer,
    LoxoneError, Result, ServerConfig,
};

use clap::{Parser, Subcommand};
#[cfg(feature = "keyring-storage")]
use loxone_mcp_rust::config::{credentials::CredentialManager, CredentialStore};
use std::env;
use tracing::{error, info};

/// Command line arguments
#[derive(Parser)]
#[command(name = "loxone-mcp-server")]
#[command(about = "Loxone Generation 1 MCP Server in Rust")]
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
        Commands::Http { port, api_key } => {
            info!(
                "🌐 Starting Loxone MCP Server with HTTP/SSE transport (n8n mode) on port {}",
                port
            );

            // Support environment variables as fallback
            let api_key = api_key
                .or_else(|| env::var("LOXONE_API_KEY").ok())
                .or_else(|| env::var("API_KEY").ok())
                .or_else(|| env::var("LOXONE_SSE_API_KEY").ok()); // Support legacy name

            run_http_server(config, port, api_key).await?;
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

/// Get API key from environment first, then from already loaded credentials
async fn get_api_key_from_credentials(credentials: &Option<LoxoneCredentials>) -> Option<String> {
    // Try environment variable first to avoid keychain prompts
    // Try new name first, then fall back to old name
    if let Ok(api_key) = env::var("LOXONE_API_KEY").or_else(|_| env::var("LOXONE_SSE_API_KEY")) {
        return Some(api_key);
    }

    // Use API key from already loaded credentials (if available)
    if let Some(creds) = credentials {
        return creds.api_key.clone();
    }

    None
}

/// Load credentials once and extract API key (avoids multiple keychain access)
async fn load_credentials_once() -> Option<LoxoneCredentials> {
    // Try environment variables first
    if let (Ok(user), Ok(pass)) = (env::var("LOXONE_USER"), env::var("LOXONE_PASS")) {
        let mut creds = create_credentials(user, pass);
        // Also get API key from environment if available
        creds.api_key = env::var("LOXONE_API_KEY")
            .or_else(|_| env::var("LOXONE_SSE_API_KEY"))
            .ok();
        return Some(creds);
    }

    // Only try keychain once if environment variables are not set
    #[cfg(feature = "keyring-storage")]
    {
        let credential_manager = CredentialManager::new(CredentialStore::Keyring);
        match credential_manager.get_credentials().await {
            Ok(creds) => Some(creds),
            Err(e) => {
                tracing::debug!("Failed to get credentials from keychain: {}", e);
                None
            }
        }
    }

    #[cfg(not(feature = "keyring-storage"))]
    None
}

/// Run server with HTTP/SSE transport (for n8n and web clients)
async fn run_http_server(config: ServerConfig, port: u16, api_key: Option<String>) -> Result<()> {
    // Load credentials once to avoid multiple keychain prompts
    let credentials = load_credentials_once().await;

    // Create MCP server
    let mcp_server = LoxoneMcpServer::new(config).await?;
    info!("✅ MCP server initialized successfully");

    // Get API key from loaded credentials
    let keychain_api_key = get_api_key_from_credentials(&credentials).await;

    // Configure authentication with keychain fallback
    let api_key_value = api_key
        .or_else(|| env::var("LOXONE_API_KEY").ok())
        .or_else(|| env::var("API_KEY").ok())
        .or_else(|| keychain_api_key.clone());

    let auth_config = match api_key_value {
        Some(key) => AuthConfig::with_key(key),
        None => {
            eprintln!("❌ No API key found!");
            eprintln!("   Set one of these environment variables:");
            eprintln!("   - LOXONE_API_KEY");
            eprintln!("   - API_KEY");
            eprintln!("   Or run 'loxone-mcp setup' to configure credentials");
            return Err(LoxoneError::invalid_input(
                "No API key configured for HTTP transport",
            ));
        }
    };

    // Log authentication token (masked for security)
    info!("🔐 Authentication configured:");
    info!(
        "   API key: {}***",
        &auth_config.api_key[..3.min(auth_config.api_key.len())]
    );

    // Create and start HTTP transport server
    let http_server = HttpTransportServer::new(mcp_server, auth_config, port);
    http_server.start().await
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // WASM entry point
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = run_wasm_server().await {
            web_sys::console::error_1(&format!("WASM server error: {}", e).into());
        }
    });
}

#[cfg(target_arch = "wasm32")]
async fn run_wasm_server() -> Result<()> {
    use wasm_bindgen::prelude::*;

    // Initialize console logging for WASM
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    web_sys::console::log_1(&"🚀 Starting Loxone MCP Server (WASM)".into());

    // Load configuration from browser storage or environment
    let config = ServerConfig::from_wasm_env().await?;

    // Create and run server
    let server = LoxoneMcpServer::new(config).await?;
    web_sys::console::log_1(&"✅ WASM Server initialized successfully".into());

    server.run().await?;
    Ok(())
}
