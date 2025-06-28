//! Security middleware and validation for MCP servers
//!
//! This crate provides comprehensive security features for MCP servers including:
//! - Input validation and sanitization
//! - Rate limiting and request throttling
//! - CORS policy management
//! - Request size limits
//! - SQL injection and XSS protection
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use pulseengine_mcp_security::{SecurityMiddleware, SecurityConfig, RequestValidator};
//! use pulseengine_mcp_protocol::Request;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create security configuration
//!     let config = SecurityConfig {
//!         max_request_size: 1024 * 1024, // 1MB limit
//!         rate_limit_requests_per_minute: 60,
//!         allowed_origins: vec!["https://example.com".to_string()],
//!         enable_ip_whitelist: true,
//!         allowed_ips: vec!["192.168.1.0/24".to_string()],
//!         ..Default::default()
//!     };
//!
//!     // Create security middleware
//!     let security = SecurityMiddleware::new(config);
//!
//!     // Validate requests
//!     let validator = RequestValidator::new();
//!
//!     // In your request handler:
//!     // let is_valid = validator.validate_request(&request).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! - **Input validation**: Comprehensive request validation with schemas
//! - **Rate limiting**: Per-IP and per-user rate limiting
//! - **CORS management**: Configurable cross-origin policies
//! - **Size limits**: Prevent DoS through large requests
//! - **Injection protection**: SQL injection and script injection prevention
//! - **Production hardened**: Battle-tested security measures

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
