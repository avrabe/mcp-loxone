//! Unified Authentication System for Loxone MCP Server
//!
//! This module provides a single, coherent authentication system that replaces
//! the previous fragmented approach. It supports multiple storage backends,
//! role-based access control, and consistent API key management across all
//! server components.
//!
//! ## Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                Authentication Manager                        │
//! │  ┌─────────────────┐    ┌──────────────────────────────┐   │
//! │  │   Validation    │    │         Key Storage          │   │
//! │  │   - Role check  │    │  ┌─────────┐  ┌─────────────┐│   │
//! │  │   - Expiry      │    │  │  File   │  │ Environment ││   │
//! │  │   - IP filter   │    │  │ Storage │  │   Storage   ││   │
//! │  │   - Rate limit  │    │  └─────────┘  └─────────────┘│   │
//! │  └─────────────────┘    └──────────────────────────────┘   │
//! │           │                           │                    │
//! │  ┌─────────────────┐    ┌──────────────────────────────┐   │
//! │  │  Memory Cache   │    │       Audit Logger          │   │
//! │  │  - Hot keys     │    │  - Auth events              │   │
//! │  │  - Fast lookup  │    │  - Security alerts          │   │
//! │  └─────────────────┘    └──────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────┘
//!          │                    │                    │
//!    ┌─────────────┐    ┌──────────────┐    ┌─────────────────┐
//!    │ HTTP/Axum   │    │  WebSocket   │    │  MCP Protocol   │
//!    │ Middleware  │    │     Auth     │    │      Auth       │
//!    └─────────────┘    └──────────────┘    └─────────────────┘
//! ```

pub mod manager;
pub mod middleware;
pub mod models;
pub mod security;
pub mod storage;
pub mod token_validator;
pub mod validation;

pub use manager::AuthenticationManager;
pub use models::{ApiKey, AuthContext, AuthResult, Role};
pub use storage::{EnvironmentStorage, FileStorage, StorageBackend};
pub use validation::Validator;

use std::sync::Arc;

/// Initialize the global authentication system
pub async fn initialize_auth_system() -> crate::error::Result<Arc<AuthenticationManager>> {
    let manager = AuthenticationManager::new().await?;
    Ok(Arc::new(manager))
}

/// Default authentication configuration
pub fn default_auth_config() -> AuthConfig {
    AuthConfig {
        storage_backend: storage::StorageBackendConfig::File {
            path: dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".loxone-mcp")
                .join("credentials.json"),
        },
        cache_size: 1000,
        enable_audit_log: true,
        session_timeout_minutes: 480, // 8 hours
        max_failed_attempts: 5,
        failed_attempt_window_minutes: 15,
    }
}

/// Authentication system configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Storage backend configuration
    pub storage_backend: storage::StorageBackendConfig,
    /// Number of keys to keep in memory cache
    pub cache_size: usize,
    /// Enable security audit logging
    pub enable_audit_log: bool,
    /// Session timeout in minutes
    pub session_timeout_minutes: u64,
    /// Maximum failed authentication attempts
    pub max_failed_attempts: u32,
    /// Time window for failed attempts (minutes)
    pub failed_attempt_window_minutes: u64,
}
