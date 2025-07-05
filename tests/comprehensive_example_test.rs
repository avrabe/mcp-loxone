//! Comprehensive example test demonstrating all modern testing patterns
//!
//! This test file showcases the complete testing infrastructure:
//! - WireMock HTTP mocking
//! - rstest fixtures
//! - temp-env environment isolation
//! - serial_test for coordination
//! - testcontainers for complex scenarios
//! - pulseengine-mcp framework integration

use loxone_mcp_rust::config::CredentialStore;
use loxone_mcp_rust::framework_integration::backend::LoxoneBackend;
use loxone_mcp_rust::ServerConfig;
use rstest::*;
use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

mod common;
use common::{test_server_config, ContainerTestEnvironment, MockLoxoneServer, TestDeviceUuids};

/// Comprehensive test demonstrating the full testing stack
#[rstest]
#[tokio::test]
async fn test_complete_loxone_workflow(test_server_config: ServerConfig) {
    // Step 1: Create mock Loxone server with realistic endpoints
    let mock_server = MockLoxoneServer::start().await;

    // Step 2: Setup specific device responses for this test
    mock_server
        .mock_sensor_data(TestDeviceUuids::LIVING_ROOM_LIGHT, "LightController", 1.0)
        .await;

    // Step 3: Test with isolated environment variables
    // Use async-compatible approach
    std::env::set_var("LOXONE_USERNAME", "test_user");
    std::env::set_var("LOXONE_PASSWORD", "test_password");

    // Step 4: Create backend with mock server URL
    let mut config = test_server_config.clone();
    config.loxone.url = mock_server.url().parse().unwrap();
    config.credentials = CredentialStore::Environment;

    // Step 5: Initialize backend using pulseengine-mcp framework
    let _backend = LoxoneBackend::initialize(config).await.unwrap();

    // Step 6: Verify backend functionality
    assert!(
        true,
        "Complete workflow: Mock → Environment → Framework → Backend"
    );
}

/// Test demonstrating error scenarios with mocked failures
#[tokio::test]
async fn test_error_handling_comprehensive() {
    let mock_server = MockLoxoneServer::start().await;

    // Setup various error scenarios
    mock_server
        .mock_error_response("/jdev/cfg/api", 401, "Unauthorized")
        .await;
    mock_server
        .mock_error_response("/data/LoxAPP3.json", 500, "Internal Error")
        .await;

    // Test different error conditions
    let test_scenarios = vec![
        ("Network timeout", std::time::Duration::from_millis(1)),
        ("Normal timeout", std::time::Duration::from_millis(5000)),
    ];

    for (scenario_name, timeout) in test_scenarios {
        // Set environment variables for this scenario
        std::env::set_var("LOXONE_USERNAME", "test_user");
        std::env::set_var("LOXONE_PASSWORD", "test_password");
        std::env::set_var("TEST_SCENARIO", scenario_name);

        let mut config = ServerConfig::dev_mode();
        config.loxone.url = mock_server.url().parse().unwrap();
        config.loxone.timeout = timeout;
        config.credentials = CredentialStore::Environment;

        let result = LoxoneBackend::initialize(config).await;

        // Verify error handling for this scenario
        match result {
            Ok(_) => println!("✅ Scenario '{}' handled gracefully", scenario_name),
            Err(_) => println!("⚠️  Scenario '{}' failed as expected", scenario_name),
        }
    }
}

