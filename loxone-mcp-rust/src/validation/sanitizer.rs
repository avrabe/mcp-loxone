//! Input sanitization and normalization

use super::{
    utils, ValidationConfig, ValidationContext, ValidationResult, ValidationWarning,
    ValidationWarningCode, Validator,
};
use crate::error::Result;
use serde_json::{Map, Value};
use tracing::debug;

/// Input sanitizer that cleans and normalizes data
pub struct SanitizerValidator {
    config: SanitizerConfig,
}

impl SanitizerValidator {
    /// Create new sanitizer with configuration
    pub fn new(config: SanitizerConfig) -> Self {
        Self { config }
    }

    /// Sanitize a JSON value recursively
    fn sanitize_value(
        &self,
        value: &Value,
        context: &ValidationContext,
    ) -> (Value, Vec<ValidationWarning>) {
        let mut warnings = Vec::new();

        match value {
            Value::String(s) => {
                let (sanitized, string_warnings) = self.sanitize_string(s, context);
                warnings.extend(string_warnings);
                (Value::String(sanitized), warnings)
            }
            Value::Array(arr) => {
                let mut sanitized_arr = Vec::new();
                for (index, item) in arr.iter().enumerate() {
                    if sanitized_arr.len() >= self.config.max_array_size {
                        warnings.push(ValidationWarning {
                            field: format!("array[{}+]", index),
                            message: format!(
                                "Array truncated at {} items",
                                self.config.max_array_size
                            ),
                            code: ValidationWarningCode::PerformanceImpact,
                            recommendation: Some(
                                "Consider reducing array size for better performance".to_string(),
                            ),
                        });
                        break;
                    }
                    let (sanitized_item, item_warnings) = self.sanitize_value(item, context);
                    warnings.extend(item_warnings);
                    sanitized_arr.push(sanitized_item);
                }
                (Value::Array(sanitized_arr), warnings)
            }
            Value::Object(obj) => {
                let (sanitized_obj, obj_warnings) = self.sanitize_object(obj, context, 0);
                warnings.extend(obj_warnings);
                (Value::Object(sanitized_obj), warnings)
            }
            Value::Number(n) => {
                let (sanitized_num, num_warnings) = self.sanitize_number(n);
                warnings.extend(num_warnings);
                (Value::Number(sanitized_num), warnings)
            }
            // Boolean and Null values don't need sanitization
            _ => (value.clone(), warnings),
        }
    }

    /// Sanitize string values
    fn sanitize_string(
        &self,
        value: &str,
        _context: &ValidationContext,
    ) -> (String, Vec<ValidationWarning>) {
        let mut warnings = Vec::new();
        let original_length = value.len();

        // Remove null bytes and control characters
        let mut sanitized = value
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
            .collect::<String>();

        // Trim whitespace
        if self.config.trim_whitespace {
            sanitized = sanitized.trim().to_string();
        }

        // Normalize whitespace
        if self.config.normalize_whitespace {
            sanitized = utils::sanitize_string(&sanitized, usize::MAX);
        }

        // Check for potentially malicious content
        if self.config.check_malicious_content && utils::contains_malicious_content(&sanitized) {
            warnings.push(ValidationWarning {
                field: "string_content".to_string(),
                message: "String contains potentially malicious content".to_string(),
                code: ValidationWarningCode::SecurityConcern,
                recommendation: Some("Review and sanitize the content manually".to_string()),
            });

            // Optionally remove malicious content
            if self.config.remove_malicious_content {
                sanitized = self.remove_malicious_patterns(&sanitized);
            }
        }

        // Truncate if too long
        if sanitized.len() > self.config.max_string_length {
            let truncated_length = self.config.max_string_length;
            sanitized = sanitized.chars().take(truncated_length).collect();
            warnings.push(ValidationWarning {
                field: "string_length".to_string(),
                message: format!(
                    "String truncated from {} to {} characters",
                    original_length, truncated_length
                ),
                code: ValidationWarningCode::PerformanceImpact,
                recommendation: Some("Consider using shorter strings".to_string()),
            });
        }

        // Check for encoding issues
        if sanitized.len() != original_length {
            warnings.push(ValidationWarning {
                field: "string_encoding".to_string(),
                message: "String contained control characters that were removed".to_string(),
                code: ValidationWarningCode::CompatibilityIssue,
                recommendation: Some("Use clean UTF-8 text without control characters".to_string()),
            });
        }

        (sanitized, warnings)
    }

