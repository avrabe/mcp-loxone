//! Admin UI for API key management
//!
//! This module provides a web interface for managing API keys.

use axum::response::{Html, IntoResponse};
use crate::shared_styles;

/// Handler for the API key management UI
pub async fn api_keys_ui() -> impl IntoResponse {
    Html(generate_api_keys_html())
}

/// Generate the API key management HTML page
fn generate_api_keys_html() -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>API Key Management - Loxone MCP Server</title>
    <style>
        {}
        
        /* Additional styles for key management */
        .key-table {{
            width: 100%;
            border-collapse: collapse;
            margin-top: 20px;
        }}
        
        .key-table th,
        .key-table td {{
            padding: 12px;
            text-align: left;
            border-bottom: 1px solid var(--border);
        }}
        
        .key-table th {{
            background-color: var(--card-bg);
            font-weight: 600;
        }}
        
        .key-table tr:hover {{
            background-color: var(--card-bg);
        }}
        
        .key-secret {{
            font-family: 'Courier New', monospace;
            background-color: var(--bg-secondary);
            padding: 4px 8px;
            border-radius: 4px;
            font-size: 0.9em;
        }}
        
        .role-badge {{
            display: inline-block;
            padding: 4px 12px;
            border-radius: 16px;
            font-size: 0.85em;
            font-weight: 500;
        }}
        
        .role-admin {{
            background-color: #dc3545;
            color: white;
        }}
        
        .role-operator {{
            background-color: #28a745;
            color: white;
        }}
        
        .role-monitor {{
            background-color: #17a2b8;
            color: white;
        }}
        
        .role-device {{
            background-color: #ffc107;
            color: #333;
        }}
        
        .role-custom {{
            background-color: #6c757d;
            color: white;
        }}
        
        .active-yes {{
            color: #28a745;
        }}
        
        .active-no {{
            color: #dc3545;
        }}
        
        .action-buttons {{
            display: flex;
            gap: 8px;
        }}
        
        .btn {{
            padding: 6px 12px;
            border: none;
            border-radius: 4px;
            cursor: pointer;
            font-size: 0.9em;
            transition: all 0.2s ease;
        }}
        
        .btn-primary {{
            background-color: #007bff;
            color: white;
        }}
        
        .btn-primary:hover {{
            background-color: #0056b3;
        }}
        
        .btn-danger {{
            background-color: #dc3545;
            color: white;
        }}
        
        .btn-danger:hover {{
            background-color: #c82333;
        }}
        
        .btn-secondary {{
            background-color: #6c757d;
            color: white;
        }}
        
        .btn-secondary:hover {{
            background-color: #545b62;
        }}
        
        .modal {{
            display: none;
            position: fixed;
            z-index: 1000;
            left: 0;
            top: 0;
            width: 100%;
            height: 100%;
            background-color: rgba(0,0,0,0.5);
        }}
        
        .modal-content {{
            background-color: var(--bg);
            margin: 10% auto;
            padding: 20px;
            border: 1px solid var(--border);
            border-radius: 8px;
            width: 80%;
            max-width: 500px;
        }}
        
        .form-group {{
            margin-bottom: 15px;
        }}
        
        .form-group label {{
            display: block;
            margin-bottom: 5px;
            font-weight: 500;
        }}
        
        .form-group input,
        .form-group select,
        .form-group textarea {{
            width: 100%;
            padding: 8px;
            border: 1px solid var(--border);
            border-radius: 4px;
            background-color: var(--bg);
            color: var(--text);
        }}
        
        .stats-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }}
        
        .stat-card {{
            background-color: var(--card-bg);
            padding: 20px;
            border-radius: 8px;
            border: 1px solid var(--border);
            text-align: center;
        }}
        
        .stat-value {{
            font-size: 2em;
            font-weight: bold;
            color: var(--primary);
        }}
        
        .stat-label {{
            color: var(--text-secondary);
            margin-top: 5px;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🔑 API Key Management</h1>
            <nav>
                <a href="/admin">← Back to Admin</a>
            </nav>
        </div>
        
        <div class="stats-grid" id="stats">
            <div class="stat-card">
                <div class="stat-value" id="totalKeys">-</div>
                <div class="stat-label">Total Keys</div>
            </div>
            <div class="stat-card">
                <div class="stat-value" id="activeKeys">-</div>
                <div class="stat-label">Active Keys</div>
            </div>
            <div class="stat-card">
                <div class="stat-value" id="expiredKeys">-</div>
                <div class="stat-label">Expired Keys</div>
            </div>
            <div class="stat-card">
                <div class="stat-value" id="blockedIps">-</div>
                <div class="stat-label">Blocked IPs</div>
            </div>
        </div>
        
        <div class="card">
            <div class="card-header">
                <h2>API Keys</h2>
                <button class="btn btn-primary" onclick="showCreateModal()">+ Create New Key</button>
            </div>
            
            <div id="keysContainer">
                <p>Loading API keys...</p>
            </div>
        </div>
        
        <!-- Create Key Modal -->
        <div id="createModal" class="modal">
            <div class="modal-content">
                <h2>Create New API Key</h2>
                <form id="createForm">
                    <div class="form-group">
                        <label for="name">Name</label>
                        <input type="text" id="name" name="name" required>
                    </div>
                    
                    <div class="form-group">
                        <label for="role">Role</label>
                        <select id="role" name="role" onchange="handleRoleChange()">
                            <option value="admin">Admin</option>
                            <option value="operator" selected>Operator</option>
                            <option value="monitor">Monitor</option>
                            <option value="device">Device</option>
                            <option value="custom">Custom</option>
                        </select>
                    </div>
                    
                    <div class="form-group" id="devicesGroup" style="display: none;">
                        <label for="devices">Allowed Devices (comma-separated UUIDs)</label>
                        <textarea id="devices" name="devices" rows="3"></textarea>
                    </div>
                    
                    <div class="form-group" id="permissionsGroup" style="display: none;">
                        <label for="permissions">Permissions (comma-separated)</label>
                        <textarea id="permissions" name="permissions" rows="3"></textarea>
                    </div>
                    
                    <div class="form-group">
                        <label for="expiresDays">Expires in (days, 0 = never)</label>
                        <input type="number" id="expiresDays" name="expiresDays" value="365" min="0">
                    </div>
                    
                    <div class="form-group">
                        <label for="ipWhitelist">IP Whitelist (comma-separated, empty = all)</label>
                        <input type="text" id="ipWhitelist" name="ipWhitelist" placeholder="192.168.1.1, 10.0.0.0/24">
                    </div>
                    
                    <div class="action-buttons">
                        <button type="submit" class="btn btn-primary">Create Key</button>
                        <button type="button" class="btn btn-secondary" onclick="hideCreateModal()">Cancel</button>
                    </div>
                </form>
            </div>
        </div>
        
        <!-- Key Created Modal -->
        <div id="keyCreatedModal" class="modal">
            <div class="modal-content">
                <h2>✅ API Key Created Successfully</h2>
                <div class="alert alert-warning">
                    <strong>⚠️ Important:</strong> Save this API key now. You won't be able to see it again!
                </div>
                <div class="form-group">
                    <label>API Key Secret:</label>
                    <div class="key-secret" id="newKeySecret" style="word-break: break-all;"></div>
                </div>
                <div class="form-group">
                    <button class="btn btn-primary" onclick="copyToClipboard()">Copy to Clipboard</button>
                    <button class="btn btn-secondary" onclick="hideKeyCreatedModal()">Close</button>
                </div>
            </div>
        </div>
    </div>
    
    <script>
        let currentKeys = [];
        
        async function loadKeys() {{
            try {{
                const response = await fetch('/admin/api/keys');
                if (!response.ok) throw new Error('Failed to load keys');
                
                const data = await response.json();
                currentKeys = data.keys;
                renderKeys();
            }} catch (error) {{
                console.error('Error loading keys:', error);
                document.getElementById('keysContainer').innerHTML = 
                    '<p class="error">Failed to load API keys. Check authentication.</p>';
            }}
        }}
        
        async function loadStats() {{
            try {{
                const response = await fetch('/admin/api/keys/stats');
                if (!response.ok) throw new Error('Failed to load stats');
                
                const stats = await response.json();
                document.getElementById('totalKeys').textContent = stats.total_keys;
                document.getElementById('activeKeys').textContent = stats.active_keys;
                document.getElementById('expiredKeys').textContent = stats.expired_keys;
                document.getElementById('blockedIps').textContent = stats.blocked_ips;
            }} catch (error) {{
                console.error('Error loading stats:', error);
            }}
        }}
        
        function renderKeys() {{
            if (currentKeys.length === 0) {{
                document.getElementById('keysContainer').innerHTML = 
                    '<p>No API keys found. Create one to get started.</p>';
                return;
            }}
            
            let html = `<table class="key-table">
                <thead>
                    <tr>
                        <th>Name</th>
                        <th>Role</th>
                        <th>Active</th>
                        <th>Created</th>
                        <th>Expires</th>
                        <th>Usage</th>
                        <th>Actions</th>
                    </tr>
                </thead>
                <tbody>`;
            
            for (const key of currentKeys) {{
                const role = key.role.type || 'unknown';
                const roleClass = `role-${{role}}`;
                const activeClass = key.active ? 'active-yes' : 'active-no';
                const activeText = key.active ? 'Yes' : 'No';
                const createdDate = new Date(key.created_at).toLocaleDateString();
                const expiresDate = key.expires_at 
                    ? new Date(key.expires_at).toLocaleDateString()
                    : 'Never';
                
                html += `<tr>
                    <td>${{key.name}}</td>
                    <td><span class="role-badge ${{roleClass}}">${{role.toUpperCase()}}</span></td>
                    <td><span class="${{activeClass}}">${{activeText}}</span></td>
                    <td>${{createdDate}}</td>
                    <td>${{expiresDate}}</td>
                    <td>${{key.usage_count}}</td>
                    <td>
                        <div class="action-buttons">
                            <button class="btn btn-secondary btn-sm" onclick="toggleKey('${{key.id}}', ${{!key.active}})">
                                ${{key.active ? 'Deactivate' : 'Activate'}}
                            </button>
                            <button class="btn btn-danger btn-sm" onclick="deleteKey('${{key.id}}')">Delete</button>
                        </div>
                    </td>
                </tr>`;
            }}
            
            html += '</tbody></table>';
            document.getElementById('keysContainer').innerHTML = html;
        }}
        
        function showCreateModal() {{
            document.getElementById('createModal').style.display = 'block';
        }}
        
        function hideCreateModal() {{
            document.getElementById('createModal').style.display = 'none';
            document.getElementById('createForm').reset();
        }}
        
        function hideKeyCreatedModal() {{
            document.getElementById('keyCreatedModal').style.display = 'none';
        }}
        
        function handleRoleChange() {{
            const role = document.getElementById('role').value;
            document.getElementById('devicesGroup').style.display = 
                role === 'device' ? 'block' : 'none';
            document.getElementById('permissionsGroup').style.display = 
                role === 'custom' ? 'block' : 'none';
        }}
        
        async function createKey(event) {{
            event.preventDefault();
            
            const formData = new FormData(event.target);
            const role = formData.get('role');
            let roleObj = {{ type: role }};
            
            if (role === 'device') {{
                const devices = formData.get('devices')
                    .split(',')
                    .map(d => d.trim())
                    .filter(d => d);
                roleObj = {{ type: 'device', allowed_devices: devices }};
            }} else if (role === 'custom') {{
                const permissions = formData.get('permissions')
                    .split(',')
                    .map(p => p.trim())
                    .filter(p => p);
                roleObj = {{ type: 'custom', permissions: permissions }};
            }}
            
            const ipWhitelist = formData.get('ipWhitelist')
                .split(',')
                .map(ip => ip.trim())
                .filter(ip => ip);
            
            const payload = {{
                name: formData.get('name'),
                role: roleObj,
                expires_days: parseInt(formData.get('expiresDays')) || null,
                ip_whitelist: ipWhitelist
            }};
            
            try {{
                const response = await fetch('/admin/api/keys', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify(payload)
                }});
                
                if (!response.ok) throw new Error('Failed to create key');
                
                const result = await response.json();
                
                // Show the key secret
                document.getElementById('newKeySecret').textContent = result.secret;
                hideCreateModal();
                document.getElementById('keyCreatedModal').style.display = 'block';
                
                // Reload keys and stats
                await Promise.all([loadKeys(), loadStats()]);
            }} catch (error) {{
                console.error('Error creating key:', error);
                alert('Failed to create API key. Check the console for details.');
            }}
        }}
        
        async function toggleKey(keyId, activate) {{
            try {{
                const response = await fetch(`/admin/api/keys/${{keyId}}`, {{
                    method: 'PUT',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ active: activate }})
                }});
                
                if (!response.ok) throw new Error('Failed to update key');
                
                await Promise.all([loadKeys(), loadStats()]);
            }} catch (error) {{
                console.error('Error updating key:', error);
                alert('Failed to update API key.');
            }}
        }}
        
        async function deleteKey(keyId) {{
            if (!confirm('Are you sure you want to delete this API key?')) {{
                return;
            }}
            
            try {{
                const response = await fetch(`/admin/api/keys/${{keyId}}`, {{
                    method: 'DELETE'
                }});
                
                if (!response.ok) throw new Error('Failed to delete key');
                
                await Promise.all([loadKeys(), loadStats()]);
            }} catch (error) {{
                console.error('Error deleting key:', error);
                alert('Failed to delete API key.');
            }}
        }}
        
        function copyToClipboard() {{
            const secret = document.getElementById('newKeySecret').textContent;
            navigator.clipboard.writeText(secret).then(() => {{
                alert('API key copied to clipboard!');
            }}).catch(err => {{
                console.error('Failed to copy:', err);
            }});
        }}
        
        // Initialize
        document.getElementById('createForm').addEventListener('submit', createKey);
        
        // Load data on page load
        Promise.all([loadKeys(), loadStats()]);
        
        // Refresh data every 30 seconds
        setInterval(() => {{
            loadStats();
        }}, 30000);
    </script>
</body>
</html>"#,
        shared_styles::get_shared_styles()
    )
}