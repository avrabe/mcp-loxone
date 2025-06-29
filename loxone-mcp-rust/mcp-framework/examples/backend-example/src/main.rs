//! Example MCP backend using the derive macro
//!
//! This example demonstrates how to use the McpBackend derive macro to create
//! a backend with minimal boilerplate and automatic error handling.

use pulseengine_mcp_cli::McpBackend;
use pulseengine_mcp_server::backend::SimpleBackend;
use serde::{Deserialize, Serialize};

/// Example backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleBackendConfig {
    pub name: String,
    pub version: String,
    pub tools_enabled: bool,
}

impl Default for ExampleBackendConfig {
    fn default() -> Self {
        Self {
            name: "Example Backend".to_string(),
            version: "1.0.0".to_string(),
            tools_enabled: true,
        }
    }
}

/// Example backend using the derive macro
#[derive(Clone, McpBackend)]
#[mcp_backend(simple)] // Use SimpleBackend for fewer required methods
pub struct ExampleBackend {
    config: ExampleBackendConfig,
}

impl ExampleBackend {
    pub fn new(config: ExampleBackendConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &ExampleBackendConfig {
        &self.config
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let config = ExampleBackendConfig::default();

    // Initialize the backend
    let backend = ExampleBackend::new(config);

    tracing::info!("Backend initialized successfully");
    tracing::info!("Server info: {:?}", backend.get_server_info());

    tracing::info!("Backend example demonstrates the McpBackend derive macro");
    tracing::info!("Config: {:?}", backend.config());

    // The derived implementation provides default no-op implementations
    // In a real backend, you would override these methods

    tracing::info!("Example backend demo completed successfully!");

    Ok(())
}
