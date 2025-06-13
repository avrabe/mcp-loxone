//! Schema validation with regex patterns and examples
//!
//! This module implements enhanced schema constraints for MCP tools with
//! regex patterns, examples, and comprehensive validation following MCP best practices.

use crate::error::{LoxoneError, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Schema constraint definition with regex patterns and examples
#[derive(Debug, Clone)]
pub struct SchemaConstraint {
    /// Field name
    pub field: String,
    /// Field type (string, number, boolean, array, object)
    pub field_type: String,
    /// Whether the field is required
    pub required: bool,
    /// Regex pattern for validation (optional)
    pub pattern: Option<Regex>,
    /// Human-readable pattern description
    pub pattern_description: Option<String>,
    /// Minimum length for strings/arrays
    pub min_length: Option<usize>,
    /// Maximum length for strings/arrays
    pub max_length: Option<usize>,
    /// Minimum value for numbers
    pub min_value: Option<f64>,
    /// Maximum value for numbers
    pub max_value: Option<f64>,
    /// Allowed enum values
    pub enum_values: Option<Vec<String>>,
    /// Example values for documentation
    pub examples: Vec<Value>,
    /// Default value (optional)
    pub default: Option<Value>,
}

impl SchemaConstraint {
    /// Create a new string constraint with regex pattern
    pub fn string_with_pattern<S: AsRef<str>>(
        field: S,
        pattern: S,
        description: S,
        required: bool,
    ) -> Result<Self> {
        let regex = Regex::new(pattern.as_ref())
            .map_err(|e| LoxoneError::config(format!("Invalid regex pattern: {}", e)))?;

        Ok(Self {
            field: field.as_ref().to_string(),
            field_type: "string".to_string(),
            required,
            pattern: Some(regex),
            pattern_description: Some(description.as_ref().to_string()),
            min_length: None,
            max_length: None,
            min_value: None,
            max_value: None,
            enum_values: None,
            examples: Vec::new(),
            default: None,
        })
    }

    /// Create a UUID constraint
    pub fn uuid<S: AsRef<str>>(field: S, required: bool) -> Result<Self> {
        Self::string_with_pattern(
            field.as_ref(),
            r"^([0-9a-fA-F]{8}[-.]?[0-9a-fA-F]{4}[-.]?[0-9a-fA-F]{4}[-.]?[0-9a-fA-F]{4}[-.]?[0-9a-fA-F]{12}|[0-9A-Fa-f]{8}\.[0-9A-Fa-f]{6}\.[A-Za-z0-9]+)$",
            "UUID format (e.g., 12345678-1234-1234-1234-123456789abc or 0CD8C06B.855703.I2)",
            required,
        )
    }

    /// Create a room name constraint
    pub fn room_name<S: AsRef<str>>(field: S, required: bool) -> Self {
        Self {
            field: field.as_ref().to_string(),
            field_type: "string".to_string(),
            required,
            pattern: None,
            pattern_description: Some("Room name (case-insensitive)".to_string()),
            min_length: Some(1),
            max_length: Some(100),
            min_value: None,
            max_value: None,
            enum_values: None,
            examples: vec![
                json!("Living Room"),
                json!("Kitchen"),
                json!("Bedroom 1"),
                json!("Office"),
            ],
            default: None,
        }
    }

    /// Create a device action constraint
    pub fn device_action<S: AsRef<str>>(field: S, required: bool) -> Self {
        Self {
            field: field.as_ref().to_string(),
            field_type: "string".to_string(),
            required,
            pattern: None,
            pattern_description: Some("Device action command".to_string()),
            min_length: Some(1),
            max_length: Some(50),
            min_value: None,
            max_value: None,
            enum_values: Some(vec![
                "on".to_string(),
                "off".to_string(),
                "toggle".to_string(),
                "pulse".to_string(),
                "up".to_string(),
                "down".to_string(),
                "stop".to_string(),
                "fullup".to_string(),
                "fulldown".to_string(),
            ]),
            examples: vec![
                json!("on"),
                json!("off"),
                json!("toggle"),
                json!("up"),
                json!("down"),
                json!("stop"),
            ],
            default: None,
        }
    }

    /// Create a temperature value constraint
    pub fn temperature<S: AsRef<str>>(field: S, required: bool) -> Self {
        Self {
            field: field.as_ref().to_string(),
            field_type: "number".to_string(),
            required,
            pattern: None,
            pattern_description: Some("Temperature in Celsius".to_string()),
            min_length: None,
            max_length: None,
            min_value: Some(-50.0),
            max_value: Some(100.0),
            enum_values: None,
            examples: vec![json!(20.5), json!(22.0), json!(18.5), json!(24.0)],
            default: None,
        }
    }

    /// Create a percentage constraint (0-100)
    pub fn percentage<S: AsRef<str>>(field: S, required: bool) -> Self {
        Self {
            field: field.as_ref().to_string(),
            field_type: "number".to_string(),
            required,
            pattern: None,
            pattern_description: Some("Percentage value (0-100)".to_string()),
            min_length: None,
            max_length: None,
            min_value: Some(0.0),
            max_value: Some(100.0),
            enum_values: None,
            examples: vec![json!(0), json!(25), json!(50), json!(75), json!(100)],
            default: None,
        }
    }

    /// Create a boolean constraint
    pub fn boolean<S: AsRef<str>>(field: S, required: bool) -> Self {
        Self {
            field: field.as_ref().to_string(),
            field_type: "boolean".to_string(),
            required,
            pattern: None,
            pattern_description: Some("Boolean value".to_string()),
            min_length: None,
            max_length: None,
            min_value: None,
            max_value: None,
            enum_values: None,
            examples: vec![json!(true), json!(false)],
            default: None,
        }
    }

    /// Add examples to the constraint
    pub fn with_examples(mut self, examples: Vec<Value>) -> Self {
        self.examples = examples;
        self
    }

    /// Add a default value
    pub fn with_default(mut self, default: Value) -> Self {
        self.default = Some(default);
        self
    }

    /// Validate a value against this constraint
    pub fn validate(&self, value: &Value) -> Result<()> {
        debug!("Validating field '{}' with value: {:?}", self.field, value);

        // Check if value is null and field is required
        if value.is_null() {
            if self.required {
                return Err(LoxoneError::invalid_input(format!(
                    "Field '{}' is required but was null",
                    self.field
                )));
            } else {
                return Ok(()); // null is allowed for optional fields
            }
        }

        // Type validation
        match self.field_type.as_str() {
            "string" => {
                if !value.is_string() {
                    return Err(LoxoneError::invalid_input(format!(
                        "Field '{}' must be a string, got: {:?}",
                        self.field, value
                    )));
                }

                let str_value = value.as_str().unwrap();

                // Length validation
                if let Some(min_len) = self.min_length {
                    if str_value.len() < min_len {
                        return Err(LoxoneError::invalid_input(format!(
                            "Field '{}' must be at least {} characters long",
                            self.field, min_len
                        )));
                    }
                }

                if let Some(max_len) = self.max_length {
                    if str_value.len() > max_len {
                        return Err(LoxoneError::invalid_input(format!(
                            "Field '{}' must be at most {} characters long",
                            self.field, max_len
                        )));
                    }
                }

                // Pattern validation
                if let Some(ref pattern) = self.pattern {
                    if !pattern.is_match(str_value) {
                        let description = self
                            .pattern_description
                            .as_deref()
                            .unwrap_or("valid format");
                        return Err(LoxoneError::invalid_input(format!(
                            "Field '{}' must match {}: '{}'",
                            self.field, description, str_value
                        )));
                    }
                }

                // Enum validation
                if let Some(ref enum_values) = self.enum_values {
                    if !enum_values.contains(&str_value.to_string()) {
                        return Err(LoxoneError::invalid_input(format!(
                            "Field '{}' must be one of: {}. Got: '{}'",
                            self.field,
                            enum_values.join(", "),
                            str_value
                        )));
                    }
                }
            }

            "number" => {
                if !value.is_number() {
                    return Err(LoxoneError::invalid_input(format!(
                        "Field '{}' must be a number, got: {:?}",
                        self.field, value
                    )));
                }

                let num_value = value.as_f64().unwrap();

                // Range validation
                if let Some(min_val) = self.min_value {
                    if num_value < min_val {
                        return Err(LoxoneError::invalid_input(format!(
                            "Field '{}' must be at least {}, got: {}",
                            self.field, min_val, num_value
                        )));
                    }
                }

                if let Some(max_val) = self.max_value {
                    if num_value > max_val {
                        return Err(LoxoneError::invalid_input(format!(
                            "Field '{}' must be at most {}, got: {}",
                            self.field, max_val, num_value
                        )));
                    }
                }
            }

            "boolean" => {
                if !value.is_boolean() {
                    return Err(LoxoneError::invalid_input(format!(
                        "Field '{}' must be a boolean, got: {:?}",
                        self.field, value
                    )));
                }
            }

            "array" => {
                if !value.is_array() {
                    return Err(LoxoneError::invalid_input(format!(
                        "Field '{}' must be an array, got: {:?}",
                        self.field, value
                    )));
                }

                let array = value.as_array().unwrap();

                // Length validation for arrays
                if let Some(min_len) = self.min_length {
                    if array.len() < min_len {
                        return Err(LoxoneError::invalid_input(format!(
                            "Field '{}' array must have at least {} items",
                            self.field, min_len
                        )));
                    }
                }

                if let Some(max_len) = self.max_length {
                    if array.len() > max_len {
                        return Err(LoxoneError::invalid_input(format!(
                            "Field '{}' array must have at most {} items",
                            self.field, max_len
                        )));
                    }
                }
            }

            "object" => {
                if !value.is_object() {
                    return Err(LoxoneError::invalid_input(format!(
                        "Field '{}' must be an object, got: {:?}",
                        self.field, value
                    )));
                }
            }

            _ => {
                warn!(
                    "Unknown field type '{}' for field '{}'",
                    self.field_type, self.field
                );
            }
        }

        Ok(())
    }

    /// Generate JSON schema representation
    pub fn to_json_schema(&self) -> Value {
        let mut schema = json!({
            "type": self.field_type,
            "description": self.pattern_description.clone().unwrap_or_else(||
                format!("Value for field '{}'", self.field))
        });

        // Add pattern if present
        if let Some(ref pattern) = self.pattern {
            schema["pattern"] = json!(pattern.as_str());
        }

        // Add string constraints
        if let Some(min_len) = self.min_length {
            if self.field_type == "string" || self.field_type == "array" {
                schema["minLength"] = json!(min_len);
            }
        }

        if let Some(max_len) = self.max_length {
            if self.field_type == "string" || self.field_type == "array" {
                schema["maxLength"] = json!(max_len);
            }
        }

        // Add number constraints
        if let Some(min_val) = self.min_value {
            schema["minimum"] = json!(min_val);
        }

        if let Some(max_val) = self.max_value {
            schema["maximum"] = json!(max_val);
        }

        // Add enum values
        if let Some(ref enum_values) = self.enum_values {
            schema["enum"] = json!(enum_values);
        }

        // Add examples
        if !self.examples.is_empty() {
            schema["examples"] = json!(self.examples);
        }

        // Add default value
        if let Some(ref default) = self.default {
            schema["default"] = default.clone();
        }

        schema
    }
}

/// Schema validator for MCP tool parameters
#[derive(Debug)]
pub struct SchemaValidator {
    constraints: HashMap<String, Vec<SchemaConstraint>>,
}

impl SchemaValidator {
    /// Create a new schema validator
    pub fn new() -> Self {
        let mut validator = Self {
            constraints: HashMap::new(),
        };

        // Initialize with standard tool schemas
        validator.init_standard_schemas();
        validator
    }

    /// Add constraints for a tool
    pub fn add_tool_constraints<S: AsRef<str>>(
        &mut self,
        tool_name: S,
        constraints: Vec<SchemaConstraint>,
    ) {
        self.constraints
            .insert(tool_name.as_ref().to_string(), constraints);
    }

    /// Validate parameters for a tool
    pub fn validate_tool_parameters<S: AsRef<str>>(
        &self,
        tool_name: S,
        parameters: &Value,
    ) -> Result<()> {
        let tool_name_str = tool_name.as_ref();
        debug!("Validating parameters for tool: {}", tool_name_str);

        let constraints = match self.constraints.get(tool_name_str) {
            Some(constraints) => constraints,
            None => {
                debug!("No constraints found for tool: {}", tool_name_str);
                return Ok(());
            }
        };

        let params_obj = match parameters.as_object() {
            Some(obj) => obj,
            None => {
                return Err(LoxoneError::invalid_input(format!(
                    "Tool '{}' parameters must be an object",
                    tool_name_str
                )));
            }
        };

        // Validate each constraint
        for constraint in constraints {
            let field_value = params_obj.get(&constraint.field).unwrap_or(&Value::Null);

            if let Err(e) = constraint.validate(field_value) {
                return Err(LoxoneError::invalid_input(format!(
                    "Tool '{}': {}",
                    tool_name_str, e
                )));
            }
        }

        // Check for unknown fields (warn only)
        for field_name in params_obj.keys() {
            let known_field = constraints.iter().any(|c| c.field == *field_name);
            if !known_field {
                warn!(
                    "Unknown parameter '{}' for tool '{}'",
                    field_name, tool_name_str
                );
            }
        }

        Ok(())
    }

    /// Get JSON schema for a tool
    pub fn get_tool_schema<S: AsRef<str>>(&self, tool_name: S) -> Option<Value> {
        let constraints = self.constraints.get(tool_name.as_ref())?;

        let mut properties = json!({});
        let mut required = Vec::new();

        for constraint in constraints {
            properties[&constraint.field] = constraint.to_json_schema();
            if constraint.required {
                required.push(&constraint.field);
            }
        }

        Some(json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        }))
    }

    /// Initialize standard schemas for common tools
    fn init_standard_schemas(&mut self) {
        // Device control schemas
        self.add_tool_constraints(
            "control_device",
            vec![
                SchemaConstraint::uuid("uuid", true)
                    .unwrap()
                    .with_examples(vec![
                        json!("12345678-1234-1234-1234-123456789abc"),
                        json!("0CD8C06B.855703.I2"),
                    ]),
                SchemaConstraint::device_action("action", true),
            ],
        );

        self.add_tool_constraints(
            "get_device_state",
            vec![SchemaConstraint::uuid("uuid", true)
                .unwrap()
                .with_examples(vec![
                    json!("12345678-1234-1234-1234-123456789abc"),
                    json!("0CD8C06B.855703.I2"),
                ])],
        );

        // Room schemas
        self.add_tool_constraints(
            "get_room_devices",
            vec![SchemaConstraint::room_name("room", true)],
        );

        self.add_tool_constraints(
            "control_room_devices",
            vec![
                SchemaConstraint::room_name("room", true),
                SchemaConstraint::device_action("action", true),
                SchemaConstraint::string_with_pattern(
                    "device_type",
                    r"^(light|blind|rolladen|jalousie|all)$",
                    "Device type filter",
                    false,
                )
                .unwrap()
                .with_examples(vec![
                    json!("light"),
                    json!("blind"),
                    json!("rolladen"),
                    json!("all"),
                ])
                .with_default(json!("all")),
            ],
        );

        // Climate control schemas
        self.add_tool_constraints(
            "set_room_temperature",
            vec![
                SchemaConstraint::room_name("room", true),
                SchemaConstraint::temperature("temperature", true),
            ],
        );

        // Sensor schemas
        self.add_tool_constraints(
            "get_sensor_reading",
            vec![SchemaConstraint::uuid("sensor_uuid", true)
                .unwrap()
                .with_examples(vec![
                    json!("0CD8C06B.855703.A1"),
                    json!("12345678-1234-1234-1234-123456789abc"),
                ])],
        );

        // Light control schemas
        self.add_tool_constraints(
            "control_light",
            vec![
                SchemaConstraint::uuid("uuid", true).unwrap(),
                SchemaConstraint::device_action("action", true),
                SchemaConstraint::percentage("brightness", false).with_examples(vec![
                    json!(0),
                    json!(25),
                    json!(50),
                    json!(75),
                    json!(100),
                ]),
            ],
        );

        // Blind control schemas
        self.add_tool_constraints(
            "control_blind",
            vec![
                SchemaConstraint::uuid("uuid", true).unwrap(),
                SchemaConstraint::device_action("action", true),
                SchemaConstraint::percentage("position", false).with_examples(vec![
                    json!(0),
                    json!(25),
                    json!(50),
                    json!(75),
                    json!(100),
                ]),
            ],
        );

        // Weather schemas
        self.add_tool_constraints(
            "get_weather_data",
            vec![SchemaConstraint::boolean("include_forecast", false).with_default(json!(false))],
        );

        // Security schemas
        self.add_tool_constraints(
            "get_security_status",
            vec![SchemaConstraint::string_with_pattern(
                "zone",
                r"^[a-zA-Z0-9_-]+$",
                "Security zone identifier",
                false,
            )
            .unwrap()
            .with_examples(vec![json!("main"), json!("perimeter"), json!("internal")])],
        );

        // Additional MCP tool schemas
        self.add_tool_constraints("list_rooms", vec![]); // No parameters

        self.add_tool_constraints(
            "control_all_rolladen",
            vec![
                SchemaConstraint::device_action("action", true).with_examples(vec![
                    json!("up"),
                    json!("down"),
                    json!("stop"),
                ]),
            ],
        );

        self.add_tool_constraints(
            "control_room_rolladen",
            vec![
                SchemaConstraint::room_name("room", true),
                SchemaConstraint::device_action("action", true).with_examples(vec![
                    json!("up"),
                    json!("down"),
                    json!("stop"),
                ]),
            ],
        );

        self.add_tool_constraints(
            "control_all_lights",
            vec![SchemaConstraint::device_action("action", true)
                .with_examples(vec![json!("on"), json!("off")])],
        );

        self.add_tool_constraints(
            "control_room_lights",
            vec![
                SchemaConstraint::room_name("room", true),
                SchemaConstraint::device_action("action", true)
                    .with_examples(vec![json!("on"), json!("off")]),
            ],
        );

        self.add_tool_constraints(
            "get_device_info",
            vec![
                SchemaConstraint::string_with_pattern(
                    "device",
                    r"^.+$",
                    "Device UUID or name",
                    true,
                )
                .unwrap()
                .with_examples(vec![
                    json!("12345678-1234-1234-1234-123456789abc"),
                    json!("Living Room Light"),
                ]),
                SchemaConstraint::room_name("room", false),
            ],
        );

        self.add_tool_constraints("get_system_info", vec![]); // No parameters

        self.add_tool_constraints("health_check", vec![]); // No parameters

        self.add_tool_constraints("get_health_status", vec![]); // No parameters
    }

    /// Get all available tool schemas
    pub fn get_all_schemas(&self) -> HashMap<String, Value> {
        let mut schemas = HashMap::new();
        for tool_name in self.constraints.keys() {
            if let Some(schema) = self.get_tool_schema(tool_name) {
                schemas.insert(tool_name.clone(), schema);
            }
        }
        schemas
    }

    /// Validate and apply defaults to parameters
    pub fn validate_and_apply_defaults<S: AsRef<str>>(
        &self,
        tool_name: S,
        parameters: &mut Value,
    ) -> Result<()> {
        let tool_name_str = tool_name.as_ref();

        // First validate
        self.validate_tool_parameters(tool_name_str, parameters)?;

        // Then apply defaults
        if let Some(constraints) = self.constraints.get(tool_name_str) {
            if let Some(params_obj) = parameters.as_object_mut() {
                for constraint in constraints {
                    if !params_obj.contains_key(&constraint.field) {
                        if let Some(ref default_value) = constraint.default {
                            params_obj.insert(constraint.field.clone(), default_value.clone());
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid_constraint() {
        let constraint = SchemaConstraint::uuid("test_uuid", true).unwrap();

        // Valid UUIDs
        assert!(constraint
            .validate(&json!("12345678-1234-1234-1234-123456789abc"))
            .is_ok());
        assert!(constraint.validate(&json!("0CD8C06B.855703.I2")).is_ok());

        // Invalid UUIDs
        assert!(constraint.validate(&json!("invalid-uuid")).is_err());
        assert!(constraint.validate(&json!("12345")).is_err());
        assert!(constraint.validate(&json!(123)).is_err());
    }

    #[test]
    fn test_device_action_constraint() {
        let constraint = SchemaConstraint::device_action("action", true);

        // Valid actions
        assert!(constraint.validate(&json!("on")).is_ok());
        assert!(constraint.validate(&json!("off")).is_ok());
        assert!(constraint.validate(&json!("toggle")).is_ok());

        // Invalid actions
        assert!(constraint.validate(&json!("invalid_action")).is_err());
        assert!(constraint.validate(&json!(123)).is_err());
    }

    #[test]
    fn test_temperature_constraint() {
        let constraint = SchemaConstraint::temperature("temp", true);

        // Valid temperatures
        assert!(constraint.validate(&json!(20.5)).is_ok());
        assert!(constraint.validate(&json!(0)).is_ok());
        assert!(constraint.validate(&json!(-10.0)).is_ok());

        // Invalid temperatures
        assert!(constraint.validate(&json!(-100.0)).is_err()); // Too cold
        assert!(constraint.validate(&json!(150.0)).is_err()); // Too hot
        assert!(constraint.validate(&json!("20")).is_err()); // Wrong type
    }

    #[test]
    fn test_schema_validator() {
        let validator = SchemaValidator::new();

        // Valid device control
        let params = json!({
            "uuid": "12345678-1234-1234-1234-123456789abc",
            "action": "on"
        });
        assert!(validator
            .validate_tool_parameters("control_device", &params)
            .is_ok());

        // Invalid device control (missing UUID)
        let invalid_params = json!({
            "action": "on"
        });
        assert!(validator
            .validate_tool_parameters("control_device", &invalid_params)
            .is_err());

        // Invalid device control (bad action)
        let invalid_params2 = json!({
            "uuid": "12345678-1234-1234-1234-123456789abc",
            "action": "invalid_action"
        });
        assert!(validator
            .validate_tool_parameters("control_device", &invalid_params2)
            .is_err());
    }

    #[test]
    fn test_schema_generation() {
        let validator = SchemaValidator::new();
        let schema = validator.get_tool_schema("control_device").unwrap();

        assert!(schema["type"] == "object");
        assert!(schema["properties"]["uuid"]["type"] == "string");
        assert!(schema["properties"]["action"]["enum"].is_array());
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("uuid")));
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("action")));
    }

    #[test]
    fn test_defaults_application() {
        let mut validator = SchemaValidator::new();

        // Add a constraint with default
        validator.add_tool_constraints(
            "test_tool",
            vec![
                SchemaConstraint::string_with_pattern("param1", ".*", "Any string", true).unwrap(),
                SchemaConstraint::boolean("param2", false).with_default(json!(true)),
            ],
        );

        let mut params = json!({
            "param1": "test_value"
        });

        validator
            .validate_and_apply_defaults("test_tool", &mut params)
            .unwrap();

        // Default should be applied
        assert_eq!(params["param2"], json!(true));
    }
}