    /// Sanitize object values
    fn sanitize_object(
        &self,
        obj: &Map<String, Value>,
        context: &ValidationContext,
        depth: usize,
    ) -> (Map<String, Value>, Vec<ValidationWarning>) {
        let mut warnings = Vec::new();
        let mut sanitized_obj = Map::new();

        // Check object depth
        if depth > self.config.max_object_depth {
            warnings.push(ValidationWarning {
                field: "object_depth".to_string(),
                message: format!(
                    "Object depth {} exceeds maximum {}",
                    depth, self.config.max_object_depth
                ),
                code: ValidationWarningCode::PerformanceImpact,
                recommendation: Some("Flatten object structure".to_string()),
            });
            return (sanitized_obj, warnings);
        }

        // Sanitize each property
        for (key, value) in obj {
            // Sanitize property key
            let (sanitized_key, key_warnings) = self.sanitize_string(key, context);
            warnings.extend(key_warnings);

            // Skip empty keys
            if sanitized_key.is_empty() {
                warnings.push(ValidationWarning {
                    field: "object_key".to_string(),
                    message: "Empty object key was skipped".to_string(),
                    code: ValidationWarningCode::CompatibilityIssue,
                    recommendation: Some("Use non-empty property names".to_string()),
                });
                continue;
            }

            // Check for reserved/dangerous property names
            if self.is_dangerous_property_name(&sanitized_key) {
                warnings.push(ValidationWarning {
                    field: sanitized_key.clone(),
                    message: format!("Property name '{}' may cause issues", sanitized_key),
                    code: ValidationWarningCode::SecurityConcern,
                    recommendation: Some("Consider using a different property name".to_string()),
                });
            }

            // Sanitize property value
            let (sanitized_value, value_warnings) = self.sanitize_value(value, context);
            warnings.extend(value_warnings);

            sanitized_obj.insert(sanitized_key, sanitized_value);

            // Limit object size
            if sanitized_obj.len() >= self.config.max_object_properties {
                warnings.push(ValidationWarning {
                    field: "object_size".to_string(),
                    message: format!(
                        "Object truncated at {} properties",
                        self.config.max_object_properties
                    ),
                    code: ValidationWarningCode::PerformanceImpact,
                    recommendation: Some("Consider reducing object size".to_string()),
                });
                break;
            }
        }

        (sanitized_obj, warnings)
    }

    /// Sanitize numeric values
    fn sanitize_number(
        &self,
        value: &serde_json::Number,
    ) -> (serde_json::Number, Vec<ValidationWarning>) {
        let mut warnings = Vec::new();

        // Check for extreme values
        if let Some(f) = value.as_f64() {
            if f.is_infinite() {
                warnings.push(ValidationWarning {
                    field: "number_value".to_string(),
                    message: "Infinite number detected".to_string(),
                    code: ValidationWarningCode::CompatibilityIssue,
                    recommendation: Some("Use finite numeric values".to_string()),
                });
                // Convert to a large but finite number
                return (
                    serde_json::Number::from_f64(f64::MAX / 2.0).unwrap_or_else(|| value.clone()),
                    warnings,
                );
            }

            if f.is_nan() {
                warnings.push(ValidationWarning {
                    field: "number_value".to_string(),
                    message: "NaN (Not a Number) detected".to_string(),
                    code: ValidationWarningCode::CompatibilityIssue,
                    recommendation: Some("Use valid numeric values".to_string()),
                });
                // Convert to zero
                return (serde_json::Number::from(0), warnings);
            }

            // Check for precision loss
            if f.abs() > 2_f64.powi(53) {
                warnings.push(ValidationWarning {
                    field: "number_precision".to_string(),
                    message: "Number may lose precision in JavaScript environments".to_string(),
                    code: ValidationWarningCode::CompatibilityIssue,
                    recommendation: Some("Consider using strings for large integers".to_string()),
                });
            }
        }

        (value.clone(), warnings)
    }

