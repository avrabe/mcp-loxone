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

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            LoxoneError::Connection(_) | 
            LoxoneError::Timeout(_) |
            LoxoneError::ServiceUnavailable(_) |
            LoxoneError::Http(_)
        )
    }

    /// Check if error indicates authentication issue
    pub fn is_auth_error(&self) -> bool {
        matches!(
            self,
            LoxoneError::Authentication(_) |
            LoxoneError::Credentials(_) |
            LoxoneError::PermissionDenied(_)
        )
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