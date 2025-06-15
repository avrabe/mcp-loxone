//! Enhanced authentication middleware for HTTP transport
//!
//! This module provides comprehensive authentication with API key management,
//! role-based access control, key rotation, and security auditing.

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable API key authentication
    pub require_api_key: bool,
    /// API key header name
    pub api_key_header: String,
    /// Key rotation interval in days
    pub key_rotation_days: i64,
    /// Maximum active keys per role
    pub max_keys_per_role: usize,
    /// Enable audit logging
    pub enable_audit_logging: bool,
    /// Session timeout in minutes
    pub session_timeout_minutes: i64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            require_api_key: true,
            api_key_header: "X-API-Key".to_string(),
            key_rotation_days: 30,
            max_keys_per_role: 10,
            enable_audit_logging: true,
            session_timeout_minutes: 480, // 8 hours
        }
    }
}

/// User role with permissions
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserRole {
    /// Full system access
    Admin,
    /// Can control devices and read all data
    Operator,
    /// Read-only access to resources
    ReadOnly,
    /// Can only access specific tools/resources
    Limited,
    /// Monitoring and health check access only
    Monitor,
}

impl UserRole {
    /// Check if role has permission for specific endpoint
    pub fn has_permission(&self, endpoint: &str, method: &str) -> bool {
        match self {
            UserRole::Admin => true, // Admin has all permissions
            UserRole::Operator => {
                // Operators can do everything except admin functions
                !endpoint.starts_with("/admin")
            }
            UserRole::ReadOnly => {
                // Read-only can only use GET methods and specific read endpoints
                matches!(
                    method,
                    "tools/list"
                        | "resources/list"
                        | "resources/read"
                        | "prompts/list"
                        | "prompts/get"
                ) || endpoint.starts_with("/health")
            }
            UserRole::Limited => {
                // Limited access to specific endpoints only
                matches!(endpoint, "/health" | "/mcp/sse")
                    || matches!(method, "tools/list" | "resources/list")
            }
            UserRole::Monitor => {
                // Monitor role can only access health and monitoring endpoints
                matches!(endpoint, "/health" | "/admin/metrics" | "/admin/status")
            }
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            UserRole::Admin => "Full administrative access",
            UserRole::Operator => "Device control and monitoring",
            UserRole::ReadOnly => "Read-only access to resources",
            UserRole::Limited => "Limited access to basic functions",
            UserRole::Monitor => "Health and monitoring access only",
        }
    }
}

/// API key with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// Unique key identifier
    pub key_id: String,
    /// The actual API key (hashed in storage)
    pub key_hash: String,
    /// User role for this key
    pub role: UserRole,
    /// Key creation time
    pub created_at: DateTime<Utc>,
    /// Key expiration time
    pub expires_at: DateTime<Utc>,
    /// Whether key is active
    pub is_active: bool,
    /// Key description/name
    pub description: String,
    /// Last used timestamp
    pub last_used: Option<DateTime<Utc>>,
    /// Usage count
    pub usage_count: u64,
    /// IP address restrictions (if any)
    pub allowed_ips: Option<HashSet<String>>,
}

impl ApiKey {
    /// Create new API key
    pub fn new(role: UserRole, description: String, validity_days: i64) -> (Self, String) {
        let key_id = Uuid::new_v4().to_string();
        let raw_key = format!(
            "lmcp_{}_{}_{}",
            role.to_string().to_lowercase(),
            &key_id[..8],
            &Uuid::new_v4().to_string().replace('-', "")[..16]
        );
        let key_hash = Self::hash_key(&raw_key);

        let api_key = Self {
            key_id,
            key_hash,
            role,
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::days(validity_days),
            is_active: true,
            description,
            last_used: None,
            usage_count: 0,
            allowed_ips: None,
        };

        (api_key, raw_key)
    }

