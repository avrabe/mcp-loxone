//! Request/Response validation middleware
//!
//! This module provides comprehensive validation middleware for MCP requests,
//! including schema validation, input sanitization, and security checks.

pub mod middleware;
pub mod rules;
pub mod sanitizer;
pub mod schema;

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

/// Validation result with detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether validation passed
    pub is_valid: bool,
    /// List of validation errors
    pub errors: Vec<ValidationError>,
    /// List of warnings (non-blocking issues)
    pub warnings: Vec<ValidationWarning>,
    /// Sanitized/normalized data (if applicable)
    pub sanitized_data: Option<serde_json::Value>,
    /// Validation metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ValidationResult {
    /// Create a successful validation result
    pub fn success() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            sanitized_data: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a failed validation result
    pub fn failure(errors: Vec<ValidationError>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
            sanitized_data: None,
            metadata: HashMap::new(),
        }
    }

    /// Add a warning to the result
    pub fn with_warning(mut self, warning: ValidationWarning) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Add sanitized data to the result
    pub fn with_sanitized_data(mut self, data: serde_json::Value) -> Self {
        self.sanitized_data = Some(data);
        self
    }

    /// Add metadata to the result
    pub fn with_metadata<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<serde_json::Value>,
    {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Convert to LoxoneError if validation failed
    pub fn to_error(&self) -> Option<LoxoneError> {
        if !self.is_valid && !self.errors.is_empty() {
            let error_messages: Vec<String> = self
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.field, e.message))
                .collect();

            Some(LoxoneError::invalid_input(format!(
                "Validation failed: {}",
                error_messages.join(", ")
            )))
        } else {
            None
        }
    }
}

/// Validation error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Field that failed validation
    pub field: String,
    /// Error message
    pub message: String,
    /// Error code for machine processing
    pub code: ValidationErrorCode,
    /// Expected value or format
    pub expected: Option<String>,
    /// Actual value that was provided
    pub actual: Option<String>,
    /// Suggested fix
    pub suggestion: Option<String>,
}

/// Validation warning (non-blocking issue)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// Field with warning
    pub field: String,
    /// Warning message
    pub message: String,
    /// Warning code
    pub code: ValidationWarningCode,
    /// Recommended action
    pub recommendation: Option<String>,
}

/// Validation error codes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationErrorCode {
    /// Required field is missing
    MissingRequired,
    /// Invalid data type
    InvalidType,
    /// Value out of valid range
    OutOfRange,
    /// Invalid format (e.g., invalid UUID, email)
    InvalidFormat,
    /// Value too long
    TooLong,
    /// Value too short
    TooShort,
    /// Invalid enum value
    InvalidEnum,
    /// Failed regex pattern
    PatternMismatch,
    /// Security policy violation
    SecurityViolation,
    /// Malicious content detected
    MaliciousContent,
    /// Rate limit exceeded
    RateLimit,
    /// Schema validation failed
    SchemaViolation,
}

/// Validation warning codes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationWarningCode {
    /// Deprecated field usage
    DeprecatedField,
    /// Suboptimal value
    SuboptimalValue,
    /// Potential security concern
    SecurityConcern,
    /// Performance impact
    PerformanceImpact,
    /// Compatibility issue
    CompatibilityIssue,
}

/// Validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Enable strict validation mode
    pub strict_mode: bool,
    /// Maximum request size in bytes
    pub max_request_size: usize,
    /// Maximum string length
    pub max_string_length: usize,
    /// Maximum array size
    pub max_array_size: usize,
    /// Maximum object depth
    pub max_object_depth: usize,
    /// Enable content sanitization
    pub enable_sanitization: bool,
    /// Enable security scanning
    pub enable_security_scan: bool,
    /// Custom validation rules
    pub custom_rules: HashMap<String, serde_json::Value>,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            strict_mode: false,
            max_request_size: 1024 * 1024, // 1MB
            max_string_length: 10000,
            max_array_size: 1000,
            max_object_depth: 10,
            enable_sanitization: true,
            enable_security_scan: true,
            custom_rules: HashMap::new(),
        }
    }
}

/// Validation context for request processing
#[derive(Debug, Clone)]
pub struct ValidationContext {
    /// Request ID for correlation
    pub request_id: String,
    /// Client information
    pub client_info: Option<ClientInfo>,
    /// Request timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Validation configuration
    pub config: Arc<ValidationConfig>,
    /// Additional context data
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ValidationContext {
    /// Create new validation context
    pub fn new(request_id: String, config: Arc<ValidationConfig>) -> Self {
        Self {
            request_id,
            client_info: None,
            timestamp: chrono::Utc::now(),
            config,
            metadata: HashMap::new(),
        }
    }

    /// Add client information
    pub fn with_client_info(mut self, client_info: ClientInfo) -> Self {
        self.client_info = Some(client_info);
        self
    }

    /// Add metadata
    pub fn with_metadata<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<serde_json::Value>,
    {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Client information for validation context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Client IP address
    pub ip_address: Option<String>,
    /// User agent string
    pub user_agent: Option<String>,
    /// Client identifier
    pub client_id: Option<String>,
    /// Authentication level
    pub auth_level: AuthLevel,
    /// Rate limiting information
    pub rate_limit_info: Option<RateLimitInfo>,
}

/// Authentication level for validation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthLevel {
    /// No authentication
    None,
    /// Basic authentication
    Basic,
    /// Full authentication with permissions
    Authenticated,
    /// Administrative access
    Admin,
}

/// Rate limiting information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitInfo {
    /// Requests made in current window
    pub current_requests: u32,
    /// Maximum requests allowed
    pub max_requests: u32,
    /// Window duration in seconds
    pub window_seconds: u32,
    /// Time until window resets
    pub reset_time: chrono::DateTime<chrono::Utc>,
}