    /// Remove malicious patterns from string
    fn remove_malicious_patterns(&self, value: &str) -> String {
        let mut sanitized = value.to_string();

        // Remove common XSS patterns
        let patterns = vec![
            (r"<script[^>]*>.*?</script>", ""),
            (r"javascript:", ""),
            (r#"on\w+\s*=\s*["'][^"']*["']"#, ""),
            (r"eval\s*\(", "eval_removed("),
            (r"exec\s*\(", "exec_removed("),
            (r"\$\(", "dollar_removed("),
        ];

        for (pattern, replacement) in patterns {
            if let Ok(regex) = regex::Regex::new(pattern) {
                sanitized = regex.replace_all(&sanitized, replacement).to_string();
            }
        }

        sanitized
    }

    /// Check if property name is dangerous
    fn is_dangerous_property_name(&self, name: &str) -> bool {
        let dangerous_names = [
            "__proto__",
            "constructor",
            "prototype",
            "eval",
            "function",
            "this",
            "arguments",
            "window",
            "document",
            "location",
            "navigator",
        ];

        dangerous_names.contains(&name.to_lowercase().as_str())
    }
}

#[async_trait::async_trait]
impl Validator for SanitizerValidator {
    async fn validate_request(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult> {
        debug!(
            "Sanitizing request data for context: {}",
            context.request_id
        );

        if !context.config.enable_sanitization {
            return Ok(ValidationResult::success().with_metadata("sanitization", "disabled"));
        }

        let (sanitized_data, warnings) = self.sanitize_value(data, context);

        let mut result = ValidationResult::success()
            .with_sanitized_data(sanitized_data)
            .with_metadata("sanitization", "enabled")
            .with_metadata("warnings_count", warnings.len());

        for warning in warnings {
            result = result.with_warning(warning);
        }

        Ok(result)
    }

    async fn validate_response(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult> {
        debug!(
            "Sanitizing response data for context: {}",
            context.request_id
        );

        if !context.config.enable_sanitization {
            return Ok(ValidationResult::success().with_metadata("sanitization", "disabled"));
        }

        let (sanitized_data, warnings) = self.sanitize_value(data, context);

        let mut result = ValidationResult::success()
            .with_sanitized_data(sanitized_data)
            .with_metadata("sanitization", "enabled")
            .with_metadata("warnings_count", warnings.len());

        for warning in warnings {
            result = result.with_warning(warning);
        }

        Ok(result)
    }

    fn name(&self) -> &'static str {
        "SanitizerValidator"
    }
}

/// Sanitizer configuration
#[derive(Debug, Clone)]
pub struct SanitizerConfig {
    /// Remove leading/trailing whitespace
    pub trim_whitespace: bool,
    /// Normalize multiple consecutive spaces
    pub normalize_whitespace: bool,
    /// Check for malicious content
    pub check_malicious_content: bool,
    /// Remove malicious content instead of just warning
    pub remove_malicious_content: bool,
    /// Maximum string length
    pub max_string_length: usize,
    /// Maximum array size
    pub max_array_size: usize,
    /// Maximum object depth
    pub max_object_depth: usize,
    /// Maximum object properties
    pub max_object_properties: usize,
}

impl Default for SanitizerConfig {
    fn default() -> Self {
        Self {
            trim_whitespace: true,
            normalize_whitespace: true,
            check_malicious_content: true,
            remove_malicious_content: false, // Just warn by default
            max_string_length: 10000,
            max_array_size: 1000,
            max_object_depth: 10,
            max_object_properties: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_string_sanitization() {
        let config = SanitizerConfig::default();
        let sanitizer = SanitizerValidator::new(config);
        let context = ValidationContext::new("test".to_string(), Default::default());

        // Test whitespace trimming
        let data = json!("  hello world  ");
        let result = sanitizer.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);
        assert_eq!(result.sanitized_data.unwrap(), json!("hello world"));
    }

    #[tokio::test]
    async fn test_malicious_content_detection() {
        let config = SanitizerConfig::default();
        let sanitizer = SanitizerValidator::new(config);
        let context = ValidationContext::new("test".to_string(), Default::default());

        let data = json!("<script>alert('xss')</script>");
        let result = sanitizer.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);
        assert!(!result.warnings.is_empty());
        assert_eq!(
            result.warnings[0].code,
            ValidationWarningCode::SecurityConcern
        );
    }

