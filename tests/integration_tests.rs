//! Integration tests for MCP tools and server functionality
//!
//! Tests the complete integration of Loxone MCP tools with the pulseengine-mcp framework

use loxone_mcp_rust::config::CredentialStore;
use loxone_mcp_rust::server::framework_backend::LoxoneFrameworkBackend;
use loxone_mcp_rust::ServerConfig;
use rstest::*;
use serial_test::serial;

mod common;
use common::MockLoxoneServer;

#[rstest]
#[tokio::test]
async fn test_loxone_backend_integration() {
    let mock_server = MockLoxoneServer::start().await;

    // Set environment variables
    std::env::set_var("LOXONE_USER", "test_user");
    std::env::set_var("LOXONE_PASS", "test_password");

    let mut config = ServerConfig::dev_mode();
    config.loxone.url = mock_server.url().parse().unwrap();
    config.credentials = CredentialStore::Environment;

    let backend = LoxoneFrameworkBackend::initialize(config).await;
    assert!(
        backend.is_ok(),
        "Loxone backend should initialize with mock server"
    );
}

#[tokio::test]
#[serial]
async fn test_device_control_integration() {
    let mock_server = MockLoxoneServer::start().await;

    // Set environment variables
    std::env::set_var("LOXONE_USER", "test_user");
    std::env::set_var("LOXONE_PASS", "test_password");

    let mut config = ServerConfig::dev_mode();
    config.loxone.url = mock_server.url().parse().unwrap();
    config.credentials = CredentialStore::Environment;

    let _backend = LoxoneFrameworkBackend::initialize(config).await.unwrap();

    // TODO: Once we know the exact MCP tool execution API, test actual device control
    // Backend successfully initialized - ready for device control testing
}

#[tokio::test]
async fn test_sensor_monitoring_integration() {
    let mock_server = MockLoxoneServer::start().await;

    // Mock sensor data
    mock_server
        .mock_sensor_data("sensor-123", "temperature", 22.5)
        .await;

    // Set environment variables
    std::env::set_var("LOXONE_USER", "test_user");
    std::env::set_var("LOXONE_PASS", "test_password");

    let mut config = ServerConfig::dev_mode();
    config.loxone.url = mock_server.url().parse().unwrap();
    config.credentials = CredentialStore::Environment;

    let _backend = LoxoneFrameworkBackend::initialize(config).await.unwrap();

    // TODO: Test actual sensor monitoring through MCP tools
    // Backend successfully initialized - ready for sensor monitoring testing
}

// Disabled tests that need framework API updates

// #[tokio::test]
// #[ignore = "Requires MCP tool execution API"]
// async fn test_full_mcp_tool_execution() {
//     // This test would demonstrate full MCP tool execution
//     // once we have the proper API from pulseengine-mcp
// }

// #[tokio::test]
// #[ignore = "Requires WebSocket implementation"]
// async fn test_websocket_integration() {
//     // Test WebSocket-based real-time updates
//     // Requires WebSocket auth implementation
// }
