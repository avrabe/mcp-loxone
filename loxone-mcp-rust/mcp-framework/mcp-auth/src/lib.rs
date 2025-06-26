//! Authentication and authorization framework for MCP servers

pub mod config;
pub mod manager;
pub mod storage;
pub mod models;

// Re-export main types
pub use config::AuthConfig;
pub use manager::AuthenticationManager;
pub use models::{ApiKey, Role, AuthResult, AuthContext};
pub use storage::{StorageBackend, FileStorage, EnvironmentStorage};

/// Initialize default authentication configuration
pub fn default_config() -> AuthConfig {
    AuthConfig::default()
}

/// Create an authentication manager with default configuration
pub async fn create_auth_manager() -> Result<AuthenticationManager, crate::manager::AuthError> {
    AuthenticationManager::new(default_config()).await
}