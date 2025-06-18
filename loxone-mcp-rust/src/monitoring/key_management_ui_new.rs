//! Web UI for API Key Management with new styling

use crate::shared_styles::{get_nav_header, get_shared_styles};

/// Generate the HTML for the key management UI
pub fn generate_html() -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>API Key Management - Loxone MCP</title>
    {}
    <style>
        /* Key management specific styles */
        .keys-container {{
            display: grid;
            gap: calc(var(--spacing-unit) * 2);
            margin-top: calc(var(--spacing-unit) * 3);
        }}
        
        .key-card {{
            background: var(--bg-secondary);
            border-radius: var(--border-radius);
            padding: calc(var(--spacing-unit) * 3);
            border: 1px solid var(--border-color);
            transition: all var(--transition-fast);
        }}
        
        .key-card:hover {{
            border-color: var(--accent-primary);
            transform: translateY(-2px);
            box-shadow: 0 4px 20px var(--shadow-color);
        }}
        
        .key-header {{
            display: flex;
            justify-content: space-between;
            align-items: start;
            margin-bottom: calc(var(--spacing-unit) * 2);
        }}
        
        .key-info {{
            flex: 1;
        }}
        
        .key-name {{
            font-size: 1.25rem;
            font-weight: 700;
            margin-bottom: calc(var(--spacing-unit) * 0.5);
        }}
        
        .key-id {{
            font-family: 'SF Mono', Monaco, 'Courier New', monospace;
            font-size: 0.875rem;
            color: var(--text-secondary);
            word-break: break-all;
        }}
        
        .key-role {{
            display: inline-flex;
            align-items: center;
            padding: calc(var(--spacing-unit) * 0.5) calc(var(--spacing-unit) * 1.5);
            border-radius: 20px;
            font-size: 0.875rem;
            font-weight: 600;
        }}
        
        .key-role.admin {{
            background: linear-gradient(135deg, hsl(14, 90%, 50%), hsl(24, 90%, 50%));
            color: white;
        }}
        
        .key-role.operator {{
            background: linear-gradient(135deg, var(--accent-primary), hsl(calc(var(--accent-hue) + 20), 70%, 50%));
            color: white;
        }}
        
        .key-role.monitor {{
            background: linear-gradient(135deg, hsl(200, 70%, 50%), hsl(220, 70%, 50%));
            color: white;
        }}
        
        .key-role.device {{
            background: linear-gradient(135deg, var(--warning-color), hsl(45, 90%, 50%));
            color: white;
        }}
        
        .key-details {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: calc(var(--spacing-unit) * 2);
            padding-top: calc(var(--spacing-unit) * 2);
            border-top: 1px solid var(--border-color);
        }}
        
        .detail-item {{
            font-size: 0.875rem;
        }}
        
        .detail-label {{
            color: var(--text-secondary);
            margin-bottom: calc(var(--spacing-unit) * 0.5);
            font-weight: 600;
        }}
        
        .detail-value {{
            color: var(--text-primary);
        }}
        
        .key-actions {{
            display: flex;
            gap: calc(var(--spacing-unit) * 1);
            margin-top: calc(var(--spacing-unit) * 2);
        }}
        
        .modal {{
            display: none;
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background: rgba(0, 0, 0, 0.8);
            backdrop-filter: blur(10px);
            z-index: 1000;
        }}
        
        .modal-content {{
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            background: var(--bg-secondary);
            border: 1px solid var(--border-color);
            border-radius: var(--border-radius);
            padding: calc(var(--spacing-unit) * 4);
            width: 90%;
            max-width: 600px;
            max-height: 90vh;
            overflow-y: auto;
            box-shadow: 0 20px 60px var(--shadow-color);
        }}
        
        .modal-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: calc(var(--spacing-unit) * 3);
        }}
        
        .modal-title {{
            font-size: 1.5rem;
            font-weight: 700;
        }}
        
        .close-btn {{
            background: none;
            border: none;
            color: var(--text-secondary);
            font-size: 1.5rem;
            cursor: pointer;
            padding: calc(var(--spacing-unit) * 1);
            border-radius: 8px;
            transition: all var(--transition-fast);
        }}
        
        .close-btn:hover {{
            background: var(--bg-primary);
            color: var(--text-primary);
        }}
        
        .form-row {{
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: calc(var(--spacing-unit) * 2);
        }}
        
        .form-row.full {{
            grid-template-columns: 1fr;
        }}
        
        .ip-list {{
            display: flex;
            flex-wrap: wrap;
            gap: calc(var(--spacing-unit) * 1);
            margin-top: calc(var(--spacing-unit) * 1);
        }}
        
        .ip-tag {{
            background: var(--bg-primary);
            padding: calc(var(--spacing-unit) * 0.5) calc(var(--spacing-unit) * 1.5);
            border-radius: 20px;
            font-size: 0.875rem;
            display: flex;
            align-items: center;
            gap: calc(var(--spacing-unit) * 0.5);
            border: 1px solid var(--border-color);
        }}
        
        .remove-ip {{
            background: none;
            border: none;
            color: var(--text-secondary);
            cursor: pointer;
            font-size: 1.2rem;
            line-height: 1;
            transition: color var(--transition-fast);
        }}
        
        .remove-ip:hover {{
            color: var(--error-color);
        }}
        
        .empty-state {{
            text-align: center;
            padding: calc(var(--spacing-unit) * 8);
            color: var(--text-secondary);
        }}
        
        .empty-state-icon {{
            font-size: 4rem;
            margin-bottom: calc(var(--spacing-unit) * 2);
            opacity: 0.5;
        }}
        
        .empty-state-title {{
            font-size: 1.25rem;
            font-weight: 600;
            margin-bottom: calc(var(--spacing-unit) * 1);
        }}
        
        .empty-state-text {{
            margin-bottom: calc(var(--spacing-unit) * 3);
        }}
    </style>
