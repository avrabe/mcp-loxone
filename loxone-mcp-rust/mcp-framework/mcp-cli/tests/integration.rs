//! Integration tests for the MCP CLI framework

use clap::Parser;
use pulseengine_mcp_cli::{
    server_builder, AuthMiddleware, CorsPolicy, DefaultLoggingConfig, LogFormat, LogOutput,
    McpConfig, McpConfiguration, RateLimitMiddleware, TransportType,
};
use pulseengine_mcp_protocol::ServerInfo;
use std::time::Duration;

/// Test configuration for integration tests
#[derive(Debug, Clone, Parser, McpConfig)]
#[command(name = "test-server", about = "Test MCP server")]
struct TestServerConfig {
    /// Server port
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// Enable debug mode
    #[arg(short, long)]
    debug: bool,

    /// API key for authentication
    #[arg(long)]
    api_key: Option<String>,

    /// Server information
    #[mcp(auto_populate)]
    #[clap(skip)]
    server_info: Option<ServerInfo>,

    /// Logging configuration
    #[mcp(logging)]
    #[clap(skip)]
    logging: Option<DefaultLoggingConfig>,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            debug: false,
            api_key: None,
            server_info: Some(pulseengine_mcp_cli::config::create_server_info(
                Some("test-server".to_string()),
                Some("1.0.0".to_string()),
            )),
            logging: Some(DefaultLoggingConfig::default()),
        }
    }
}

#[test]
fn test_cli_parsing_integration() {
    // Test parsing with various arguments
    let config = TestServerConfig::try_parse_from([
        "test",
        "--port",
        "3000",
        "--debug",
        "--api-key",
        "secret",
    ])
    .expect("Failed to parse arguments");

    assert_eq!(config.port, 3000);
    assert!(config.debug);
    assert_eq!(config.api_key, Some("secret".to_string()));
}

#[test]
fn test_mcp_configuration_trait() {
    let config = TestServerConfig::default();

    // Test trait methods
    let server_info = config.get_server_info();
    assert_eq!(server_info.server_info.name, "test-server");
    assert_eq!(server_info.server_info.version, "1.0.0");

    let logging_config = config.get_logging_config();
    assert_eq!(logging_config.level, "info");
    assert!(matches!(logging_config.format, LogFormat::Pretty));
    assert!(matches!(logging_config.output, LogOutput::Stdout));

    // Test validation
    assert!(config.validate().is_ok());
}

