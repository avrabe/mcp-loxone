//! Authentication configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Storage backend configuration
    pub storage: StorageConfig,
    /// Enable authentication (if false, all requests are allowed)
    pub enabled: bool,
    /// Cache size for API keys
    pub cache_size: usize,
    /// Session timeout in seconds
    pub session_timeout_secs: u64,
    /// Maximum failed attempts before rate limiting
    pub max_failed_attempts: u32,
    /// Rate limiting window in seconds
    pub rate_limit_window_secs: u64,
}

/// Storage configuration for authentication data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageConfig {
    /// File-based storage
    File {
        /// Path to storage directory
        path: PathBuf,
    },
    /// Environment variable storage
    Environment {
        /// Prefix for environment variables
        prefix: String,
    },
    /// Memory-only storage (for testing)
    Memory,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            storage: StorageConfig::File {
                path: dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".loxone")
                    .join("auth"),
            },
            enabled: true,
            cache_size: 1000,
            session_timeout_secs: 3600, // 1 hour
            max_failed_attempts: 5,
            rate_limit_window_secs: 900, // 15 minutes
        }
    }
}

impl AuthConfig {
    /// Create a disabled authentication configuration
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }
    
    /// Create a memory-only configuration (for testing)
    pub fn memory() -> Self {
        Self {
            storage: StorageConfig::Memory,
            ..Default::default()
        }
    }
}