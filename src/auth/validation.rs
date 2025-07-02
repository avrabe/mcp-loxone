//! Authentication validation logic
//!
//! This module handles the validation of API keys, rate limiting,
//! and permission checking for authenticated requests.

use crate::auth::models::{ApiKey, AuthContext, AuthResult};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Rate limiting state for failed authentication attempts
#[derive(Debug, Clone)]
pub struct RateLimitState {
    /// Number of failed attempts
    failed_attempts: u32,
    /// When the first attempt in the current window occurred
    window_start: DateTime<Utc>,
    /// When the client is blocked until (if any)
    blocked_until: Option<DateTime<Utc>>,
}

/// Authentication validator with rate limiting and security features
pub struct Validator {
    /// Configuration
    pub config: ValidationConfig,
    /// Rate limiting state per IP
    pub rate_limit_state: Arc<RwLock<HashMap<String, RateLimitState>>>,
}

/// Validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Maximum failed attempts before rate limiting (after this many attempts, requests are blocked)
    pub max_failed_attempts: u32,
    /// Time window for tracking failed attempts (minutes)
    pub failed_attempt_window_minutes: u64,
    /// How long to block after max attempts (minutes)
    pub block_duration_minutes: u64,
    /// Session timeout (minutes)
    pub session_timeout_minutes: u64,
    /// Enable strict IP validation
    pub strict_ip_validation: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_failed_attempts: 4, // Block after 4 failed attempts (5th attempt returns RateLimited)
            failed_attempt_window_minutes: 15,
            block_duration_minutes: 30,
            session_timeout_minutes: 480, // 8 hours
            strict_ip_validation: true,
        }
    }
}

impl Validator {
    /// Create a new validator with the given configuration
    pub fn new(config: ValidationConfig) -> Self {
        Self {
            config,
            rate_limit_state: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a validator with default configuration
    pub fn with_defaults() -> Self {
        Self::new(ValidationConfig::default())
    }

    /// Validate an authentication request
    pub async fn validate_authentication(
        &self,
        api_key_secret: &str,
        client_ip: &str,
        stored_keys: &HashMap<String, ApiKey>,
    ) -> AuthResult {
        // Check rate limiting first
        if let Some(blocked_until) = self.check_rate_limit(client_ip).await {
            return AuthResult::RateLimited {
                retry_after_seconds: (blocked_until.timestamp() - Utc::now().timestamp()) as u64,
            };
        }

        // Find the API key
        let key = match self.find_key_by_secret(api_key_secret, stored_keys) {
            Some(key) => key,
            None => {
                self.record_failed_attempt(client_ip).await;
                return AuthResult::Unauthorized {
                    reason: "Invalid API key".to_string(),
                };
            }
        };

        // Validate the key
        if let Err(reason) = self.validate_key(key, client_ip) {
            self.record_failed_attempt(client_ip).await;
            return AuthResult::Unauthorized { reason };
        }

        // Clear any failed attempts for this IP
        self.clear_failed_attempts(client_ip).await;

        // Create authentication context
        let context = AuthContext::new(key, client_ip.to_string());

        AuthResult::Success(Box::new(crate::auth::models::AuthSuccess {
            key: key.clone(),
            context,
        }))
    }

    /// Check if a session has the required permission
    pub fn check_permission(&self, context: &AuthContext, permission: &str) -> bool {
        // Check if session is still valid
        if !context.is_valid(self.config.session_timeout_minutes) {
            warn!("Session expired for key: {}", context.key_id);
            return false;
        }

        // Check role-based permission
        context.role.has_permission(permission)
    }

    /// Check if an IP is currently rate limited
    async fn check_rate_limit(&self, client_ip: &str) -> Option<DateTime<Utc>> {
        let rate_limits = self.rate_limit_state.read().await;

        if let Some(state) = rate_limits.get(client_ip) {
            if let Some(blocked_until) = state.blocked_until {
                if Utc::now() < blocked_until {
                    return Some(blocked_until);
                }
            }
        }

        None
    }

    /// Record a failed authentication attempt
    async fn record_failed_attempt(&self, client_ip: &str) {
        let mut rate_limits = self.rate_limit_state.write().await;
        let now = Utc::now();

        let state = rate_limits
            .entry(client_ip.to_string())
            .or_insert_with(|| RateLimitState {
                failed_attempts: 0,
                window_start: now,
                blocked_until: None,
            });

        // Check if we're in a new time window
        let window_duration =
            chrono::Duration::minutes(self.config.failed_attempt_window_minutes as i64);
        if now - state.window_start > window_duration {
            // Reset to new window
            state.failed_attempts = 1;
            state.window_start = now;
            state.blocked_until = None;
        } else {
            // Increment attempts in current window
            state.failed_attempts += 1;

            // Check if we've exceeded the limit
            if state.failed_attempts >= self.config.max_failed_attempts {
                let block_duration =
                    chrono::Duration::minutes(self.config.block_duration_minutes as i64);
                state.blocked_until = Some(now + block_duration);

                warn!(
                    "IP {} blocked for {} minutes after {} failed attempts",
                    client_ip, self.config.block_duration_minutes, state.failed_attempts
                );
            }
        }

        debug!(
            "Failed attempt #{} from IP {} (window started: {})",
            state.failed_attempts, client_ip, state.window_start
        );
    }

    /// Clear failed attempts for an IP (after successful auth)
    async fn clear_failed_attempts(&self, client_ip: &str) {
        let mut rate_limits = self.rate_limit_state.write().await;
        if rate_limits.remove(client_ip).is_some() {
            debug!("Cleared failed attempts for IP: {}", client_ip);
        }
    }

    /// Find an API key by its secret
    fn find_key_by_secret<'a>(
        &self,
        secret: &str,
        keys: &'a HashMap<String, ApiKey>,
    ) -> Option<&'a ApiKey> {
        keys.values().find(|key| key.secret == secret)
    }

