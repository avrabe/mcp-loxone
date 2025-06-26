//! Structured logging framework for MCP servers
//!
//! This crate provides comprehensive logging capabilities for MCP servers including:
//! - Structured logging with tracing
//! - Metrics collection and reporting  
//! - Log sanitization for security
//! - Performance monitoring integration
//!
//! # Example
//!
//! ```rust,no_run
//! use mcp_logging::{MetricsCollector, StructuredLogger};
//!
//! #[tokio::main]
//! async fn main() {
//!     let metrics = MetricsCollector::new();
//!     let logger = StructuredLogger::new();
//!     
//!     // Initialize structured logging
//!     logger.init().expect("Failed to initialize logging");
//!     
//!     // Log with structured context
//!     tracing::info!("Server started", server_type = "mcp", version = "1.0");
//! }
//! ```

pub mod metrics;
pub mod sanitization;
pub mod structured;

// Re-export main types for convenience
pub use metrics::{
    MetricsCollector, RequestMetrics, HealthMetrics, 
    BusinessMetrics, ErrorMetrics, MetricsSnapshot, ErrorRecord,
    get_metrics
};
pub use sanitization::{LogSanitizer, SanitizationConfig};
pub use structured::{
    StructuredLogger, StructuredContext, ErrorClass
};

/// Result type for logging operations
pub type Result<T> = std::result::Result<T, LoggingError>;

/// Logging error types
#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Tracing error: {0}")]
    Tracing(String),
}

/// Generic error trait for classification
pub trait ErrorClassification: std::fmt::Display + std::error::Error {
    fn error_type(&self) -> &str;
    fn is_retryable(&self) -> bool;
    fn is_timeout(&self) -> bool;
    fn is_auth_error(&self) -> bool;
    fn is_connection_error(&self) -> bool;
}