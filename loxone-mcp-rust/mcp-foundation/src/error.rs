//! Error types for MCP Foundation

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// MCP Foundation result type
pub type Result<T> = std::result::Result<T, Error>;

/// MCP Foundation error types
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    /// Invalid request format or parameters
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    /// Method not found
    #[error("Method not found: {method}")]
    MethodNotFound { method: String },

    /// Invalid parameters
    #[error("Invalid parameters: {message}")]
    InvalidParams { message: String },

    /// Internal server error
    #[error("Internal error: {message}")]
    InternalError { message: String },

    /// Parse error
    #[error("Parse error: {message}")]
    ParseError { message: String },

    /// Connection error
    #[error("Connection error: {message}")]
    ConnectionError { message: String },

    /// Timeout error
    #[error("Timeout: {message}")]
    Timeout { message: String },

    /// Resource not found
    #[error("Resource not found: {resource}")]
    ResourceNotFound { resource: String },

    /// Tool not found
    #[error("Tool not found: {tool}")]
    ToolNotFound { tool: String },

    /// Authorization error
    #[error("Authorization error: {message}")]
    AuthorizationError { message: String },

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {message}")]
    RateLimitExceeded { message: String },
}

impl Error {
    /// Create an invalid request error
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            message: message.into(),
        }
    }

    /// Create a method not found error
    pub fn method_not_found(method: impl Into<String>) -> Self {
        Self::MethodNotFound {
            method: method.into(),
        }
    }

    /// Create an invalid parameters error
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::InvalidParams {
            message: message.into(),
        }
    }

    /// Create an internal error
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::InternalError {
            message: message.into(),
        }
    }

    /// Create a parse error
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
        }
    }

    /// Create a connection error
    pub fn connection_error(message: impl Into<String>) -> Self {
        Self::ConnectionError {
            message: message.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::Timeout {
            message: message.into(),
        }
    }

    /// Create a resource not found error
    pub fn resource_not_found(resource: impl Into<String>) -> Self {
        Self::ResourceNotFound {
            resource: resource.into(),
        }
    }

    /// Create a tool not found error
    pub fn tool_not_found(tool: impl Into<String>) -> Self {
        Self::ToolNotFound { tool: tool.into() }
    }

    /// Create an authorization error
    pub fn authorization_error(message: impl Into<String>) -> Self {
        Self::AuthorizationError {
            message: message.into(),
        }
    }

    /// Create a rate limit exceeded error
    pub fn rate_limit_exceeded(message: impl Into<String>) -> Self {
        Self::RateLimitExceeded {
            message: message.into(),
        }
    }

    /// Get the JSON-RPC error code for this error
    pub fn json_rpc_code(&self) -> i32 {
        match self {
            Error::InvalidRequest { .. } => -32600,
            Error::MethodNotFound { .. } => -32601,
            Error::InvalidParams { .. } => -32602,
            Error::ParseError { .. } => -32700,
            Error::InternalError { .. } => -32603,
            Error::ConnectionError { .. } => -32001,
            Error::Timeout { .. } => -32002,
            Error::ResourceNotFound { .. } => -32004,
            Error::ToolNotFound { .. } => -32005,
            Error::AuthorizationError { .. } => -32006,
            Error::RateLimitExceeded { .. } => -32007,
        }
    }
}

/// Convert from standard I/O errors
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::connection_error(err.to_string())
    }
}

/// Convert from JSON serialization errors
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::parse_error(err.to_string())
    }
}
