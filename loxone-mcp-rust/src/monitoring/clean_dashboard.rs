//! Clean and modern dashboard with improved layout

use crate::shared_styles::{get_nav_header, get_shared_styles};

/// Generate clean modern dashboard HTML
pub fn generate_clean_dashboard_html() -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="de">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Loxone Dashboard</title>
    {shared_styles}
    <style>
        /* Clean Dashboard Specific Styles */
        .dashboard-container {{
            min-height: 100vh;
            background: linear-gradient(135deg, 
                hsla(220, 30%, 95%, 1) 0%, 
                hsla(240, 25%, 98%, 1) 100%);
        }}
        
        .main-grid {{
            display: grid;
            grid-template-columns: 1fr;
            gap: 2rem;
            max-width: 1400px;
            margin: 0 auto;
            padding: 2rem;
        }}
        
        @media (min-width: 768px) {{
            .main-grid {{
                grid-template-columns: 300px 1fr;
            }}
        }}
        
        /* Sidebar */
        .sidebar {{
            background: white;
            border-radius: 16px;
            padding: 2rem;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.1);
            height: fit-content;
            position: sticky;
            top: 2rem;
        }}
        
        .system-status {{
            text-align: center;
            margin-bottom: 2rem;
            padding: 1.5rem;
            background: linear-gradient(135deg, #4F46E5, #7C3AED);
            border-radius: 12px;
            color: white;
        }}
        
        .status-indicator {{
            width: 12px;
            height: 12px;
            border-radius: 50%;
            display: inline-block;
            margin-right: 8px;
        }}
        
        .status-connected {{ background: #10B981; }}
        .status-disconnected {{ background: #EF4444; }}
        .status-warning {{ background: #F59E0B; }}
        
        .stat-item {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 1rem 0;
            border-bottom: 1px solid #F3F4F6;
        }}
        
        .stat-item:last-child {{
            border-bottom: none;
        }}
        
        .stat-label {{
            font-weight: 500;
            color: #6B7280;
        }}
        
        .stat-value {{
            font-weight: 700;
            color: #1F2937;
            font-size: 1.1rem;
        }}
        
        /* Main Content */
        .main-content {{
            display: flex;
            flex-direction: column;
            gap: 2rem;
        }}
        
        .rooms-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
            gap: 1.5rem;
        }}
        
        .room-card {{
            background: white;
            border-radius: 16px;
            overflow: hidden;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.08);
            transition: all 0.3s ease;
        }}
        
        .room-card:hover {{
            transform: translateY(-4px);
            box-shadow: 0 8px 30px rgba(0, 0, 0, 0.12);
        }}
        
        .room-header {{
            background: linear-gradient(135deg, #667EEA, #764BA2);
            color: white;
            padding: 1.5rem;
            text-align: center;
        }}
        
        .room-name {{
            font-size: 1.25rem;
            font-weight: 700;
            margin: 0 0 0.5rem 0;
        }}
        
        .room-info {{
            font-size: 0.9rem;
            opacity: 0.9;
        }}
        
        .room-body {{
            padding: 1.5rem;
        }}
        
        .device-category {{
            margin-bottom: 1.5rem;
        }}
        
        .device-category:last-child {{
            margin-bottom: 0;
        }}
        
        .category-header {{
            display: flex;
            align-items: center;
            gap: 0.5rem;
            margin-bottom: 1rem;
            font-weight: 600;
            font-size: 0.9rem;
            color: #374151;
        }}
        
        .category-icon {{
            font-size: 1.2rem;
        }}
        
        .devices-list {{
            display: grid;
            gap: 0.75rem;
        }}
        
        .device-item {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 0.75rem 1rem;
            background: #F9FAFB;
            border-radius: 8px;
            transition: all 0.2s ease;
        }}
        
        .device-item:hover {{
            background: #F3F4F6;
        }}
        
        .device-item.active {{
            background: linear-gradient(135deg, #ECFDF5, #D1FAE5);
            border-left: 3px solid #10B981;
        }}
        
        .device-name {{
            font-weight: 500;
            color: #1F2937;
            font-size: 0.9rem;
        }}
        
        .device-status {{
            font-size: 0.8rem;
            font-weight: 600;
            padding: 0.25rem 0.5rem;
            border-radius: 4px;
        }}
        
        .status-on {{
            background: #ECFDF5;
            color: #059669;
        }}
        
        .status-off {{
            background: #F3F4F6;
            color: #6B7280;
        }}
        
        .status-partial {{
            background: #FEF3C7;
            color: #D97706;
        }}
        
        .empty-state {{
            text-align: center;
            padding: 3rem;
            color: #6B7280;
        }}
        
        .loading {{
            text-align: center;
            padding: 2rem;
            color: #6B7280;
        }}
        
        .error {{
            text-align: center;
            padding: 2rem;
            color: #EF4444;
            background: #FEF2F2;
            border-radius: 8px;
            margin: 1rem 0;
        }}
        
        /* Responsive */
        @media (max-width: 767px) {{
            .main-grid {{
                padding: 1rem;
                gap: 1rem;
            }}
            
            .sidebar {{
                position: static;
                order: 2;
            }}
            
            .rooms-grid {{
                grid-template-columns: 1fr;
            }}
        }}
    </style>
</head>
<body>
    {nav_header}
    
    <div class="dashboard-container">
        <div class="main-grid">
            <!-- Sidebar -->
            <aside class="sidebar">
                <div class="system-status">
                    <div id="connectionStatus">
                        <span class="status-indicator status-disconnected"></span>
                        Verbindung wird hergestellt...
                    </div>
                    <div style="margin-top: 0.5rem; font-size: 0.9rem; opacity: 0.9;">
                        Loxone MCP Dashboard
                    </div>
                </div>
                
                <div id="systemStats">
                    <div class="stat-item">
                        <span class="stat-label">Räume</span>
                        <span class="stat-value" id="totalRooms">-</span>
                    </div>
                    <div class="stat-item">
                        <span class="stat-label">Geräte</span>
                        <span class="stat-value" id="totalDevices">-</span>
                    </div>
                    <div class="stat-item">
                        <span class="stat-label">Aktiv</span>
                        <span class="stat-value" id="activeDevices">-</span>
                    </div>
                    <div class="stat-item">
                        <span class="stat-label">Letzte Aktualisierung</span>
                        <span class="stat-value" id="lastUpdate">-</span>
                    </div>
                </div>
                
                <!-- Server Performance Metrics -->
                <div style="margin-top: 2rem; padding-top: 2rem; border-top: 1px solid #E5E7EB;">
                    <h3 style="margin: 0 0 1rem 0; color: #374151; font-size: 1rem;">Server Performance</h3>
                    <div id="serverMetrics">
                        <div class="stat-item">
                            <span class="stat-label">CPU</span>
                            <span class="stat-value" id="cpuUsage">-</span>
                        </div>
                        <div class="stat-item">
                            <span class="stat-label">Memory</span>
                            <span class="stat-value" id="memoryUsage">-</span>
                        </div>
                        <div class="stat-item">
                            <span class="stat-label">Requests/min</span>
                            <span class="stat-value" id="requestsPerMin">-</span>
                        </div>
                        <div class="stat-item">
                            <span class="stat-label">Avg Response</span>
                            <span class="stat-value" id="avgResponseTime">-</span>
                        </div>
                        <div class="stat-item">
                            <span class="stat-label">Uptime</span>
                            <span class="stat-value" id="uptime">-</span>
                        </div>
                        <div class="stat-item">
                            <span class="stat-label">Tools Executed</span>
                            <span class="stat-value" id="toolsExecuted">-</span>
                        </div>
                    </div>
                </div>
            </aside>
            
            <!-- Main Content -->
            <main class="main-content">
                <div id="roomsContainer">
                    <div class="loading">
                        <div>📡 Lade Raum- und Gerätedaten...</div>
                    </div>
                </div>
            </main>
        </div>
    </div>
    
    <script>
        class CleanDashboard {{
            constructor() {{
                this.ws = null;
                this.reconnectAttempts = 0;
                this.maxReconnectAttempts = 5;
                this.reconnectInterval = 3000;
                this.lastValidData = null; // Store last valid data to prevent flicker
                
                this.init();
            }}
            
            async init() {{
                await this.loadInitialData();
                this.connectWebSocket();
                
                // Auto-refresh disabled - using WebSocket for real-time updates
                // setInterval(() => this.loadInitialData(), 90000);
            }}
            
            async loadInitialData() {{
                try {{
                    const response = await fetch('/dashboard/api/data');
                    if (!response.ok) throw new Error(`HTTP ${{response.status}}`);
                    
                    const data = await response.json();
                    console.log('Initial dashboard data loaded:', data);
                    console.log('Operational section:', data.operational);
                    if (data.operational) {{
                        console.log('Performance data:', data.operational.performance);
                        console.log('Network data:', data.operational.network);
                        console.log('Uptime data:', data.operational.uptime);
                    }}
                    this.updateDashboard(data);
                }} catch (error) {{
                    console.error('Failed to load data:', error);
                    // Only show error if we don't have any data yet
                    if (!this.lastValidData) {{
                        this.showError('Fehler beim Laden der Dashboard-Daten - Wiederholung in 90 Sekunden...');
                    }} else {{
                        console.log('Using cached data due to fetch error');
                    }}
                }}
            }}
            
            updateDashboard(data) {{
                console.log('Dashboard data received:', data);
                
                // Robust data extraction
                let connectionStatus = 'Disconnected';
                let deviceMatrix = [];
                let stats = {{}};
                
                // Try multiple data structure formats
                if (data) {{
                    // Format 1: New nested format
                    if (data.realtime && data.devices) {{
                        connectionStatus = data.realtime.system_health?.connection_status || 'Disconnected';
                        deviceMatrix = data.devices.device_matrix || [];
                        stats = data.operational?.statistics || {{}};
                        console.log('Using new nested format');
                    }}
                    // Format 2: Old flat format
                    else if (data.device_matrix) {{
                        connectionStatus = data.connection_status || 'Disconnected';
                        deviceMatrix = data.device_matrix || [];
                        stats = data.statistics || {{}};
                        console.log('Using old flat format');
                    }}
                    // Format 3: Direct device list
                    else if (Array.isArray(data)) {{
                        deviceMatrix = data;
                        console.log('Using direct array format');
                    }}
                }}
                
                console.log('Extracted data:', {{ 
                    connectionStatus, 
                    deviceMatrixLength: deviceMatrix.length,
                    stats 
                }});
                
                // Update connection status immediately
                this.updateConnectionStatus(connectionStatus);
                
                // Only update rooms/devices if we have valid data, otherwise preserve existing data
                if (deviceMatrix && deviceMatrix.length > 0) {{
                    this.updateStats({{ statistics: stats, device_matrix: deviceMatrix, operational: data.operational }});
                    this.updateRooms(deviceMatrix);
                    this.lastValidData = {{ deviceMatrix, stats, operational: data.operational }}; // Store for fallback
                }} else {{
                    console.warn('No valid device matrix found in data');
                    // Use last valid data if available, otherwise show loading message
                    if (this.lastValidData) {{
                        console.log('Using cached data to prevent flicker');
                        this.updateStats({{ statistics: this.lastValidData.stats, device_matrix: this.lastValidData.deviceMatrix, operational: this.lastValidData.operational }});
                        this.updateRooms(this.lastValidData.deviceMatrix);
                    }} else if (document.getElementById('roomsContainer').innerHTML.includes('loading')) {{
                        this.showError('Keine Gerätedaten verfügbar - Verbindung wird hergestellt...');
                    }}
                }}
            }}
            
            updateConnectionStatus(status) {{
                const statusEl = document.getElementById('connectionStatus');
                const indicator = statusEl.querySelector('.status-indicator');
                
                indicator.className = 'status-indicator';
                
                if (status === 'Connected') {{
                    indicator.classList.add('status-connected');
                    statusEl.innerHTML = '<span class="status-indicator status-connected"></span>Verbunden';
                }} else {{
                    indicator.classList.add('status-disconnected');
                    statusEl.innerHTML = '<span class="status-indicator status-disconnected"></span>Getrennt';
                }}
            }}
            
            updateStats(data) {{
                document.getElementById('totalRooms').textContent = data.device_matrix?.length || 0;
                document.getElementById('totalDevices').textContent = data.statistics?.total_devices || 0;
                document.getElementById('activeDevices').textContent = data.statistics?.active_devices || 0;
                document.getElementById('lastUpdate').textContent = new Date().toLocaleTimeString('de-DE');
                
                // Update server performance metrics
                this.updateServerMetrics(data);
            }}
            
            updateServerMetrics(data) {{
                // Debug: log the full data structure to understand format
                console.log('Full dashboard data structure:', data);
                console.log('Operational section:', data.operational);
                
                // Extract server metrics from the operational section
                const performance = data.operational?.performance || {{}};
                const network = data.operational?.network || {{}};
                const uptime = data.operational?.uptime || {{}};
                const mcp = data.operational?.mcp || {{}};
                
                console.log('Extracted sections:', {{ performance, network, uptime, mcp }});
                
                // Update CPU usage
                const cpuUsage = performance.cpu_usage || 0;
                document.getElementById('cpuUsage').textContent = `${{cpuUsage.toFixed(1)}}%`;
                
                // Update memory usage (check both possible field names)
                const memoryUsage = performance.memory_usage || performance.memory_usage_percent || 0;
                document.getElementById('memoryUsage').textContent = `${{memoryUsage.toFixed(1)}}%`;
                
                // Update requests per minute
                const requestsPerMin = network.requests_per_minute || 0;
                document.getElementById('requestsPerMin').textContent = Math.round(requestsPerMin);
                
                // Update average response time (check both possible field names)
                const avgResponseTime = network.response_time || network.average_response_time_ms || 0;
                document.getElementById('avgResponseTime').textContent = `${{avgResponseTime.toFixed(0)}}ms`;
                
                // Update uptime
                const uptimeFormatted = uptime.uptime_formatted || 'Unknown';
                document.getElementById('uptime').textContent = uptimeFormatted;
                
                // Update tools executed
                const toolsExecuted = mcp.tools_executed || 0;
                document.getElementById('toolsExecuted').textContent = toolsExecuted;
                
                // Debug log to console
                console.log('Server metrics updated:', {{
                    cpu: cpuUsage,
                    memory: memoryUsage,
                    requests: requestsPerMin,
                    response: avgResponseTime,
                    uptime: uptimeFormatted,
                    tools: toolsExecuted
                }});
            }}
            
            updateRooms(deviceMatrix) {{
                const container = document.getElementById('roomsContainer');
                
                if (!deviceMatrix || deviceMatrix.length === 0) {{
                    // Only show empty state if container doesn't already have content
                    if (!container.innerHTML || container.innerHTML.includes('loading')) {{
                        container.innerHTML = '<div class="empty-state">Keine Raumdaten verfügbar</div>';
                    }}
                    return;
                }}
                
                const roomsHtml = deviceMatrix.map(room => {{
                    const devices = room.devices || [];
                    const activeDevices = devices.filter(d => 
                        d.status_color === 'green' || 
                        (d.states?.active && d.states.active > 0)
                    ).length;
                    
                    // Group devices by category
                    const categories = {{}};
                    devices.forEach(device => {{
                        const category = this.getDeviceCategory(device.device_type);
                        if (!categories[category]) {{
                            categories[category] = [];
                        }}
                        categories[category].push(device);
                    }});
                    
                    const categoriesHtml = Object.entries(categories).map(([categoryName, categoryDevices]) => {{
                        const icon = this.getCategoryIcon(categoryName);
                        const devicesHtml = categoryDevices.map(device => {{
                            const isActive = device.status_color === 'green' || 
                                           (device.states?.active && device.states.active > 0);
                            const statusClass = isActive ? 'status-on' : 'status-off';
                            const statusText = device.state_display || (isActive ? 'Ein' : 'Aus');
                            
                            return `
                                <div class="device-item ${{isActive ? 'active' : ''}}">
                                    <span class="device-name">${{device.name}}</span>
                                    <span class="device-status ${{statusClass}}">${{statusText}}</span>
                                </div>
                            `;
                        }}).join('');
                        
                        return `
                            <div class="device-category">
                                <div class="category-header">
                                    <span class="category-icon">${{icon}}</span>
                                    <span>${{categoryName}} (${{categoryDevices.length}})</span>
                                </div>
                                <div class="devices-list">
                                    ${{devicesHtml}}
                                </div>
                            </div>
                        `;
                    }}).join('');
                    
                    return `
                        <div class="room-card">
                            <div class="room-header">
                                <h3 class="room-name">${{room.room_name}}</h3>
                                <div class="room-info">
                                    ${{devices.length}} Geräte • ${{activeDevices}} aktiv
                                </div>
                            </div>
                            <div class="room-body">
                                ${{categoriesHtml || '<div class="empty-state">Keine Geräte</div>'}}
                            </div>
                        </div>
                    `;
                }}).join('');
                
                container.innerHTML = `<div class="rooms-grid">${{roomsHtml}}</div>`;
            }}
            
            getDeviceCategory(deviceType) {{
                if (!deviceType) return 'Sonstiges';
                
                const type = deviceType.toLowerCase();
                if (type.includes('light') || type.includes('dimmer')) return 'Beleuchtung';
                if (type.includes('jalousie') || type.includes('blind')) return 'Beschattung';
                if (type.includes('temperature') || type.includes('climate')) return 'Klima';
                if (type.includes('sensor') || type.includes('analog')) return 'Sensoren';
                if (type.includes('switch') || type.includes('pushbutton')) return 'Schalter';
                if (type.includes('alarm') || type.includes('smoke')) return 'Sicherheit';
                return 'Sonstiges';
            }}
            
            getCategoryIcon(category) {{
                const icons = {{
                    'Beleuchtung': '💡',
                    'Beschattung': '🪟',
                    'Klima': '🌡️',
                    'Sensoren': '📊',
                    'Schalter': '🔘',
                    'Sicherheit': '🔒',
                    'Sonstiges': '⚙️'
                }};
                return icons[category] || '⚙️';
            }}
            
            connectWebSocket() {{
                // Enable WebSocket for real-time updates
                console.log('Connecting to WebSocket for real-time updates...');
                
                const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
                const wsUrl = `${{protocol}}//${{window.location.host}}/dashboard/ws`;
                
                try {{
                    this.ws = new WebSocket(wsUrl);
                    
                    this.ws.onopen = () => {{
                        console.log('WebSocket connected');
                        this.reconnectAttempts = 0;
                    }};
                    
                    this.ws.onmessage = (event) => {{
                        try {{
                            const message = JSON.parse(event.data);
                            console.log('WebSocket message received:', message);
                            
                            // Handle different message types
                            if (message.update_type === 'FullRefresh' && message.data) {{
                                // Full dashboard refresh with nested data
                                this.updateDashboard(message.data);
                            }} else if (message.device_matrix || (message.devices && message.devices.device_matrix)) {{
                                // Direct data update
                                this.updateDashboard(message);
                            }} else if (message.data) {{
                                // Wrapped data update
                                this.updateDashboard(message.data);
                            }}
                        }} catch (error) {{
                            console.error('WebSocket message error:', error);
                        }}
                    }};
                    
                    this.ws.onclose = () => {{
                        console.log('WebSocket disconnected');
                        this.scheduleReconnect();
                    }};
                    
                    this.ws.onerror = (error) => {{
                        console.error('WebSocket error:', error);
                    }};
                }} catch (error) {{
                    console.error('WebSocket connection failed:', error);
                }}
            }}
            
            scheduleReconnect() {{
                if (this.reconnectAttempts < this.maxReconnectAttempts) {{
                    setTimeout(() => {{
                        this.reconnectAttempts++;
                        this.connectWebSocket();
                    }}, this.reconnectInterval);
                }}
            }}
            
            showError(message) {{
                const container = document.getElementById('roomsContainer');
                container.innerHTML = `<div class="error">${{message}}</div>`;
            }}
        }}
        
        // Initialize dashboard when page loads
        document.addEventListener('DOMContentLoaded', () => {{
            new CleanDashboard();
        }});
    </script>
</body>
</html>"#,
        shared_styles = get_shared_styles(),
        nav_header = get_nav_header("Loxone Dashboard", true)
    )
}
