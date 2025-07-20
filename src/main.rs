//! Loxone MCP Server - Main Entry Point using MCP Framework CLI
//!
//! This uses the MCP framework's CLI features for automatic configuration
//! and transport management.

use pulseengine_mcp_auth::AuthenticationManager;
use pulseengine_mcp_cli::config::{DefaultLoggingConfig, LogFormat, LogOutput};
use pulseengine_mcp_cli::McpConfiguration;
use pulseengine_mcp_monitoring::MetricsCollector;
use pulseengine_mcp_protocol::{Implementation, ServerCapabilities, ServerInfo};
use pulseengine_mcp_security::SecurityMiddleware;
use pulseengine_mcp_server::{middleware::MiddlewareStack, GenericServerHandler};
use pulseengine_mcp_transport::{create_transport, Transport};

use loxone_mcp_rust::{
    config::{
        credential_registry::CredentialRegistry, credentials::create_best_credential_manager,
    },
    server::framework_backend::create_loxone_backend,
    Result, ServerConfig as LoxoneServerConfig,
};

use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::info;

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

    /// Server information (auto-populated by framework)
    #[clap(skip)]
    server_info: Option<ServerInfo>,

    /// Logging configuration (managed by framework)
    #[clap(skip)]
    logging: Option<DefaultLoggingConfig>,
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

impl McpConfiguration for Config {
    fn initialize_logging(&self) -> std::result::Result<(), pulseengine_mcp_cli::CliError> {
        let log_config = self.logging.as_ref().cloned().unwrap_or_else(|| {
            DefaultLoggingConfig {
                level: if self.debug {
                    "debug".to_string()
                } else {
                    "info".to_string()
                },
                format: LogFormat::Compact, // Use compact format instead of pretty
                output: LogOutput::Stdout,
                structured: false,
            }
        });

        log_config.initialize()
    }

    fn get_server_info(&self) -> &ServerInfo {
        static SERVER_INFO: std::sync::OnceLock<ServerInfo> = std::sync::OnceLock::new();
        self.server_info
            .as_ref()
            .unwrap_or_else(|| SERVER_INFO.get_or_init(get_default_server_info))
    }

    fn get_logging_config(&self) -> &DefaultLoggingConfig {
        static LOGGING_CONFIG: std::sync::OnceLock<DefaultLoggingConfig> =
            std::sync::OnceLock::new();
        self.logging
            .as_ref()
            .unwrap_or_else(|| LOGGING_CONFIG.get_or_init(get_default_logging_config))
    }

    fn validate(&self) -> std::result::Result<(), pulseengine_mcp_cli::CliError> {
        // Check if we have credential ID or direct credentials
        let has_credential_id = self.credential_id.is_some();
        let has_direct_credentials = self.loxone_host.is_some()
            && self.loxone_user.is_some()
            && self.loxone_password.is_some();

        // Validate Loxone credentials if not in offline mode
        match &self.transport {
            TransportCommand::Stdio { offline } => {
                if !offline && !has_credential_id && !has_direct_credentials {
                    return Err(pulseengine_mcp_cli::CliError::configuration(
                        "Loxone credentials required. Use --credential-id <id>, set LOXONE_HOST/LOXONE_USER/LOXONE_PASS, or use --offline mode"
                    ));
                }
            }
            TransportCommand::Http { dev_mode, .. } => {
                if !dev_mode && !has_credential_id && !has_direct_credentials {
                    return Err(pulseengine_mcp_cli::CliError::configuration(
                        "Loxone credentials required. Use --credential-id <id>, set LOXONE_HOST/LOXONE_USER/LOXONE_PASS, or use --dev-mode"
                    ));
                }
            }
            TransportCommand::StreamableHttp { .. } => {
                if !has_credential_id && !has_direct_credentials {
                    return Err(pulseengine_mcp_cli::CliError::configuration(
                        "Loxone credentials required. Use --credential-id <id> or set LOXONE_HOST/LOXONE_USER/LOXONE_PASS"
                    ));
                }
            }
        }
        Ok(())
    }
}

