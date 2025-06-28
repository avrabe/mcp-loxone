//! Business logic validation rules

use super::{
    AuthLevel, ValidationContext, ValidationError, ValidationErrorCode, ValidationResult, Validator,
};
use crate::error::Result;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Business rules validator
pub struct RulesValidator {
    rules: Vec<Box<dyn ValidationRule>>,
}

impl Default for RulesValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl RulesValidator {
    /// Create new rules validator
    pub fn new() -> Self {
        Self { rules: Vec::new() }
            // Add default rules
            .add_rule(Box::new(AuthorizationRule))
            .add_rule(Box::new(RateLimitRule))
            .add_rule(Box::new(ResourceAccessRule))
            .add_rule(Box::new(LoxoneSpecificRule))
            .add_rule(Box::new(SecurityPolicyRule))
    }

    /// Add a custom validation rule
    pub fn add_rule(mut self, rule: Box<dyn ValidationRule>) -> Self {
        self.rules.push(rule);
        self
    }

    /// Run all rules against the data
    async fn run_rules(
        &self,
        data: &Value,
        context: &ValidationContext,
        is_request: bool,
    ) -> Result<ValidationResult> {
        let mut combined_errors = Vec::new();
        let mut combined_metadata = HashMap::new();

        for rule in &self.rules {
            debug!("Running validation rule: {}", rule.name());

            match if is_request {
                rule.validate_request(data, context).await
            } else {
                rule.validate_response(data, context).await
            } {
                Ok(result) => {
                    combined_errors.extend(result.errors);
                    combined_metadata.extend(result.metadata);
                }
                Err(e) => {
                    warn!("Validation rule {} failed: {}", rule.name(), e);
                    combined_errors.push(ValidationError {
                        field: "rule_execution".to_string(),
                        message: format!("Rule {} failed: {}", rule.name(), e),
                        code: ValidationErrorCode::SchemaViolation,
                        expected: None,
                        actual: None,
                        suggestion: Some("Check rule configuration".to_string()),
                    });
                }
            }
        }

        let is_valid = combined_errors.is_empty();
        Ok(ValidationResult {
            is_valid,
            errors: combined_errors,
            warnings: Vec::new(),
            sanitized_data: None,
            metadata: combined_metadata,
        })
    }
}

