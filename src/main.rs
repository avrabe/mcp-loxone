//! Loxone MCP Server - Main Entry Point
//!
//! Uses pulseengine-mcp 0.17.0 framework with macros support.
//!
//! This server supports two modes:
//! - Macro-based: Uses #[mcp_server] and #[mcp_tools] for simplified tool definitions
//! - Legacy: Uses manual McpBackend implementation for complex HTTP setups

use pulseengine_mcp_server::McpServerBuilder;

use loxone_mcp_rust::{
    Result, ServerConfig as LoxoneServerConfig,
    config::{
        credential_registry::CredentialRegistry, credentials::create_best_credential_manager,
    },
    server::macro_backend::LoxoneMcpServer,
};

use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Loxone MCP Server Configuration
#[derive(Parser, Debug)]
#[command(name = "loxone-mcp-server")]
#[command(about = "High-performance Loxone MCP Server with multi-transport support")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Config {
    /// Transport configuration
    #[command(subcommand)]
    transport: TransportCommand,

    /// Enable debug logging
    #[arg(long, global = true)]
    debug: bool,

    /// Loxone Miniserver host
    #[arg(long, global = true, env = "LOXONE_HOST")]
    loxone_host: Option<String>,

    /// Loxone username
    #[arg(long, global = true, env = "LOXONE_USER")]
    loxone_user: Option<String>,

    /// Loxone password
    #[arg(long, global = true, env = "LOXONE_PASS")]
    loxone_password: Option<String>,

    /// Credential ID (from loxone-mcp-auth)
    #[arg(long, global = true)]
    credential_id: Option<String>,

    /// Disable SSL certificate verification (not recommended for production)
    #[arg(long, global = true)]
    insecure: bool,
}

#[derive(Subcommand, Debug)]
enum TransportCommand {
    /// Run with stdio transport (Claude Desktop)
    Stdio {
        /// Enable offline mode (no Loxone connection required)
        #[arg(long)]
        offline: bool,
    },
    /// Run with HTTP transport (MCP Inspector, n8n)
    Http {
        /// Port to listen on
        #[arg(short, long, default_value = "3001")]
        port: u16,

        /// Enable SSE support for legacy clients
        #[arg(long)]
        enable_sse: bool,

        /// API key for authentication
        #[arg(long, env = "LOXONE_API_KEY")]
        api_key: Option<String>,

        /// Enable development mode (no auth)
        #[arg(long)]
        dev_mode: bool,

        /// Enable CORS (permissive mode)
        #[arg(long)]
        enable_cors: bool,
    },
    /// Run with streamable HTTP transport (new MCP Inspector)
    StreamableHttp {
        /// Port to listen on
        #[arg(short, long, default_value = "3001")]
        port: u16,

        /// Enable CORS
        #[arg(long)]
        enable_cors: bool,
    },
}