#[test]
fn test_auto_populate_integration() {
    let mut config = TestServerConfig {
        port: 9000,
        debug: true,
        api_key: Some("test-key".to_string()),
        server_info: None,
        logging: None,
    };

    // Auto-populate should fill in missing fields
    config.auto_populate();

    assert!(config.server_info.is_some());
    let server_info = config.server_info.as_ref().unwrap();
    assert_eq!(server_info.server_info.name, env!("CARGO_PKG_NAME"));
    assert_eq!(server_info.server_info.version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn test_server_builder_integration() {
    let config = TestServerConfig::default();

    let server_config = server_builder()
        .with_server_info(config.get_server_info().clone())
        .with_transport(TransportType::Http {
            port: config.port,
            host: "localhost".to_string(),
        })
        .with_cors_policy(CorsPolicy::permissive())
        .with_middleware(AuthMiddleware::bearer("test-key"))
        .with_middleware(RateLimitMiddleware::per_second(100))
        .with_metrics_endpoint("/metrics")
        .with_health_endpoint("/health")
        .with_compression(true)
        .with_connection_timeout(Duration::from_secs(30))
        .with_max_connections(1000)
        .build()
        .expect("Failed to build server config");

    // Verify configuration
    assert_eq!(server_config.port(), Some(8080));
    assert_eq!(server_config.host(), Some("localhost"));
    assert!(server_config.cors_policy.is_some());
    assert_eq!(server_config.middleware.len(), 2);
    assert_eq!(server_config.metrics_endpoint, Some("/metrics".to_string()));
    assert_eq!(server_config.health_endpoint, Some("/health".to_string()));
    assert!(server_config.enable_compression);
    assert_eq!(server_config.connection_timeout, Duration::from_secs(30));
    assert_eq!(server_config.max_connections, 1000);
}

#[test]
fn test_transport_types_integration() {
    // Test HTTP transport
    let http_config = server_builder()
        .with_server_info(pulseengine_mcp_cli::config::create_server_info(None, None))
        .with_transport(TransportType::Http {
            port: 8080,
            host: "0.0.0.0".to_string(),
        })
        .build()
        .unwrap();

    assert_eq!(http_config.port(), Some(8080));
    assert_eq!(http_config.host(), Some("0.0.0.0"));

    // Test WebSocket transport
    let ws_config = server_builder()
        .with_server_info(pulseengine_mcp_cli::config::create_server_info(None, None))
        .with_transport(TransportType::WebSocket {
            port: 8081,
            host: "localhost".to_string(),
            path: "/ws".to_string(),
        })
        .build()
        .unwrap();

    assert_eq!(ws_config.port(), Some(8081));
    assert_eq!(ws_config.host(), Some("localhost"));

    // Test stdio transport
    let stdio_config = server_builder()
        .with_server_info(pulseengine_mcp_cli::config::create_server_info(None, None))
        .with_transport(TransportType::Stdio)
        .build()
        .unwrap();

    assert_eq!(stdio_config.port(), None);
    assert_eq!(stdio_config.host(), None);
}

#[test]
fn test_cors_configuration() {
    // Test permissive CORS
    let permissive = CorsPolicy::permissive();
    assert!(permissive.allowed_origins.contains(&"*".to_string()));
    assert!(permissive.allowed_methods.contains(&"GET".to_string()));
    assert!(permissive.allowed_methods.contains(&"POST".to_string()));
    assert!(!permissive.allow_credentials);

    // Test strict CORS
    let strict = CorsPolicy::strict();
    assert!(strict.allowed_origins.is_empty());
    assert_eq!(strict.allowed_methods.len(), 2);
    assert!(strict.allow_credentials);

    // Test custom CORS
    let custom = CorsPolicy::permissive()
        .allow_origin("https://example.com")
        .allow_origin("https://app.example.com")
        .allow_method("PATCH");

    assert_eq!(custom.allowed_origins.len(), 3); // *, example.com, app.example.com
    assert!(custom.allowed_methods.contains(&"PATCH".to_string()));
}

#[test]
fn test_middleware_configuration() {
    // Test auth middleware
    let auth = AuthMiddleware::bearer("secret-key");
    assert_eq!(auth.name, "auth");
    assert_eq!(auth.config.get("api_key"), Some(&"secret-key".to_string()));
    assert_eq!(auth.config.get("type"), Some(&"bearer".to_string()));

    let basic_auth = AuthMiddleware::basic_auth("user", "pass");
    assert_eq!(basic_auth.name, "auth");
    assert_eq!(basic_auth.config.get("username"), Some(&"user".to_string()));
    assert_eq!(basic_auth.config.get("password"), Some(&"pass".to_string()));
    assert_eq!(basic_auth.config.get("type"), Some(&"basic".to_string()));

    // Test rate limit middleware
    let rate_limit = RateLimitMiddleware::per_second(100);
    assert_eq!(rate_limit.name, "rate_limit");
    assert_eq!(
        rate_limit.config.get("requests_per_second"),
        Some(&"100".to_string())
    );

    let rate_limit_burst = RateLimitMiddleware::with_burst(100, 200);
    assert_eq!(
        rate_limit_burst.config.get("burst_size"),
        Some(&"200".to_string())
    );
}

#[test]
fn test_advanced_server_configuration() {
    let config = server_builder()
        .with_server_info(pulseengine_mcp_cli::config::create_server_info(
            Some("advanced-test".to_string()),
            Some("2.0.0".to_string()),
        ))
        .with_port(9000)
        .with_cors_policy(CorsPolicy::strict().allow_origin("https://trusted.com"))
        .with_middleware(AuthMiddleware::bearer("api-key-123"))
        .with_middleware(RateLimitMiddleware::with_burst(50, 100))
        .with_metrics_endpoint("/api/metrics")
        .with_health_endpoint("/api/health")
        .with_custom_endpoint("/api/v1/status", "GET", "status_handler")
        .with_custom_endpoint("/api/v1/admin", "POST", "admin_handler")
        .with_connection_timeout(Duration::from_secs(60))
        .with_max_connections(5000)
        .with_compression(true)
        .with_tls("/path/to/cert.pem", "/path/to/key.pem")
        .build()
        .expect("Failed to build advanced config");

    // Verify all settings
    assert_eq!(config.port(), Some(9000));
    assert!(config.cors_policy.is_some());

    let cors = config.cors_policy.as_ref().unwrap();
    assert!(cors
        .allowed_origins
        .contains(&"https://trusted.com".to_string()));

    assert_eq!(config.middleware.len(), 2);
    assert_eq!(config.custom_endpoints.len(), 2);
    assert_eq!(config.metrics_endpoint, Some("/api/metrics".to_string()));
    assert_eq!(config.health_endpoint, Some("/api/health".to_string()));
    assert_eq!(config.connection_timeout, Duration::from_secs(60));
    assert_eq!(config.max_connections, 5000);
    assert!(config.enable_compression);
    assert!(config.enable_tls);
    assert_eq!(config.tls_cert_path, Some("/path/to/cert.pem".to_string()));
    assert_eq!(config.tls_key_path, Some("/path/to/key.pem".to_string()));
    assert!(config.is_tls_configured());
}

#[test]
fn test_logging_configuration() {
    use pulseengine_mcp_cli::{LogFormat, LogOutput};

    // Test default logging
    let default_log = DefaultLoggingConfig::default();
    assert_eq!(default_log.level, "info");
    assert!(matches!(default_log.format, LogFormat::Pretty));
    assert!(matches!(default_log.output, LogOutput::Stdout));
    assert!(default_log.structured);

    // Test custom logging
    let custom_log = DefaultLoggingConfig {
        level: "debug".to_string(),
        format: LogFormat::Json,
        output: LogOutput::Stderr,
        structured: false,
    };

    assert_eq!(custom_log.level, "debug");
    assert!(matches!(custom_log.format, LogFormat::Json));
    assert!(matches!(custom_log.output, LogOutput::Stderr));
    assert!(!custom_log.structured);
}

#[test]
fn test_complete_flow_integration() {
    // Simulate complete flow from CLI parsing to server configuration

    // 1. Parse CLI arguments
    let cli_config = TestServerConfig::try_parse_from([
        "test",
        "--port",
        "4000",
        "--debug",
        "--api-key",
        "production-key",
    ])
    .expect("Failed to parse CLI");

    // 2. Validate configuration
    cli_config
        .validate()
        .expect("Configuration validation failed");

    // 3. Build server configuration
    let server_config = server_builder()
        .with_server_info(cli_config.get_server_info().clone())
        .with_port(cli_config.port)
        .with_cors_policy(if cli_config.debug {
            CorsPolicy::permissive()
        } else {
            CorsPolicy::strict()
        })
        .with_middleware(
            cli_config
                .api_key
                .as_ref()
                .map(AuthMiddleware::bearer)
                .unwrap(),
        )
        .with_metrics_endpoint("/metrics")
        .with_health_endpoint("/health")
        .build()
        .expect("Failed to build server");

    // 4. Verify final configuration
    assert_eq!(server_config.port(), Some(4000));
    assert!(server_config.cors_policy.is_some());
    assert_eq!(server_config.middleware.len(), 1);
    assert_eq!(server_config.middleware[0].name, "auth");

    // The server is now ready to be started with this configuration
}