    /// Validate an API key
    fn validate_key(&self, key: &ApiKey, client_ip: &str) -> std::result::Result<(), String> {
        // Check if key is active
        if !key.active {
            return Err("API key is disabled".to_string());
        }

        // Check if key has expired
        if let Some(expires_at) = key.expires_at {
            if Utc::now() > expires_at {
                return Err("API key has expired".to_string());
            }
        }

        // Check IP whitelist
        if self.config.strict_ip_validation && !key.is_ip_allowed(client_ip) {
            return Err(format!("IP address {client_ip} not allowed for this key"));
        }

        Ok(())
    }

    /// Clean up old rate limit entries (should be called periodically)
    pub async fn cleanup_rate_limits(&self) {
        let mut rate_limits = self.rate_limit_state.write().await;
        let now = Utc::now();
        let cleanup_threshold = chrono::Duration::hours(24); // Remove entries older than 24 hours

        let initial_count = rate_limits.len();
        rate_limits.retain(|_ip, state| {
            // Keep if blocked and still in block period
            if let Some(blocked_until) = state.blocked_until {
                if now < blocked_until {
                    return true;
                }
            }

            // Keep if within the tracking window
            now - state.window_start < cleanup_threshold
        });

        let removed_count = initial_count - rate_limits.len();
        if removed_count > 0 {
            debug!("Cleaned up {} old rate limit entries", removed_count);
        }
    }

    /// Get current rate limit statistics
    pub async fn get_rate_limit_stats(&self) -> RateLimitStats {
        let rate_limits = self.rate_limit_state.read().await;
        let now = Utc::now();

        let mut stats = RateLimitStats {
            total_tracked_ips: rate_limits.len(),
            currently_blocked_ips: 0,
            total_failed_attempts: 0,
        };

        for state in rate_limits.values() {
            stats.total_failed_attempts += state.failed_attempts as u64;

            if let Some(blocked_until) = state.blocked_until {
                if now < blocked_until {
                    stats.currently_blocked_ips += 1;
                }
            }
        }

        stats
    }
}

/// Rate limiting statistics
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    /// Number of IPs being tracked
    pub total_tracked_ips: usize,
    /// Number of currently blocked IPs
    pub currently_blocked_ips: u32,
    /// Total failed attempts across all IPs
    pub total_failed_attempts: u64,
}

/// Permission constants for common operations
pub mod permissions {
    pub const ADMIN_CREATE_KEY: &str = "admin.create_key";
    pub const ADMIN_DELETE_KEY: &str = "admin.delete_key";
    pub const ADMIN_LIST_KEYS: &str = "admin.list_keys";
    pub const ADMIN_VIEW_AUDIT: &str = "admin.view_audit";

