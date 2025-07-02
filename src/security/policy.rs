//! Security policy implementation for authentication, session management, and access control

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Security policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicyConfig {
    /// Authentication policy
    pub authentication: AuthenticationPolicy,
    /// Session management policy
    pub session_management: SessionPolicy,
    /// Access control policy
    pub access_control: AccessControlPolicy,
    /// Password policy
    pub password_policy: PasswordPolicy,
    /// API key policy
    pub api_key_policy: ApiKeyPolicy,
    /// Audit policy
    pub audit_policy: AuditPolicy,
}

/// Authentication policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationPolicy {
    /// Require authentication for all endpoints
    pub require_authentication: bool,
    /// Allow basic authentication
    pub allow_basic_auth: bool,
    /// Allow bearer token authentication
    pub allow_bearer_auth: bool,
    /// Allow API key authentication
    pub allow_api_key_auth: bool,
    /// Multi-factor authentication settings
    pub mfa_settings: MfaSettings,
    /// Login attempt limits
    pub login_attempt_limits: LoginAttemptLimits,
    /// Authentication timeout
    pub auth_timeout: Duration,
}

/// Multi-factor authentication settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaSettings {
    /// Enable MFA
    pub enabled: bool,
    /// Require MFA for admin users
    pub require_for_admin: bool,
    /// MFA methods allowed
    pub allowed_methods: Vec<MfaMethod>,
    /// MFA bypass conditions
    pub bypass_conditions: Vec<MfaBypassCondition>,
}

/// MFA method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MfaMethod {
    /// Time-based one-time password
    Totp,
    /// SMS-based OTP
    Sms,
    /// Email-based OTP
    Email,
    /// Hardware security key
    SecurityKey,
    /// Backup codes
    BackupCodes,
}

/// MFA bypass condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MfaBypassCondition {
    /// Trusted IP address
    TrustedIp(String),
    /// Trusted device
    TrustedDevice,
    /// Recent successful MFA
    RecentMfa(Duration),
}

/// Login attempt limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginAttemptLimits {
    /// Maximum failed attempts before lockout
    pub max_failed_attempts: u32,
    /// Lockout duration
    pub lockout_duration: Duration,
    /// Reset period for failed attempts
    pub reset_period: Duration,
    /// Progressive lockout (increase duration with each lockout)
    pub progressive_lockout: bool,
}

/// Session management policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPolicy {
    /// Session timeout (idle)
    pub idle_timeout: Duration,
    /// Absolute session timeout
    pub absolute_timeout: Duration,
    /// Concurrent session limit
    pub concurrent_sessions: Option<u32>,
    /// Session renewal allowed
    pub allow_renewal: bool,
    /// Session fixation protection
    pub regenerate_session_id: bool,
    /// Secure session cookies
    pub secure_cookies: bool,
    /// HttpOnly cookies
    pub http_only_cookies: bool,
    /// SameSite cookie policy
    pub same_site_policy: SameSitePolicy,
}

/// SameSite cookie policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SameSitePolicy {
    /// Strict - cookies sent only for same-site requests
    Strict,
    /// Lax - cookies sent for same-site requests and top-level navigations
    Lax,
    /// None - cookies sent for all requests (requires Secure)
    None,
}

/// Access control policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControlPolicy {
    /// Role-based access control enabled
    pub rbac_enabled: bool,
    /// Default role for new users
    pub default_role: String,
    /// Role definitions
    pub roles: HashMap<String, Role>,
    /// Resource-based access control
    pub resource_permissions: HashMap<String, ResourcePermission>,
    /// IP-based restrictions
    pub ip_restrictions: IpRestrictions,
}

/// Role definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Role name
    pub name: String,
    /// Role description
    pub description: String,
    /// Permissions granted
    pub permissions: Vec<Permission>,
    /// Inherits from other roles
    pub inherits_from: Vec<String>,
    /// Role priority (higher = more privileged)
    pub priority: u32,
}

/// Permission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    /// Resource type
    pub resource: String,
    /// Actions allowed
    pub actions: Vec<String>,
    /// Conditions for permission
    pub conditions: Vec<PermissionCondition>,
}

/// Permission condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionCondition {
    /// Time-based restriction
    TimeRange {
        start_hour: u8,
        end_hour: u8,
        days: Vec<String>,
    },
    /// IP-based restriction
    IpRange(String),
    /// Resource ownership
    OwnerOnly,
    /// Custom condition
    Custom(String),
}

/// Resource permission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePermission {
    /// Resource identifier pattern
    pub pattern: String,
    /// Required authentication level
    pub required_auth_level: AuthLevel,
    /// Required roles
    pub required_roles: Vec<String>,
    /// Additional conditions
    pub conditions: Vec<PermissionCondition>,
}

