//! Test server that mimics what MCP Inspector expects

use mcp_transport::{http::HttpTransport, RequestHandler, Transport};
use pulseengine_mcp_protocol::{Request, Response};
use serde_json::json;
use tracing::{debug, info};

// Handler that logs requests and returns proper responses
fn inspector_handler(
    request: Request,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> {
    Box::pin(async move {
        info!(
            "Received request: method={}, id={:?}",
            request.method, request.id
        );
        debug!("Request params: {:?}", request.params);

        // Handle initialize request from MCP Inspector
        if request.method == "initialize" {
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
                        "name": "test-mcp-server",
                        "version": "0.1.0"
                    }
                })),
                error: None,
            }
        } else {
            // Echo other requests
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
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with debug level
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .init();

    info!("Starting MCP Inspector test server");

    // Create HTTP transport on port 3001 (default for MCP Inspector)
    let mut transport = HttpTransport::new(3001);

    // Start the transport
    let handler: RequestHandler = Box::new(inspector_handler);
    transport.start(handler).await?;

    info!("✅ Server ready for MCP Inspector");
    info!("Connect MCP Inspector to: http://localhost:3001");
    info!("");
    info!("Expected flow:");
    info!("1. Inspector connects to GET /sse");
    info!("2. Server sends 'connection' event");
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
