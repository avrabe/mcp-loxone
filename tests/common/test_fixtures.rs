//! Test fixtures and utilities for consistent test setup
//!
//! Provides reusable test data, configuration helpers, and common
//! test patterns using rstest fixtures.

use loxone_mcp_rust::config::{CredentialStore, LoxoneConfig, ServerConfig};
use rstest::*;
use std::time::Duration;
use temp_env::with_vars;
use url::Url;

/// Create a test Loxone configuration pointing to a mock server
#[fixture]
pub fn test_loxone_config(#[default("http://localhost:8080")] mock_url: &str) -> LoxoneConfig {
    LoxoneConfig {
        url: Url::parse(mock_url).expect("Valid URL"),
        username: "test_user".to_string(),
        verify_ssl: false,
        timeout: Duration::from_secs(5),
        max_retries: 1,
        max_connections: Some(5),
        #[cfg(feature = "websocket")]
        websocket: Default::default(),
        auth_method: loxone_mcp_rust::config::AuthMethod::Basic,
    }
}

/// Create a test server configuration for development/testing
#[fixture]
pub fn test_server_config() -> ServerConfig {
    let mut config = ServerConfig::dev_mode();
    config.credentials = CredentialStore::Environment;
    config
}

/// Environment variables for clean testing
pub fn get_test_env_vars() -> Vec<(&'static str, Option<&'static str>)> {
    vec![
        ("LOXONE_USERNAME", Some("test_user")),
        ("LOXONE_PASSWORD", Some("test_password")),
        ("LOXONE_URL", Some("http://localhost:8080")),
        ("LOXONE_LOG_LEVEL", Some("debug")),
    ]
}

/// Common test device UUIDs for consistent testing
pub struct TestDeviceUuids;

impl TestDeviceUuids {
    pub const LIVING_ROOM_LIGHT: &'static str = "0cd8c06b-855703-ffff-ffff000000000010";
    pub const KITCHEN_LIGHT: &'static str = "0cd8c06b-855703-ffff-ffff000000000011";
    pub const LIVING_ROOM_BLINDS: &'static str = "0cd8c06b-855703-ffff-ffff000000000020";
    pub const WINDOW_SENSOR: &'static str = "0cd8c06b-855703-ffff-ffff000000000030";
    pub const TEMPERATURE_SENSOR: &'static str = "0cd8c06b-855703-ffff-ffff000000000031";
}

/// Common test room UUIDs
pub struct TestRoomUuids;

impl TestRoomUuids {
    pub const LIVING_ROOM: &'static str = "0cd8c06b-855703-ffff-ffff000000000000";
    pub const KITCHEN: &'static str = "0cd8c06b-855703-ffff-ffff000000000001";
    pub const BEDROOM: &'static str = "0cd8c06b-855703-ffff-ffff000000000002";
}

/// Helper to run tests with isolated environment
pub fn with_test_env<F, R>(test_fn: F) -> R
where
    F: FnOnce() -> R,
{
    with_vars(get_test_env_vars(), test_fn)
}

/// Async helper to run tests with isolated environment
pub async fn with_test_env_async<F, Fut, R>(test_fn: F) -> R
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = R>,
{
    // Note: temp-env doesn't directly support async, but we can use it in a sync wrapper
    with_vars(get_test_env_vars(), || {
        // Create a new tokio runtime for the isolated test
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(test_fn())
    })
}

/// Sample sensor data for testing
pub struct TestSensorData;

impl TestSensorData {
    pub fn temperature_reading() -> serde_json::Value {
        serde_json::json!({
            "LL": {
                "value": 22.5,
                "Code": "200"
            }
        })
    }

    pub fn window_sensor_open() -> serde_json::Value {
        serde_json::json!({
            "LL": {
                "value": 1,
                "Code": "200"
            }
        })
    }

    pub fn window_sensor_closed() -> serde_json::Value {
        serde_json::json!({
            "LL": {
                "value": 0,
                "Code": "200"
            }
        })
    }
}

/// Sample device control responses
pub struct TestControlResponses;

impl TestControlResponses {
    pub fn light_on_success() -> serde_json::Value {
        serde_json::json!({
            "LL": {
                "control": "jdev/sps/io/Light/On",
                "value": "1",
                "Code": "200"
            }
        })
    }

    pub fn blinds_up_success() -> serde_json::Value {
        serde_json::json!({
            "LL": {
                "control": "jdev/sps/io/Jalousie/FullUp",
                "value": "1",
                "Code": "200"
            }
        })
    }

    pub fn error_response() -> serde_json::Value {
        serde_json::json!({
            "LL": {
                "control": "jdev/sps/io/Device/Action",
                "value": "Device not found",
                "Code": "404"
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest]
    fn test_fixture_loxone_config(test_loxone_config: LoxoneConfig) {
        assert_eq!(test_loxone_config.username, "test_user");
        assert!(!test_loxone_config.verify_ssl);
    }

    #[rstest]
    fn test_fixture_server_config(test_server_config: ServerConfig) {
        assert_eq!(test_server_config.credentials, CredentialStore::Environment);
    }

    #[test]
    fn test_device_uuids() {
        assert!(!TestDeviceUuids::LIVING_ROOM_LIGHT.is_empty());
        assert!(!TestRoomUuids::LIVING_ROOM.is_empty());
    }

    #[test]
    fn test_with_env_isolation() {
        with_test_env(|| {
            assert_eq!(std::env::var("LOXONE_USERNAME").unwrap(), "test_user");
        });
    }
}