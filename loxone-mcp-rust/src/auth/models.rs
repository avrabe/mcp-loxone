//! Unified authentication data models
//!
//! This module defines the core data structures used throughout the
//! authentication system, ensuring consistency across all components.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// User roles with granular permissions
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Full system access - all operations including user management
    Admin,
    /// Device control and monitoring - no user/key management
    Operator,
    /// Read-only access to all resources and status
    Monitor,
    /// Limited access to specific devices only
    Device {
        /// List of device UUIDs this key can control
        allowed_devices: Vec<String>,
    },
    /// Custom role with specific permission set
    Custom {
        /// List of specific permissions
        permissions: Vec<String>,
    },
}

impl Role {
    /// Check if this role has a specific permission
    pub fn has_permission(&self, permission: &str) -> bool {
        match self {
            Role::Admin => true, // Admin has all permissions
            Role::Operator => !permission.starts_with("admin."), // No admin permissions
            Role::Monitor => permission.starts_with("read.") || permission == "health.check",
            Role::Device { allowed_devices } => {
                // Check if permission is for an allowed device
                if let Some(device_uuid) = permission.strip_prefix("device.") {
                    allowed_devices.contains(&device_uuid.to_string())
                } else {
                    false
                }
            }
            Role::Custom { permissions } => permissions.contains(&permission.to_string()),
        }
    }

    /// Get a human-readable description of this role
    pub fn description(&self) -> String {
        match self {
            Role::Admin => "Full administrative access".to_string(),
            Role::Operator => "Device control and monitoring".to_string(),
            Role::Monitor => "Read-only system monitoring".to_string(),
            Role::Device { allowed_devices } => {
                format!("Device control for {} devices", allowed_devices.len())
            }
            Role::Custom { permissions } => {
                format!("Custom role with {} permissions", permissions.len())
            }
        }
    }
}

/// Unified API key structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// Unique key identifier (format: lmcp_{role}_{timestamp}_{random})
    pub id: String,
    
    /// The actual secret token used for authentication
    pub secret: String,
    
    /// Human-readable name/description
    pub name: String,
    
    /// Role-based permissions
    pub role: Role,
    
    /// Who created this key
    pub created_by: String,
    
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Optional expiration timestamp
    pub expires_at: Option<DateTime<Utc>>,
    
    /// IP address whitelist (empty = all IPs allowed)
    #[serde(default)]
    pub ip_whitelist: Vec<String>,
    
    /// Is the key currently active
    pub active: bool,
    
    /// Last time this key was used
    #[serde(default)]
    pub last_used: Option<DateTime<Utc>>,
    
    /// Number of times this key has been used
    #[serde(default)]
    pub usage_count: u64,
    
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl ApiKey {
    /// Create a new API key with generated ID and secret
    pub fn new(name: String, role: Role, created_by: String, expires_in_days: Option<u32>) -> Self {
        let id = Self::generate_id(&role);
        let secret = Self::generate_secret(&role);
        
        Self {
            id,
            secret,
            name,
            role,
            created_by,
            created_at: Utc::now(),
            expires_at: expires_in_days.map(|days| {
                Utc::now() + chrono::Duration::days(days as i64)
            }),
            ip_whitelist: Vec::new(),
            active: true,
            last_used: None,
            usage_count: 0,
            metadata: HashMap::new(),
        }
    }
    
    /// Generate a unique key ID based on role and timestamp
    fn generate_id(role: &Role) -> String {
        let role_prefix = match role {
            Role::Admin => "admin",
            Role::Operator => "operator", 
            Role::Monitor => "monitor",
            Role::Device { .. } => "device",
            Role::Custom { .. } => "custom",
        };
        
        let timestamp = Utc::now().timestamp_millis() % 100000; // Last 5 digits
        let random = &Uuid::new_v4().to_string().replace('-', "")[..8];
        
        format!("lmcp_{}_{:05}_{}", role_prefix, timestamp, random)
    }
    
    /// Generate a cryptographically secure secret token
    fn generate_secret(role: &Role) -> String {
        let role_prefix = match role {
            Role::Admin => "admin",
            Role::Operator => "op", 
            Role::Monitor => "mon",
            Role::Device { .. } => "dev",
            Role::Custom { .. } => "cust",
        };
        
        let random_part = Uuid::new_v4().to_string().replace('-', "");
        format!("lmcp_{}_{}", role_prefix, random_part)
    }
    
    /// Check if this API key is currently valid
    pub fn is_valid(&self) -> bool {
        if !self.active {
            return false;
        }
        
        if let Some(expires_at) = self.expires_at {
            if Utc::now() > expires_at {
                return false;
            }
        }
        
        true
    }
    
    /// Check if this key is allowed to access from the given IP
    pub fn is_ip_allowed(&self, ip: &str) -> bool {
        if self.ip_whitelist.is_empty() {
            return true; // No restrictions
        }
        
        self.ip_whitelist.iter().any(|allowed_ip| {
            ip == allowed_ip || self.ip_matches_pattern(ip, allowed_ip)
        })
    }
    
    /// Check if IP matches a pattern (supports CIDR and wildcards)
    fn ip_matches_pattern(&self, ip: &str, pattern: &str) -> bool {
        // Simple wildcard matching for now
        // TODO: Implement proper CIDR matching
        if pattern.contains('*') {
            let pattern_parts: Vec<&str> = pattern.split('.').collect();
            let ip_parts: Vec<&str> = ip.split('.').collect();
            
            if pattern_parts.len() != ip_parts.len() {
                return false;
            }
            
            pattern_parts.iter().zip(ip_parts.iter()).all(|(p, i)| {
                p == &"*" || p == i
            })
        } else {
            ip == pattern
        }
    }
    
    /// Update usage statistics
    pub fn record_usage(&mut self) {
        self.last_used = Some(Utc::now());
        self.usage_count += 1;
    }
}

