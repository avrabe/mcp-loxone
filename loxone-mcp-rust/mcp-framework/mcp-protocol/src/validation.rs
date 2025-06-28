//! Validation utilities for MCP protocol types

use crate::{Error, Result};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;
use validator::Validate;

/// Protocol validation utilities
pub struct Validator;

impl Validator {
    /// Validate a UUID string
    pub fn validate_uuid(uuid_str: &str) -> Result<Uuid> {
        uuid_str
            .parse::<Uuid>()
            .map_err(|e| Error::validation_error(format!("Invalid UUID: {}", e)))
    }

    /// Validate that a string is not empty
    pub fn validate_non_empty(value: &str, field_name: &str) -> Result<()> {
        if value.trim().is_empty() {
            Err(Error::validation_error(format!(
                "{} cannot be empty",
                field_name
            )))
        } else {
            Ok(())
        }
    }

    /// Validate a tool name (must be alphanumeric with underscores)
    pub fn validate_tool_name(name: &str) -> Result<()> {
        Self::validate_non_empty(name, "Tool name")?;

        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(Error::validation_error(
                "Tool name must contain only alphanumeric characters, underscores, and hyphens",
            ));
        }

        Ok(())
    }

    /// Validate a resource URI
    pub fn validate_resource_uri(uri: &str) -> Result<()> {
        Self::validate_non_empty(uri, "Resource URI")?;

        // Basic URI validation - must not contain control characters
        if uri.chars().any(|c| c.is_control()) {
            return Err(Error::validation_error(
                "Resource URI cannot contain control characters",
            ));
        }

        Ok(())
    }

    /// Validate JSON schema
    pub fn validate_json_schema(schema: &Value) -> Result<()> {
        // Basic validation - ensure it's an object with a "type" field
        if let Some(obj) = schema.as_object() {
            if !obj.contains_key("type") {
                return Err(Error::validation_error(
                    "JSON schema must have a 'type' field",
                ));
            }
        } else {
            return Err(Error::validation_error("JSON schema must be an object"));
        }

        Ok(())
    }

    /// Validate tool arguments against a schema
    pub fn validate_tool_arguments(args: &HashMap<String, Value>, schema: &Value) -> Result<()> {
        // Basic validation - check required properties if defined
        if let Some(schema_obj) = schema.as_object() {
            if let Some(_properties) = schema_obj.get("properties").and_then(|p| p.as_object()) {
                if let Some(required) = schema_obj.get("required").and_then(|r| r.as_array()) {
                    for req_field in required {
                        if let Some(field_name) = req_field.as_str() {
                            if !args.contains_key(field_name) {
                                return Err(Error::validation_error(format!(
                                    "Required argument '{}' is missing",
                                    field_name
                                )));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate pagination parameters
    pub fn validate_pagination(cursor: Option<&str>, limit: Option<u32>) -> Result<()> {
        if let Some(cursor_val) = cursor {
            Self::validate_non_empty(cursor_val, "Cursor")?;
        }

        if let Some(limit_val) = limit {
            if limit_val == 0 {
                return Err(Error::validation_error("Limit must be greater than 0"));
            }
            if limit_val > 1000 {
                return Err(Error::validation_error("Limit cannot exceed 1000"));
            }
        }

        Ok(())
    }

    /// Validate prompt name
    pub fn validate_prompt_name(name: &str) -> Result<()> {
        Self::validate_non_empty(name, "Prompt name")?;

        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(Error::validation_error(
                "Prompt name must contain only alphanumeric characters, underscores, hyphens, and dots"
            ));
        }

        Ok(())
    }

    /// Validate a struct using the validator crate
    pub fn validate_struct<T: Validate>(item: &T) -> Result<()> {
        item.validate()
            .map_err(|e| Error::validation_error(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_uuid() {
        let valid_uuid = "550e8400-e29b-41d4-a716-446655440000";
        assert!(Validator::validate_uuid(valid_uuid).is_ok());

        let invalid_uuid = "not-a-uuid";
        assert!(Validator::validate_uuid(invalid_uuid).is_err());
    }

    #[test]
    fn test_validate_non_empty() {
        assert!(Validator::validate_non_empty("valid", "field").is_ok());
        assert!(Validator::validate_non_empty("", "field").is_err());
        assert!(Validator::validate_non_empty("   ", "field").is_err());
    }

    #[test]
    fn test_validate_tool_name() {
        assert!(Validator::validate_tool_name("valid_tool").is_ok());
        assert!(Validator::validate_tool_name("tool-name").is_ok());
        assert!(Validator::validate_tool_name("tool123").is_ok());
        assert!(Validator::validate_tool_name("").is_err());
        assert!(Validator::validate_tool_name("invalid tool").is_err());
        assert!(Validator::validate_tool_name("tool@name").is_err());
    }

    #[test]
    fn test_validate_json_schema() {
        let valid_schema = json!({"type": "object"});
        assert!(Validator::validate_json_schema(&valid_schema).is_ok());

        let invalid_schema = json!("not an object");
        assert!(Validator::validate_json_schema(&invalid_schema).is_err());

        let no_type_schema = json!({"properties": {}});
        assert!(Validator::validate_json_schema(&no_type_schema).is_err());
    }

    #[test]
    fn test_validate_pagination() {
        assert!(Validator::validate_pagination(None, None).is_ok());
        assert!(Validator::validate_pagination(Some("cursor"), Some(10)).is_ok());
        assert!(Validator::validate_pagination(Some(""), None).is_err());
        assert!(Validator::validate_pagination(None, Some(0)).is_err());
        assert!(Validator::validate_pagination(None, Some(1001)).is_err());
    }
}
