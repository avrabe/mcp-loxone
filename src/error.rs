//! Error types for the Loxone MCP server
//!
//! This module provides comprehensive error handling with structured error codes,
//! recovery suggestions, and production-safe logging integration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

    /// Database errors
    #[error("Database error: {0}")]
    Database(String),

    /// Cryptographic errors
    #[error("Crypto error: {0}")]
    Crypto(String),

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

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Network errors
    #[error("Network error: {0}")]
    Network(String),

    /// External service errors
    #[error("External service error: {0}")]
    ExternalService(String),

    /// Parsing errors
    #[error("Parsing error: {0}")]
    Parsing(String),
}

/// Structured error code for machine-readable error handling
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    // Connection errors (1000-1099)
    ConnectionTimeout,
    ConnectionRefused,
    ConnectionLost,
    NetworkUnreachable,

    // Authentication errors (1100-1199)
    InvalidCredentials,
    AuthenticationExpired,
    PermissionDenied,
    ConsentRequired,

    // Configuration errors (1200-1299)
    ConfigurationMissing,
    ConfigurationInvalid,
    ConfigurationCorrupted,

    // Device errors (1300-1399)
    DeviceNotFound,
    DeviceOffline,
    DeviceControlFailed,
    DeviceTypeUnsupported,

    // Data errors (1400-1499)
    ParsingFailed,
    InvalidInput,
    ValidationFailed,
    DataCorrupted,

    // Resource errors (1500-1599)
    ResourceExhausted,
    RateLimitExceeded,
    QuotaExceeded,
    StorageFull,

    // Service errors (1600-1699)
    ServiceUnavailable,
    ServiceTimeout,
    ExternalServiceError,
    DependencyFailure,

    // Protocol errors (1700-1799)
    ProtocolViolation,
    UnsupportedOperation,
    MessageMalformed,
    VersionMismatch,

    // Security errors (1800-1899)
    CryptographicError,
    IntegrityViolation,
    SecurityPolicyViolation,

    // Internal errors (1900-1999)
    InternalError,
    NotImplemented,
    UnexpectedState,
}

impl ErrorCode {
    /// Get numeric error code
    pub fn as_number(&self) -> u32 {
        match self {
            // Connection errors (1000-1099)
            ErrorCode::ConnectionTimeout => 1001,
            ErrorCode::ConnectionRefused => 1002,
            ErrorCode::ConnectionLost => 1003,
            ErrorCode::NetworkUnreachable => 1004,

            // Authentication errors (1100-1199)
            ErrorCode::InvalidCredentials => 1101,
            ErrorCode::AuthenticationExpired => 1102,
            ErrorCode::PermissionDenied => 1103,
            ErrorCode::ConsentRequired => 1104,

            // Configuration errors (1200-1299)
            ErrorCode::ConfigurationMissing => 1201,
            ErrorCode::ConfigurationInvalid => 1202,
            ErrorCode::ConfigurationCorrupted => 1203,

            // Device errors (1300-1399)
            ErrorCode::DeviceNotFound => 1301,
            ErrorCode::DeviceOffline => 1302,
            ErrorCode::DeviceControlFailed => 1303,
            ErrorCode::DeviceTypeUnsupported => 1304,

            // Data errors (1400-1499)
            ErrorCode::ParsingFailed => 1401,
            ErrorCode::InvalidInput => 1402,
            ErrorCode::ValidationFailed => 1403,
            ErrorCode::DataCorrupted => 1404,

            // Resource errors (1500-1599)
            ErrorCode::ResourceExhausted => 1501,
            ErrorCode::RateLimitExceeded => 1502,
            ErrorCode::QuotaExceeded => 1503,
            ErrorCode::StorageFull => 1504,

            // Service errors (1600-1699)
            ErrorCode::ServiceUnavailable => 1601,
            ErrorCode::ServiceTimeout => 1602,
            ErrorCode::ExternalServiceError => 1603,
            ErrorCode::DependencyFailure => 1604,

            // Protocol errors (1700-1799)
            ErrorCode::ProtocolViolation => 1701,
            ErrorCode::UnsupportedOperation => 1702,
            ErrorCode::MessageMalformed => 1703,
            ErrorCode::VersionMismatch => 1704,

            // Security errors (1800-1899)
            ErrorCode::CryptographicError => 1801,
            ErrorCode::IntegrityViolation => 1802,
            ErrorCode::SecurityPolicyViolation => 1803,

            // Internal errors (1900-1999)
            ErrorCode::InternalError => 1901,
            ErrorCode::NotImplemented => 1902,
            ErrorCode::UnexpectedState => 1903,
        }
    }

