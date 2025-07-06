//! Example demonstrating the ToolContextBuilder pattern to reduce cloning
//!
//! This example shows how to use the ToolContextBuilder to efficiently create
//! multiple tool contexts without excessive Arc cloning.

use loxone_mcp_rust::tools::ToolContextBuilder;
use std::sync::Arc;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Example showing typical usage pattern in handlers
    example_handler_usage().await?;

    // Example showing builder reuse
    example_builder_reuse().await?;

    Ok(())
}

/// Example showing typical handler usage pattern
async fn example_handler_usage() -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Example: Handler Usage Pattern ===");

    // In a real handler, you would have a server instance
    // For this demo, we'll create a mock scenario

    // Simulated server components (normally these would be in your server struct)
    let client = create_mock_client();
    let context = create_mock_context();
    let value_resolver = create_mock_value_resolver();
    let state_manager = create_mock_state_manager().await;

    // OLD WAY: Creating contexts with repeated cloning
    info!("Old way - creating contexts with manual cloning:");
    {
        use loxone_mcp_rust::tools::ToolContext;
        use std::time::Instant;

        let start = Instant::now();

        // Handler 1: control_lights
        let tool_context1 = ToolContext::with_services(
            client.clone(),         // Clone 1
            context.clone(),        // Clone 2
            value_resolver.clone(), // Clone 3
            state_manager.clone(),  // Clone 4
        );

        // Handler 2: control_blinds
        let tool_context2 = ToolContext::with_services(
            client.clone(),         // Clone 5
            context.clone(),        // Clone 6
            value_resolver.clone(), // Clone 7
            state_manager.clone(),  // Clone 8
        );

        // Handler 3: get_sensor_data
        let tool_context3 = ToolContext::with_services(
            client.clone(),         // Clone 9
            context.clone(),        // Clone 10
            value_resolver.clone(), // Clone 11
            state_manager.clone(),  // Clone 12
        );

        let elapsed = start.elapsed();
        info!("Created 3 contexts with 12 Arc clones in {:?}", elapsed);

        // Use the contexts
        let _ = (tool_context1, tool_context2, tool_context3);
    }

    info!("\nNew way - using ToolContextBuilder:");
    {
        use std::time::Instant;

        let start = Instant::now();

        // Create builder once with initial clones
        let builder = ToolContextBuilder::new(
            client.clone(),         // Clone 1
            context.clone(),        // Clone 2
            value_resolver.clone(), // Clone 3
            state_manager.clone(),  // Clone 4
        );

        // Now create contexts efficiently
        let tool_context1 = builder.build(); // 4 internal clones
        let tool_context2 = builder.build(); // 4 internal clones
        let tool_context3 = builder.build(); // 4 internal clones

        let elapsed = start.elapsed();
        info!(
            "Created 3 contexts with 4 initial + 12 internal Arc clones in {:?}",
            elapsed
        );
        info!("But the builder can be reused for many more contexts!");

        // Use the contexts
        let _ = (tool_context1, tool_context2, tool_context3);
    }

    Ok(())
}

/// Example showing builder reuse across multiple operations
async fn example_builder_reuse() -> Result<(), Box<dyn std::error::Error>> {
    info!("\n=== Example: Builder Reuse Pattern ===");

    // Simulated server components
    let client = create_mock_client();
    let context = create_mock_context();
    let value_resolver = create_mock_value_resolver();
    let state_manager = create_mock_state_manager().await;

    // Create builder once
    let builder = ToolContextBuilder::new(client, context, value_resolver, state_manager);

    // Simulate a server struct that holds the builder
    struct MockServer {
        context_builder: ToolContextBuilder,
    }

    let server = MockServer {
        context_builder: builder,
    };

    // Now in various handler methods, we can efficiently create contexts

    // Handler method 1
    async fn handle_device_control(server: &MockServer) {
        let context = server.context_builder.build();
        info!("Device control handler got context efficiently");
        // Use context for device control...
        let _ = context;
    }

    // Handler method 2
    async fn handle_room_query(server: &MockServer) {
        let context = server.context_builder.build();
        info!("Room query handler got context efficiently");
        // Use context for room queries...
        let _ = context;
    }

    // Handler method 3 - without state manager
    async fn handle_simple_query(server: &MockServer) {
        let context = server.context_builder.build_without_state_manager();
        info!("Simple query handler got context without state manager");
        // Use context for simple queries...
        let _ = context;
    }

    // Call multiple handlers
    handle_device_control(&server).await;
    handle_room_query(&server).await;
    handle_simple_query(&server).await;

    info!("\nUsing builder reference for nested calls:");

    // Example with builder reference for nested function calls
    async fn complex_operation(builder_ref: loxone_mcp_rust::tools::ToolContextBuilderRef<'_>) {
        // Nested function 1
        async fn operation_part1(builder_ref: loxone_mcp_rust::tools::ToolContextBuilderRef<'_>) {
            let context = builder_ref.build();
            info!("Part 1 got context via reference");
            let _ = context;
        }

        // Nested function 2
        async fn operation_part2(builder_ref: loxone_mcp_rust::tools::ToolContextBuilderRef<'_>) {
            let context = builder_ref.build();
            info!("Part 2 got context via reference");
            let _ = context;
        }

        operation_part1(builder_ref).await;
        operation_part2(builder_ref).await;
    }

    // Use builder reference for complex operations
    complex_operation(server.context_builder.as_ref()).await;

    Ok(())
}

