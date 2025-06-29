//! Test server for MCP Inspector using streamable-http transport

use pulseengine_mcp_transport::{streamable_http::StreamableHttpTransport, RequestHandler, Transport};
use pulseengine_mcp_protocol::{Request, Response};
use serde_json::json;
use tracing::info;

// Handler for MCP Inspector
fn inspector_handler(
    request: Request,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> {
    Box::pin(async move {
        info!(
            "📥 Received: method={}, id={:?}",
            request.method, request.id
        );

        match request.method.as_str() {
            "initialize" => {
                info!("🚀 Handling initialize request");
                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {},
                            "resources": {},
                            "prompts": {}
                        },
                        "serverInfo": {
                            "name": "test-streamable-http-server",
                            "version": "0.1.0"
                        }
                    })),
                    error: None,
                }
            }
            "notifications/initialized" => {
                info!("✅ Client initialized successfully");
                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({})),
                    error: None,
                }
            }
            _ => {
                info!("Echo request: {}", request.method);
                Response {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "echo": request.method,
                        "params": request.params,
                    })),
                    error: None,
                }
            }
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("🚀 Starting MCP Streamable HTTP Server for Inspector");
    info!("This implements the newer streamable-http transport");

    // Create streamable HTTP transport
    let mut transport = StreamableHttpTransport::new(3001);

    // Start the transport
    let handler: RequestHandler = Box::new(inspector_handler);
    transport.start(handler).await?;

    info!("✅ Server ready at http://localhost:3001");
    info!("");
    info!("Connect MCP Inspector to: http://localhost:3001");
    info!("Transport type: streamable-http");
    info!("");
    info!("Expected flow:");
    info!("1. Inspector connects to GET /sse for session");
    info!("2. Server returns connection confirmation (not SSE stream)");
    info!("3. Inspector sends POST /messages with initialize");
    info!("4. Server responds with capabilities");
    info!("");
    info!("Press Ctrl+C to stop");

    // Keep server running
    tokio::signal::ctrl_c().await?;

    info!("Shutting down...");
    transport.stop().await?;

    Ok(())
}
