//! Advanced MCP server example demonstrating the complete ServerConfig API
//!
//! This example shows how to use all the advanced features of the ServerConfig builder
//! including transport configuration, CORS policies, middleware, custom endpoints,
//! and advanced server options.

use clap::Parser;
use pulseengine_mcp_cli::{
    server_builder, AuthMiddleware, CorsPolicy, DefaultLoggingConfig, McpConfig, 
    RateLimitMiddleware, TransportType
};
use pulseengine_mcp_protocol::ServerInfo;
use std::time::Duration;

/// Advanced server configuration demonstrating all available options
#[derive(Debug, Clone, Parser, McpConfig)]
#[command(name = "advanced-server")]
#[command(about = "An advanced MCP server demonstrating the complete framework API")]
struct AdvancedServerConfig {
    /// Server port
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// Server host
    #[arg(long, default_value = "localhost")]
    host: String,

    /// Transport type (http, ws, stdio)
    #[arg(short, long, default_value = "http")]
    transport: String,

    /// WebSocket path (only for WebSocket transport)
    #[arg(long, default_value = "/ws")]
    ws_path: String,

    /// API key for authentication
    #[arg(long)]
    api_key: Option<String>,

    /// Enable rate limiting
    #[arg(long)]
    enable_rate_limiting: bool,

    /// Requests per second for rate limiting
    #[arg(long, default_value = "100")]
    rate_limit_rps: u32,

    /// Enable CORS
    #[arg(long)]
    enable_cors: bool,

    /// Allow credentials in CORS
    #[arg(long)]
    cors_allow_credentials: bool,

    /// Enable compression
    #[arg(long)]
    enable_compression: bool,

    /// Enable TLS
    #[arg(long)]
    enable_tls: bool,

    /// TLS certificate path
    #[arg(long)]
    tls_cert: Option<String>,

    /// TLS private key path
    #[arg(long)]
    tls_key: Option<String>,

    /// Maximum connections
    #[arg(long, default_value = "1000")]
    max_connections: usize,

    /// Connection timeout in seconds
    #[arg(long, default_value = "30")]
    connection_timeout: u64,

    /// Enable metrics endpoint
    #[arg(long)]
    enable_metrics: bool,

    /// Metrics endpoint path
    #[arg(long, default_value = "/metrics")]
    metrics_path: String,

    /// Enable health endpoint
    #[arg(long)]
    enable_health: bool,

    /// Health endpoint path
    #[arg(long, default_value = "/health")]
    health_path: String,

    /// Server information (auto-populated from Cargo.toml)
    #[mcp(auto_populate)]
    #[clap(skip)]
    server_info: Option<ServerInfo>,

    /// Logging configuration
    #[mcp(logging)]
    #[clap(skip)]
    logging: Option<DefaultLoggingConfig>,
}

impl Default for AdvancedServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            host: "localhost".to_string(),
            transport: "http".to_string(),
            ws_path: "/ws".to_string(),
            api_key: None,
            enable_rate_limiting: false,
            rate_limit_rps: 100,
            enable_cors: false,
            cors_allow_credentials: false,
            enable_compression: false,
            enable_tls: false,
            tls_cert: None,
            tls_key: None,
            max_connections: 1000,
            connection_timeout: 30,
            enable_metrics: false,
            metrics_path: "/metrics".to_string(),
            enable_health: false,
            health_path: "/health".to_string(),
            server_info: Some(pulseengine_mcp_cli::config::create_server_info(None, None)),
            logging: Some(DefaultLoggingConfig::default()),
        }
    }
}

fn create_transport_from_config(config: &AdvancedServerConfig) -> TransportType {
    match config.transport.as_str() {
        "http" => TransportType::Http {
            port: config.port,
            host: config.host.clone(),
        },
        "ws" | "websocket" => TransportType::WebSocket {
            port: config.port,
            host: config.host.clone(),
            path: config.ws_path.clone(),
        },
        "stdio" => TransportType::Stdio,
        _ => {
            tracing::warn!("Unknown transport type '{}', defaulting to HTTP", config.transport);
            TransportType::Http {
                port: config.port,
                host: config.host.clone(),
            }
        }
    }
}

