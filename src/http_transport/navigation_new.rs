//! New navigation page with shared styles

use crate::shared_styles::{get_api_key_preservation_script, get_nav_header, get_shared_styles};

/// Generate the main navigation hub HTML
pub fn generate_navigation_html() -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Loxone MCP Server - Admin Hub</title>
    {}
    {}
    <style>
        /* Navigation specific styles */
        .nav-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(350px, 1fr));
            gap: calc(var(--spacing-unit) * 3);
            margin-top: calc(var(--spacing-unit) * 3);
        }}

        .nav-category {{
            background: var(--bg-secondary);
            border-radius: var(--border-radius);
            padding: calc(var(--spacing-unit) * 3);
            border: 1px solid var(--border-color);
            transition: all var(--transition-fast);
        }}

        .nav-category:hover {{
            border-color: var(--accent-primary);
            transform: translateY(-4px);
            box-shadow: 0 8px 25px var(--shadow-color);
        }}

        .category-header {{
            display: flex;
            align-items: center;
            gap: calc(var(--spacing-unit) * 1.5);
            margin-bottom: calc(var(--spacing-unit) * 2);
        }}

        .category-icon {{
            width: 48px;
            height: 48px;
            background: linear-gradient(135deg, var(--accent-primary), hsl(calc(var(--accent-hue) + 30), 70%, 50%));
            border-radius: 12px;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 1.75rem;
            color: white;
            transition: transform var(--transition-fast);
        }}

        .nav-category:hover .category-icon {{
            transform: scale(1.1) rotate(5deg);
        }}

        .category-title {{
            font-size: 1.25rem;
            font-weight: 700;
            margin: 0;
        }}

        .nav-links {{
            display: flex;
            flex-direction: column;
            gap: calc(var(--spacing-unit) * 1);
        }}

        .nav-link {{
            display: flex;
            align-items: center;
            gap: calc(var(--spacing-unit) * 1.5);
            padding: calc(var(--spacing-unit) * 1.5);
            background: var(--bg-primary);
            border: 1px solid var(--border-color);
            border-radius: calc(var(--border-radius) / 2);
            color: var(--text-primary);
            text-decoration: none;
            transition: all var(--transition-fast);
            cursor: pointer;
        }}

        .nav-link:hover {{
            background: var(--bg-secondary);
            border-color: var(--accent-primary);
            color: var(--accent-primary);
            transform: translateX(4px);
        }}

        .link-icon {{
            font-size: 1.25rem;
            width: 28px;
            text-align: center;
            transition: transform var(--transition-fast);
        }}

        .nav-link:hover .link-icon {{
            transform: scale(1.2);
        }}

        .link-content {{
            flex: 1;
        }}

        .link-title {{
            font-weight: 600;
            margin-bottom: calc(var(--spacing-unit) * 0.25);
        }}

        .link-description {{
            font-size: 0.875rem;
            color: var(--text-secondary);
        }}

        .status-indicator {{
            width: 12px;
            height: 12px;
            border-radius: 50%;
            background: var(--success-color);
            position: relative;
            animation: pulse 2s infinite;
        }}

        @keyframes pulse {{
            0% {{ opacity: 1; }}
            50% {{ opacity: 0.5; }}
            100% {{ opacity: 1; }}
        }}

        .hero-section {{
            text-align: center;
            margin-bottom: calc(var(--spacing-unit) * 4);
            padding: calc(var(--spacing-unit) * 4);
            background: linear-gradient(135deg, var(--bg-secondary), var(--bg-primary));
            border-radius: var(--border-radius);
            border: 1px solid var(--border-color);
        }}

        .hero-title {{
            font-size: 2.5rem;
            font-weight: 700;
            margin-bottom: calc(var(--spacing-unit) * 1);
            background: linear-gradient(135deg, var(--accent-primary), hsl(calc(var(--accent-hue) + 40), 70%, 50%));
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
        }}

        .hero-subtitle {{
            font-size: 1.125rem;
            color: var(--text-secondary);
        }}

        .quick-stats {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
            gap: calc(var(--spacing-unit) * 2);
            margin-bottom: calc(var(--spacing-unit) * 4);
        }}

        .stat-card {{
            background: var(--bg-secondary);
            padding: calc(var(--spacing-unit) * 2);
            border-radius: calc(var(--border-radius) / 2);
            text-align: center;
            border: 1px solid var(--border-color);
            transition: all var(--transition-fast);
        }}

        .stat-card:hover {{
            border-color: var(--accent-primary);
            transform: translateY(-2px);
        }}

        .stat-value {{
            font-size: 1.75rem;
            font-weight: 700;
            color: var(--accent-primary);
        }}

        .stat-label {{
            font-size: 0.875rem;
            color: var(--text-secondary);
        }}

        .footer-info {{
            margin-top: calc(var(--spacing-unit) * 6);
            padding-top: calc(var(--spacing-unit) * 3);
            border-top: 1px solid var(--border-color);
            text-align: center;
            color: var(--text-secondary);
        }}

        .version-info {{
            font-size: 0.875rem;
            margin-bottom: calc(var(--spacing-unit) * 1);
        }}
    </style>
