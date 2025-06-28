//! Authentication and authorization framework for MCP servers
//!
//! This crate provides secure authentication mechanisms for MCP servers including:
//! - API key management with roles and permissions
//! - Token-based authentication with expiration
//! - IP whitelisting and rate limiting
//! - Multiple storage backends (file, environment, database)
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use pulseengine_mcp_auth::{AuthenticationManager, AuthConfig, Role};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create authentication manager
//!     let config = AuthConfig::default();
//!     let mut auth_manager = AuthenticationManager::new(config).await?;
//!
//!     // Create API key for admin user
//!     let api_key = auth_manager.create_api_key(
//!         "admin-key".to_string(),
//!         Role::Admin,
//!         None, // No expiration
//!         Some(vec!["192.168.1.0/24".to_string()]) // IP whitelist
//!     ).await?;
//!
//!     println!("Created API key: {}", api_key.key);
//!
//!     // Validate API key in request handler
//!     let is_valid = auth_manager.validate_api_key(&api_key.key).await?;
//!     println!("Key is valid: {}", is_valid.is_some());
//!
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! - **Role-based access control**: Admin, Operator, ReadOnly roles
//! - **Secure key generation**: Cryptographically secure random keys
//! - **Flexible storage**: File-based, environment variables, or custom backends
//! - **IP restrictions**: Optional IP whitelisting per key
//! - **Audit logging**: Track key usage and authentication events
//! - **Production ready**: Used in real-world deployments

pub mod config;
pub mod manager;
pub mod models;
pub mod storage;

// Re-export main types
pub use config::AuthConfig;
pub use manager::AuthenticationManager;
pub use models::{ApiKey, AuthContext, AuthResult, Role};
pub use storage::{EnvironmentStorage, FileStorage, StorageBackend};

/// Initialize default authentication configuration
pub fn default_config() -> AuthConfig {
    AuthConfig::default()
}

/// Create an authentication manager with default configuration
pub async fn create_auth_manager() -> Result<AuthenticationManager, crate::manager::AuthError> {
    AuthenticationManager::new(default_config()).await
}
