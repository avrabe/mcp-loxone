//! Security middleware and validation for MCP servers

pub mod config;
pub mod middleware;
pub mod validation;

pub use config::SecurityConfig;
pub use middleware::SecurityMiddleware;
pub use validation::RequestValidator;

/// Default security configuration
pub fn default_config() -> SecurityConfig {
    SecurityConfig::default()
}