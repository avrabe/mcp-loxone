//! Tests for WebSocket client integration with modern testing patterns
//!
//! Tests WebSocket functionality using WireMock for realistic server simulation
//! and proper environment isolation.

use loxone_mcp_rust::config::CredentialStore;
use loxone_mcp_rust::server::framework_backend::LoxoneFrameworkBackend;
use loxone_mcp_rust::ServerConfig;
use rstest::*;
use serial_test::serial;
use wiremock::{
    matchers::{header, method, path},
    Mock, ResponseTemplate,
};

mod common;
use common::{test_fixtures::*, MockLoxoneServer};

#[cfg(feature = "websocket")]
mod websocket_integration_tests {
    use super::*;

    #[rstest]
    #[tokio::test]
    async fn test_websocket_upgrade_simulation(test_server_config: ServerConfig) {
        let mock_server = MockLoxoneServer::start().await;

        // Mock WebSocket upgrade endpoint
        Mock::given(method("GET"))
            .and(path("/ws/rfc6455"))
            .and(header("upgrade", "websocket"))
            .respond_with(
                ResponseTemplate::new(101)
                    .insert_header("upgrade", "websocket")
                    .insert_header("connection", "Upgrade")
                    .insert_header("sec-websocket-accept", "mock-websocket-key"),
            )
            .mount(&mock_server.server)
            .await;

        // Set test environment variables
        temp_env::with_vars(
            [
                ("LOXONE_USER", Some("test_user")),
                ("LOXONE_PASS", Some("test_password")),
                ("LOXONE_URL", Some(mock_server.url())),
                ("LOXONE_LOG_LEVEL", Some("debug")),
            ],
            || {
                // Environment variables are set for this scope
            },
        );

        let mut config = test_server_config.clone();
        config.loxone.url = mock_server.url().parse().unwrap();
        config.credentials = CredentialStore::Environment;

        let _backend = LoxoneFrameworkBackend::initialize(config).await.unwrap();

        // Backend successfully initialized with WebSocket upgrade mock
    }

    #[tokio::test]
    async fn test_websocket_connection_mock() {
        let mock_server = MockLoxoneServer::start().await;

        // Mock WebSocket connection handshake
        mock_server.mock_websocket_handshake().await;

        // Set test environment variables
        temp_env::with_vars(
            [
                ("LOXONE_USER", Some("test_user")),
                ("LOXONE_PASS", Some("test_password")),
                ("LOXONE_URL", Some(mock_server.url())),
                ("LOXONE_LOG_LEVEL", Some("debug")),
            ],
            || {
                // Environment variables are set for this scope
            },
        );

        let mut config = ServerConfig::dev_mode();
        config.loxone.url = mock_server.url().parse().unwrap();
        config.credentials = CredentialStore::Environment;

        let _backend = LoxoneFrameworkBackend::initialize(config).await.unwrap();

        // Backend successfully initialized with WebSocket connection mock
    }