/// Authentication level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthLevel {
    /// No authentication required
    None,
    /// Basic authentication
    Basic,
    /// Standard authentication
    Standard,
    /// Elevated authentication (with MFA)
    Elevated,
    /// Admin authentication
    Admin,
}

/// IP-based restrictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpRestrictions {
    /// Enable IP restrictions
    pub enabled: bool,
    /// Whitelist mode (only allow listed IPs)
    pub whitelist_mode: bool,
    /// Allowed IP ranges
    pub allowed_ranges: Vec<String>,
    /// Blocked IP ranges
    pub blocked_ranges: Vec<String>,
    /// Geo-blocking enabled
    pub geo_blocking: bool,
    /// Allowed countries (ISO codes)
    pub allowed_countries: Vec<String>,
    /// Blocked countries (ISO codes)
    pub blocked_countries: Vec<String>,
}

/// Password policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordPolicy {
    /// Minimum password length
    pub min_length: usize,
    /// Maximum password length
    pub max_length: usize,
    /// Require uppercase letters
    pub require_uppercase: bool,
    /// Require lowercase letters
    pub require_lowercase: bool,
    /// Require numbers
    pub require_numbers: bool,
    /// Require special characters
    pub require_special_chars: bool,
    /// Minimum character types required
    pub min_character_types: u8,
    /// Password history (prevent reuse)
    pub history_count: u32,
    /// Password expiry
    pub expiry_days: Option<u32>,
    /// Common password blacklist
    pub use_blacklist: bool,
    /// Similarity check with username
    pub check_username_similarity: bool,
}

/// API key policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyPolicy {
    /// API key length
    pub key_length: usize,
    /// Key expiry
    pub expiry_days: Option<u32>,
    /// Allow key rotation
    pub allow_rotation: bool,
    /// Rotation period
    pub rotation_period: Option<Duration>,
    /// Scope restrictions
    pub scope_restrictions: bool,
    /// Rate limiting per key
    pub per_key_rate_limits: bool,
    /// Key naming pattern
    pub naming_pattern: Option<String>,
}

/// Audit policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPolicy {
    /// Enable audit logging
    pub enabled: bool,
    /// Log successful authentications
    pub log_success: bool,
    /// Log failed authentications
    pub log_failures: bool,
    /// Log permission denials
    pub log_denials: bool,
    /// Log sensitive operations
    pub log_sensitive_ops: bool,
    /// Retention period
    pub retention_days: u32,
    /// Real-time alerting
    pub real_time_alerts: bool,
    /// Alert thresholds
    pub alert_thresholds: AlertThresholds,
}

/// Alert thresholds for security events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// Failed login attempts threshold
    pub failed_logins: u32,
    /// Permission denials threshold
    pub permission_denials: u32,
    /// Suspicious activity score threshold
    pub suspicious_score: u32,
    /// Time window for thresholds (minutes)
    pub time_window_minutes: u32,
}

impl SecurityPolicyConfig {
    /// Create secure production policy
    pub fn secure() -> Self {
        Self {
            authentication: AuthenticationPolicy {
                require_authentication: true,
                allow_basic_auth: false,
                allow_bearer_auth: true,
                allow_api_key_auth: true,
                mfa_settings: MfaSettings {
                    enabled: false, // Can be enabled as needed
                    require_for_admin: true,
                    allowed_methods: vec![MfaMethod::Totp, MfaMethod::BackupCodes],
                    bypass_conditions: vec![MfaBypassCondition::RecentMfa(Duration::from_secs(
                        3600,
                    ))],
                },
                login_attempt_limits: LoginAttemptLimits {
                    max_failed_attempts: 5,
                    lockout_duration: Duration::from_secs(900), // 15 minutes
                    reset_period: Duration::from_secs(3600),    // 1 hour
                    progressive_lockout: true,
                },
                auth_timeout: Duration::from_secs(30),
            },
            session_management: SessionPolicy {
                idle_timeout: Duration::from_secs(1800),      // 30 minutes
                absolute_timeout: Duration::from_secs(28800), // 8 hours
                concurrent_sessions: Some(5),
                allow_renewal: true,
                regenerate_session_id: true,
                secure_cookies: true,
                http_only_cookies: true,
                same_site_policy: SameSitePolicy::Strict,
            },
            access_control: AccessControlPolicy {
                rbac_enabled: true,
                default_role: "user".to_string(),
                roles: Self::default_roles(),
                resource_permissions: Self::default_resource_permissions(),
                ip_restrictions: IpRestrictions {
                    enabled: false,
                    whitelist_mode: false,
                    allowed_ranges: Vec::new(),
                    blocked_ranges: Vec::new(),
                    geo_blocking: false,
                    allowed_countries: Vec::new(),
                    blocked_countries: Vec::new(),
                },
            },
            password_policy: PasswordPolicy {
                min_length: 12,
                max_length: 128,
                require_uppercase: true,
                require_lowercase: true,
                require_numbers: true,
                require_special_chars: true,
                min_character_types: 3,
                history_count: 5,
                expiry_days: Some(90),
                use_blacklist: true,
                check_username_similarity: true,
            },
            api_key_policy: ApiKeyPolicy {
                key_length: 32,
                expiry_days: Some(365),
                allow_rotation: true,
                rotation_period: Some(Duration::from_secs(86400 * 90)), // 90 days
                scope_restrictions: true,
                per_key_rate_limits: true,
                naming_pattern: Some("^[a-zA-Z0-9_-]+$".to_string()),
            },
            audit_policy: AuditPolicy {
                enabled: true,
                log_success: false, // Can be verbose
                log_failures: true,
                log_denials: true,
                log_sensitive_ops: true,
                retention_days: 90,
                real_time_alerts: true,
                alert_thresholds: AlertThresholds {
                    failed_logins: 10,
                    permission_denials: 20,
                    suspicious_score: 50,
                    time_window_minutes: 15,
                },
            },
        }
    }

