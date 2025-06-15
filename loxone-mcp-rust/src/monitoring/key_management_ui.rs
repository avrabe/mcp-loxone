//! Web UI for API Key Management

use crate::error::Result;
use crate::security::key_store::{ApiKey, ApiKeyRole, KeyStore};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Key management UI controller
#[derive(Clone)]
pub struct KeyManagementUI {
    key_store: Arc<KeyStore>,
}

impl KeyManagementUI {
    /// Create new key management UI
    pub fn new(key_store: Arc<KeyStore>) -> Self {
        Self { key_store }
    }

    /// Render the main key management page
    pub async fn render_page(&self) -> Html<String> {
        Html(Self::generate_html())
    }

    /// API: List all keys
    pub async fn list_keys(&self) -> Result<Json<Vec<ApiKeyInfo>>> {
        let keys = self.key_store.list_keys().await;
        let key_infos: Vec<ApiKeyInfo> = keys
            .into_iter()
            .map(|key| ApiKeyInfo {
                id: key.id,
                name: key.name,
                role: format!("{:?}", key.role),
                created_by: key.created_by,
                created_at: key.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                expires_at: key
                    .expires_at
                    .map(|e| e.format("%Y-%m-%d %H:%M:%S").to_string()),
                active: key.active,
                ip_whitelist: key.ip_whitelist,
                last_used: key
                    .last_used
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
                usage_count: key.usage_count,
            })
            .collect();
        Ok(Json(key_infos))
    }

    /// API: Generate new key
    pub async fn generate_key(
        &self,
        request: Json<GenerateKeyRequest>,
    ) -> Result<Json<ApiKeyInfo>> {
        let role = match request.role.to_lowercase().as_str() {
            "admin" => ApiKeyRole::Admin,
            "operator" => ApiKeyRole::Operator,
            "monitor" => ApiKeyRole::Monitor,
            "device" => ApiKeyRole::Device {
                allowed_devices: request.devices.clone().unwrap_or_default(),
            },
            _ => return Err(crate::error::LoxoneError::config("Invalid role")),
        };

        let key_id = Self::generate_key_id(&role);
        let key = ApiKey {
            id: key_id.clone(),
            name: request.name.clone(),
            role,
            created_by: "Web UI".to_string(),
            created_at: Utc::now(),
            expires_at: request
                .expires_days
                .filter(|&days| days > 0)
                .map(|days| Utc::now() + Duration::days(days as i64)),
            ip_whitelist: request.ip_whitelist.clone().unwrap_or_default(),
            active: true,
            last_used: None,
            usage_count: 0,
            metadata: Default::default(),
        };

        self.key_store.add_key(key.clone()).await?;

        Ok(Json(ApiKeyInfo {
            id: key.id,
            name: key.name,
            role: format!("{:?}", key.role),
            created_by: key.created_by,
            created_at: key.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            expires_at: key
                .expires_at
                .map(|e| e.format("%Y-%m-%d %H:%M:%S").to_string()),
            active: key.active,
            ip_whitelist: key.ip_whitelist,
            last_used: None,
            usage_count: 0,
        }))
    }

    /// API: Update key
    pub async fn update_key(
        &self,
        key_id: Path<String>,
        request: Json<UpdateKeyRequest>,
    ) -> Result<StatusCode> {
        let mut key = self
            .key_store
            .get_key(&key_id)
            .await
            .ok_or_else(|| crate::error::LoxoneError::not_found("Key not found"))?;

        if let Some(name) = &request.name {
            key.name = name.clone();
        }
        if let Some(active) = request.active {
            key.active = active;
        }
        if let Some(expires_days) = request.expires_days {
            key.expires_at = if expires_days > 0 {
                Some(Utc::now() + Duration::days(expires_days as i64))
            } else {
                None
            };
        }
        if let Some(ip_whitelist) = &request.ip_whitelist {
            key.ip_whitelist = ip_whitelist.clone();
        }

        self.key_store.update_key(key).await?;
        Ok(StatusCode::OK)
    }

    /// API: Delete key
    pub async fn delete_key(&self, key_id: Path<String>) -> Result<StatusCode> {
        self.key_store.remove_key(&key_id).await?;
        Ok(StatusCode::OK)
    }