/// Successful authentication data
#[derive(Debug, Clone)]
pub struct AuthSuccess {
    /// Authenticated key
    pub key: ApiKey,
    /// Generated session context
    pub context: AuthContext,
}

/// Authentication result
#[derive(Debug, Clone)]
pub enum AuthResult {
    /// Authentication successful
    Success(Box<AuthSuccess>),
    /// Authentication failed due to invalid credentials
    Unauthorized {
        /// Reason for failure
        reason: String,
    },
    /// Authentication failed due to insufficient permissions
    Forbidden {
        /// Reason for denial
        reason: String,
    },
    /// Authentication failed due to rate limiting
    RateLimited {
        /// When to retry
        retry_after_seconds: u64,
    },
}

/// Authentication context for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    /// Session identifier
    pub session_id: String,
    
    /// API key being used
    pub key_id: String,
    
    /// User role
    pub role: Role,
    
    /// Client IP address
    pub client_ip: String,
    
    /// Session creation time
    pub created_at: DateTime<Utc>,
    
    /// Last activity time
    pub last_activity: DateTime<Utc>,
    
    /// Request count in this session
    pub request_count: u64,
}

impl AuthContext {
    /// Create a new authentication context
    pub fn new(key: &ApiKey, client_ip: String) -> Self {
        Self {
            session_id: Uuid::new_v4().to_string(),
            key_id: key.id.clone(),
            role: key.role.clone(),
            client_ip,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            request_count: 0,
        }
    }
    
    /// Check if this session is still valid
    pub fn is_valid(&self, timeout_minutes: u64) -> bool {
        let timeout_threshold = Utc::now() - chrono::Duration::minutes(timeout_minutes as i64);
        self.last_activity > timeout_threshold
    }
    
    /// Update session activity
    pub fn record_activity(&mut self) {
        self.last_activity = Utc::now();
        self.request_count += 1;
    }
}

/// Security audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Event type (auth_success, auth_failure, key_created, etc.)
    pub event_type: String,
    
    /// API key involved (if any)
    pub key_id: Option<String>,
    
    /// Client IP address
    pub client_ip: String,
    
    /// User agent or client identifier
    pub user_agent: Option<String>,
    
    /// Success or failure
    pub success: bool,
    
    /// Additional details
    pub details: HashMap<String, String>,
}

impl AuditEvent {
    /// Create a new authentication success event
    pub fn auth_success(key_id: String, client_ip: String) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type: "auth_success".to_string(),
            key_id: Some(key_id),
            client_ip,
            user_agent: None,
            success: true,
            details: HashMap::new(),
        }
    }
    
    /// Create a new authentication failure event
    pub fn auth_failure(client_ip: String, reason: String) -> Self {
        let mut details = HashMap::new();
        details.insert("reason".to_string(), reason);
        
        Self {
            timestamp: Utc::now(),
            event_type: "auth_failure".to_string(),
            key_id: None,
            client_ip,
            user_agent: None,
            success: false,
            details,
        }
    }
    
    /// Create a new key creation event
    pub fn key_created(key_id: String, creator: String, role: Role) -> Self {
        let mut details = HashMap::new();
        details.insert("creator".to_string(), creator);
        details.insert("role".to_string(), format!("{:?}", role));
        
        Self {
            timestamp: Utc::now(),
            event_type: "key_created".to_string(),
            key_id: Some(key_id),
            client_ip: "system".to_string(),
            user_agent: None,
            success: true,
            details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_creation() {
        let key = ApiKey::new(
            "Test Key".to_string(),
            Role::Operator,
            "test_user".to_string(),
            Some(365),
        );
        
        assert!(key.id.starts_with("lmcp_operator_"));
        assert!(key.secret.starts_with("lmcp_op_"));
        assert_eq!(key.name, "Test Key");
        assert_eq!(key.role, Role::Operator);
        assert!(key.is_valid());
        assert!(key.expires_at.is_some());
    }

    #[test]
    fn test_role_permissions() {
        let admin = Role::Admin;
        let operator = Role::Operator;
        let monitor = Role::Monitor;
        
        assert!(admin.has_permission("admin.create_user"));
        assert!(admin.has_permission("device.control"));
        
        assert!(!operator.has_permission("admin.create_user"));
        assert!(operator.has_permission("device.control"));
        
        assert!(!monitor.has_permission("device.control"));
        assert!(monitor.has_permission("read.status"));
    }

    #[test]
    fn test_ip_whitelist() {
        let mut key = ApiKey::new(
            "Test".to_string(),
            Role::Operator,
            "test".to_string(),
            None,
        );
        
        // No restrictions - all IPs allowed
        assert!(key.is_ip_allowed("192.168.1.1"));
        assert!(key.is_ip_allowed("10.0.0.1"));
        
        // Add specific IP
        key.ip_whitelist.push("192.168.1.1".to_string());
        assert!(key.is_ip_allowed("192.168.1.1"));
        assert!(!key.is_ip_allowed("192.168.1.2"));
        
        // Add wildcard pattern
        key.ip_whitelist.push("192.168.1.*".to_string());
        assert!(key.is_ip_allowed("192.168.1.100"));
        assert!(!key.is_ip_allowed("192.168.2.1"));
    }
}