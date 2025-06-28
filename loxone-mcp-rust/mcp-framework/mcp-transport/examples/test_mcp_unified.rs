//! Unified MCP server that handles both SSE and streamable-http clients

use pulseengine_mcp_protocol::{Request, Response};
use mcp_transport::{http::HttpTransport, RequestHandler, Transport};
use serde_json::json;
use tracing::{debug, info};

// Handler that properly responds to MCP protocol
fn mcp_handler(
    request: Request,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> {
    Box::pin(async move {
        info!("📥 Request: {} (id: {:?})", request.method, request.id);

        match request.method.as_str() {
            "initialize" => {
                info!("🚀 Initializing MCP server");
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
                            "logging": {}
                        },
                        "serverInfo": {
                            "name": "mcp-unified-server",
                            "version": "1.0.0"
                        }
                    })),
                    error: None,
                }
            }
            "initialized" => {
                info!("✅ Client initialized");
                // This is a notification, so we return a response with null id
                Response {
                    jsonrpc: "2.0".to_string(),
                    id: serde_json::Value::Null,
                    result: Some(json!({})),
                    error: None,
                }
            }
            "tools/list" => {
                info!("📋 Listing tools");
                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "tools": [
                            {
                                "name": "test_tool",
                                "description": "A test tool",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "message": {
                                            "type": "string"
                                        }
                                    }
                                }
                            }
                        ]
                    })),
                    error: None,
                }
            }
            "resources/list" => {
                info!("📁 Listing resources");
                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "resources": [
                            {
                                "uri": "test://resource",
                                "name": "Test Resource",
                                "mimeType": "text/plain"
                            }
                        ]
                    })),
                    error: None,
                }
            }
            "prompts/list" => {
                info!("💬 Listing prompts");
                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "prompts": []
                    })),
                    error: None,
                }
            }
            _ => {
                debug!("Unknown method: {}", request.method);
                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(pulseengine_mcp_protocol::Error::method_not_found(&request.method)),
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

    info!("🚀 Starting Unified MCP Server");
    info!("This server handles both SSE and streamable-http clients");

    // Create HTTP transport with default configuration
    let mut transport = HttpTransport::new(3001);

    // Start the transport
    let handler: RequestHandler = Box::new(mcp_handler);
    transport.start(handler).await?;

    info!("✅ Server ready at http://localhost:3001");
    info!("");
    info!("Endpoints:");
    info!("  POST /messages - Send MCP messages");
    info!("  GET  /sse      - SSE/streamable-http endpoint");
    info!("");
    info!("The server automatically detects:");
    info!("  - MCP Inspector (returns JSON for streamable-http)");
    info!("  - Pure SSE clients (returns event stream)");
    info!("");
    info!("Press Ctrl+C to stop");

    // Keep server running
    tokio::signal::ctrl_c().await?;

    info!("Shutting down...");
    transport.stop().await?;

    Ok(())
}
