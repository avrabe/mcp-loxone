//! MCP-compliant Standard I/O transport implementation

use crate::{
    batch::{create_error_response, process_batch, JsonRpcMessage},
    validation::{extract_id_from_malformed, validate_message_string},
    RequestHandler, Transport, TransportError,
};
use async_trait::async_trait;
use pulseengine_mcp_protocol::Response;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

/// Configuration for stdio transport
#[derive(Debug, Clone)]
pub struct StdioConfig {
    /// Maximum message size in bytes (default: 10MB)
    pub max_message_size: usize,
    /// Enable message validation
    pub validate_messages: bool,
}

impl Default for StdioConfig {
    fn default() -> Self {
        Self {
            max_message_size: 10 * 1024 * 1024, // 10MB
            validate_messages: true,
        }
    }
}

/// Standard I/O transport for MCP protocol
///
/// Implements the MCP stdio transport specification:
/// - Messages are delimited by newlines
/// - Messages MUST NOT contain embedded newlines
/// - Messages must be valid UTF-8
/// - Supports JSON-RPC batching
/// - Proper error handling with ID preservation
pub struct StdioTransport {
    running: Arc<std::sync::atomic::AtomicBool>,
    config: StdioConfig,
}

impl StdioTransport {
    /// Create a new stdio transport with default configuration
    pub fn new() -> Self {
        Self {
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            config: StdioConfig::default(),
        }
    }

    /// Create a new stdio transport with custom configuration
    pub fn with_config(config: StdioConfig) -> Self {
        Self {
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            config,
        }
    }

    /// Process a single line from stdin
    async fn process_line(
        &self,
        line: &str,
        handler: &RequestHandler,
        stdout: &mut tokio::io::Stdout,
    ) -> Result<(), TransportError> {
        // Validate message according to MCP spec
        if self.config.validate_messages {
            if let Err(e) = validate_message_string(line, Some(self.config.max_message_size)) {
                warn!("Message validation failed: {}", e);

                // Try to extract ID for error response
                let request_id = extract_id_from_malformed(line);
                let error_response = create_error_response(
                    pulseengine_mcp_protocol::Error::invalid_request(format!(
                        "Message validation failed: {e}"
                    )),
                    request_id,
                );

                self.send_response(stdout, &error_response).await?;
                return Ok(());
            }
        }

        debug!("Processing message: {}", line);

        // Parse JSON-RPC message (single or batch)
        let message = match JsonRpcMessage::parse(line) {
            Ok(msg) => msg,
            Err(e) => {
                error!("Failed to parse JSON: {}", e);

                // Try to extract ID for error response
                let request_id = extract_id_from_malformed(line);
                let error_response = create_error_response(
                    pulseengine_mcp_protocol::Error::parse_error(format!("Invalid JSON: {e}")),
                    request_id,
                );

                self.send_response(stdout, &error_response).await?;
                return Ok(());
            }
        };

        // Validate JSON-RPC structure
        if let Err(e) = message.validate() {
            warn!("JSON-RPC validation failed: {}", e);

            // For invalid structure, we can't reliably extract ID, use null
            let error_response = create_error_response(
                pulseengine_mcp_protocol::Error::invalid_request(format!("Invalid JSON-RPC: {e}")),
                serde_json::Value::Null,
            );

            self.send_response(stdout, &error_response).await?;
            return Ok(());
        }

        // Process the message (handles both single and batch)
        match process_batch(message, handler).await {
            Ok(Some(response_message)) => {
                // Send response(s)
                let response_json = response_message.to_string().map_err(|e| {
                    TransportError::Protocol(format!("Failed to serialize response: {e}"))
                })?;

                self.send_line(stdout, &response_json).await?;
            }
            Ok(None) => {
                // No response needed (notifications only)
                debug!("No response needed for message");
            }
            Err(e) => {
                error!("Failed to process message: {}", e);

                // Send generic error response
                let error_response = create_error_response(
                    pulseengine_mcp_protocol::Error::internal_error(format!(
                        "Processing failed: {e}"
                    )),
                    serde_json::Value::Null,
                );

                self.send_response(stdout, &error_response).await?;
            }
        }

        Ok(())
    }

    /// Send a response to stdout
    async fn send_response(
        &self,
        stdout: &mut tokio::io::Stdout,
        response: &Response,
    ) -> Result<(), TransportError> {
        let response_json = serde_json::to_string(response)
            .map_err(|e| TransportError::Protocol(format!("Failed to serialize response: {e}")))?;

        self.send_line(stdout, &response_json).await
    }

