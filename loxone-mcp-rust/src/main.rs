//! Loxone MCP Server - Main Entry Point using MCP Framework CLI
//!
//! This uses the MCP framework's CLI features for automatic configuration
//! and transport management.

use pulseengine_mcp_cli::{McpConfiguration};
use pulseengine_mcp_cli::config::{DefaultLoggingConfig, LogFormat, LogOutput};
use pulseengine_mcp_cli::server::{TransportType};
use pulseengine_mcp_protocol::{ServerInfo, ServerCapabilities, Implementation};
use pulseengine_mcp_server::{GenericServerHandler, middleware::MiddlewareStack};
use pulseengine_mcp_transport::{create_transport, Transport};
use pulseengine_mcp_auth::AuthenticationManager;
use pulseengine_mcp_security::SecurityMiddleware;
use pulseengine_mcp_monitoring::MetricsCollector;

use loxone_mcp_rust::{LoxoneBackend, Result, ServerConfig as LoxoneServerConfig};

use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::{info, warn};

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
        #[arg(long, env = "MCP_API_KEY")]
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
                level: if self.debug { "debug".to_string() } else { "info".to_string() },
                format: LogFormat::Compact, // Use compact format instead of pretty
                output: LogOutput::Stdout,
                structured: false,
            }
        });
        
        log_config.initialize()
    }
    
    fn get_server_info(&self) -> &ServerInfo {
        static SERVER_INFO: std::sync::OnceLock<ServerInfo> = std::sync::OnceLock::new();
        self.server_info.as_ref().unwrap_or_else(|| {
            SERVER_INFO.get_or_init(|| get_default_server_info())
        })
    }
    
    fn get_logging_config(&self) -> &DefaultLoggingConfig {
        static LOGGING_CONFIG: std::sync::OnceLock<DefaultLoggingConfig> = std::sync::OnceLock::new();
        self.logging.as_ref().unwrap_or_else(|| {
            LOGGING_CONFIG.get_or_init(|| get_default_logging_config())
        })
    }
    
    fn validate(&self) -> std::result::Result<(), pulseengine_mcp_cli::CliError> {
        // Validate Loxone credentials if not in offline mode
        match &self.transport {
            TransportCommand::Stdio { offline } => {
                if !offline && (self.loxone_host.is_none() || self.loxone_user.is_none() || self.loxone_password.is_none()) {
                    return Err(pulseengine_mcp_cli::CliError::configuration(
                        "Loxone credentials required. Set LOXONE_HOST, LOXONE_USER, and LOXONE_PASS or use --offline mode"
                    ));
                }
            }
            TransportCommand::Http { dev_mode, .. } => {
                if !dev_mode && (self.loxone_host.is_none() || self.loxone_user.is_none() || self.loxone_password.is_none()) {
                    return Err(pulseengine_mcp_cli::CliError::configuration(
                        "Loxone credentials required. Set LOXONE_HOST, LOXONE_USER, and LOXONE_PASS or use --dev-mode"
                    ));
                }
            }
            TransportCommand::StreamableHttp { .. } => {
                if self.loxone_host.is_none() || self.loxone_user.is_none() || self.loxone_password.is_none() {
                    return Err(pulseengine_mcp_cli::CliError::configuration(
                        "Loxone credentials required. Set LOXONE_HOST, LOXONE_USER, and LOXONE_PASS"
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

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments using framework
    let config = Config::parse();
    
    // Initialize logging through framework
    config.initialize_logging()
        .map_err(|e| loxone_mcp_rust::LoxoneError::config(e.to_string()))?;
    
    // Validate configuration
    config.validate()
        .map_err(|e| loxone_mcp_rust::LoxoneError::config(e.to_string()))?;
    
    info!("🚀 Starting Loxone MCP Server v{}", env!("CARGO_PKG_VERSION"));
    
    // Create Loxone configuration
    let loxone_config = match &config.transport {
        TransportCommand::Stdio { offline } => {
            if *offline {
                info!("Running in offline mode - no Loxone connection");
                LoxoneServerConfig::offline_mode()
            } else {
                {
                    let mut server_config = LoxoneServerConfig::default();
                    server_config.loxone.url = format!("http://{}", config.loxone_host.clone().unwrap())
                        .parse()
                        .map_err(|e| loxone_mcp_rust::LoxoneError::config(format!("Invalid URL: {e}")))?;
                    server_config.loxone.username = config.loxone_user.clone().unwrap();
                    server_config.loxone.timeout = std::time::Duration::from_secs(30);
                    server_config.loxone.verify_ssl = false;
                    // Force basic auth for classic Gen1 Miniservers to avoid account lockout
                    server_config.loxone.auth_method = loxone_mcp_rust::config::AuthMethod::Basic;
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
                    server_config.loxone.url = format!("http://{}", config.loxone_host.clone().unwrap())
                        .parse()
                        .map_err(|e| loxone_mcp_rust::LoxoneError::config(format!("Invalid URL: {e}")))?;
                    server_config.loxone.username = config.loxone_user.clone().unwrap();
                    server_config.loxone.timeout = std::time::Duration::from_secs(30);
                    server_config.loxone.verify_ssl = false;
                    // Force basic auth for classic Gen1 Miniservers to avoid account lockout
                    server_config.loxone.auth_method = loxone_mcp_rust::config::AuthMethod::Basic;
                    server_config
                }
            }
        }
        TransportCommand::StreamableHttp { .. } => {
            let mut server_config = LoxoneServerConfig::default();
            server_config.loxone.url = format!("https://{}", config.loxone_host.clone().unwrap())
                .parse()
                .map_err(|e| loxone_mcp_rust::LoxoneError::config(format!("Invalid URL: {e}")))?;
            server_config.loxone.username = config.loxone_user.clone().unwrap();
            server_config.loxone.timeout = std::time::Duration::from_secs(30);
            server_config.loxone.verify_ssl = false;
            server_config
        }
    };
    
    // Initialize Loxone backend
    let backend = Arc::new(LoxoneBackend::initialize(loxone_config).await?);
    info!("✅ Loxone backend initialized");
    
    // Create middleware stack based on transport
    let middleware = match &config.transport {
        TransportCommand::Stdio { .. } => {
            // Minimal middleware for stdio
            MiddlewareStack::new()
        }
        TransportCommand::Http { api_key, dev_mode, .. } => {
            let mut stack = MiddlewareStack::new();
            
            // Add security middleware
            let security_config = pulseengine_mcp_security::SecurityConfig::default();
            stack = stack.with_security(SecurityMiddleware::new(security_config));
            
            // Add auth middleware if not in dev mode
            if !dev_mode {
                let auth_config = pulseengine_mcp_auth::AuthConfig::default();
                let auth_manager = Arc::new(
                    AuthenticationManager::new(auth_config).await
                        .map_err(|e| loxone_mcp_rust::LoxoneError::config(e.to_string()))?
                );
                
                // Create API key if provided
                // TODO: Update for new framework API
                if let Some(_key) = api_key {
                    warn!("API key creation not yet implemented in framework migration");
                    // auth_manager.create_api_key(
                    //     "cli-key".to_string(),
                    //     pulseengine_mcp_auth::Role::Admin,
                    //     None,
                    //     None,
                    // ).await
                    //     .map_err(|e| loxone_mcp_rust::LoxoneError::config(e.to_string()))?;
                    // info!("Created API key for authentication");
                }
                
                stack = stack.with_auth(auth_manager);
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
    let auth_manager = Arc::new(
        AuthenticationManager::new(pulseengine_mcp_auth::AuthConfig::default()).await
            .map_err(|e| loxone_mcp_rust::LoxoneError::config(e.to_string()))?
    );
    let handler = GenericServerHandler::new(backend, auth_manager, middleware);
    
    // Create and configure transport based on command
    let mut transport: Box<dyn Transport> = match &config.transport {
        TransportCommand::Stdio { .. } => {
            info!("Starting stdio transport for Claude Desktop");
            create_transport(pulseengine_mcp_transport::TransportConfig::Stdio)
                .map_err(|e| loxone_mcp_rust::LoxoneError::connection(e.to_string()))?
        }
        TransportCommand::Http { port, enable_sse, enable_cors, .. } => {
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
            
            // Use framework's streamable HTTP transport
            let streamable = pulseengine_mcp_transport::streamable_http::StreamableHttpTransport::new(*port);
            
            if *enable_cors {
                info!("CORS enabled for Streamable HTTP transport");
            }
            
            Box::new(streamable)
        }
    };
    
    // Start the transport with the handler
    transport.start(Box::new(move |req| {
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
            tokio::signal::ctrl_c().await
                .map_err(|e| loxone_mcp_rust::LoxoneError::connection(
                    format!("Failed to listen for shutdown signal: {e}")
                ))?;
            
            info!("👋 Shutdown signal received, stopping server...");
            transport.stop().await
                .map_err(|e| loxone_mcp_rust::LoxoneError::connection(e.to_string()))?;
        }
    }
    
    Ok(())
}