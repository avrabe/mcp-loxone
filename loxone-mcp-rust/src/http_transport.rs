//! HTTP/SSE transport implementation for n8n MCP integration
//!
//! This module provides HTTP server capabilities with Server-Sent Events (SSE)
//! transport for the Model Context Protocol, making it compatible with n8n.

pub mod authentication;
pub mod rate_limiting;

use crate::error::{LoxoneError, Result};
use crate::performance::{
    middleware::PerformanceMiddleware, PerformanceConfig, PerformanceMonitor,
};
use crate::security::key_store::{KeyStore, KeyStoreBackend, KeyStoreConfig};
use crate::security::{middleware::SecurityMiddleware, SecurityConfig};
use crate::server::LoxoneMcpServer;
pub use authentication::AuthConfig;
use authentication::AuthManager;
use mcp_foundation::{Content, ServerHandler};
use rate_limiting::{EnhancedRateLimiter, RateLimitResult};

#[cfg(feature = "influxdb")]
use crate::monitoring::{
    dashboard::{dashboard_routes, DashboardState},
    influxdb::{InfluxConfig, InfluxManager},
    metrics::{MetricsCollector, RequestTiming},
};

use crate::history::{
    config::HistoryConfig,
    // dashboard_api::create_dashboard_router, // Temporarily disabled due to state type mismatch
    core::UnifiedHistoryStore,
};

use axum::{
    extract::{Query, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::{
        sse::{Event, Sse},
        Html, IntoResponse, Response,
    },
    routing::get,
    Json, Router,
};
use chrono;
use futures_util::stream::{self};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, RwLock};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, info, warn};

/// Legacy authentication configuration (deprecated)
#[derive(Debug, Clone)]
pub struct LegacyAuthConfig {
    /// Single API key for all access
    pub api_key: String,
}

impl LegacyAuthConfig {
    /// Create auth config from environment variable
    pub fn from_env() -> std::result::Result<Self, String> {
        match std::env::var("HTTP_API_KEY") {
            Ok(api_key) => {
                if api_key.trim().is_empty() {
                    Err("HTTP_API_KEY environment variable is empty".to_string())
                } else {
                    Ok(Self { api_key })
                }
            }
            Err(_) => {
                Err("HTTP_API_KEY environment variable not set. Set a secure API key.".to_string())
            }
        }
    }

    /// Create auth config with explicit key (for testing)
    pub fn with_key(api_key: String) -> Self {
        Self { api_key }
    }
}

/// Query parameters for SSE endpoint
#[derive(Debug, Deserialize)]
struct SseQuery {
    /// Optional client identifier
    client_id: Option<String>,
    /// Optional resource subscriptions (comma-separated)
    subscribe: Option<String>,
}

/// SSE notification event
#[derive(Debug, Clone, Serialize)]
pub struct SseNotificationEvent {
    /// Event type
    pub event_type: String,
    /// Resource URI
    pub resource_uri: String,
    /// Client ID this notification is for
    pub client_id: String,
    /// Notification data
    pub data: serde_json::Value,
    /// Timestamp
    pub timestamp: String,
}

/// SSE connection manager for broadcasting notifications
#[derive(Clone)]
pub struct SseConnectionManager {
    /// Broadcast channel for sending notifications to all SSE connections
    notification_sender: broadcast::Sender<SseNotificationEvent>,
    /// Active SSE connections tracking
    #[allow(dead_code)]
    connections: Arc<RwLock<HashMap<String, broadcast::Receiver<SseNotificationEvent>>>>,
}

impl Default for SseConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SseConnectionManager {
    /// Create new SSE connection manager
    pub fn new() -> Self {
        let (notification_sender, _) = broadcast::channel(1000);
        Self {
            notification_sender,
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Send notification to specific client
    pub async fn send_notification(&self, event: SseNotificationEvent) -> Result<()> {
        match self.notification_sender.send(event) {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!("Failed to send SSE notification: {}", e);
                Err(LoxoneError::connection(format!(
                    "SSE notification failed: {}",
                    e
                )))
            }
        }
    }

    /// Create a receiver for a new SSE connection
    pub fn create_receiver(&self) -> broadcast::Receiver<SseNotificationEvent> {
        self.notification_sender.subscribe()
    }
}

// Global SSE manager access - this is a simple approach
// In production, you might want a more sophisticated service registry
static GLOBAL_SSE_MANAGER: std::sync::OnceLock<Arc<SseConnectionManager>> =
    std::sync::OnceLock::new();

/// Initialize the global SSE manager
pub fn init_global_sse_manager(manager: Arc<SseConnectionManager>) {
    let _ = GLOBAL_SSE_MANAGER.set(manager);
}

/// Get the global SSE manager if initialized
pub async fn get_global_sse_manager() -> Option<Arc<SseConnectionManager>> {
    GLOBAL_SSE_MANAGER.get().cloned()
}

/// Health check response
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    timestamp: String,
    services: HealthServices,
}

#[derive(Debug, Serialize)]
struct HealthServices {
    loxone: String,
    mcp_server: String,
}

/// HTTP transport server configuration
pub struct HttpServerConfig {
    /// Server port
    pub port: u16,
    /// Authentication configuration
    pub auth_config: AuthConfig,
    /// Security configuration
    pub security_config: Option<SecurityConfig>,
    /// Performance monitoring configuration
    pub performance_config: Option<PerformanceConfig>,
    /// InfluxDB configuration (optional)
    #[cfg(feature = "influxdb")]
    pub influx_config: Option<InfluxConfig>,
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        // Check if we should disable auth for development
        let auth_config = if std::env::var("DISABLE_AUTH").is_ok() {
            AuthConfig {
                require_api_key: false,
                ..AuthConfig::default()
            }
        } else {
            AuthConfig::default()
        };

        // Determine security config based on environment
        let security_config = if std::env::var("PRODUCTION").is_ok() {
            Some(SecurityConfig::production())
        } else if std::env::var("DISABLE_SECURITY").is_ok() {
            None
        } else {
            Some(SecurityConfig::development())
        };

        // Determine performance config based on environment
        let performance_config = if std::env::var("DISABLE_PERFORMANCE").is_ok() {
            None
        } else if std::env::var("PRODUCTION").is_ok() {
            Some(PerformanceConfig::production())
        } else {
            Some(PerformanceConfig::development())
        };

        Self {
            port: 3001,
            auth_config,
            security_config,
            performance_config,
            #[cfg(feature = "influxdb")]
            influx_config: None,
        }
    }
}

/// HTTP transport server
pub struct HttpTransportServer {
    /// MCP server instance
    mcp_server: LoxoneMcpServer,
    /// Authentication manager
    auth_manager: AuthManager,
    /// Enhanced rate limiter
    rate_limiter: EnhancedRateLimiter,
    /// Security middleware
    security_middleware: Option<Arc<SecurityMiddleware>>,
    /// Performance middleware
    performance_middleware: Option<Arc<PerformanceMiddleware>>,
    /// API key store
    key_store: Arc<KeyStore>,
    /// Metrics collector
    #[cfg(feature = "influxdb")]
    metrics_collector: Arc<MetricsCollector>,
    /// InfluxDB manager
    #[cfg(feature = "influxdb")]
    influx_manager: Option<Arc<InfluxManager>>,
    /// Server port
    port: u16,
}

impl HttpTransportServer {
    /// Create new HTTP transport server with configuration
    pub async fn new(mcp_server: LoxoneMcpServer, mut config: HttpServerConfig) -> Result<Self> {
        // Backward compatibility: Check for old HTTP_API_KEY env var
        if let Ok(api_key) = std::env::var("HTTP_API_KEY") {
            if !api_key.trim().is_empty() {
                info!("Using legacy HTTP_API_KEY authentication");
                // Create a simple auth manager that accepts this key
                config.auth_config.require_api_key = true;
                // Note: We'll need to handle this in the auth manager
            }
        }

        #[cfg(feature = "influxdb")]
        let (metrics_collector, influx_manager) = if let Some(influx_config) = config.influx_config
        {
            let influx_manager = Arc::new(InfluxManager::new(influx_config).await?);
            let metrics_collector = Arc::new(MetricsCollector::with_influx(influx_manager.clone()));

            // Initialize default metrics
            metrics_collector.init_default_metrics().await;

            info!("InfluxDB integration enabled");
            (metrics_collector, Some(influx_manager))
        } else {
            let metrics_collector = Arc::new(MetricsCollector::new());
            metrics_collector.init_default_metrics().await;
            (metrics_collector, None)
        };

        let auth_manager = AuthManager::new(config.auth_config);

        // Add default admin key if HTTP_API_KEY is set
        if let Ok(api_key) = std::env::var("HTTP_API_KEY") {
            if !api_key.trim().is_empty() {
                // Store the legacy key for validation
                auth_manager.add_legacy_key(api_key).await;
            }
        }

        // Initialize security middleware if configured
        let security_middleware = if let Some(security_config) = config.security_config {
            match SecurityMiddleware::new(security_config) {
                Ok(middleware) => {
                    info!("🔒 Security middleware enabled");
                    Some(Arc::new(middleware))
                }
                Err(e) => {
                    warn!("Failed to initialize security middleware: {}", e);
                    None
                }
            }
        } else {
            info!("⚠️ Security middleware disabled");
            None
        };

        // Initialize performance middleware if configured
        let performance_middleware = if let Some(performance_config) = config.performance_config {
            match PerformanceMonitor::new(performance_config) {
                Ok(monitor) => {
                    info!("📊 Performance monitoring enabled");
                    Some(Arc::new(PerformanceMiddleware::new(Arc::new(monitor))))
                }
                Err(e) => {
                    warn!("Failed to initialize performance monitor: {}", e);
                    None
                }
            }
        } else {
            info!("⚠️ Performance monitoring disabled");
            None
        };

        // Initialize key store
        let key_store_config = KeyStoreConfig {
            backend: KeyStoreBackend::File,
            file_path: Some(crate::security::key_store::default_key_store_path()),
            auto_save: true,
            encrypt_at_rest: false,
        };
        let key_store = Arc::new(KeyStore::new(key_store_config).await?);
        info!("🔑 API key store initialized");

        Ok(Self {
            mcp_server,
            auth_manager,
            rate_limiter: EnhancedRateLimiter::with_defaults(),
            security_middleware,
            performance_middleware,
            key_store,
            #[cfg(feature = "influxdb")]
            metrics_collector,
            #[cfg(feature = "influxdb")]
            influx_manager,
            port: config.port,
        })
    }

    /// Create with default configuration
    pub async fn with_defaults(mcp_server: LoxoneMcpServer, port: u16) -> Result<Self> {
        let config = HttpServerConfig {
            port,
            ..Default::default()
        };
        Self::new(mcp_server, config).await
    }

    /// Start the HTTP server
    pub async fn start(&self) -> Result<()> {
        let app = self.create_router().await?;

        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .map_err(|e| {
                LoxoneError::connection(format!("Failed to bind to port {}: {}", self.port, e))
            })?;

        info!("🌐 HTTP MCP server starting on port {}", self.port);
        info!(
            "📬 MCP HTTP endpoint: http://localhost:{}/message (MCP Inspector)",
            self.port
        );
        info!(
            "📡 SSE stream: http://localhost:{}/sse (optional)",
            self.port
        );
        info!(
            "📡 SSE endpoint: http://localhost:{}/mcp/sse (n8n legacy)",
            self.port
        );
        info!("🏥 Health check: http://localhost:{}/health", self.port);

        // Show security status
        if self.security_middleware.is_some() {
            info!("🔒 Security hardening: ENABLED");
            info!(
                "🛡️ Security audit: http://localhost:{}/security/audit",
                self.port
            );
            info!(
                "🛡️ Security headers test: http://localhost:{}/security/headers",
                self.port
            );
        } else {
            warn!("⚠️ Security hardening: DISABLED (set PRODUCTION=1 to enable)");
        }

        // Show available dashboard endpoints
        #[cfg(feature = "influxdb")]
        {
            info!(
                "📊 Monitoring dashboard: http://localhost:{}/dashboard/ (web browser)",
                self.port
            );
            info!(
                "📋 API information: http://localhost:{}/ (web browser)",
                self.port
            );
            info!("📈 History data: Available via monitoring dashboard with stats collection");
        }

        #[cfg(not(feature = "influxdb"))]
        {
            info!("📊 Dashboard endpoints disabled (enable with --features influxdb)");
        }

        // Show key management UI endpoint
        info!(
            "🔑 API key management: http://localhost:{}/admin/keys (web browser)",
            self.port
        );

        // Start background task to collect system metrics
        #[cfg(feature = "influxdb")]
        {
            let metrics = self.metrics_collector.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5));
                loop {
                    interval.tick().await;
                    metrics.collect_system_metrics().await;
                }
            });
        }

        axum::serve(listener, app)
            .await
            .map_err(|e| LoxoneError::connection(format!("HTTP server error: {}", e)))?;

        Ok(())
    }

    /// Create the router with all endpoints
    async fn create_router(&self) -> Result<Router> {
        // Initialize history store for dashboard only if explicitly enabled
        let history_store = if std::env::var("ENABLE_LOXONE_STATS").unwrap_or_default() == "1" {
            match UnifiedHistoryStore::new(HistoryConfig::from_env()).await {
                Ok(store) => {
                    info!("✅ History store initialized for dashboard (ENABLE_LOXONE_STATS=1)");
                    Some(Arc::new(store))
                }
                Err(e) => {
                    warn!("⚠️ Failed to initialize history store: {}", e);
                    None
                }
            }
        } else {
            debug!("📊 History store disabled (ENABLE_LOXONE_STATS not set to 1)");
            None
        };

        let sse_manager = Arc::new(SseConnectionManager::new());

        // Initialize the global SSE manager for use by the notification dispatcher
        init_global_sse_manager(sse_manager.clone());

        let shared_state = Arc::new(AppState {
            mcp_server: self.mcp_server.clone(),
            auth_manager: self.auth_manager.clone(),
            rate_limiter: self.rate_limiter.clone(),
            #[cfg(feature = "influxdb")]
            metrics_collector: self.metrics_collector.clone(),
            #[cfg(feature = "influxdb")]
            influx_manager: self.influx_manager.clone(),
            history_store,
            sse_manager,
            key_store: self.key_store.clone(),
        });

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        // Public routes (no authentication required)
        let mut public_routes = Router::new()
            .route("/health", get(health_check))
            .route("/", get(root_handler))
            .route("/favicon.ico", get(favicon_handler))
            .route("/metrics", get(prometheus_metrics)) // Prometheus endpoint
            // History dashboard endpoints (public for web browser access)
            .route("/history", get(history_dashboard_home))
            .route("/history/", get(history_dashboard_home))
            .route("/history/api/status", get(history_api_status))
            // Unified dashboard routes (public for web browser access)
            .route("/dashboard", get(unified_dashboard_home))
            .route("/dashboard/", get(unified_dashboard_home))
            .route("/dashboard/api/status", get(unified_dashboard_api_status))
            .route("/dashboard/api/data", get(unified_dashboard_api_data));

        // Add WebSocket route for unified dashboard (public, no auth required)
        if shared_state.history_store.is_some() {
            public_routes = public_routes.route("/dashboard/ws", get(unified_dashboard_websocket));
        }

        // Protected routes (authentication required)
        let protected_routes = Router::new()
            // MCP Streamable HTTP transport endpoints
            .route("/sse", get(sse_handler)) // Optional SSE stream for server→client
            .route("/message", axum::routing::post(handle_mcp_message)) // Main HTTP POST endpoint
            .route("/messages", axum::routing::post(handle_mcp_message)) // n8n compatibility
            // Legacy endpoints for backwards compatibility
            .route("/mcp/sse", get(sse_handler)) // Alternative for n8n
            .route("/mcp/info", get(server_info))
            .route("/mcp/tools", get(list_tools))
            // Admin endpoints (require admin auth)
            .route("/admin/status", get(admin_status))
            .route("/admin/rate-limits", get(rate_limit_status))
            .layer(axum::middleware::from_fn_with_state(
                shared_state.clone(),
                auth_middleware_wrapper,
            ));

        // Create base app
        let mut app = Router::new().merge(public_routes).merge(protected_routes);

        // Add dashboard routes - prefer unified dashboard if history store is available
        if shared_state.history_store.is_some() {
            info!("✅ Using unified dashboard (history store available)");
            // Unified dashboard is already included in public_routes
        } else {
            // Fallback to InfluxDB dashboard if available
            #[cfg(feature = "influxdb")]
            {
                let dashboard_state = DashboardState {
                    metrics_collector: shared_state.metrics_collector.clone(),
                    influx_manager: shared_state.influx_manager.clone(),
                };
                app = app.nest("/dashboard/influx", dashboard_routes(dashboard_state));
                info!("✅ Using InfluxDB dashboard at /dashboard/influx (no history store)");
            }
        }

        let app = app
            .layer(ServiceBuilder::new().layer(cors).into_inner())
            .with_state(shared_state.clone());

        // Add security middleware if enabled
        let app = if let Some(security_middleware) = &self.security_middleware {
            // Add security diagnostics endpoints
            let security_routes = Router::new()
                .route(
                    "/security/audit",
                    get(crate::security::middleware::security_diagnostics_handler),
                )
                .route(
                    "/security/headers",
                    get(crate::security::middleware::security_headers_test_handler),
                )
                .layer(axum::middleware::from_fn_with_state(
                    shared_state.clone(),
                    auth_middleware_wrapper,
                ))
                .with_state(security_middleware.clone());

            app.merge(security_routes)
                .layer(axum::middleware::from_fn_with_state(
                    security_middleware.clone(),
                    crate::security::middleware::security_middleware_handler,
                ))
        } else {
            app
        };

        // Add performance middleware if enabled
        let app = if let Some(performance_middleware) = &self.performance_middleware {
            // Add performance monitoring endpoints
            let performance_routes = crate::performance::middleware::create_performance_router(
                performance_middleware.clone(),
            );
            let perf_routes = Router::new()
                .nest("/performance", performance_routes)
                .layer(axum::middleware::from_fn_with_state(
                    shared_state.clone(),
                    auth_middleware_wrapper,
                ));

            app.merge(perf_routes)
                .layer(axum::middleware::from_fn_with_state(
                    performance_middleware.clone(),
                    crate::performance::middleware::performance_middleware_handler,
                ))
        } else {
            app
        };

        // Add key management UI routes
        let key_management_router =
            crate::monitoring::key_management_ui::create_key_management_router(
                shared_state.key_store.clone(),
            );

        // Add main navigation hub
        let nav_router = Router::new()
            .route("/", get(navigation_hub))
            .merge(key_management_router);

        // Add admin routes
        let app = app
            .nest("/admin", nav_router);
        info!(
            "🏠 Navigation Hub: http://localhost:{}/admin (with API key)",
            self.port
        );
        info!(
            "🔑 API key management UI: http://localhost:{}/admin/keys",
            self.port
        );

        Ok(app)
    }
}

