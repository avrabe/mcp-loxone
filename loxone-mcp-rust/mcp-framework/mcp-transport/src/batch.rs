//! JSON-RPC batch message handling

use crate::{validation::validate_batch, RequestHandler, TransportError};
use pulseengine_mcp_protocol::{Request, Response};
use serde_json::Value;
use tracing::debug;

/// Represents a JSON-RPC message that can be either single or batch
#[derive(Debug, Clone)]
pub enum JsonRpcMessage {
    Single(Value),
    Batch(Vec<Value>),
}

/// Represents a processed batch result
#[derive(Debug)]
pub struct BatchResult {
    pub responses: Vec<Response>,
    pub has_notifications: bool,
}

impl JsonRpcMessage {
    /// Parse a JSON string into a JsonRpcMessage
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        let value: Value = serde_json::from_str(text)?;

        if let Some(array) = value.as_array() {
            Ok(JsonRpcMessage::Batch(array.clone()))
        } else {
            Ok(JsonRpcMessage::Single(value))
        }
    }

    /// Convert to JSON string
    pub fn to_string(&self) -> Result<String, serde_json::Error> {
        match self {
            JsonRpcMessage::Single(value) => serde_json::to_string(value),
            JsonRpcMessage::Batch(values) => serde_json::to_string(values),
        }
    }

    /// Validate the message according to JSON-RPC and MCP specs
    pub fn validate(&self) -> Result<(), TransportError> {
        match self {
            JsonRpcMessage::Single(value) => {
                crate::validation::validate_jsonrpc_message(value)
                    .map_err(|e| TransportError::Protocol(e.to_string()))?;
                Ok(())
            }
            JsonRpcMessage::Batch(values) => {
                if values.is_empty() {
                    return Err(TransportError::Protocol(
                        "Batch cannot be empty".to_string(),
                    ));
                }

                validate_batch(values).map_err(|e| TransportError::Protocol(e.to_string()))?;
                Ok(())
            }
        }
    }

    /// Extract requests from the message (filtering out notifications)
    pub fn extract_requests(&self) -> Result<Vec<Request>, TransportError> {
        let mut requests = Vec::new();

        match self {
            JsonRpcMessage::Single(value) => {
                if let Ok(request) = serde_json::from_value::<Request>(value.clone()) {
                    // Only include if it has an ID (requests, not notifications)
                    if !request.id.is_null() {
                        requests.push(request);
                    }
                }
            }
            JsonRpcMessage::Batch(values) => {
                for value in values {
                    if let Ok(request) = serde_json::from_value::<Request>(value.clone()) {
                        // Only include if it has an ID (requests, not notifications)
                        if !request.id.is_null() {
                            requests.push(request);
                        }
                    }
                }
            }
        }

        Ok(requests)
    }

    /// Extract notifications from the message
    pub fn extract_notifications(&self) -> Result<Vec<Request>, TransportError> {
        let mut notifications = Vec::new();

        match self {
            JsonRpcMessage::Single(value) => {
                if let Ok(request) = serde_json::from_value::<Request>(value.clone()) {
                    // Only include if it doesn't have an ID (notifications)
                    if request.id.is_null() {
                        notifications.push(request);
                    }
                }
            }
            JsonRpcMessage::Batch(values) => {
                for value in values {
                    if let Ok(request) = serde_json::from_value::<Request>(value.clone()) {
                        // Only include if it doesn't have an ID (notifications)
                        if request.id.is_null() {
                            notifications.push(request);
                        }
                    }
                }
            }
        }

        Ok(notifications)
    }

    /// Check if this message contains any requests (vs only notifications)
    pub fn has_requests(&self) -> bool {
        match self {
            JsonRpcMessage::Single(value) => {
                if let Ok(request) = serde_json::from_value::<Request>(value.clone()) {
                    !request.id.is_null()
                } else {
                    false
                }
            }
            JsonRpcMessage::Batch(values) => values.iter().any(|value| {
                if let Ok(request) = serde_json::from_value::<Request>(value.clone()) {
                    !request.id.is_null()
                } else {
                    false
                }
            }),
        }
    }
}