    /// Create development policy (more relaxed)
    pub fn development() -> Self {
        Self {
            authentication: AuthenticationPolicy {
                require_authentication: false,
                allow_basic_auth: true,
                allow_bearer_auth: true,
                allow_api_key_auth: true,
                mfa_settings: MfaSettings {
                    enabled: false,
                    require_for_admin: false,
                    allowed_methods: vec![],
                    bypass_conditions: vec![],
                },
                login_attempt_limits: LoginAttemptLimits {
                    max_failed_attempts: 100,
                    lockout_duration: Duration::from_secs(60),
                    reset_period: Duration::from_secs(300),
                    progressive_lockout: false,
                },
                auth_timeout: Duration::from_secs(300),
            },
            session_management: SessionPolicy {
                idle_timeout: Duration::from_secs(86400),      // 24 hours
                absolute_timeout: Duration::from_secs(604800), // 7 days
                concurrent_sessions: None,
                allow_renewal: true,
                regenerate_session_id: false,
                secure_cookies: false,
                http_only_cookies: true,
                same_site_policy: SameSitePolicy::Lax,
            },
            access_control: AccessControlPolicy {
                rbac_enabled: false,
                default_role: "admin".to_string(),
                roles: Self::default_roles(),
                resource_permissions: HashMap::new(),
                ip_restrictions: IpRestrictions {
                    enabled: false,
                    whitelist_mode: false,
                    allowed_ranges: vec!["127.0.0.1/32".to_string(), "::1/128".to_string()],
                    blocked_ranges: Vec::new(),
                    geo_blocking: false,
                    allowed_countries: Vec::new(),
                    blocked_countries: Vec::new(),
                },
            },
            password_policy: PasswordPolicy {
                min_length: 6,
                max_length: 256,
                require_uppercase: false,
                require_lowercase: false,
                require_numbers: false,
                require_special_chars: false,
                min_character_types: 1,
                history_count: 0,
                expiry_days: None,
                use_blacklist: false,
                check_username_similarity: false,
            },
            api_key_policy: ApiKeyPolicy {
                key_length: 16,
                expiry_days: None,
                allow_rotation: false,
                rotation_period: None,
                scope_restrictions: false,
                per_key_rate_limits: false,
                naming_pattern: None,
            },
            audit_policy: AuditPolicy {
                enabled: true,
                log_success: false,
                log_failures: true,
                log_denials: false,
                log_sensitive_ops: false,
                retention_days: 7,
                real_time_alerts: false,
                alert_thresholds: AlertThresholds {
                    failed_logins: 100,
                    permission_denials: 100,
                    suspicious_score: 1000,
                    time_window_minutes: 60,
                },
            },
        }
    }

    /// Create testing policy (minimal security)
    pub fn testing() -> Self {
        let mut config = Self::development();
        config.authentication.require_authentication = false;
        config.audit_policy.enabled = false;
        config
    }