    pub const DEVICE_READ: &str = "device.read";
    pub const DEVICE_CONTROL: &str = "device.control";

    pub const SYSTEM_STATUS: &str = "system.status";
    pub const SYSTEM_HEALTH: &str = "system.health";

    pub const MCP_TOOLS_LIST: &str = "mcp.tools.list";
    pub const MCP_TOOLS_EXECUTE: &str = "mcp.tools.execute";
    pub const MCP_RESOURCES_LIST: &str = "mcp.resources.list";
    pub const MCP_RESOURCES_READ: &str = "mcp.resources.read";
}

/// Helper function to extract client IP from various sources
pub fn extract_client_ip(headers: &axum::http::HeaderMap) -> String {
    // Try various headers in order of preference
    for header_name in ["x-forwarded-for", "x-real-ip", "x-client-ip"] {
        if let Some(ip) = headers.get(header_name) {
            if let Ok(ip_str) = ip.to_str() {
                // Take the first IP if there are multiple (comma-separated)
                let ip = ip_str.split(',').next().unwrap_or(ip_str).trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }

    "unknown".to_string()
}

/// Helper function to extract API key from request
pub fn extract_api_key(headers: &axum::http::HeaderMap, query: Option<&str>) -> Option<String> {
    // Try Authorization header with Bearer token
    if let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }

    // Try X-API-Key header
    if let Some(api_key_header) = headers.get("x-api-key") {
        if let Ok(key) = api_key_header.to_str() {
            return Some(key.to_string());
        }
    }

    // Try query parameter
    if let Some(query_string) = query {
        for param in query_string.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                if key == "api_key" {
                    return Some(urlencoding::decode(value).unwrap_or_default().to_string());
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::models::Role;

    #[tokio::test]
    async fn test_rate_limiting() {
        let validator = Validator::with_defaults();
        let keys = HashMap::new();

        // Test multiple failed attempts
        for i in 1..=5 {
            let result = validator
                .validate_authentication("invalid_key", "192.168.1.1", &keys)
                .await;

            match result {
                AuthResult::Unauthorized { .. } if i < 5 => {
                    // Expected for first 4 attempts
                }
                AuthResult::RateLimited { .. } if i == 5 => {
                    // Expected for 5th attempt
                }
                _ => panic!("Unexpected result for attempt {i}: {result:?}"),
            }
        }
    }

    #[tokio::test]
    async fn test_successful_authentication() {
        let validator = Validator::with_defaults();

        let mut keys = HashMap::new();
        let key = ApiKey::new(
            "Test Key".to_string(),
            Role::Operator,
            "test_user".to_string(),
            Some(365),
        );
        let secret = key.secret.clone();
        keys.insert(key.id.clone(), key);

        let result = validator
            .validate_authentication(&secret, "192.168.1.1", &keys)
            .await;

        match result {
            AuthResult::Success(auth_success) => {
                assert_eq!(auth_success.key.secret, secret);
                assert_eq!(auth_success.context.client_ip, "192.168.1.1");
            }
            _ => panic!("Expected successful authentication, got: {result:?}"),
        }
    }

    #[tokio::test]
    async fn test_permission_checking() {
        let validator = Validator::with_defaults();

        let admin_key = ApiKey::new(
            "Admin Key".to_string(),
            Role::Admin,
            "admin".to_string(),
            None,
        );
        let admin_context = AuthContext::new(&admin_key, "127.0.0.1".to_string());

        let operator_key = ApiKey::new(
            "Operator Key".to_string(),
            Role::Operator,
            "operator".to_string(),
            None,
        );
        let operator_context = AuthContext::new(&operator_key, "127.0.0.1".to_string());

        // Admin should have all permissions
        assert!(validator.check_permission(&admin_context, permissions::ADMIN_CREATE_KEY));
        assert!(validator.check_permission(&admin_context, permissions::DEVICE_CONTROL));

        // Operator should not have admin permissions
        assert!(!validator.check_permission(&operator_context, permissions::ADMIN_CREATE_KEY));
        assert!(validator.check_permission(&operator_context, permissions::DEVICE_CONTROL));
    }
}