</head>
<body>
    {}

    <div class="container">
        <div class="hero-section">
            <h1 class="hero-title">Loxone MCP Server</h1>
            <p class="hero-subtitle">Central administration and monitoring hub</p>
        </div>

        <div class="quick-stats">
            <div class="stat-card">
                <div class="stat-value" id="statusValue">●</div>
                <div class="stat-label">System Status</div>
            </div>
            <div class="stat-card">
                <div class="stat-value" id="toolsValue">30+</div>
                <div class="stat-label">MCP Tools</div>
            </div>
            <div class="stat-card">
                <div class="stat-value" id="uptimeValue">--</div>
                <div class="stat-label">Uptime</div>
            </div>
            <div class="stat-card">
                <div class="stat-value" id="connectionsValue">--</div>
                <div class="stat-label">Active Connections</div>
            </div>
        </div>

        <div class="nav-grid">
            <div class="nav-category">
                <div class="category-header">
                    <div class="category-icon">📊</div>
                    <h2 class="category-title">Monitoring & Analytics</h2>
                </div>
                <div class="nav-links">
                    <a href="/dashboard/" class="nav-link">
                        <span class="link-icon">🏠</span>
                        <div class="link-content">
                            <div class="link-title">Live Dashboard</div>
                            <div class="link-description">Real-time room and device monitoring</div>
                        </div>
                        <div class="status-indicator"></div>
                    </a>
                    <a href="/history/" class="nav-link">
                        <span class="link-icon">📈</span>
                        <div class="link-content">
                            <div class="link-title">History Dashboard</div>
                            <div class="link-description">Historical data and trends</div>
                        </div>
                    </a>
                    <a href="/health" class="nav-link">
                        <span class="link-icon">💚</span>
                        <div class="link-content">
                            <div class="link-title">Health Check</div>
                            <div class="link-description">System status and diagnostics</div>
                        </div>
                    </a>
                </div>
            </div>

            <div class="nav-category">
                <div class="category-header">
                    <div class="category-icon">🔐</div>
                    <h2 class="category-title">Security & Access</h2>
                </div>
                <div class="nav-links">
                    <a href="/admin/keys" class="nav-link">
                        <span class="link-icon">🔑</span>
                        <div class="link-content">
                            <div class="link-title">API Key Management</div>
                            <div class="link-description">Generate and manage access keys</div>
                        </div>
                    </a>
                    <a href="/security/audit" class="nav-link">
                        <span class="link-icon">🛡️</span>
                        <div class="link-content">
                            <div class="link-title">Security Audit</div>
                            <div class="link-description">Review security configurations</div>
                        </div>
                    </a>
                    <a href="/security/headers" class="nav-link">
                        <span class="link-icon">🔒</span>
                        <div class="link-content">
                            <div class="link-title">Security Headers</div>
                            <div class="link-description">HTTP security header testing</div>
                        </div>
                    </a>
                </div>
            </div>

            <div class="nav-category">
                <div class="category-header">
                    <div class="category-icon">⚙️</div>
                    <h2 class="category-title">System Configuration</h2>
                </div>
                <div class="nav-links">
                    <a href="/api/tools" class="nav-link">
                        <span class="link-icon">🛠️</span>
                        <div class="link-content">
                            <div class="link-title">MCP Tools</div>
                            <div class="link-description">Available MCP tools and capabilities</div>
                        </div>
                    </a>
                    <a href="/api/resources" class="nav-link">
                        <span class="link-icon">📁</span>
                        <div class="link-content">
                            <div class="link-title">MCP Resources</div>
                            <div class="link-description">System resources and data</div>
                        </div>
                    </a>
                    <a href="/api/prompts" class="nav-link">
                        <span class="link-icon">💬</span>
                        <div class="link-content">
                            <div class="link-title">MCP Prompts</div>
                            <div class="link-description">Interactive prompt templates</div>
                        </div>
                    </a>
                </div>
            </div>

            <div class="nav-category">
                <div class="category-header">
                    <div class="category-icon">🔗</div>
                    <h2 class="category-title">API Endpoints</h2>
                </div>
                <div class="nav-links">
                    <a href="/message" class="nav-link">
                        <span class="link-icon">📨</span>
                        <div class="link-content">
                            <div class="link-title">MCP HTTP</div>
                            <div class="link-description">Model Context Protocol endpoint</div>
                        </div>
                    </a>
                    <a href="/sse" class="nav-link">
                        <span class="link-icon">📡</span>
                        <div class="link-content">
                            <div class="link-title">Server-Sent Events</div>
                            <div class="link-description">Real-time event streaming</div>
                        </div>
                    </a>
                    <a href="/" class="nav-link">
                        <span class="link-icon">📋</span>
                        <div class="link-content">
                            <div class="link-title">API Information</div>
                            <div class="link-description">Server info and documentation</div>
                        </div>
                    </a>
                </div>
            </div>
        </div>

        <div class="footer-info">
            <div class="version-info">
                Loxone MCP Server v1.0.0 | Built with Rust 🦀
            </div>
            <div>
                <a href="https://github.com/anthropics/claude-code" style="color: var(--accent-primary);">
                    Generated with Claude Code
                </a>
            </div>
        </div>
    </div>

    <script>
        // Load system stats
        async function loadStats() {{
            try {{
                // Get health status
                const response = await fetch('/health');
                const health = await response.json();

                document.getElementById('statusValue').textContent = health.status === 'ok' ? '🟢' : '🔴';

                // Get admin status for more details
                const adminResponse = await fetch('/admin/status');
                if (adminResponse.ok) {{
                    const adminData = await adminResponse.json();
                    document.getElementById('connectionsValue').textContent = adminData.connections || '0';
                }}

                // Calculate uptime (for now, just show server start time)
                const serverStartTime = sessionStorage.getItem('serverStartTime') || Date.now();
                if (!sessionStorage.getItem('serverStartTime')) {{
                    sessionStorage.setItem('serverStartTime', serverStartTime);
                }}
                const uptimeMs = Date.now() - parseInt(serverStartTime);
                const uptimeHours = Math.floor(uptimeMs / (1000 * 60 * 60));
                const uptimeMinutes = Math.floor((uptimeMs % (1000 * 60 * 60)) / (1000 * 60));
                document.getElementById('uptimeValue').textContent = uptimeHours > 0 ? `${{uptimeHours}}h ${{uptimeMinutes}}m` : `${{uptimeMinutes}}m`;

            }} catch (error) {{
                console.warn('Failed to load stats:', error);
                document.getElementById('statusValue').textContent = '🟡';
            }}
        }}

        // Load stats on page load
        document.addEventListener('DOMContentLoaded', loadStats);

        // Refresh stats every 30 seconds
        setInterval(loadStats, 30000);
    </script>
</body>
</html>"#,
        get_shared_styles(),
        get_api_key_preservation_script(),
        get_nav_header("Admin Hub", false)
    )
}
