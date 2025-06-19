//! HTTP/SSE transport implementation for n8n MCP integration
//!
//! This module provides HTTP server capabilities with Server-Sent Events (SSE)
//! transport for the Model Context Protocol, making it compatible with n8n.

pub mod admin_api;
pub mod admin_keys_ui;
pub mod cache_api;
pub mod dashboard_api;
pub mod dashboard_data_unified;
pub mod dashboard_performance;
pub mod fast_dashboard;
pub mod navigation_new;
pub mod rate_limiting;
pub mod state_api;

use crate::error::{LoxoneError, Result};
use crate::performance::{
    middleware::PerformanceMiddleware, PerformanceConfig, PerformanceMonitor,
};
use crate::security::{middleware::SecurityMiddleware, SecurityConfig};
use crate::server::LoxoneMcpServer;
use crate::auth::AuthenticationManager;
use mcp_foundation::ServerHandler;
use rate_limiting::{EnhancedRateLimiter, RateLimitResult};

#[cfg(feature = "influxdb")]
use crate::monitoring::{
    dashboard::{dashboard_routes, DashboardState},
    influxdb::{InfluxConfig, InfluxManager},
    metrics::{MetricsCollector, RequestTiming},
};

// Removed history imports - module was unused

use axum::{
    extract::{Path, Query, Request, State},
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
// use tower::ServiceBuilder;
// use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, info, warn};


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
    auth_manager: Arc<AuthenticationManager>,
    /// Enhanced rate limiter
    rate_limiter: EnhancedRateLimiter,
    /// Security middleware
    security_middleware: Option<Arc<SecurityMiddleware>>,
    /// Performance middleware
    performance_middleware: Option<Arc<PerformanceMiddleware>>,
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
    pub async fn new(mcp_server: LoxoneMcpServer, config: HttpServerConfig) -> Result<Self> {

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

        // Initialize unified authentication manager
        let auth_manager = crate::auth::initialize_auth_system().await?;


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


        Ok(Self {
            mcp_server,
            auth_manager,
            rate_limiter: EnhancedRateLimiter::with_defaults(),
            security_middleware,
            performance_middleware,
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
        // History store removed - unused module

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
                    sse_manager,
        });

        // let cors = CorsLayer::new()
        //     .allow_origin(Any)
        //     .allow_methods(Any)
        //     .allow_headers(Any);

        // Public routes (no authentication required)
        let public_routes = Router::new()
            .route("/health", get(health_check))
            .route("/", get(root_handler))
            .route("/favicon.ico", get(favicon_handler))
            .route("/metrics", get(prometheus_metrics)) // Prometheus endpoint
            // History dashboard endpoints (public for web browser access)
            // Unified dashboard routes (public for web browser access)
            .route("/dashboard", get(unified_dashboard_home))
            .route("/dashboard/", get(unified_dashboard_home))
            .route("/dashboard/api/status", get(unified_dashboard_api_status))
            .route("/dashboard/api/data", get(unified_dashboard_api_data))
            .route("/dashboard/ws", get(unified_dashboard_websocket))
            // Server metrics test endpoint (public for debugging)
            .route("/dashboard/api/metrics", get(server_metrics_test))
            // High-performance dashboard endpoints for <100ms response times (disabled until tower deps are fixed)
            // .merge(fast_dashboard::create_fast_dashboard_router())
            ;

        // History-based dashboard removed - unused module

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
            // Admin navigation hub and key management UI
            .route("/admin", get(navigation_hub))
            .route("/admin/", get(navigation_hub))
            .route("/admin/keys", get(admin_keys_ui::api_keys_ui))
            // API key management endpoints
            .route("/admin/api/keys", get(|State(state): State<Arc<AppState>>| async move {
                admin_api::list_keys(State(state.auth_manager.clone())).await
            }))
            .route("/admin/api/keys", axum::routing::post(|State(state): State<Arc<AppState>>, Json(request): Json<admin_api::CreateKeyRequest>| async move {
                admin_api::create_key(State(state.auth_manager.clone()), Json(request)).await
            }))
            .route("/admin/api/keys/stats", get(|State(state): State<Arc<AppState>>| async move {
                admin_api::get_auth_stats(State(state.auth_manager.clone())).await
            }))
            .route("/admin/api/keys/:id", get(|State(state): State<Arc<AppState>>, Path(key_id): Path<String>| async move {
                admin_api::get_key(State(state.auth_manager.clone()), Path(key_id)).await
            }))
            .route("/admin/api/keys/:id", axum::routing::put(|State(state): State<Arc<AppState>>, Path(key_id): Path<String>, Json(request): Json<admin_api::UpdateKeyRequest>| async move {
                admin_api::update_key(State(state.auth_manager.clone()), Path(key_id), Json(request)).await
            }))
            .route("/admin/api/keys/:id", axum::routing::delete(|State(state): State<Arc<AppState>>, Path(key_id): Path<String>| async move {
                admin_api::delete_key(State(state.auth_manager.clone()), Path(key_id)).await
            }))
            .route("/admin/api/audit", get(|State(state): State<Arc<AppState>>| async move {
                admin_api::get_audit_events(State(state.auth_manager.clone())).await
            }))
            .layer(axum::middleware::from_fn_with_state(
                shared_state.clone(),
                auth_middleware_wrapper,
            ));

        // Create base app
        let mut app = Router::new().merge(public_routes).merge(protected_routes);

        // Add InfluxDB dashboard
        #[cfg(feature = "influxdb")]
        {
            let dashboard_state = DashboardState {
                metrics_collector: shared_state.metrics_collector.clone(),
                influx_manager: shared_state.influx_manager.clone(),
            };
            app = app.nest("/dashboard/influx", dashboard_routes(dashboard_state));
            info!("✅ Using InfluxDB dashboard at /dashboard/influx");
        }

        let app = app
            // .layer(ServiceBuilder::new().layer(cors).into_inner())
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

        info!(
            "🏠 Navigation Hub: http://localhost:{}/admin (with API key)",
            self.port
        );
        info!(
            "🔑 API key management UI: http://localhost:{}/admin/keys",
            self.port
        );
        info!(
            "🔑 API key management endpoints:",
        );
        info!(
            "   - GET    /admin/api/keys         - List all keys",
        );
        info!(
            "   - POST   /admin/api/keys         - Create new key",
        );
        info!(
            "   - GET    /admin/api/keys/:id     - Get specific key",
        );
        info!(
            "   - PUT    /admin/api/keys/:id     - Update key",
        );
        info!(
            "   - DELETE /admin/api/keys/:id     - Delete key",
        );
        info!(
            "   - GET    /admin/api/keys/stats   - Auth statistics",
        );
        info!(
            "   - GET    /admin/api/audit        - Audit log",
        );

        Ok(app)
    }
}