// Mock implementations for demo purposes

// Simple mock client implementation for demo
struct DemoMockClient;

#[async_trait::async_trait]
impl loxone_mcp_rust::client::LoxoneClient for DemoMockClient {
    async fn connect(&mut self) -> loxone_mcp_rust::Result<()> {
        Ok(())
    }

    async fn is_connected(&self) -> loxone_mcp_rust::Result<bool> {
        Ok(true)
    }

    async fn disconnect(&mut self) -> loxone_mcp_rust::Result<()> {
        Ok(())
    }

    async fn send_command(
        &self,
        _uuid: &str,
        _command: &str,
    ) -> loxone_mcp_rust::Result<loxone_mcp_rust::client::LoxoneResponse> {
        Ok(loxone_mcp_rust::client::LoxoneResponse {
            code: 200,
            value: serde_json::Value::String("OK".to_string()),
        })
    }

    async fn get_structure(
        &self,
    ) -> loxone_mcp_rust::Result<loxone_mcp_rust::client::LoxoneStructure> {
        Ok(loxone_mcp_rust::client::LoxoneStructure {
            last_modified: "2024-01-01T00:00:00Z".to_string(),
            controls: std::collections::HashMap::new(),
            rooms: std::collections::HashMap::new(),
            cats: std::collections::HashMap::new(),
            global_states: std::collections::HashMap::new(),
        })
    }

    async fn get_device_states(
        &self,
        _uuids: &[String],
    ) -> loxone_mcp_rust::Result<std::collections::HashMap<String, serde_json::Value>> {
        Ok(std::collections::HashMap::new())
    }

    async fn get_state_values(
        &self,
        _state_uuids: &[String],
    ) -> loxone_mcp_rust::Result<std::collections::HashMap<String, serde_json::Value>> {
        Ok(std::collections::HashMap::new())
    }

    async fn get_system_info(&self) -> loxone_mcp_rust::Result<serde_json::Value> {
        Ok(serde_json::json!({"version": "mock"}))
    }

    async fn health_check(&self) -> loxone_mcp_rust::Result<bool> {
        Ok(true)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn create_mock_client() -> Arc<dyn loxone_mcp_rust::client::LoxoneClient> {
    Arc::new(DemoMockClient)
}

fn create_mock_context() -> Arc<loxone_mcp_rust::client::ClientContext> {
    Arc::new(loxone_mcp_rust::client::ClientContext::new())
}

fn create_mock_value_resolver() -> Arc<loxone_mcp_rust::services::UnifiedValueResolver> {
    let client = create_mock_client();
    let sensor_registry = Arc::new(loxone_mcp_rust::services::SensorTypeRegistry::new());
    Arc::new(loxone_mcp_rust::services::UnifiedValueResolver::new(
        client,
        sensor_registry,
    ))
}

async fn create_mock_state_manager() -> Option<Arc<loxone_mcp_rust::services::StateManager>> {
    let value_resolver = create_mock_value_resolver();
    match loxone_mcp_rust::services::StateManager::new(value_resolver).await {
        Ok(manager) => Some(Arc::new(manager)),
        Err(_) => None,
    }
}