fn create_cors_policy(config: &AdvancedServerConfig) -> Option<CorsPolicy> {
    if config.enable_cors {
        let mut cors = CorsPolicy::permissive();
        if config.cors_allow_credentials {
            cors.allow_credentials = true;
            // When allowing credentials, we can't use wildcard origins
            cors.allowed_origins = vec!["http://localhost:3000".to_string()];
        }
        Some(cors)
    } else {
        None
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let config = AdvancedServerConfig::parse();

    // Initialize logging
    config.initialize_logging()?;

    // Validate configuration
    config.validate()?;

    tracing::info!("Starting advanced MCP server with full configuration");
    tracing::info!("Config: {:#?}", config);

    // Create transport based on configuration
    let transport = create_transport_from_config(&config);
    tracing::info!("Transport: {:?}", transport);

    // Create CORS policy if enabled
    let cors_policy = create_cors_policy(&config);
    if let Some(ref cors) = cors_policy {
        tracing::info!("CORS enabled: {:?}", cors);
    }

    // Start building the server configuration
    let mut server_config_builder = server_builder()
        .with_server_info(config.get_server_info().clone())
        .with_transport(transport)
        .with_max_connections(config.max_connections)
        .with_connection_timeout(Duration::from_secs(config.connection_timeout))
        .with_compression(config.enable_compression);

    // Add CORS if enabled
    if let Some(cors) = cors_policy {
        server_config_builder = server_config_builder.with_cors_policy(cors);
    }

    // Add authentication middleware if API key is provided
    if let Some(api_key) = &config.api_key {
        tracing::info!("Adding authentication middleware");
        server_config_builder = server_config_builder
            .with_middleware(AuthMiddleware::new(api_key));
    }

    // Add rate limiting middleware if enabled
    if config.enable_rate_limiting {
        tracing::info!("Adding rate limiting middleware: {} requests/sec", config.rate_limit_rps);
        server_config_builder = server_config_builder
            .with_middleware(RateLimitMiddleware::new(config.rate_limit_rps));
    }

    // Add metrics endpoint if enabled
    if config.enable_metrics {
        tracing::info!("Adding metrics endpoint: {}", config.metrics_path);
        server_config_builder = server_config_builder
            .with_metrics_endpoint(&config.metrics_path);
    }

    // Add health endpoint if enabled
    if config.enable_health {
        tracing::info!("Adding health endpoint: {}", config.health_path);
        server_config_builder = server_config_builder
            .with_health_endpoint(&config.health_path);
    }

    // Add custom endpoints for demonstration
    server_config_builder = server_config_builder
        .with_custom_endpoint("/api/v1/status", "GET", "status_handler")
        .with_custom_endpoint("/api/v1/info", "GET", "info_handler")
        .with_custom_endpoint("/api/v1/config", "POST", "config_handler");

    // Add TLS if enabled and properly configured
    if config.enable_tls {
        if let (Some(cert_path), Some(key_path)) = (&config.tls_cert, &config.tls_key) {
            tracing::info!("Enabling TLS with cert: {}, key: {}", cert_path, key_path);
            server_config_builder = server_config_builder
                .with_tls(cert_path, key_path);
        } else {
            tracing::warn!("TLS enabled but certificate or key path not provided");
        }
    }

    // Build the server configuration
    let server_config = server_config_builder.build()?;

    // Display final configuration
    tracing::info!("Server configuration built successfully:");
    tracing::info!("  Transport: {:?}", server_config.transport);
    tracing::info!("  Port: {:?}", server_config.port());
    tracing::info!("  Host: {:?}", server_config.host());
    tracing::info!("  CORS enabled: {}", server_config.cors_policy.is_some());
    tracing::info!("  Middleware count: {}", server_config.middleware.len());
    tracing::info!("  Custom endpoints: {}", server_config.custom_endpoints.len());
    tracing::info!("  Metrics endpoint: {:?}", server_config.metrics_endpoint);
    tracing::info!("  Health endpoint: {:?}", server_config.health_endpoint);
    tracing::info!("  Max connections: {}", server_config.max_connections);
    tracing::info!("  Connection timeout: {:?}", server_config.connection_timeout);
    tracing::info!("  Compression enabled: {}", server_config.enable_compression);
    tracing::info!("  TLS configured: {}", server_config.is_tls_configured());

    // Demonstrate middleware configuration
    for (i, middleware) in server_config.middleware.iter().enumerate() {
        tracing::info!("  Middleware {}: {} ({:?})", i + 1, middleware.name, middleware.config);
    }

    // Demonstrate custom endpoints
    for (i, endpoint) in server_config.custom_endpoints.iter().enumerate() {
        tracing::info!("  Endpoint {}: {} {} -> {}", 
            i + 1, endpoint.method, endpoint.path, endpoint.handler_name);
    }

    tracing::info!("This example demonstrates the complete ServerConfig API");
    tracing::info!("In a real implementation, you would now:");
    tracing::info!("  1. Create the actual MCP server with this configuration");
    tracing::info!("  2. Set up all the middleware and endpoints");
    tracing::info!("  3. Start the server and handle incoming requests");
    tracing::info!("  4. Implement graceful shutdown");

    // For this example, we'll just wait for Ctrl+C
    tracing::info!("Server configuration complete. Press Ctrl+C to exit.");
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down gracefully");

    Ok(())
}