//! Enhanced structured logging for better observability
//!
//! This module provides comprehensive structured logging with:
//! - Contextual field enrichment
//! - Performance metrics
//! - Error classification
//! - Business logic tracing
//! - System health indicators

use crate::sanitization::get_sanitizer;
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Structured logging context with rich observability fields
#[derive(Debug, Clone)]
pub struct StructuredContext {
    /// Request tracking
    pub request_id: String,
    pub parent_request_id: Option<String>,
    pub correlation_id: String,

    /// Service identification
    pub service_name: String,
    pub service_version: String,
    pub instance_id: String,

    /// Request metadata
    pub tool_name: String,
    pub tool_version: Option<String>,
    pub client_id: Option<String>,
    pub user_agent: Option<String>,
    pub session_id: Option<String>,

    /// Performance metrics
    pub start_time: Instant,
    pub start_timestamp: u64,

    /// Loxone-specific context
    pub loxone_host: Option<String>,
    pub loxone_version: Option<String>,
    pub room_name: Option<String>,
    pub device_uuid: Option<String>,
    pub device_type: Option<String>,

    /// Custom fields for business logic
    pub custom_fields: HashMap<String, Value>,
}

impl StructuredContext {
    /// Create a new structured context
    pub fn new(tool_name: String) -> Self {
        let now = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            request_id: generate_request_id(),
            parent_request_id: None,
            correlation_id: generate_correlation_id(),
            service_name: "loxone-mcp-server".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            instance_id: generate_instance_id(),
            tool_name,
            tool_version: None,
            client_id: None,
            user_agent: None,
            session_id: None,
            start_time: now,
            start_timestamp: timestamp,
            loxone_host: None,
            loxone_version: None,
            room_name: None,
            device_uuid: None,
            device_type: None,
            custom_fields: HashMap::new(),
        }
    }

    /// Create a child context for sub-operations
    pub fn child(&self, operation: &str) -> Self {
        let mut child = Self::new(format!("{}::{}", self.tool_name, operation));
        child.parent_request_id = Some(self.request_id.clone());
        child.correlation_id = self.correlation_id.clone();
        child.client_id = self.client_id.clone();
        child.user_agent = self.user_agent.clone();
        child.session_id = self.session_id.clone();
        child.loxone_host = self.loxone_host.clone();
        child.loxone_version = self.loxone_version.clone();
        child
    }

    /// Add Loxone-specific context
    pub fn with_loxone_context(mut self, host: String, version: Option<String>) -> Self {
        self.loxone_host = Some(host);
        self.loxone_version = version;
        self
    }

    /// Add device context
    pub fn with_device_context(
        mut self,
        device_uuid: String,
        device_type: Option<String>,
        room_name: Option<String>,
    ) -> Self {
        self.device_uuid = Some(device_uuid);
        self.device_type = device_type;
        self.room_name = room_name;
        self
    }

    /// Add client context
    pub fn with_client_context(
        mut self,
        client_id: String,
        user_agent: Option<String>,
        session_id: Option<String>,
    ) -> Self {
        self.client_id = Some(client_id);
        self.user_agent = user_agent;
        self.session_id = session_id;
        self
    }

    /// Add custom field
    pub fn with_field<K: ToString, V: Into<Value>>(mut self, key: K, value: V) -> Self {
        self.custom_fields.insert(key.to_string(), value.into());
        self
    }

    /// Get elapsed time since context creation
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed().as_millis() as u64
    }
}

/// Error classification for better observability
#[derive(Debug, Clone)]
pub enum ErrorClass {
    /// Client errors (4xx equivalent)
    Client { error_type: String, retryable: bool },
    /// Server errors (5xx equivalent)
    Server { error_type: String, retryable: bool },
    /// Network/communication errors
    Network { error_type: String, timeout: bool },
    /// Authentication/authorization errors
    Auth { error_type: String },
    /// Business logic errors
    Business { error_type: String, domain: String },
}