#[async_trait::async_trait]
impl Validator for RulesValidator {
    async fn validate_request(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult> {
        debug!(
            "Running business rules validation for request: {}",
            context.request_id
        );
        self.run_rules(data, context, true).await
    }

    async fn validate_response(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult> {
        debug!(
            "Running business rules validation for response: {}",
            context.request_id
        );
        self.run_rules(data, context, false).await
    }

    fn name(&self) -> &'static str {
        "RulesValidator"
    }
}

/// Trait for individual validation rules
#[async_trait::async_trait]
pub trait ValidationRule: Send + Sync {
    /// Validate a request
    async fn validate_request(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult>;

    /// Validate a response
    async fn validate_response(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult>;

    /// Get rule name
    fn name(&self) -> &'static str;
}

/// Authorization validation rule
pub struct AuthorizationRule;

#[async_trait::async_trait]
impl ValidationRule for AuthorizationRule {
    async fn validate_request(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult> {
        let method = data
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");

        let auth_level = context
            .client_info
            .as_ref()
            .map(|info| &info.auth_level)
            .unwrap_or(&AuthLevel::None);

        // Define method authorization requirements
        let required_auth = match method {
            // Public methods
            "initialize" | "ping" => AuthLevel::None,

            // Basic authentication required
            "tools/list" | "resources/list" | "prompts/list" => AuthLevel::Basic,

            // Full authentication required for read operations
            "tools/call" | "resources/read" | "prompts/get" => AuthLevel::Authenticated,

            // Admin required for write operations
            "sampling/createMessage" => AuthLevel::Authenticated,

            // Unknown methods require authentication
            _ => AuthLevel::Authenticated,
        };

        let is_authorized = match (auth_level, &required_auth) {
            (AuthLevel::Admin, _) => true,
            (AuthLevel::Authenticated, AuthLevel::Admin) => false,
            (AuthLevel::Authenticated, _) => true,
            (AuthLevel::Basic, AuthLevel::Authenticated | AuthLevel::Admin) => false,
            (AuthLevel::Basic, _) => true,
            (AuthLevel::None, AuthLevel::None) => true,
            (AuthLevel::None, _) => false,
        };

        if !is_authorized {
            return Ok(ValidationResult::failure(vec![ValidationError {
                field: "authorization".to_string(),
                message: format!(
                    "Method '{}' requires {:?} authentication, but client has {:?}",
                    method, required_auth, auth_level
                ),
                code: ValidationErrorCode::SecurityViolation,
                expected: Some(format!("{required_auth:?} authentication")),
                actual: Some(format!("{auth_level:?} authentication")),
                suggestion: Some("Authenticate with higher privileges".to_string()),
            }]));
        }

        Ok(ValidationResult::success()
            .with_metadata("auth_level", format!("{auth_level:?}"))
            .with_metadata("required_auth", format!("{required_auth:?}"))
            .with_metadata("method", method))
    }

    async fn validate_response(
        &self,
        _data: &Value,
        _context: &ValidationContext,
    ) -> Result<ValidationResult> {
        // Authorization is only checked for requests
        Ok(ValidationResult::success())
    }

    fn name(&self) -> &'static str {
        "AuthorizationRule"
    }
}

/// Rate limiting validation rule
pub struct RateLimitRule;

#[async_trait::async_trait]
impl ValidationRule for RateLimitRule {
    async fn validate_request(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult> {
        let method = data
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");

        if let Some(client_info) = &context.client_info {
            if let Some(rate_limit) = &client_info.rate_limit_info {
                // Check if rate limit is exceeded
                if rate_limit.current_requests >= rate_limit.max_requests {
                    let reset_time = rate_limit.reset_time;
                    let now = chrono::Utc::now();

                    if now < reset_time {
                        return Ok(ValidationResult::failure(vec![ValidationError {
                            field: "rate_limit".to_string(),
                            message: format!(
                                "Rate limit exceeded: {}/{} requests in {} seconds",
                                rate_limit.current_requests,
                                rate_limit.max_requests,
                                rate_limit.window_seconds
                            ),
                            code: ValidationErrorCode::RateLimit,
                            expected: Some(format!(
                                "Max {} requests per {} seconds",
                                rate_limit.max_requests, rate_limit.window_seconds
                            )),
                            actual: Some(format!("{} requests", rate_limit.current_requests)),
                            suggestion: Some(format!(
                                "Wait until {} to make more requests",
                                reset_time.format("%H:%M:%S")
                            )),
                        }]));
                    }
                }

                // Different rate limits for different methods
                let method_limit_factor = match method {
                    "sampling/createMessage" => 0.1, // Expensive operations get lower limits
                    "tools/call" => 0.5,
                    _ => 1.0,
                };

                let effective_limit = (rate_limit.max_requests as f64 * method_limit_factor) as u32;

                if rate_limit.current_requests >= effective_limit {
                    return Ok(ValidationResult::failure(vec![ValidationError {
                        field: "method_rate_limit".to_string(),
                        message: format!(
                            "Method '{}' rate limit exceeded: {}/{} requests",
                            method, rate_limit.current_requests, effective_limit
                        ),
                        code: ValidationErrorCode::RateLimit,
                        expected: Some(format!(
                            "Max {} requests for method '{}'",
                            effective_limit, method
                        )),
                        actual: Some(format!("{} requests", rate_limit.current_requests)),
                        suggestion: Some("Reduce request frequency for this method".to_string()),
                    }]));
                }
            }
        }

        Ok(ValidationResult::success()
            .with_metadata("rate_limit_checked", true)
            .with_metadata("method", method))
    }

    async fn validate_response(
        &self,
        _data: &Value,
        _context: &ValidationContext,
    ) -> Result<ValidationResult> {
        // Rate limiting is only checked for requests
        Ok(ValidationResult::success())
    }

    fn name(&self) -> &'static str {
        "RateLimitRule"
    }
}

/// Resource access validation rule
pub struct ResourceAccessRule;

#[async_trait::async_trait]
impl ValidationRule for ResourceAccessRule {
    async fn validate_request(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult> {
        let method = data
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");

        // Check resource access for specific methods
        if method == "resources/read" {
            if let Some(params) = data.get("params") {
                if let Some(uri) = params.get("uri").and_then(|u| u.as_str()) {
                    // Validate URI format and accessibility
                    if !self.is_valid_resource_uri(uri) {
                        return Ok(ValidationResult::failure(vec![ValidationError {
                            field: "uri".to_string(),
                            message: format!("Invalid resource URI: {}", uri),
                            code: ValidationErrorCode::InvalidFormat,
                            expected: Some("Valid resource URI (scheme:path)".to_string()),
                            actual: Some(uri.to_string()),
                            suggestion: Some("Use a valid resource URI format".to_string()),
                        }]));
                    }

                    // Check if resource is accessible based on authentication
                    let auth_level = context
                        .client_info
                        .as_ref()
                        .map(|info| &info.auth_level)
                        .unwrap_or(&AuthLevel::None);

                    if !self.is_resource_accessible(uri, auth_level) {
                        return Ok(ValidationResult::failure(vec![ValidationError {
                            field: "resource_access".to_string(),
                            message: format!("Access denied to resource: {}", uri),
                            code: ValidationErrorCode::SecurityViolation,
                            expected: Some("Sufficient privileges for resource".to_string()),
                            actual: Some(format!("{auth_level:?} authentication")),
                            suggestion: Some(
                                "Request access or authenticate with higher privileges".to_string(),
                            ),
                        }]));
                    }
                }
            }
        }

        Ok(ValidationResult::success().with_metadata("resource_access_checked", true))
    }

    async fn validate_response(
        &self,
        _data: &Value,
        _context: &ValidationContext,
    ) -> Result<ValidationResult> {
        Ok(ValidationResult::success())
    }

    fn name(&self) -> &'static str {
        "ResourceAccessRule"
    }
}

impl ResourceAccessRule {
    /// Check if URI format is valid
    fn is_valid_resource_uri(&self, uri: &str) -> bool {
        // Basic URI validation - should have scheme
        uri.contains(':') && !uri.starts_with(':') && uri.len() > 3
    }

    /// Check if resource is accessible based on auth level
    fn is_resource_accessible(&self, uri: &str, auth_level: &AuthLevel) -> bool {
        // Define resource access policies
        match uri {
            // Public resources
            uri if uri.starts_with("public:") => true,

            // Config and system resources require authentication
            uri if uri.starts_with("config:") || uri.starts_with("system:") => {
                matches!(auth_level, AuthLevel::Authenticated | AuthLevel::Admin)
            }

            // Admin resources require admin access
            uri if uri.starts_with("admin:") => {
                matches!(auth_level, AuthLevel::Admin)
            }

            // Loxone resources require authentication
            uri if uri.starts_with("loxone:") => {
                matches!(auth_level, AuthLevel::Authenticated | AuthLevel::Admin)
            }

            // Default: require authentication for unknown resources
            _ => matches!(auth_level, AuthLevel::Authenticated | AuthLevel::Admin),
        }
    }
}

/// Loxone-specific validation rule
pub struct LoxoneSpecificRule;

#[async_trait::async_trait]
impl ValidationRule for LoxoneSpecificRule {
    async fn validate_request(
        &self,
        data: &Value,
        _context: &ValidationContext,
    ) -> Result<ValidationResult> {
        let method = data
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");

        if method == "tools/call" {
            if let Some(params) = data.get("params") {
                if let Some(tool_name) = params.get("name").and_then(|n| n.as_str()) {
                    // Validate Loxone-specific tool calls
                    if let Some(error) = self.validate_loxone_tool_call(tool_name, params) {
                        return Ok(ValidationResult::failure(vec![error]));
                    }
                }
            }
        }

        Ok(ValidationResult::success().with_metadata("loxone_validation", true))
    }

    async fn validate_response(
        &self,
        _data: &Value,
        _context: &ValidationContext,
    ) -> Result<ValidationResult> {
        Ok(ValidationResult::success())
    }

    fn name(&self) -> &'static str {
        "LoxoneSpecificRule"
    }
}

impl LoxoneSpecificRule {
    /// Validate Loxone tool calls
    fn validate_loxone_tool_call(
        &self,
        tool_name: &str,
        params: &Value,
    ) -> Option<ValidationError> {
        match tool_name {
            "get_lights" | "control_light" => {
                // Validate room parameter if present
                if let Some(args) = params.get("arguments") {
                    if let Some(room) = args.get("room").and_then(|r| r.as_str()) {
                        if !self.is_valid_room_name(room) {
                            return Some(ValidationError {
                                field: "arguments.room".to_string(),
                                message: format!("Invalid room name: {}", room),
                                code: ValidationErrorCode::InvalidFormat,
                                expected: Some("Valid Loxone room name".to_string()),
                                actual: Some(room.to_string()),
                                suggestion: Some(
                                    "Use a valid room name without special characters".to_string(),
                                ),
                            });
                        }
                    }

                    // Validate device UUID if present
                    if let Some(uuid) = args.get("uuid").and_then(|u| u.as_str()) {
                        if !self.is_valid_loxone_uuid(uuid) {
                            return Some(ValidationError {
                                field: "arguments.uuid".to_string(),
                                message: format!("Invalid Loxone UUID format: {}", uuid),
                                code: ValidationErrorCode::InvalidFormat,
                                expected: Some(
                                    "Loxone UUID format (XXXXXXXX-XXXXXX-XXX)".to_string(),
                                ),
                                actual: Some(uuid.to_string()),
                                suggestion: Some("Use proper Loxone UUID format".to_string()),
                            });
                        }
                    }
                }
            }

            "get_blinds" | "control_blind" => {
                if let Some(args) = params.get("arguments") {
                    // Validate position parameter for blind control
                    if tool_name == "control_blind" {
                        if let Some(position) = args.get("position") {
                            if let Some(pos_num) = position.as_f64() {
                                if !(0.0..=1.0).contains(&pos_num) {
                                    return Some(ValidationError {
                                        field: "arguments.position".to_string(),
                                        message: format!(
                                            "Blind position out of range: {}",
                                            pos_num
                                        ),
                                        code: ValidationErrorCode::OutOfRange,
                                        expected: Some("Position between 0.0 and 1.0".to_string()),
                                        actual: Some(pos_num.to_string()),
                                        suggestion: Some(
                                            "Use position value between 0.0 (up) and 1.0 (down)"
                                                .to_string(),
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            _ => {
                // Unknown tool - this might be handled elsewhere
            }
        }

        None
    }

    /// Validate Loxone room name
    fn is_valid_room_name(&self, room: &str) -> bool {
        // Room names should be reasonable length and contain safe characters
        !room.is_empty()
            && room.len() <= 50
            && room
                .chars()
                .all(|c| c.is_alphanumeric() || " _-".contains(c))
    }

    /// Validate Loxone UUID format
    fn is_valid_loxone_uuid(&self, uuid: &str) -> bool {
        // Loxone UUIDs have format: XXXXXXXX-XXXXXX-XXX
        let parts: Vec<&str> = uuid.split('-').collect();
        parts.len() == 3
            && parts[0].len() == 8
            && parts[1].len() == 6
            && parts[2].len() == 3
            && parts
                .iter()
                .all(|part| part.chars().all(|c| c.is_ascii_hexdigit()))
    }
}

/// Security policy validation rule
pub struct SecurityPolicyRule;

#[async_trait::async_trait]
impl ValidationRule for SecurityPolicyRule {
    async fn validate_request(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult> {
        let mut errors = Vec::new();

        // Check request size
        let request_size = serde_json::to_string(data).unwrap_or_default().len();
        if request_size > context.config.max_request_size {
            errors.push(ValidationError {
                field: "request_size".to_string(),
                message: format!(
                    "Request too large: {} > {}",
                    request_size, context.config.max_request_size
                ),
                code: ValidationErrorCode::SecurityViolation,
                expected: Some(format!("Max {} bytes", context.config.max_request_size)),
                actual: Some(format!("{} bytes", request_size)),
                suggestion: Some("Reduce request payload size".to_string()),
            });
        }

        // Check object depth
        if !crate::validation::utils::validate_object_depth(data, context.config.max_object_depth) {
            errors.push(ValidationError {
                field: "object_depth".to_string(),
                message: format!(
                    "Object depth exceeds maximum: {}",
                    context.config.max_object_depth
                ),
                code: ValidationErrorCode::SecurityViolation,
                expected: Some(format!("Max depth: {}", context.config.max_object_depth)),
                actual: Some("Excessive depth".to_string()),
                suggestion: Some("Flatten object structure".to_string()),
            });
        }

        // Check for suspicious patterns in request
        if self.contains_suspicious_patterns(data) {
            errors.push(ValidationError {
                field: "security_scan".to_string(),
                message: "Request contains suspicious patterns".to_string(),
                code: ValidationErrorCode::SecurityViolation,
                expected: Some("Clean request data".to_string()),
                actual: Some("Suspicious patterns detected".to_string()),
                suggestion: Some("Remove potentially malicious content".to_string()),
            });
        }

        let is_valid = errors.is_empty();
        Ok(ValidationResult {
            is_valid,
            errors,
            warnings: Vec::new(),
            sanitized_data: None,
            metadata: HashMap::from([
                ("security_scan".to_string(), serde_json::Value::Bool(true)),
                (
                    "request_size".to_string(),
                    serde_json::Value::Number(request_size.into()),
                ),
            ]),
        })
    }

    async fn validate_response(
        &self,
        _data: &Value,
        _context: &ValidationContext,
    ) -> Result<ValidationResult> {
        // Security policies are primarily for requests
        Ok(ValidationResult::success())
    }

    fn name(&self) -> &'static str {
        "SecurityPolicyRule"
    }
}

impl SecurityPolicyRule {
    /// Check for suspicious patterns in the data
    fn contains_suspicious_patterns(&self, data: &Value) -> bool {
        let data_str = serde_json::to_string(data).unwrap_or_default();

        // Check for common attack patterns
        let suspicious_patterns = vec![
            "eval(",
            "exec(",
            "system(",
            "shell_exec(",
            "passthru(",
            "file_get_contents(",
            "base64_decode(",
            "unserialize(",
            "../",
            "..\\",
            "/etc/passwd",
            "/etc/shadow",
            "cmd.exe",
            "powershell",
        ];

        suspicious_patterns
            .iter()
            .any(|pattern| data_str.to_lowercase().contains(&pattern.to_lowercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::ClientInfo;
    use serde_json::json;

    fn create_test_context(auth_level: AuthLevel) -> ValidationContext {
        let mut context = ValidationContext::new("test".to_string(), Default::default());
        context.client_info = Some(ClientInfo {
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("test".to_string()),
            client_id: Some("test_client".to_string()),
            auth_level,
            rate_limit_info: None,
        });
        context
    }

    #[tokio::test]
    async fn test_authorization_rule() {
        let rule = AuthorizationRule;

        // Test public method with no auth
        let data = json!({"method": "initialize"});
        let context = create_test_context(AuthLevel::None);
        let result = rule.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);

        // Test authenticated method without auth
        let data = json!({"method": "tools/call"});
        let context = create_test_context(AuthLevel::None);
        let result = rule.validate_request(&data, &context).await.unwrap();
        assert!(!result.is_valid);
        assert_eq!(
            result.errors[0].code,
            ValidationErrorCode::SecurityViolation
        );

        // Test authenticated method with auth
        let context = create_test_context(AuthLevel::Authenticated);
        let result = rule.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);
    }

    #[tokio::test]
    async fn test_loxone_specific_rule() {
        let rule = LoxoneSpecificRule;
        let context = create_test_context(AuthLevel::Authenticated);

        // Test valid Loxone tool call
        let data = json!({
            "method": "tools/call",
            "params": {
                "name": "get_lights",
                "arguments": {
                    "room": "Living_Room"
                }
            }
        });
        let result = rule.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);

        // Test invalid room name
        let data = json!({
            "method": "tools/call",
            "params": {
                "name": "get_lights",
                "arguments": {
                    "room": "Invalid<>Room"
                }
            }
        });
        let result = rule.validate_request(&data, &context).await.unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::InvalidFormat);

        // Test invalid UUID
        let data = json!({
            "method": "tools/call",
            "params": {
                "name": "control_light",
                "arguments": {
                    "uuid": "invalid-uuid-format"
                }
            }
        });
        let result = rule.validate_request(&data, &context).await.unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::InvalidFormat);

        // Test valid Loxone UUID
        let data = json!({
            "method": "tools/call",
            "params": {
                "name": "control_light",
                "arguments": {
                    "uuid": "12345678-ABCDEF-123"
                }
            }
        });
        let result = rule.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);
    }

    #[tokio::test]
    async fn test_security_policy_rule() {
        let rule = SecurityPolicyRule;
        let context = create_test_context(AuthLevel::Authenticated);

        // Test normal request
        let data = json!({"method": "tools/call", "params": {"name": "test"}});
        let result = rule.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);

        // Test request with suspicious content
        let data = json!({
            "method": "tools/call",
            "params": {
                "malicious": "eval(document.cookie)"
            }
        });
        let result = rule.validate_request(&data, &context).await.unwrap();
        assert!(!result.is_valid);
        assert_eq!(
            result.errors[0].code,
            ValidationErrorCode::SecurityViolation
        );
    }

    #[tokio::test]
    async fn test_rules_validator() {
        let validator = RulesValidator::new();
        let context = create_test_context(AuthLevel::Authenticated);

        // Test valid request
        let data = json!({
            "method": "tools/call",
            "params": {
                "name": "get_lights",
                "arguments": {"room": "Kitchen"}
            }
        });
        let result = validator.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);
    }
}