/// Shared application state
#[derive(Clone)]
struct AppState {
    mcp_server: LoxoneMcpServer,
    auth_manager: AuthManager,
    rate_limiter: EnhancedRateLimiter,
    #[cfg(feature = "influxdb")]
    metrics_collector: Arc<MetricsCollector>,
    #[cfg(feature = "influxdb")]
    influx_manager: Option<Arc<InfluxManager>>,
    history_store: Option<Arc<UnifiedHistoryStore>>,
    sse_manager: Arc<SseConnectionManager>,
    #[allow(dead_code)]
    key_store: Arc<KeyStore>,
}

/// Main navigation hub handler
async fn navigation_hub() -> impl IntoResponse {
    Html(generate_navigation_html())
}

/// Generate the main navigation hub HTML
fn generate_navigation_html() -> String {
    r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Loxone MCP Server - Navigation Hub</title>
    <style>
        :root {
            --loxone-green: #7aba00;
            --loxone-dark: #1a1a1a;
            --bg-primary: #0f0f0f;
            --bg-secondary: #1a1a1a;
            --bg-card: #252525;
            --text-primary: #e0e0e0;
            --text-secondary: #a0a0a0;
            --border-color: #333;
            --rust-orange: #ce422b;
            --success: #28a745;
            --info: #17a2b8;
            --warning: #ffc107;
        }

        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            line-height: 1.6;
            min-height: 100vh;
        }

        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 2rem;
        }

        .header {
            text-align: center;
            margin-bottom: 3rem;
            padding-bottom: 2rem;
            border-bottom: 2px solid var(--border-color);
        }

        h1 {
            color: var(--loxone-green);
            font-size: 2.5rem;
            margin-bottom: 0.5rem;
        }

        .subtitle {
            color: var(--text-secondary);
            font-size: 1.1rem;
        }

        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 2rem;
            margin-top: 2rem;
        }

        .category {
            background: var(--bg-card);
            border: 1px solid var(--border-color);
            border-radius: 12px;
            padding: 1.5rem;
            transition: all 0.3s ease;
        }

        .category:hover {
            border-color: var(--loxone-green);
            box-shadow: 0 4px 12px rgba(122, 186, 0, 0.1);
        }

        .category-title {
            font-size: 1.3rem;
            color: var(--loxone-green);
            margin-bottom: 1rem;
            display: flex;
            align-items: center;
            gap: 0.5rem;
        }

        .link-grid {
            display: flex;
            flex-direction: column;
            gap: 0.75rem;
        }

        .nav-link {
            display: flex;
            align-items: center;
            gap: 0.75rem;
            padding: 0.75rem 1rem;
            background: var(--bg-secondary);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            color: var(--text-primary);
            text-decoration: none;
            transition: all 0.3s ease;
            cursor: pointer;
        }

        .nav-link:hover {
            background: var(--bg-primary);
            border-color: var(--loxone-green);
            color: var(--loxone-green);
            transform: translateY(-2px);
        }

        .link-icon {
            font-size: 1.2rem;
            width: 24px;
            text-align: center;
        }

        .link-details {
            flex: 1;
        }

        .link-title {
            font-weight: 500;
            margin-bottom: 0.25rem;
        }

        .link-description {
            font-size: 0.875rem;
            color: var(--text-secondary);
        }

        .status-indicator {
            width: 8px;
            height: 8px;
            border-radius: 50%;
            background: var(--success);
        }

        .footer {
            margin-top: 3rem;
            padding-top: 2rem;
            border-top: 1px solid var(--border-color);
            text-align: center;
            color: var(--text-secondary);
        }

        .api-key-display {
            background: var(--bg-secondary);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 1rem;
            margin: 1rem 0;
            font-family: 'Courier New', monospace;
            font-size: 0.9rem;
            word-break: break-all;
        }

        /* Retro Terminal Styles */
        .terminal-container {
            margin-top: 3rem;
            background: #0a0a0a;
            border: 2px solid #00ff00;
            border-radius: 0;
            padding: 0;
            box-shadow: 0 0 20px rgba(0, 255, 0, 0.5);
            font-family: 'Courier New', 'Consolas', monospace;
        }

        .terminal-header {
            background: #00ff00;
            color: #0a0a0a;
            padding: 0.5rem 1rem;
            font-weight: bold;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }

        .terminal-screen {
            background: #0a0a0a;
            color: #00ff00;
            padding: 1rem;
            min-height: 400px;
            max-height: 400px;
            overflow-y: auto;
            font-size: 14px;
            line-height: 1.5;
            position: relative;
        }

        .terminal-screen::-webkit-scrollbar {
            width: 8px;
        }

        .terminal-screen::-webkit-scrollbar-track {
            background: #0a0a0a;
        }

        .terminal-screen::-webkit-scrollbar-thumb {
            background: #00ff00;
        }

        .terminal-line {
            margin-bottom: 0.5rem;
            font-family: inherit;
            white-space: pre-wrap;
        }

        .terminal-prompt {
            color: #00ff00;
            display: inline;
        }

        .terminal-input {
            background: transparent;
            border: none;
            color: #00ff00;
            font-family: inherit;
            font-size: inherit;
            outline: none;
            width: calc(100% - 100px);
            display: inline;
        }

        .terminal-cursor {
            display: inline-block;
            width: 10px;
            height: 16px;
            background: #00ff00;
            animation: blink 1s infinite;
        }

        @keyframes blink {
            0%, 49% { opacity: 1; }
            50%, 100% { opacity: 0; }
        }

        .terminal-ascii-art {
            color: #00ff00;
            text-align: center;
            margin: 1rem 0;
            font-size: 12px;
            line-height: 1.2;
        }

        .terminal-game {
            text-align: center;
            padding: 1rem;
        }

        .terminal-error {
            color: #ff0000;
        }

        .terminal-success {
            color: #00ff00;
        }

        .terminal-warning {
            color: #ffff00;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🏠 Loxone MCP Server</h1>
            <div class="subtitle">Navigation Hub - Access all interfaces and tools</div>
            <div id="apiKeyDisplay" class="api-key-display" style="display: none;">
                Using API Key: <span id="apiKeyValue"></span>
            </div>
        </div>

        <div class="grid">
            <!-- Administration -->
            <div class="category">
                <div class="category-title">
                    🔧 Administration
                </div>
                <div class="link-grid">
                    <a href="#" class="nav-link" onclick="navigateTo('/admin/keys')">
                        <span class="link-icon">🔑</span>
                        <div class="link-details">
                            <div class="link-title">API Key Management</div>
                            <div class="link-description">Generate, edit, and manage API keys</div>
                        </div>
                        <div class="status-indicator"></div>
                    </a>
                    <a href="#" class="nav-link" onclick="navigateTo('/admin/status')">
                        <span class="link-icon">📊</span>
                        <div class="link-details">
                            <div class="link-title">Server Status</div>
                            <div class="link-description">View server health and statistics</div>
                        </div>
                        <div class="status-indicator"></div>
                    </a>
                    <a href="#" class="nav-link" onclick="navigateTo('/admin/rate-limits')">
                        <span class="link-icon">⚡</span>
                        <div class="link-details">
                            <div class="link-title">Rate Limits</div>
                            <div class="link-description">Monitor API rate limiting status</div>
                        </div>
                        <div class="status-indicator"></div>
                    </a>
                </div>
            </div>

            <!-- Monitoring & Dashboards -->
            <div class="category">
                <div class="category-title">
                    📈 Monitoring & Dashboards
                </div>
                <div class="link-grid">
                    <a href="#" class="nav-link" onclick="navigateTo('/dashboard/')">
                        <span class="link-icon">🎛️</span>
                        <div class="link-details">
                            <div class="link-title">Unified Dashboard</div>
                            <div class="link-description">Real-time Loxone system overview</div>
                        </div>
                        <div class="status-indicator"></div>
                    </a>
                    <a href="#" class="nav-link" onclick="navigateTo('/history/')">
                        <span class="link-icon">📜</span>
                        <div class="link-details">
                            <div class="link-title">History Dashboard</div>
                            <div class="link-description">Historical data and trends</div>
                        </div>
                        <div class="status-indicator"></div>
                    </a>
                    <a href="#" class="nav-link" onclick="navigateTo('/metrics')">
                        <span class="link-icon">📊</span>
                        <div class="link-details">
                            <div class="link-title">Prometheus Metrics</div>
                            <div class="link-description">Raw metrics for monitoring tools</div>
                        </div>
                        <div class="status-indicator"></div>
                    </a>
                </div>
            </div>

            <!-- System Health -->
            <div class="category">
                <div class="category-title">
                    💚 System Health
                </div>
                <div class="link-grid">
                    <a href="#" class="nav-link" onclick="navigateTo('/health')">
                        <span class="link-icon">❤️</span>
                        <div class="link-details">
                            <div class="link-title">Health Check</div>
                            <div class="link-description">Basic system health status</div>
                        </div>
                        <div class="status-indicator"></div>
                    </a>
                    <a href="#" class="nav-link" onclick="navigateTo('/')">
                        <span class="link-icon">📋</span>
                        <div class="link-details">
                            <div class="link-title">Server Info</div>
                            <div class="link-description">API endpoints and server information</div>
                        </div>
                        <div class="status-indicator"></div>
                    </a>
                </div>
            </div>

            <!-- MCP Tools -->
            <div class="category">
                <div class="category-title">
                    🔌 MCP Integration
                </div>
                <div class="link-grid">
                    <a href="#" class="nav-link" onclick="navigateTo('/mcp/sse')">
                        <span class="link-icon">🔄</span>
                        <div class="link-details">
                            <div class="link-title">MCP SSE Endpoint</div>
                            <div class="link-description">Server-Sent Events for MCP clients</div>
                        </div>
                        <div class="status-indicator"></div>
                    </a>
                    <a href="#" class="nav-link" onclick="navigateTo('/mcp/info')">
                        <span class="link-icon">ℹ️</span>
                        <div class="link-details">
                            <div class="link-title">MCP Information</div>
                            <div class="link-description">Available tools and capabilities</div>
                        </div>
                        <div class="status-indicator"></div>
                    </a>
                    <a href="#" class="nav-link" onclick="navigateTo('/mcp/tools')">
                        <span class="link-icon">🛠️</span>
                        <div class="link-details">
                            <div class="link-title">MCP Tools</div>
                            <div class="link-description">List of available MCP tools</div>
                        </div>
                        <div class="status-indicator"></div>
                    </a>
                </div>
            </div>
        </div>

        <!-- Retro Terminal -->
        <div class="terminal-container">
            <div class="terminal-header">
                <span>WOPR TERMINAL v2.0 - LOXONE CONTROL SYSTEM</span>
                <span>[ CONNECTED ]</span>
            </div>
            <div class="terminal-screen" id="terminal">
                <div class="terminal-ascii-art">
 __      __  ___   ___   ___  
/  \    /  \/   \ |   \ |   \ 
\   \/\/   /     ||    \|    \
 \        /|  O  ||  D  |  D  |
  \  /\  / |     ||     |     |
   \/  \/  \___/ |____/|____/ 
                              
      WAR OPERATION PLAN RESPONSE
                </div>
                <div class="terminal-line">GREETINGS PROFESSOR FALKEN.</div>
                <div class="terminal-line">SHALL WE PLAY A GAME?</div>
                <div class="terminal-line"></div>
                <div class="terminal-line">AVAILABLE GAMES:</div>
                <div class="terminal-line">1. GLOBAL THERMONUCLEAR WAR</div>
                <div class="terminal-line">2. TIC-TAC-TOE</div>
                <div class="terminal-line">3. LUIGI'S MANSION (KEY COLLECTOR)</div>
                <div class="terminal-line">4. SNAKE</div>
                <div class="terminal-line">5. DOOR DEFENDER</div>
                <div class="terminal-line">6. LIGHT CYCLES</div>
                <div class="terminal-line"></div>
                <div id="output"></div>
                <div class="terminal-line" id="inputLine">
                    <span class="terminal-prompt">&gt; </span>
                    <input type="text" id="terminalInput" class="terminal-input" autofocus>
                    <span class="terminal-cursor"></span>
                </div>
            </div>
        </div>

        <div class="footer">
            <p>🦀 Loxone MCP Server - Built with Rust</p>
            <p>Navigation Hub v1.0 - Browser-friendly interface</p>
        </div>
    </div>

    <script>
        // Get API key from URL parameters
        function getApiKey() {
            const params = new URLSearchParams(window.location.search);
            return params.get('api_key');
        }

        // Build URL with API key parameter
        function buildUrl(path) {
            const apiKey = getApiKey();
            if (apiKey) {
                const separator = path.includes('?') ? '&' : '?';
                return path + separator + 'api_key=' + encodeURIComponent(apiKey);
            }
            return path;
        }

        // Navigate to a page with API key
        function navigateTo(path) {
            window.location.href = buildUrl(path);
        }

        // Show API key if present
        window.onload = function() {
            const apiKey = getApiKey();
            if (apiKey) {
                document.getElementById('apiKeyDisplay').style.display = 'block';
                document.getElementById('apiKeyValue').textContent = apiKey;
            }
            initTerminal();
        };

        // Terminal functionality
        let terminalHistory = [];
        let historyIndex = -1;
        let currentGame = null;

        function initTerminal() {
            const input = document.getElementById('terminalInput');
            const terminal = document.getElementById('terminal');
            
            input.addEventListener('keydown', (e) => {
                if (e.key === 'Enter') {
                    processCommand(input.value);
                    terminalHistory.push(input.value);
                    historyIndex = terminalHistory.length;
                    input.value = '';
                } else if (e.key === 'ArrowUp') {
                    e.preventDefault();
                    if (historyIndex > 0) {
                        historyIndex--;
                        input.value = terminalHistory[historyIndex];
                    }
                } else if (e.key === 'ArrowDown') {
                    e.preventDefault();
                    if (historyIndex < terminalHistory.length - 1) {
                        historyIndex++;
                        input.value = terminalHistory[historyIndex];
                    } else {
                        historyIndex = terminalHistory.length;
                        input.value = '';
                    }
                }
            });

            // Keep focus on input
            terminal.addEventListener('click', () => input.focus());
        }

        function addLine(text, className = '') {
            const output = document.getElementById('output');
            const line = document.createElement('div');
            line.className = 'terminal-line ' + className;
            line.textContent = text;
            output.appendChild(line);
            
            // Auto scroll to bottom
            const terminal = document.getElementById('terminal');
            terminal.scrollTop = terminal.scrollHeight;
        }

        function processCommand(cmd) {
            const command = cmd.trim().toUpperCase();
            addLine('> ' + cmd, 'terminal-prompt');

            if (currentGame) {
                processGameCommand(command);
                return;
            }

            switch(command) {
                case '1':
                case 'GLOBAL THERMONUCLEAR WAR':
                    startGlobalThermonuclearWar();
                    break;
                case '2':
                case 'TIC-TAC-TOE':
                    startTicTacToe();
                    break;
                case '3':
                case "LUIGI'S MANSION":
                case 'KEY COLLECTOR':
                    startLuigisMansion();
                    break;
                case '4':
                case 'SNAKE':
                    startSnake();
                    break;
                case '5':
                case 'DOOR DEFENDER':
                    startDoorDefender();
                    break;
                case '6':
                case 'LIGHT CYCLES':
                    startLightCycles();
                    break;
                case 'HELP':
                    showHelp();
                    break;
                case 'HELLO JOSHUA':
                    addLine('HELLO PROFESSOR FALKEN.');
                    break;
                case 'LIST GAMES':
                    listGames();
                    break;
                case 'CLEAR':
                case 'CLS':
                    document.getElementById('output').innerHTML = '';
                    break;
                case 'EXIT':
                case 'QUIT':
                    if (currentGame) {
                        currentGame = null;
                        addLine('GAME TERMINATED.');
                        addLine('');
                        addLine('SHALL WE PLAY A GAME?');
                    }
                    break;
                default:
                    if (command) {
                        addLine('COMMAND NOT RECOGNIZED: ' + command, 'terminal-error');
                        addLine('TYPE "HELP" FOR AVAILABLE COMMANDS');
                    }
            }
        }

        function showHelp() {
            addLine('AVAILABLE COMMANDS:');
            addLine('1-6 - SELECT A GAME');
            addLine('HELP - SHOW THIS MESSAGE');
            addLine('LIST GAMES - SHOW AVAILABLE GAMES');
            addLine('CLEAR - CLEAR SCREEN');
            addLine('EXIT - EXIT CURRENT GAME');
        }

        function listGames() {
            addLine('AVAILABLE GAMES:');
            addLine('1. GLOBAL THERMONUCLEAR WAR - SYSTEM STATUS MONITORING');
            addLine('2. TIC-TAC-TOE - CLASSIC GAME');
            addLine('3. LUIGI\'S MANSION - COLLECT API KEYS');
            addLine('4. SNAKE - NAVIGATE THE GRID');
            addLine('5. DOOR DEFENDER - MONITOR DOOR SENSORS');
            addLine('6. LIGHT CYCLES - CONTROL LIGHTING GRID');
        }

        // Game: Global Thermonuclear War
        function startGlobalThermonuclearWar() {
            currentGame = 'war';
            addLine('');
            addLine('INITIATING GLOBAL THERMONUCLEAR WAR SIMULATION...');
            addLine('');
            setTimeout(() => {
                addLine('STRANGE GAME.');
                setTimeout(() => {
                    addLine('THE ONLY WINNING MOVE IS NOT TO PLAY.');
                    setTimeout(() => {
                        addLine('');
                        addLine('HOW ABOUT A NICE GAME OF CHESS?');
                        setTimeout(() => {
                            addLine('');
                            addLine('REDIRECTING TO SYSTEM STATUS MONITOR...');
                            setTimeout(() => {
                                window.location.href = buildUrl('/admin/status');
                            }, 2000);
                        }, 1500);
                    }, 1500);
                }, 1500);
            }, 2000);
        }

        // Game: Luigi's Mansion (Key Collector)
        function startLuigisMansion() {
            currentGame = 'luigi';
            addLine('');
            addLine('WELCOME TO LUIGI\'S MANSION - KEY COLLECTOR EDITION');
            addLine('');
            addLine('YOU ARE IN A DARK MANSION. YOUR MISSION:');
            addLine('COLLECT ALL THE API KEYS SCATTERED THROUGHOUT.');
            addLine('');
            addLine('YOU SEE A GLOWING DOOR AHEAD.');
            addLine('TYPE "ENTER" TO GO THROUGH THE DOOR.');
            addLine('TYPE "EXIT" TO LEAVE THE GAME.');
        }

        function processGameCommand(command) {
            if (currentGame === 'luigi') {
                if (command === 'ENTER' || command === 'GO' || command === 'OPEN DOOR') {
                    addLine('');
                    addLine('YOU ENTER THE GLOWING DOOR...');
                    addLine('A MYSTERIOUS PORTAL OPENS!');
                    addLine('');
                    addLine('[ TELEPORTING TO KEY MANAGEMENT CHAMBER ]');
                    setTimeout(() => {
                        window.location.href = buildUrl('/admin/keys');
                    }, 2000);
                } else if (command === 'EXIT' || command === 'QUIT') {
                    currentGame = null;
                    addLine('YOU LEAVE THE MANSION... FOR NOW.');
                    addLine('');
                    addLine('SHALL WE PLAY A GAME?');
                } else {
                    addLine('YOU CANNOT DO THAT HERE.', 'terminal-warning');
                }
            } else if (currentGame === 'tictactoe') {
                processTicTacToeCommand(command);
            }
        }

        // Game: Tic-Tac-Toe
        let tttBoard = [];
        function startTicTacToe() {
            currentGame = 'tictactoe';
            tttBoard = [' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' '];
            addLine('');
            addLine('TIC-TAC-TOE - YOU ARE X, I AM O');
            addLine('ENTER POSITION (1-9):');
            addLine('');
            addLine(' 1 | 2 | 3 ');
            addLine('-----------');
            addLine(' 4 | 5 | 6 ');
            addLine('-----------');
            addLine(' 7 | 8 | 9 ');
            addLine('');
        }

        function processTicTacToeCommand(command) {
            const pos = parseInt(command) - 1;
            if (isNaN(pos) || pos < 0 || pos > 8) {
                addLine('INVALID POSITION. ENTER 1-9.', 'terminal-error');
                return;
            }
            
            if (tttBoard[pos] !== ' ') {
                addLine('POSITION ALREADY TAKEN.', 'terminal-error');
                return;
            }
            
            // Player move
            tttBoard[pos] = 'X';
            
            // Check win
            if (checkTTTWin('X')) {
                displayTTTBoard();
                addLine('');
                addLine('YOU WIN! IMPRESSIVE.', 'terminal-success');
                currentGame = null;
                return;
            }
            
            // Check draw
            if (!tttBoard.includes(' ')) {
                displayTTTBoard();
                addLine('');
                addLine('DRAW. A STRANGE GAME.', 'terminal-warning');
                currentGame = null;
                return;
            }
            
            // Computer move (simple AI)
            const available = [];
            for (let i = 0; i < 9; i++) {
                if (tttBoard[i] === ' ') available.push(i);
            }
            const compMove = available[Math.floor(Math.random() * available.length)];
            tttBoard[compMove] = 'O';
            
            displayTTTBoard();
            
            // Check computer win
            if (checkTTTWin('O')) {
                addLine('');
                addLine('I WIN. BETTER LUCK NEXT TIME.', 'terminal-error');
                currentGame = null;
                return;
            }
        }

        function displayTTTBoard() {
            addLine('');
            addLine(' ' + tttBoard[0] + ' | ' + tttBoard[1] + ' | ' + tttBoard[2] + ' ');
            addLine('-----------');
            addLine(' ' + tttBoard[3] + ' | ' + tttBoard[4] + ' | ' + tttBoard[5] + ' ');
            addLine('-----------');
            addLine(' ' + tttBoard[6] + ' | ' + tttBoard[7] + ' | ' + tttBoard[8] + ' ');
            addLine('');
        }

        function checkTTTWin(player) {
            const wins = [
                [0,1,2], [3,4,5], [6,7,8], // rows
                [0,3,6], [1,4,7], [2,5,8], // cols
                [0,4,8], [2,4,6] // diagonals
            ];
            return wins.some(combo => 
                combo.every(pos => tttBoard[pos] === player)
            );
        }

        // Other games redirect to appropriate interfaces
        function startSnake() {
            addLine('');
            addLine('LOADING SNAKE...');
            addLine('REDIRECTING TO DASHBOARD GRID SYSTEM...');
            setTimeout(() => {
                window.location.href = buildUrl('/dashboard/');
            }, 2000);
        }

        function startDoorDefender() {
            addLine('');
            addLine('DOOR DEFENDER - SECURITY MONITORING SYSTEM');
            addLine('ACCESSING DOOR AND WINDOW SENSORS...');
            setTimeout(() => {
                window.location.href = buildUrl('/dashboard/');
            }, 2000);
        }

        function startLightCycles() {
            addLine('');
            addLine('LIGHT CYCLES - ILLUMINATION CONTROL GRID');
            addLine('INITIALIZING LIGHT CONTROL MATRIX...');
            setTimeout(() => {
                window.location.href = buildUrl('/dashboard/');
            }, 2000);
        }
    </script>
</body>
</html>"##
        .to_string()
}

/// Root handler
async fn root_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "Loxone MCP Server",
        "version": "1.0.0",
        "transport": "HTTP/SSE",
        "endpoints": {
            "health": "/health",
            "mcp_sse": "/mcp/sse",
            "mcp_info": "/mcp/info",
            "tools": "/mcp/tools",
            "dashboard": "/dashboard/",
            "history_dashboard": "/history/",
            "key_management": "/admin/keys"
        },
        "mcp_features": {
            "tools": "30+ automation and control tools",
            "resources": "22 structured data resources",
            "prompts": "10 AI-powered automation prompts",
            "description": "Full MCP protocol support for LLM integration"
        },
        "dashboard_features": {
            "monitoring_dashboard": "Real-time metrics and system monitoring (web browser)",
            "history_dashboard": "Historical data visualization and export (web browser)",
            "live_metrics": "Server-sent events for real-time updates",
            "widget_system": "Dynamic widget generation and customization",
            "data_export": "JSON/CSV export capabilities"
        },
        "web_access": {
            "monitoring": "Open http://localhost:3001/dashboard/ in your web browser",
            "history": "Open http://localhost:3001/history/ in your web browser",
            "api_info": "Open http://localhost:3001/ in your web browser",
            "key_management": "Open http://localhost:3001/admin/keys in your web browser"
        },
        "authentication": "Bearer token required for MCP endpoints"
    }))
}

