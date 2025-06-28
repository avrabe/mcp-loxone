//! JSON Schema validation for MCP requests and responses

use super::{
    utils, ValidationContext, ValidationError, ValidationErrorCode, ValidationResult, Validator,
};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, warn};

/// JSON Schema validator
pub struct SchemaValidator {
    /// MCP request schemas
    request_schemas: HashMap<String, Schema>,
    /// MCP response schemas
    response_schemas: HashMap<String, Schema>,
    /// Global schema definitions
    #[allow(dead_code)]
    definitions: HashMap<String, Schema>,
}

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaValidator {
    /// Create new schema validator with MCP schemas
    pub fn new() -> Self {
        let mut validator = Self {
            request_schemas: HashMap::new(),
            response_schemas: HashMap::new(),
            definitions: HashMap::new(),
        };

        // Load standard MCP schemas
        validator.load_mcp_schemas();
        validator
    }

    /// Load standard MCP protocol schemas
    fn load_mcp_schemas(&mut self) {
        // MCP initialize request schema
        self.add_request_schema(
            "initialize",
            Schema {
                schema_type: SchemaType::Object,
                properties: Some(HashMap::from([
                    (
                        "protocolVersion".to_string(),
                        Schema {
                            schema_type: SchemaType::String,
                            pattern: Some(r"^\d+\.\d+\.\d+$".to_string()),
                            ..Default::default()
                        },
                    ),
                    (
                        "capabilities".to_string(),
                        Schema {
                            schema_type: SchemaType::Object,
                            properties: Some(HashMap::from([
                                (
                                    "roots".to_string(),
                                    Schema {
                                        schema_type: SchemaType::Object,
                                        properties: Some(HashMap::from([(
                                            "listChanged".to_string(),
                                            Schema {
                                                schema_type: SchemaType::Boolean,
                                                ..Default::default()
                                            },
                                        )])),
                                        ..Default::default()
                                    },
                                ),
                                (
                                    "sampling".to_string(),
                                    Schema {
                                        schema_type: SchemaType::Object,
                                        ..Default::default()
                                    },
                                ),
                            ])),
                            ..Default::default()
                        },
                    ),
                    (
                        "clientInfo".to_string(),
                        Schema {
                            schema_type: SchemaType::Object,
                            required: Some(vec!["name".to_string(), "version".to_string()]),
                            properties: Some(HashMap::from([
                                (
                                    "name".to_string(),
                                    Schema {
                                        schema_type: SchemaType::String,
                                        min_length: Some(1),
                                        max_length: Some(100),
                                        ..Default::default()
                                    },
                                ),
                                (
                                    "version".to_string(),
                                    Schema {
                                        schema_type: SchemaType::String,
                                        pattern: Some(r"^\d+\.\d+\.\d+.*$".to_string()),
                                        ..Default::default()
                                    },
                                ),
                            ])),
                            ..Default::default()
                        },
                    ),
                ])),
                required: Some(vec![
                    "protocolVersion".to_string(),
                    "capabilities".to_string(),
                    "clientInfo".to_string(),
                ]),
                ..Default::default()
            },
        );

        // Tool call request schema
        self.add_request_schema(
            "tools/call",
            Schema {
                schema_type: SchemaType::Object,
                properties: Some(HashMap::from([
                    (
                        "name".to_string(),
                        Schema {
                            schema_type: SchemaType::String,
                            min_length: Some(1),
                            max_length: Some(100),
                            pattern: Some(r"^[a-zA-Z][a-zA-Z0-9_-]*$".to_string()),
                            ..Default::default()
                        },
                    ),
                    (
                        "arguments".to_string(),
                        Schema {
                            schema_type: SchemaType::Object,
                            ..Default::default()
                        },
                    ),
                ])),
                required: Some(vec!["name".to_string()]),
                ..Default::default()
            },
        );

        // Resource request schema
        self.add_request_schema(
            "resources/read",
            Schema {
                schema_type: SchemaType::Object,
                properties: Some(HashMap::from([(
                    "uri".to_string(),
                    Schema {
                        schema_type: SchemaType::String,
                        min_length: Some(1),
                        max_length: Some(500),
                        pattern: Some(r"^[a-zA-Z][a-zA-Z0-9+.-]*:".to_string()), // URI scheme pattern
                        ..Default::default()
                    },
                )])),
                required: Some(vec!["uri".to_string()]),
                ..Default::default()
            },
        );

        // Sampling request schema
        self.add_request_schema(
            "sampling/createMessage",
            Schema {
                schema_type: SchemaType::Object,
                properties: Some(HashMap::from([
                    (
                        "messages".to_string(),
                        Schema {
                            schema_type: SchemaType::Array,
                            items: Some(Box::new(Schema {
                                schema_type: SchemaType::Object,
                                properties: Some(HashMap::from([
                                    (
                                        "role".to_string(),
                                        Schema {
                                            schema_type: SchemaType::String,
                                            enum_values: Some(vec![
                                                "user".to_string(),
                                                "assistant".to_string(),
                                                "system".to_string(),
                                            ]),
                                            ..Default::default()
                                        },
                                    ),
                                    (
                                        "content".to_string(),
                                        Schema {
                                            schema_type: SchemaType::Object,
                                            properties: Some(HashMap::from([
                                                (
                                                    "type".to_string(),
                                                    Schema {
                                                        schema_type: SchemaType::String,
                                                        enum_values: Some(vec![
                                                            "text".to_string(),
                                                            "image".to_string(),
                                                        ]),
                                                        ..Default::default()
                                                    },
                                                ),
                                                (
                                                    "text".to_string(),
                                                    Schema {
                                                        schema_type: SchemaType::String,
                                                        max_length: Some(100000),
                                                        ..Default::default()
                                                    },
                                                ),
                                            ])),
                                            required: Some(vec!["type".to_string()]),
                                            ..Default::default()
                                        },
                                    ),
                                ])),
                                required: Some(vec!["role".to_string(), "content".to_string()]),
                                ..Default::default()
                            })),
                            min_items: Some(1),
                            max_items: Some(50),
                            ..Default::default()
                        },
                    ),
                    (
                        "maxTokens".to_string(),
                        Schema {
                            schema_type: SchemaType::Integer,
                            minimum: Some(1.0),
                            maximum: Some(8192.0),
                            ..Default::default()
                        },
                    ),
                    (
                        "temperature".to_string(),
                        Schema {
                            schema_type: SchemaType::Number,
                            minimum: Some(0.0),
                            maximum: Some(2.0),
                            ..Default::default()
                        },
                    ),
                ])),
                required: Some(vec!["messages".to_string()]),
                ..Default::default()
            },
        );

        // Add response schemas
        self.add_response_schema(
            "tools/call",
            Schema {
                schema_type: SchemaType::Object,
                properties: Some(HashMap::from([
                    (
                        "content".to_string(),
                        Schema {
                            schema_type: SchemaType::Array,
                            items: Some(Box::new(Schema {
                                schema_type: SchemaType::Object,
                                properties: Some(HashMap::from([
                                    (
                                        "type".to_string(),
                                        Schema {
                                            schema_type: SchemaType::String,
                                            enum_values: Some(vec![
                                                "text".to_string(),
                                                "resource".to_string(),
                                            ]),
                                            ..Default::default()
                                        },
                                    ),
                                    (
                                        "text".to_string(),
                                        Schema {
                                            schema_type: SchemaType::String,
                                            ..Default::default()
                                        },
                                    ),
                                ])),
                                required: Some(vec!["type".to_string()]),
                                ..Default::default()
                            })),
                            ..Default::default()
                        },
                    ),
                    (
                        "isError".to_string(),
                        Schema {
                            schema_type: SchemaType::Boolean,
                            ..Default::default()
                        },
                    ),
                ])),
                ..Default::default()
            },
        );
    }

    /// Add a request schema
    pub fn add_request_schema(&mut self, method: &str, schema: Schema) {
        self.request_schemas.insert(method.to_string(), schema);
    }

    /// Add a response schema
    pub fn add_response_schema(&mut self, method: &str, schema: Schema) {
        self.response_schemas.insert(method.to_string(), schema);
    }

    /// Validate data against a schema
    fn validate_against_schema(
        &self,
        data: &Value,
        schema: &Schema,
        field_path: &str,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Type validation
        if !self.validate_type(data, &schema.schema_type) {
            errors.push(ValidationError {
                field: field_path.to_string(),
                message: format!(
                    "Expected type {:?}, got {:?}",
                    schema.schema_type,
                    self.get_value_type(data)
                ),
                code: ValidationErrorCode::InvalidType,
                expected: Some(format!("{:?}", schema.schema_type)),
                actual: Some(format!("{:?}", self.get_value_type(data))),
                suggestion: Some(format!("Convert value to {:?}", schema.schema_type)),
            });
            return errors; // Return early if type is wrong
        }

        // Validate based on type
        match (&schema.schema_type, data) {
            (SchemaType::String, Value::String(s)) => {
                self.validate_string(s, schema, field_path, &mut errors);
            }
            (SchemaType::Integer | SchemaType::Number, Value::Number(n)) => {
                self.validate_number(n, schema, field_path, &mut errors);
            }
            (SchemaType::Array, Value::Array(arr)) => {
                self.validate_array(arr, schema, field_path, &mut errors);
            }
            (SchemaType::Object, Value::Object(obj)) => {
                self.validate_object(obj, schema, field_path, &mut errors);
            }
            _ => {}
        }

        errors
    }

    /// Validate string values
    fn validate_string(
        &self,
        value: &str,
        schema: &Schema,
        field_path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        // Length validation
        if let Some(min_len) = schema.min_length {
            if value.len() < min_len {
                errors.push(ValidationError {
                    field: field_path.to_string(),
                    message: format!("String too short: {} < {}", value.len(), min_len),
                    code: ValidationErrorCode::TooShort,
                    expected: Some(format!("Minimum length: {min_len}")),
                    actual: Some(format!("Length: {}", value.len())),
                    suggestion: Some("Provide a longer string".to_string()),
                });
            }
        }

        if let Some(max_len) = schema.max_length {
            if value.len() > max_len {
                errors.push(ValidationError {
                    field: field_path.to_string(),
                    message: format!("String too long: {} > {}", value.len(), max_len),
                    code: ValidationErrorCode::TooLong,
                    expected: Some(format!("Maximum length: {max_len}")),
                    actual: Some(format!("Length: {}", value.len())),
                    suggestion: Some("Provide a shorter string".to_string()),
                });
            }
        }

        // Pattern validation
        if let Some(pattern) = &schema.pattern {
            if let Ok(regex) = regex::Regex::new(pattern) {
                if !regex.is_match(value) {
                    errors.push(ValidationError {
                        field: field_path.to_string(),
                        message: format!("String does not match pattern: {pattern}"),
                        code: ValidationErrorCode::PatternMismatch,
                        expected: Some(format!("Pattern: {pattern}")),
                        actual: Some(value.to_string()),
                        suggestion: Some(
                            "Provide a string matching the required pattern".to_string(),
                        ),
                    });
                }
            }
        }

        // Enum validation
        if let Some(enum_values) = &schema.enum_values {
            if !enum_values.contains(&value.to_string()) {
                errors.push(ValidationError {
                    field: field_path.to_string(),
                    message: format!("Invalid enum value: {value}"),
                    code: ValidationErrorCode::InvalidEnum,
                    expected: Some(format!("One of: {:?}", enum_values)),
                    actual: Some(value.to_string()),
                    suggestion: Some(format!("Use one of: {}", enum_values.join(", "))),
                });
            }
        }

        // Security validation
        if utils::contains_malicious_content(value) {
            errors.push(ValidationError {
                field: field_path.to_string(),
                message: "String contains potentially malicious content".to_string(),
                code: ValidationErrorCode::MaliciousContent,
                expected: Some("Safe string content".to_string()),
                actual: Some("Potentially malicious content".to_string()),
                suggestion: Some(
                    "Remove script tags, JavaScript, and SQL injection attempts".to_string(),
                ),
            });
        }
    }

    /// Validate numeric values
    fn validate_number(
        &self,
        value: &serde_json::Number,
        schema: &Schema,
        field_path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        let num_value = if let Some(i) = value.as_i64() {
            i as f64
        } else if let Some(f) = value.as_f64() {
            f
        } else {
            return;
        };

        // Range validation
        if let Some(min) = schema.minimum {
            if num_value < min {
                errors.push(ValidationError {
                    field: field_path.to_string(),
                    message: format!("Number too small: {} < {}", num_value, min),
                    code: ValidationErrorCode::OutOfRange,
                    expected: Some(format!("Minimum: {min}")),
                    actual: Some(num_value.to_string()),
                    suggestion: Some(format!("Use a value >= {}", min)),
                });
            }
        }

        if let Some(max) = schema.maximum {
            if num_value > max {
                errors.push(ValidationError {
                    field: field_path.to_string(),
                    message: format!("Number too large: {} > {}", num_value, max),
                    code: ValidationErrorCode::OutOfRange,
                    expected: Some(format!("Maximum: {max}")),
                    actual: Some(num_value.to_string()),
                    suggestion: Some(format!("Use a value <= {}", max)),
                });
            }
        }
    }

    /// Validate array values
    fn validate_array(
        &self,
        value: &[Value],
        schema: &Schema,
        field_path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        // Length validation
        if let Some(min_items) = schema.min_items {
            if value.len() < min_items {
                errors.push(ValidationError {
                    field: field_path.to_string(),
                    message: format!("Array too short: {} < {}", value.len(), min_items),
                    code: ValidationErrorCode::TooShort,
                    expected: Some(format!("Minimum items: {}", min_items)),
                    actual: Some(format!("Items: {}", value.len())),
                    suggestion: Some("Add more items to the array".to_string()),
                });
            }
        }

        if let Some(max_items) = schema.max_items {
            if value.len() > max_items {
                errors.push(ValidationError {
                    field: field_path.to_string(),
                    message: format!("Array too long: {} > {}", value.len(), max_items),
                    code: ValidationErrorCode::TooLong,
                    expected: Some(format!("Maximum items: {}", max_items)),
                    actual: Some(format!("Items: {}", value.len())),
                    suggestion: Some("Remove some items from the array".to_string()),
                });
            }
        }

        // Validate array items
        if let Some(items_schema) = &schema.items {
            for (index, item) in value.iter().enumerate() {
                let item_path = format!("{}[{}]", field_path, index);
                let item_errors = self.validate_against_schema(item, items_schema, &item_path);
                errors.extend(item_errors);
            }
        }
    }

    /// Validate object values
    fn validate_object(
        &self,
        value: &serde_json::Map<String, Value>,
        schema: &Schema,
        field_path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        // Required fields validation
        if let Some(required) = &schema.required {
            for req_field in required {
                if !value.contains_key(req_field) {
                    errors.push(ValidationError {
                        field: format!("{}.{}", field_path, req_field),
                        message: format!("Required field '{}' is missing", req_field),
                        code: ValidationErrorCode::MissingRequired,
                        expected: Some(format!("Field: {}", req_field)),
                        actual: Some("Field not present".to_string()),
                        suggestion: Some(format!("Add the required field '{}'", req_field)),
                    });
                }
            }
        }

        // Validate properties
        if let Some(properties) = &schema.properties {
            for (prop_name, prop_value) in value {
                if let Some(prop_schema) = properties.get(prop_name) {
                    let prop_path = if field_path.is_empty() {
                        prop_name.clone()
                    } else {
                        format!("{}.{}", field_path, prop_name)
                    };
                    let prop_errors =
                        self.validate_against_schema(prop_value, prop_schema, &prop_path);
                    errors.extend(prop_errors);
                }
            }
        }
    }

    /// Check if value matches expected type
    fn validate_type(&self, value: &Value, expected_type: &SchemaType) -> bool {
        match (expected_type, value) {
            (SchemaType::String, Value::String(_)) => true,
            (SchemaType::Integer, Value::Number(n)) => n.is_i64(),
            (SchemaType::Number, Value::Number(_)) => true,
            (SchemaType::Boolean, Value::Bool(_)) => true,
            (SchemaType::Array, Value::Array(_)) => true,
            (SchemaType::Object, Value::Object(_)) => true,
            (SchemaType::Null, Value::Null) => true,
            _ => false,
        }
    }

    /// Get the type of a JSON value
    fn get_value_type(&self, value: &Value) -> SchemaType {
        match value {
            Value::String(_) => SchemaType::String,
            Value::Number(n) if n.is_i64() => SchemaType::Integer,
            Value::Number(_) => SchemaType::Number,
            Value::Bool(_) => SchemaType::Boolean,
            Value::Array(_) => SchemaType::Array,
            Value::Object(_) => SchemaType::Object,
            Value::Null => SchemaType::Null,
        }
    }
}

