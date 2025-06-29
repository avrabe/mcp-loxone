//! Server integration utilities

use crate::{CliError, McpConfiguration};
use pulseengine_mcp_protocol::ServerInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::info;

/// Run an MCP server with the given configuration
pub async fn run_server<C>(_config: C) -> std::result::Result<(), CliError>
where
    C: McpConfiguration,
{
    // This is a placeholder implementation
    // In the full implementation, this would:
    // 1. Initialize logging
    // 2. Create server from configuration
    // 3. Set up signal handling
    // 4. Start the server
    // 5. Handle graceful shutdown

    info!("Starting MCP server...");

    // Initialize logging
    _config.initialize_logging()?;

    // Validate configuration
    _config.validate()?;

    info!("Server info: {:?}", _config.get_server_info());

    // TODO: Integrate with actual server implementation
    // let server = create_server_from_config(config).await?;
    // server.run().await?;

    Err(CliError::server_setup(
        "Server implementation not yet complete",
    ))
}

/// Create server configuration builder
pub fn server_builder() -> ServerBuilder {
    ServerBuilder::new()
}

/// Transport type for the MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportType {
    /// HTTP transport
    Http { port: u16, host: String },
    /// WebSocket transport
    WebSocket {
        port: u16,
        host: String,
        path: String,
    },
    /// Standard I/O transport
    Stdio,
}

impl Default for TransportType {
    fn default() -> Self {
        Self::Http {
            port: 8080,
            host: "localhost".to_string(),
        }
    }
}

/// CORS policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsPolicy {
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub allow_credentials: bool,
    pub max_age: Option<Duration>,
}

impl CorsPolicy {
    /// Create a permissive CORS policy (allows all origins)
    pub fn permissive() -> Self {
        Self {
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
                "OPTIONS".to_string(),
            ],
            allowed_headers: vec!["*".to_string()],
            allow_credentials: false,
            max_age: Some(Duration::from_secs(3600)),
        }
    }

    /// Create a strict CORS policy
    pub fn strict() -> Self {
        Self {
            allowed_origins: vec![],
            allowed_methods: vec!["GET".to_string(), "POST".to_string()],
            allowed_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
            allow_credentials: true,
            max_age: Some(Duration::from_secs(300)),
        }
    }

    /// Add allowed origin
    pub fn allow_origin(mut self, origin: impl Into<String>) -> Self {
        self.allowed_origins.push(origin.into());
        self
    }

    /// Add allowed method
    pub fn allow_method(mut self, method: impl Into<String>) -> Self {
        self.allowed_methods.push(method.into());
        self
    }
}

/// Custom endpoint configuration
#[derive(Debug, Clone)]
pub struct CustomEndpoint {
    pub path: String,
    pub method: String,
    pub handler_name: String,
}

impl CustomEndpoint {
    pub fn new(
        path: impl Into<String>,
        method: impl Into<String>,
        handler_name: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            method: method.into(),
            handler_name: handler_name.into(),
        }
    }
}

/// Middleware configuration
#[derive(Debug, Clone)]
pub struct MiddlewareConfig {
    pub name: String,
    pub config: HashMap<String, String>,
}

impl MiddlewareConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            config: HashMap::new(),
        }
    }

    pub fn with_config(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.insert(key.into(), value.into());
        self
    }
}

/// Builder for server configuration
pub struct ServerBuilder {
    server_info: Option<ServerInfo>,
    transport: Option<TransportType>,
    cors_policy: Option<CorsPolicy>,
    middleware: Vec<MiddlewareConfig>,
    custom_endpoints: Vec<CustomEndpoint>,
    metrics_endpoint: Option<String>,
    health_endpoint: Option<String>,
    connection_timeout: Option<Duration>,
    max_connections: Option<usize>,
    enable_compression: bool,
    enable_tls: bool,
    tls_cert_path: Option<String>,
    tls_key_path: Option<String>,
}

