//! Unified dashboard with new styling

use crate::shared_styles::{get_nav_header, get_shared_styles};

/// Generate the main dashboard HTML
pub fn generate_dashboard_html() -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Loxone MCP Dashboard</title>
    {}
    <style>
        /* Dashboard-specific styles */
        .dashboard-grid {{
            display: grid;
            grid-template-columns: 1fr;
            gap: calc(var(--spacing-unit) * 3);
            margin-top: calc(var(--spacing-unit) * 3);
        }}
        
        @media (min-width: 1200px) {{
            .dashboard-grid {{
                grid-template-columns: 1fr 1fr;
                grid-template-areas: 
                    "devices devices"
                    "realtime operational"
                    "trends trends";
            }}
            
            .dashboard-section:nth-child(1) {{ grid-area: realtime; }}
            .dashboard-section:nth-child(2) {{ grid-area: devices; }}
            .dashboard-section:nth-child(3) {{ grid-area: operational; }}
            .dashboard-section:nth-child(4) {{ grid-area: trends; }}
        }}
        
        .dashboard-section {{
            background: var(--bg-secondary);
            border-radius: var(--border-radius);
            padding: calc(var(--spacing-unit) * 3);
            box-shadow: 0 2px 10px var(--shadow-color);
            transition: transform var(--transition-fast);
        }}
        
        .dashboard-section:hover {{
            transform: translateY(-2px);
            box-shadow: 0 4px 20px var(--shadow-color);
        }}
        
        .section-header {{
            display: flex;
            align-items: center;
            gap: calc(var(--spacing-unit) * 1.5);
            margin-bottom: calc(var(--spacing-unit) * 2);
        }}
        
        .section-icon {{
            width: 40px;
            height: 40px;
            border-radius: 10px;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 1.5rem;
            transition: transform var(--transition-fast);
        }}
        
        .dashboard-section:hover .section-icon {{
            transform: scale(1.1) rotate(5deg);
        }}
        
        .section-icon.realtime {{ 
            background: linear-gradient(135deg, var(--success-color), hsl(165, 70%, 45%)); 
            color: white;
        }}
        .section-icon.devices {{ 
            background: linear-gradient(135deg, hsl(var(--accent-hue), 70%, 50%), hsl(calc(var(--accent-hue) + 20), 70%, 50%)); 
            color: white;
        }}
        .section-icon.operational {{ 
            background: linear-gradient(135deg, var(--warning-color), hsl(45, 90%, 50%)); 
            color: white;
        }}
        .section-icon.trends {{ 
            background: linear-gradient(135deg, hsl(280, 70%, 50%), hsl(300, 70%, 50%)); 
            color: white;
        }}
        
        .section-title {{
            font-size: 1.25rem;
            font-weight: 700;
            margin: 0;
        }}
        
        .websocket-status {{
            position: fixed;
            top: calc(var(--header-height) + var(--spacing-unit) * 2);
            right: calc(var(--spacing-unit) * 3);
            background: var(--bg-secondary);
            border-radius: 24px;
            padding: calc(var(--spacing-unit) * 1) calc(var(--spacing-unit) * 2);
            box-shadow: 0 2px 10px var(--shadow-color);
            font-size: 0.875rem;
            font-weight: 600;
            z-index: 100;
            display: flex;
            align-items: center;
            gap: calc(var(--spacing-unit) * 1);
            transition: all var(--transition-fast);
        }}
        
        .websocket-status.connected {{ 
            border-left: 3px solid var(--success-color);
        }}
        .websocket-status.disconnected {{ 
            border-left: 3px solid var(--error-color);
        }}
        
        .status-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: calc(var(--spacing-unit) * 2);
            margin-bottom: calc(var(--spacing-unit) * 3);
        }}
        
        .status-metric {{
            background: var(--bg-primary);
            border-radius: calc(var(--border-radius) / 2);
            padding: calc(var(--spacing-unit) * 2);
            border: 1px solid var(--border-color);
            transition: all var(--transition-fast);
        }}
        
        .status-metric:hover {{
            border-color: var(--accent-primary);
            transform: translateY(-2px);
        }}
        
        .status-metric-label {{
            font-size: 0.875rem;
            color: var(--text-secondary);
            margin-bottom: calc(var(--spacing-unit) * 0.5);
        }}
        
        .status-metric-value {{
            font-size: 1.5rem;
            font-weight: 700;
            color: var(--text-primary);
        }}
        
        .room-card {{
            background: var(--bg-primary);
            border-radius: var(--border-radius);
            overflow: hidden;
            margin-bottom: calc(var(--spacing-unit) * 2);
            border: 1px solid var(--border-color);
            transition: all var(--transition-fast);
        }}
        
        .room-card:hover {{
            border-color: var(--accent-primary);
            box-shadow: 0 4px 12px var(--shadow-color);
        }}
        
        .room-card.active {{
            border-left: 4px solid var(--success-color);
        }}
        
        .room-header {{
            padding: calc(var(--spacing-unit) * 2);
            background: var(--bg-secondary);
            display: flex;
            justify-content: space-between;
            align-items: center;
        }}
        
        .room-name {{
            font-size: 1.125rem;
            font-weight: 600;
            margin: 0;
        }}
        
        .room-temp {{
            font-size: 1.25rem;
            font-weight: 700;
            color: var(--accent-primary);
        }}
        
        .room-devices {{
            padding: calc(var(--spacing-unit) * 2);
        }}
        
        .device-type-group {{
            margin-bottom: calc(var(--spacing-unit) * 2);
        }}
        
        .device-type-label {{
            font-size: 0.8125rem;
            font-weight: 600;
            color: var(--text-secondary);
            margin-bottom: calc(var(--spacing-unit) * 1);
            display: flex;
            align-items: center;
            gap: calc(var(--spacing-unit) * 0.5);
        }}
        
        .device-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
            gap: calc(var(--spacing-unit) * 1);
        }}
        
        .device-tile {{
            background: var(--bg-secondary);
            border: 1px solid var(--border-color);
            border-radius: calc(var(--border-radius) / 2);
            padding: calc(var(--spacing-unit) * 1.5);
            text-align: center;
            transition: all var(--transition-fast);
            cursor: pointer;
        }}
        
        .device-tile:hover {{
            border-color: var(--accent-primary);
            transform: translateY(-2px);
        }}
        
        .device-tile.active {{
            background: hsla(var(--success-color), 0.1);
            border-color: var(--success-color);
        }}
        
        .device-tile-name {{
            font-size: 0.75rem;
            font-weight: 600;
            margin-bottom: calc(var(--spacing-unit) * 0.5);
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
        }}
        
        .device-tile-state {{
            font-size: 0.875rem;
            font-weight: 700;
        }}
        
        .room-footer {{
            padding: calc(var(--spacing-unit) * 1.5);
            background: var(--bg-secondary);
            border-top: 1px solid var(--border-color);
            font-size: 0.75rem;
            color: var(--text-secondary);
            text-align: center;
        }}
    </style>