#[async_trait::async_trait]
impl Validator for SchemaValidator {
    async fn validate_request(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult> {
        debug!(
            "Validating request schema for context: {}",
            context.request_id
        );

        // Extract method from request data
        let method = if let Some(method_value) = data.get("method") {
            if let Some(method_str) = method_value.as_str() {
                method_str
            } else {
                return Ok(ValidationResult::failure(vec![ValidationError {
                    field: "method".to_string(),
                    message: "Method must be a string".to_string(),
                    code: ValidationErrorCode::InvalidType,
                    expected: Some("string".to_string()),
                    actual: Some("non-string".to_string()),
                    suggestion: Some("Provide method as a string".to_string()),
                }]));
            }
        } else {
            return Ok(ValidationResult::failure(vec![ValidationError {
                field: "method".to_string(),
                message: "Method field is required".to_string(),
                code: ValidationErrorCode::MissingRequired,
                expected: Some("method field".to_string()),
                actual: Some("missing".to_string()),
                suggestion: Some("Add method field to request".to_string()),
            }]));
        };

        // Validate against appropriate schema
        if let Some(schema) = self.request_schemas.get(method) {
            let params = data.get("params").unwrap_or(&Value::Null);
            let errors = self.validate_against_schema(params, schema, "params");

            let result = if errors.is_empty() {
                ValidationResult::success()
            } else {
                ValidationResult::failure(errors)
            };

            Ok(result
                .with_metadata("method", method)
                .with_metadata("schema_type", "request"))
        } else {
            warn!("No schema found for method: {}", method);
            Ok(ValidationResult::success()
                .with_metadata("method", method)
                .with_metadata("schema_found", false))
        }
    }

    async fn validate_response(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> Result<ValidationResult> {
        debug!(
            "Validating response schema for context: {}",
            context.request_id
        );

        // For responses, we need to get the method from context metadata
        let method = if let Some(method_value) = context.metadata.get("method") {
            if let Some(method_str) = method_value.as_str() {
                method_str
            } else {
                return Ok(ValidationResult::success()
                    .with_metadata("validation_skipped", "no_method_in_context"));
            }
        } else {
            return Ok(ValidationResult::success()
                .with_metadata("validation_skipped", "no_method_in_context"));
        };

        if let Some(schema) = self.response_schemas.get(method) {
            let result_data = data.get("result").unwrap_or(data);
            let errors = self.validate_against_schema(result_data, schema, "result");

            let result = if errors.is_empty() {
                ValidationResult::success()
            } else {
                ValidationResult::failure(errors)
            };

            Ok(result
                .with_metadata("method", method)
                .with_metadata("schema_type", "response"))
        } else {
            Ok(ValidationResult::success()
                .with_metadata("method", method)
                .with_metadata("schema_found", false))
        }
    }

    fn name(&self) -> &'static str {
        "SchemaValidator"
    }
}

/// JSON Schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    #[serde(rename = "type")]
    pub schema_type: SchemaType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_items: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
}

