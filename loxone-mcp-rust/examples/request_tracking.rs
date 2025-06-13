//! Example demonstrating request ID tracking and observability
//!
//! This example shows how the new request tracking system works for
//! better debugging and distributed system observability.

use loxone_mcp_rust::server::request_context::{RequestContext, RequestTracker};
use serde_json::json;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🚀 Request tracking example");

    // Example 1: Basic request tracking
    let ctx = RequestContext::new("list_rooms".to_string());
    let _span = RequestTracker::create_span(&ctx);

    let params = json!({"room_filter": "kitchen"});
    RequestTracker::log_request_start(&ctx, &params);

    // Simulate some work
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    RequestTracker::log_request_end(&ctx, true, None);
    RequestTracker::log_if_slow(&ctx, 100); // This will trigger since we took 150ms

    info!("Request {} completed in {}ms", ctx.id, ctx.elapsed_ms());

    // Example 2: Request with client information
    let ctx2 = RequestContext::with_client(
        "control_device".to_string(),
        Some("client-123".to_string()),
        Some("n8n/1.0".to_string()),
        Some("session-456".to_string()),
    );

    let _span2 = RequestTracker::create_span(&ctx2);
    let params2 = json!({"device": "living-room-light", "action": "on"});
    RequestTracker::log_request_start(&ctx2, &params2);

    // Simulate work
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    RequestTracker::log_request_end(&ctx2, true, None);
    info!("Request {} completed in {}ms", ctx2.id, ctx2.elapsed_ms());

    // Example 3: Failed request
    let ctx3 = RequestContext::new("control_device".to_string());
    let _span3 = RequestTracker::create_span(&ctx3);

    let params3 = json!({"device": "invalid-device", "action": "on"});
    RequestTracker::log_request_start(&ctx3, &params3);

    // Simulate error
    let error = loxone_mcp_rust::error::LoxoneError::not_found("Device not found");
    RequestTracker::log_request_end(&ctx3, false, Some(&error));

    // Example 4: Child context for sub-operations
    let parent_ctx = RequestContext::new("complex_operation".to_string());
    info!("Parent request: {}", parent_ctx.id);

    let child_ctx = parent_ctx.child("sub_operation");
    info!(
        "Child request: {} (parent: {})",
        child_ctx.id, parent_ctx.id
    );

    Ok(())
}