// We'll use lazy_static or provide these at runtime
fn get_default_server_info() -> ServerInfo {
    ServerInfo {
        protocol_version: pulseengine_mcp_protocol::ProtocolVersion {
            major: 0,
            minor: 1,
            patch: 0,
        },
        capabilities: ServerCapabilities {
            tools: Some(pulseengine_mcp_protocol::ToolsCapability { list_changed: None }),
            resources: Some(pulseengine_mcp_protocol::ResourcesCapability {
                subscribe: Some(true),
                list_changed: None,
            }),
            prompts: Some(pulseengine_mcp_protocol::PromptsCapability { list_changed: None }),
            logging: None,
            sampling: None,
            elicitation: None,
        },
        server_info: Implementation {
            name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        instructions: None,
    }
}

// We'll provide this at runtime
fn get_default_logging_config() -> DefaultLoggingConfig {
    DefaultLoggingConfig {
        level: "info".to_string(),
        format: LogFormat::Compact, // Use compact format instead of pretty
        output: LogOutput::Stdout,
        structured: false,
    }
}

/// Load credentials from credential ID
async fn load_credentials_by_id(credential_id: &str) -> Result<(String, String, String)> {
    // Load registry
    let registry = CredentialRegistry::load()?;

    // Find credential by ID
    let stored = registry.get_credential(credential_id)
        .ok_or_else(|| loxone_mcp_rust::LoxoneError::config(
            format!("Credential ID '{credential_id}' not found. Use 'loxone-mcp-auth list' to see available credentials")
        ))?;

    // Load actual credentials from storage
    let manager = create_best_credential_manager().await?;

    // Set host for retrieval (the credential manager needs this)
    std::env::set_var("LOXONE_HOST", format!("{}:{}", stored.host, stored.port));

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
    // Create best available credential manager
    let manager = create_best_credential_manager().await?;

    // Try to get credentials
    let credentials = manager.get_credentials().await?;

    // Get host from environment variable
    let host = std::env::var("LOXONE_HOST").map_err(|_| {
        loxone_mcp_rust::LoxoneError::config("LOXONE_HOST environment variable not set".to_string())
    })?;

    Ok((host, credentials.username, credentials.password))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments using framework
    let config = Config::parse();

    // Initialize logging through framework
    config
        .initialize_logging()
        .map_err(|e| loxone_mcp_rust::LoxoneError::config(e.to_string()))?;

    // Validate configuration
    config
        .validate()
        .map_err(|e| loxone_mcp_rust::LoxoneError::config(e.to_string()))?;

    info!(
        "🚀 Starting Loxone MCP Server v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Load credentials with clear precedence order:
    // 1. Credential ID (if provided)
    // 2. Direct CLI arguments / environment variables
    // 3. Auto-detect from credential manager (fallback)
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
        // Try to auto-detect credentials from best available backend
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

    // Create Loxone configuration
    let loxone_config = match &config.transport {
        TransportCommand::Stdio { offline } => {
            if *offline {
                info!("Running in offline mode - no Loxone connection");
                LoxoneServerConfig::offline_mode()
            } else {
                {
                    let mut server_config = LoxoneServerConfig::default();
                    server_config.loxone.url =
                        format!("http://{loxone_host}").parse().map_err(|e| {
                            loxone_mcp_rust::LoxoneError::config(format!("Invalid URL: {e}"))
                        })?;
                    server_config.loxone.username = loxone_user.clone();
                    server_config.loxone.timeout = std::time::Duration::from_secs(30);
                    server_config.loxone.verify_ssl = false;
                    // Let the adaptive client factory handle auth method selection based on server capabilities
                    server_config
                }
            }
        }
        TransportCommand::Http { dev_mode, .. } => {
            if *dev_mode {
                info!("Running in development mode - minimal configuration");
                LoxoneServerConfig::dev_mode()
            } else {
                {
                    let mut server_config = LoxoneServerConfig::default();
                    server_config.loxone.url =
                        format!("http://{loxone_host}").parse().map_err(|e| {
                            loxone_mcp_rust::LoxoneError::config(format!("Invalid URL: {e}"))
                        })?;
                    server_config.loxone.username = loxone_user.clone();
                    server_config.loxone.timeout = std::time::Duration::from_secs(30);
                    server_config.loxone.verify_ssl = false;
                    // Let the adaptive client factory handle auth method selection based on server capabilities
                    server_config
                }
            }
        }
        TransportCommand::StreamableHttp { .. } => {
            let mut server_config = LoxoneServerConfig::default();
            server_config.loxone.url = format!("https://{loxone_host}")
                .parse()
                .map_err(|e| loxone_mcp_rust::LoxoneError::config(format!("Invalid URL: {e}")))?;
            server_config.loxone.username = loxone_user.clone();
            server_config.loxone.timeout = std::time::Duration::from_secs(30);
            server_config.loxone.verify_ssl = false;
            server_config
        }
    };

    // Create framework authentication manager for HTTP transport
    let framework_auth_manager = match &config.transport {
        TransportCommand::Http { dev_mode, .. } if !dev_mode => {
            // Create minimal framework auth configuration
            let auth_config = pulseengine_mcp_auth::AuthConfig {
                enabled: true,
                ..Default::default()
            };

            let auth_manager = AuthenticationManager::new(auth_config)
                .await
                .map_err(|e| loxone_mcp_rust::LoxoneError::config(e.to_string()))?;

            info!("✅ Framework authentication initialized");
            Some(Arc::new(auth_manager))
        }
        _ => {
            // Create minimal auth manager for other transports
            let auth_config = pulseengine_mcp_auth::AuthConfig {
                enabled: false,
                ..Default::default()
            };

            let auth_manager = AuthenticationManager::new(auth_config)
                .await
                .map_err(|e| loxone_mcp_rust::LoxoneError::config(e.to_string()))?;

            Some(Arc::new(auth_manager))
        }
    };

    // Create Loxone framework backend
    let backend = create_loxone_backend(loxone_config).await?;
    info!("✅ Loxone framework backend initialized");

    // Create middleware stack based on transport
    let middleware = match &config.transport {
        TransportCommand::Stdio { .. } => {
            // Minimal middleware for stdio
            MiddlewareStack::new()
        }
        TransportCommand::Http { api_key, .. } => {
            let mut stack = MiddlewareStack::new();

            // Add security middleware
            let security_config = pulseengine_mcp_security::SecurityConfig::default();
            stack = stack.with_security(SecurityMiddleware::new(security_config));

            // Add auth middleware if framework auth was created
            if let Some(ref auth_manager) = framework_auth_manager {
                // Configure API key if provided
                if let Some(key) = api_key {
                    if key.len() < 32 {
                        return Err(loxone_mcp_rust::LoxoneError::config(
                            "API key must be at least 32 characters for security",
                        ));
                    }

                    info!(
                        "API key provided for HTTP authentication (key length: {} chars)",
                        key.len()
                    );
                }

                // Add framework auth to middleware stack
                stack = stack.with_auth(auth_manager.clone());
                info!("Framework authentication middleware added");
            } else {
                info!("Development mode - authentication disabled");
            }

            // Add monitoring
            let monitoring_config = pulseengine_mcp_monitoring::MonitoringConfig::default();
            let metrics_collector = Arc::new(MetricsCollector::new(monitoring_config));
            stack = stack.with_monitoring(metrics_collector);

            stack
        }
        TransportCommand::StreamableHttp { .. } => {
            // Full middleware stack for streamable HTTP
            let mut stack = MiddlewareStack::new();

            // Security
            let security_config = pulseengine_mcp_security::SecurityConfig::default();
            stack = stack.with_security(SecurityMiddleware::new(security_config));

            // Monitoring
            let monitoring_config = pulseengine_mcp_monitoring::MonitoringConfig::default();
            let metrics_collector = Arc::new(MetricsCollector::new(monitoring_config));
            stack = stack.with_monitoring(metrics_collector);

            stack
        }
    };

    // Create generic handler with middleware
    let final_auth_manager =
        framework_auth_manager.expect("Framework auth manager should be initialized");
    let handler = GenericServerHandler::new(backend, final_auth_manager, middleware);

    // Create and configure transport based on command
    let mut transport: Box<dyn Transport> = match &config.transport {
        TransportCommand::Stdio { .. } => {
            info!("Starting stdio transport for Claude Desktop");
            create_transport(pulseengine_mcp_transport::TransportConfig::Stdio)
                .map_err(|e| loxone_mcp_rust::LoxoneError::connection(e.to_string()))?
        }
        TransportCommand::Http {
            port,
            enable_sse,
            enable_cors,
            ..
        } => {
            if *enable_sse {
                info!("Starting HTTP transport with SSE support on port {}", port);
            } else {
                info!("Starting HTTP transport on port {}", port);
            }

            // Use framework's HTTP transport which supports both SSE and streamable modes
            let http_transport = pulseengine_mcp_transport::http::HttpTransport::new(*port);

            if *enable_cors {
                // Framework's HTTP transport has built-in CORS support
                info!("CORS enabled for HTTP transport");
            }

            Box::new(http_transport)
        }
        TransportCommand::StreamableHttp { port, enable_cors } => {
            info!("Starting Streamable HTTP transport on port {}", port);

            if *enable_cors {
                info!("CORS enabled for Streamable HTTP transport");
            }

            // Use framework's create_transport function for consistency
            create_transport(pulseengine_mcp_transport::TransportConfig::StreamableHttp {
                port: *port,
                host: None,
            })
            .map_err(|e| loxone_mcp_rust::LoxoneError::connection(e.to_string()))?
        }
    };

    // Start the transport with the handler
    transport
        .start(Box::new(move |req| {
            let handler = handler.clone();
            Box::pin(async move {
                handler.handle_request(req).await.unwrap_or_else(|e| {
                    tracing::error!("Request handling error: {}", e);
                    pulseengine_mcp_protocol::Response {
                        jsonrpc: "2.0".to_string(),
                        id: serde_json::Value::Null,
                        result: None,
                        error: Some(pulseengine_mcp_protocol::Error::internal_error(
                            e.to_string(),
                        )),
                    }
                })
            })
        }))
        .await
        .map_err(|e| loxone_mcp_rust::LoxoneError::connection(e.to_string()))?;

    info!("✅ Server started successfully");

    // Handle shutdown based on transport type
    match config.transport {
        TransportCommand::Stdio { .. } => {
            // Stdio runs until input closes
            info!("Server running. Will exit when stdin closes.");
            // The transport handles the lifecycle
        }
        _ => {
            // HTTP transports need to wait for shutdown signal
            info!("Server running. Press Ctrl+C to stop.");
            tokio::signal::ctrl_c().await.map_err(|e| {
                loxone_mcp_rust::LoxoneError::connection(format!(
                    "Failed to listen for shutdown signal: {e}"
                ))
            })?;

            info!("👋 Shutdown signal received, stopping server...");
            transport
                .stop()
                .await
                .map_err(|e| loxone_mcp_rust::LoxoneError::connection(e.to_string()))?;
        }
    }

    Ok(())
}