</head>
<body>
    {}
    
    <div class="container">
        <div class="websocket-status" id="wsStatus">
            <span id="wsIcon">⚪</span>
            <span id="wsText">Connecting...</span>
        </div>
        
        <div class="status-grid" id="statusBar">
            <div class="loading">Loading system status...</div>
        </div>
        
        <div class="dashboard-grid">
            <div class="dashboard-section">
                <div class="section-header">
                    <div class="section-icon realtime">📊</div>
                    <h2 class="section-title">Real-time Monitoring</h2>
                </div>
                <div id="realtimeContent" class="loading">Loading real-time data...</div>
            </div>
            
            <div class="dashboard-section">
                <div class="section-header">
                    <div class="section-icon devices">🏠</div>
                    <h2 class="section-title">Rooms & Devices</h2>
                </div>
                <div id="devicesContent" class="loading">Loading device data...</div>
            </div>
            
            <div class="dashboard-section">
                <div class="section-header">
                    <div class="section-icon operational">⚙️</div>
                    <h2 class="section-title">System Performance</h2>
                </div>
                <div id="operationalContent" class="loading">Loading operational data...</div>
            </div>
            
            <div class="dashboard-section">
                <div class="section-header">
                    <div class="section-icon trends">📈</div>
                    <h2 class="section-title">Historical Trends</h2>
                </div>
                <div id="trendsContent" class="loading">Loading trend data...</div>
            </div>
        </div>
    </div>
    
    <script>
        class DashboardApp {{
            constructor() {{
                this.ws = null;
                this.reconnectInterval = 5000;
                this.maxReconnectAttempts = 10;
                this.reconnectAttempts = 0;
                this.init();
            }}
            
            init() {{
                this.loadInitialData();
                this.connectWebSocket();
                
                // Reload page if it's been open for more than 1 hour
                setTimeout(() => location.reload(), 3600000);
            }}
            
            async loadInitialData() {{
                try {{
                    const response = await fetch('/dashboard/api/data');
                    if (!response.ok) throw new Error(`HTTP ${{response.status}}`);
                    
                    const data = await response.json();
                    this.updateDashboard(data);
                }} catch (error) {{
                    console.error('Failed to load initial data:', error);
                    this.showError('Failed to load dashboard data');
                }}
            }}
            
            connectWebSocket() {{
                const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
                const wsUrl = `${{protocol}}//${{window.location.host}}/dashboard/ws`;
                
                this.ws = new WebSocket(wsUrl);
                
                this.ws.onopen = () => {{
                    console.log('WebSocket connected');
                    this.updateWebSocketStatus('connected');
                    this.reconnectAttempts = 0;
                }};
                
                this.ws.onmessage = (event) => {{
                    try {{
                        const data = JSON.parse(event.data);
                        if (data.update_type === 'FullRefresh') {{
                            this.updateDashboard(data.data);
                        }} else {{
                            this.handleUpdate(data);
                        }}
                    }} catch (error) {{
                        console.error('Failed to parse WebSocket message:', error);
                    }}
                }};
                
                this.ws.onclose = () => {{
                    console.log('WebSocket disconnected');
                    this.updateWebSocketStatus('disconnected');
                    this.scheduleReconnect();
                }};
                
                this.ws.onerror = (error) => {{
                    console.error('WebSocket error:', error);
                    this.updateWebSocketStatus('error');
                }};
            }}
            
            scheduleReconnect() {{
                if (this.reconnectAttempts < this.maxReconnectAttempts) {{
                    setTimeout(() => {{
                        console.log(`Reconnecting WebSocket (attempt ${{this.reconnectAttempts + 1}})`);
                        this.reconnectAttempts++;
                        this.connectWebSocket();
                    }}, this.reconnectInterval);
                }}
            }}
            
            updateWebSocketStatus(status) {{
                const statusEl = document.getElementById('wsStatus');
                const iconEl = document.getElementById('wsIcon');
                const textEl = document.getElementById('wsText');
                
                statusEl.className = `websocket-status ${{status}}`;
                
                switch (status) {{
                    case 'connected':
                        iconEl.textContent = '🟢';
                        textEl.textContent = 'Live Updates';
                        break;
                    case 'disconnected':
                        iconEl.textContent = '🔴';
                        textEl.textContent = 'Disconnected';
                        break;
                    case 'error':
                        iconEl.textContent = '⚠️';
                        textEl.textContent = 'Connection Error';
                        break;
                }}
            }}
            
            updateDashboard(data) {{
                this.updateStatusBar(data.realtime?.system_health);
                this.updateRealtimeSection(data.realtime);
                this.updateDevicesSection(data.devices);
                this.updateOperationalSection(data.operational);
                this.updateTrendsSection(data.trends);
            }}
            
            updateStatusBar(systemHealth) {{
                const statusBar = document.getElementById('statusBar');
                
                if (!systemHealth) {{
                    statusBar.innerHTML = '<div class="loading">Loading status...</div>';
                    return;
                }}
                
                const connectionStatus = this.formatConnectionStatus(systemHealth.connection_status);
                const connectionClass = this.getConnectionStatusClass(systemHealth.connection_status);
                
                statusBar.innerHTML = `
                    <div class="status-metric">
                        <div class="status-metric-label">Connection</div>
                        <div class="status-metric-value status-badge ${{connectionClass}}">
                            ${{connectionStatus}}
                        </div>
                    </div>
                    <div class="status-metric">
                        <div class="status-metric-label">Last Update</div>
                        <div class="status-metric-value">${{this.formatTime(systemHealth.last_update)}}</div>
                    </div>
                    <div class="status-metric">
                        <div class="status-metric-label">Error Rate</div>
                        <div class="status-metric-value">${{systemHealth.error_rate.toFixed(1)}}/min</div>
                    </div>
                    <div class="status-metric">
                        <div class="status-metric-label">Response Time</div>
                        <div class="status-metric-value">${{systemHealth.avg_response_time_ms.toFixed(0)}}ms</div>
                    </div>
                `;
            }}
            
            updateRealtimeSection(realtime) {{
                const content = document.getElementById('realtimeContent');
                
                if (!realtime) {{
                    content.innerHTML = '<div class="error-message">No real-time data available</div>';
                    return;
                }}
                
                content.innerHTML = `
                    <div class="mb-2">
                        <strong>Active Sensors:</strong> ${{realtime.active_sensors?.length || 0}}
                    </div>
                    <div class="mb-2">
                        <strong>Recent Activity:</strong>
                        <div class="mt-1">
                            ${{realtime.recent_activity?.length ? 
                                realtime.recent_activity.slice(0, 5).map(activity => 
                                    `<div class="text-muted" style="font-size: 0.875rem; margin: 4px 0;">
                                        ${{activity.device_name}} - ${{activity.action}}
                                    </div>`
                                ).join('') : 
                                '<div class="text-muted" style="font-size: 0.875rem;">No recent activity</div>'
                            }}
                        </div>
                    </div>
                `;
            }}
            
            updateDevicesSection(devices) {{
                const content = document.getElementById('devicesContent');
                
                if (!devices || !devices.device_matrix) {{
                    content.innerHTML = '<div class="error-message">No device data available</div>';
                    return;
                }}
                
                const roomsHtml = devices.device_matrix.map(roomGroup => {{
                    const roomSummary = devices.rooms?.find(r => r.name === roomGroup.room_name) || {{}};
                    
                    // Group devices by type
                    const devicesByType = {{}};
                    roomGroup.devices.forEach(device => {{
                        const type = this.getDeviceCategory(device.device_type || device.type);
                        if (!devicesByType[type]) devicesByType[type] = [];
                        devicesByType[type].push(device);
                    }});
                    
                    const hasActiveDevices = roomGroup.devices.some(d => 
                        d.status_color === 'green' || d.status_color === 'blue' ||
                        (d.states?.active && d.states.active > 0)
                    );
                    
                    const tempDevice = roomGroup.devices.find(d => 
                        d.device_type?.includes('Temperature') || d.device_type?.includes('InfoOnlyAnalog')
                    );
                    const tempDisplay = roomSummary.temp_display || 
                        (tempDevice && tempDevice.state ? `${{tempDevice.state}}°C` : '');
                    
                    return `
                        <div class="room-card ${{hasActiveDevices ? 'active' : ''}}">
                            <div class="room-header">
                                <h3 class="room-name">${{roomGroup.room_name}}</h3>
                                ${{tempDisplay ? `<span class="room-temp">${{tempDisplay}}</span>` : ''}}
                            </div>
                            <div class="room-devices">
                                ${{Object.entries(devicesByType).map(([type, typeDevices]) => `
                                    <div class="device-type-group">
                                        <div class="device-type-label">
                                            ${{this.getTypeIcon(type)}} ${{type}}
                                        </div>
                                        <div class="device-grid">
                                            ${{typeDevices.map(device => {{
                                                const isActive = device.status_color === 'green' || 
                                                               device.status_color === 'blue' ||
                                                               (device.states?.active && device.states.active > 0);
                                                const stateDisplay = device.state_display || 
                                                                   (device.states?.active !== undefined ? 
                                                                    (device.states.active > 0 ? 'On' : 'Off') : 
                                                                    'Unknown');
                                                
                                                return `
                                                    <div class="device-tile ${{isActive ? 'active' : ''}}">
                                                        <div class="device-tile-name">${{device.name}}</div>
                                                        <div class="device-tile-state" style="color: ${{this.getStatusColor(device.status_color || (isActive ? 'green' : 'gray'))}}">
                                                            ${{stateDisplay}}
                                                        </div>
                                                    </div>
                                                `;
                                            }}).join('')}}
                                        </div>
                                    </div>
                                `).join('')}}
                            </div>
                            <div class="room-footer">
                                ${{roomGroup.devices.length}} devices • Last update: ${{new Date().toLocaleTimeString()}}
                            </div>
                        </div>
                    `;
                }}).join('') || '<div class="text-muted">No room data available</div>';
                
                content.innerHTML = roomsHtml;
            }}
            
            getDeviceCategory(deviceType) {{
                if (!deviceType) return 'Other';
                if (deviceType.includes('Light') || deviceType === 'Switch' || deviceType === 'Dimmer') return 'Lighting';
                if (deviceType.includes('Jalousie') || deviceType.includes('Blind')) return 'Blinds';
                if (deviceType.includes('Temperature') || deviceType.includes('Climate')) return 'Climate';
                if (deviceType.includes('Sensor') || deviceType.includes('Motion')) return 'Sensors';
                if (deviceType.includes('InfoOnlyAnalog')) return 'Sensors';
                return 'Other';
            }}
            
            getTypeIcon(type) {{
                const icons = {{
                    'Lighting': '💡',
                    'Blinds': '🪟',
                    'Climate': '🌡️',
                    'Sensors': '📊',
                    'Other': '⚙️'
                }};
                return icons[type] || '📦';
            }}
            
            getStatusColor(colorName) {{
                const colors = {{
                    'green': 'var(--success-color)',
                    'blue': 'var(--accent-primary)', 
                    'orange': 'var(--warning-color)',
                    'red': 'var(--error-color)',
                    'gray': 'var(--text-secondary)'
                }};
                return colors[colorName] || 'var(--text-secondary)';
            }}
            
            updateOperationalSection(operational) {{
                const content = document.getElementById('operationalContent');
                
                if (!operational) {{
                    content.innerHTML = '<div class="error-message">No operational data available</div>';
                    return;
                }}
                
                content.innerHTML = `
                    <div class="status-grid">
                        <div class="status-metric">
                            <div class="status-metric-label">API Requests</div>
                            <div class="status-metric-value">
                                ${{operational.api_performance?.requests_per_minute?.toFixed(1) || 0}}/min
                            </div>
                        </div>
                        <div class="status-metric">
                            <div class="status-metric-label">Avg Response</div>
                            <div class="status-metric-value">
                                ${{operational.api_performance?.avg_response_time_ms?.toFixed(0) || 0}}ms
                            </div>
                        </div>
                        <div class="status-metric">
                            <div class="status-metric-label">Rate Limiter</div>
                            <div class="status-metric-value">
                                ${{operational.rate_limiter?.blocked_requests || 0}} blocked
                            </div>
                        </div>
                        <div class="status-metric">
                            <div class="status-metric-label">WebSockets</div>
                            <div class="status-metric-value">
                                ${{operational.resources?.websocket_connections || 0}} active
                            </div>
                        </div>
                    </div>
                `;
            }}
            
            updateTrendsSection(trends) {{
                const content = document.getElementById('trendsContent');
                content.innerHTML = '<div class="text-muted text-center">Historical trend analysis coming soon...</div>';
            }}
            
            handleUpdate(update) {{
                console.log('Received update:', update.update_type);
            }}
            
            getConnectionStatusClass(status) {{
                if (typeof status === 'string') return 'error';
                if (status?.Connected !== undefined) return 'success';
                if (status?.Connecting !== undefined) return 'warning';
                return 'error';
            }}
            
            formatConnectionStatus(status) {{
                if (typeof status === 'string') return status;
                if (status?.Connected !== undefined) return 'Connected';
                if (status?.Connecting !== undefined) return 'Connecting';
                if (status?.Disconnected !== undefined) return 'Disconnected';
                if (status?.Error !== undefined) return `Error: ${{status.Error}}`;
                return 'Unknown';
            }}
            
            formatTime(timestamp) {{
                return new Date(timestamp).toLocaleTimeString();
            }}
            
            showError(message) {{
                const statusBar = document.getElementById('statusBar');
                statusBar.innerHTML = `<div class="error-message">${{message}}</div>`;
            }}
        }}
        
        // Initialize dashboard when page loads
        document.addEventListener('DOMContentLoaded', () => {{
            new DashboardApp();
        }});
    </script>
</body>
</html>"#,
        get_shared_styles(),
        get_nav_header("Loxone MCP Dashboard", true)
    )
}