impl Config {
    /// Initialize logging based on debug flag
    fn initialize_logging(&self) {
        let filter = if self.debug {
            EnvFilter::new("debug")
        } else {
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
        };

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().compact())
            .init();
    }

    /// Validate configuration
    fn validate(&self) -> Result<()> {
        let has_credential_id = self.credential_id.is_some();
        let has_direct_credentials = self.loxone_host.is_some()
            && self.loxone_user.is_some()
            && self.loxone_password.is_some();

        match &self.transport {
            TransportCommand::Stdio { offline } => {
                if !offline && !has_credential_id && !has_direct_credentials {
                    return Err(loxone_mcp_rust::LoxoneError::config(
                        "Loxone credentials required. Use --credential-id <id>, set LOXONE_HOST/LOXONE_USER/LOXONE_PASS, or use --offline mode",
                    ));
                }
            }
            TransportCommand::Http { dev_mode, .. } => {
                if !dev_mode && !has_credential_id && !has_direct_credentials {
                    return Err(loxone_mcp_rust::LoxoneError::config(
                        "Loxone credentials required. Use --credential-id <id>, set LOXONE_HOST/LOXONE_USER/LOXONE_PASS, or use --dev-mode",
                    ));
                }
            }
            TransportCommand::StreamableHttp { .. } => {
                if !has_credential_id && !has_direct_credentials {
                    return Err(loxone_mcp_rust::LoxoneError::config(
                        "Loxone credentials required. Use --credential-id <id> or set LOXONE_HOST/LOXONE_USER/LOXONE_PASS",
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Load credentials from credential ID
async fn load_credentials_by_id(credential_id: &str) -> Result<(String, String, String)> {
    let registry = CredentialRegistry::load()?;

    let stored = registry.get_credential(credential_id).ok_or_else(|| {
        loxone_mcp_rust::LoxoneError::config(format!(
            "Credential ID '{credential_id}' not found. Use 'loxone-mcp-auth list' to see available credentials"
        ))
    })?;

    let manager = create_best_credential_manager().await?;
    // SAFETY: This is called early in main before spawning threads that read env vars
    unsafe { std::env::set_var("LOXONE_HOST", format!("{}:{}", stored.host, stored.port)) };

    let credentials = manager.get_credentials().await.map_err(|e| {
        loxone_mcp_rust::LoxoneError::config(format!(
            "Failed to load credentials for ID '{credential_id}': {e}"
        ))
    })?;

    let host = format!("{}:{}", stored.host, stored.port);
    Ok((host, credentials.username, credentials.password))
}

/// Try to auto-detect credentials from available credential managers
async fn try_auto_detect_credentials() -> Result<(String, String, String)> {
    let manager = create_best_credential_manager().await?;
    let credentials = manager.get_credentials().await?;

    let host = std::env::var("LOXONE_HOST").map_err(|_| {
        loxone_mcp_rust::LoxoneError::config("LOXONE_HOST environment variable not set".to_string())
    })?;

    Ok((host, credentials.username, credentials.password))
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse();

    // Initialize logging
    config.initialize_logging();

    // Validate configuration
    config.validate()?;

    if config.insecure {
        warn!(
            "SSL certificate verification is DISABLED (--insecure). This is not recommended for production use."
        );
    }

    // Ensure the PulseEngine master encryption key is available.
    // This prevents the "aead::Error" on restart (issue #23).
    if let Err(e) = loxone_mcp_rust::config::master_key::ensure_master_key() {
        tracing::warn!(
            "Failed to ensure master encryption key (falling back to framework default): {e}"
        );
    }

    info!(
        "🚀 Starting Loxone MCP Server v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Load credentials with precedence: credential_id > direct args > auto-detect
    let (loxone_host, loxone_user, _loxone_password) = if let Some(credential_id) =
        &config.credential_id
    {
        info!("🔑 Loading credentials from ID: {}", credential_id);
        load_credentials_by_id(credential_id).await?
    } else if config.loxone_host.is_some()
        && config.loxone_user.is_some()
        && config.loxone_password.is_some()
    {
        info!("🔑 Using direct CLI credentials");
        (
            config.loxone_host.clone().unwrap(),
            config.loxone_user.clone().unwrap(),
            config.loxone_password.clone().unwrap(),
        )
    } else {
        info!("🔍 Auto-detecting credentials from available backends...");
        match try_auto_detect_credentials().await {
            Ok((host, user, pass)) => {
                info!("✅ Auto-detected credentials from credential manager");
                (host, user, pass)
            }
            Err(e) => {
                return Err(loxone_mcp_rust::LoxoneError::config(format!(
                    "No credentials available. Please either:\n\
                         1. Use --credential-id <id> (run 'loxone-mcp-auth list' to see available IDs)\n\
                         2. Set --loxone-host, --loxone-user, --loxone-password\n\
                         3. Set environment variables LOXONE_HOST, LOXONE_USER, LOXONE_PASS\n\
                         4. Run 'loxone-mcp-setup' to configure credentials\n\
                         \n\
                         Error details: {e}"
                )));
            }
        }
    };

    // Build a LoxoneMcpServer with Loxone client for all online modes
    let build_mcp_server =
        |loxone_host: &str, loxone_user: &str, loxone_password: &str, insecure: bool| {
            let host = loxone_host.to_string();
            let user = loxone_user.to_string();
            let pass = loxone_password.to_string();
            async move {
                use loxone_mcp_rust::client::{ClientContext, LoxoneHttpClient};
                use loxone_mcp_rust::config::credentials::LoxoneCredentials;
                use loxone_mcp_rust::services::SensorTypeRegistry;

                // Resolve the Loxone host URL
                // Supports: plain IPs, explicit URLs, and dns.loxonecloud.com/<ms-id> redirect
                let loxone_url: url::Url = if host.starts_with("http://") || host.starts_with("https://") {
                    host.parse().map_err(|e| {
                        loxone_mcp_rust::LoxoneError::config(format!("Invalid URL: {e}"))
                    })?
                } else if host.starts_with("dns.loxonecloud.com") {
                    // Cloud redirect: dns.loxonecloud.com/<ms-id>
                    let redirect_url = format!("https://{host}");
                    info!("🌐 Resolving Loxone cloud redirect: {redirect_url}");
                    let client = reqwest::Client::builder()
                        .redirect(reqwest::redirect::Policy::none())
                        .build()
                        .map_err(|e| {
                            loxone_mcp_rust::LoxoneError::config(format!("Failed to create HTTP client: {e}"))
                        })?;
                    let resp = client.head(&redirect_url).send().await.map_err(|e| {
                        loxone_mcp_rust::LoxoneError::connection(format!(
                            "Failed to reach Loxone cloud redirect: {e}"
                        ))
                    })?;
                    let location = resp.headers().get(reqwest::header::LOCATION)
                        .and_then(|v| v.to_str().ok())
                        .ok_or_else(|| {
                            loxone_mcp_rust::LoxoneError::connection(
                                "No redirect location from Loxone cloud — Miniserver may be offline".to_string()
                            )
                        })?;
                    info!("✅ Resolved cloud redirect → {location}");
                    let mut resolved: url::Url = location.parse().map_err(|e| {
                        loxone_mcp_rust::LoxoneError::config(format!("Invalid redirect URL: {e}"))
                    })?;
                    // Strip the path — only keep scheme + host + port as base URL
                    resolved.set_path("");
                    resolved
                } else {
                    // Local IP or hostname
                    format!("http://{host}").parse().map_err(|e| {
                        loxone_mcp_rust::LoxoneError::config(format!("Invalid URL: {e}"))
                    })?
                };

                let loxone_cfg = loxone_mcp_rust::config::LoxoneConfig {
                    url: loxone_url,
                    timeout: std::time::Duration::from_secs(30),
                    verify_ssl: !insecure,
                    ..Default::default()
                };

                let credentials = LoxoneCredentials {
                    username: user,
                    password: pass,
                    api_key: None,
                    #[cfg(feature = "crypto-openssl")]
                    public_key: None,
                };

                let client = LoxoneHttpClient::new(loxone_cfg, credentials)
                    .await
                    .map_err(|e| {
                        loxone_mcp_rust::LoxoneError::connection(format!(
                            "Failed to create client: {e}"
                        ))
                    })?;

                let context = Arc::new(ClientContext::new());
                let client_arc: Arc<dyn loxone_mcp_rust::client::LoxoneClient> = Arc::new(client);
                let sensor_registry = Arc::new(SensorTypeRegistry::new());
                let value_resolver =
                    Arc::new(loxone_mcp_rust::services::UnifiedValueResolver::new(
                        client_arc.clone(),
                        sensor_registry,
                    ));

                info!("✅ Loxone client connected");

                Ok::<LoxoneMcpServer, loxone_mcp_rust::LoxoneError>(LoxoneMcpServer::with_context(
                    client_arc,
                    context,
                    value_resolver,
                    None,
                    LoxoneServerConfig::default(),
                ))
            }
        };

    match config.transport {
        TransportCommand::Stdio { offline } => {
            LoxoneMcpServer::configure_stdio_logging();

            let server = if offline {
                info!("🚀 Starting MCP server in offline mode (stdio)");
                LoxoneMcpServer::with_defaults()
            } else {
                info!("🚀 Starting MCP server with Loxone connection (stdio)");
                build_mcp_server(
                    &loxone_host,
                    &loxone_user,
                    &_loxone_password,
                    config.insecure,
                )
                .await?
            };

            let mut mcp_server = server.serve_stdio().await.map_err(|e| {
                loxone_mcp_rust::LoxoneError::connection(format!("Failed to start server: {e}"))
            })?;
            info!("✅ Server started (stdio)");
            mcp_server.run().await.map_err(|e| {
                loxone_mcp_rust::LoxoneError::connection(format!("Server error: {e}"))
            })?;
        }

        TransportCommand::Http { port, dev_mode, .. } => {
            let server = if dev_mode {
                warn!("Development mode enabled — no auth, localhost only");
                LoxoneMcpServer::with_defaults()
            } else {
                info!(
                    "🚀 Starting MCP server with Loxone connection (HTTP port {})",
                    port
                );
                build_mcp_server(
                    &loxone_host,
                    &loxone_user,
                    &_loxone_password,
                    config.insecure,
                )
                .await?
            };

            let serve_result: std::result::Result<
                pulseengine_mcp_server::McpServer<LoxoneMcpServer>,
                _,
            > = server.serve_http(port).await;
            let mut mcp_server = serve_result.map_err(|e| {
                loxone_mcp_rust::LoxoneError::connection(format!("Failed to start server: {e}"))
            })?;
            info!("✅ Server started (HTTP port {})", port);
            let run_result: std::result::Result<(), _> = mcp_server.run().await;
            run_result.map_err(|e| {
                loxone_mcp_rust::LoxoneError::connection(format!("Server error: {e}"))
            })?;
        }

        TransportCommand::StreamableHttp { port, .. } => {
            info!(
                "🚀 Starting MCP server with Loxone connection (Streamable HTTP port {})",
                port
            );
            let server = build_mcp_server(
                &loxone_host,
                &loxone_user,
                &_loxone_password,
                config.insecure,
            )
            .await?;

            let serve_result: std::result::Result<
                pulseengine_mcp_server::McpServer<LoxoneMcpServer>,
                _,
            > = server.serve_http(port).await;
            let mut mcp_server = serve_result.map_err(|e| {
                loxone_mcp_rust::LoxoneError::connection(format!("Failed to start server: {e}"))
            })?;
            info!("✅ Server started (Streamable HTTP port {})", port);
            let run_result: std::result::Result<(), _> = mcp_server.run().await;
            run_result.map_err(|e| {
                loxone_mcp_rust::LoxoneError::connection(format!("Server error: {e}"))
            })?;
        }
    }

    Ok(())
}