    #[tokio::test]
    async fn test_malicious_content_removal() {
        let mut config = SanitizerConfig::default();
        config.remove_malicious_content = true;
        let sanitizer = SanitizerValidator::new(config);
        let context = ValidationContext::new("test".to_string(), Default::default());

        let data = json!("Hello <script>alert('xss')</script> World");
        let result = sanitizer.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);
        let sanitized = result.sanitized_data.unwrap();
        assert_eq!(sanitized, json!("Hello  World"));
    }

    #[tokio::test]
    async fn test_object_sanitization() {
        let config = SanitizerConfig::default();
        let sanitizer = SanitizerValidator::new(config);
        let context = ValidationContext::new("test".to_string(), Default::default());

        let data = json!({
            "  normal_key  ": "value",
            "__proto__": "dangerous",
            "": "empty_key",
            "good_key": "  good value  "
        });

        let result = sanitizer.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);

        let sanitized = result.sanitized_data.unwrap();
        let obj = sanitized.as_object().unwrap();

        // Check that keys are trimmed
        assert!(obj.contains_key("normal_key"));
        assert!(obj.contains_key("__proto__")); // Dangerous but preserved
        assert!(!obj.contains_key("")); // Empty key should be removed
        assert_eq!(obj.get("good_key").unwrap(), "good value"); // Value should be trimmed

        // Should have warnings about dangerous property name and empty key
        assert!(!result.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_array_size_limits() {
        let config = SanitizerConfig {
            max_array_size: 2,
            ..Default::default()
        };
        let sanitizer = SanitizerValidator::new(config);
        let context = ValidationContext::new("test".to_string(), Default::default());

        let data = json!([1, 2, 3, 4, 5]);
        let result = sanitizer.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);

        let sanitized = result.sanitized_data.unwrap();
        let arr = sanitized.as_array().unwrap();
        assert_eq!(arr.len(), 2); // Should be truncated

        // Should have warning about truncation
        assert!(!result.warnings.is_empty());
        assert_eq!(
            result.warnings[0].code,
            ValidationWarningCode::PerformanceImpact
        );
    }

    #[tokio::test]
    async fn test_number_sanitization() {
        let config = SanitizerConfig::default();
        let sanitizer = SanitizerValidator::new(config);
        let context = ValidationContext::new("test".to_string(), Default::default());

        // Test with a very large number that might lose precision
        let data = json!(9007199254740992_i64); // 2^53
        let result = sanitizer.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);

        // Should have warning about precision
        assert!(!result.warnings.is_empty());
        assert_eq!(
            result.warnings[0].code,
            ValidationWarningCode::CompatibilityIssue
        );
    }

    #[tokio::test]
    async fn test_disabled_sanitization() {
        let config = SanitizerConfig::default();
        let sanitizer = SanitizerValidator::new(config);

        let validation_config = ValidationConfig {
            enable_sanitization: false,
            ..Default::default()
        };
        let context = ValidationContext::new("test".to_string(), validation_config);

        let data = json!("  unsanitized  ");
        let result = sanitizer.validate_request(&data, &context).await.unwrap();
        assert!(result.is_valid);
        assert!(result.sanitized_data.is_none()); // No sanitization should occur
        assert_eq!(result.metadata.get("sanitization").unwrap(), "disabled");
    }
}
