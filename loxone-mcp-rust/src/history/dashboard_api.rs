//! REST API endpoints for the dynamic dashboard

use super::core::UnifiedHistoryStore;
use super::dynamic_dashboard::{DynamicDashboard, DynamicDashboardConfig, DynamicDashboardLayout};
use crate::client::ClientContext;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Dashboard API state
#[derive(Clone)]
pub struct DashboardApiState {
    dashboard: Arc<DynamicDashboard>,
    config: Arc<RwLock<DynamicDashboardConfig>>,
    layouts_cache: Arc<RwLock<HashMap<String, CachedLayout>>>,
}

/// Cached dashboard layout
#[derive(Debug, Clone)]
struct CachedLayout {
    layout: DynamicDashboardLayout,
    created_at: chrono::DateTime<chrono::Utc>,
    cache_key: String,
}

/// Query parameters for dashboard requests
#[derive(Debug, Deserialize, Serialize)]
struct DashboardQuery {
    /// Filter by room
    room: Option<String>,
    /// Filter by device type
    device_type: Option<String>,
    /// Filter by sensor type
    sensor_type: Option<String>,
    /// Time range (1h, 6h, 24h, 7d)
    time_range: Option<String>,
    /// Force refresh (bypass cache)
    refresh: Option<bool>,
    /// Custom configuration
    config: Option<String>, // JSON string
}

/// Dashboard API error response
#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
    code: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl DashboardApiState {
    /// Create new dashboard API state
    pub fn new(
        history_store: Arc<UnifiedHistoryStore>,
        client_context: Arc<ClientContext>,
    ) -> Self {
        let dashboard = Arc::new(DynamicDashboard::new(history_store, client_context));

        Self {
            dashboard,
            config: Arc::new(RwLock::new(DynamicDashboardConfig::default())),
            layouts_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate cache key from query parameters
    fn generate_cache_key(query: &DashboardQuery) -> String {
        format!(
            "dashboard:{}:{}:{}:{}",
            query.room.as_deref().unwrap_or("all"),
            query.device_type.as_deref().unwrap_or("all"),
            query.sensor_type.as_deref().unwrap_or("all"),
            query.time_range.as_deref().unwrap_or("24h")
        )
    }

    /// Check if cached layout is still valid
    fn is_cache_valid(cached: &CachedLayout, config: &DynamicDashboardConfig) -> bool {
        let age = chrono::Utc::now() - cached.created_at;
        age.num_seconds() < config.discovery_interval_seconds as i64
    }
}

/// Create dashboard API router
pub fn create_dashboard_router(
    history_store: Arc<UnifiedHistoryStore>,
    client_context: Arc<ClientContext>,
) -> Router {
    let state = DashboardApiState::new(history_store, client_context);

    Router::new()
        .route("/", get(get_dashboard_layout))
        .route("/widgets", get(list_available_widgets))
        .route("/widgets/:widget_id", get(get_widget_data))
        .route("/config", get(get_dashboard_config))
        .route("/config", axum::routing::post(update_dashboard_config))
        .route("/discovery", axum::routing::post(trigger_discovery))
        .route("/filters", get(get_available_filters))
        .route("/export", get(export_dashboard_data))
        .with_state(state)
}

/// Get dashboard layout
async fn get_dashboard_layout(
    State(state): State<DashboardApiState>,
    Query(query): Query<DashboardQuery>,
) -> std::result::Result<Json<DynamicDashboardLayout>, (StatusCode, Json<ApiError>)> {
    debug!("Dashboard layout request: {:?}", query);

    let cache_key = DashboardApiState::generate_cache_key(&query);
    let config = state.config.read().await.clone();

    // Check cache unless refresh is forced
    if !query.refresh.unwrap_or(false) {
        let cache = state.layouts_cache.read().await;
        if let Some(cached) = cache.get(&cache_key) {
            if DashboardApiState::is_cache_valid(cached, &config) {
                debug!("Returning cached dashboard layout");
                return Ok(Json(cached.layout.clone()));
            }
        }
    }

    // Apply custom config if provided
    let mut effective_config = config;
    if let Some(config_json) = query.config.clone() {
        match serde_json::from_str::<DynamicDashboardConfig>(&config_json) {
            Ok(custom_config) => {
                debug!("Applying custom dashboard config");
                effective_config = custom_config;
            }
            Err(e) => {
                warn!("Invalid custom config JSON: {}", e);
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ApiError {
                        error: format!("Invalid config JSON: {}", e),
                        code: "INVALID_CONFIG".to_string(),
                        timestamp: chrono::Utc::now(),
                    }),
                ));
            }
        }
    }

    // Generate new layout
    match state
        .dashboard
        .generate_dashboard_layout(&effective_config)
        .await
    {
        Ok(mut layout) => {
            // Apply filters from query
            layout = apply_query_filters(layout, &query).await;

            // Cache the result
            let cached = CachedLayout {
                layout: layout.clone(),
                created_at: chrono::Utc::now(),
                cache_key: cache_key.clone(),
            };
            state.layouts_cache.write().await.insert(cache_key, cached);

            Ok(Json(layout))
        }
        Err(e) => {
            warn!("Failed to generate dashboard layout: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: format!("Failed to generate dashboard: {}", e),
                    code: "GENERATION_FAILED".to_string(),
                    timestamp: chrono::Utc::now(),
                }),
            ))
        }
    }
}

