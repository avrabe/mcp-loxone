//! Unified dashboard controller and routes
//!
//! This module provides the main dashboard implementation that replaces
//! the fragmented approach with a single comprehensive monitoring interface.

use crate::monitoring::unified_collector::{DashboardData, UnifiedDataCollector};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Unified dashboard controller
pub struct UnifiedDashboardController {
    /// Data collector service
    data_collector: Arc<UnifiedDataCollector>,

    /// WebSocket connection manager
    websocket_manager: Arc<RwLock<WebSocketManager>>,
}

/// WebSocket connection manager
#[derive(Default)]
struct WebSocketManager {
    /// Connected clients count
    client_count: u32,

    /// Connection statistics
    stats: WebSocketStats,
}

/// WebSocket statistics
#[derive(Default, Clone)]
pub struct WebSocketStats {
    total_connections: u64,
    active_connections: u32,
    messages_sent: u64,
    messages_failed: u64,
}

/// Query parameters for dashboard API
#[derive(Debug, Deserialize)]
pub struct DashboardQuery {
    /// Data sections to include
    sections: Option<String>,

    /// Data format (json, minimal)
    #[allow(dead_code)]
    format: Option<String>,

    /// Include metadata
    metadata: Option<bool>,
}

impl UnifiedDashboardController {
    /// Create new unified dashboard controller
    pub fn new(data_collector: Arc<UnifiedDataCollector>) -> Self {
        Self {
            data_collector,
            websocket_manager: Arc::new(RwLock::new(WebSocketManager::default())),
        }
    }

    /// Create dashboard router  
    pub fn router(self) -> Router {
        Router::new()
            .route("/", get(dashboard_home))
            .route("/api/status", get(dashboard_status))
            .route("/api/data", get(dashboard_data))
            .route("/api/data/:section", get(dashboard_section))
            .route("/ws", get(dashboard_websocket))
            .route("/health", get(dashboard_health))
            .with_state(Arc::new(self))
    }

    /// Get current dashboard data
    pub async fn get_data(&self, query: DashboardQuery) -> DashboardData {
        let mut data = self.data_collector.get_dashboard_data().await;

        // Filter sections if requested
        if let Some(sections) = query.sections {
            let requested: Vec<&str> = sections.split(',').collect();

            if !requested.contains(&"realtime") {
                data.realtime = Default::default();
            }
            if !requested.contains(&"devices") {
                data.devices = Default::default();
            }
            if !requested.contains(&"operational") {
                data.operational = Default::default();
            }
            if !requested.contains(&"trends") {
                data.trends = Default::default();
            }
        }

        // Remove metadata if not requested
        if query.metadata == Some(false) {
            data.metadata = Default::default();
        }

        data
    }

    /// Get WebSocket statistics
    pub async fn get_websocket_stats(&self) -> WebSocketStats {
        let manager = self.websocket_manager.read().await;
        manager.stats.clone()
    }
}

/// Main dashboard home page
async fn dashboard_home(
    State(controller): State<Arc<UnifiedDashboardController>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Check if request wants JSON (API call)
    if let Some(accept) = headers.get(header::ACCEPT) {
        if accept.to_str().unwrap_or("").contains("application/json") {
            let data = controller
                .get_data(DashboardQuery {
                    sections: None,
                    format: Some("json".to_string()),
                    metadata: Some(true),
                })
                .await;

            return Json(serde_json::json!({
                "status": "ok",
                "data": data,
                "endpoints": {
                    "websocket": "/dashboard/ws",
                    "api_data": "/dashboard/api/data",
                    "health": "/dashboard/health"
                }
            }))
            .into_response();
        }
    }

    // Return HTML dashboard for browsers
    Html(generate_dashboard_html()).into_response()
}

/// Dashboard status endpoint
async fn dashboard_status(
    State(controller): State<Arc<UnifiedDashboardController>>,
) -> impl IntoResponse {
    let data = controller
        .get_data(DashboardQuery {
            sections: Some("realtime".to_string()),
            format: Some("minimal".to_string()),
            metadata: Some(true),
        })
        .await;

    let websocket_stats = controller.get_websocket_stats().await;

    Json(serde_json::json!({
        "status": "ok",
        "system_health": data.realtime.system_health,
        "websocket_connections": websocket_stats.active_connections,
        "last_update": data.metadata.last_update,
        "data_age_seconds": data.metadata.data_age_seconds
    }))
}