    /// Hash API key for secure storage
    fn hash_key(key: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verify key matches hash
    pub fn verify_key(&self, key: &str) -> bool {
        Self::hash_key(key) == self.key_hash
    }

    /// Check if key is valid (active and not expired)
    pub fn is_valid(&self) -> bool {
        self.is_active && Utc::now() < self.expires_at
    }

    /// Check if key is expiring soon (within 7 days)
    pub fn is_expiring_soon(&self) -> bool {
        let warning_threshold = Utc::now() + Duration::days(7);
        self.expires_at < warning_threshold
    }

    /// Update usage statistics
    pub fn update_usage(&mut self) {
        self.last_used = Some(Utc::now());
        self.usage_count += 1;
    }
}

/// Authentication session
#[derive(Debug, Clone)]
pub struct AuthSession {
    /// Session ID
    pub session_id: String,
    /// Associated API key
    pub api_key: ApiKey,
    /// Session creation time
    pub created_at: DateTime<Utc>,
    /// Last activity time
    pub last_activity: DateTime<Utc>,
    /// Client IP address
    pub client_ip: String,
    /// Request count in this session
    pub request_count: u64,
}

impl AuthSession {
    /// Create new session
    pub fn new(api_key: ApiKey, client_ip: String) -> Self {
        Self {
            session_id: Uuid::new_v4().to_string(),
            api_key,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            client_ip,
            request_count: 0,
        }
    }

    /// Check if session is valid
    pub fn is_valid(&self, timeout_minutes: i64) -> bool {
        let timeout_threshold = Utc::now() - Duration::minutes(timeout_minutes);
        self.last_activity > timeout_threshold && self.api_key.is_valid()
    }

    /// Update session activity
    pub fn update_activity(&mut self) {
        self.last_activity = Utc::now();
        self.request_count += 1;
    }
}

/// Audit log entry
#[derive(Debug, Clone, Serialize)]
pub struct AuditLogEntry {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: String,
    /// API key ID (if applicable)
    pub key_id: Option<String>,
    /// User role
    pub role: Option<UserRole>,
    /// Client IP address
    pub client_ip: String,
    /// Endpoint accessed
    pub endpoint: String,
    /// HTTP method
    pub method: String,
    /// Success/failure
    pub success: bool,
    /// Error message (if any)
    pub error_message: Option<String>,
    /// Additional context
    pub context: serde_json::Value,
}

/// Enhanced authentication manager
#[derive(Clone)]
pub struct AuthManager {
    /// Configuration
    config: AuthConfig,
    /// API keys storage
    api_keys: Arc<RwLock<HashMap<String, ApiKey>>>,
    /// Active sessions
    sessions: Arc<RwLock<HashMap<String, AuthSession>>>,
    /// Audit log (in-memory for now, could be database)
    audit_log: Arc<RwLock<Vec<AuditLogEntry>>>,
}

impl AuthManager {
    /// Create new authentication manager
    pub fn new(config: AuthConfig) -> Self {
        Self {
            config,
            api_keys: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(AuthConfig::default())
    }

    /// Add API key
    #[allow(clippy::too_many_arguments)]
    pub async fn add_api_key(&self, role: UserRole, description: String) -> Result<String, String> {
        let (api_key, raw_key) = ApiKey::new(
            role.clone(),
            description.clone(),
            self.config.key_rotation_days,
        );

        let mut keys = self.api_keys.write().await;

        // Check max keys per role
        let role_count = keys
            .values()
            .filter(|k| k.role == api_key.role && k.is_active)
            .count();
        if role_count >= self.config.max_keys_per_role {
            return Err(format!(
                "Maximum number of keys ({}) reached for role {:?}",
                self.config.max_keys_per_role, api_key.role
            ));
        }

        let audit_role = api_key.role.clone();
        let audit_description = api_key.description.clone();
        let key_id = api_key.key_id.clone();

        keys.insert(key_id, api_key);

        self.log_audit_event(
            "api_key_created".to_owned(),
            None,
            Some(audit_role),
            "system".to_owned(),
            "/admin/keys".to_owned(),
            "POST".to_owned(),
            true,
            None,
            serde_json::json!({"description": audit_description}),
        )
        .await;

        Ok(raw_key)
    }

    /// Add legacy HTTP_API_KEY for backward compatibility
    pub async fn add_legacy_key(&self, api_key: String) {
        let legacy_key = ApiKey {
            key_id: "legacy".to_string(),
            key_hash: ApiKey::hash_key(&api_key),
            role: UserRole::Admin,
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::days(365 * 10), // 10 years
            is_active: true,
            description: "Legacy HTTP_API_KEY".to_string(),
            last_used: None,
            usage_count: 0,
            allowed_ips: None,
        };

        let mut keys = self.api_keys.write().await;
        keys.insert("legacy".to_string(), legacy_key);

        debug!("Added legacy HTTP_API_KEY for backward compatibility");
    }

    /// Authenticate request with API key
    pub async fn authenticate(&self, headers: &HeaderMap, client_ip: String) -> AuthResult {
        if !self.config.require_api_key {
            return AuthResult::Success {
                role: UserRole::Admin,
                session_id: "no-auth".to_string(),
            };
        }

        let api_key = match self.extract_api_key(headers) {
            Some(key) => key,
            None => {
                self.log_auth_failure("missing_api_key", client_ip, String::new())
                    .await;
                return AuthResult::Unauthorized("Missing API key".to_string());
            }
        };

        // Find and validate API key
        let mut keys = self.api_keys.write().await;
        let mut found_key = None;

        for key in keys.values_mut() {
            if key.verify_key(&api_key) {
                if !key.is_valid() {
                    self.log_auth_failure("invalid_key", client_ip, key.key_id.clone())
                        .await;
                    return AuthResult::Unauthorized("Invalid or expired API key".to_string());
                }

                // Check IP restrictions
                if let Some(ref allowed_ips) = key.allowed_ips {
                    if !allowed_ips.contains(&client_ip) {
                        self.log_auth_failure("ip_restriction", client_ip, key.key_id.clone())
                            .await;
                        return AuthResult::Forbidden("IP address not allowed".to_string());
                    }
                }

                key.update_usage();
                found_key = Some(key.clone());
                break;
            }
        }

        let key = match found_key {
            Some(k) => k,
            None => {
                self.log_auth_failure("key_not_found", client_ip, "unknown".to_owned())
                    .await;
                return AuthResult::Unauthorized("Invalid API key".to_string());
            }
        };

        // Create or update session
        let session = AuthSession::new(key.clone(), client_ip.clone());
        let session_id = session.session_id.clone();

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session);

        AuthResult::Success {
            role: key.role,
            session_id,
        }
    }

