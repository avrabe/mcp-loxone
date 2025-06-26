//! Security configuration

use serde::{Deserialize, Serialize};

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable request validation
    pub validate_requests: bool,
    /// Enable rate limiting
    pub rate_limiting: bool,
    /// Maximum requests per minute
    pub max_requests_per_minute: u32,
    /// Enable CORS
    pub cors_enabled: bool,
    /// Allowed origins for CORS
    pub cors_origins: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            validate_requests: true,
            rate_limiting: true,
            max_requests_per_minute: 60,
            cors_enabled: false,
            cors_origins: vec!["*".to_string()],
        }
    }
}