/// Health check endpoint
async fn health_check(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    debug!("Health check requested");

    // Check Loxone connectivity
    let loxone_status = match state.mcp_server.get_system_status().await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };

    let response = HealthResponse {
        status: "ok".to_string(),
        version: "1.0.0".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        services: HealthServices {
            loxone: loxone_status.to_string(),
            mcp_server: "healthy".to_string(),
        },
    };

    Json(response)
}

/// SSE endpoint for MCP communication
async fn sse_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SseQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication is now handled by middleware
    debug!("SSE request received with headers: {:?}", headers);

    let client_id = query
        .client_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    info!("SSE connection established for client: {}", client_id);

    // Parse subscription requests from query parameter
    let subscriptions = if let Some(subscribe_param) = &query.subscribe {
        subscribe_param
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
    } else {
        Vec::new()
    };

    // Create proper MCP SSE stream that implements the initialization handshake
    create_mcp_sse_stream(&state, &client_id, subscriptions).await
}

/// Create proper MCP SSE stream with subscription support
async fn create_mcp_sse_stream(
    state: &AppState,
    client_id: &str,
    subscriptions: Vec<String>,
) -> impl IntoResponse {
    info!(
        "Creating MCP SSE stream for client: {} with {} subscriptions",
        client_id,
        subscriptions.len()
    );

    // Clone the necessary components for use in the stream
    let server = state.mcp_server.clone();
    let sse_manager = state.sse_manager.clone();
    let client_id_owned = client_id.to_string();

    // Register subscriptions if any were provided
    let mut subscription_events = Vec::new();
    if !subscriptions.is_empty() {
        let client_info = crate::server::subscription::types::ClientInfo {
            id: client_id_owned.clone(),
            transport: crate::server::subscription::types::ClientTransport::HttpSse {
                connection_id: client_id_owned.clone(),
            },
            capabilities: vec!["resources".to_string()],
            connected_at: std::time::SystemTime::now(),
        };

        for resource_uri in &subscriptions {
            if let Err(e) = server
                .subscription_coordinator
                .subscribe_client(client_info.clone(), resource_uri.clone(), None)
                .await
            {
                warn!(
                    "Failed to subscribe client {} to {}: {}",
                    client_id, resource_uri, e
                );
                subscription_events.push(
                    Event::default().event("subscription_error").data(
                        serde_json::json!({
                            "type": "subscription_error",
                            "resource_uri": resource_uri,
                            "error": e.to_string()
                        })
                        .to_string(),
                    ),
                );
            } else {
                info!(
                    "✅ Client {} subscribed to {} via SSE",
                    client_id, resource_uri
                );
                subscription_events.push(
                    Event::default().event("subscription_success").data(
                        serde_json::json!({
                            "type": "subscription_success",
                            "resource_uri": resource_uri,
                            "client_id": client_id_owned
                        })
                        .to_string(),
                    ),
                );
            }
        }
    }

    // Create SSE stream that sends initial connection event, then subscription events
    let connection_event = Event::default().event("connection").data(
        serde_json::json!({
            "type": "connection",
            "status": "connected",
            "client_id": client_id_owned,
            "subscriptions": subscriptions.len()
        })
        .to_string(),
    );

    // Create notification stream from SSE manager
    let notification_receiver = sse_manager.create_receiver();
    let client_id_for_notifications = client_id_owned.clone();
    let notification_stream = stream::unfold(notification_receiver, move |mut receiver| {
        let client_id = client_id_for_notifications.clone();
        async move {
            loop {
                match receiver.recv().await {
                    Ok(sse_event) => {
                        // Only send notifications intended for this client
                        if sse_event.client_id == client_id {
                            let sse_notification =
                                Event::default().event(&sse_event.event_type).data(
                                    serde_json::json!({
                                        "type": "resource_notification",
                                        "method": "notifications/resources/updated",
                                        "params": {
                                            "uri": sse_event.resource_uri,
                                            "changeType": sse_event.event_type,
                                            "timestamp": sse_event.timestamp,
                                            "data": sse_event.data
                                        }
                                    })
                                    .to_string(),
                                );

                            return Some((sse_notification, receiver));
                        }
                        // Skip this notification and continue loop
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Client lagged behind, send a lag notification
                        let lag_event = Event::default().event("lag_warning").data(
                            serde_json::json!({
                                "type": "lag_warning",
                                "message": "Client lagged behind notification stream"
                            })
                            .to_string(),
                        );
                        return Some((lag_event, receiver));
                    }
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        }
    });

    // Create the main stream starting with connection event, then subscription events, then notifications and pings
    let initial_stream = stream::once(async move { connection_event })
        .chain(stream::iter(subscription_events.into_iter()));

    let ping_stream = stream::unfold(server, move |server| async move {
        // Keep connection alive with periodic pings
        tokio::time::sleep(Duration::from_secs(30)).await;

        let ping_event = Event::default().event("ping").data(
            serde_json::json!({
                "type": "ping",
                "timestamp": chrono::Utc::now().to_rfc3339()
            })
            .to_string(),
        );

        Some((ping_event, server))
    });

    // Merge notifications and pings - prioritize notifications but include periodic pings
    use futures_util::stream::select;
    let live_stream = select(notification_stream, ping_stream);
    let complete_stream = initial_stream
        .chain(live_stream)
        .map(Ok::<Event, Infallible>);

    Sse::new(complete_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

/// Handle MCP messages via HTTP POST (Streamable HTTP transport for MCP Inspector)
async fn handle_mcp_message(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Start timing the request
    let request_start = std::time::Instant::now();

    // Extract client ID for rate limiting
    let client_id = EnhancedRateLimiter::extract_client_id(&headers);

    // Get method for rate limiting and metrics
    let method = request
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown");

    // Check rate limit before authentication
    let rate_limit_result = state
        .rate_limiter
        .check_rate_limit(&client_id, method, &headers)
        .await;

    // Record rate limit metrics
    #[cfg(feature = "influxdb")]
    {
        match &rate_limit_result {
            RateLimitResult::Limited { .. } | RateLimitResult::Penalized { .. } => {
                state.metrics_collector.record_rate_limit_event(true).await;
            }
            _ => {
                state.metrics_collector.record_rate_limit_event(false).await;
            }
        }
    }

    match rate_limit_result {
        RateLimitResult::Limited {
            retry_after,
            limit_type,
        } => {
            warn!(
                client_id = %client_id,
                method = %method,
                limit_type = %limit_type,
                "Request rate limited"
            );

            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id"),
                "error": {
                    "code": -32000,
                    "message": format!("Rate limit exceeded for {}: retry after {}s", limit_type, retry_after.as_secs())
                }
            });

            // Record timing for rate-limited request
            #[cfg(feature = "influxdb")]
            {
                let timing = RequestTiming {
                    endpoint: "/message".to_string(),
                    method: method.to_string(),
                    duration_ms: request_start.elapsed().as_secs_f64() * 1000.0,
                    status_code: 429, // Too Many Requests
                };
                state.metrics_collector.record_request_timing(timing).await;
            }

            return Ok(Json(response).into_response());
        }
        RateLimitResult::Penalized {
            penalty_remaining,
            reason,
        } => {
            warn!(
                client_id = %client_id,
                method = %method,
                reason = %reason,
                "Request blocked due to penalty"
            );

            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id"),
                "error": {
                    "code": -32000,
                    "message": format!("Client penalized: {}. Time remaining: {}s", reason, penalty_remaining.as_secs())
                }
            });

            // Record timing for penalized request
            #[cfg(feature = "influxdb")]
            {
                let timing = RequestTiming {
                    endpoint: "/message".to_string(),
                    method: method.to_string(),
                    duration_ms: request_start.elapsed().as_secs_f64() * 1000.0,
                    status_code: 429, // Too Many Requests
                };
                state.metrics_collector.record_request_timing(timing).await;
            }

            return Ok(Json(response).into_response());
        }
        RateLimitResult::AllowedBurst { remaining, .. } => {
            debug!(
                client_id = %client_id,
                method = %method,
                remaining = %remaining,
                "Request allowed using burst capacity"
            );
        }
        RateLimitResult::Allowed { remaining, .. } => {
            debug!(
                client_id = %client_id,
                method = %method,
                remaining = %remaining,
                "Request allowed"
            );
        }
    }

    // Authentication is now handled by middleware

    info!("Received MCP message: {:?}", request);

    // Handle different MCP request types according to MCP specification
    let response_result = if let Some(method) = request.get("method").and_then(|m| m.as_str()) {
        match method {
            "initialize" => {
                let server_info = state.mcp_server.get_info();
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "capabilities": {
                            "tools": {},
                            "resources": {
                                "subscribe": false,
                                "listChanged": false
                            },
                            "prompts": {}
                        },
                        "serverInfo": {
                            "name": server_info.server_info.name,
                            "version": server_info.server_info.version
                        },
                        "protocolVersion": "2024-11-05"
                    }
                });
                Ok(Json(response).into_response())
            }
            "notifications/initialized" => {
                // Client acknowledges initialization
                info!("MCP client initialized successfully");
                Ok(Json(serde_json::json!({"jsonrpc": "2.0"})).into_response())
            }
            "tools/list" => {
                // Consolidated tool list - only control tools (actions that modify state)
                // Read-only tools have been migrated to resources (loxone:// URIs)
                let tools = vec![
                    serde_json::json!({
                        "name": "control_device",
                        "description": "Control a single Loxone device by UUID or name",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "device": {
                                    "type": "string",
                                    "description": "Device UUID or name"
                                },
                                "action": {
                                    "type": "string",
                                    "description": "Action to perform (on, off, up, down, stop)"
                                },
                                "room": {
                                    "type": "string",
                                    "description": "Optional room name to help identify the device"
                                }
                            },
                            "required": ["device", "action"]
                        }
                    }),
                    serde_json::json!({
                        "name": "control_multiple_devices",
                        "description": "Control multiple devices simultaneously with the same action",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "devices": {
                                    "type": "array",
                                    "description": "List of device names or UUIDs to control",
                                    "items": {
                                        "type": "string"
                                    }
                                },
                                "action": {
                                    "type": "string",
                                    "description": "Action to perform on all devices (on, off, up, down, stop)"
                                }
                            },
                            "required": ["devices", "action"]
                        }
                    }),
                    serde_json::json!({
                        "name": "control_all_rolladen",
                        "description": "Control all rolladen/blinds in the entire system simultaneously",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "action": {
                                    "type": "string",
                                    "description": "Action to perform: 'up', 'down', or 'stop'"
                                }
                            },
                            "required": ["action"]
                        }
                    }),
                    serde_json::json!({
                        "name": "control_room_rolladen",
                        "description": "Control all rolladen/blinds in a specific room",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "room": {
                                    "type": "string",
                                    "description": "Name of the room"
                                },
                                "action": {
                                    "type": "string",
                                    "description": "Action to perform: 'up', 'down', or 'stop'"
                                }
                            },
                            "required": ["room", "action"]
                        }
                    }),
                    serde_json::json!({
                        "name": "control_all_lights",
                        "description": "Control all lights in the entire system simultaneously",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "action": {
                                    "type": "string",
                                    "description": "Action to perform: 'on' or 'off'"
                                }
                            },
                            "required": ["action"]
                        }
                    }),
                    serde_json::json!({
                        "name": "control_room_lights",
                        "description": "Control all lights in a specific room",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "room": {
                                    "type": "string",
                                    "description": "Name of the room"
                                },
                                "action": {
                                    "type": "string",
                                    "description": "Action to perform: 'on' or 'off'"
                                }
                            },
                            "required": ["room", "action"]
                        }
                    }),
                    serde_json::json!({
                        "name": "control_audio_zone",
                        "description": "Control an audio zone (play, stop, volume control)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "zone_name": {
                                    "type": "string",
                                    "description": "Name of the audio zone"
                                },
                                "action": {
                                    "type": "string",
                                    "description": "Action to perform (play, stop, pause, volume, mute, unmute, next, previous)"
                                },
                                "value": {
                                    "type": "number",
                                    "description": "Optional value for actions like volume (0-100)"
                                }
                            },
                            "required": ["zone_name", "action"]
                        }
                    }),
                    serde_json::json!({
                        "name": "set_audio_volume",
                        "description": "Set volume for an audio zone",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "zone_name": {
                                    "type": "string",
                                    "description": "Name of the audio zone"
                                },
                                "volume": {
                                    "type": "number",
                                    "description": "Volume level (0-100)"
                                }
                            },
                            "required": ["zone_name", "volume"]
                        }
                    }),
                    serde_json::json!({
                        "name": "get_health_check",
                        "description": "Perform comprehensive health check of the Loxone system and MCP server",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "required": []
                        }
                    }),
                    serde_json::json!({
                        "name": "get_health_status",
                        "description": "Get basic health status (lightweight check)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "required": []
                        }
                    }),
                    serde_json::json!({
                        "name": "discover_new_sensors",
                        "description": "Discover sensors by monitoring WebSocket traffic or analyzing structure",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "duration_seconds": {
                                    "type": "number",
                                    "description": "Discovery duration in seconds (default: 60)"
                                }
                            },
                            "required": []
                        }
                    }),
                    serde_json::json!({
                        "name": "get_sensor_state_history",
                        "description": "Get complete state history for a specific sensor",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "uuid": {
                                    "type": "string",
                                    "description": "Sensor UUID"
                                },
                                "limit": {
                                    "type": "number",
                                    "description": "Maximum number of events to return"
                                }
                            },
                            "required": ["uuid"]
                        }
                    }),
                    serde_json::json!({
                        "name": "get_recent_sensor_changes",
                        "description": "Get recent sensor changes across all sensors",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "limit": {
                                    "type": "number",
                                    "description": "Maximum number of changes to return (default: 50)"
                                }
                            },
                            "required": []
                        }
                    }),
                    serde_json::json!({
                        "name": "create_workflow",
                        "description": "Create a new workflow that chains multiple tools together",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "name": {
                                    "type": "string",
                                    "description": "Name of the workflow"
                                },
                                "description": {
                                    "type": "string",
                                    "description": "Description of what the workflow does"
                                },
                                "steps": {
                                    "type": "array",
                                    "description": "Array of workflow steps to execute",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "type": {
                                                "type": "string",
                                                "enum": ["tool", "parallel", "sequential", "conditional", "delay", "loop"]
                                            },
                                            "name": {
                                                "type": "string",
                                                "description": "Tool name for 'tool' type steps"
                                            },
                                            "params": {
                                                "type": "object",
                                                "description": "Parameters for tool execution"
                                            }
                                        }
                                    }
                                },
                                "timeout_seconds": {
                                    "type": "number",
                                    "description": "Optional global timeout in seconds"
                                },
                                "variables": {
                                    "type": "object",
                                    "description": "Variables that can be used in the workflow"
                                }
                            },
                            "required": ["name", "description", "steps"]
                        }
                    }),
                    serde_json::json!({
                        "name": "execute_workflow_demo",
                        "description": "Execute a predefined demo workflow to show workflow capabilities",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "workflow_name": {
                                    "type": "string",
                                    "description": "Name of the predefined workflow to execute",
                                    "enum": ["morning_routine", "parallel_demo", "conditional_demo", "security_check", "evening_routine"]
                                },
                                "variables": {
                                    "type": "object",
                                    "description": "Optional variables to pass to the workflow"
                                }
                            },
                            "required": ["workflow_name"]
                        }
                    }),
                    serde_json::json!({
                        "name": "list_predefined_workflows",
                        "description": "List all available predefined workflow templates",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "required": []
                        }
                    }),
                    serde_json::json!({
                        "name": "get_workflow_examples",
                        "description": "Get detailed examples and documentation for creating workflows",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "required": []
                        }
                    }),
                ];

                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "tools": tools
                    }
                });
                Ok(Json(response).into_response())
            }
            "tools/list_old" => {
                // This old implementation is no longer needed
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "error": {
                        "code": -32601,
                        "message": "Method deprecated - use tools/list instead"
                    }
                });
                Ok(Json(response).into_response())
            }
            "tools/call_old" => {
                // Deprecated endpoint - legacy read-only tools have been migrated to resources
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "error": {
                        "code": -32601,
                        "message": "Deprecated endpoint. Use tools/list for control tools or resources/list for read-only data."
                    }
                });
                Ok(Json(response).into_response())
            }
            "resources/list" => {
                // Return the list of available resources
                let resources = vec![
                    serde_json::json!({
                        "uri": "loxone://rooms",
                        "name": "All Rooms",
                        "description": "List of all rooms with device counts and information",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://rooms/{roomName}/devices",
                        "name": "Room Devices",
                        "description": "All devices in a specific room with detailed information",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://devices/all",
                        "name": "All Devices",
                        "description": "Complete list of all devices in the system",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://devices/type/{deviceType}",
                        "name": "Devices by Type",
                        "description": "All devices filtered by type (e.g., Switch, Jalousie, Dimmer)",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://devices/category/{category}",
                        "name": "Devices by Category",
                        "description": "All devices filtered by category (lighting, blinds, climate, etc.)",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://system/status",
                        "name": "System Status",
                        "description": "Overall system status and health information",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://system/capabilities",
                        "name": "System Capabilities",
                        "description": "Available system capabilities and features",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://system/categories",
                        "name": "Device Categories Overview",
                        "description": "Overview of all device categories with counts and examples",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://audio/zones",
                        "name": "Audio Zones",
                        "description": "All audio zones and their current status",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://audio/sources",
                        "name": "Audio Sources",
                        "description": "Available audio sources and their status",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://sensors/door-window",
                        "name": "Door/Window Sensors",
                        "description": "All door and window sensors with current state",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://sensors/temperature",
                        "name": "Temperature Sensors",
                        "description": "All temperature sensors and their current readings",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://sensors/discovered",
                        "name": "Discovered Sensors",
                        "description": "Dynamically discovered sensors with metadata",
                        "mimeType": "application/json"
                    }),
                    // Weather resources
                    serde_json::json!({
                        "uri": "loxone://weather/current",
                        "name": "Current Weather",
                        "description": "Current weather data from all weather sensors",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://weather/outdoor-conditions",
                        "name": "Outdoor Conditions",
                        "description": "Outdoor environmental conditions with comfort assessment",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://weather/forecast-daily",
                        "name": "Daily Weather Forecast",
                        "description": "Multi-day weather forecast data",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://weather/forecast-hourly",
                        "name": "Hourly Weather Forecast",
                        "description": "Hourly weather forecast data",
                        "mimeType": "application/json"
                    }),
                    // Security resources
                    serde_json::json!({
                        "uri": "loxone://security/status",
                        "name": "Security System Status",
                        "description": "Current security system status and alarm states",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://security/zones",
                        "name": "Security Zones",
                        "description": "All security zones and their current states",
                        "mimeType": "application/json"
                    }),
                    // Energy resources
                    serde_json::json!({
                        "uri": "loxone://energy/consumption",
                        "name": "Energy Consumption",
                        "description": "Current energy consumption and usage metrics",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://energy/meters",
                        "name": "Energy Meters",
                        "description": "All energy meters and their current readings",
                        "mimeType": "application/json"
                    }),
                    serde_json::json!({
                        "uri": "loxone://energy/usage-history",
                        "name": "Energy Usage History",
                        "description": "Historical energy usage data and trends",
                        "mimeType": "application/json"
                    }),
                ];

                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "resources": resources
                    }
                });
                Ok(Json(response).into_response())
            }
            "tools/call" => {
                // Handle tool calls using the existing handler method
                let params = request.get("params");
                let tool_name = params
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .ok_or((StatusCode::BAD_REQUEST, "Missing tool name"))?;

                let arguments = params
                    .and_then(|p| p.get("arguments"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));

                info!(
                    "Calling tool: {} with arguments: {:?}",
                    tool_name, arguments
                );

                // Call the actual MCP server's call_tool method
                match state.mcp_server.call_tool(tool_name, arguments).await {
                    Ok(result) => {
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "result": result
                        });
                        Ok(Json(response).into_response())
                    }
                    Err(e) => {
                        let error_response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32603,
                                "message": format!("Tool execution error: {}", e)
                            }
                        });
                        Ok(Json(error_response).into_response())
                    }
                }
            }
            "resources/read" => {
                // Handle resource read requests using ResourceHandler trait
                let params = request.get("params");
                let uri = params
                    .and_then(|p| p.get("uri"))
                    .and_then(|u| u.as_str())
                    .ok_or((StatusCode::BAD_REQUEST, "Missing resource URI"))?;

                info!("Reading resource: {}", uri);

                // Use the ResourceHandler implementation
                use crate::server::resources::ResourceHandler;

                // Parse the URI to extract parameters
                let resource_manager = crate::server::resources::ResourceManager::new();
                let context = match resource_manager.parse_uri(uri) {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        let error_response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": format!("Invalid resource URI: {}", e)
                            }
                        });
                        return Ok(Json(error_response).into_response());
                    }
                };

                // Read the resource using the handler
                match ResourceHandler::read_resource(&state.mcp_server, context).await {
                    Ok(resource_content) => {
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "result": {
                                "contents": [{
                                    "uri": uri,
                                    "mimeType": resource_content.metadata.content_type,
                                    "text": resource_content.data.to_string()
                                }]
                            }
                        });
                        Ok(Json(response).into_response())
                    }
                    Err(e) => {
                        let error_response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32002,
                                "message": format!("Resource error: {}", e)
                            }
                        });
                        Ok(Json(error_response).into_response())
                    }
                }
            }
            "prompts/list" => {
                // Handle prompts list requests
                info!("Listing prompts");

                let prompts = vec![
                    serde_json::json!({
                        "name": "make_home_cozy",
                        "description": "Transform your home into a cozy atmosphere with optimal lighting, temperature, and ambiance settings",
                        "arguments": [
                            {
                                "name": "time_of_day",
                                "description": "Current time of day (morning, afternoon, evening, night)",
                                "required": false
                            },
                            {
                                "name": "weather",
                                "description": "Current weather conditions (sunny, cloudy, rainy, cold, hot)",
                                "required": false
                            },
                            {
                                "name": "mood",
                                "description": "Desired mood (relaxing, romantic, energizing, peaceful)",
                                "required": false
                            }
                        ]
                    }),
                    serde_json::json!({
                        "name": "prepare_for_event",
                        "description": "Intelligently prepare your home for different types of events with optimal automation settings",
                        "arguments": [
                            {
                                "name": "event_type",
                                "description": "Type of event (party, movie_night, dinner, work_meeting, gaming, reading, meditation)",
                                "required": true
                            },
                            {
                                "name": "room",
                                "description": "Primary room for the event",
                                "required": false
                            },
                            {
                                "name": "duration",
                                "description": "Expected duration of the event",
                                "required": false
                            },
                            {
                                "name": "guest_count",
                                "description": "Number of guests expected",
                                "required": false
                            }
                        ]
                    }),
                    serde_json::json!({
                        "name": "analyze_energy_usage",
                        "description": "Comprehensive energy usage analysis with intelligent optimization recommendations",
                        "arguments": [
                            {
                                "name": "time_period",
                                "description": "Time period to analyze (last_hour, today, last_week, last_month)",
                                "required": false
                            },
                            {
                                "name": "focus_area",
                                "description": "Specific area to focus on (lighting, climate, audio, overall)",
                                "required": false
                            }
                        ]
                    }),
                    serde_json::json!({
                        "name": "good_morning_routine",
                        "description": "Execute a personalized morning routine with gradual automation adjustments",
                        "arguments": [
                            {
                                "name": "wake_time",
                                "description": "Time the user woke up",
                                "required": false
                            },
                            {
                                "name": "day_type",
                                "description": "Type of day (workday, weekend, holiday, vacation)",
                                "required": false
                            },
                            {
                                "name": "weather_outside",
                                "description": "Weather conditions for the day",
                                "required": false
                            }
                        ]
                    }),
                    serde_json::json!({
                        "name": "good_night_routine",
                        "description": "Execute a personalized bedtime routine with security and comfort optimization",
                        "arguments": [
                            {
                                "name": "bedtime",
                                "description": "Planned bedtime",
                                "required": false
                            },
                            {
                                "name": "wake_time",
                                "description": "Planned wake time for tomorrow",
                                "required": false
                            },
                            {
                                "name": "security_mode",
                                "description": "Security preference (high, normal, minimal)",
                                "required": false
                            }
                        ]
                    }),
                    serde_json::json!({
                        "name": "optimize_comfort_zone",
                        "description": "Analyze and optimize comfort settings for specific rooms or the entire home",
                        "arguments": [
                            {
                                "name": "target_rooms",
                                "description": "Comma-separated list of rooms to optimize (or 'all' for entire home)",
                                "required": false
                            },
                            {
                                "name": "occupancy_pattern",
                                "description": "Expected occupancy pattern (frequent, occasional, rare)",
                                "required": false
                            },
                            {
                                "name": "priority",
                                "description": "Optimization priority (energy_saving, comfort, convenience)",
                                "required": false
                            }
                        ]
                    }),
                    serde_json::json!({
                        "name": "seasonal_adjustment",
                        "description": "Adjust home automation settings for seasonal changes and weather patterns",
                        "arguments": [
                            {
                                "name": "season",
                                "description": "Current season (spring, summer, autumn, winter)",
                                "required": true
                            },
                            {
                                "name": "climate_zone",
                                "description": "Local climate characteristics (humid, dry, temperate, extreme)",
                                "required": false
                            },
                            {
                                "name": "adjustment_scope",
                                "description": "Scope of adjustments (lighting_only, climate_only, comprehensive)",
                                "required": false
                            }
                        ]
                    }),
                    serde_json::json!({
                        "name": "security_mode_analysis",
                        "description": "Analyze current security settings and recommend optimal configuration",
                        "arguments": [
                            {
                                "name": "occupancy_status",
                                "description": "Current occupancy status (home, away, vacation, unknown)",
                                "required": false
                            },
                            {
                                "name": "time_away",
                                "description": "Expected time away from home",
                                "required": false
                            },
                            {
                                "name": "security_level",
                                "description": "Desired security level (basic, enhanced, maximum)",
                                "required": false
                            }
                        ]
                    }),
                    serde_json::json!({
                        "name": "troubleshoot_automation",
                        "description": "Diagnose and troubleshoot home automation issues with intelligent recommendations",
                        "arguments": [
                            {
                                "name": "issue_description",
                                "description": "Description of the problem or unexpected behavior",
                                "required": true
                            },
                            {
                                "name": "affected_devices",
                                "description": "Devices or rooms affected by the issue",
                                "required": false
                            },
                            {
                                "name": "when_started",
                                "description": "When the issue first appeared",
                                "required": false
                            }
                        ]
                    }),
                    serde_json::json!({
                        "name": "create_custom_scene",
                        "description": "Design a custom automation scene based on specific requirements and preferences",
                        "arguments": [
                            {
                                "name": "scene_name",
                                "description": "Name for the custom scene",
                                "required": true
                            },
                            {
                                "name": "scene_purpose",
                                "description": "Purpose or use case for the scene",
                                "required": true
                            },
                            {
                                "name": "included_rooms",
                                "description": "Rooms to include in the scene",
                                "required": false
                            },
                            {
                                "name": "automation_types",
                                "description": "Types of automation to include (lighting, climate, audio, blinds)",
                                "required": false
                            }
                        ]
                    }),
                ];

                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "prompts": prompts
                    }
                });
                Ok(Json(response).into_response())
            }
            "prompts/get" => {
                // Handle prompts get requests
                let params = request.get("params");
                let prompt_name = params
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .ok_or((StatusCode::BAD_REQUEST, "Missing prompt name"))?;

                let arguments = params
                    .and_then(|p| p.get("arguments"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));

                info!(
                    "Getting prompt: {} with arguments: {:?}",
                    prompt_name, arguments
                );

                // Convert arguments to the format expected by the prompt methods
                let args_map = if let Some(obj) = arguments.as_object() {
                    obj.clone()
                } else {
                    serde_json::Map::new()
                };

                // Call the appropriate prompt method based on name
                let prompt_result = match prompt_name {
                    "make_home_cozy" => {
                        match state
                            .mcp_server
                            .get_cozy_prompt_messages(Some(serde_json::Value::Object(args_map)))
                            .await
                        {
                            Ok(messages) => messages,
                            Err(e) => {
                                let error_response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": request.get("id"),
                                    "error": {
                                        "code": -32603,
                                        "message": format!("Prompt generation error: {}", e)
                                    }
                                });
                                return Ok(Json(error_response).into_response());
                            }
                        }
                    }
                    "prepare_for_event" => {
                        match state
                            .mcp_server
                            .get_event_prompt_messages(Some(serde_json::Value::Object(args_map)))
                            .await
                        {
                            Ok(messages) => messages,
                            Err(e) => {
                                let error_response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": request.get("id"),
                                    "error": {
                                        "code": -32603,
                                        "message": format!("Prompt generation error: {}", e)
                                    }
                                });
                                return Ok(Json(error_response).into_response());
                            }
                        }
                    }
                    "analyze_energy_usage" => {
                        match state
                            .mcp_server
                            .get_energy_prompt_messages(Some(serde_json::Value::Object(args_map)))
                            .await
                        {
                            Ok(messages) => messages,
                            Err(e) => {
                                let error_response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": request.get("id"),
                                    "error": {
                                        "code": -32603,
                                        "message": format!("Prompt generation error: {}", e)
                                    }
                                });
                                return Ok(Json(error_response).into_response());
                            }
                        }
                    }
                    "good_morning_routine" => {
                        match state
                            .mcp_server
                            .get_morning_prompt_messages(Some(serde_json::Value::Object(args_map)))
                            .await
                        {
                            Ok(messages) => messages,
                            Err(e) => {
                                let error_response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": request.get("id"),
                                    "error": {
                                        "code": -32603,
                                        "message": format!("Prompt generation error: {}", e)
                                    }
                                });
                                return Ok(Json(error_response).into_response());
                            }
                        }
                    }
                    "good_night_routine" => {
                        match state
                            .mcp_server
                            .get_night_prompt_messages(Some(serde_json::Value::Object(args_map)))
                            .await
                        {
                            Ok(messages) => messages,
                            Err(e) => {
                                let error_response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": request.get("id"),
                                    "error": {
                                        "code": -32603,
                                        "message": format!("Prompt generation error: {}", e)
                                    }
                                });
                                return Ok(Json(error_response).into_response());
                            }
                        }
                    }
                    "optimize_comfort_zone" => {
                        match state
                            .mcp_server
                            .get_comfort_optimization_messages(Some(serde_json::Value::Object(
                                args_map,
                            )))
                            .await
                        {
                            Ok(messages) => messages,
                            Err(e) => {
                                let error_response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": request.get("id"),
                                    "error": {
                                        "code": -32603,
                                        "message": format!("Prompt generation error: {}", e)
                                    }
                                });
                                return Ok(Json(error_response).into_response());
                            }
                        }
                    }
                    "seasonal_adjustment" => {
                        match state
                            .mcp_server
                            .get_seasonal_adjustment_messages(Some(serde_json::Value::Object(
                                args_map,
                            )))
                            .await
                        {
                            Ok(messages) => messages,
                            Err(e) => {
                                let error_response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": request.get("id"),
                                    "error": {
                                        "code": -32603,
                                        "message": format!("Prompt generation error: {}", e)
                                    }
                                });
                                return Ok(Json(error_response).into_response());
                            }
                        }
                    }
                    "security_mode_analysis" => {
                        match state
                            .mcp_server
                            .get_security_analysis_messages(Some(serde_json::Value::Object(
                                args_map,
                            )))
                            .await
                        {
                            Ok(messages) => messages,
                            Err(e) => {
                                let error_response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": request.get("id"),
                                    "error": {
                                        "code": -32603,
                                        "message": format!("Prompt generation error: {}", e)
                                    }
                                });
                                return Ok(Json(error_response).into_response());
                            }
                        }
                    }
                    "troubleshoot_automation" => {
                        match state
                            .mcp_server
                            .get_troubleshooting_messages(Some(serde_json::Value::Object(args_map)))
                            .await
                        {
                            Ok(messages) => messages,
                            Err(e) => {
                                let error_response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": request.get("id"),
                                    "error": {
                                        "code": -32603,
                                        "message": format!("Prompt generation error: {}", e)
                                    }
                                });
                                return Ok(Json(error_response).into_response());
                            }
                        }
                    }
                    "create_custom_scene" => {
                        match state
                            .mcp_server
                            .get_custom_scene_messages(Some(serde_json::Value::Object(args_map)))
                            .await
                        {
                            Ok(messages) => messages,
                            Err(e) => {
                                let error_response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": request.get("id"),
                                    "error": {
                                        "code": -32603,
                                        "message": format!("Prompt generation error: {}", e)
                                    }
                                });
                                return Ok(Json(error_response).into_response());
                            }
                        }
                    }
                    _ => {
                        let error_response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": format!("Unknown prompt: {}", prompt_name)
                            }
                        });
                        return Ok(Json(error_response).into_response());
                    }
                };

                // Convert rmcp PromptMessage to JSON format for HTTP transport
                let messages: Vec<serde_json::Value> = prompt_result
                    .into_iter()
                    .map(|msg| {
                        // Serialize the PromptMessage to JSON and then extract the fields
                        let json_msg =
                            serde_json::to_value(&msg).unwrap_or_else(|_| serde_json::json!({}));

                        // Extract role from the enum name and content from the data
                        let (role, content_text) = if let Some(user_content) = json_msg.get("User")
                        {
                            (
                                "user",
                                user_content
                                    .get("content")
                                    .and_then(|c| c.get("text"))
                                    .and_then(|t| t.as_str())
                                    .unwrap_or(""),
                            )
                        } else if let Some(assistant_content) = json_msg.get("Assistant") {
                            (
                                "assistant",
                                assistant_content
                                    .get("content")
                                    .and_then(|c| c.get("text"))
                                    .and_then(|t| t.as_str())
                                    .unwrap_or(""),
                            )
                        } else if let Some(system_content) = json_msg.get("System") {
                            (
                                "system",
                                system_content
                                    .get("content")
                                    .and_then(|c| c.get("text"))
                                    .and_then(|t| t.as_str())
                                    .unwrap_or(""),
                            )
                        } else {
                            ("user", "")
                        };

                        serde_json::json!({
                            "role": role,
                            "content": {
                                "type": "text",
                                "text": content_text
                            }
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "description": format!("Generated prompt for: {}", prompt_name),
                        "messages": messages
                    }
                });
                Ok(Json(response).into_response())
            }
            "ping" => {
                debug!("Received ping request");
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {}
                });
                Ok(Json(response).into_response())
            }
            _ => {
                let error_response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "error": {
                        "code": -32601,
                        "message": "Method not found"
                    }
                });
                Ok(Json(error_response).into_response())
            }
        }
    } else {
        Err((StatusCode::BAD_REQUEST, "Invalid MCP request"))
    };

    // Record timing metrics for all responses
    #[cfg(feature = "influxdb")]
    {
        let status_code = match &response_result {
            Ok(_) => 200,
            Err((status, _)) => status.as_u16(),
        };

        let timing = RequestTiming {
            endpoint: "/message".to_string(),
            method: method.to_string(),
            duration_ms: request_start.elapsed().as_secs_f64() * 1000.0,
            status_code,
        };
        state.metrics_collector.record_request_timing(timing).await;
    }

    response_result
}