    /// Check endpoint permissions
    pub async fn check_permissions(&self, session_id: &str, endpoint: &str, method: &str) -> bool {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            if session.is_valid(self.config.session_timeout_minutes) {
                return session.api_key.role.has_permission(endpoint, method);
            }
        }
        false
    }

    /// Extract API key from headers
    fn extract_api_key(&self, headers: &HeaderMap) -> Option<String> {
        // Try custom header first
        if let Some(key) = headers.get(&self.config.api_key_header) {
            if let Ok(key_str) = key.to_str() {
                return Some(key_str.to_string());
            }
        }

        // Try Authorization header with Bearer token
        if let Some(auth) = headers.get("authorization") {
            if let Ok(auth_str) = auth.to_str() {
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    return Some(token.to_string());
                }
            }
        }

        None
    }

    /// Log authentication failure
    async fn log_auth_failure(&self, event_type: &str, client_ip: String, key_id: String) {
        self.log_audit_event(
            format!("auth_failure_{}", event_type),
            if key_id.is_empty() {
                None
            } else {
                Some(key_id)
            },
            None,
            client_ip,
            "/auth".to_owned(),
            "AUTH".to_owned(),
            false,
            Some(format!("Authentication failed: {}", event_type)),
            serde_json::json!({}),
        )
        .await;
    }

    /// Log audit event
    #[allow(clippy::too_many_arguments)]
    async fn log_audit_event(
        &self,
        event_type: String,
        key_id: Option<String>,
        role: Option<UserRole>,
        client_ip: String,
        endpoint: String,
        method: String,
        success: bool,
        error_message: Option<String>,
        context: serde_json::Value,
    ) {
        if !self.config.enable_audit_logging {
            return;
        }

        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            event_type,
            key_id,
            role,
            client_ip,
            endpoint,
            method,
            success,
            error_message,
            context,
        };

        let mut audit_log = self.audit_log.write().await;
        audit_log.push(entry);

        // Keep only last 10000 entries to prevent memory bloat
        if audit_log.len() > 10000 {
            audit_log.drain(0..1000);
        }
    }

    /// Get authentication statistics
    pub async fn get_auth_stats(&self) -> AuthStatistics {
        let keys = self.api_keys.read().await;
        let sessions = self.sessions.read().await;
        let audit_log = self.audit_log.read().await;

        let total_keys = keys.len();
        let active_keys = keys.values().filter(|k| k.is_valid()).count();
        let expiring_keys = keys.values().filter(|k| k.is_expiring_soon()).count();
        let active_sessions = sessions
            .values()
            .filter(|s| s.is_valid(self.config.session_timeout_minutes))
            .count();

        let recent_failures = audit_log
            .iter()
            .filter(|entry| entry.timestamp > Utc::now() - Duration::hours(1))
            .filter(|entry| !entry.success)
            .count();

        AuthStatistics {
            total_keys,
            active_keys,
            expiring_keys,
            active_sessions,
            recent_auth_failures: recent_failures,
            total_audit_entries: audit_log.len(),
        }
    }

    /// Cleanup expired sessions
    pub async fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        sessions.retain(|_, session| session.is_valid(self.config.session_timeout_minutes));
    }

    /// Rotate API key
    pub async fn rotate_api_key(&self, key_id: &str) -> Result<String, String> {
        let mut keys = self.api_keys.write().await;

        if let Some(old_key) = keys.get(key_id) {
            let (new_key, raw_key) = ApiKey::new(
                old_key.role.clone(),
                format!("{} (rotated)", old_key.description),
                self.config.key_rotation_days,
            );

            // Deactivate old key
            if let Some(old_key_mut) = keys.get_mut(key_id) {
                old_key_mut.is_active = false;
            }

            // Add new key
            keys.insert(new_key.key_id.clone(), new_key);

            Ok(raw_key)
        } else {
            Err("API key not found".to_string())
        }
    }
}