    /// Get error category
    pub fn category(&self) -> &'static str {
        match self.as_number() {
            1000..=1099 => "connection",
            1100..=1199 => "authentication",
            1200..=1299 => "configuration",
            1300..=1399 => "device",
            1400..=1499 => "data",
            1500..=1599 => "resource",
            1600..=1699 => "service",
            1700..=1799 => "protocol",
            1800..=1899 => "security",
            1900..=1999 => "internal",
            _ => "unknown",
        }
    }
}

/// Recovery suggestion for error handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverySuggestion {
    /// Human-readable description of the suggested action
    pub description: String,
    /// Whether this action can be automated
    pub automated: bool,
    /// Code snippet or command to execute recovery
    pub action_code: Option<String>,
    /// Estimated recovery time in seconds
    pub estimated_time_seconds: Option<u32>,
    /// Prerequisites for this recovery action
    pub prerequisites: Vec<String>,
}

/// Structured error context with additional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Error code for machine processing
    pub code: ErrorCode,
    /// Component that generated the error
    pub component: String,
    /// Operation that was being performed
    pub operation: String,
    /// Additional metadata about the error
    pub metadata: HashMap<String, serde_json::Value>,
    /// Recovery suggestions
    pub recovery_suggestions: Vec<RecoverySuggestion>,
    /// Timestamp when error occurred
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Request/session ID for correlation
    pub correlation_id: Option<String>,
    /// Stack trace (only in debug builds)
    #[cfg(debug_assertions)]
    pub stack_trace: Option<String>,
}

impl ErrorContext {
    /// Create new error context
    pub fn new(code: ErrorCode, component: &str, operation: &str) -> Self {
        Self {
            code,
            component: component.to_string(),
            operation: operation.to_string(),
            metadata: HashMap::new(),
            recovery_suggestions: Vec::new(),
            timestamp: chrono::Utc::now(),
            correlation_id: None,
            #[cfg(debug_assertions)]
            stack_trace: None,
        }
    }

    /// Add metadata to error context
    pub fn with_metadata<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<serde_json::Value>,
    {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Add recovery suggestion
    pub fn with_recovery(mut self, suggestion: RecoverySuggestion) -> Self {
        self.recovery_suggestions.push(suggestion);
        self
    }

    /// Set correlation ID for request tracking
    pub fn with_correlation_id<S: Into<String>>(mut self, id: S) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    /// Add stack trace in debug builds
    #[cfg(debug_assertions)]
    pub fn with_stack_trace(mut self) -> Self {
        self.stack_trace = Some(format!("{:?}", backtrace::Backtrace::new()));
        self
    }
}

/// Enhanced error representation for production logging and monitoring
#[derive(Debug, Clone, Serialize)]
pub struct StructuredError {
    /// Error code for machine processing
    pub code: ErrorCode,
    /// Numeric error code
    pub code_number: u32,
    /// Error category
    pub category: &'static str,
    /// Production-safe error message
    pub message: String,
    /// Original error message (only in debug builds)
    #[cfg(debug_assertions)]
    pub debug_message: String,
    /// Whether this error is retryable
    pub is_retryable: bool,
    /// Whether this is an authentication error
    pub is_auth_error: bool,
    /// Component that generated the error
    pub component: String,
    /// Operation that was being performed
    pub operation: String,
    /// Additional context metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Recovery suggestions
    pub recovery_suggestions: Vec<RecoverySuggestion>,
    /// Error severity level
    pub severity: ErrorSeverity,
    /// Timestamp when error occurred
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Request/session ID for correlation
    pub correlation_id: Option<String>,
}

/// Error severity levels for monitoring and alerting
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Low severity - information only
    Info,
    /// Medium severity - warning condition
    Warning,
    /// High severity - error condition
    Error,
    /// Critical severity - immediate attention required
    Critical,
}