impl ErrorClass {
    /// Classify an error using the ErrorClassification trait
    pub fn from_error<E: crate::ErrorClassification>(error: &E) -> Self {
        if error.is_auth_error() {
            Self::Auth {
                error_type: error.error_type().to_string(),
            }
        } else if error.is_connection_error() {
            Self::Network {
                error_type: error.error_type().to_string(),
                timeout: error.is_timeout(),
            }
        } else if error.is_timeout() {
            Self::Network {
                error_type: error.error_type().to_string(),
                timeout: true,
            }
        } else if error.is_retryable() {
            Self::Server {
                error_type: error.error_type().to_string(),
                retryable: true,
            }
        } else {
            Self::Client {
                error_type: error.error_type().to_string(),
                retryable: false,
            }
        }
    }
}

/// Enhanced structured logger
pub struct StructuredLogger;

impl StructuredLogger {
    /// Log request start with comprehensive context
    pub fn log_request_start(ctx: &StructuredContext, params: &Value) {
        let sanitized_params = sanitize_value(params);

        info!(
            // Core request tracking
            request_id = %ctx.request_id,
            parent_request_id = ?ctx.parent_request_id,
            correlation_id = %ctx.correlation_id,

            // Service identification
            service_name = %ctx.service_name,
            service_version = %ctx.service_version,
            instance_id = %ctx.instance_id,

            // Request metadata
            tool_name = %ctx.tool_name,
            tool_version = ?ctx.tool_version,
            client_id = ?ctx.client_id,
            user_agent = ?ctx.user_agent,
            session_id = ?ctx.session_id,

            // Timing
            start_timestamp = ctx.start_timestamp,

            // Loxone context
            loxone_host = ?ctx.loxone_host,
            loxone_version = ?ctx.loxone_version,
            room_name = ?ctx.room_name,
            device_uuid = ?ctx.device_uuid,
            device_type = ?ctx.device_type,

            // Request data
            params = ?sanitized_params,

            // Custom fields
            custom_fields = ?ctx.custom_fields,

            "MCP request started"
        );
    }

    /// Log request completion with performance metrics
    pub fn log_request_end<E: crate::ErrorClassification>(
        ctx: &StructuredContext,
        success: bool,
        error: Option<&E>,
        response_size: Option<usize>,
    ) {
        let duration_ms = ctx.elapsed_ms();
        let end_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let error_class = error.map(ErrorClass::from_error);

        if success {
            info!(
                // Core tracking
                request_id = %ctx.request_id,
                correlation_id = %ctx.correlation_id,
                tool_name = %ctx.tool_name,

                // Performance metrics
                duration_ms = duration_ms,
                start_timestamp = ctx.start_timestamp,
                end_timestamp = end_timestamp,
                response_size_bytes = ?response_size,

                // Context
                loxone_host = ?ctx.loxone_host,
                room_name = ?ctx.room_name,
                device_uuid = ?ctx.device_uuid,

                "MCP request completed successfully"
            );
        } else {
            error!(
                // Core tracking
                request_id = %ctx.request_id,
                correlation_id = %ctx.correlation_id,
                tool_name = %ctx.tool_name,

                // Performance metrics
                duration_ms = duration_ms,
                start_timestamp = ctx.start_timestamp,
                end_timestamp = end_timestamp,

                // Error classification (sanitized)
                error_class = ?error_class,
                error_message = ?error.map(|e| get_sanitizer().sanitize_error(e)),

                // Context
                loxone_host = ?ctx.loxone_host,
                room_name = ?ctx.room_name,
                device_uuid = ?ctx.device_uuid,

                "MCP request failed"
            );
        }
    }

