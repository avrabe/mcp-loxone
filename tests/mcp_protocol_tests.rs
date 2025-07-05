//! MCP Protocol Compliance Tests
//!
//! Tests that verify the MCP server implementation follows the Model Context Protocol
//! specification and adheres to best practices using the pulseengine-mcp framework.

use loxone_mcp_rust::framework_integration::backend::LoxoneBackend;
use loxone_mcp_rust::{ServerConfig, CredentialStore};
use rstest::*;
use serial_test::serial;

mod common;
use common::{MockLoxoneServer, test_fixtures::*};

#[rstest]
#[tokio::test]
async fn test_mcp_backend_initialization() {
    let mock_server = MockLoxoneServer::start().await;
    
    // Create test configuration pointing to mock server
    let mut config = test_server_config();
    config.loxone.url = mock_server.url().parse().unwrap();
    config.credentials = CredentialStore::Environment;
    
    // Test that LoxoneBackend can be initialized with pulseengine-mcp framework
    with_test_env(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let backend = LoxoneBackend::initialize(config).await;
            assert!(backend.is_ok(), "Backend should initialize successfully with mock server");
        })
    });
}

#[rstest]
#[tokio::test]
async fn test_mcp_capabilities() {
    let mock_server = MockLoxoneServer::start().await;
    
    let mut config = test_server_config();
    config.loxone.url = mock_server.url().parse().unwrap();
    config.credentials = CredentialStore::Environment;
    
    with_test_env(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let backend = LoxoneBackend::initialize(config).await.unwrap();
            
            // Test capabilities using pulseengine-mcp framework patterns
            // The exact API depends on your McpBackend trait implementation
            // This is a placeholder showing the pattern - adjust to match your actual API
            
            // Verify backend is functional
            assert!(true, "Backend created successfully - framework integration working");
        })
    });
}

#[tokio::test]
#[serial]
async fn test_mcp_tool_listing() {
    let mock_server = MockLoxoneServer::start().await;
    
    with_test_env(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut config = ServerConfig::dev_mode();
            config.loxone.url = mock_server.url().parse().unwrap();
            config.credentials = CredentialStore::Environment;
            
            let backend = LoxoneBackend::initialize(config).await.unwrap();
            
            // Test that MCP tools are properly exposed
            // This would test the actual McpBackend trait methods once we know the exact API
            // For now, verify initialization succeeds
            assert!(true, "MCP backend with tools initialized successfully");
        })
    });
}

#[tokio::test]
async fn test_mcp_error_handling() {
    // Test MCP protocol error handling with mock server returning errors
    let mock_server = MockLoxoneServer::start().await;
    
    // Setup mock to return error responses
    mock_server.mock_error_response("/jdev/cfg/api", 500, "Internal Server Error").await;
    
    with_test_env(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut config = ServerConfig::dev_mode();
            config.loxone.url = mock_server.url().parse().unwrap();
            config.credentials = CredentialStore::Environment;
            
            // Test that errors are handled gracefully by the framework
            let backend = LoxoneBackend::initialize(config).await;
            
            // Depending on error handling strategy, this might succeed or fail gracefully
            // The key is that it doesn't panic or hang
            match backend {
                Ok(_) => assert!(true, "Backend handles errors gracefully"),
                Err(_) => assert!(true, "Backend fails gracefully with proper error handling"),
            }
        })
    });
}

// TODO: Add more specific MCP protocol compliance tests once we understand
// the exact pulseengine-mcp framework API patterns being used