/// Sanitized error representation for production logging (legacy compatibility)
#[derive(Debug, Clone, Serialize)]
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

    /// Create a database error
    pub fn database<S: Into<String>>(msg: S) -> Self {
        Self::Database(msg.into())
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

    /// Create a crypto error
    pub fn crypto<S: Into<String>>(msg: S) -> Self {
        Self::Crypto(msg.into())
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

    /// Create a rate limit error
    pub fn rate_limit_error<S: Into<String>>(msg: S) -> Self {
        Self::RateLimit(msg.into())
    }

    /// Create a network error
    pub fn network_error<S: Into<String>>(msg: S) -> Self {
        Self::Network(msg.into())
    }

    /// Create an external service error
    pub fn external_service_error<S: Into<String>>(msg: S) -> Self {
        Self::ExternalService(msg.into())
    }

    /// Create a parsing error
    pub fn parsing_error<S: Into<String>>(msg: S) -> Self {
        Self::Parsing(msg.into())
    }

    /// Create an internal error
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self::Generic(anyhow::anyhow!(msg.into()))
    }

    /// Create a configuration error (alias for backwards compatibility)
    pub fn configuration_error<S: Into<String>>(msg: S) -> Self {
        Self::Config(msg.into())
    }

    /// Create a validation error
    pub fn validation<S: Into<String>>(msg: S) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a serialization error
    pub fn serialization<S: Into<String>>(msg: S) -> Self {
        Self::Parsing(msg.into())
    }

    /// Map LoxoneError to structured error code
    pub fn to_error_code(&self) -> ErrorCode {
        match self {
            LoxoneError::Connection(_) => ErrorCode::ConnectionLost,
            LoxoneError::Authentication(_) => ErrorCode::InvalidCredentials,
            LoxoneError::Config(_) => ErrorCode::ConfigurationInvalid,
            LoxoneError::Credentials(_) => ErrorCode::InvalidCredentials,
            LoxoneError::DeviceControl(_) => ErrorCode::DeviceControlFailed,
            LoxoneError::SensorDiscovery(_) => ErrorCode::DeviceNotFound,
            LoxoneError::Discovery(_) => ErrorCode::NetworkUnreachable,
            LoxoneError::Timeout(_) => ErrorCode::ConnectionTimeout,
            LoxoneError::InvalidInput(_) => ErrorCode::InvalidInput,
            LoxoneError::NotFound(_) => ErrorCode::DeviceNotFound,
            LoxoneError::PermissionDenied(_) => ErrorCode::PermissionDenied,
            LoxoneError::ServiceUnavailable(_) => ErrorCode::ServiceUnavailable,
            LoxoneError::ResourceExhausted(_) => ErrorCode::ResourceExhausted,
            LoxoneError::ConsentDenied(_) => ErrorCode::ConsentRequired,
            LoxoneError::RateLimit(_) => ErrorCode::RateLimitExceeded,
            LoxoneError::Network(_) => ErrorCode::NetworkUnreachable,
            LoxoneError::ExternalService(_) => ErrorCode::ExternalServiceError,
            LoxoneError::Parsing(_) => ErrorCode::ParsingFailed,
            LoxoneError::Mcp(_) => ErrorCode::ProtocolViolation,
            LoxoneError::Json(_) => ErrorCode::ParsingFailed,
            LoxoneError::Http(_) => ErrorCode::ExternalServiceError,
            LoxoneError::Io(_) => ErrorCode::InternalError,
            LoxoneError::Generic(_) => ErrorCode::InternalError,
            #[cfg(feature = "websocket")]
            LoxoneError::WebSocket(_) => ErrorCode::ConnectionLost,
            LoxoneError::Crypto(_) => ErrorCode::InternalError,
            LoxoneError::Database(_) => ErrorCode::InternalError,
            #[cfg(target_arch = "wasm32")]
            LoxoneError::Wasm(_) => ErrorCode::InternalError,
        }
    }

    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            LoxoneError::Authentication(_) | LoxoneError::Credentials(_) => ErrorSeverity::Critical,
            LoxoneError::Config(_) | LoxoneError::PermissionDenied(_) => ErrorSeverity::Error,
            LoxoneError::Connection(_) | LoxoneError::ServiceUnavailable(_) => {
                ErrorSeverity::Warning
            }
            LoxoneError::Timeout(_) | LoxoneError::Network(_) => ErrorSeverity::Warning,
            LoxoneError::DeviceControl(_) | LoxoneError::NotFound(_) => ErrorSeverity::Error,
            LoxoneError::InvalidInput(_) | LoxoneError::Parsing(_) => ErrorSeverity::Warning,
            LoxoneError::ResourceExhausted(_) | LoxoneError::RateLimit(_) => ErrorSeverity::Error,
            _ => ErrorSeverity::Error,
        }
    }

    /// Generate recovery suggestions for the error
    pub fn generate_recovery_suggestions(&self) -> Vec<RecoverySuggestion> {
        match self {
            LoxoneError::Connection(_) => vec![
                RecoverySuggestion {
                    description: "Check network connectivity and Loxone Miniserver availability"
                        .to_string(),
                    automated: true,
                    action_code: Some("ping miniserver_ip".to_string()),
                    estimated_time_seconds: Some(5),
                    prerequisites: vec![],
                },
                RecoverySuggestion {
                    description: "Retry connection with exponential backoff".to_string(),
                    automated: true,
                    action_code: None,
                    estimated_time_seconds: Some(30),
                    prerequisites: vec![],
                },
            ],
            LoxoneError::Authentication(_) => vec![
                RecoverySuggestion {
                    description: "Verify credentials are correct and not expired".to_string(),
                    automated: false,
                    action_code: Some("loxone-mcp verify".to_string()),
                    estimated_time_seconds: Some(10),
                    prerequisites: vec!["Valid credentials".to_string()],
                },
                RecoverySuggestion {
                    description: "Re-run credential setup".to_string(),
                    automated: false,
                    action_code: Some("loxone-mcp setup".to_string()),
                    estimated_time_seconds: Some(60),
                    prerequisites: vec!["Access to Loxone Config".to_string()],
                },
            ],
            LoxoneError::Config(_) => vec![
                RecoverySuggestion {
                    description: "Check configuration file syntax and completeness".to_string(),
                    automated: true,
                    action_code: None,
                    estimated_time_seconds: Some(5),
                    prerequisites: vec![],
                },
                RecoverySuggestion {
                    description: "Reset to default configuration".to_string(),
                    automated: true,
                    action_code: Some("rm config.toml && loxone-mcp setup".to_string()),
                    estimated_time_seconds: Some(30),
                    prerequisites: vec![],
                },
            ],
            LoxoneError::DeviceControl(_) => vec![
                RecoverySuggestion {
                    description: "Check if device is online and accessible".to_string(),
                    automated: true,
                    action_code: None,
                    estimated_time_seconds: Some(10),
                    prerequisites: vec![],
                },
                RecoverySuggestion {
                    description: "Verify device UUID and permissions".to_string(),
                    automated: false,
                    action_code: None,
                    estimated_time_seconds: Some(15),
                    prerequisites: vec!["Loxone Config access".to_string()],
                },
            ],
            LoxoneError::Timeout(_) => vec![
                RecoverySuggestion {
                    description: "Increase timeout values in configuration".to_string(),
                    automated: false,
                    action_code: None,
                    estimated_time_seconds: Some(5),
                    prerequisites: vec![],
                },
                RecoverySuggestion {
                    description: "Check network latency to Miniserver".to_string(),
                    automated: true,
                    action_code: Some("ping -c 4 miniserver_ip".to_string()),
                    estimated_time_seconds: Some(10),
                    prerequisites: vec![],
                },
            ],
            _ => vec![RecoverySuggestion {
                description: "Check logs for more detailed error information".to_string(),
                automated: false,
                action_code: Some("journalctl -u loxone-mcp".to_string()),
                estimated_time_seconds: Some(10),
                prerequisites: vec![],
            }],
        }
    }

    /// Create a structured error from this LoxoneError
    pub fn to_structured_error(&self, context: Option<ErrorContext>) -> StructuredError {
        let error_code = self.to_error_code();
        let base_context =
            context.unwrap_or_else(|| ErrorContext::new(error_code.clone(), "unknown", "unknown"));

        StructuredError {
            code: error_code.clone(),
            code_number: error_code.as_number(),
            category: error_code.category(),
            message: self.sanitized_message(),
            #[cfg(debug_assertions)]
            debug_message: self.to_string(),
            is_retryable: self.is_retryable(),
            is_auth_error: self.is_auth_error(),
            component: base_context.component,
            operation: base_context.operation,
            metadata: base_context.metadata,
            recovery_suggestions: self.generate_recovery_suggestions(),
            severity: self.severity(),
            timestamp: base_context.timestamp,
            correlation_id: base_context.correlation_id,
        }
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
                LoxoneError::RateLimit(_) => "Rate limit exceeded".to_string(),
                LoxoneError::Network(_) => "Network operation failed".to_string(),
                LoxoneError::ExternalService(_) => "External service error".to_string(),
                LoxoneError::Parsing(_) => "Data parsing error".to_string(),
                LoxoneError::Io(_) => "I/O operation failed".to_string(),
                LoxoneError::Json(_) => "Data parsing error".to_string(),
                LoxoneError::Generic(_) => "Internal error occurred".to_string(),
                #[cfg(feature = "websocket")]
                LoxoneError::WebSocket(_) => "WebSocket connection error".to_string(),
                #[cfg(feature = "crypto-openssl")]
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

/// Error logging and reporting utilities
pub struct ErrorReporter;

impl ErrorReporter {
    /// Log a structured error with appropriate severity
    pub fn log_error(error: &LoxoneError, context: Option<ErrorContext>) {
        let structured = error.to_structured_error(context);

        match structured.severity {
            ErrorSeverity::Critical => {
                tracing::error!(
                    error_code = structured.code_number,
                    category = structured.category,
                    component = structured.component,
                    operation = structured.operation,
                    correlation_id = structured.correlation_id,
                    recovery_suggestions = ?structured.recovery_suggestions,
                    "Critical error occurred: {}",
                    structured.message
                );
            }
            ErrorSeverity::Error => {
                tracing::error!(
                    error_code = structured.code_number,
                    category = structured.category,
                    component = structured.component,
                    operation = structured.operation,
                    correlation_id = structured.correlation_id,
                    "Error occurred: {}",
                    structured.message
                );
            }
            ErrorSeverity::Warning => {
                tracing::warn!(
                    error_code = structured.code_number,
                    category = structured.category,
                    component = structured.component,
                    operation = structured.operation,
                    correlation_id = structured.correlation_id,
                    "Warning: {}",
                    structured.message
                );
            }
            ErrorSeverity::Info => {
                tracing::info!(
                    error_code = structured.code_number,
                    category = structured.category,
                    component = structured.component,
                    operation = structured.operation,
                    correlation_id = structured.correlation_id,
                    "Info: {}",
                    structured.message
                );
            }
        }
    }

    /// Create an error context with stack trace (debug builds only)
    pub fn create_context(code: ErrorCode, component: &str, operation: &str) -> ErrorContext {
        #[cfg(debug_assertions)]
        {
            ErrorContext::new(code, component, operation).with_stack_trace()
        }

        #[cfg(not(debug_assertions))]
        {
            ErrorContext::new(code, component, operation)
        }
    }

    /// Format error for API responses
    pub fn format_api_error(error: &LoxoneError, include_details: bool) -> serde_json::Value {
        let structured = error.to_structured_error(None);

        let mut response = serde_json::json!({
            "error": {
                "code": structured.code_number,
                "category": structured.category,
                "message": structured.message,
                "retryable": structured.is_retryable,
                "timestamp": structured.timestamp
            }
        });

        if include_details {
            response["error"]["recovery_suggestions"] =
                serde_json::to_value(structured.recovery_suggestions).unwrap_or_default();
            response["error"]["component"] = serde_json::Value::String(structured.component);
            response["error"]["operation"] = serde_json::Value::String(structured.operation);

            if let Some(correlation_id) = structured.correlation_id {
                response["error"]["correlation_id"] = serde_json::Value::String(correlation_id);
            }
        }

        response
    }

    /// Generate error metrics for monitoring
    pub fn generate_metrics(error: &LoxoneError) -> HashMap<String, serde_json::Value> {
        let structured = error.to_structured_error(None);

        HashMap::from([
            (
                "error_code".to_string(),
                serde_json::Value::Number(structured.code_number.into()),
            ),
            (
                "category".to_string(),
                serde_json::Value::String(structured.category.to_string()),
            ),
            (
                "severity".to_string(),
                serde_json::Value::String(format!("{:?}", structured.severity)),
            ),
            (
                "retryable".to_string(),
                serde_json::Value::Bool(structured.is_retryable),
            ),
            (
                "auth_error".to_string(),
                serde_json::Value::Bool(structured.is_auth_error),
            ),
            (
                "component".to_string(),
                serde_json::Value::String(structured.component),
            ),
            (
                "timestamp".to_string(),
                serde_json::Value::String(structured.timestamp.to_rfc3339()),
            ),
        ])
    }
}

/// Macro for easy structured error logging
#[macro_export]
macro_rules! log_structured_error {
    ($error:expr, $component:expr, $operation:expr) => {
        $crate::error::ErrorReporter::log_error(
            &$error,
            Some($crate::error::ErrorReporter::create_context(
                $error.to_error_code(),
                $component,
                $operation,
            )),
        )
    };
    ($error:expr, $component:expr, $operation:expr, $correlation_id:expr) => {
        $crate::error::ErrorReporter::log_error(
            &$error,
            Some(
                $crate::error::ErrorReporter::create_context(
                    $error.to_error_code(),
                    $component,
                    $operation,
                )
                .with_correlation_id($correlation_id),
            ),
        )
    };
}

#[cfg(feature = "websocket")]
impl From<tokio_tungstenite::tungstenite::Error> for LoxoneError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        LoxoneError::WebSocket(err.to_string())
    }
}