/// Full dashboard data endpoint
async fn dashboard_data(
    State(controller): State<Arc<UnifiedDashboardController>>,
    Query(query): Query<DashboardQuery>,
) -> impl IntoResponse {
    let data = controller.get_data(query).await;
    Json(data)
}

/// Individual dashboard section endpoint
async fn dashboard_section(
    State(controller): State<Arc<UnifiedDashboardController>>,
    axum::extract::Path(section): axum::extract::Path<String>,
    Query(query): Query<DashboardQuery>,
) -> impl IntoResponse {
    let mut query = query;
    query.sections = Some(section.clone());

    let data = controller.get_data(query).await;

    let section_data = match section.as_str() {
        "realtime" => serde_json::to_value(&data.realtime).unwrap(),
        "devices" => serde_json::to_value(&data.devices).unwrap(),
        "operational" => serde_json::to_value(&data.operational).unwrap(),
        "trends" => serde_json::to_value(&data.trends).unwrap(),
        "metadata" => serde_json::to_value(&data.metadata).unwrap(),
        _ => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "error": "Unknown section",
                "available_sections": ["realtime", "devices", "operational", "trends", "metadata"]
            }))).into_response();
        }
    };

    Json(section_data).into_response()
}

/// Dashboard health check
async fn dashboard_health(
    State(controller): State<Arc<UnifiedDashboardController>>,
) -> impl IntoResponse {
    let data = controller
        .get_data(DashboardQuery {
            sections: Some("realtime".to_string()),
            format: Some("minimal".to_string()),
            metadata: Some(false),
        })
        .await;

    let is_healthy = matches!(
        data.realtime.system_health.connection_status,
        crate::monitoring::unified_collector::ConnectionStatus::Connected
    );

    let status_code = if is_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(serde_json::json!({
            "status": if is_healthy { "healthy" } else { "unhealthy" },
            "connection_status": data.realtime.system_health.connection_status,
            "last_update": data.realtime.system_health.last_update,
            "error_rate": data.realtime.system_health.error_rate
        })),
    )
}

/// Dashboard WebSocket endpoint
async fn dashboard_websocket(
    ws: WebSocketUpgrade,
    State(controller): State<Arc<UnifiedDashboardController>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_websocket(socket, controller))
}

/// Handle WebSocket connection
async fn handle_websocket(socket: WebSocket, controller: Arc<UnifiedDashboardController>) {
    info!("New dashboard WebSocket connection");

    // Update connection stats
    {
        let mut manager = controller.websocket_manager.write().await;
        manager.client_count += 1;
        manager.stats.total_connections += 1;
        manager.stats.active_connections += 1;
    }

    let (mut sender, mut receiver) = socket.split();

    // Subscribe to real-time updates
    let mut update_rx = controller.data_collector.subscribe_updates();

    // Send initial data
    let initial_data = controller.data_collector.get_dashboard_data().await;
    if let Ok(json) = serde_json::to_string(&initial_data) {
        if sender.send(Message::Text(json)).await.is_err() {
            warn!("Failed to send initial data to WebSocket client");
            return;
        }
    }

    // Handle messages
    let controller_clone = controller.clone();
    let update_task = tokio::spawn(async move {
        while let Ok(update) = update_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&update) {
                if sender.send(Message::Text(json)).await.is_err() {
                    debug!("WebSocket client disconnected");
                    break;
                }

                let mut manager = controller_clone.websocket_manager.write().await;
                manager.stats.messages_sent += 1;
            } else {
                let mut manager = controller_clone.websocket_manager.write().await;
                manager.stats.messages_failed += 1;
            }
        }
    });

    // Handle incoming messages (ping/pong, client preferences, etc.)
    let ping_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    debug!("Received WebSocket message: {}", text);
                    // Handle client messages (preferences, subscriptions, etc.)
                }
                Ok(Message::Close(_)) => {
                    debug!("WebSocket client requested close");
                    break;
                }
                Ok(Message::Pong(_)) => {
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
        _ = update_task => {},
        _ = ping_task => {},
    }

    // Update connection stats
    {
        let mut manager = controller.websocket_manager.write().await;
        manager.client_count = manager.client_count.saturating_sub(1);
        manager.stats.active_connections = manager.stats.active_connections.saturating_sub(1);
    }

    info!("Dashboard WebSocket connection closed");
}

/// Generate the main dashboard HTML
pub fn generate_dashboard_html() -> String {
    // Use the new clean dashboard
    crate::monitoring::clean_dashboard::generate_clean_dashboard_html()
}