/// Authentication result
#[derive(Debug)]
pub enum AuthResult {
    Success { role: UserRole, session_id: String },
    Unauthorized(String),
    Forbidden(String),
}

/// Authentication statistics
#[derive(Debug, Serialize)]
pub struct AuthStatistics {
    pub total_keys: usize,
    pub active_keys: usize,
    pub expiring_keys: usize,
    pub active_sessions: usize,
    pub recent_auth_failures: usize,
    pub total_audit_entries: usize,
}

/// Authentication middleware
pub async fn auth_middleware(
    State(auth_manager): State<Arc<AuthManager>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let headers = request.headers();
    let client_ip = extract_client_ip(headers);

    // Get endpoint path and method
    let path = request.uri().path().to_string();
    let method = request.method().as_str().to_string();

    // Authenticate request
    let auth_result = auth_manager.authenticate(headers, client_ip.clone()).await;

    match auth_result {
        AuthResult::Success { role, session_id } => {
            // Check endpoint permissions
            if !auth_manager
                .check_permissions(&session_id, &path, &method)
                .await
            {
                warn!(
                    client_ip = %client_ip,
                    role = ?role,
                    endpoint = %path,
                    method = %method,
                    "Access denied: insufficient permissions"
                );
                return Err(StatusCode::FORBIDDEN);
            }

            // Add session info to request extensions
            request.extensions_mut().insert(AuthInfo {
                role: role.clone(),
                session_id: session_id.clone(),
                client_ip: client_ip.clone(),
            });

            debug!(
                client_ip = %client_ip,
                role = ?role,
                endpoint = %path,
                "Request authenticated successfully"
            );

            Ok(next.run(request).await)
        }
        AuthResult::Unauthorized(msg) => {
            warn!(client_ip = %client_ip, error = %msg, "Authentication failed");
            Err(StatusCode::UNAUTHORIZED)
        }
        AuthResult::Forbidden(msg) => {
            warn!(client_ip = %client_ip, error = %msg, "Access forbidden");
            Err(StatusCode::FORBIDDEN)
        }
    }
}

/// Extract client IP from headers
pub fn extract_client_ip(headers: &HeaderMap) -> String {
    // Try various headers to get real client IP
    for header_name in ["x-forwarded-for", "x-real-ip", "x-client-ip"] {
        if let Some(ip) = headers.get(header_name) {
            if let Ok(ip_str) = ip.to_str() {
                let ip = ip_str.split(',').next().unwrap_or(ip_str).trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }
    "unknown".to_string()
}

/// Authentication information added to request
#[derive(Debug, Clone)]
pub struct AuthInfo {
    pub role: UserRole,
    pub session_id: String,
    pub client_ip: String,
}

/// Convert UserRole to string for serialization
impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let role_str = match self {
            UserRole::Admin => "admin",
            UserRole::Operator => "operator",
            UserRole::ReadOnly => "readonly",
            UserRole::Limited => "limited",
            UserRole::Monitor => "monitor",
        };
        write!(f, "{}", role_str)
    }
}
