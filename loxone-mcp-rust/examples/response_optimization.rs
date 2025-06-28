//! Example demonstrating response optimization for MCP best practices
//!
//! This example shows how optimized responses provide better user experience
//! by returning empty results instead of "not found" error messages.
//!
//! NOTE: This example is temporarily disabled as the response_optimization module
//! is being refactored as part of the framework migration.

fn main() {
    println!("Response optimization example is temporarily disabled during refactoring.");
}

/*
// Response optimization example - currently disabled as module is being refactored
use loxone_mcp_rust::server::response_optimization::OptimizedResponses;
use loxone_mcp_rust::tools::ToolResponse;
use mcp_protocol::{CallToolResult, Content};
use tracing::{info, Level};

fn extract_content_text(result: &mcp_protocol::CallToolResult) -> String {
    if let Some(content) = result.content.first() {
        match content {
            mcp_protocol::Content::Text { text } => text.clone(),
            _ => "No text content".to_string(),
        }
    } else {
        "No text content".to_string()
    }
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🔧 Response optimization demonstration");

    // Example 1: Optimized vs Traditional Room Not Found
    info!("\n📁 Example 1: Room Not Found");

    info!("❌ Traditional approach (error response):");
    info!("   Status: Error");
    info!("   Message: 'Room Kitchen not found'");

    info!("✅ Optimized approach (success with guidance):");
    let optimized_room = OptimizedResponses::room_not_found("Kitchen");
    let content = extract_content_text(&optimized_room);
    info!("   Status: Success");
    info!("   Content: {}", content);

    // Example 2: Optimized vs Traditional Device Not Found
    info!("\n🔌 Example 2: Device Not Found");

    info!("❌ Traditional approach (error response):");
    info!("   Status: Error");
    info!("   Message: 'Device invalid-light not found'");

    info!("✅ Optimized approach (success with guidance):");
    let optimized_device = OptimizedResponses::device_not_found("invalid-light");
    let content = extract_content_text(&optimized_device);
    info!("   Status: Success");
    info!("   Content: {}", content);

    // Example 3: Empty Results Instead of Errors
    info!("\n💡 Example 3: No Lights Found");

    info!("❌ Traditional approach (error response):");
    info!("   Status: Error");
    info!("   Message: 'No lights found in room Basement'");

    info!("✅ Optimized approach (empty success response):");
    let optimized_lights = OptimizedResponses::empty_lights(Some("Basement"));
    let content = extract_content_text(&optimized_lights);
    info!("   Status: Success");
    info!("   Content: {}", content);

    // Example 4: Empty Blinds Response
    info!("\n🪟 Example 4: No Blinds Found");

    info!("❌ Traditional approach (error response):");
    info!("   Status: Error");
    info!("   Message: 'No rolladen/blinds found in the system'");

    info!("✅ Optimized approach (empty success response):");
    let optimized_blinds = OptimizedResponses::empty_blinds(Some("system"));
    let content = extract_content_text(&optimized_blinds);
    info!("   Status: Success");
    info!("   Content: {}", content);

    // Example 5: Empty Operation Result
    info!("\n⚡ Example 5: Operation with No Items");

    info!("❌ Traditional approach (error response):");
    info!("   Status: Error");
    info!("   Message: 'No devices affected by operation'");

    info!("✅ Optimized approach (successful operation with zero items):");
    let optimized_operation =
        OptimizedResponses::empty_operation_result("control_lights", Some("Empty Room"));
    let content = extract_content_text(&optimized_operation);
    info!("   Status: Success");
    info!("   Content: {}", content);

    // Example 6: Tool Response Examples
    info!("\n🛠️ Example 6: Tool Response Patterns");

    info!("✅ Empty tool response:");
    let empty_tool = ToolResponse::empty();
    info!("   Status: {}", empty_tool.status);
    info!(
        "   Data: {}",
        serde_json::to_string(&empty_tool.data).unwrap()
    );

    info!("✅ Empty with context:");
    let context_tool = ToolResponse::empty_with_context("sensors in garage");
    info!("   Status: {}", context_tool.status);
    info!(
        "   Data: {}",
        serde_json::to_string(&context_tool.data).unwrap()
    );

    info!("✅ Not found with suggestion:");
    let not_found_tool = ToolResponse::not_found(
        "non-existent-sensor",
        Some("Use list_sensors to see available sensors"),
    );
    info!("   Status: {}", not_found_tool.status);
    info!(
        "   Data: {}",
        serde_json::to_string(&not_found_tool.data).unwrap()
    );

    // Benefits Summary
    info!("\n📊 Benefits of Optimized Responses:");
    info!("   • Better user experience - no confusing errors");
    info!("   • Consistent JSON structure for parsing");
    info!("   • Helpful suggestions for next actions");
    info!("   • Follows MCP best practices");
    info!("   • Easier to handle in client applications");
    info!("   • Provides context for empty results");

    info!("\n✅ Response optimization demonstration complete");
}
*/