    /// Log performance warnings
    pub fn log_slow_request(ctx: &StructuredContext, threshold_ms: u64) {
        let duration_ms = ctx.elapsed_ms();
        if duration_ms > threshold_ms {
            warn!(
                request_id = %ctx.request_id,
                correlation_id = %ctx.correlation_id,
                tool_name = %ctx.tool_name,
                duration_ms = duration_ms,
                threshold_ms = threshold_ms,

                // Performance context
                loxone_host = ?ctx.loxone_host,
                device_uuid = ?ctx.device_uuid,
                room_name = ?ctx.room_name,

                "Slow MCP request detected"
            );
        }
    }

    /// Log Loxone API calls with timing
    pub fn log_loxone_api_call(
        ctx: &StructuredContext,
        method: &str,
        endpoint: &str,
        duration_ms: u64,
        status_code: Option<u16>,
        error: Option<&str>,
    ) {
        if let Some(err) = error {
            warn!(
                request_id = %ctx.request_id,
                correlation_id = %ctx.correlation_id,

                // API call details
                api_method = method,
                api_endpoint = endpoint,
                api_duration_ms = duration_ms,
                api_status_code = ?status_code,
                api_error = err,

                // Context
                loxone_host = ?ctx.loxone_host,

                "Loxone API call failed"
            );
        } else {
            info!(
                request_id = %ctx.request_id,
                correlation_id = %ctx.correlation_id,

                // API call details
                api_method = method,
                api_endpoint = endpoint,
                api_duration_ms = duration_ms,
                api_status_code = ?status_code,

                // Context
                loxone_host = ?ctx.loxone_host,

                "Loxone API call completed"
            );
        }
    }

    /// Log device control operations
    pub fn log_device_operation(
        ctx: &StructuredContext,
        operation: &str,
        device_uuid: &str,
        device_name: Option<&str>,
        room_name: Option<&str>,
        success: bool,
        error: Option<&str>,
    ) {
        if success {
            info!(
                request_id = %ctx.request_id,
                correlation_id = %ctx.correlation_id,

                // Device operation details
                device_operation = operation,
                device_uuid = device_uuid,
                device_name = ?device_name,
                device_room = ?room_name,

                // Context
                loxone_host = ?ctx.loxone_host,

                "Device operation completed successfully"
            );
        } else {
            error!(
                request_id = %ctx.request_id,
                correlation_id = %ctx.correlation_id,

                // Device operation details
                device_operation = operation,
                device_uuid = device_uuid,
                device_name = ?device_name,
                device_room = ?room_name,
                device_error = ?error,

                // Context
                loxone_host = ?ctx.loxone_host,

                "Device operation failed"
            );
        }
    }

    /// Log system health metrics
    pub fn log_health_metrics(
        connection_status: bool,
        api_latency_ms: Option<u64>,
        active_requests: usize,
        error_rate: f64,
        memory_usage_mb: Option<f64>,
    ) {
        info!(
            // Health indicators
            system_healthy = connection_status,
            api_latency_ms = ?api_latency_ms,
            active_requests = active_requests,
            error_rate_percent = error_rate * 100.0,
            memory_usage_mb = ?memory_usage_mb,

            // Service identification
            service_name = "loxone-mcp-server",
            service_version = env!("CARGO_PKG_VERSION"),

            "System health metrics"
        );
    }

    /// Create a structured tracing span
    pub fn create_span(ctx: &StructuredContext) -> tracing::Span {
        tracing::info_span!(
            "mcp_request",
            request_id = %ctx.request_id,
            correlation_id = %ctx.correlation_id,
            tool_name = %ctx.tool_name,
            client_id = ?ctx.client_id,
            loxone_host = ?ctx.loxone_host,
            device_uuid = ?ctx.device_uuid,
            room_name = ?ctx.room_name
        )
    }
}

/// Sanitize values for logging (remove sensitive data)
fn sanitize_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sanitized = serde_json::Map::new();
            for (key, val) in map {
                if is_sensitive_field(key) {
                    sanitized.insert(key.clone(), Value::String("***".to_string()));
                } else {
                    sanitized.insert(key.clone(), sanitize_value(val));
                }
            }
            Value::Object(sanitized)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(sanitize_value).collect()),
        _ => value.clone(),
    }
}