    /// Generate a new key ID
    fn generate_key_id(role: &ApiKeyRole) -> String {
        let role_prefix = match role {
            ApiKeyRole::Admin => "admin",
            ApiKeyRole::Operator => "operator",
            ApiKeyRole::Monitor => "monitor",
            ApiKeyRole::Device { .. } => "device",
            ApiKeyRole::Custom { .. } => "custom",
        };

        let seq = chrono::Utc::now().timestamp_millis() % 1000;
        let random = &Uuid::new_v4().to_string()[..12];

        format!("lmcp_{}_{:03}_{}", role_prefix, seq, random)
    }

    /// Generate the HTML for the key management UI
    fn generate_html() -> String {
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>API Key Management - Loxone MCP</title>
    <style>
        :root {
            --loxone-green: #7aba00;
            --loxone-dark: #1a1a1a;
            --bg-primary: #0f0f0f;
            --bg-secondary: #1a1a1a;
            --bg-card: #252525;
            --text-primary: #e0e0e0;
            --text-secondary: #a0a0a0;
            --border-color: #333;
            --rust-orange: #ce422b;
            --danger: #dc3545;
            --warning: #ffc107;
            --info: #17a2b8;
        }

        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            line-height: 1.6;
        }

        .container {
            max-width: 1400px;
            margin: 0 auto;
            padding: 2rem;
        }

