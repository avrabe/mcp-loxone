//! Example integration of dynamic dashboard with HTTP server

use super::dashboard_api::create_dashboard_router;
use super::core::UnifiedHistoryStore;
use crate::client::ClientContext;
use crate::error::Result;
use axum::{
    response::Html,
    routing::get,
    Router,
};
use std::sync::Arc;

/// Create a complete web interface with dynamic dashboard
pub fn create_dashboard_web_interface(
    history_store: Arc<UnifiedHistoryStore>,
    client_context: Arc<ClientContext>,
) -> Router {
    Router::new()
        // Serve the dashboard UI
        .route("/", get(dashboard_home))
        
        // Dashboard API endpoints
        .nest("/api/dashboard", create_dashboard_router(history_store, client_context))
        
        // Static assets (in a real implementation, you'd serve from files)
        .route("/dashboard.js", get(dashboard_javascript))
        .route("/dashboard.css", get(dashboard_css))
}

/// Serve the main dashboard HTML page
async fn dashboard_home() -> Html<&'static str> {
    Html(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Loxone Dynamic Dashboard</title>
    <link rel="stylesheet" href="/dashboard.css">
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
</head>
<body>
    <div id="app">
        <header class="header">
            <h1>🏠 Loxone Dynamic Dashboard</h1>
            <div class="controls">
                <select id="roomFilter">
                    <option value="">All Rooms</option>
                </select>
                <select id="timeRange">
                    <option value="1h">Last Hour</option>
                    <option value="6h">Last 6 Hours</option>
                    <option value="24h" selected>Last 24 Hours</option>
                    <option value="7d">Last 7 Days</option>
                </select>
                <button id="refreshBtn">🔄 Refresh</button>
            </div>
        </header>
        
        <main class="dashboard-grid" id="dashboardGrid">
            <div class="loading">
                <div class="spinner"></div>
                <p>Discovering available data sources...</p>
            </div>
        </main>
        
        <footer class="footer">
            <div class="discovery-info">
                <span id="discoveryInfo">Discovery in progress...</span>
                <span id="lastUpdate"></span>
            </div>
        </footer>
    </div>
    
    <script src="/dashboard.js"></script>
</body>
</html>
"#)
}

/// Serve dashboard JavaScript
async fn dashboard_javascript() -> &'static str {
    r#"
class DynamicDashboard {
    constructor() {
        this.apiBase = '/api/dashboard';
        this.refreshInterval = null;
        this.widgets = new Map();
        this.init();
    }
    
    async init() {
        this.setupEventListeners();
        await this.loadDashboard();
        this.startAutoRefresh();
    }
    
    setupEventListeners() {
        document.getElementById('refreshBtn').addEventListener('click', () => {
            this.loadDashboard(true);
        });
        
        document.getElementById('roomFilter').addEventListener('change', () => {
            this.loadDashboard();
        });
        
        document.getElementById('timeRange').addEventListener('change', () => {
            this.loadDashboard();
        });
    }
    
    async loadDashboard(forceRefresh = false) {
        try {
            const params = new URLSearchParams();
            
            const room = document.getElementById('roomFilter').value;
            const timeRange = document.getElementById('timeRange').value;
            
            if (room) params.append('room', room);
            if (timeRange) params.append('time_range', timeRange);
            if (forceRefresh) params.append('refresh', 'true');
            
            const response = await fetch(`${this.apiBase}/?${params}`);
            const layout = await response.json();
            
            this.renderDashboard(layout);
            this.updateDiscoveryInfo(layout.discovery_info);
            this.updateFilters(layout.available_filters);
            
        } catch (error) {
            console.error('Failed to load dashboard:', error);
            this.showError('Failed to load dashboard data');
        }
    }
    
    renderDashboard(layout) {
        const grid = document.getElementById('dashboardGrid');
        grid.innerHTML = '';
        
        layout.widgets.forEach(widget => {
            const widgetElement = this.createWidgetElement(widget);
            grid.appendChild(widgetElement);
        });
        
        document.getElementById('lastUpdate').textContent = 
            `Last updated: ${new Date(layout.generated_at).toLocaleTimeString()}`;
    }
    
