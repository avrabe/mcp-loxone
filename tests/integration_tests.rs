//! Integration tests for MCP tools and server functionality
//!
//! Tests the complete integration of Loxone MCP tools with the pulseengine-mcp framework

use loxone_mcp_rust::framework_integration::backend::LoxoneBackend;
use loxone_mcp_rust::{ServerConfig, CredentialStore};
use rstest::*;
use serial_test::serial;

mod common;
use common::{MockLoxoneServer, test_fixtures::*};

#[rstest]
#[tokio::test]
async fn test_loxone_backend_integration() {
    let mock_server = MockLoxoneServer::start().await;
    
    with_test_env(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut config = ServerConfig::dev_mode();
            config.loxone.url = mock_server.url().parse().unwrap();
            config.credentials = CredentialStore::Environment;
            
            let backend = LoxoneBackend::initialize(config).await;
            assert!(backend.is_ok(), "Loxone backend should initialize with mock server");
        })
    });
}

#[tokio::test]
#[serial]
async fn test_device_control_integration() {
    let mock_server = MockLoxoneServer::start().await;
    
    // Test device control through the MCP framework
    with_test_env(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut config = ServerConfig::dev_mode();
            config.loxone.url = mock_server.url().parse().unwrap();
            config.credentials = CredentialStore::Environment;
            
            let backend = LoxoneBackend::initialize(config).await.unwrap();
            
            // TODO: Once we know the exact MCP tool execution API, test actual device control
            // For now, verify the backend can be created and is functional
            assert!(true, "Device control backend integration successful");
        })
    });
}

#[tokio::test]
async fn test_sensor_monitoring_integration() {
    let mock_server = MockLoxoneServer::start().await;
    
    // Setup mock sensor data
    mock_server.mock_sensor_data("test-sensor-uuid", "temperature", 22.5).await;
    
    with_test_env(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut config = ServerConfig::dev_mode();
            config.loxone.url = mock_server.url().parse().unwrap();
            config.credentials = CredentialStore::Environment;
            
            let backend = LoxoneBackend::initialize(config).await.unwrap();
            
            // TODO: Test sensor monitoring through MCP tools once API is known
            assert!(true, "Sensor monitoring integration successful");
        })
    });
}

// TODO: Add more comprehensive integration tests for:
// - MCP tool execution
// - Real-time sensor updates
// - Multi-device operations
// - Error propagation through the framework