/// Process a batch of requests through a handler
pub async fn process_batch(
    message: JsonRpcMessage,
    handler: &RequestHandler,
) -> Result<Option<JsonRpcMessage>, TransportError> {
    debug!("Processing batch message");

    // Validate the message first
    message.validate()?;

    // Extract requests and notifications
    let requests = message.extract_requests()?;
    let notifications = message.extract_notifications()?;

    debug!(
        "Batch contains {} requests and {} notifications",
        requests.len(),
        notifications.len()
    );

    // Process notifications (no response expected)
    for notification in notifications {
        debug!("Processing notification: {}", notification.method);
        let _response = handler(notification).await;
        // Notifications don't generate responses, so we ignore the result
    }

    // If no requests, return None (no response needed)
    if requests.is_empty() {
        return Ok(None);
    }

    // Process requests and collect responses
    let mut responses = Vec::new();

    for request in requests {
        debug!(
            "Processing request: {} (ID: {})",
            request.method, request.id
        );
        let response = handler(request).await;
        responses.push(response);
    }

    // Return appropriate response format
    let response_message = if responses.len() == 1 && !matches!(message, JsonRpcMessage::Batch(_)) {
        // Single request, single response
        let response_value = serde_json::to_value(&responses[0])
            .map_err(|e| TransportError::Protocol(format!("Failed to serialize response: {e}")))?;
        JsonRpcMessage::Single(response_value)
    } else {
        // Batch response
        let response_values: Result<Vec<Value>, _> =
            responses.iter().map(serde_json::to_value).collect();

        let response_values = response_values.map_err(|e| {
            TransportError::Protocol(format!("Failed to serialize batch response: {e}"))
        })?;

        JsonRpcMessage::Batch(response_values)
    };

    Ok(Some(response_message))
}

/// Create an error response for a malformed request
pub fn create_error_response(
    error: pulseengine_mcp_protocol::Error,
    request_id: Value,
) -> Response {
    Response {
        jsonrpc: "2.0".to_string(),
        id: request_id,
        result: None,
        error: Some(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulseengine_mcp_protocol::{Error as McpError, Request, Response};
    use serde_json::json;

    // Mock handler for testing
    fn mock_handler(
        request: Request,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> {
        Box::pin(async move {
            Response {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(json!({"method": request.method})),
                error: None,
            }
        })
    }

    #[test]
    fn test_jsonrpc_message_parsing() {
        // Single message
        let single_json = r#"{"jsonrpc": "2.0", "method": "test", "id": 1}"#;
        let single_msg = JsonRpcMessage::parse(single_json).unwrap();
        assert!(matches!(single_msg, JsonRpcMessage::Single(_)));

        // Batch message
        let batch_json = r#"[{"jsonrpc": "2.0", "method": "test1", "id": 1}, {"jsonrpc": "2.0", "method": "test2"}]"#;
        let batch_msg = JsonRpcMessage::parse(batch_json).unwrap();
        assert!(matches!(batch_msg, JsonRpcMessage::Batch(_)));
    }

    #[test]
    fn test_extract_requests_and_notifications() {
        let batch_json = r#"[
            {"jsonrpc": "2.0", "method": "request1", "id": 1},
            {"jsonrpc": "2.0", "method": "notification1"},
            {"jsonrpc": "2.0", "method": "request2", "id": 2}
        ]"#;

        let message = JsonRpcMessage::parse(batch_json).unwrap();

        let requests = message.extract_requests().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method, "request1");
        assert_eq!(requests[1].method, "request2");

        let notifications = message.extract_notifications().unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].method, "notification1");
    }

    #[tokio::test]
    async fn test_process_batch() {
        let handler: RequestHandler = Box::new(mock_handler);

        // Test single request
        let single_json = r#"{"jsonrpc": "2.0", "method": "test", "id": 1}"#;
        let single_msg = JsonRpcMessage::parse(single_json).unwrap();

        let result = process_batch(single_msg, &handler).await.unwrap();
        assert!(result.is_some());

        // Test notification only (should return None)
        let notification_json = r#"{"jsonrpc": "2.0", "method": "test"}"#;
        let notification_msg = JsonRpcMessage::parse(notification_json).unwrap();

        let result = process_batch(notification_msg, &handler).await.unwrap();
        assert!(result.is_none());

        // Test batch with mixed requests and notifications
        let batch_json = r#"[
            {"jsonrpc": "2.0", "method": "request1", "id": 1},
            {"jsonrpc": "2.0", "method": "notification1"},
            {"jsonrpc": "2.0", "method": "request2", "id": 2}
        ]"#;
        let batch_msg = JsonRpcMessage::parse(batch_json).unwrap();

        let result = process_batch(batch_msg, &handler).await.unwrap();
        assert!(result.is_some());

        if let Some(JsonRpcMessage::Batch(responses)) = result {
            assert_eq!(responses.len(), 2); // Only requests generate responses
        } else {
            panic!("Expected batch response");
        }
    }

    #[test]
    fn test_create_error_response() {
        let error = McpError::parse_error("Test error");
        let response = create_error_response(error, json!(123));

        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, json!(123));
        assert!(response.result.is_none());
        assert!(response.error.is_some());
    }
}