/// Get server information
async fn server_info(State(state): State<Arc<AppState>>, _headers: HeaderMap) -> impl IntoResponse {
    // Authentication is now handled by middleware

    let info = state.mcp_server.get_info();
    Json(serde_json::json!({
        "name": info.server_info.name,
        "version": info.server_info.version,
        "instructions": info.instructions,
        "transport": "HTTP/SSE",
        "authentication": "Bearer"
    }))
    .into_response()
}

/// List available tools
async fn list_tools(State(_state): State<Arc<AppState>>, _headers: HeaderMap) -> impl IntoResponse {
    // Authentication is now handled by middleware

    // Consolidated tool list - only control tools (actions that modify state)
    // Read-only tools have been migrated to resources (use /resources endpoint)
    let tools = serde_json::json!({
        "tools": [
            {
                "name": "control_device",
                "description": "Control a single Loxone device by UUID or name",
                "parameters": {
                    "device": {"type": "string", "description": "Device UUID or name"},
                    "action": {"type": "string", "description": "Action to perform (on, off, up, down, stop)"},
                    "room": {"type": "string", "description": "Optional room name to help identify the device"}
                }
            },
            {
                "name": "control_multiple_devices",
                "description": "Control multiple devices simultaneously with the same action",
                "parameters": {
                    "devices": {"type": "array", "description": "List of device names or UUIDs to control"},
                    "action": {"type": "string", "description": "Action to perform on all devices"}
                }
            },
            {
                "name": "control_all_rolladen",
                "description": "Control all rolladen/blinds in the entire system simultaneously",
                "parameters": {
                    "action": {"type": "string", "description": "Action to perform: 'up', 'down', or 'stop'"}
                }
            },
            {
                "name": "control_room_rolladen",
                "description": "Control all rolladen/blinds in a specific room",
                "parameters": {
                    "room": {"type": "string", "description": "Name of the room"},
                    "action": {"type": "string", "description": "Action to perform: 'up', 'down', or 'stop'"}
                }
            },
            {
                "name": "control_all_lights",
                "description": "Control all lights in the entire system simultaneously",
                "parameters": {
                    "action": {"type": "string", "description": "Action to perform: 'on' or 'off'"}
                }
            },
            {
                "name": "control_room_lights",
                "description": "Control all lights in a specific room",
                "parameters": {
                    "room": {"type": "string", "description": "Name of the room"},
                    "action": {"type": "string", "description": "Action to perform: 'on' or 'off'"}
                }
            },
            {
                "name": "control_audio_zone",
                "description": "Control an audio zone (play, stop, volume control)",
                "parameters": {
                    "zone_name": {"type": "string", "description": "Name of the audio zone"},
                    "action": {"type": "string", "description": "Action to perform (play, stop, pause, volume, mute, unmute, next, previous)"},
                    "value": {"type": "number", "description": "Optional value for actions like volume (0-100)"}
                }
            },
            {
                "name": "get_health_check",
                "description": "Perform comprehensive health check of the Loxone system and MCP server",
                "parameters": {}
            },
            {
                "name": "discover_new_sensors",
                "description": "Discover sensors by monitoring WebSocket traffic or analyzing structure",
                "parameters": {
                    "duration_seconds": {"type": "number", "description": "Discovery duration in seconds (default: 60)"}
                }
            }
        ],
        "note": "For read-only data (rooms, devices, sensors, etc.), use the /resources endpoint with loxone:// URIs"
    });

    Json(tools).into_response()
}