    /// Default role definitions
    fn default_roles() -> HashMap<String, Role> {
        let mut roles = HashMap::new();

        roles.insert(
            "admin".to_string(),
            Role {
                name: "admin".to_string(),
                description: "Administrator with full access".to_string(),
                permissions: vec![Permission {
                    resource: "*".to_string(),
                    actions: vec!["*".to_string()],
                    conditions: vec![],
                }],
                inherits_from: vec!["user".to_string()],
                priority: 100,
            },
        );

        roles.insert(
            "user".to_string(),
            Role {
                name: "user".to_string(),
                description: "Standard user with read/write access".to_string(),
                permissions: vec![
                    Permission {
                        resource: "tools".to_string(),
                        actions: vec!["read".to_string(), "execute".to_string()],
                        conditions: vec![],
                    },
                    Permission {
                        resource: "resources".to_string(),
                        actions: vec!["read".to_string()],
                        conditions: vec![],
                    },
                    Permission {
                        resource: "prompts".to_string(),
                        actions: vec!["read".to_string(), "execute".to_string()],
                        conditions: vec![],
                    },
                ],
                inherits_from: vec!["guest".to_string()],
                priority: 50,
            },
        );

        roles.insert(
            "guest".to_string(),
            Role {
                name: "guest".to_string(),
                description: "Guest with limited read-only access".to_string(),
                permissions: vec![
                    Permission {
                        resource: "health".to_string(),
                        actions: vec!["read".to_string()],
                        conditions: vec![],
                    },
                    Permission {
                        resource: "info".to_string(),
                        actions: vec!["read".to_string()],
                        conditions: vec![],
                    },
                ],
                inherits_from: vec![],
                priority: 10,
            },
        );

        roles
    }

    /// Default resource permissions
    fn default_resource_permissions() -> HashMap<String, ResourcePermission> {
        let mut permissions = HashMap::new();

        permissions.insert(
            "/admin/*".to_string(),
            ResourcePermission {
                pattern: "/admin/*".to_string(),
                required_auth_level: AuthLevel::Admin,
                required_roles: vec!["admin".to_string()],
                conditions: vec![],
            },
        );

        permissions.insert(
            "/api/*".to_string(),
            ResourcePermission {
                pattern: "/api/*".to_string(),
                required_auth_level: AuthLevel::Standard,
                required_roles: vec!["user".to_string(), "admin".to_string()],
                conditions: vec![],
            },
        );

        permissions.insert(
            "/public/*".to_string(),
            ResourcePermission {
                pattern: "/public/*".to_string(),
                required_auth_level: AuthLevel::None,
                required_roles: vec![],
                conditions: vec![],
            },
        );

        permissions
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate password policy
        if self.password_policy.min_length > self.password_policy.max_length {
            return Err(LoxoneError::invalid_input(
                "Password min length cannot exceed max length",
            ));
        }

        if self.password_policy.min_character_types > 4 {
            return Err(LoxoneError::invalid_input(
                "Cannot require more than 4 character types",
            ));
        }

        // Validate session policy
        if self.session_management.idle_timeout > self.session_management.absolute_timeout {
            return Err(LoxoneError::invalid_input(
                "Idle timeout cannot exceed absolute timeout",
            ));
        }

        // Validate roles
        for role in self.access_control.roles.values() {
            // Check for circular inheritance
            if self.has_circular_inheritance(&role.name, &mut Vec::new()) {
                return Err(LoxoneError::invalid_input(format!(
                    "Circular inheritance detected for role: {}",
                    role.name
                )));
            }
        }

        Ok(())
    }

    /// Check for circular role inheritance
    fn has_circular_inheritance(&self, role_name: &str, visited: &mut Vec<String>) -> bool {
        if visited.contains(&role_name.to_string()) {
            return true;
        }

        visited.push(role_name.to_string());

        if let Some(role) = self.access_control.roles.get(role_name) {
            for parent in &role.inherits_from {
                if self.has_circular_inheritance(parent, visited) {
                    return true;
                }
            }
        }

        visited.pop();
        false
    }

    /// Check if configuration is secure
    pub fn is_secure(&self) -> bool {
        self.authentication.require_authentication
            && !self.authentication.allow_basic_auth
            && self.session_management.secure_cookies
            && self.password_policy.min_length >= 12
            && self.audit_policy.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_policy_validation() {
        let policy = SecurityPolicyConfig::secure();
        assert!(policy.validate().is_ok());
        assert!(policy.is_secure());
    }

    #[test]
    fn test_development_policy() {
        let policy = SecurityPolicyConfig::development();
        assert!(policy.validate().is_ok());
        assert!(!policy.is_secure());
    }

    #[test]
    fn test_circular_inheritance_detection() {
        let mut policy = SecurityPolicyConfig::secure();

        // Create circular inheritance
        if let Some(admin_role) = policy.access_control.roles.get_mut("admin") {
            admin_role.inherits_from.push("admin".to_string());
        }

        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_invalid_password_policy() {
        let mut policy = SecurityPolicyConfig::secure();
        policy.password_policy.min_length = 100;
        policy.password_policy.max_length = 50;

        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_invalid_session_policy() {
        let mut policy = SecurityPolicyConfig::secure();
        policy.session_management.idle_timeout = Duration::from_secs(86400);
        policy.session_management.absolute_timeout = Duration::from_secs(3600);

        assert!(policy.validate().is_err());
    }
}
