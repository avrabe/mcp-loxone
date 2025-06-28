//! Test HTTP/SSE transport implementation

use mcp_transport::{http::HttpTransport, RequestHandler, Transport, TransportError};
use pulseengine_mcp_protocol::{Request, Response};
use serde_json::json;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

// Simple echo handler
fn echo_handler(
    request: Request,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> {
    Box::pin(async move {
        info!("Received request: {:?}", request);
        Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(json!({
                "echo": request.method,
                "params": request.params,
            })),
            error: None,
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("Starting HTTP/SSE transport test server");

    // Create HTTP transport
    let mut transport = HttpTransport::new(3001);

    // Start the transport
    let handler: RequestHandler = Box::new(echo_handler);
    transport.start(handler).await?;

    info!("Server started on http://localhost:3001");
    info!("Endpoints:");
    info!("  POST http://localhost:3001/messages - Send messages");
    info!("  GET  http://localhost:3001/sse      - SSE stream");

    // Keep server running
    info!("Server is running. Press Ctrl+C to stop.");

    // Send periodic test messages
    let transport_clone = Arc::new(transport);
    tokio::spawn(async move {
        let mut counter = 0;
        loop {
            sleep(Duration::from_secs(5)).await;
            counter += 1;
            let message = json!({
                "jsonrpc": "2.0",
                "method": "test.notification",
                "params": {
                    "counter": counter,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }
            })
            .to_string();

            if let Err(e) = transport_clone.broadcast_message(&message).await {
                error!("Failed to broadcast message: {}", e);
            } else {
                info!("Broadcast message #{}", counter);
            }
        }
    });

    // Keep main thread alive
    loop {
        sleep(Duration::from_secs(60)).await;
    }
}