        .header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 2rem;
            padding-bottom: 1rem;
            border-bottom: 2px solid var(--border-color);
        }

        h1 {
            color: var(--loxone-green);
            font-size: 2rem;
        }

        .subtitle {
            color: var(--text-secondary);
            font-size: 0.9rem;
            margin-top: 0.25rem;
        }

        .btn {
            background: var(--loxone-green);
            color: white;
            border: none;
            padding: 0.75rem 1.5rem;
            border-radius: 8px;
            font-size: 1rem;
            cursor: pointer;
            transition: all 0.3s ease;
            text-decoration: none;
            display: inline-flex;
            align-items: center;
            gap: 0.5rem;
        }

        .btn:hover {
            background: #6aa000;
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(122, 186, 0, 0.3);
        }

        .btn-danger {
            background: var(--danger);
        }

        .btn-danger:hover {
            background: #c82333;
        }

        .btn-secondary {
            background: var(--bg-card);
            border: 1px solid var(--border-color);
        }

        .btn-secondary:hover {
            background: var(--bg-secondary);
        }

        .btn-small {
            padding: 0.5rem 1rem;
            font-size: 0.875rem;
        }

        .keys-grid {
            display: grid;
            gap: 1rem;
            margin-top: 2rem;
        }

        .key-card {
            background: var(--bg-card);
            border: 1px solid var(--border-color);
            border-radius: 12px;
            padding: 1.5rem;
            transition: all 0.3s ease;
        }

        .key-card:hover {
            border-color: var(--loxone-green);
            box-shadow: 0 4px 12px rgba(122, 186, 0, 0.1);
        }

        .key-header {
            display: flex;
            justify-content: space-between;
            align-items: start;
            margin-bottom: 1rem;
        }

        .key-id {
            font-family: 'Courier New', monospace;
            font-size: 0.9rem;
            color: var(--text-secondary);
            word-break: break-all;
        }

        .key-name {
            font-size: 1.2rem;
            font-weight: 500;
            margin-bottom: 0.25rem;
        }

        .key-role {
            display: inline-block;
            padding: 0.25rem 0.75rem;
            background: var(--loxone-green);
            color: white;
            border-radius: 20px;
            font-size: 0.875rem;
            font-weight: 500;
        }

        .key-role.admin {
            background: var(--rust-orange);
        }

        .key-role.monitor {
            background: var(--info);
        }

        .key-role.device {
            background: var(--warning);
            color: var(--bg-primary);
        }

        .key-details {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 1rem;
            margin-top: 1rem;
            padding-top: 1rem;
            border-top: 1px solid var(--border-color);
        }

        .detail-item {
            font-size: 0.875rem;
        }

        .detail-label {
            color: var(--text-secondary);
            margin-bottom: 0.25rem;
        }

        .detail-value {
            color: var(--text-primary);
        }

        .status-badge {
            display: inline-flex;
            align-items: center;
            gap: 0.25rem;
            padding: 0.25rem 0.75rem;
            border-radius: 20px;
            font-size: 0.875rem;
        }

        .status-active {
            background: rgba(122, 186, 0, 0.2);
            color: var(--loxone-green);
        }

        .status-inactive {
            background: rgba(220, 53, 69, 0.2);
            color: var(--danger);
        }

        .key-actions {
            display: flex;
            gap: 0.5rem;
            margin-top: 1rem;
        }

        .modal {
            display: none;
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background: rgba(0, 0, 0, 0.8);
            z-index: 1000;
        }

        .modal-content {
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            background: var(--bg-card);
            border: 1px solid var(--border-color);
            border-radius: 12px;
            padding: 2rem;
            width: 90%;
            max-width: 600px;
            max-height: 90vh;
            overflow-y: auto;
        }

        .modal-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 1.5rem;
        }

        .modal-title {
            font-size: 1.5rem;
            color: var(--loxone-green);
        }

        .close-btn {
            background: none;
            border: none;
            color: var(--text-secondary);
            font-size: 1.5rem;
            cursor: pointer;
            padding: 0.5rem;
        }

        .close-btn:hover {
            color: var(--text-primary);
        }

        .form-group {
            margin-bottom: 1.5rem;
        }

        .form-label {
            display: block;
            margin-bottom: 0.5rem;
            color: var(--text-primary);
            font-weight: 500;
        }

        .form-input, .form-select, .form-textarea {
            width: 100%;
            padding: 0.75rem;
            background: var(--bg-secondary);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            color: var(--text-primary);
            font-size: 1rem;
        }

        .form-input:focus, .form-select:focus, .form-textarea:focus {
            outline: none;
            border-color: var(--loxone-green);
        }

        .form-help {
            font-size: 0.875rem;
            color: var(--text-secondary);
            margin-top: 0.25rem;
        }

        .ip-list {
            display: flex;
            flex-wrap: wrap;
            gap: 0.5rem;
            margin-top: 0.5rem;
        }

        .ip-tag {
            background: var(--bg-secondary);
            padding: 0.25rem 0.75rem;
            border-radius: 20px;
            font-size: 0.875rem;
            display: flex;
            align-items: center;
            gap: 0.5rem;
        }

        .remove-ip {
            background: none;
            border: none;
            color: var(--text-secondary);
            cursor: pointer;
            font-size: 1.2rem;
            line-height: 1;
        }

        .remove-ip:hover {
            color: var(--danger);
        }

        .loading {
            text-align: center;
            padding: 3rem;
            color: var(--text-secondary);
        }

        .error-message {
            background: rgba(220, 53, 69, 0.1);
            border: 1px solid var(--danger);
            color: var(--danger);
            padding: 1rem;
            border-radius: 8px;
            margin-bottom: 1rem;
        }

        .success-message {
            background: rgba(122, 186, 0, 0.1);
            border: 1px solid var(--loxone-green);
            color: var(--loxone-green);
            padding: 1rem;
            border-radius: 8px;
            margin-bottom: 1rem;
        }

        .copy-btn {
            background: var(--bg-secondary);
            border: 1px solid var(--border-color);
            color: var(--text-primary);
            padding: 0.5rem 1rem;
            border-radius: 6px;
            cursor: pointer;
            font-size: 0.875rem;
            transition: all 0.3s ease;
        }

        .copy-btn:hover {
            background: var(--bg-primary);
            border-color: var(--loxone-green);
        }

        .copied {
            background: var(--loxone-green);
            color: white;
        }

        @media (max-width: 768px) {
            .container {
                padding: 1rem;
            }

            .header {
                flex-direction: column;
                align-items: start;
                gap: 1rem;
            }

            .key-details {
                grid-template-columns: 1fr;
            }
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <div>
                <h1>API Key Management</h1>
                <div class="subtitle">Manage access keys for Loxone MCP Server</div>
            </div>
            <button class="btn" onclick="showGenerateModal()">
                <span>+</span> Generate New Key
            </button>
        </div>

        <div id="loading" class="loading">Loading keys...</div>
        <div id="error" class="error-message" style="display: none;"></div>
        <div id="success" class="success-message" style="display: none;"></div>
        <div id="keys-container" class="keys-grid"></div>
    </div>

    <!-- Generate Key Modal -->
    <div id="generateModal" class="modal">
        <div class="modal-content">
            <div class="modal-header">
                <h2 class="modal-title">Generate New API Key</h2>
                <button class="close-btn" onclick="hideGenerateModal()">×</button>
            </div>

            <form id="generateForm" onsubmit="generateKey(event)">
                <div class="form-group">
                    <label class="form-label" for="name">Key Name</label>
                    <input type="text" id="name" class="form-input" required placeholder="e.g., Home Assistant Integration">
                    <div class="form-help">A descriptive name for this key</div>
                </div>

                <div class="form-group">
                    <label class="form-label" for="role">Role</label>
                    <select id="role" class="form-select" required onchange="roleChanged()">
                        <option value="admin">Admin - Full system access</option>
                        <option value="operator" selected>Operator - Device control and monitoring</option>
                        <option value="monitor">Monitor - Read-only access</option>
                        <option value="device">Device - Specific device control</option>
                    </select>
                    <div class="form-help">Permission level for this key</div>
                </div>

                <div id="devicesGroup" class="form-group" style="display: none;">
                    <label class="form-label" for="devices">Allowed Devices</label>
                    <input type="text" id="devices" class="form-input" placeholder="bedroom-light, kitchen-blinds">
                    <div class="form-help">Comma-separated list of device IDs</div>
                </div>

                <div class="form-group">
                    <label class="form-label" for="expires">Expiration (days)</label>
                    <input type="number" id="expires" class="form-input" min="0" value="365">
                    <div class="form-help">0 = never expires</div>
                </div>

                <div class="form-group">
                    <label class="form-label">IP Whitelist</label>
                    <input type="text" id="ipInput" class="form-input" placeholder="192.168.1.50 or 192.168.1.0/24">
                    <button type="button" class="btn btn-secondary btn-small" onclick="addIP()" style="margin-top: 0.5rem;">Add IP</button>
                    <div class="form-help">Leave empty to allow all IPs</div>
                    <div id="ipList" class="ip-list"></div>
                </div>

                <div style="display: flex; gap: 1rem; margin-top: 2rem;">
                    <button type="submit" class="btn">Generate Key</button>
                    <button type="button" class="btn btn-secondary" onclick="hideGenerateModal()">Cancel</button>
                </div>
            </form>
        </div>
    </div>

    <!-- Edit Key Modal -->
    <div id="editModal" class="modal">
        <div class="modal-content">
            <div class="modal-header">
                <h2 class="modal-title">Edit API Key</h2>
                <button class="close-btn" onclick="hideEditModal()">×</button>
            </div>

            <form id="editForm" onsubmit="updateKey(event)">
                <input type="hidden" id="editKeyId">

                <div class="form-group">
                    <label class="form-label" for="editName">Key Name</label>
                    <input type="text" id="editName" class="form-input" required>
                </div>

                <div class="form-group">
                    <label class="form-label" for="editExpires">Extend Expiration (days from now)</label>
                    <input type="number" id="editExpires" class="form-input" min="0" value="0">
                    <div class="form-help">0 = don't change expiration</div>
                </div>

                <div class="form-group">
                    <label class="form-label">
                        <input type="checkbox" id="editActive"> Active
                    </label>
                    <div class="form-help">Deactivate to temporarily disable this key</div>
                </div>

                <div style="display: flex; gap: 1rem; margin-top: 2rem;">
                    <button type="submit" class="btn">Update Key</button>
                    <button type="button" class="btn btn-secondary" onclick="hideEditModal()">Cancel</button>
                </div>
            </form>
        </div>
    </div>

    <script>
        let keys = [];
        let ipWhitelist = [];

        // Get API key from URL parameters
        function getApiKey() {
            const params = new URLSearchParams(window.location.search);
            return params.get('api_key');
        }

        // Build URL with API key parameter
        function buildApiUrl(path) {
            const apiKey = getApiKey();
            if (apiKey) {
                const separator = path.includes('?') ? '&' : '?';
                return path + separator + 'api_key=' + encodeURIComponent(apiKey);
            }
            return path;
        }

        // Load keys on page load
        window.onload = function() {
            loadKeys();
        };

        async function loadKeys() {
            try {
                const response = await fetch(buildApiUrl('/admin/api/keys'));
                if (!response.ok) throw new Error('Failed to load keys');
                
                keys = await response.json();
                renderKeys();
                document.getElementById('loading').style.display = 'none';
            } catch (error) {
                showError('Failed to load keys: ' + error.message);
                document.getElementById('loading').style.display = 'none';
            }
        }

        function renderKeys() {
            const container = document.getElementById('keys-container');
            
            if (keys.length === 0) {
                container.innerHTML = '<div class="loading">No API keys found. Generate your first key to get started.</div>';
                return;
            }

            // Count active admin keys for protection logic
            const activeAdminKeys = keys.filter(k => k.role.toLowerCase() === 'admin' && k.active);
            const isLastAdmin = (key) => key.role.toLowerCase() === 'admin' && key.active && activeAdminKeys.length === 1;

            container.innerHTML = keys.map(key => `
                <div class="key-card">
                    <div class="key-header">
                        <div>
                            <div class="key-name">
                                ${escapeHtml(key.name)}
                                ${isLastAdmin(key) ? '<span style="color: var(--loxone-green); margin-left: 0.5rem;" title="Protected - last admin key">🔒</span>' : ''}
                            </div>
                            <div class="key-id">${key.id}</div>
                        </div>
                        <div style="display: flex; align-items: center; gap: 1rem;">
                            <span class="key-role ${key.role.toLowerCase()}">${key.role}</span>
                            <span class="status-badge ${key.active ? 'status-active' : 'status-inactive'}">
                                ${key.active ? '● Active' : '● Inactive'}
                            </span>
                        </div>
                    </div>

                    <div class="key-details">
                        <div class="detail-item">
                            <div class="detail-label">Created</div>
                            <div class="detail-value">${key.created_at}</div>
                        </div>
                        <div class="detail-item">
                            <div class="detail-label">Created By</div>
                            <div class="detail-value">${key.created_by}</div>
                        </div>
                        <div class="detail-item">
                            <div class="detail-label">Expires</div>
                            <div class="detail-value">${key.expires_at || 'Never'}</div>
                        </div>
                        <div class="detail-item">
                            <div class="detail-label">Last Used</div>
                            <div class="detail-value">${key.last_used || 'Never'}</div>
                        </div>
                        <div class="detail-item">
                            <div class="detail-label">Usage Count</div>
                            <div class="detail-value">${key.usage_count}</div>
                        </div>
                        <div class="detail-item">
                            <div class="detail-label">IP Restrictions</div>
                            <div class="detail-value">${key.ip_whitelist.length > 0 ? key.ip_whitelist.join(', ') : 'All allowed'}</div>
                        </div>
                    </div>

                    <div class="key-actions">
                        <button class="copy-btn" onclick="copyKey('${key.id}')">📋 Copy Key</button>
                        <button class="btn btn-secondary btn-small" onclick="editKey('${key.id}')" ${isLastAdmin(key) ? 'title="Edit (activation protected)"' : ''}>
                            ${isLastAdmin(key) ? '✏️🔒 Edit' : '✏️ Edit'}
                        </button>
                        ${isLastAdmin(key) 
                            ? '<button class="btn btn-secondary btn-small" disabled title="Cannot delete the last admin key">🗑️🔒 Protected</button>'
                            : `<button class="btn btn-danger btn-small" onclick="deleteKey('${key.id}')">🗑️ Delete</button>`
                        }
                    </div>
                </div>
            `).join('');
        }

        function showGenerateModal() {
            document.getElementById('generateModal').style.display = 'block';
            ipWhitelist = [];
            updateIPList();
        }

        function hideGenerateModal() {
            document.getElementById('generateModal').style.display = 'none';
            document.getElementById('generateForm').reset();
            ipWhitelist = [];
            updateIPList();
        }

        function roleChanged() {
            const role = document.getElementById('role').value;
            document.getElementById('devicesGroup').style.display = role === 'device' ? 'block' : 'none';
        }

        function addIP() {
            const input = document.getElementById('ipInput');
            const ip = input.value.trim();
            
            if (ip && !ipWhitelist.includes(ip)) {
                ipWhitelist.push(ip);
                updateIPList();
                input.value = '';
            }
        }

        function removeIP(ip) {
            ipWhitelist = ipWhitelist.filter(item => item !== ip);
            updateIPList();
        }

        function updateIPList() {
            const container = document.getElementById('ipList');
            container.innerHTML = ipWhitelist.map(ip => `
                <div class="ip-tag">
                    ${ip}
                    <button class="remove-ip" onclick="removeIP('${ip}')">×</button>
                </div>
            `).join('');
        }

        async function generateKey(event) {
            event.preventDefault();
            
            const role = document.getElementById('role').value;
            const data = {
                name: document.getElementById('name').value,
                role: role,
                expires_days: parseInt(document.getElementById('expires').value) || 0,
                ip_whitelist: ipWhitelist.length > 0 ? ipWhitelist : null
            };

            if (role === 'device') {
                const devices = document.getElementById('devices').value
                    .split(',')
                    .map(d => d.trim())
                    .filter(d => d);
                if (devices.length > 0) {
                    data.devices = devices;
                }
            }

            try {
                const response = await fetch(buildApiUrl('/admin/api/keys'), {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(data)
                });

                if (!response.ok) throw new Error('Failed to generate key');
                
                const newKey = await response.json();
                showSuccess(`Generated new key: ${newKey.id}`);
                hideGenerateModal();
                loadKeys();

                // Auto-copy the new key
                copyKey(newKey.id);
            } catch (error) {
                showError('Failed to generate key: ' + error.message);
            }
        }

        function editKey(keyId) {
            const key = keys.find(k => k.id === keyId);
            if (!key) return;

            // Check if this is the last admin key (protection logic)
            const activeAdminKeys = keys.filter(k => k.role.toLowerCase() === 'admin' && k.active);
            const isLastAdminKey = key.role.toLowerCase() === 'admin' && key.active && activeAdminKeys.length === 1;

            document.getElementById('editKeyId').value = key.id;
            document.getElementById('editName').value = key.name;
            document.getElementById('editActive').checked = key.active;
            
            // Disable the Active checkbox if this is the last admin key
            const activeCheckbox = document.getElementById('editActive');
            if (isLastAdminKey) {
                activeCheckbox.disabled = true;
                activeCheckbox.parentElement.title = 'Cannot deactivate the last admin key';
                activeCheckbox.parentElement.style.opacity = '0.6';
            } else {
                activeCheckbox.disabled = false;
                activeCheckbox.parentElement.title = '';
                activeCheckbox.parentElement.style.opacity = '1';
            }
            
            document.getElementById('editModal').style.display = 'block';
        }

        function hideEditModal() {
            document.getElementById('editModal').style.display = 'none';
            document.getElementById('editForm').reset();
        }

        async function updateKey(event) {
            event.preventDefault();
            
            const keyId = document.getElementById('editKeyId').value;
            const keyToEdit = keys.find(k => k.id === keyId);
            const newActiveState = document.getElementById('editActive').checked;
            
            // Check if this would deactivate the last admin key
            const activeAdminKeys = keys.filter(k => k.role.toLowerCase() === 'admin' && k.active);
            if (keyToEdit && keyToEdit.role.toLowerCase() === 'admin' && 
                keyToEdit.active && !newActiveState && activeAdminKeys.length === 1) {
                showError('Cannot deactivate the last active admin key. This would lock you out of the system.');
                return;
            }
            
            const data = {
                name: document.getElementById('editName').value,
                active: newActiveState
            };

            const expires = parseInt(document.getElementById('editExpires').value || 0);
            if (expires > 0) {
                data.expires_days = expires;
            }

            try {
                const response = await fetch(buildApiUrl(`/admin/api/keys/${keyId}`), {
                    method: 'PUT',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(data)
                });

                if (!response.ok) throw new Error('Failed to update key');
                
                showSuccess('Key updated successfully');
                hideEditModal();
                loadKeys();
            } catch (error) {
                showError('Failed to update key: ' + error.message);
            }
        }

        async function deleteKey(keyId) {
            // Check if this is the last admin key
            const adminKeys = keys.filter(k => k.role.toLowerCase() === 'admin' && k.active);
            const keyToDelete = keys.find(k => k.id === keyId);
            
            if (keyToDelete && keyToDelete.role.toLowerCase() === 'admin' && adminKeys.length === 1) {
                showError('Cannot delete the last active admin key. This would lock you out of the system.');
                return;
            }

            if (!confirm('Are you sure you want to delete this key? This action cannot be undone.')) {
                return;
            }

            try {
                const response = await fetch(buildApiUrl(`/admin/api/keys/${keyId}`), {
                    method: 'DELETE'
                });

                if (!response.ok) throw new Error('Failed to delete key');
                
                showSuccess('Key deleted successfully');
                loadKeys();
            } catch (error) {
                showError('Failed to delete key: ' + error.message);
            }
        }

        function copyKey(keyId) {
            navigator.clipboard.writeText(keyId).then(() => {
                // Find the specific button that was clicked
                const buttons = document.querySelectorAll('.copy-btn');
                buttons.forEach(btn => {
                    if (btn.onclick.toString().includes(keyId)) {
                        btn.classList.add('copied');
                        btn.innerHTML = '✓ Copied!';
                        setTimeout(() => {
                            btn.classList.remove('copied');
                            btn.innerHTML = '📋 Copy Key';
                        }, 2000);
                    }
                });
                showSuccess(`API key ${keyId} copied to clipboard!`);
            }).catch(err => {
                showError('Failed to copy to clipboard: ' + err.message);
            });
        }

        function showError(message) {
            const errorDiv = document.getElementById('error');
            errorDiv.textContent = message;
            errorDiv.style.display = 'block';
            setTimeout(() => {
                errorDiv.style.display = 'none';
            }, 5000);
        }

        function showSuccess(message) {
            const successDiv = document.getElementById('success');
            successDiv.textContent = message;
            successDiv.style.display = 'block';
            setTimeout(() => {
                successDiv.style.display = 'none';
            }, 5000);
        }

        function escapeHtml(text) {
            const map = {
                '&': '&amp;',
                '<': '&lt;',
                '>': '&gt;',
                '"': '&quot;',
                "'": '&#039;'
            };
            return text.replace(/[&<>"']/g, m => map[m]);
        }
    </script>
</body>
</html>"#.to_string()
    }
}

/// API key info for JSON responses
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: String,
    pub role: String,
    pub created_by: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub active: bool,
    pub ip_whitelist: Vec<String>,
    pub last_used: Option<String>,
    pub usage_count: u64,
}