/// Shared application state
#[derive(Clone)]
struct AppState {
    mcp_server: LoxoneMcpServer,
    auth_manager: Arc<AuthenticationManager>,
    rate_limiter: EnhancedRateLimiter,
    #[cfg(feature = "influxdb")]
    metrics_collector: Arc<MetricsCollector>,
    #[cfg(feature = "influxdb")]
    influx_manager: Option<Arc<InfluxManager>>,
    sse_manager: Arc<SseConnectionManager>,
}

/// Main navigation hub handler
async fn navigation_hub() -> impl IntoResponse {
    Html(generate_navigation_html())
}

/// Generate the main navigation hub HTML
fn generate_navigation_html() -> String {
    // Use the new styled version
    crate::http_transport::navigation_new::generate_navigation_html()
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
            "live_metrics": "Server-sent events for real-time updates",
            "widget_system": "Dynamic widget generation and customization",
            "data_export": "JSON/CSV export capabilities"
        },
        "web_access": {
            "monitoring": "Open http://localhost:3001/dashboard/ in your web browser",
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

    // Record metrics in our server metrics collector (always enabled)
    {
        let duration = request_start.elapsed();
        let is_tool_call = method == "tools/call";

        // Record basic request
        state
            .mcp_server
            .get_metrics_collector()
            .record_request(duration, 0, 0)
            .await; // bytes not tracked yet

        // Record tool execution if this was a tool call
        if is_tool_call {
            state
                .mcp_server
                .get_metrics_collector()
                .record_tool_execution(method, duration)
                .await;
        }

        // Record MCP-specific metrics
        match method {
            "tools/call" => {
                state
                    .mcp_server
                    .get_metrics_collector()
                    .record_tool_execution(method, duration)
                    .await
            }
            "resources/read" => state
                .mcp_server
                .get_metrics_collector()
                .record_resource_access(),
            "prompts/get" => state.mcp_server.get_metrics_collector().record_prompt(),
            _ => {}
        }
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
            "expired_keys": auth_stats.expired_keys,
            "blocked_ips": auth_stats.currently_blocked_ips,
            "failed_attempts": auth_stats.total_failed_attempts
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
    // Use unified authentication system
    unified_auth_middleware(State(state.auth_manager.clone()), request, next).await
}

/// Unified authentication middleware using the new AuthenticationManager
async fn unified_auth_middleware(
    State(auth_manager): State<Arc<AuthenticationManager>>,
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    let headers = request.headers();
    let query_string = request.uri().query();

    // Authenticate the request using the unified system
    match auth_manager.authenticate_request(headers, query_string).await {
        crate::auth::models::AuthResult::Success(_) => {
            // Authentication successful, proceed with the request
            Ok(next.run(request).await)
        }
        crate::auth::models::AuthResult::Unauthorized { reason } => {
            warn!("Authentication failed: {}", reason);
            Err(StatusCode::UNAUTHORIZED)
        }
        crate::auth::models::AuthResult::Forbidden { reason } => {
            warn!("Access forbidden: {}", reason);
            Err(StatusCode::FORBIDDEN)
        }
        crate::auth::models::AuthResult::RateLimited { retry_after_seconds } => {
            warn!("Rate limited for {} seconds", retry_after_seconds);
            Err(StatusCode::TOO_MANY_REQUESTS)
        }
    }
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
    let start_time = std::time::Instant::now();

    // Use the unified dashboard data helper for clean, consistent sensor values
    use crate::http_transport::dashboard_data_unified::get_unified_dashboard_data;

    let result = get_unified_dashboard_data(&state.mcp_server).await;

    // Record metrics for all dashboard data requests
    let duration = start_time.elapsed();
    state
        .mcp_server
        .get_metrics_collector()
        .record_request(duration, 0, 0)
        .await;

    result
}

/// Unified dashboard API data
async fn unified_dashboard_api_data(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let start_time = std::time::Instant::now();

    let dashboard_data = get_dashboard_data(&state).await;

    // Record metrics for dashboard API requests
    let duration = start_time.elapsed();
    state
        .mcp_server
        .get_metrics_collector()
        .record_request(duration, 0, 0)
        .await;

    Json(dashboard_data)
}

/// Generate unified dashboard HTML (embedded version)
fn generate_unified_dashboard_html() -> String {
    // Use the new clean dashboard
    crate::monitoring::clean_dashboard::generate_clean_dashboard_html()
}

/// Unified dashboard WebSocket endpoint with authentication
async fn unified_dashboard_websocket(
    ws: axum::extract::WebSocketUpgrade,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
) -> std::result::Result<axum::response::Response, StatusCode> {
    // Check for API key authentication
    if let Some(api_key) = params.get("api_key") {
        debug!("WebSocket authentication attempt with API key: {}", &api_key[..8.min(api_key.len())]);
        
        // Use the unified authentication manager
        let auth_result = state.auth_manager.authenticate(
            api_key,
            "websocket_client",
        ).await;
        
        match auth_result {
            crate::auth::models::AuthResult::Success(auth_success) => {
                debug!("WebSocket authentication successful for key: {}", auth_success.key.id);
                return Ok(ws.on_upgrade(move |socket| handle_unified_dashboard_websocket(socket, state)));
            }
            crate::auth::models::AuthResult::Unauthorized { reason } => {
                warn!("WebSocket authentication failed: {}", reason);
            }
            crate::auth::models::AuthResult::Forbidden { reason } => {
                warn!("WebSocket authentication forbidden: {}", reason);
            }
            crate::auth::models::AuthResult::RateLimited { retry_after_seconds } => {
                warn!("WebSocket authentication rate limited for {} seconds", retry_after_seconds);
            }
        }
    } else {
        warn!("WebSocket connection attempted without API key");
    }
    
    // Authentication failed
    Err(StatusCode::UNAUTHORIZED)
}

/// Handle unified dashboard WebSocket connection
async fn handle_unified_dashboard_websocket(
    socket: axum::extract::ws::WebSocket,
    state: Arc<AppState>,
) {
    use futures_util::{SinkExt, StreamExt};
    use tracing::{debug, error, info, warn};

    info!("New unified dashboard WebSocket connection");

    // Track connection opening
    state.mcp_server.get_metrics_collector().connection_opened();

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

    // Track connection closing
    state.mcp_server.get_metrics_collector().connection_closed();

    info!("Unified dashboard WebSocket connection closed");
}

/// Server metrics test endpoint for debugging
async fn server_metrics_test(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let metrics = state.mcp_server.get_metrics_collector().get_metrics().await;
    Json(serde_json::json!({
        "server_metrics": metrics,
        "metrics_timestamp": chrono::Utc::now(),
        "debug_info": {
            "uptime_seconds": metrics.uptime.uptime_seconds,
            "total_requests": metrics.network.total_requests,
            "cpu_usage": metrics.performance.cpu_usage_percent,
            "memory_mb": metrics.performance.memory_usage_mb,
            "requests_per_minute": metrics.network.requests_per_minute,
            "tools_executed": metrics.mcp.tools_executed
        }
    }))
}

// Note: LLM sampling endpoints were planned but not implemented in this version
// The infrastructure exists via MCP Prompts protocol which can be accessed via
// the standard MCP interface for LLM integration