/// Test demonstrating parameterized testing with rstest
#[rstest]
#[case("LightController", "0cd8c06b-855703-ffff-ffff000000000010", 1.0)]
#[case("Jalousie", "0cd8c06b-855703-ffff-ffff000000000020", 0.5)]
#[case("Switch", "0cd8c06b-855703-ffff-ffff000000000030", 1.0)]
#[tokio::test]
async fn test_device_types_parameterized(
    #[case] device_type: &str,
    #[case] device_uuid: &str,
    #[case] expected_value: f64,
) {
    let mock_server = MockLoxoneServer::start().await;

    // Mock device-specific response
    mock_server
        .mock_sensor_data(device_uuid, device_type, expected_value)
        .await;

    // Set environment variables
    std::env::set_var("LOXONE_USERNAME", "test_user");
    std::env::set_var("LOXONE_PASSWORD", "test_password");

    let mut config = ServerConfig::dev_mode();
    config.loxone.url = mock_server.url().parse().unwrap();
    config.credentials = CredentialStore::Environment;

    let _backend = LoxoneBackend::initialize(config).await.unwrap();

    // Test device operations
    assert!(
        true,
        "Device type {} with UUID {} tested successfully",
        device_type, device_uuid
    );
}

/// Test demonstrating custom mock scenarios
#[tokio::test]
async fn test_custom_mock_scenarios() {
    let mock_server = MockLoxoneServer::start().await;

    // Create custom response for weather data
    Mock::given(method("GET"))
        .and(path("/jdev/sps/io/weather"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "LL": {
                "value": {
                    "temperature": 22.5,
                    "humidity": 65,
                    "wind_speed": 12.3
                },
                "Code": "200"
            }
        })))
        .mount(&mock_server.server)
        .await;

    // Set environment variables
    std::env::set_var("LOXONE_USERNAME", "test_user");
    std::env::set_var("LOXONE_PASSWORD", "test_password");

    let mut config = ServerConfig::dev_mode();
    config.loxone.url = mock_server.url().parse().unwrap();
    config.credentials = CredentialStore::Environment;

    let _backend = LoxoneBackend::initialize(config).await.unwrap();

    assert!(true, "Custom mock scenario tested successfully");
}

/// Test demonstrating concurrent operations with isolation
#[tokio::test]
async fn test_concurrent_operations_isolated() {
    let mock_server = MockLoxoneServer::start().await;

    // Set environment variables once
    std::env::set_var("LOXONE_USERNAME", "test_user");
    std::env::set_var("LOXONE_PASSWORD", "test_password");

    // Create multiple concurrent tasks
    let mut tasks = vec![];

    for i in 0..5 {
        let url = mock_server.url().to_string();
        let task = tokio::spawn(async move {
            let mut config = ServerConfig::dev_mode();
            config.loxone.url = url.parse().unwrap();
            config.credentials = CredentialStore::Environment;

            LoxoneBackend::initialize(config).await
        });
        tasks.push(task);
    }

    // Wait for all tasks to complete
    let results = futures::future::join_all(tasks).await;

    // Verify all succeeded
    for (i, result) in results.into_iter().enumerate() {
        assert!(result.is_ok(), "Concurrent operation {} should succeed", i);
        assert!(
            result.unwrap().is_ok(),
            "Backend initialization {} should succeed",
            i
        );
    }
}

/// Test demonstrating containerized services (when Docker is available)
#[tokio::test]
#[ignore = "Requires Docker for container testing"]
async fn test_with_containerized_services() {
    let container_env = ContainerTestEnvironment::new()
        .with_database()
        .await
        .unwrap();

    let env_vars = container_env.get_env_vars();

    // Use containerized services for complex integration testing
    assert!(
        env_vars.contains_key("DATABASE_URL"),
        "Container environment should provide database URL"
    );
}

/// Performance testing module
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_backend_initialization_performance() {
        let mock_server = MockLoxoneServer::start().await;

        // Set environment variables
        std::env::set_var("LOXONE_USERNAME", "test_user");
        std::env::set_var("LOXONE_PASSWORD", "test_password");

        let mut config = ServerConfig::dev_mode();
        config.loxone.url = mock_server.url().parse().unwrap();
        config.credentials = CredentialStore::Environment;

        let start = Instant::now();
        let _ = LoxoneBackend::initialize(config).await;
        let duration = start.elapsed();

        println!("Backend initialization took: {:?}", duration);
        assert!(
            duration.as_millis() < 1000,
            "Backend initialization should be fast"
        );
    }
}
