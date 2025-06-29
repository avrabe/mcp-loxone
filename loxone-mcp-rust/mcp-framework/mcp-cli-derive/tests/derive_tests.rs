//! Simplified integration tests for derive macros

use pulseengine_mcp_cli_derive::{McpBackend, McpConfig};

#[test]
fn test_mcp_config_compiles() {
    // This test ensures the macro compiles correctly
    #[derive(McpConfig, Clone, Default)]
    #[allow(dead_code)]
    struct TestConfig {
        port: u16,
        server_info: Option<pulseengine_mcp_protocol::ServerInfo>,
        logging: Option<pulseengine_mcp_cli::DefaultLoggingConfig>,
    }

    // If this compiles, the macro works
}

#[test]
fn test_mcp_backend_compiles() {
    // Define a config type that the macro expects
    #[derive(Clone)]
    #[allow(dead_code)]
    struct TestBackendConfig {
        value: String,
    }

    // This test ensures the macro compiles correctly
    #[derive(Clone, McpBackend)]
    #[mcp_backend(simple, config = "TestBackendConfig")]
    #[allow(dead_code)]
    struct TestBackend {
        config: TestBackendConfig,
    }

    // If this compiles, the macro works
}