    createWidgetElement(widget) {
        const div = document.createElement('div');
        div.className = `widget widget-${widget.size.toLowerCase()}`;
        div.id = `widget-${widget.id}`;
        
        div.innerHTML = `
            <div class="widget-header">
                <h3>${widget.title}</h3>
                <span class="widget-type">${widget.widget_type}</span>
            </div>
            <div class="widget-content" id="content-${widget.id}">
                <div class="loading-small">Loading...</div>
            </div>
        `;
        
        // Render widget content based on type
        setTimeout(() => {
            this.renderWidgetContent(widget);
        }, 100);
        
        return div;
    }
    
    renderWidgetContent(widget) {
        const content = document.getElementById(`content-${widget.id}`);
        
        switch (widget.widget_type) {
            case 'room_grid':
                this.renderRoomGrid(content, widget.data);
                break;
            case 'device_list':
                this.renderDeviceList(content, widget.data);
                break;
            case 'temperature_chart':
                this.renderTemperatureChart(content, widget.data);
                break;
            case 'door_window_status':
                this.renderDoorWindowStatus(content, widget.data);
                break;
            case 'activity_timeline':
                this.renderActivityTimeline(content, widget.data);
                break;
            case 'metrics_chart':
                this.renderMetricsChart(content, widget.data);
                break;
            default:
                content.innerHTML = `<pre>${JSON.stringify(widget.data, null, 2)}</pre>`;
        }
    }
    
    renderRoomGrid(content, data) {
        const rooms = Object.entries(data);
        content.innerHTML = `
            <div class="room-grid">
                ${rooms.map(([room, stats]) => `
                    <div class="room-card">
                        <h4>${room}</h4>
                        <div class="room-stats">
                            <span>📱 ${stats.device_count} devices</span>
                            <span>🟢 ${stats.active_devices} active</span>
                            <span>📊 ${stats.sensor_count} sensors</span>
                        </div>
                    </div>
                `).join('')}
            </div>
        `;
    }
    
    renderDeviceList(content, devices) {
        content.innerHTML = `
            <div class="device-list">
                ${devices.map(device => `
                    <div class="device-item ${device.is_active ? 'active' : 'inactive'}">
                        <div class="device-info">
                            <strong>${device.name}</strong>
                            <span class="device-type">${device.device_type}</span>
                            ${device.room ? `<span class="room">📍 ${device.room}</span>` : ''}
                        </div>
                        <div class="device-status">
                            <span class="status-indicator ${device.is_active ? 'on' : 'off'}">
                                ${device.is_active ? '🟢' : '🔴'}
                            </span>
                            <span class="last-seen">
                                ${new Date(device.last_seen).toLocaleTimeString()}
                            </span>
                        </div>
                    </div>
                `).join('')}
            </div>
        `;
    }
    
    renderTemperatureChart(content, sensors) {
        const canvas = document.createElement('canvas');
        content.innerHTML = '';
        content.appendChild(canvas);
        
        // Simple chart with Chart.js (in a real implementation, you'd use actual sensor data)
        new Chart(canvas, {
            type: 'line',
            data: {
                labels: ['1h ago', '45m ago', '30m ago', '15m ago', 'now'],
                datasets: sensors.map((sensor, i) => ({
                    label: sensor.name,
                    data: [sensor.min_value, sensor.avg_value, sensor.max_value, sensor.avg_value, sensor.avg_value],
                    borderColor: `hsl(${i * 60}, 70%, 50%)`,
                    backgroundColor: `hsla(${i * 60}, 70%, 50%, 0.1)`,
                    tension: 0.3
                }))
            },
            options: {
                responsive: true,
                scales: {
                    y: {
                        title: {
                            display: true,
                            text: sensors[0]?.unit || '°C'
                        }
                    }
                }
            }
        });
    }
    
    renderDoorWindowStatus(content, sensors) {
        content.innerHTML = `
            <div class="sensor-status-grid">
                ${sensors.map(sensor => `
                    <div class="sensor-status-item">
                        <span class="sensor-name">${sensor.name}</span>
                        <span class="sensor-value ${sensor.avg_value > 0.5 ? 'open' : 'closed'}">
                            ${sensor.avg_value > 0.5 ? '🟠 Open' : '🟢 Closed'}
                        </span>
                    </div>
                `).join('')}
            </div>
        `;
    }
    
