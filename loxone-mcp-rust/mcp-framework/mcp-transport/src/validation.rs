//! Message validation utilities for MCP transport compliance

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Message contains embedded newlines")]
    EmbeddedNewlines,
    
    #[error("Message is not valid UTF-8: {0}")]
    InvalidUtf8(String),
    
    #[error("Request ID cannot be null")]
    NullRequestId,
    
    #[error("Notification cannot have an ID")]
    NotificationWithId,
    
    #[error("Message exceeds maximum size: {size} > {max}")]
    MessageTooLarge { size: usize, max: usize },
    
    #[error("Invalid JSON-RPC format: {0}")]
    InvalidFormat(String),
}

/// Validates a raw message string for MCP compliance
pub fn validate_message_string(message: &str, max_size: Option<usize>) -> Result<(), ValidationError> {
    // Check for embedded newlines (MCP spec requirement)
    if message.contains('\n') || message.contains('\r') {
        return Err(ValidationError::EmbeddedNewlines);
    }
    
    // Check message size limit
    if let Some(max) = max_size {
        if message.len() > max {
            return Err(ValidationError::MessageTooLarge {
                size: message.len(),
                max,
            });
        }
    }
    
    // UTF-8 validation is implicit in Rust strings, but we validate the bytes
    if !message.is_ascii() {
        // For non-ASCII, ensure it's valid UTF-8 by checking byte validity
        if let Err(e) = std::str::from_utf8(message.as_bytes()) {
            return Err(ValidationError::InvalidUtf8(e.to_string()));
        }
    }
    
    Ok(())
}

/// Validates JSON-RPC message structure and ID requirements
pub fn validate_jsonrpc_message(value: &Value) -> Result<MessageType, ValidationError> {
    let obj = value.as_object()
        .ok_or_else(|| ValidationError::InvalidFormat("Message must be a JSON object".to_string()))?;
    
    // Check for required jsonrpc field
    if obj.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
        return Err(ValidationError::InvalidFormat("Missing or invalid jsonrpc field".to_string()));
    }
    
    // Determine message type and validate ID requirements
    if obj.contains_key("method") {
        // This is a request or notification
        let has_id = obj.contains_key("id");
        let id_value = obj.get("id");
        
        if has_id {
            // Request: ID cannot be null
            if id_value == Some(&Value::Null) {
                return Err(ValidationError::NullRequestId);
            }
            Ok(MessageType::Request)
        } else {
            // Notification: should not have ID
            Ok(MessageType::Notification)
        }
    } else if obj.contains_key("result") || obj.contains_key("error") {
        // Response: must have ID
        if !obj.contains_key("id") {
            return Err(ValidationError::InvalidFormat("Response must have an ID".to_string()));
        }
        Ok(MessageType::Response)
    } else {
        Err(ValidationError::InvalidFormat("Unknown message type".to_string()))
    }
}

/// Attempts to extract ID from a malformed JSON request for error responses
pub fn extract_id_from_malformed(text: &str) -> Value {
    // Try to parse as JSON object and extract ID
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        if let Some(obj) = value.as_object() {
            if let Some(id) = obj.get("id") {
                return id.clone();
            }
        }
    }
    
    // Try regex-based extraction as fallback
    if let Some(id_match) = extract_id_with_regex(text) {
        return id_match;
    }
    
    // Default to null if we can't extract
    Value::Null
}

/// Validates a batch of JSON-RPC messages
pub fn validate_batch(batch: &[Value]) -> Result<Vec<MessageType>, ValidationError> {
    if batch.is_empty() {
        return Err(ValidationError::InvalidFormat("Batch cannot be empty".to_string()));
    }
    
    let mut types = Vec::new();
    for message in batch {
        types.push(validate_jsonrpc_message(message)?);
    }
    
    Ok(types)
}

/// JSON-RPC message types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageType {
    Request,
    Response,
    Notification,
}

/// Regex-based ID extraction for malformed JSON (fallback)
fn extract_id_with_regex(text: &str) -> Option<Value> {
    use regex::Regex;
    
    // Try to match common ID patterns
    let patterns = [
        r#""id"\s*:\s*"([^"]+)""#,  // String ID
        r#""id"\s*:\s*(\d+)"#,      // Number ID
        r#""id"\s*:\s*(null)"#,     // Null ID
    ];
    
    for pattern in &patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(captures) = re.captures(text) {
                if let Some(id_str) = captures.get(1) {
                    let id_text = id_str.as_str();
                    
                    // Try to parse as number first
                    if let Ok(num) = id_text.parse::<i64>() {
                        return Some(Value::Number(num.into()));
                    }
                    
                    // Check for null
                    if id_text == "null" {
                        return Some(Value::Null);
                    }
                    
                    // Default to string
                    return Some(Value::String(id_text.to_string()));
                }
            }
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_message_string() {
        // Valid message
        assert!(validate_message_string("hello world", None).is_ok());
        
        // Invalid: embedded newline
        assert!(matches!(
            validate_message_string("hello\nworld", None),
            Err(ValidationError::EmbeddedNewlines)
        ));
        
        // Invalid: embedded carriage return
        assert!(matches!(
            validate_message_string("hello\rworld", None),
            Err(ValidationError::EmbeddedNewlines)
        ));
        
        // Invalid: too large
        assert!(matches!(
            validate_message_string("hello world", Some(5)),
            Err(ValidationError::MessageTooLarge { .. })
        ));
    }

    #[test]
    fn test_validate_jsonrpc_message() {
        // Valid request
        let request = json!({
            "jsonrpc": "2.0",
            "method": "test",
            "id": 1
        });
        assert_eq!(validate_jsonrpc_message(&request).unwrap(), MessageType::Request);
        
        // Valid notification
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "test"
        });
        assert_eq!(validate_jsonrpc_message(&notification).unwrap(), MessageType::Notification);
        
        // Valid response
        let response = json!({
            "jsonrpc": "2.0",
            "result": "ok",
            "id": 1
        });
        assert_eq!(validate_jsonrpc_message(&response).unwrap(), MessageType::Response);
        
        // Invalid: request with null ID
        let invalid_request = json!({
            "jsonrpc": "2.0",
            "method": "test",
            "id": null
        });
        assert!(matches!(
            validate_jsonrpc_message(&invalid_request),
            Err(ValidationError::NullRequestId)
        ));
    }

    #[test]
    fn test_extract_id_from_malformed() {
        // Valid JSON with extractable ID
        let text = r#"{"jsonrpc": "2.0", "method": "test", "id": 123}"#;
        assert_eq!(extract_id_from_malformed(text), json!(123));
        
        // Invalid JSON but regex can extract
        let text = r#"{"jsonrpc": "2.0", "method": "test", "id": "abc""#; // Missing closing brace
        assert_eq!(extract_id_from_malformed(text), json!("abc"));
        
        // No ID extractable
        let text = r#"{"jsonrpc": "2.0", "method": "test"}"#;
        assert_eq!(extract_id_from_malformed(text), Value::Null);
    }

    #[test]
    fn test_validate_batch() {
        let batch = vec![
            json!({"jsonrpc": "2.0", "method": "test1", "id": 1}),
            json!({"jsonrpc": "2.0", "method": "test2"}),
        ];
        
        let types = validate_batch(&batch).unwrap();
        assert_eq!(types, vec![MessageType::Request, MessageType::Notification]);
        
        // Empty batch should fail
        assert!(validate_batch(&[]).is_err());
    }
}