/// Apply query filters to dashboard layout
async fn apply_query_filters(
    mut layout: DynamicDashboardLayout,
    query: &DashboardQuery,
) -> DynamicDashboardLayout {
    // Filter widgets based on query parameters
    if let Some(ref room_filter) = query.room {
        layout.widgets.retain(|widget| {
            // Check if widget data contains room information
            if let Ok(data) = serde_json::from_value::<serde_json::Value>(widget.data.clone()) {
                if let Some(obj) = data.as_object() {
                    for (_, value) in obj {
                        if let Some(room) = value.get("room").and_then(|r| r.as_str()) {
                            if room == room_filter {
                                return true;
                            }
                        }
                    }
                }
            }
            false
        });
    }

    // Add filter information to layout
    if query.room.is_some() || query.device_type.is_some() || query.sensor_type.is_some() {
        // Add applied filters info (this would be used by the frontend)
        // For now, we just keep the layout as-is
    }

    layout
}

/// List available widget types
async fn list_available_widgets(
    State(state): State<DashboardApiState>,
) -> std::result::Result<Json<Vec<WidgetInfo>>, (StatusCode, Json<ApiError>)> {
    // Trigger discovery to get current data
    if let Err(e) = state.dashboard.discover_data_sources().await {
        warn!("Discovery failed: {}", e);
    }

    let widget_types = vec![
        WidgetInfo {
            id: "rooms_overview".to_string(),
            name: "Rooms Overview".to_string(),
            description: "Grid view of all rooms with device counts".to_string(),
            category: "overview".to_string(),
            supported_sizes: vec!["medium".to_string(), "large".to_string()],
        },
        WidgetInfo {
            id: "active_devices".to_string(),
            name: "Active Devices".to_string(),
            description: "List of recently active devices".to_string(),
            category: "devices".to_string(),
            supported_sizes: vec!["medium".to_string(), "large".to_string()],
        },
        WidgetInfo {
            id: "temperature_chart".to_string(),
            name: "Temperature Sensors".to_string(),
            description: "Chart showing temperature readings over time".to_string(),
            category: "sensors".to_string(),
            supported_sizes: vec![
                "medium".to_string(),
                "large".to_string(),
                "wide".to_string(),
            ],
        },
        WidgetInfo {
            id: "door_window_status".to_string(),
            name: "Door/Window Status".to_string(),
            description: "Current state of doors and windows".to_string(),
            category: "sensors".to_string(),
            supported_sizes: vec!["small".to_string(), "medium".to_string()],
        },
        WidgetInfo {
            id: "system_metrics".to_string(),
            name: "System Metrics".to_string(),
            description: "System health and performance metrics".to_string(),
            category: "system".to_string(),
            supported_sizes: vec!["medium".to_string(), "large".to_string()],
        },
        WidgetInfo {
            id: "activity_timeline".to_string(),
            name: "Activity Timeline".to_string(),
            description: "Recent system activity and events".to_string(),
            category: "activity".to_string(),
            supported_sizes: vec!["large".to_string(), "wide".to_string()],
        },
    ];

    Ok(Json(widget_types))
}

