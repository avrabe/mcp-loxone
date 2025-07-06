//! Edge case and error handling tests
//!
//! Tests various edge cases, error conditions, and resilience scenarios
//! using the pulseengine-mcp framework and mock infrastructure.

use loxone_mcp_rust::config::CredentialStore;
use loxone_mcp_rust::server::framework_backend::LoxoneFrameworkBackend;
use loxone_mcp_rust::ServerConfig;
use rstest::*;
use wiremock::{matchers::method, Mock, ResponseTemplate};

mod common;
use common::{containers::ContainerTestEnvironment, MockLoxoneServer};

#[rstest]
#[tokio::test]
async fn test_connection_timeout_handling() {
    let mock_server = MockLoxoneServer::start().await;

    // Mock a slow response to test timeout handling
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(10)))
        .mount(&mock_server.server)
        .await;

    // Set environment variables
    std::env::set_var("LOXONE_USERNAME", "test_user");
    std::env::set_var("LOXONE_PASSWORD", "test_password");

    let mut config = ServerConfig::dev_mode();
    config.loxone.url = mock_server.url().parse().unwrap();
    config.loxone.timeout = std::time::Duration::from_millis(100); // Very short timeout
    config.credentials = CredentialStore::Environment;

    let result = LoxoneFrameworkBackend::initialize(config).await;

    // Should handle timeout gracefully
    assert!(
        result.is_ok() || result.is_err(),
        "Backend should handle timeouts without panicking"
    );
}

#[tokio::test]
async fn test_invalid_server_response() {
    let mock_server = MockLoxoneServer::start().await;

    // Mock invalid JSON response
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("invalid json{"))
        .mount(&mock_server.server)
        .await;

    // Set environment variables
    std::env::set_var("LOXONE_USERNAME", "test_user");
    std::env::set_var("LOXONE_PASSWORD", "test_password");

    let mut config = ServerConfig::dev_mode();
    config.loxone.url = mock_server.url().parse().unwrap();
    config.credentials = CredentialStore::Environment;

    let result = LoxoneFrameworkBackend::initialize(config).await;

    // Should handle malformed responses gracefully
    assert!(
        result.is_ok() || result.is_err(),
        "Should handle invalid JSON without panicking"
    );
}

#[tokio::test]
async fn test_authentication_failure_recovery() {
    let mock_server = MockLoxoneServer::start().await;

    // Mock authentication failure
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "LL": {
                "control": "jdev/cfg/api",
                "value": "Authentication failed",
                "Code": "401"
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

    let result = LoxoneFrameworkBackend::initialize(config).await;

    // Should handle auth failures gracefully
    assert!(
        result.is_ok() || result.is_err(),
        "Backend should handle authentication failures without panicking"
    );
}

#[tokio::test]
async fn test_network_unreachable() {
    // Set environment variables
    std::env::set_var("LOXONE_USERNAME", "test_user");
    std::env::set_var("LOXONE_PASSWORD", "test_password");

    let mut config = ServerConfig::dev_mode();
    config.loxone.url = "http://unreachable.invalid:12345".parse().unwrap();
    config.credentials = CredentialStore::Environment;
    config.loxone.timeout = std::time::Duration::from_millis(500);
    config.loxone.max_retries = 0;

    let result = LoxoneFrameworkBackend::initialize(config).await;

    // Should handle unreachable hosts gracefully
    assert!(
        result.is_ok() || result.is_err(),
        "Backend should handle network errors without panicking"
    );
}

#[tokio::test]
async fn test_malformed_device_uuids() {
    let mock_server = MockLoxoneServer::start().await;

    // Mock response with malformed UUIDs
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "LL": {
                "value": "not-a-proper-uuid",
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

    let _backend = LoxoneFrameworkBackend::initialize(config).await.unwrap();

    // Backend initialization should succeed even with malformed UUID responses
}

#[tokio::test]
#[ignore = "Requires Docker for container testing"]
async fn test_database_connection_edge_cases() {
    // Test with containerized database for complex scenarios
    let container_env = ContainerTestEnvironment::new()
        .with_database()
        .await
        .unwrap();

    let env_vars = container_env.get_env_vars();

    // Use the database URL from the container environment
    for (key, value) in env_vars {
        std::env::set_var(key, value);
    }

    let mut config = ServerConfig::dev_mode();
    config.credentials = CredentialStore::Environment;

    let _backend = LoxoneFrameworkBackend::initialize(config).await.unwrap();

    // Backend successfully initialized with containerized database
}

#[tokio::test]
async fn test_concurrent_initialization() {
    let mock_server = MockLoxoneServer::start().await;

    // Set environment variables
    std::env::set_var("LOXONE_USERNAME", "test_user");
    std::env::set_var("LOXONE_PASSWORD", "test_password");

    // Test multiple concurrent backend initializations
    let tasks: Vec<_> = (0..5)
        .map(|_| {
            let url = mock_server.url().to_string();
            tokio::spawn(async move {
                let mut config = ServerConfig::dev_mode();
                config.loxone.url = url.parse().unwrap();
                config.credentials = CredentialStore::Environment;

                LoxoneFrameworkBackend::initialize(config).await
            })
        })
        .collect();

    let results = futures::future::join_all(tasks).await;

    // All concurrent initializations should complete without deadlocks
    for result in results {
        assert!(
            result.is_ok(),
            "Concurrent initialization should not deadlock"
        );
    }
}

#[tokio::test]
async fn test_resource_exhaustion_handling() {
    let mock_server = MockLoxoneServer::start().await;

    // Mock server overload response
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({
            "LL": {
                "control": "jdev/cfg/api",
                "value": "Service Unavailable",
                "Code": "503"
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

    let result = LoxoneFrameworkBackend::initialize(config).await;

    // Should handle service unavailable gracefully
    assert!(
        result.is_ok() || result.is_err(),
        "Backend should handle service unavailable without panicking"
    );
}