// Legacy error handling removed:
// - rsa::Error: RSA crate disabled due to RUSTSEC-2023-0071 vulnerability
// - keyring::Error: Keyring crate disabled due to unmaintained dependencies

impl From<regex::Error> for LoxoneError {
    fn from(err: regex::Error) -> Self {
        LoxoneError::InvalidInput(format!("Regex pattern error: {err}"))
    }
}

// Implement ErrorClassification trait for LoxoneError to work with mcp-logging
impl pulseengine_mcp_logging::ErrorClassification for LoxoneError {
    fn error_type(&self) -> &str {
        match self {
            LoxoneError::Connection(_) => "connection_error",
            LoxoneError::Authentication(_) => "authentication_error",
            LoxoneError::Http(_) => "http_error",
            LoxoneError::Json(_) => "json_error",
            LoxoneError::Config(_) => "config_error",
            LoxoneError::Credentials(_) => "credentials_error",
            LoxoneError::Crypto(_) => "crypto_error",
            LoxoneError::DeviceControl(_) => "device_control_error",
            LoxoneError::SensorDiscovery(_) => "sensor_discovery_error",
            LoxoneError::Discovery(_) => "discovery_error",
            #[cfg(feature = "websocket")]
            LoxoneError::WebSocket(_) => "websocket_error",
            LoxoneError::Mcp(_) => "mcp_protocol_error",
            #[cfg(target_arch = "wasm32")]
            LoxoneError::Wasm(_) => "wasm_error",
            LoxoneError::Io(_) => "io_error",
            LoxoneError::Generic(_) => "generic_error",
            LoxoneError::Timeout(_) => "timeout_error",
            LoxoneError::InvalidInput(_) => "invalid_input_error",
            LoxoneError::NotFound(_) => "not_found_error",
            LoxoneError::PermissionDenied(_) => "permission_denied_error",
            LoxoneError::ServiceUnavailable(_) => "service_unavailable_error",
            LoxoneError::ResourceExhausted(_) => "resource_exhausted_error",
            LoxoneError::ConsentDenied(_) => "consent_denied_error",
            LoxoneError::RateLimit(_) => "rate_limit_error",
            LoxoneError::Network(_) => "network_error",
            LoxoneError::ExternalService(_) => "external_service_error",
            LoxoneError::Parsing(_) => "parsing_error",
            LoxoneError::Database(_) => "database_error",
        }
    }

    fn is_retryable(&self) -> bool {
        self.is_retryable()
    }

    fn is_timeout(&self) -> bool {
        matches!(self, LoxoneError::Timeout(_))
    }

    fn is_auth_error(&self) -> bool {
        self.is_auth_error()
    }

    fn is_connection_error(&self) -> bool {
        matches!(
            self,
            LoxoneError::Connection(_) | LoxoneError::Network(_) | LoxoneError::Http(_)
        )
    }
}