/// Generate key request
#[derive(Debug, Deserialize)]
pub struct GenerateKeyRequest {
    pub name: String,
    pub role: String,
    pub expires_days: Option<u32>,
    pub ip_whitelist: Option<Vec<String>>,
    pub devices: Option<Vec<String>>,
}

/// Update key request
#[derive(Debug, Deserialize)]
pub struct UpdateKeyRequest {
    pub name: Option<String>,
    pub active: Option<bool>,
    pub expires_days: Option<u32>,
    pub ip_whitelist: Option<Vec<String>>,
}

/// Create router for key management UI
pub fn create_key_management_router(key_store: Arc<KeyStore>) -> axum::Router {
    use axum::routing::{delete, get, post, put};

    let ui = Arc::new(KeyManagementUI::new(key_store));

    axum::Router::new()
        .route("/keys", get(render_page_handler))
        .route("/api/keys", get(list_keys_handler))
        .route("/api/keys", post(generate_key_handler))
        .route("/api/keys/:id", put(update_key_handler))
        .route("/api/keys/:id", delete(delete_key_handler))
        .with_state(ui)
}

// Handler functions
async fn render_page_handler(State(ui): State<Arc<KeyManagementUI>>) -> Html<String> {
    ui.render_page().await
}

async fn list_keys_handler(State(ui): State<Arc<KeyManagementUI>>) -> Response {
    match ui.list_keys().await {
        Ok(keys) => keys.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn generate_key_handler(
    State(ui): State<Arc<KeyManagementUI>>,
    req: Json<GenerateKeyRequest>,
) -> Response {
    match ui.generate_key(req).await {
        Ok(key) => key.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn update_key_handler(
    State(ui): State<Arc<KeyManagementUI>>,
    Path(key_id): Path<String>,
    req: Json<UpdateKeyRequest>,
) -> Response {
    match ui.update_key(Path(key_id), req).await {
        Ok(status) => status.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_key_handler(
    State(ui): State<Arc<KeyManagementUI>>,
    Path(key_id): Path<String>,
) -> Response {
    match ui.delete_key(Path(key_id)).await {
        Ok(status) => status.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