    /// Send a line to stdout with proper newline handling
    async fn send_line(
        &self,
        stdout: &mut tokio::io::Stdout,
        line: &str,
    ) -> Result<(), TransportError> {
        // Validate outgoing message
        if self.config.validate_messages {
            if let Err(e) = validate_message_string(line, Some(self.config.max_message_size)) {
                return Err(TransportError::Protocol(format!(
                    "Outgoing message validation failed: {e}"
                )));
            }
        }

        debug!("Sending response: {}", line);

        // Write with newline
        let line_with_newline = format!("{line}\n");

        if let Err(e) = stdout.write_all(line_with_newline.as_bytes()).await {
            return Err(TransportError::Connection(format!(
                "Failed to write to stdout: {e}"
            )));
        }

        if let Err(e) = stdout.flush().await {
            return Err(TransportError::Connection(format!(
                "Failed to flush stdout: {e}"
            )));
        }

        Ok(())
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn start(&mut self, handler: RequestHandler) -> Result<(), TransportError> {
        info!("Starting MCP-compliant stdio transport");
        info!("Max message size: {} bytes", self.config.max_message_size);
        info!("Message validation: {}", self.config.validate_messages);

        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        while self.running.load(std::sync::atomic::Ordering::Relaxed) {
            line.clear();

            match reader.read_line(&mut line).await {
                Ok(0) => {
                    debug!("EOF reached, stopping stdio transport");
                    break;
                }
                Ok(_) => {
                    // Remove trailing newline for processing
                    let trimmed_line = line.trim_end_matches(['\n', '\r']);

                    // Skip empty lines
                    if trimmed_line.is_empty() {
                        continue;
                    }

                    // Process the line
                    if let Err(e) = self.process_line(trimmed_line, &handler, &mut stdout).await {
                        error!("Failed to process line: {}", e);
                        // Continue processing other messages
                    }
                }
                Err(e) => {
                    error!("Failed to read from stdin: {}", e);
                    return Err(TransportError::Connection(format!("Stdin read error: {e}")));
                }
            }
        }

        info!("Stdio transport stopped");
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), TransportError> {
        info!("Stopping stdio transport");
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn health_check(&self) -> Result<(), TransportError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            Ok(())
        } else {
            Err(TransportError::Connection(
                "Transport not running".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulseengine_mcp_protocol::{Error as McpError, Request, Response};
    use serde_json::json;
    use std::io::Cursor;

    // Mock handler for testing
    fn mock_handler(
        request: Request,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> {
        Box::pin(async move {
            if request.method == "error_method" {
                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(McpError::method_not_found("Method not found")),
                }
            } else {
                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({"echo": request.method})),
                    error: None,
                }
            }
        })
    }

    #[tokio::test]
    async fn test_stdio_config() {
        let config = StdioConfig {
            max_message_size: 1024,
            validate_messages: true,
        };

        let transport = StdioTransport::with_config(config.clone());
        assert_eq!(transport.config.max_message_size, 1024);
        assert!(transport.config.validate_messages);
    }

    #[tokio::test]
    async fn test_message_validation() {
        let _transport = StdioTransport::new();
        let _handler: RequestHandler = Box::new(mock_handler);

        // Create a mock stdout
        let mut stdout_buffer = Vec::<u8>::new();
        let _stdout = Cursor::new(&mut stdout_buffer);

        // Test invalid message with embedded newline
        let invalid_line = "{\"jsonrpc\": \"2.0\", \"method\": \"test\n\", \"id\": 1}";

        // This should create a mock stdout that we can write to
        // For this test, we'll just verify the validation logic
        assert!(validate_message_string(invalid_line, Some(1024)).is_err());
    }

    #[test]
    fn test_extract_id_from_malformed() {
        // Test valid JSON with ID
        let text = r#"{"jsonrpc": "2.0", "method": "test", "id": 123}"#;
        let id = extract_id_from_malformed(text);
        assert_eq!(id, json!(123));

        // Test string ID
        let text = r#"{"jsonrpc": "2.0", "method": "test", "id": "abc"}"#;
        let id = extract_id_from_malformed(text);
        assert_eq!(id, json!("abc"));

        // Test malformed JSON
        let text = r#"{"jsonrpc": "2.0", "method": "test", "id": 456"#; // Missing closing brace
        let id = extract_id_from_malformed(text);
        assert_eq!(id, json!(456));

        // Test no ID
        let text = r#"{"jsonrpc": "2.0", "method": "test"}"#;
        let id = extract_id_from_malformed(text);
        assert_eq!(id, serde_json::Value::Null);
    }

    #[test]
    fn test_default_config() {
        let config = StdioConfig::default();
        assert_eq!(config.max_message_size, 10 * 1024 * 1024);
        assert!(config.validate_messages);
    }

    #[tokio::test]
    async fn test_health_check() {
        let transport = StdioTransport::new();

        // Initially not running
        assert!(transport.health_check().await.is_err());

        // Set as running
        transport
            .running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(transport.health_check().await.is_ok());
    }
}