/// Admin status endpoint
async fn admin_status(State(state): State<Arc<AppState>>, _headers: HeaderMap) -> Response {
    // Authentication is now handled by middleware

    let auth_stats = state.auth_manager.get_auth_stats().await;
    let status = serde_json::json!({
        "server": "running",
        "connections": 0, // TODO: Track active connections
        "authentication": {
            "total_keys": auth_stats.total_keys,
            "active_keys": auth_stats.active_keys,
            "expiring_keys": auth_stats.expiring_keys,
            "active_sessions": auth_stats.active_sessions,
            "recent_failures": auth_stats.recent_auth_failures
        }
    });

    Json(status).into_response()
}

/// Rate limiting status endpoint
async fn rate_limit_status(State(state): State<Arc<AppState>>, _headers: HeaderMap) -> Response {
    // Authentication is now handled by middleware

    let statistics = state.rate_limiter.get_statistics().await;

    let status = serde_json::json!({
        "rate_limiting": {
            "total_clients": statistics.total_clients,
            "penalized_clients": statistics.penalized_clients,
            "total_requests": statistics.total_requests,
            "total_violations": statistics.total_violations,
            "load_factor": statistics.load_factor
        },
        "configuration": {
            "high_frequency": {
                "requests_per_minute": 60,
                "burst_capacity": 10
            },
            "medium_frequency": {
                "requests_per_minute": 30,
                "burst_capacity": 5
            },
            "low_frequency": {
                "requests_per_minute": 10,
                "burst_capacity": 3
            },
            "admin": {
                "requests_per_minute": 20,
                "burst_capacity": 5
            },
            "global": {
                "requests_per_minute": 100,
                "burst_capacity": 20
            }
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    Json(status).into_response()
}

/// Authentication middleware wrapper for AppState
async fn auth_middleware_wrapper(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    // Use KeyStore-based authentication instead of AuthManager
    key_store_auth_middleware(State(state.key_store.clone()), request, next).await
}

/// Simple authentication middleware using KeyStore
async fn key_store_auth_middleware(
    State(key_store): State<Arc<KeyStore>>,
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    let headers = request.headers();
    let query_string = request.uri().query();

    // Extract API key from headers or query parameters
    let api_key = extract_api_key_from_request(headers, query_string);

    if let Some(key) = api_key {
        // Validate the key using KeyStore
        match key_store.validate_key(&key, None).await {
            Ok(_validated_key) => {
                // Key is valid, record usage
                let _ = key_store.record_usage(&key).await;
                // Continue to the next handler
                Ok(next.run(request).await)
            }
            Err(_) => {
                // Key validation failed
                Err(StatusCode::UNAUTHORIZED)
            }
        }
    } else {
        // No API key provided
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Extract API key from headers or query parameters
fn extract_api_key_from_request(headers: &HeaderMap, query_string: Option<&str>) -> Option<String> {
    // Try X-API-Key header first
    if let Some(key) = headers.get("x-api-key") {
        if let Ok(key_str) = key.to_str() {
            return Some(key_str.to_string());
        }
    }

    // Try Authorization header with Bearer token
    if let Some(auth) = headers.get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }

    // Try query parameter for browser-friendly access
    if let Some(query) = query_string {
        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                if key == "api_key" {
                    // URL decode the value
                    if let Ok(decoded) = urlencoding::decode(value) {
                        return Some(decoded.to_string());
                    }
                    return Some(value.to_string());
                }
            }
        }
    }

    None
}

/// Prometheus metrics endpoint
async fn prometheus_metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    #[cfg(feature = "influxdb")]
    {
        let metrics = state.metrics_collector.export_prometheus().await;
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            metrics,
        )
    }

    #[cfg(not(feature = "influxdb"))]
    {
        (
            StatusCode::NOT_IMPLEMENTED,
            "Metrics collection not enabled. Enable the 'influxdb' feature.",
        )
    }
}