/// Check if a field contains sensitive data
fn is_sensitive_field(field: &str) -> bool {
    let field_lower = field.to_lowercase();
    field_lower.contains("password")
        || field_lower.contains("secret")
        || field_lower.contains("token")
        || field_lower.contains("api_key")
        || field_lower.contains("apikey")
        || field_lower.contains("auth")
        || field_lower.contains("credential")
        || field_lower.contains("private_key")
        || field_lower.contains("session")
}

/// Generate a unique request ID
fn generate_request_id() -> String {
    let uuid = Uuid::new_v4();
    let bytes = uuid.as_bytes();
    hex::encode(&bytes[..8]) // 16 character hex string
}

/// Generate a correlation ID for request tracing
fn generate_correlation_id() -> String {
    let uuid = Uuid::new_v4();
    let bytes = uuid.as_bytes();
    hex::encode(&bytes[..12]) // 24 character hex string
}

/// Generate a service instance ID
fn generate_instance_id() -> String {
    use std::sync::OnceLock;
    static INSTANCE_ID: OnceLock<String> = OnceLock::new();

    INSTANCE_ID
        .get_or_init(|| {
            let uuid = Uuid::new_v4();
            let bytes = uuid.as_bytes();
            hex::encode(&bytes[..6]) // 12 character hex string
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structured_context_creation() {
        let ctx = StructuredContext::new("test_tool".to_string());
        assert_eq!(ctx.tool_name, "test_tool");
        assert_eq!(ctx.service_name, "loxone-mcp-server");
        assert!(ctx.request_id.len() == 16);
        assert!(ctx.correlation_id.len() == 24);
    }

    #[test]
    fn test_child_context() {
        let parent = StructuredContext::new("parent_tool".to_string());
        let child = parent.child("sub_operation");

        assert_eq!(child.tool_name, "parent_tool::sub_operation");
        assert_eq!(child.correlation_id, parent.correlation_id);
        assert_eq!(child.parent_request_id, Some(parent.request_id));
    }

    #[test]
    fn test_context_enrichment() {
        let ctx = StructuredContext::new("test_tool".to_string())
            .with_loxone_context("192.168.1.100".to_string(), Some("12.0.0".to_string()))
            .with_device_context(
                "device-123".to_string(),
                Some("Switch".to_string()),
                Some("Living Room".to_string()),
            )
            .with_field("custom_field", "custom_value");

        assert_eq!(ctx.loxone_host, Some("192.168.1.100".to_string()));
        assert_eq!(ctx.device_uuid, Some("device-123".to_string()));
        assert_eq!(
            ctx.custom_fields.get("custom_field"),
            Some(&Value::String("custom_value".to_string()))
        );
    }

    #[test]
    fn test_error_classification() {
        // Create a mock error implementing ErrorClassification
        #[derive(Debug)]
        struct MockError;

        impl std::fmt::Display for MockError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "Mock authentication error")
            }
        }

        impl std::error::Error for MockError {}

        impl crate::ErrorClassification for MockError {
            fn error_type(&self) -> &str {
                "auth_error"
            }
            fn is_retryable(&self) -> bool {
                false
            }
            fn is_timeout(&self) -> bool {
                false
            }
            fn is_auth_error(&self) -> bool {
                true
            }
            fn is_connection_error(&self) -> bool {
                false
            }
        }

        let mock_error = MockError;
        let error_class = ErrorClass::from_error(&mock_error);

        matches!(error_class, ErrorClass::Auth { .. });
    }

    #[test]
    fn test_sanitize_sensitive_data() {
        let data = serde_json::json!({
            "username": "test_user",
            "password": "secret123",
            "device_id": "dev123"
        });

        let sanitized = sanitize_value(&data);
        assert_eq!(sanitized["username"], "test_user");
        assert_eq!(sanitized["password"], "***");
        assert_eq!(sanitized["device_id"], "dev123");
    }
}