/// Get specific widget data
async fn get_widget_data(
    State(state): State<DashboardApiState>,
    Path(widget_id): Path<String>,
    Query(_query): Query<DashboardQuery>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    debug!("Widget data request for: {}", widget_id);

    let config = state.config.read().await.clone();

    match state.dashboard.generate_dashboard_layout(&config).await {
        Ok(layout) => {
            if let Some(widget) = layout.widgets.iter().find(|w| w.id == widget_id) {
                Ok(Json(widget.data.clone()))
            } else {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(ApiError {
                        error: format!("Widget '{}' not found", widget_id),
                        code: "WIDGET_NOT_FOUND".to_string(),
                        timestamp: chrono::Utc::now(),
                    }),
                ))
            }
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Failed to generate widget data: {}", e),
                code: "WIDGET_GENERATION_FAILED".to_string(),
                timestamp: chrono::Utc::now(),
            }),
        )),
    }
}

/// Get dashboard configuration
async fn get_dashboard_config(
    State(state): State<DashboardApiState>,
) -> Json<DynamicDashboardConfig> {
    let config = state.config.read().await.clone();
    Json(config)
}

/// Update dashboard configuration
async fn update_dashboard_config(
    State(state): State<DashboardApiState>,
    Json(new_config): Json<DynamicDashboardConfig>,
) -> std::result::Result<Json<DynamicDashboardConfig>, (StatusCode, Json<ApiError>)> {
    debug!("Updating dashboard configuration");

    // Clear cache when config changes
    state.layouts_cache.write().await.clear();

    // Update config
    *state.config.write().await = new_config.clone();

    Ok(Json(new_config))
}

/// Trigger data source discovery
async fn trigger_discovery(
    State(state): State<DashboardApiState>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    debug!("Triggering manual data source discovery");

    match state.dashboard.discover_data_sources().await {
        Ok(()) => {
            // Clear cache after discovery
            state.layouts_cache.write().await.clear();

            Ok(Json(serde_json::json!({
                "status": "success",
                "message": "Data source discovery completed",
                "timestamp": chrono::Utc::now()
            })))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Discovery failed: {}", e),
                code: "DISCOVERY_FAILED".to_string(),
                timestamp: chrono::Utc::now(),
            }),
        )),
    }
}

/// Get available filters
async fn get_available_filters(
    State(state): State<DashboardApiState>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let config = state.config.read().await.clone();

    match state.dashboard.generate_dashboard_layout(&config).await {
        Ok(layout) => Ok(Json(serde_json::json!({
            "filters": layout.available_filters,
            "discovery_info": layout.discovery_info
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Failed to get filters: {}", e),
                code: "FILTERS_FAILED".to_string(),
                timestamp: chrono::Utc::now(),
            }),
        )),
    }
}

/// Export dashboard data
async fn export_dashboard_data(
    State(state): State<DashboardApiState>,
    Query(query): Query<DashboardQuery>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    debug!("Dashboard data export request");

    let config = state.config.read().await.clone();

    match state.dashboard.generate_dashboard_layout(&config).await {
        Ok(layout) => {
            // Create export data with metadata
            let export_data = serde_json::json!({
                "export_info": {
                    "generated_at": chrono::Utc::now(),
                    "query_params": query,
                    "total_widgets": layout.widgets.len()
                },
                "layout": layout,
                "format_version": "1.0"
            });

            Ok(Json(export_data))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Export failed: {}", e),
                code: "EXPORT_FAILED".to_string(),
                timestamp: chrono::Utc::now(),
            }),
        )),
    }
}

/// Widget information for API responses
#[derive(Debug, Serialize)]
struct WidgetInfo {
    id: String,
    name: String,
    description: String,
    category: String,
    supported_sizes: Vec<String>,
}