    renderActivityTimeline(content, events) {
        content.innerHTML = `
            <div class="activity-timeline">
                ${events.slice(0, 10).map(event => `
                    <div class="timeline-item">
                        <div class="timeline-time">
                            ${new Date(event.timestamp).toLocaleTimeString()}
                        </div>
                        <div class="timeline-content">
                            <strong>${this.getEventTitle(event)}</strong>
                            <p>${this.getEventDescription(event)}</p>
                        </div>
                    </div>
                `).join('')}
            </div>
        `;
    }
    
    renderMetricsChart(content, metrics) {
        content.innerHTML = `
            <div class="metrics-summary">
                <h4>System Health</h4>
                <div class="metric-items">
                    ${Array.from(metrics).map(metric => `
                        <div class="metric-item">
                            <span class="metric-name">${metric}</span>
                            <span class="metric-status">✓</span>
                        </div>
                    `).join('')}
                </div>
            </div>
        `;
    }
    
    getEventTitle(event) {
        switch (event.category?.type) {
            case 'DeviceState': return `${event.category.data.device_name} changed`;
            case 'SensorReading': return `${event.category.data.sensor_name} reading`;
            case 'SystemMetric': return `System: ${event.category.data.metric_name}`;
            case 'AuditEvent': return `Audit: ${event.category.data.action}`;
            default: return 'System Event';
        }
    }
    
    getEventDescription(event) {
        switch (event.category?.type) {
            case 'DeviceState': 
                return `Device in ${event.category.data.room || 'unknown room'} changed state`;
            case 'SensorReading': 
                return `${event.category.data.value} ${event.category.data.unit}`;
            default: 
                return 'System activity detected';
        }
    }
    
    updateDiscoveryInfo(info) {
        const discoveryInfo = document.getElementById('discoveryInfo');
        discoveryInfo.textContent = 
            `📊 ${info.total_devices} devices, ${info.total_sensors} sensors, ${info.total_rooms} rooms`;
    }
    
    updateFilters(filters) {
        const roomFilter = document.getElementById('roomFilter');
        const currentRoom = roomFilter.value;
        
        // Update room filter options
        const roomFilterObj = filters.find(f => f.name === 'room');
        if (roomFilterObj) {
            roomFilter.innerHTML = '<option value="">All Rooms</option>';
            roomFilterObj.options.forEach(room => {
                const option = document.createElement('option');
                option.value = room;
                option.textContent = room;
                if (room === currentRoom) option.selected = true;
                roomFilter.appendChild(option);
            });
        }
    }
    
    startAutoRefresh() {
        this.refreshInterval = setInterval(() => {
            this.loadDashboard();
        }, 30000); // Refresh every 30 seconds
    }
    
    showError(message) {
        const grid = document.getElementById('dashboardGrid');
        grid.innerHTML = `
            <div class="error">
                <h3>⚠️ Error</h3>
                <p>${message}</p>
                <button onclick="location.reload()">Retry</button>
            </div>
        `;
    }
}

// Initialize dashboard when page loads
document.addEventListener('DOMContentLoaded', () => {
    new DynamicDashboard();
});
"#
}

/// Serve dashboard CSS
async fn dashboard_css() -> &'static str {
    r#"
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #f5f7fa;
    color: #333;
    line-height: 1.6;
}

.header {
    background: white;
    border-bottom: 1px solid #e1e8ed;
    padding: 1rem 2rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
}

.header h1 {
    color: #2c3e50;
    font-size: 1.5rem;
}

.controls {
    display: flex;
    gap: 1rem;
    align-items: center;
}

.controls select, .controls button {
    padding: 0.5rem 1rem;
    border: 1px solid #ddd;
    border-radius: 6px;
    background: white;
    font-size: 0.9rem;
}

.controls button {
    background: #3498db;
    color: white;
    cursor: pointer;
    border: none;
}

.controls button:hover {
    background: #2980b9;
}

.dashboard-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 1.5rem;
    padding: 2rem;
    max-width: 1400px;
    margin: 0 auto;
}

.widget {
    background: white;
    border-radius: 12px;
    box-shadow: 0 4px 6px rgba(0,0,0,0.1);
    overflow: hidden;
    transition: transform 0.2s, box-shadow 0.2s;
}

.widget:hover {
    transform: translateY(-2px);
    box-shadow: 0 8px 15px rgba(0,0,0,0.15);
}