impl ServerBuilder {
    pub fn new() -> Self {
        Self {
            server_info: None,
            transport: None,
            cors_policy: None,
            middleware: Vec::new(),
            custom_endpoints: Vec::new(),
            metrics_endpoint: None,
            health_endpoint: None,
            connection_timeout: None,
            max_connections: None,
            enable_compression: false,
            enable_tls: false,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }

    pub fn with_server_info(mut self, info: ServerInfo) -> Self {
        self.server_info = Some(info);
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.transport = Some(TransportType::Http {
            port,
            host: "localhost".to_string(),
        });
        self
    }

    pub fn with_transport(mut self, transport: TransportType) -> Self {
        self.transport = Some(transport);
        self
    }

    pub fn with_cors_policy(mut self, cors: CorsPolicy) -> Self {
        self.cors_policy = Some(cors);
        self
    }

    pub fn with_middleware(mut self, middleware: MiddlewareConfig) -> Self {
        self.middleware.push(middleware);
        self
    }

    pub fn with_metrics_endpoint(mut self, path: impl Into<String>) -> Self {
        self.metrics_endpoint = Some(path.into());
        self
    }

    pub fn with_health_endpoint(mut self, path: impl Into<String>) -> Self {
        self.health_endpoint = Some(path.into());
        self
    }

    pub fn with_custom_endpoint(
        mut self,
        path: impl Into<String>,
        method: impl Into<String>,
        handler_name: impl Into<String>,
    ) -> Self {
        self.custom_endpoints
            .push(CustomEndpoint::new(path, method, handler_name));
        self
    }

    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = Some(timeout);
        self
    }

    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = Some(max);
        self
    }

    pub fn with_compression(mut self, enable: bool) -> Self {
        self.enable_compression = enable;
        self
    }

    pub fn with_tls(mut self, cert_path: impl Into<String>, key_path: impl Into<String>) -> Self {
        self.enable_tls = true;
        self.tls_cert_path = Some(cert_path.into());
        self.tls_key_path = Some(key_path.into());
        self
    }

    pub fn build(self) -> Result<BuiltServerConfig, CliError> {
        Ok(BuiltServerConfig {
            server_info: self
                .server_info
                .ok_or_else(|| CliError::configuration("Server info is required"))?,
            transport: self.transport.unwrap_or_default(),
            cors_policy: self.cors_policy,
            middleware: self.middleware,
            custom_endpoints: self.custom_endpoints,
            metrics_endpoint: self.metrics_endpoint,
            health_endpoint: self.health_endpoint,
            connection_timeout: self.connection_timeout.unwrap_or(Duration::from_secs(30)),
            max_connections: self.max_connections.unwrap_or(1000),
            enable_compression: self.enable_compression,
            enable_tls: self.enable_tls,
            tls_cert_path: self.tls_cert_path,
            tls_key_path: self.tls_key_path,
        })
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Built server configuration
#[derive(Debug, Clone)]
pub struct BuiltServerConfig {
    pub server_info: ServerInfo,
    pub transport: TransportType,
    pub cors_policy: Option<CorsPolicy>,
    pub middleware: Vec<MiddlewareConfig>,
    pub custom_endpoints: Vec<CustomEndpoint>,
    pub metrics_endpoint: Option<String>,
    pub health_endpoint: Option<String>,
    pub connection_timeout: Duration,
    pub max_connections: usize,
    pub enable_compression: bool,
    pub enable_tls: bool,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
}

impl BuiltServerConfig {
    /// Get the server port from transport configuration
    pub fn port(&self) -> Option<u16> {
        match &self.transport {
            TransportType::Http { port, .. } | TransportType::WebSocket { port, .. } => Some(*port),
            TransportType::Stdio => None,
        }
    }

    /// Get the server host from transport configuration
    pub fn host(&self) -> Option<&str> {
        match &self.transport {
            TransportType::Http { host, .. } | TransportType::WebSocket { host, .. } => Some(host),
            TransportType::Stdio => None,
        }
    }

    /// Check if TLS is enabled and properly configured
    pub fn is_tls_configured(&self) -> bool {
        self.enable_tls && self.tls_cert_path.is_some() && self.tls_key_path.is_some()
    }
}

/// Authentication middleware configuration
pub struct AuthMiddleware;

impl AuthMiddleware {
    pub fn bearer(api_key: impl Into<String>) -> MiddlewareConfig {
        MiddlewareConfig::new("auth")
            .with_config("api_key", api_key)
            .with_config("type", "bearer")
    }

    pub fn basic_auth(
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> MiddlewareConfig {
        MiddlewareConfig::new("auth")
            .with_config("username", username)
            .with_config("password", password)
            .with_config("type", "basic")
    }
}

/// Rate limiting middleware configuration
pub struct RateLimitMiddleware;

impl RateLimitMiddleware {
    pub fn per_second(requests_per_second: u32) -> MiddlewareConfig {
        MiddlewareConfig::new("rate_limit")
            .with_config("requests_per_second", requests_per_second.to_string())
    }

    pub fn with_burst(requests_per_second: u32, burst_size: u32) -> MiddlewareConfig {
        MiddlewareConfig::new("rate_limit")
            .with_config("requests_per_second", requests_per_second.to_string())
            .with_config("burst_size", burst_size.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::create_server_info;

    #[test]
    fn test_server_builder_basic() {
        let server_info = create_server_info(Some("test".to_string()), Some("1.0.0".to_string()));

        let config = server_builder()
            .with_server_info(server_info)
            .with_port(3000)
            .build()
            .unwrap();

        assert_eq!(config.port(), Some(3000));
        assert_eq!(config.server_info.server_info.name, "test");
    }

    #[test]
    fn test_server_builder_advanced() {
        let server_info =
            create_server_info(Some("advanced".to_string()), Some("1.0.0".to_string()));

        let config = server_builder()
            .with_server_info(server_info)
            .with_transport(TransportType::Http {
                port: 8080,
                host: "0.0.0.0".to_string(),
            })
            .with_cors_policy(CorsPolicy::permissive())
            .with_middleware(AuthMiddleware::bearer("secret-key"))
            .with_middleware(RateLimitMiddleware::per_second(100))
            .with_metrics_endpoint("/metrics")
            .with_health_endpoint("/health")
            .with_custom_endpoint("/api/v1/custom", "POST", "custom_handler")
            .with_compression(true)
            .build()
            .unwrap();

        assert_eq!(config.port(), Some(8080));
        assert_eq!(config.host(), Some("0.0.0.0"));
        assert!(config.cors_policy.is_some());
        assert_eq!(config.middleware.len(), 2);
        assert_eq!(config.custom_endpoints.len(), 1);
        assert_eq!(config.metrics_endpoint, Some("/metrics".to_string()));
        assert_eq!(config.health_endpoint, Some("/health".to_string()));
        assert!(config.enable_compression);
    }

    #[test]
    fn test_cors_policy() {
        let cors = CorsPolicy::permissive()
            .allow_origin("https://example.com")
            .allow_method("PATCH");

        assert!(cors.allowed_origins.contains(&"*".to_string()));
        assert!(cors
            .allowed_origins
            .contains(&"https://example.com".to_string()));
        assert!(cors.allowed_methods.contains(&"PATCH".to_string()));
    }

    #[test]
    fn test_transport_types() {
        let http_transport = TransportType::Http {
            port: 8080,
            host: "localhost".to_string(),
        };

        let ws_transport = TransportType::WebSocket {
            port: 8081,
            host: "localhost".to_string(),
            path: "/ws".to_string(),
        };

        let stdio_transport = TransportType::Stdio;

        assert!(matches!(http_transport, TransportType::Http { .. }));
        assert!(matches!(ws_transport, TransportType::WebSocket { .. }));
        assert!(matches!(stdio_transport, TransportType::Stdio));
    }

    #[test]
    fn test_tls_configuration() {
        let server_info =
            create_server_info(Some("tls-test".to_string()), Some("1.0.0".to_string()));

        // Test TLS configuration
        let tls_config = server_builder()
            .with_server_info(server_info.clone())
            .with_port(443)
            .with_tls("/path/to/cert.pem", "/path/to/key.pem")
            .build()
            .unwrap();

        assert!(tls_config.enable_tls);
        assert_eq!(
            tls_config.tls_cert_path,
            Some("/path/to/cert.pem".to_string())
        );
        assert_eq!(
            tls_config.tls_key_path,
            Some("/path/to/key.pem".to_string())
        );
        assert!(tls_config.is_tls_configured());

        // Test incomplete TLS configuration
        let incomplete_tls = server_builder()
            .with_server_info(server_info)
            .with_port(443)
            .build()
            .unwrap();

        assert!(!incomplete_tls.enable_tls);
        assert!(!incomplete_tls.is_tls_configured());
    }

    #[test]
    fn test_connection_limits() {
        let server_info =
            create_server_info(Some("limits-test".to_string()), Some("1.0.0".to_string()));

        // Test custom limits
        let custom_limits = server_builder()
            .with_server_info(server_info.clone())
            .with_max_connections(10000)
            .with_connection_timeout(Duration::from_secs(120))
            .build()
            .unwrap();

        assert_eq!(custom_limits.max_connections, 10000);
        assert_eq!(custom_limits.connection_timeout, Duration::from_secs(120));

        // Test defaults
        let default_limits = server_builder()
            .with_server_info(server_info)
            .build()
            .unwrap();

        assert_eq!(default_limits.max_connections, 1000);
        assert_eq!(default_limits.connection_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_custom_endpoints() {
        let server_info = create_server_info(
            Some("endpoints-test".to_string()),
            Some("1.0.0".to_string()),
        );

        let config = server_builder()
            .with_server_info(server_info)
            .with_custom_endpoint("/api/v1/users", "GET", "list_users")
            .with_custom_endpoint("/api/v1/users", "POST", "create_user")
            .with_custom_endpoint("/api/v1/users/{id}", "GET", "get_user")
            .with_custom_endpoint("/api/v1/users/{id}", "PUT", "update_user")
            .with_custom_endpoint("/api/v1/users/{id}", "DELETE", "delete_user")
            .build()
            .unwrap();

        assert_eq!(config.custom_endpoints.len(), 5);

        // Verify endpoints
        let endpoints = &config.custom_endpoints;
        assert_eq!(endpoints[0].path, "/api/v1/users");
        assert_eq!(endpoints[0].method, "GET");
        assert_eq!(endpoints[0].handler_name, "list_users");

        assert_eq!(endpoints[4].path, "/api/v1/users/{id}");
        assert_eq!(endpoints[4].method, "DELETE");
        assert_eq!(endpoints[4].handler_name, "delete_user");
    }

    #[test]
    fn test_middleware_ordering() {
        let server_info = create_server_info(
            Some("middleware-test".to_string()),
            Some("1.0.0".to_string()),
        );

        let config = server_builder()
            .with_server_info(server_info)
            .with_middleware(AuthMiddleware::bearer("key1"))
            .with_middleware(RateLimitMiddleware::per_second(50))
            .with_middleware(AuthMiddleware::basic_auth("user", "pass"))
            .with_middleware(RateLimitMiddleware::with_burst(100, 200))
            .build()
            .unwrap();

        assert_eq!(config.middleware.len(), 4);

        // Verify middleware order is preserved
        assert_eq!(config.middleware[0].name, "auth");
        assert_eq!(
            config.middleware[0].config.get("api_key"),
            Some(&"key1".to_string())
        );

        assert_eq!(config.middleware[1].name, "rate_limit");
        assert_eq!(
            config.middleware[1].config.get("requests_per_second"),
            Some(&"50".to_string())
        );

        assert_eq!(config.middleware[2].name, "auth");
        assert_eq!(
            config.middleware[2].config.get("type"),
            Some(&"basic".to_string())
        );

        assert_eq!(config.middleware[3].name, "rate_limit");
        assert_eq!(
            config.middleware[3].config.get("burst_size"),
            Some(&"200".to_string())
        );
    }
}