/// Main validator trait
#[async_trait::async_trait]
pub trait Validator: Send + Sync {
    /// Validate a request
    async fn validate_request(
        &self,
        data: &serde_json::Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult>;

    /// Validate a response
    async fn validate_response(
        &self,
        data: &serde_json::Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult>;

    /// Get validator name for logging
    fn name(&self) -> &'static str;
}

/// Composite validator that runs multiple validators
pub struct CompositeValidator {
    validators: Vec<Box<dyn Validator>>,
    #[allow(dead_code)]
    config: Arc<ValidationConfig>,
}

impl CompositeValidator {
    /// Create new composite validator
    pub fn new(config: Arc<ValidationConfig>) -> Self {
        Self {
            validators: Vec::new(),
            config,
        }
    }

    /// Add a validator to the chain
    pub fn add_validator(mut self, validator: Box<dyn Validator>) -> Self {
        self.validators.push(validator);
        self
    }

    /// Run all validators and combine results
    async fn run_validators<'a>(
        &'a self,
        data: &'a serde_json::Value,
        context: &'a ValidationContext,
        validate_fn: impl for<'b> Fn(
            &'b dyn Validator,
            &'b serde_json::Value,
            &'b ValidationContext,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ValidationResult>> + Send + 'b>,
        >,
    ) -> Result<ValidationResult> {
        let mut combined_result = ValidationResult::success();
        let mut has_errors = false;

        for validator in &self.validators {
            debug!("Running validator: {}", validator.name());

            match validate_fn(validator.as_ref(), data, context).await {
                Ok(result) => {
                    // Combine errors
                    combined_result.errors.extend(result.errors);
                    combined_result.warnings.extend(result.warnings);

                    // Track if we have any errors
                    if !result.is_valid {
                        has_errors = true;
                    }

                    // Use sanitized data from the last validator that provides it
                    if result.sanitized_data.is_some() {
                        combined_result.sanitized_data = result.sanitized_data;
                    }

                    // Combine metadata
                    combined_result.metadata.extend(result.metadata);
                }
                Err(e) => {
                    warn!("Validator {} failed: {}", validator.name(), e);
                    has_errors = true;
                    combined_result.errors.push(ValidationError {
                        field: "validator".to_string(),
                        message: format!("Validator {} failed: {}", validator.name(), e),
                        code: ValidationErrorCode::SchemaViolation,
                        expected: None,
                        actual: None,
                        suggestion: Some("Check validator configuration".to_string()),
                    });
                }
            }
        }

        combined_result.is_valid = !has_errors;
        Ok(combined_result)
    }
}

#[async_trait::async_trait]
impl Validator for CompositeValidator {
    async fn validate_request(
        &self,
        data: &serde_json::Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult> {
        self.run_validators(data, context, |validator, data, context| {
            Box::pin(validator.validate_request(data, context))
        })
        .await
    }

    async fn validate_response(
        &self,
        data: &serde_json::Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult> {
        self.run_validators(data, context, |validator, data, context| {
            Box::pin(validator.validate_response(data, context))
        })
        .await
    }

    fn name(&self) -> &'static str {
        "CompositeValidator"
    }
}

/// Utility functions for common validations
pub mod utils {

    use regex::Regex;
    use std::sync::OnceLock;

    /// Check if a string is a valid UUID
    pub fn is_valid_uuid(value: &str) -> bool {
        uuid::Uuid::parse_str(value).is_ok()
    }

    /// Check if a string is a valid email
    pub fn is_valid_email(value: &str) -> bool {
        static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();
        let regex = EMAIL_REGEX.get_or_init(|| {
            Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap()
        });
        regex.is_match(value)
    }

    /// Check if a string is a valid IP address
    pub fn is_valid_ip(value: &str) -> bool {
        value.parse::<std::net::IpAddr>().is_ok()
    }