.widget-small { grid-column: span 1; min-height: 200px; }
.widget-medium { grid-column: span 1; min-height: 300px; }
.widget-large { grid-column: span 2; min-height: 400px; }
.widget-wide { grid-column: span 3; min-height: 300px; }

.widget-header {
    background: #34495e;
    color: white;
    padding: 1rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
}

.widget-header h3 {
    font-size: 1.1rem;
    font-weight: 600;
}

.widget-type {
    background: rgba(255,255,255,0.2);
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.8rem;
}

.widget-content {
    padding: 1.5rem;
    height: calc(100% - 60px);
    overflow-y: auto;
}

.loading, .loading-small {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #7f8c8d;
}

.spinner {
    width: 40px;
    height: 40px;
    border: 4px solid #ecf0f1;
    border-top: 4px solid #3498db;
    border-radius: 50%;
    animation: spin 1s linear infinite;
    margin-bottom: 1rem;
}

@keyframes spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
}

.room-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 1rem;
}

.room-card {
    background: #f8f9fa;
    border: 1px solid #e9ecef;
    border-radius: 8px;
    padding: 1rem;
    text-align: center;
}

.room-card h4 {
    margin-bottom: 0.5rem;
    color: #2c3e50;
}

.room-stats {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85rem;
    color: #7f8c8d;
}

.device-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
}

.device-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem;
    background: #f8f9fa;
    border-radius: 8px;
    border-left: 4px solid #ddd;
}

.device-item.active {
    border-left-color: #27ae60;
    background: #f0fff4;
}

.device-item.inactive {
    border-left-color: #e74c3c;
    background: #fff5f5;
}

.device-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
}

.device-type {
    font-size: 0.8rem;
    color: #7f8c8d;
}

.room {
    font-size: 0.8rem;
    color: #3498db;
}

.device-status {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.25rem;
}

.status-indicator {
    font-size: 1.2rem;
}

.last-seen {
    font-size: 0.8rem;
    color: #7f8c8d;
}

.activity-timeline {
    display: flex;
    flex-direction: column;
    gap: 1rem;
}

.timeline-item {
    display: flex;
    gap: 1rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid #ecf0f1;
}

.timeline-item:last-child {
    border-bottom: none;
}

.timeline-time {
    font-size: 0.8rem;
    color: #7f8c8d;
    min-width: 80px;
}

.timeline-content h4 {
    margin-bottom: 0.25rem;
    color: #2c3e50;
}

.timeline-content p {
    font-size: 0.9rem;
    color: #7f8c8d;
}

.metrics-summary {
    text-align: center;
}

.metric-items {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-top: 1rem;
}

.metric-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem;
    background: #f8f9fa;
    border-radius: 6px;
}

.metric-status {
    color: #27ae60;
    font-weight: bold;
}

.sensor-status-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 1rem;
}

.sensor-status-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem;
    background: #f8f9fa;
    border-radius: 8px;
}

.sensor-value.open {
    color: #e67e22;
    font-weight: 600;
}

.sensor-value.closed {
    color: #27ae60;
    font-weight: 600;
}

.footer {
    background: white;
    border-top: 1px solid #e1e8ed;
    padding: 1rem 2rem;
    text-align: center;
    color: #7f8c8d;
    font-size: 0.9rem;
}

.discovery-info {
    display: flex;
    justify-content: space-between;
    align-items: center;
}

.error {
    grid-column: 1 / -1;
    text-align: center;
    padding: 3rem;
    background: white;
    border-radius: 12px;
    box-shadow: 0 4px 6px rgba(0,0,0,0.1);
}

.error h3 {
    color: #e74c3c;
    margin-bottom: 1rem;
}

.error button {
    margin-top: 1rem;
    padding: 0.75rem 1.5rem;
    background: #3498db;
    color: white;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 1rem;
}

@media (max-width: 768px) {
    .header {
        flex-direction: column;
        gap: 1rem;
        padding: 1rem;
    }
    
    .controls {
        flex-wrap: wrap;
        justify-content: center;
    }
    
    .dashboard-grid {
        grid-template-columns: 1fr;
        padding: 1rem;
    }
    
    .widget-large, .widget-wide {
        grid-column: span 1;
    }
    
    .discovery-info {
        flex-direction: column;
        gap: 0.5rem;
    }
}
"#
}