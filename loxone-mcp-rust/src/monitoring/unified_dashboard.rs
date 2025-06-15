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
#[derive(Default)]
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
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Loxone MCP Dashboard</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #f5f5f7;
            color: #1d1d1f;
            line-height: 1.5;
        }
        
        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
        }
        
        .header {
            background: white;
            border-radius: 12px;
            padding: 24px;
            margin-bottom: 24px;
            box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1);
        }
        
        .header h1 {
            font-size: 28px;
            font-weight: 600;
            margin-bottom: 8px;
        }
        
        .header .subtitle {
            color: #6e6e73;
            font-size: 16px;
        }
        
        .status-bar {
            display: flex;
            gap: 16px;
            margin-bottom: 24px;
            flex-wrap: wrap;
        }
        
        .status-card {
            background: white;
            border-radius: 8px;
            padding: 16px;
            flex: 1;
            min-width: 200px;
            box-shadow: 0 1px 5px rgba(0, 0, 0, 0.1);
        }
        
        .status-card.connected { border-left: 4px solid #30d158; }
        .status-card.warning { border-left: 4px solid #ff9f0a; }
        .status-card.error { border-left: 4px solid #ff3b30; }
        
        .status-title {
            font-size: 14px;
            font-weight: 500;
            color: #6e6e73;
            margin-bottom: 4px;
        }
        
        .status-value {
            font-size: 18px;
            font-weight: 600;
        }
        
        .dashboard-grid {
            display: grid;
            grid-template-columns: 1fr;
            gap: 24px;
            margin-bottom: 24px;
        }
        
        @media (min-width: 1200px) {
            .dashboard-grid {
                grid-template-columns: 1fr 1fr;
                grid-template-areas: 
                    "devices devices"
                    "realtime operational"
                    "trends trends";
            }
            
            .dashboard-section:nth-child(1) { grid-area: realtime; }
            .dashboard-section:nth-child(2) { grid-area: devices; }
            .dashboard-section:nth-child(3) { grid-area: operational; }
            .dashboard-section:nth-child(4) { grid-area: trends; }
        }
        
        .dashboard-section {
            background: white;
            border-radius: 12px;
            padding: 24px;
            box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1);
        }
        
        .section-title {
            font-size: 20px;
            font-weight: 600;
            margin-bottom: 16px;
            display: flex;
            align-items: center;
            gap: 8px;
        }
        
        .section-title .icon {
            width: 24px;
            height: 24px;
            border-radius: 6px;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 14px;
        }
        
        .icon.realtime { background: #30d158; color: white; }
        .icon.devices { background: #007aff; color: white; }
        .icon.operational { background: #ff9f0a; color: white; }
        .icon.trends { background: #af52de; color: white; }
        
        .loading {
            text-align: center;
            padding: 40px;
            color: #6e6e73;
        }
        
        .loading::after {
            content: '';
            display: inline-block;
            width: 20px;
            height: 20px;
            border: 2px solid #e5e5e7;
            border-top: 2px solid #007aff;
            border-radius: 50%;
            animation: spin 1s linear infinite;
            margin-left: 8px;
        }
        
        @keyframes spin {
            to { transform: rotate(360deg); }
        }
        
        .error-message {
            background: #fff2f2;
            border: 1px solid #fecaca;
            border-radius: 8px;
            padding: 16px;
            color: #dc2626;
            margin: 16px 0;
        }
        
        .device-grid {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
            gap: 12px;
            margin-top: 16px;
        }
        
        .device-card {
            padding: 12px;
            border-radius: 8px;
            border: 1px solid #e5e5e7;
            text-align: center;
            transition: all 0.2s;
        }
        
        .device-card:hover {
            border-color: #007aff;
            transform: translateY(-2px);
        }
        
        .device-card.active { background: #e8f5e8; border-color: #30d158; }
        .device-card.inactive { background: #f8f8f8; border-color: #d1d1d6; }
        
        .device-name {
            font-size: 12px;
            font-weight: 500;
            margin-bottom: 4px;
        }
        
        .device-status {
            font-size: 10px;
            color: #6e6e73;
        }
        
        .websocket-status {
            position: fixed;
            top: 20px;
            right: 20px;
            background: white;
            border-radius: 20px;
            padding: 8px 16px;
            box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1);
            font-size: 12px;
            font-weight: 500;
            z-index: 1000;
        }
        
        .websocket-status.connected { border-left: 3px solid #30d158; }
        .websocket-status.disconnected { border-left: 3px solid #ff3b30; }
        
        @media (max-width: 768px) {
            .container { padding: 16px; }
            .dashboard-grid { grid-template-columns: 1fr; }
            .status-bar { flex-direction: column; }
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Loxone MCP Dashboard</h1>
            <div class="subtitle">Unified monitoring and control interface</div>
        </div>
        
        <div class="websocket-status" id="wsStatus">
            Connecting...
        </div>
        
        <div class="status-bar" id="statusBar">
            <div class="loading">Loading system status...</div>
        </div>
        
        <div class="dashboard-grid">
            <div class="dashboard-section">
                <div class="section-title">
                    <div class="icon realtime">🔴</div>
                    Real-time Monitoring
                </div>
                <div id="realtimeContent" class="loading">Loading real-time data...</div>
            </div>
            
            <div class="dashboard-section">
                <div class="section-title">
                    <div class="icon devices">🏠</div>
                    Device & Room Overview
                </div>
                <div id="devicesContent" class="loading">Loading device data...</div>
            </div>
            
            <div class="dashboard-section">
                <div class="section-title">
                    <div class="icon operational">⚙️</div>
                    Operational Metrics
                </div>
                <div id="operationalContent" class="loading">Loading operational data...</div>
            </div>
            
            <div class="dashboard-section">
                <div class="section-title">
                    <div class="icon trends">📊</div>
                    Historical Trends
                </div>
                <div id="trendsContent" class="loading">Loading trend data...</div>
            </div>
        </div>
    </div>
    
    <script>
        class DashboardApp {
            constructor() {
                this.ws = null;
                this.reconnectInterval = 5000;
                this.maxReconnectAttempts = 10;
                this.reconnectAttempts = 0;
                this.init();
            }
            
            init() {
                this.loadInitialData();
                this.connectWebSocket();
                
                // Reload page if it's been open for more than 1 hour
                setTimeout(() => location.reload(), 3600000);
            }
            
            async loadInitialData() {
                try {
                    const response = await fetch('/dashboard/api/data');
                    if (!response.ok) throw new Error(`HTTP ${response.status}`);
                    
                    const data = await response.json();
                    this.updateDashboard(data);
                } catch (error) {
                    console.error('Failed to load initial data:', error);
                    this.showError('Failed to load dashboard data');
                }
            }
            
            connectWebSocket() {
                const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
                const wsUrl = `${protocol}//${window.location.host}/dashboard/ws`;
                
                this.ws = new WebSocket(wsUrl);
                
                this.ws.onopen = () => {
                    console.log('WebSocket connected');
                    this.updateWebSocketStatus('connected');
                    this.reconnectAttempts = 0;
                };
                
                this.ws.onmessage = (event) => {
                    try {
                        const data = JSON.parse(event.data);
                        if (data.update_type === 'FullRefresh') {
                            this.updateDashboard(data.data);
                        } else {
                            this.handleUpdate(data);
                        }
                    } catch (error) {
                        console.error('Failed to parse WebSocket message:', error);
                    }
                };
                
                this.ws.onclose = () => {
                    console.log('WebSocket disconnected');
                    this.updateWebSocketStatus('disconnected');
                    this.scheduleReconnect();
                };
                
                this.ws.onerror = (error) => {
                    console.error('WebSocket error:', error);
                    this.updateWebSocketStatus('error');
                };
            }
            
            scheduleReconnect() {
                if (this.reconnectAttempts < this.maxReconnectAttempts) {
                    setTimeout(() => {
                        console.log(`Reconnecting WebSocket (attempt ${this.reconnectAttempts + 1})`);
                        this.reconnectAttempts++;
                        this.connectWebSocket();
                    }, this.reconnectInterval);
                }
            }
            
            updateWebSocketStatus(status) {
                const statusEl = document.getElementById('wsStatus');
                statusEl.className = `websocket-status ${status}`;
                
                switch (status) {
                    case 'connected':
                        statusEl.textContent = '🟢 Live Updates';
                        break;
                    case 'disconnected':
                        statusEl.textContent = '🔴 Disconnected';
                        break;
                    case 'error':
                        statusEl.textContent = '⚠️ Connection Error';
                        break;
                }
            }
            
            updateDashboard(data) {
                this.updateStatusBar(data.realtime?.system_health);
                this.updateRealtimeSection(data.realtime);
                this.updateDevicesSection(data.devices);
                this.updateOperationalSection(data.operational);
                this.updateTrendsSection(data.trends);
            }
            
            updateStatusBar(systemHealth) {
                const statusBar = document.getElementById('statusBar');
                
                if (!systemHealth) {
                    statusBar.innerHTML = '<div class="loading">Loading status...</div>';
                    return;
                }
                
                const connectionClass = this.getConnectionStatusClass(systemHealth.connection_status);
                
                statusBar.innerHTML = `
                    <div class="status-card ${connectionClass}">
                        <div class="status-title">Connection</div>
                        <div class="status-value">${this.formatConnectionStatus(systemHealth.connection_status)}</div>
                    </div>
                    <div class="status-card">
                        <div class="status-title">Last Update</div>
                        <div class="status-value">${this.formatTime(systemHealth.last_update)}</div>
                    </div>
                    <div class="status-card">
                        <div class="status-title">Error Rate</div>
                        <div class="status-value">${systemHealth.error_rate.toFixed(1)}/min</div>
                    </div>
                    <div class="status-card">
                        <div class="status-title">Response Time</div>
                        <div class="status-value">${systemHealth.avg_response_time_ms.toFixed(0)}ms</div>
                    </div>
                `;
            }
            
            updateRealtimeSection(realtime) {
                const content = document.getElementById('realtimeContent');
                
                if (!realtime) {
                    content.innerHTML = '<div class="error-message">No real-time data available</div>';
                    return;
                }
                
                content.innerHTML = `
                    <div style="margin-bottom: 16px;">
                        <strong>Active Sensors:</strong> ${realtime.active_sensors?.length || 0}
                    </div>
                    <div style="margin-bottom: 16px;">
                        <strong>Recent Activity:</strong>
                        ${realtime.recent_activity?.length ? 
                            realtime.recent_activity.slice(0, 5).map(activity => 
                                `<div style="font-size: 12px; margin: 4px 0;">${activity.device_name} - ${activity.action}</div>`
                            ).join('') : 
                            '<div style="font-size: 12px; color: #6e6e73;">No recent activity</div>'
                        }
                    </div>
                `;
            }
            
            updateDevicesSection(devices) {
                const content = document.getElementById('devicesContent');
                
                if (!devices) {
                    content.innerHTML = '<div class="error-message">No device data available</div>';
                    return;
                }
                
                // Create integrated room cards with live device data
                const roomsHtml = devices.device_matrix?.map(roomGroup => {
                    // Get room summary from devices.rooms if available
                    const roomSummary = devices.rooms?.find(r => r.name === roomGroup.room_name) || {};
                    
                    // Group devices by type for better display
                    const devicesByType = {};
                    roomGroup.devices.forEach(device => {
                        const type = this.getDeviceCategory(device.device_type);
                        if (!devicesByType[type]) devicesByType[type] = [];
                        devicesByType[type].push(device);
                    });
                    
                    // Calculate room status
                    const hasActiveDevices = roomGroup.devices.some(d => 
                        d.status_color === 'green' || d.status_color === 'blue'
                    );
                    const borderColor = hasActiveDevices ? '#30d158' : '#e5e5e7';
                    
                    // Temperature display
                    const tempDevice = roomGroup.devices.find(d => 
                        d.device_type.includes('Temperature') || d.device_type.includes('InfoOnlyAnalog')
                    );
                    const tempDisplay = roomSummary.temp_display || 
                        (tempDevice && tempDevice.state ? `${tempDevice.state}°C` : '');
                    
                    return `
                        <div class="room-card" style="margin-bottom: 16px; background: white; border-radius: 12px; box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1); border-left: 4px solid ${borderColor}; overflow: hidden;">
                            <div style="padding: 16px;">
                                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;">
                                    <h3 style="margin: 0; font-size: 18px; font-weight: 600;">${roomGroup.room_name}</h3>
                                    ${tempDisplay ? `<span style="font-size: 20px; color: #007aff; font-weight: 500;">${tempDisplay}</span>` : ''}
                                </div>
                                
                                ${Object.entries(devicesByType).map(([type, devices]) => `
                                    <div style="margin-bottom: 12px;">
                                        <div style="font-size: 13px; font-weight: 500; color: #6e6e73; margin-bottom: 6px;">
                                            ${this.getTypeIcon(type)} ${type}
                                        </div>
                                        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 8px;">
                                            ${devices.map(device => `
                                                <div style="padding: 8px; background: #f8f8f8; border-radius: 6px; border: 1px solid ${this.getStatusBorderColor(device.status_color)};">
                                                    <div style="font-size: 11px; font-weight: 500; color: #1d1d1f; margin-bottom: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                                                        ${device.name}
                                                    </div>
                                                    <div style="font-size: 12px; color: ${this.getStatusColor(device.status_color)}; font-weight: 600;">
                                                        ${device.state_display}
                                                    </div>
                                                </div>
                                            `).join('')}
                                        </div>
                                    </div>
                                `).join('')}
                                
                                <div style="margin-top: 12px; padding-top: 12px; border-top: 1px solid #e5e5e7; font-size: 11px; color: #8e8e93;">
                                    ${roomGroup.devices.length} devices • Last update: ${new Date(roomGroup.devices[0]?.last_update).toLocaleTimeString()}
                                </div>
                            </div>
                        </div>
                    `;
                }).join('') || '<div style="color: #6e6e73;">No room data available</div>';
                
                content.innerHTML = roomsHtml;
            }
            
            getDeviceCategory(deviceType) {
                if (!deviceType) return 'Other';
                if (deviceType.includes('Light') || deviceType === 'Switch' || deviceType === 'Dimmer') return 'Lighting';
                if (deviceType.includes('Jalousie') || deviceType.includes('Blind')) return 'Blinds';
                if (deviceType.includes('Temperature') || deviceType.includes('Climate')) return 'Climate';
                if (deviceType.includes('Sensor') || deviceType.includes('Motion')) return 'Sensors';
                if (deviceType.includes('InfoOnlyAnalog')) return 'Sensors';
                return 'Other';
            }
            
            getTypeIcon(type) {
                const icons = {
                    'Lighting': '💡',
                    'Blinds': '🪟',
                    'Climate': '🌡️',
                    'Sensors': '📊',
                    'Other': '⚙️'
                };
                return icons[type] || '📦';
            }
            
            getStatusBorderColor(color) {
                const colors = {
                    'green': '#d1f2d1',
                    'blue': '#d1e7ff',
                    'orange': '#ffe4cc',
                    'red': '#ffd1d1',
                    'gray': '#e5e5e7'
                };
                return colors[color] || '#e5e5e7';
            }
            
            getStatusColor(colorName) {
                const colors = {
                    'green': '#30d158',
                    'blue': '#007aff', 
                    'orange': '#ff9f0a',
                    'red': '#ff3b30',
                    'gray': '#8e8e93'
                };
                return colors[colorName] || '#8e8e93';
            }
            
            updateOperationalSection(operational) {
                const content = document.getElementById('operationalContent');
                
                if (!operational) {
                    content.innerHTML = '<div class="error-message">No operational data available</div>';
                    return;
                }
                
                content.innerHTML = `
                    <div style="margin-bottom: 12px;">
                        <strong>API Performance:</strong>
                        <div style="font-size: 12px;">
                            ${operational.api_performance?.requests_per_minute?.toFixed(1) || 0} req/min,
                            ${operational.api_performance?.avg_response_time_ms?.toFixed(0) || 0}ms avg
                        </div>
                    </div>
                    <div style="margin-bottom: 12px;">
                        <strong>Rate Limiter:</strong>
                        <div style="font-size: 12px;">
                            ${operational.rate_limiter?.recent_hits || 0} hits,
                            ${operational.rate_limiter?.blocked_requests || 0} blocked
                        </div>
                    </div>
                    <div>
                        <strong>Resources:</strong>
                        <div style="font-size: 12px;">
                            ${operational.resources?.websocket_connections || 0} WebSocket connections
                        </div>
                    </div>
                `;
            }
            
            updateTrendsSection(trends) {
                const content = document.getElementById('trendsContent');
                content.innerHTML = '<div style="color: #6e6e73;">Historical trend analysis coming soon...</div>';
            }
            
            handleUpdate(update) {
                // Handle incremental updates
                console.log('Received update:', update.update_type);
                
                // For now, just log the update
                // In a full implementation, this would update specific sections
            }
            
            getConnectionStatusClass(status) {
                if (typeof status === 'string') return 'error';
                if (status?.Connected !== undefined) return 'connected';
                if (status?.Connecting !== undefined) return 'warning';
                return 'error';
            }
            
            formatConnectionStatus(status) {
                if (typeof status === 'string') return status;
                if (status?.Connected !== undefined) return 'Connected';
                if (status?.Connecting !== undefined) return 'Connecting';
                if (status?.Disconnected !== undefined) return 'Disconnected';
                if (status?.Error !== undefined) return `Error: ${status.Error}`;
                return 'Unknown';
            }
            
            formatTime(timestamp) {
                return new Date(timestamp).toLocaleTimeString();
            }
            
            showError(message) {
                const statusBar = document.getElementById('statusBar');
                statusBar.innerHTML = `<div class="error-message">${message}</div>`;
            }
        }
        
        // Initialize dashboard when page loads
        document.addEventListener('DOMContentLoaded', () => {
            new DashboardApp();
        });
    </script>
</body>
</html>"#.to_string()
}

impl Clone for WebSocketStats {
    fn clone(&self) -> Self {
        Self {
            total_connections: self.total_connections,
            active_connections: self.active_connections,
            messages_sent: self.messages_sent,
            messages_failed: self.messages_failed,
        }
    }
}