impl Default for Schema {
    fn default() -> Self {
        Self {
            schema_type: SchemaType::Object,
            properties: None,
            required: None,
            items: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            min_items: None,
            max_items: None,
            pattern: None,
            enum_values: None,
        }
    }
}

/// JSON Schema types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SchemaType {
    String,
    Integer,
    Number,
    Boolean,
    Array,
    Object,
    Null,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_schema_validator_creation() {
        let validator = SchemaValidator::new();
        assert!(validator.request_schemas.contains_key("initialize"));
        assert!(validator.request_schemas.contains_key("tools/call"));
    }

    #[tokio::test]
    async fn test_valid_tool_call_request() {
        let validator = SchemaValidator::new();
        let context = ValidationContext::new("test".to_string(), Default::default());

        let request = json!({
            "method": "tools/call",
            "params": {
                "name": "get_lights",
                "arguments": {
                    "room": "living_room"
                }
            }
        });

        let result = validator
            .validate_request(&request, &context)
            .await
            .unwrap();
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_invalid_tool_call_request() {
        let validator = SchemaValidator::new();
        let context = ValidationContext::new("test".to_string(), Default::default());

        let request = json!({
            "method": "tools/call",
            "params": {
                // Missing required "name" field
                "arguments": {}
            }
        });

        let result = validator
            .validate_request(&request, &context)
            .await
            .unwrap();
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
        assert_eq!(result.errors[0].code, ValidationErrorCode::MissingRequired);
    }

    #[tokio::test]
    async fn test_string_validation() {
        let validator = SchemaValidator::new();
        let schema = Schema {
            schema_type: SchemaType::String,
            min_length: Some(3),
            max_length: Some(10),
            pattern: Some(r"^[a-z]+$".to_string()),
            ..Default::default()
        };

        // Valid string
        let errors = validator.validate_against_schema(&json!("hello"), &schema, "test");
        assert!(errors.is_empty());

        // Too short
        let errors = validator.validate_against_schema(&json!("hi"), &schema, "test");
        assert!(!errors.is_empty());
        assert_eq!(errors[0].code, ValidationErrorCode::TooShort);

        // Too long
        let errors = validator.validate_against_schema(&json!("verylongstring"), &schema, "test");
        assert!(!errors.is_empty());
        assert_eq!(errors[0].code, ValidationErrorCode::TooLong);

        // Invalid pattern
        let errors = validator.validate_against_schema(&json!("Hello123"), &schema, "test");
        assert!(!errors.is_empty());
        assert_eq!(errors[0].code, ValidationErrorCode::PatternMismatch);
    }

    #[tokio::test]
    async fn test_malicious_content_detection() {
        let validator = SchemaValidator::new();
        let schema = Schema {
            schema_type: SchemaType::String,
            ..Default::default()
        };

        let malicious_content = json!("<script>alert('xss')</script>");
        let errors = validator.validate_against_schema(&malicious_content, &schema, "test");
        assert!(!errors.is_empty());
        assert_eq!(errors[0].code, ValidationErrorCode::MaliciousContent);
    }
}