</head>
<body>
    {}
    
    <div class="container">
        <div class="card">
            <div class="card-header">
                <div>
                    <h2 class="card-title">API Key Management</h2>
                    <p class="text-muted">Manage access keys for the Loxone MCP server</p>
                </div>
                <button class="button" onclick="showNewKeyModal()">
                    <span>🔑</span>
                    <span>Generate New Key</span>
                </button>
            </div>
            
            <div id="keysContainer" class="keys-container">
                <div class="loading">Loading keys...</div>
            </div>
        </div>
    </div>
    
    <!-- New Key Modal -->
    <div id="newKeyModal" class="modal">
        <div class="modal-content">
            <div class="modal-header">
                <h3 class="modal-title">Generate New API Key</h3>
                <button class="close-btn" onclick="hideNewKeyModal()">×</button>
            </div>
            
            <form id="newKeyForm" onsubmit="generateKey(event)">
                <div class="form-row">
                    <div class="form-group">
                        <label class="form-label" for="keyName">Key Name</label>
                        <input type="text" id="keyName" class="form-input" required 
                               placeholder="e.g., Home Assistant Integration">
                    </div>
                    
                    <div class="form-group">
                        <label class="form-label" for="keyRole">Role</label>
                        <select id="keyRole" class="form-input" required onchange="handleRoleChange()">
                            <option value="monitor">Monitor (Read-only)</option>
                            <option value="operator">Operator (Read/Write)</option>
                            <option value="admin">Admin (Full Access)</option>
                            <option value="device">Device (Limited)</option>
                        </select>
                    </div>
                </div>
                
                <div class="form-row">
                    <div class="form-group">
                        <label class="form-label" for="expiresIn">Expires In (days)</label>
                        <input type="number" id="expiresIn" class="form-input" 
                               placeholder="Leave empty for no expiry" min="1">
                    </div>
                    
                    <div class="form-group">
                        <label class="form-label" for="ipWhitelist">IP Whitelist</label>
                        <input type="text" id="ipInput" class="form-input" 
                               placeholder="e.g., 192.168.1.100">
                        <div id="ipList" class="ip-list"></div>
                    </div>
                </div>
                
                <div id="deviceSection" class="form-row full" style="display: none;">
                    <div class="form-group">
                        <label class="form-label" for="devices">Allowed Devices</label>
                        <textarea id="devices" class="form-input" rows="3" 
                                  placeholder="Enter device UUIDs, one per line"></textarea>
                    </div>
                </div>
                
                <div class="form-row full">
                    <button type="submit" class="button">
                        <span>✨</span>
                        <span>Generate Key</span>
                    </button>
                </div>
            </form>
        </div>
    </div>
    
    <script>
        let ipWhitelist = [];
        
        // Load keys on page load
        document.addEventListener('DOMContentLoaded', loadKeys);
        
        async function loadKeys() {{
            try {{
                const response = await fetch('/admin/keys/api/list');
                const keys = await response.json();
                displayKeys(keys);
            }} catch (error) {{
                console.error('Failed to load keys:', error);
                document.getElementById('keysContainer').innerHTML = 
                    '<div class="error-message">Failed to load API keys</div>';
            }}
        }}
        
        function displayKeys(keys) {{
            const container = document.getElementById('keysContainer');
            
            if (keys.length === 0) {{
                container.innerHTML = `
                    <div class="empty-state">
                        <div class="empty-state-icon">🔐</div>
                        <h3 class="empty-state-title">No API Keys Yet</h3>
                        <p class="empty-state-text">Generate your first API key to get started</p>
                        <button class="button" onclick="showNewKeyModal()">
                            <span>🔑</span>
                            <span>Generate First Key</span>
                        </button>
                    </div>
                `;
                return;
            }}
            
            container.innerHTML = keys.map(key => `
                <div class="key-card">
                    <div class="key-header">
                        <div class="key-info">
                            <h3 class="key-name">${{key.name}}</h3>
                            <code class="key-id">${{key.id}}</code>
                        </div>
                        <span class="key-role ${{key.role.toLowerCase()}}">${{key.role}}</span>
                    </div>
                    
                    <div class="key-details">
                        <div class="detail-item">
                            <div class="detail-label">Status</div>
                            <div class="detail-value">
                                <span class="status-badge ${{key.active ? 'success' : 'error'}}">
                                    ${{key.active ? '🟢 Active' : '🔴 Inactive'}}
                                </span>
                            </div>
                        </div>
                        
                        <div class="detail-item">
                            <div class="detail-label">Created</div>
                            <div class="detail-value">${{key.created_at}}</div>
                        </div>
                        
                        <div class="detail-item">
                            <div class="detail-label">Expires</div>
                            <div class="detail-value">${{key.expires_at || 'Never'}}</div>
                        </div>
                        
                        <div class="detail-item">
                            <div class="detail-label">Usage</div>
                            <div class="detail-value">${{key.usage_count}} requests</div>
                        </div>
                        
                        ${{key.ip_whitelist && key.ip_whitelist.length > 0 ? `
                        <div class="detail-item">
                            <div class="detail-label">IP Whitelist</div>
                            <div class="detail-value">${{key.ip_whitelist.join(', ')}}</div>
                        </div>
                        ` : ''}}
                    </div>
                    
                    <div class="key-actions">
                        <button class="button secondary" onclick="toggleKey('${{key.id}}', ${{!key.active}})">
                            ${{key.active ? '⏸️ Deactivate' : '▶️ Activate'}}
                        </button>
                        <button class="button secondary" onclick="deleteKey('${{key.id}}')">
                            🗑️ Delete
                        </button>
                    </div>
                </div>
            `).join('');
        }}
        
        function showNewKeyModal() {{
            document.getElementById('newKeyModal').style.display = 'block';
        }}
        
        function hideNewKeyModal() {{
            document.getElementById('newKeyModal').style.display = 'none';
            document.getElementById('newKeyForm').reset();
            ipWhitelist = [];
            updateIpList();
        }}
        
        function handleRoleChange() {{
            const role = document.getElementById('keyRole').value;
            const deviceSection = document.getElementById('deviceSection');
            deviceSection.style.display = role === 'device' ? 'block' : 'none';
        }}
        
        // IP whitelist management
        document.getElementById('ipInput').addEventListener('keypress', function(e) {{
            if (e.key === 'Enter') {{
                e.preventDefault();
                const ip = this.value.trim();
                if (ip && !ipWhitelist.includes(ip)) {{
                    ipWhitelist.push(ip);
                    updateIpList();
                    this.value = '';
                }}
            }}
        }});
        
        function updateIpList() {{
            const ipListEl = document.getElementById('ipList');
            ipListEl.innerHTML = ipWhitelist.map(ip => `
                <div class="ip-tag">
                    ${{ip}}
                    <button class="remove-ip" onclick="removeIp('${{ip}}')">&times;</button>
                </div>
            `).join('');
        }}
        
        function removeIp(ip) {{
            ipWhitelist = ipWhitelist.filter(i => i !== ip);
            updateIpList();
        }}
        
        async function generateKey(event) {{
            event.preventDefault();
            
            const data = {{
                name: document.getElementById('keyName').value,
                role: document.getElementById('keyRole').value,
                expires_days: parseInt(document.getElementById('expiresIn').value) || 0,
                ip_whitelist: ipWhitelist.length > 0 ? ipWhitelist : null,
            }};
            
            if (data.role === 'device') {{
                const devices = document.getElementById('devices').value
                    .split('\n')
                    .map(d => d.trim())
                    .filter(d => d);
                if (devices.length > 0) {{
                    data.devices = devices;
                }}
            }}
            
            try {{
                const response = await fetch('/admin/keys/api/generate', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify(data)
                }});
                
                if (response.ok) {{
                    hideNewKeyModal();
                    loadKeys();
                }} else {{
                    alert('Failed to generate key');
                }}
            }} catch (error) {{
                console.error('Error generating key:', error);
                alert('Error generating key');
            }}
        }}
        
        async function toggleKey(keyId, activate) {{
            try {{
                const endpoint = activate ? 'activate' : 'deactivate';
                const response = await fetch(`/admin/keys/api/${{keyId}}/${{endpoint}}`, {{
                    method: 'POST'
                }});
                
                if (response.ok) {{
                    loadKeys();
                }}
            }} catch (error) {{
                console.error('Error toggling key:', error);
            }}
        }}
        
        async function deleteKey(keyId) {{
            if (!confirm('Are you sure you want to delete this API key?')) {{
                return;
            }}
            
            try {{
                const response = await fetch(`/admin/keys/api/${{keyId}}`, {{
                    method: 'DELETE'
                }});
                
                if (response.ok) {{
                    loadKeys();
                }}
            }} catch (error) {{
                console.error('Error deleting key:', error);
            }}
        }}
    </script>
</body>
</html>"#,
        get_shared_styles(),
        get_nav_header("API Key Management", true)
    )
}