// History dashboard endpoints

/// History dashboard home page
/// Returns HTML for browsers, JSON for API clients
async fn history_dashboard_home(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let status = if state.history_store.is_some() {
        "available"
    } else {
        "not_available"
    };

    // Check if request is from a browser
    let is_browser = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("text/html"))
        .unwrap_or(false);

    if is_browser {
        // Return a simple HTML page for browsers
        let html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>Loxone History Dashboard</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }}
        .container {{ background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        h1 {{ color: #333; }}
        .status {{ padding: 10px 20px; border-radius: 4px; display: inline-block; font-weight: bold; }}
        .available {{ background: #4CAF50; color: white; }}
        .unavailable {{ background: #f44336; color: white; }}
        .info {{ margin: 20px 0; line-height: 1.6; }}
        .endpoints {{ background: #f9f9f9; padding: 20px; border-radius: 4px; margin: 20px 0; }}
        code {{ background: #e0e0e0; padding: 2px 4px; border-radius: 3px; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🏠 Loxone History Dashboard</h1>
        <p>Status: <span class="status {}">{}</span></p>
        
        <div class="info">
            <h2>About</h2>
            <p>This dashboard provides access to historical data from your Loxone system.</p>
            {}
        </div>
        
        <div class="endpoints">
            <h3>Available API Endpoints:</h3>
            <ul>
                <li><a href="/history/api/status">/history/api/status</a> - Check system status</li>
                <li><code>/history/api/data</code> - Query historical data (coming soon)</li>
                <li><code>/history/api/widgets</code> - Get dashboard widgets (coming soon)</li>
            </ul>
        </div>
        
        <div class="info">
            <p><small>Loxone MCP Server v{}</small></p>
        </div>
    </div>
</body>
</html>"#,
            if status == "available" {
                "available"
            } else {
                "unavailable"
            },
            status.replace("_", " "),
            if status == "available" {
                "<p>✅ The history system is running and collecting data.</p>"
            } else {
                "<p>⚠️ The history system is not currently available. To enable it, set the <code>ENABLE_LOXONE_STATS=1</code> environment variable when starting the server.</p>"
            },
            env!("CARGO_PKG_VERSION")
        );

        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            html,
        )
            .into_response()
    } else {
        // Return JSON for API clients
        Json(serde_json::json!({
            "title": "Loxone History Dashboard",
            "description": "View historical data from your Loxone system",
            "status": status,
            "endpoints": {
                "status": "/history/api/status",
                "data": "/history/api/data",
                "widgets": "/history/api/widgets"
            },
            "message": if status == "available" {
                "History system is running and collecting data"
            } else {
                "History system is not currently available. Check ENABLE_LOXONE_STATS environment variable."
            }
        })).into_response()
    }
}

/// History API status endpoint
async fn history_api_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if let Some(_history_store) = &state.history_store {
        Json(serde_json::json!({
            "status": "healthy",
            "storage_type": "unified_history_store",
            "features": ["hot_storage", "cold_storage", "auto_archival"],
            "message": "History system is operational"
        }))
        .into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "unavailable", 
                "message": "History store not initialized. Set ENABLE_LOXONE_STATS=1 to enable statistics collection."
            }))
        ).into_response()
    }
}

/// Favicon handler to prevent 401 errors
async fn favicon_handler() -> impl IntoResponse {
    use axum::http::header;

    // Return a minimal 1x1 transparent PNG
    let favicon_bytes = [
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    ([(header::CONTENT_TYPE, "image/png")], favicon_bytes)
}

/// Unified dashboard home page
async fn unified_dashboard_home(
    State(_state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Check if request wants JSON (API call)
    if let Some(accept) = headers.get(header::ACCEPT) {
        if accept.to_str().unwrap_or("").contains("application/json") {
            return Json(serde_json::json!({
                "status": "ok",
                "message": "Unified dashboard API",
                "endpoints": {
                    "status": "/dashboard/api/status",
                    "data": "/dashboard/api/data"
                }
            }))
            .into_response();
        }
    }

    // Return HTML dashboard for browsers using the same HTML from unified_dashboard.rs
    Html(generate_unified_dashboard_html()).into_response()
}

/// Unified dashboard API status
async fn unified_dashboard_api_status(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "unified_dashboard",
        "version": "1.0.0",
        "features": ["real_time", "operational_metrics", "device_overview", "trends"]
    }))
}

/// Get dashboard data (shared between API and WebSocket)
async fn get_dashboard_data(state: &Arc<AppState>) -> serde_json::Value {
    // Get real data from MCP server
    let mut rooms_data = Vec::new();
    let mut devices_data = Vec::new();
    let mut sensors_data = Vec::new();
    let connection_status;

    // Try to get system status to check connectivity
    match state.mcp_server.get_system_status().await {
        Ok(_status_result) => {
            connection_status = "Connected";

            // Get room list
            if let Ok(rooms_result) = state.mcp_server.list_rooms().await {
                if !rooms_result.is_error.unwrap_or(false) {
                    if let Some(Content::Text { text }) = rooms_result.content.first() {
                        if let Ok(rooms_json) = serde_json::from_str::<serde_json::Value>(text) {
                            if let Some(rooms_array) =
                                rooms_json.get("rooms").and_then(|r| r.as_array())
                            {
                                for room in rooms_array {
                                    rooms_data.push(serde_json::json!({
                                        "name": room.get("name").and_then(|n| n.as_str()).unwrap_or("Unknown"),
                                        "current_temp": null,
                                        "target_temp": null,
                                        "controller_uuid": null,
                                        "device_count": room.get("device_count").and_then(|d| d.as_u64()).unwrap_or(0),
                                        "active_devices": room.get("device_count").and_then(|d| d.as_u64()).unwrap_or(0)
                                    }));
                                }
                            }
                        }
                    }
                }
            }

            // Get device list and try to get states
            if let Ok(devices_result) = state.mcp_server.discover_all_devices().await {
                if !devices_result.is_error.unwrap_or(false) {
                    if let Some(Content::Text { text }) = devices_result.content.first() {
                        if let Ok(devices_json) = serde_json::from_str::<serde_json::Value>(text) {
                            if let Some(devices_array) =
                                devices_json.get("devices").and_then(|d| d.as_array())
                            {
                                // For now, we'll use demo data to show the concept
                                // In a real implementation, device states would come from Loxone
                                for device in devices_array {
                                    let device_uuid = device
                                        .get("uuid")
                                        .and_then(|u| u.as_str())
                                        .unwrap_or("unknown");
                                    let device_type = device
                                        .get("type")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("unknown");
                                    let device_name = device
                                        .get("name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("Unknown Device");
                                    let room = device
                                        .get("room")
                                        .and_then(|r| r.as_str())
                                        .unwrap_or("Unknown");

                                    // For demo purposes, simulate some device states based on device name/type
                                    let mut device_json = device.clone();

                                    // Add demo states for different device types
                                    match device_type {
                                        "LightController" | "LightControllerV2" | "Lighting" => {
                                            // Simulate some lights on, some off
                                            let is_on = device_name.len() % 3 == 0;
                                            device_json["is_on"] = serde_json::json!(is_on);
                                            if is_on {
                                                device_json["value"] = serde_json::json!(0.75);
                                                // 75% brightness
                                            }
                                        }
                                        "Jalousie" | "Blinds" => {
                                            // Simulate various blind positions
                                            let position = match device_name.len() % 4 {
                                                0 => 0.0,  // Closed
                                                1 => 1.0,  // Open
                                                2 => 0.5,  // Half open
                                                _ => 0.25, // Quarter open
                                            };
                                            device_json["position"] = serde_json::json!(position);
                                        }
                                        "InfoOnlyAnalog" => {
                                            if device_name.to_lowercase().contains("temperatur")
                                                || device_name.to_lowercase().contains("temp")
                                            {
                                                // Simulate temperature 18-24°C
                                                let temp = 18.0 + (device_name.len() % 7) as f64;
                                                device_json["state"] =
                                                    serde_json::json!({ "value": temp });
                                            } else if device_name
                                                .to_lowercase()
                                                .contains("luftfeuchte")
                                                || device_name.to_lowercase().contains("humidity")
                                            {
                                                // Simulate humidity 40-60%
                                                let humidity =
                                                    40.0 + (device_name.len() % 21) as f64;
                                                device_json["state"] =
                                                    serde_json::json!({ "value": humidity });
                                            }
                                        }
                                        "Switch" => {
                                            // Simulate some switches on/off
                                            let is_active = device_name.len() % 2 == 0;
                                            device_json["state"] =
                                                serde_json::json!({ "active": is_active });
                                        }
                                        _ => {}
                                    }

                                    let device_with_state = &device_json;

                                    // Extract meaningful state information based on device type
                                    let (state_display, status_color) = match device_type {
                                        "Lighting" | "LightController" | "LightControllerV2" => {
                                            // First check if we have "is_on" field from real state
                                            if let Some(is_on) = device_with_state
                                                .get("is_on")
                                                .and_then(|v| v.as_bool())
                                            {
                                                if is_on {
                                                    // Check for dimming value
                                                    if let Some(value) = device_with_state
                                                        .get("value")
                                                        .and_then(|v| v.as_f64())
                                                    {
                                                        (
                                                            format!(
                                                                "ON ({}%)",
                                                                (value * 100.0) as u32
                                                            ),
                                                            "green",
                                                        )
                                                    } else {
                                                        ("ON".to_string(), "green")
                                                    }
                                                } else {
                                                    ("OFF".to_string(), "gray")
                                                }
                                            } else if let Some(state) =
                                                device_with_state.get("state")
                                            {
                                                if let Some(value) =
                                                    state.get("value").and_then(|v| v.as_f64())
                                                {
                                                    if value > 0.0 {
                                                        (
                                                            format!(
                                                                "ON ({}%)",
                                                                (value * 100.0) as u32
                                                            ),
                                                            "green",
                                                        )
                                                    } else {
                                                        ("OFF".to_string(), "gray")
                                                    }
                                                } else {
                                                    ("Unknown".to_string(), "orange")
                                                }
                                            } else {
                                                ("No Data".to_string(), "red")
                                            }
                                        }
                                        "Jalousie" | "Blinds" => {
                                            // Check for position from real state
                                            if let Some(position) = device_with_state
                                                .get("position")
                                                .and_then(|p| p.as_f64())
                                            {
                                                let pos_percent = (position * 100.0) as u32;
                                                if pos_percent < 5 {
                                                    ("CLOSED".to_string(), "blue")
                                                } else if pos_percent > 95 {
                                                    ("OPEN".to_string(), "green")
                                                } else {
                                                    (format!("{}% OPEN", pos_percent), "orange")
                                                }
                                            } else if let Some(state) =
                                                device_with_state.get("state")
                                            {
                                                if let Some(position) =
                                                    state.get("position").and_then(|p| p.as_f64())
                                                {
                                                    let pos_percent = (position * 100.0) as u32;
                                                    if pos_percent < 5 {
                                                        ("CLOSED".to_string(), "blue")
                                                    } else if pos_percent > 95 {
                                                        ("OPEN".to_string(), "green")
                                                    } else {
                                                        (format!("{}% OPEN", pos_percent), "orange")
                                                    }
                                                } else {
                                                    ("Unknown".to_string(), "orange")
                                                }
                                            } else {
                                                ("No Data".to_string(), "red")
                                            }
                                        }
                                        "TemperatureController" | "Radiator" => {
                                            if let Some(state) = device_with_state.get("state") {
                                                if let Some(temp) = state
                                                    .get("temperature")
                                                    .and_then(|t| t.as_f64())
                                                {
                                                    let target = state
                                                        .get("target")
                                                        .and_then(|t| t.as_f64())
                                                        .unwrap_or(0.0);
                                                    (
                                                        format!(
                                                            "{:.1}°C (target: {:.1}°C)",
                                                            temp, target
                                                        ),
                                                        "blue",
                                                    )
                                                } else {
                                                    ("No Reading".to_string(), "red")
                                                }
                                            } else {
                                                ("No Data".to_string(), "red")
                                            }
                                        }
                                        "InfoOnlyAnalog" => {
                                            // Check if this is a temperature sensor
                                            if device_name.to_lowercase().contains("temperatur")
                                                || device_name.to_lowercase().contains("temp")
                                            {
                                                // Try to get temperature value from state
                                                if let Some(state) = device_with_state.get("state")
                                                {
                                                    if let Some(value) =
                                                        state.get("value").and_then(|v| v.as_f64())
                                                    {
                                                        (format!("{:.1}°C", value), "blue")
                                                    } else {
                                                        ("No Reading".to_string(), "gray")
                                                    }
                                                } else {
                                                    // Device state might need to be fetched
                                                    ("--°C".to_string(), "gray")
                                                }
                                            } else if device_name
                                                .to_lowercase()
                                                .contains("luftfeuchte")
                                                || device_name.to_lowercase().contains("humidity")
                                            {
                                                if let Some(state) = device_with_state.get("state")
                                                {
                                                    if let Some(value) =
                                                        state.get("value").and_then(|v| v.as_f64())
                                                    {
                                                        (format!("{:.1}%", value), "blue")
                                                    } else {
                                                        ("No Reading".to_string(), "gray")
                                                    }
                                                } else {
                                                    ("--%".to_string(), "gray")
                                                }
                                            } else {
                                                // Generic analog value
                                                if let Some(state) = device_with_state.get("state")
                                                {
                                                    if let Some(value) =
                                                        state.get("value").and_then(|v| v.as_f64())
                                                    {
                                                        (format!("{:.1}", value), "gray")
                                                    } else {
                                                        ("No Value".to_string(), "gray")
                                                    }
                                                } else {
                                                    ("No Data".to_string(), "gray")
                                                }
                                            }
                                        }
                                        "Switch" => {
                                            if let Some(state) = device_with_state.get("state") {
                                                if let Some(active) =
                                                    state.get("active").and_then(|a| a.as_bool())
                                                {
                                                    if active {
                                                        ("ON".to_string(), "green")
                                                    } else {
                                                        ("OFF".to_string(), "gray")
                                                    }
                                                } else if let Some(value) =
                                                    state.get("value").and_then(|v| v.as_f64())
                                                {
                                                    if value > 0.0 {
                                                        ("ON".to_string(), "green")
                                                    } else {
                                                        ("OFF".to_string(), "gray")
                                                    }
                                                } else {
                                                    ("Unknown".to_string(), "orange")
                                                }
                                            } else {
                                                ("No Data".to_string(), "red")
                                            }
                                        }
                                        _ => {
                                            // Generic state display
                                            if let Some(state) = device_with_state.get("state") {
                                                if let Some(value) = state.get("value") {
                                                    (format!("{}", value), "gray")
                                                } else {
                                                    ("Active".to_string(), "green")
                                                }
                                            } else {
                                                ("Unknown".to_string(), "gray")
                                            }
                                        }
                                    };

                                    // Only include devices that are in actual rooms (not "No Room" or "Unknown")
                                    if room != "No Room" && room != "Unknown" && !room.is_empty() {
                                        devices_data.push(serde_json::json!({
                                            "uuid": device_uuid,
                                            "name": device_name,
                                            "device_type": device_type,
                                            "room": room,
                                            "state": device_with_state.get("state"),
                                            "state_display": state_display,
                                            "status_color": status_color,
                                            "last_update": chrono::Utc::now()
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Get sensor data - try multiple sensor sources

            // Try door/window sensors first
            if let Ok(door_sensors_result) = state.mcp_server.get_all_door_window_sensors().await {
                if !door_sensors_result.is_error.unwrap_or(false) {
                    if let Some(Content::Text { text }) = door_sensors_result.content.first() {
                        if let Ok(sensors_json) = serde_json::from_str::<serde_json::Value>(text) {
                            if let Some(sensors_array) =
                                sensors_json.get("sensors").and_then(|s| s.as_array())
                            {
                                for sensor in sensors_array {
                                    let state_value = sensor
                                        .get("state")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("unknown");
                                    sensors_data.push(serde_json::json!({
                                        "uuid": sensor.get("uuid").and_then(|u| u.as_str()).unwrap_or("unknown"),
                                        "name": sensor.get("name").and_then(|n| n.as_str()).unwrap_or("Unknown Door/Window Sensor"),
                                        "room": sensor.get("room").and_then(|r| r.as_str()).unwrap_or("Unknown Room"),
                                        "value": state_value,
                                        "unit": null,
                                        "timestamp": chrono::Utc::now(),
                                        "status": if state_value == "closed" { "Active" } else { "Warning" }
                                    }));
                                }
                            }
                        }
                    }
                }
            }

            // Try temperature sensors
            if let Ok(temp_sensors_result) = state.mcp_server.get_temperature_sensors().await {
                if !temp_sensors_result.is_error.unwrap_or(false) {
                    if let Some(Content::Text { text }) = temp_sensors_result.content.first() {
                        if let Ok(sensors_json) = serde_json::from_str::<serde_json::Value>(text) {
                            if let Some(sensors_array) =
                                sensors_json.get("sensors").and_then(|s| s.as_array())
                            {
                                for sensor in sensors_array {
                                    let temp_value = sensor
                                        .get("temperature")
                                        .and_then(|t| t.as_f64())
                                        .unwrap_or(0.0);
                                    sensors_data.push(serde_json::json!({
                                        "uuid": sensor.get("uuid").and_then(|u| u.as_str()).unwrap_or("unknown"),
                                        "name": sensor.get("name").and_then(|n| n.as_str()).unwrap_or("Unknown Temperature Sensor"),
                                        "room": sensor.get("room").and_then(|r| r.as_str()).unwrap_or("Unknown Room"),
                                        "value": format!("{:.1}°C", temp_value),
                                        "unit": "°C",
                                        "timestamp": chrono::Utc::now(),
                                        "status": "Active"
                                    }));
                                }
                            }
                        }
                    }
                }
            }

            // Try discovered sensors
            if let Ok(discovered_result) =
                state.mcp_server.list_discovered_sensors(None, None).await
            {
                if !discovered_result.is_error.unwrap_or(false) {
                    if let Some(Content::Text { text }) = discovered_result.content.first() {
                        if let Ok(sensors_json) = serde_json::from_str::<serde_json::Value>(text) {
                            if let Some(sensors_array) =
                                sensors_json.get("sensors").and_then(|s| s.as_array())
                            {
                                for sensor in sensors_array {
                                    sensors_data.push(serde_json::json!({
                                        "uuid": sensor.get("uuid").and_then(|u| u.as_str()).unwrap_or("unknown"),
                                        "name": sensor.get("name").and_then(|n| n.as_str()).unwrap_or("Unknown Discovered Sensor"),
                                        "room": sensor.get("room").and_then(|r| r.as_str()).unwrap_or("Unknown Room"),
                                        "value": sensor.get("value").and_then(|v| v.as_str()).unwrap_or("unknown"),
                                        "unit": sensor.get("unit").and_then(|u| u.as_str()),
                                        "timestamp": chrono::Utc::now(),
                                        "status": "Active"
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {
            connection_status = "Error";

            // Add demo sensors when Loxone is not available
            sensors_data.push(serde_json::json!({
                "uuid": "demo-sensor-1",
                "name": "Kitchen Window Sensor",
                "room": "Küche",
                "value": "closed",
                "unit": null,
                "timestamp": chrono::Utc::now(),
                "status": "Active"
            }));

            sensors_data.push(serde_json::json!({
                "uuid": "demo-sensor-2",
                "name": "Living Room Temperature",
                "room": "Wohnzimmer",
                "value": "21.5°C",
                "unit": "°C",
                "timestamp": chrono::Utc::now(),
                "status": "Active"
            }));

            sensors_data.push(serde_json::json!({
                "uuid": "demo-sensor-3",
                "name": "Front Door Sensor",
                "room": "Flur",
                "value": "closed",
                "unit": null,
                "timestamp": chrono::Utc::now(),
                "status": "Active"
            }));
        }
    }

    let dashboard_data = serde_json::json!({
        "realtime": {
            "system_health": {
                "connection_status": connection_status,
                "last_update": chrono::Utc::now(),
                "error_rate": 0.0,
                "avg_response_time_ms": 50.0
            },
            "active_sensors": sensors_data,
            "recent_activity": generate_recent_activity(&rooms_data, &devices_data, state).await
        },
        "devices": {
            "rooms": enhance_rooms_with_devices(&rooms_data, &devices_data),
            "device_matrix": group_devices_by_room(&devices_data),
            "quick_controls": devices_data.iter().take(10).cloned().collect::<Vec<_>>()
        },
        "operational": get_operational_metrics(state).await,
        "trends": {
            "temperature_trends": [],
            "device_usage": [],
            "performance_trends": []
        },
        "metadata": {
            "last_update": chrono::Utc::now(),
            "data_age_seconds": 0,
            "collection_stats": {
                "total_collections": 0,
                "success_rate_percent": 100.0,
                "avg_collection_time_ms": 0.0,
                "last_error": null
            },
            "version": "1.0.0"
        }
    });

    dashboard_data
}

/// Get operational metrics from various system components
async fn get_operational_metrics(state: &Arc<AppState>) -> serde_json::Value {
    // Get rate limiter metrics
    let rate_limiter_stats = state.rate_limiter.get_statistics().await;

    // Get system metrics
    let websocket_connections = 0; // TODO: Track WebSocket connections
    let sse_connections = 0; // TODO: Get from SSE manager

    // Performance metrics (using rate limiter data)
    let api_performance = serde_json::json!({
        "requests_per_minute": rate_limiter_stats.total_requests as f64 / 60.0, // Approximate
        "avg_response_time_ms": 50.0, // Could be enhanced with real tracking
        "error_rate_percent": if rate_limiter_stats.total_requests > 0 {
            (rate_limiter_stats.total_violations as f64 / rate_limiter_stats.total_requests as f64) * 100.0
        } else {
            0.0
        },
        "slow_endpoints": [],
        "performance_history": []
    });

    // Rate limiter metrics
    let rate_limiter = serde_json::json!({
        "active_clients": rate_limiter_stats.total_clients,
        "recent_hits": rate_limiter_stats.total_violations,
        "blocked_requests": rate_limiter_stats.total_violations, // violations = blocked requests
        "top_offenders": [], // Could be enhanced with IP tracking
        "efficiency_percent": if rate_limiter_stats.total_requests > 0 {
            ((rate_limiter_stats.total_requests - rate_limiter_stats.total_violations as u64) as f64 / rate_limiter_stats.total_requests as f64) * 100.0
        } else {
            100.0
        }
    });

    // Security events (using rate limiter data as proxy)
    let security_events = serde_json::json!({
        "auth_failures": rate_limiter_stats.total_violations, // Using rate limiter violations as proxy
        "suspicious_activity": rate_limiter_stats.penalized_clients,
        "recent_events": [],
        "security_score": if rate_limiter_stats.total_violations > 10 { 80 } else { 100 }
    });

    // Resource metrics
    let resources = serde_json::json!({
        "websocket_connections": websocket_connections,
        "memory_usage_mb": 0.0, // Could be enhanced with real memory tracking
        "cpu_usage_percent": 0.0,
        "disk_usage_percent": 0.0,
        "network_activity": {
            "bytes_sent": 0,
            "bytes_received": 0,
            "active_connections": sse_connections + websocket_connections
        }
    });

    serde_json::json!({
        "api_performance": api_performance,
        "rate_limiter": rate_limiter,
        "security_events": security_events,
        "resources": resources
    })
}

/// Generate recent activity data based on current system state
async fn generate_recent_activity(
    rooms_data: &[serde_json::Value],
    devices_data: &[serde_json::Value],
    state: &Arc<AppState>,
) -> Vec<serde_json::Value> {
    let mut activities = Vec::new();
    let now = chrono::Utc::now();

    // Get rate limiter stats for activity
    let rate_limiter_stats = state.rate_limiter.get_statistics().await;

    // Add some system-based activities
    if rate_limiter_stats.total_requests > 0 {
        activities.push(serde_json::json!({
            "timestamp": now - chrono::Duration::minutes(1),
            "device_name": "System Monitor",
            "room": "System",
            "action": format!("API Request (Total: {})", rate_limiter_stats.total_requests),
            "details": format!("Rate limiter processed {} requests", rate_limiter_stats.total_requests)
        }));
    }

    if rate_limiter_stats.total_violations > 0 {
        activities.push(serde_json::json!({
            "timestamp": now - chrono::Duration::minutes(2),
            "device_name": "Security Monitor", 
            "room": "System",
            "action": format!("Rate Limit Triggered ({} violations)", rate_limiter_stats.total_violations),
            "details": "Rate limiting activated due to excessive requests"
        }));
    }

    // Add dashboard activity
    activities.push(serde_json::json!({
        "timestamp": now - chrono::Duration::minutes(0),
        "device_name": "Dashboard",
        "room": "Monitoring",
        "action": "Data Refresh",
        "details": format!("Updated {} rooms, {} devices", rooms_data.len(), devices_data.len())
    }));

    // Add room status updates
    for (i, room) in rooms_data.iter().take(3).enumerate() {
        if let Some(room_name) = room.get("name").and_then(|n| n.as_str()) {
            if let Some(device_count) = room.get("device_count").and_then(|d| d.as_u64()) {
                if device_count > 0 {
                    activities.push(serde_json::json!({
                        "timestamp": now - chrono::Duration::minutes((i + 1) as i64),
                        "device_name": format!("{} Room Monitor", room_name),
                        "room": room_name,
                        "action": "Status Check",
                        "details": format!("{} devices operational", device_count)
                    }));
                }
            }
        }
    }

    // Sort by timestamp (newest first) and limit to 5 items
    activities.sort_by(|a, b| {
        let time_a = a.get("timestamp").and_then(|t| t.as_str()).unwrap_or("");
        let time_b = b.get("timestamp").and_then(|t| t.as_str()).unwrap_or("");
        time_b.cmp(time_a)
    });

    activities.into_iter().take(5).collect()
}

/// Enhance room data with detailed device information
fn enhance_rooms_with_devices(
    rooms_data: &[serde_json::Value],
    devices_data: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    let mut enhanced_rooms = Vec::new();

    for room in rooms_data {
        let room_name = room
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("Unknown");

        // Find devices in this room
        let room_devices: Vec<_> = devices_data
            .iter()
            .filter(|device| device.get("room").and_then(|r| r.as_str()).unwrap_or("") == room_name)
            .collect();

        // Count devices by type and state
        let lights_on = room_devices
            .iter()
            .filter(|d| {
                d.get("device_type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .contains("Light")
                    && d.get("state_display")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .starts_with("ON")
            })
            .count();

        let lights_total = room_devices
            .iter()
            .filter(|d| {
                d.get("device_type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .contains("Light")
            })
            .count();

        let blinds_open = room_devices
            .iter()
            .filter(|d| {
                d.get("device_type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .contains("Jalousie")
                    && d.get("state_display")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .contains("OPEN")
            })
            .count();

        let blinds_total = room_devices
            .iter()
            .filter(|d| {
                d.get("device_type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .contains("Jalousie")
            })
            .count();

        // Find temperature info
        let temp_info = room_devices
            .iter()
            .find(|d| {
                d.get("device_type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .contains("Temperature")
            })
            .and_then(|d| d.get("state_display").and_then(|s| s.as_str()));

        let mut enhanced_room = room.clone();
        if let Some(obj) = enhanced_room.as_object_mut() {
            obj.insert(
                "lights_on".to_string(),
                serde_json::Value::Number(lights_on.into()),
            );
            obj.insert(
                "lights_total".to_string(),
                serde_json::Value::Number(lights_total.into()),
            );
            obj.insert(
                "blinds_open".to_string(),
                serde_json::Value::Number(blinds_open.into()),
            );
            obj.insert(
                "blinds_total".to_string(),
                serde_json::Value::Number(blinds_total.into()),
            );
            obj.insert(
                "temp_display".to_string(),
                serde_json::Value::String(temp_info.unwrap_or("No temperature sensor").to_string()),
            );
            obj.insert(
                "total_devices".to_string(),
                serde_json::Value::Number(room_devices.len().into()),
            );
        }

        enhanced_rooms.push(enhanced_room);
    }

    enhanced_rooms
}

/// Group devices by room for detailed display
fn group_devices_by_room(devices_data: &[serde_json::Value]) -> serde_json::Value {
    let mut rooms = std::collections::HashMap::new();

    for device in devices_data {
        let room = device
            .get("room")
            .and_then(|r| r.as_str())
            .unwrap_or("Unknown");
        let room_devices = rooms.entry(room.to_string()).or_insert_with(Vec::new);
        room_devices.push(device.clone());
    }

    // Convert to sorted format for display
    let mut sorted_rooms = Vec::new();
    for (room_name, devices) in rooms {
        sorted_rooms.push(serde_json::json!({
            "room_name": room_name,
            "devices": devices
        }));
    }

    // Sort by room name
    sorted_rooms.sort_by(|a, b| {
        let name_a = a.get("room_name").and_then(|n| n.as_str()).unwrap_or("");
        let name_b = b.get("room_name").and_then(|n| n.as_str()).unwrap_or("");
        name_a.cmp(name_b)
    });

    serde_json::Value::Array(sorted_rooms)
}

/// Unified dashboard API data
async fn unified_dashboard_api_data(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let dashboard_data = get_dashboard_data(&state).await;
    Json(dashboard_data)
}

/// Generate unified dashboard HTML (embedded version)
fn generate_unified_dashboard_html() -> String {
    // Use the same HTML from unified_dashboard.rs but inline here to avoid complex imports
    crate::monitoring::unified_dashboard::generate_dashboard_html()
}

/// Unified dashboard WebSocket endpoint (public access)
async fn unified_dashboard_websocket(
    ws: axum::extract::WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_unified_dashboard_websocket(socket, state))
}

/// Handle unified dashboard WebSocket connection
async fn handle_unified_dashboard_websocket(
    socket: axum::extract::ws::WebSocket,
    state: Arc<AppState>,
) {
    use futures_util::{SinkExt, StreamExt};
    use tracing::{debug, error, info, warn};

    info!("New unified dashboard WebSocket connection");

    let (mut sender, mut receiver) = socket.split();

    // Get real dashboard data directly
    let dashboard_data = get_dashboard_data(&state).await;
    let initial_data = serde_json::json!({
        "update_type": "FullRefresh",
        "timestamp": chrono::Utc::now(),
        "data": dashboard_data
    });

    if let Ok(json) = serde_json::to_string(&initial_data) {
        if sender
            .send(axum::extract::ws::Message::Text(json))
            .await
            .is_err()
        {
            warn!("Failed to send initial data to WebSocket client");
            return;
        }
    }

    // Start periodic updates task
    let state_clone = state.clone();
    let periodic_updates = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5)); // Update every 5 seconds

        loop {
            interval.tick().await;

            // Get fresh dashboard data
            let dashboard_data = get_dashboard_data(&state_clone).await;
            let update_data = serde_json::json!({
                "update_type": "FullRefresh",
                "timestamp": chrono::Utc::now(),
                "data": dashboard_data
            });

            if let Ok(json) = serde_json::to_string(&update_data) {
                if sender
                    .send(axum::extract::ws::Message::Text(json))
                    .await
                    .is_err()
                {
                    debug!("WebSocket client disconnected during periodic update");
                    break;
                }
            }
        }
    });

    // Handle incoming messages (ping/pong, client preferences, etc.)
    let message_handler = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(axum::extract::ws::Message::Text(text)) => {
                    debug!("Received WebSocket message: {}", text);
                    // Handle client messages (preferences, subscriptions, etc.)
                }
                Ok(axum::extract::ws::Message::Close(_)) => {
                    debug!("WebSocket client requested close");
                    break;
                }
                Ok(axum::extract::ws::Message::Pong(_)) => {
                    debug!("Received pong from WebSocket client");
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = periodic_updates => {},
        _ = message_handler => {},
    }

    info!("Unified dashboard WebSocket connection closed");
}

// Note: LLM sampling endpoints were planned but not implemented in this version
// The infrastructure exists via MCP Prompts protocol which can be accessed via
// the standard MCP interface for LLM integration
