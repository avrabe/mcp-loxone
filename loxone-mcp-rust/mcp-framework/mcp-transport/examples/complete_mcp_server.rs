//! Complete MCP server example implementing all standard protocol methods
//!
//! This example demonstrates a fully-featured MCP server that implements:
//! - Initialize/initialized handshake
//! - Tools (list, call)
//! - Resources (list, read, templates, subscribe/unsubscribe)
//! - Prompts (list, get)
//! - Completion
//! - Logging
//! - Ping
//! - Error handling for unknown methods

use pulseengine_mcp_protocol::{Error, Request, Response};
use pulseengine_mcp_transport::{http::HttpTransport, RequestHandler, Transport};
use serde_json::json;
use tracing::{debug, info, warn};

/// Complete MCP handler implementing all protocol methods
fn complete_mcp_handler(
    request: Request,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> {
    Box::pin(async move {
        info!("📥 Request: {} (id: {:?})", request.method, request.id);

        match request.method.as_str() {
            // === INITIALIZATION ===
            "initialize" => {
                info!("🚀 Initializing MCP server");

                // Parse initialization parameters
                let client_info = request
                    .params
                    .get("clientInfo")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        format!(
                            "{} v{}",
                            obj.get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown"),
                            obj.get("version")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                        )
                    })
                    .unwrap_or_else(|| "Unknown Client".to_string());

                info!("🤝 Client: {}", client_info);

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {
                                "listChanged": true
                            },
                            "resources": {
                                "subscribe": true,
                                "listChanged": true
                            },
                            "prompts": {
                                "listChanged": true
                            },
                            "logging": {
                                "level": "info"
                            },
                            "sampling": {}
                        },
                        "serverInfo": {
                            "name": "complete-mcp-server",
                            "version": "1.0.0"
                        },
                        "instructions": "This is a complete MCP server demonstrating all protocol methods."
                    })),
                    error: None,
                }
            }

            "initialized" => {
                info!("✅ Client initialization complete");
                // Notification - no response needed
                Response {
                    jsonrpc: "2.0".to_string(),
                    id: serde_json::Value::Null,
                    result: Some(json!({})),
                    error: None,
                }
            }

            // === TOOLS ===
            "tools/list" => {
                info!("🔧 Listing available tools");

                // Parse pagination parameters
                let cursor = request.params.get("cursor").and_then(|v| v.as_str());

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "tools": [
                            {
                                "name": "echo",
                                "description": "Echo back the provided message",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "message": {
                                            "type": "string",
                                            "description": "Message to echo back"
                                        }
                                    },
                                    "required": ["message"]
                                }
                            },
                            {
                                "name": "calculate",
                                "description": "Perform basic arithmetic calculations",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "operation": {
                                            "type": "string",
                                            "enum": ["add", "subtract", "multiply", "divide"],
                                            "description": "Arithmetic operation to perform"
                                        },
                                        "a": {
                                            "type": "number",
                                            "description": "First operand"
                                        },
                                        "b": {
                                            "type": "number",
                                            "description": "Second operand"
                                        }
                                    },
                                    "required": ["operation", "a", "b"]
                                }
                            },
                            {
                                "name": "time",
                                "description": "Get current time information",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "format": {
                                            "type": "string",
                                            "enum": ["iso", "timestamp", "human"],
                                            "description": "Time format to return"
                                        }
                                    }
                                }
                            }
                        ],
                        "nextCursor": cursor.map(|_| "next-page")
                    })),
                    error: None,
                }
            }

            "tools/call" => {
                let tool_name = request
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                info!("⚡ Calling tool: {}", tool_name);

                let default_args = serde_json::Value::Object(Default::default());
                let arguments = request.params.get("arguments").unwrap_or(&default_args);

                let result = match tool_name {
                    "echo" => {
                        let message = arguments
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("No message provided");

                        json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Echo: {}", message)
                                }
                            ],
                            "isError": false
                        })
                    }

                    "calculate" => {
                        let operation = arguments
                            .get("operation")
                            .and_then(|v| v.as_str())
                            .unwrap_or("add");
                        let a = arguments.get("a").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let b = arguments.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);

                        let result = match operation {
                            "add" => a + b,
                            "subtract" => a - b,
                            "multiply" => a * b,
                            "divide" => {
                                if b != 0.0 {
                                    a / b
                                } else {
                                    return Response {
                                        jsonrpc: "2.0".to_string(),
                                        id: request.id,
                                        result: Some(json!({
                                            "content": [
                                                {
                                                    "type": "text",
                                                    "text": "Error: Division by zero"
                                                }
                                            ],
                                            "isError": true
                                        })),
                                        error: None,
                                    };
                                }
                            }
                            _ => {
                                return Response {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: None,
                                    error: Some(Error::invalid_params(format!(
                                        "Unknown operation: {}",
                                        operation
                                    ))),
                                };
                            }
                        };

                        json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("{} {} {} = {}", a, operation, b, result)
                                }
                            ],
                            "isError": false
                        })
                    }

                    "time" => {
                        let format = arguments
                            .get("format")
                            .and_then(|v| v.as_str())
                            .unwrap_or("iso");
                        let now = chrono::Utc::now();

                        let time_str = match format {
                            "iso" => now.to_rfc3339(),
                            "timestamp" => now.timestamp().to_string(),
                            "human" => now.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                            _ => now.to_rfc3339(),
                        };

                        json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Current time ({}): {}", format, time_str)
                                }
                            ],
                            "isError": false
                        })
                    }

                    _ => {
                        return Response {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: None,
                            error: Some(Error::method_not_found(&format!(
                                "Tool not found: {}",
                                tool_name
                            ))),
                        };
                    }
                };

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(result),
                    error: None,
                }
            }

            // === RESOURCES ===
            "resources/list" => {
                info!("📁 Listing available resources");

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "resources": [
                            {
                                "uri": "file://config.json",
                                "name": "Server Configuration",
                                "description": "Current server configuration",
                                "mimeType": "application/json"
                            },
                            {
                                "uri": "file://status.txt",
                                "name": "Server Status",
                                "description": "Current server status information",
                                "mimeType": "text/plain"
                            },
                            {
                                "uri": "file://logs.txt",
                                "name": "Server Logs",
                                "description": "Recent server log entries",
                                "mimeType": "text/plain"
                            }
                        ]
                    })),
                    error: None,
                }
            }

            "resources/read" => {
                let uri = request
                    .params
                    .get("uri")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                info!("📖 Reading resource: {}", uri);

                let (mime_type, content) = match uri {
                    "file://config.json" => (
                        "application/json",
                        json!({
                            "server": "complete-mcp-server",
                            "version": "1.0.0",
                            "features": ["tools", "resources", "prompts", "logging", "sampling"]
                        }).to_string()
                    ),
                    "file://status.txt" => (
                        "text/plain",
                        "Server Status: RUNNING\nUptime: 1h 23m 45s\nConnections: 1 active\nRequests handled: 42".to_string()
                    ),
                    "file://logs.txt" => (
                        "text/plain",
                        "[INFO] Server started\n[INFO] Client connected\n[INFO] Processing requests\n[DEBUG] Tools listed\n[DEBUG] Resource accessed".to_string()
                    ),
                    _ => {
                        return Response {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: None,
                            error: Some(Error::resource_not_found(uri)),
                        };
                    }
                };

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "contents": [
                            {
                                "uri": uri,
                                "mimeType": mime_type,
                                "text": content
                            }
                        ]
                    })),
                    error: None,
                }
            }

            "resources/templates/list" => {
                info!("📋 Listing resource templates");

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "resourceTemplates": [
                            {
                                "uriTemplate": "file://logs/{date}.txt",
                                "name": "Daily Logs",
                                "description": "Server logs for a specific date",
                                "mimeType": "text/plain"
                            },
                            {
                                "uriTemplate": "file://metrics/{metric}.json",
                                "name": "Metrics Data",
                                "description": "Server metrics in JSON format",
                                "mimeType": "application/json"
                            }
                        ]
                    })),
                    error: None,
                }
            }

            "resources/subscribe" => {
                let uri = request
                    .params
                    .get("uri")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                info!("🔔 Subscribing to resource: {}", uri);

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({})),
                    error: None,
                }
            }

            "resources/unsubscribe" => {
                let uri = request
                    .params
                    .get("uri")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                info!("🔕 Unsubscribing from resource: {}", uri);

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({})),
                    error: None,
                }
            }

            // === PROMPTS ===
            "prompts/list" => {
                info!("💬 Listing available prompts");

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "prompts": [
                            {
                                "name": "analyze_data",
                                "description": "Analyze provided data and generate insights",
                                "arguments": [
                                    {
                                        "name": "data_type",
                                        "description": "Type of data to analyze",
                                        "required": true
                                    },
                                    {
                                        "name": "context",
                                        "description": "Additional context for analysis",
                                        "required": false
                                    }
                                ]
                            },
                            {
                                "name": "summarize",
                                "description": "Create a summary of the provided content",
                                "arguments": [
                                    {
                                        "name": "length",
                                        "description": "Desired summary length (short, medium, long)",
                                        "required": false
                                    }
                                ]
                            }
                        ]
                    })),
                    error: None,
                }
            }

            "prompts/get" => {
                let name = request
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let arguments = request
                    .params
                    .get("arguments")
                    .and_then(|v| v.as_object())
                    .cloned()
                    .unwrap_or_default();

                info!("📝 Getting prompt: {} with args: {:?}", name, arguments);

                let (description, messages) = match name {
                    "analyze_data" => {
                        let data_type = arguments
                            .get("data_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let context = arguments
                            .get("context")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        (
                            "Data analysis prompt".to_string(),
                            vec![
                                json!({
                                    "role": "system",
                                    "content": {
                                        "type": "text",
                                        "text": format!("You are a data analyst. Analyze the {} data provided. Context: {}", data_type, context)
                                    }
                                }),
                                json!({
                                    "role": "user",
                                    "content": {
                                        "type": "text",
                                        "text": "Please analyze the data and provide insights, trends, and recommendations."
                                    }
                                }),
                            ],
                        )
                    }

                    "summarize" => {
                        let length = arguments
                            .get("length")
                            .and_then(|v| v.as_str())
                            .unwrap_or("medium");

                        (
                            "Content summarization prompt".to_string(),
                            vec![
                                json!({
                                    "role": "system",
                                    "content": {
                                        "type": "text",
                                        "text": format!("You are a professional summarizer. Create a {} summary of the provided content.", length)
                                    }
                                }),
                                json!({
                                    "role": "user",
                                    "content": {
                                        "type": "text",
                                        "text": "Please summarize the content, highlighting the key points and main ideas."
                                    }
                                }),
                            ],
                        )
                    }

                    _ => {
                        return Response {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: None,
                            error: Some(Error::resource_not_found(&format!(
                                "Prompt not found: {}",
                                name
                            ))),
                        };
                    }
                };

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "description": description,
                        "messages": messages
                    })),
                    error: None,
                }
            }

            // === COMPLETION ===
            "completion/complete" => {
                let ref_ = request
                    .params
                    .get("ref")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                info!("🔍 Providing completion for: {}", ref_);

                let completions = match ref_ {
                    "tool_names" => vec!["echo", "calculate", "time"],
                    "operations" => vec!["add", "subtract", "multiply", "divide"],
                    "formats" => vec!["iso", "timestamp", "human"],
                    _ => vec!["unknown_completion"],
                };

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "completion": completions.into_iter().map(|c| {
                            json!({
                                "completion": c,
                                "hasMore": false
                            })
                        }).collect::<Vec<_>>()
                    })),
                    error: None,
                }
            }

            // === LOGGING ===
            "logging/setLevel" => {
                let level = request
                    .params
                    .get("level")
                    .and_then(|v| v.as_str())
                    .unwrap_or("info");

                info!("📊 Setting log level to: {}", level);

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({})),
                    error: None,
                }
            }

            // === PING ===
            "ping" => {
                debug!("🏓 Ping received");

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({})),
                    error: None,
                }
            }

            // === UNKNOWN METHODS ===
            _ => {
                warn!("❓ Unknown method: {}", request.method);

                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(Error::method_not_found(&request.method)),
                }
            }
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .init();

    info!("🚀 Starting Complete MCP Server");
    info!("This server implements ALL standard MCP protocol methods");

    // Create HTTP transport
    let mut transport = HttpTransport::new(3002);

    // Start the transport
    let handler: RequestHandler = Box::new(complete_mcp_handler);
    transport.start(handler).await?;

    info!("✅ Complete MCP Server ready at http://localhost:3002");
    info!("");
    info!("📋 Supported Methods:");
    info!("  🔧 Tools:");
    info!("    - tools/list");
    info!("    - tools/call (echo, calculate, time)");
    info!("  📁 Resources:");
    info!("    - resources/list");
    info!("    - resources/read");
    info!("    - resources/templates/list");
    info!("    - resources/subscribe");
    info!("    - resources/unsubscribe");
    info!("  💬 Prompts:");
    info!("    - prompts/list");
    info!("    - prompts/get (analyze_data, summarize)");
    info!("  🔍 Completion:");
    info!("    - completion/complete");
    info!("  📊 Logging:");
    info!("    - logging/setLevel");
    info!("  🏓 Utilities:");
    info!("    - ping");
    info!("  🚀 Initialization:");
    info!("    - initialize");
    info!("    - initialized");
    info!("");
    info!("🔗 Test with MCP Inspector or curl:");
    info!("  curl -X POST http://localhost:3002/messages \\");
    info!("    -H 'Content-Type: application/json' \\");
    info!("    -d '{{\"jsonrpc\":\"2.0\",\"method\":\"tools/list\",\"id\":1}}'");
    info!("");
    info!("Press Ctrl+C to stop");

    // Keep server running
    tokio::signal::ctrl_c().await?;

    info!("Shutting down...");
    transport.stop().await?;

    Ok(())
}