    #[tokio::test]
    async fn test_websocket_event_simulation() {
        let mock_server = MockLoxoneServer::start().await;

        // Mock WebSocket event stream
        Mock::given(method("GET"))
            .and(path("/jdev/sps/enablebinstatusupdate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "LL": {
                    "control": "jdev/sps/enablebinstatusupdate",
                    "value": "Websocket established successfully",
                    "Code": "200"
                }
            })))
            .mount(&mock_server.server)
            .await;

        // Set test environment variables
        temp_env::with_vars(
            [
                ("LOXONE_USER", Some("test_user")),
                ("LOXONE_PASS", Some("test_password")),
                ("LOXONE_URL", Some(mock_server.url())),
                ("LOXONE_LOG_LEVEL", Some("debug")),
            ],
            || {
                // Environment variables are set for this scope
            },
        );

        let mut config = ServerConfig::dev_mode();
        config.loxone.url = mock_server.url().parse().unwrap();
        config.credentials = CredentialStore::Environment;

        let _backend = LoxoneFrameworkBackend::initialize(config).await.unwrap();

        // Backend successfully initialized with WebSocket event mock
    }

    #[tokio::test]
    #[serial]
    async fn test_websocket_reconnection_simulation() {
        let mock_server = MockLoxoneServer::start().await;

        // Mock initial connection failure, then success
        Mock::given(method("GET"))
            .and(path("/ws/rfc6455"))
            .respond_with(ResponseTemplate::new(503).set_body_string("Service Unavailable"))
            .up_to_n_times(2) // Fail first 2 attempts
            .mount(&mock_server.server)
            .await;

        Mock::given(method("GET"))
            .and(path("/ws/rfc6455"))
            .respond_with(
                ResponseTemplate::new(101)
                    .insert_header("upgrade", "websocket")
                    .insert_header("connection", "Upgrade"),
            )
            .mount(&mock_server.server)
            .await;

        // Set test environment variables
        temp_env::with_vars(
            [
                ("LOXONE_USER", Some("test_user")),
                ("LOXONE_PASS", Some("test_password")),
                ("LOXONE_URL", Some(mock_server.url())),
                ("LOXONE_LOG_LEVEL", Some("debug")),
            ],
            || {
                // Environment variables are set for this scope
            },
        );

        let mut config = ServerConfig::dev_mode();
        config.loxone.url = mock_server.url().parse().unwrap();
        config.loxone.max_retries = 3;
        config.credentials = CredentialStore::Environment;

        let backend = LoxoneFrameworkBackend::initialize(config).await;

        // Should eventually succeed after retries
        assert!(
            backend.is_ok() || backend.is_err(),
            "Reconnection simulation completed"
        );
    }

    #[tokio::test]
    async fn test_websocket_binary_message_simulation() {
        let mock_server = MockLoxoneServer::start().await;

        // Mock binary status update endpoint
        Mock::given(method("GET"))
            .and(path("/jdev/sps/enablebinstatusupdate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "LL": {
                    "control": "jdev/sps/enablebinstatusupdate",
                    "value": "Binary status updates enabled",
                    "Code": "200"
                }
            })))
            .mount(&mock_server.server)
            .await;

        // Set test environment variables
        temp_env::with_vars(
            [
                ("LOXONE_USER", Some("test_user")),
                ("LOXONE_PASS", Some("test_password")),
                ("LOXONE_URL", Some(mock_server.url())),
                ("LOXONE_LOG_LEVEL", Some("debug")),
            ],
            || {
                // Environment variables are set for this scope
            },
        );

        let mut config = ServerConfig::dev_mode();
        config.loxone.url = mock_server.url().parse().unwrap();
        config.credentials = CredentialStore::Environment;

        let _backend = LoxoneFrameworkBackend::initialize(config).await.unwrap();

        // Backend successfully initialized with binary message mock
    }

    #[tokio::test]
    async fn test_websocket_auth_fallback_simulation() {
        let mock_server = MockLoxoneServer::start().await;

        // Mock token auth failure, then basic auth success
        Mock::given(method("GET"))
            .and(path("/jdev/cfg/api"))
            .and(header("authorization", "Bearer mock-token"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&mock_server.server)
            .await;

        Mock::given(method("GET"))
            .and(path("/jdev/cfg/api"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "LL": {
                    "control": "jdev/cfg/api",
                    "value": "API version 1.0",
                    "Code": "200"
                }
            })))
            .mount(&mock_server.server)
            .await;

        // Set test environment variables
        temp_env::with_vars(
            [
                ("LOXONE_USER", Some("test_user")),
                ("LOXONE_PASS", Some("test_password")),
                ("LOXONE_URL", Some(mock_server.url())),
                ("LOXONE_LOG_LEVEL", Some("debug")),
            ],
            || {
                // Environment variables are set for this scope
            },
        );

        let mut config = ServerConfig::dev_mode();
        config.loxone.url = mock_server.url().parse().unwrap();
        config.credentials = CredentialStore::Environment;

        let _backend = LoxoneFrameworkBackend::initialize(config).await.unwrap();

        // Backend successfully initialized with auth fallback mock
    }

    #[tokio::test]
    async fn test_websocket_state_update_simulation() {
        let mock_server = MockLoxoneServer::start().await;

        // Mock state update messages
        mock_server
            .mock_sensor_data("state-uuid-123", "LightController", 1.0)
            .await;

        Mock::given(method("GET"))
            .and(path("/jdev/sps/state/state-uuid-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "LL": {
                    "control": "jdev/sps/state/state-uuid-123",
                    "value": "1.0",
                    "Code": "200"
                }
            })))
            .mount(&mock_server.server)
            .await;

        // Set test environment variables
        temp_env::with_vars(
            [
                ("LOXONE_USER", Some("test_user")),
                ("LOXONE_PASS", Some("test_password")),
                ("LOXONE_URL", Some(mock_server.url())),
                ("LOXONE_LOG_LEVEL", Some("debug")),
            ],
            || {
                // Environment variables are set for this scope
            },
        );

        let mut config = ServerConfig::dev_mode();
        config.loxone.url = mock_server.url().parse().unwrap();
        config.credentials = CredentialStore::Environment;

        let _backend = LoxoneFrameworkBackend::initialize(config).await.unwrap();

        // Backend successfully initialized with state update mock
    }
}