    /// Check if a string contains potentially malicious content
    pub fn contains_malicious_content(value: &str) -> bool {
        static MALICIOUS_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
        let patterns = MALICIOUS_PATTERNS.get_or_init(|| {
            vec![
                Regex::new(r"<script[^>]*>").unwrap(),
                Regex::new(r"javascript:").unwrap(),
                Regex::new(r"on\w+\s*=").unwrap(),
                Regex::new(r"eval\s*\(").unwrap(),
                Regex::new(r"exec\s*\(").unwrap(),
                Regex::new(r"\$\(").unwrap(), // jQuery-like injection
                regex::RegexBuilder::new(r"union\s+select")
                    .case_insensitive(true)
                    .build()
                    .unwrap(),
                regex::RegexBuilder::new(r"drop\s+table")
                    .case_insensitive(true)
                    .build()
                    .unwrap(),
            ]
        });

        patterns.iter().any(|pattern| pattern.is_match(value))
    }

    /// Sanitize string by removing potentially dangerous characters
    pub fn sanitize_string(value: &str, max_length: usize) -> String {
        let mut sanitized = value
            .chars()
            .filter(|c| {
                // Allow alphanumeric, space, and common punctuation
                c.is_alphanumeric() || " .,!?-_()[]{}:;@#$%^&*+=|\\~`".contains(*c)
            })
            .take(max_length)
            .collect::<String>();

        // Remove leading/trailing whitespace
        sanitized = sanitized.trim().to_string();

        // Replace multiple consecutive spaces with single space
        static MULTI_SPACE_REGEX: OnceLock<Regex> = OnceLock::new();
        let regex = MULTI_SPACE_REGEX.get_or_init(|| Regex::new(r"\s+").unwrap());
        regex.replace_all(&sanitized, " ").to_string()
    }

    /// Check if value is within numeric range
    pub fn is_in_range<T>(value: T, min: T, max: T) -> bool
    where
        T: PartialOrd,
    {
        value >= min && value <= max
    }

    /// Validate object depth to prevent stack overflow
    pub fn validate_object_depth(value: &serde_json::Value, max_depth: usize) -> bool {
        fn check_depth(value: &serde_json::Value, current_depth: usize, max_depth: usize) -> bool {
            if current_depth > max_depth {
                return false;
            }

            match value {
                serde_json::Value::Object(obj) => {
                    for v in obj.values() {
                        if !check_depth(v, current_depth + 1, max_depth) {
                            return false;
                        }
                    }
                }
                serde_json::Value::Array(arr) => {
                    for v in arr {
                        if !check_depth(v, current_depth + 1, max_depth) {
                            return false;
                        }
                    }
                }
                _ => {}
            }

            true
        }

        check_depth(value, 0, max_depth)
    }
}

#[cfg(test)]
mod tests {
    use super::utils::*;
    use super::*;

    #[test]
    fn test_uuid_validation() {
        assert!(is_valid_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!is_valid_uuid("invalid-uuid"));
        assert!(!is_valid_uuid(""));
    }

    #[test]
    fn test_email_validation() {
        assert!(is_valid_email("test@example.com"));
        assert!(is_valid_email("user.name@domain.co.uk"));
        assert!(!is_valid_email("invalid-email"));
        assert!(!is_valid_email("@domain.com"));
        assert!(!is_valid_email("user@"));
    }

    #[test]
    fn test_malicious_content_detection() {
        assert!(contains_malicious_content("<script>alert('xss')</script>"));
        assert!(contains_malicious_content("javascript:void(0)"));
        assert!(contains_malicious_content("onclick=alert()"));
        assert!(contains_malicious_content("UNION SELECT * FROM users"));
        assert!(!contains_malicious_content("normal text content"));
    }

    #[test]
    fn test_string_sanitization() {
        assert_eq!(sanitize_string("Hello, World!", 20), "Hello, World!");
        assert_eq!(sanitize_string("  spaced  text  ", 20), "spaced text");
        assert_eq!(sanitize_string("toolongtext", 5), "toolo");
        assert_eq!(sanitize_string("text<script>", 20), "textscript");
    }

    #[test]
    fn test_object_depth_validation() {
        let shallow = serde_json::json!({"a": 1, "b": 2});
        assert!(validate_object_depth(&shallow, 5));

        let deep = serde_json::json!({
            "level1": {
                "level2": {
                    "level3": {
                        "level4": {
                            "level5": "too deep"
                        }
                    }
                }
            }
        });
        assert!(!validate_object_depth(&deep, 3));
        assert!(validate_object_depth(&deep, 5));
    }

    #[tokio::test]
    async fn test_validation_result() {
        let mut result = ValidationResult::success();
        assert!(result.is_valid);
        assert!(result.errors.is_empty());

        result = result.with_warning(ValidationWarning {
            field: "test_field".to_string(),
            message: "Test warning".to_string(),
            code: ValidationWarningCode::DeprecatedField,
            recommendation: None,
        });
        assert_eq!(result.warnings.len(), 1);

        let error_result = ValidationResult::failure(vec![ValidationError {
            field: "test".to_string(),
            message: "Test error".to_string(),
            code: ValidationErrorCode::MissingRequired,
            expected: None,
            actual: None,
            suggestion: None,
        }]);
        assert!(!error_result.is_valid);
        assert!(!error_result.errors.is_empty());
    }
}
