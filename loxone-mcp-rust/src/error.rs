//! Error types for the Loxone MCP server

use thiserror::Error;

/// Result type alias for Loxone operations
pub type Result<T> = std::result::Result<T, LoxoneError>;

/// Comprehensive error types for Loxone MCP operations
#[derive(Error, Debug)]
pub enum LoxoneError {
    /// Connection errors
    #[error("Connection error: {0}")]
    Connection(String),

    /// Authentication errors  
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// HTTP client errors
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON parsing errors
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Credential storage errors
    #[error("Credential error: {0}")]
    Credentials(String),

    /// Device control errors
    #[error("Device control error: {0}")]
    DeviceControl(String),

    /// Sensor discovery errors
    #[error("Sensor discovery error: {0}")]
    SensorDiscovery(String),

    /// Network discovery errors
    #[error("Discovery failed: {0}")]
    Discovery(String),

    /// WebSocket errors
    #[cfg(feature = "websocket")]
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// Encryption/decryption errors
    #[cfg(feature = "crypto")]
    #[error("Crypto error: {0}")]
    Crypto(String),

    /// MCP protocol errors
    #[error("MCP protocol error: {0}")]
    Mcp(String),

    /// WASM-specific errors
    #[cfg(target_arch = "wasm32")]
    #[error("WASM error: {0}")]
    Wasm(String),

    /// Generic I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic errors
    #[error("Generic error: {0}")]
    Generic(#[from] anyhow::Error),

    /// Timeout errors
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Invalid input errors
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Not found errors (devices, rooms, etc.)
    #[error("Not found: {0}")]
    NotFound(String),

    /// Permission denied errors
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Service unavailable
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Resource exhausted
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Consent denied errors
    #[error("Consent denied: {0}")]
    ConsentDenied(String),
}

/// Sanitized error representation for production logging
#[derive(Debug, Clone, serde::Serialize)]
pub struct SanitizedError {
    pub error_type: String,
    pub message: String,
    pub is_retryable: bool,
    pub is_auth_error: bool,
}

impl LoxoneError {
    /// Create a connection error
    pub fn connection<S: Into<String>>(msg: S) -> Self {
        Self::Connection(msg.into())
    }

    /// Create an authentication error
    pub fn authentication<S: Into<String>>(msg: S) -> Self {
        Self::Authentication(msg.into())
    }

    /// Create a configuration error
    pub fn config<S: Into<String>>(msg: S) -> Self {
        Self::Config(msg.into())
    }

    /// Create a credentials error
    pub fn credentials<S: Into<String>>(msg: S) -> Self {
        Self::Credentials(msg.into())
    }

    /// Create a device control error
    pub fn device_control<S: Into<String>>(msg: S) -> Self {
        Self::DeviceControl(msg.into())
    }

    /// Create a sensor discovery error
    pub fn sensor_discovery<S: Into<String>>(msg: S) -> Self {
        Self::SensorDiscovery(msg.into())
    }

    /// Create a discovery error
    pub fn discovery<S: Into<String>>(msg: S) -> Self {
        Self::Discovery(msg.into())
    }

    /// Create a timeout error
    pub fn timeout<S: Into<String>>(msg: S) -> Self {
        Self::Timeout(msg.into())
    }

    /// Create an invalid input error
    pub fn invalid_input<S: Into<String>>(msg: S) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a not found error
    pub fn not_found<S: Into<String>>(msg: S) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create a resource exhausted error
    pub fn resource_exhausted<S: Into<String>>(msg: S) -> Self {
        Self::ResourceExhausted(msg.into())
    }

    /// Create a consent denied error
    pub fn consent_denied<S: Into<String>>(msg: S) -> Self {
        Self::ConsentDenied(msg.into())
    }

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            LoxoneError::Connection(_)
                | LoxoneError::Timeout(_)
                | LoxoneError::ServiceUnavailable(_)
                | LoxoneError::Http(_)
        )
    }

    /// Check if error indicates authentication issue
    pub fn is_auth_error(&self) -> bool {
        matches!(
            self,
            LoxoneError::Authentication(_)
                | LoxoneError::Credentials(_)
                | LoxoneError::PermissionDenied(_)
        )
    }

    /// Get a production-safe error message that doesn't expose sensitive information
    pub fn sanitized_message(&self) -> String {
        #[cfg(debug_assertions)]
        {
            // In debug builds, show full error details
            self.to_string()
        }
        #[cfg(not(debug_assertions))]
        {
            // In production builds, sanitize sensitive information
            match self {
                LoxoneError::Authentication(_) => "Authentication failed".to_string(),
                LoxoneError::Credentials(_) => "Credential access error".to_string(),
                LoxoneError::PermissionDenied(_) => "Access denied".to_string(),
                LoxoneError::Connection(_) => "Network connection issue".to_string(),
                LoxoneError::Timeout(_) => "Operation timed out".to_string(),
                LoxoneError::Http(_) => "HTTP request failed".to_string(),
                LoxoneError::Config(_) => "Configuration error".to_string(),
                LoxoneError::DeviceControl(_) => "Device control failed".to_string(),
                LoxoneError::SensorDiscovery(_) => "Sensor discovery failed".to_string(),
                LoxoneError::Discovery(_) => "Network discovery failed".to_string(),
                LoxoneError::Mcp(_) => "MCP protocol error".to_string(),
                LoxoneError::InvalidInput(_) => "Invalid input provided".to_string(),
                LoxoneError::NotFound(_) => "Requested resource not found".to_string(),
                LoxoneError::ServiceUnavailable(_) => "Service temporarily unavailable".to_string(),
                LoxoneError::ResourceExhausted(_) => "Resource limits exceeded".to_string(),
                LoxoneError::ConsentDenied(_) => "Operation requires user consent".to_string(),
                LoxoneError::Io(_) => "I/O operation failed".to_string(),
                LoxoneError::Json(_) => "Data parsing error".to_string(),
                LoxoneError::Generic(_) => "Internal error occurred".to_string(),
                #[cfg(feature = "websocket")]
                LoxoneError::WebSocket(_) => "WebSocket connection error".to_string(),
                #[cfg(feature = "crypto")]
                LoxoneError::Crypto(_) => "Cryptographic operation failed".to_string(),
                #[cfg(target_arch = "wasm32")]
                LoxoneError::Wasm(_) => "WASM runtime error".to_string(),
            }
        }
    }

    /// Create a sanitized version of the error for logging
    pub fn sanitized_error(&self) -> SanitizedError {
        SanitizedError {
            error_type: format!("{:?}", std::mem::discriminant(self)),
            message: self.sanitized_message(),
            is_retryable: self.is_retryable(),
            is_auth_error: self.is_auth_error(),
        }
    }
}

#[cfg(feature = "websocket")]
impl From<tokio_tungstenite::tungstenite::Error> for LoxoneError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        LoxoneError::WebSocket(err.to_string())
    }
}

#[cfg(feature = "crypto")]
impl From<rsa::Error> for LoxoneError {
    fn from(err: rsa::Error) -> Self {
        LoxoneError::Crypto(format!("RSA error: {}", err))
    }
}

#[cfg(feature = "keyring-storage")]
impl From<keyring::Error> for LoxoneError {
    fn from(err: keyring::Error) -> Self {
        LoxoneError::Credentials(format!("Keyring error: {}", err))
    }
}
