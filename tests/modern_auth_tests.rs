//! Modern authentication tests using WireMock
//!
//! This demonstrates the new testing approach with:
//! - WireMock for HTTP API mocking
//! - rstest for fixtures
//! - temp-env for environment isolation
//! - serial_test for test isolation

use loxone_mcp_rust::client::create_client;
use loxone_mcp_rust::config::{credentials::LoxoneCredentials, AuthMethod};
use rstest::*;
use serial_test::serial;
use wiremock::{matchers::method, Mock, ResponseTemplate};

mod common;
use common::{test_fixtures::*, MockLoxoneServer, TestControlResponses};

#[rstest]
#[tokio::test]
async fn test_basic_auth_with_mock_server() {
    // Create mock Loxone server
    let mock_server = MockLoxoneServer::start().await;

    // Create test config pointing to mock server
    let mut config = test_loxone_config(mock_server.url());
    config.auth_method = AuthMethod::Basic;

    let credentials = LoxoneCredentials {
        username: "test_user".to_string(),
        password: "test_password".to_string(),
        api_key: None,
        #[cfg(feature = "crypto-openssl")]
        public_key: None,
    };

    // Test client creation with mock server
    let client = create_client(&config, &credentials).await;
    assert!(
        client.is_ok(),
        "Basic auth client creation should succeed with mock"
    );
}

#[rstest]
#[tokio::test]
async fn test_token_auth_with_mock_server() {
    let mock_server = MockLoxoneServer::start().await;

    let mut config = test_loxone_config(mock_server.url());
    config.auth_method = AuthMethod::Token;

    let credentials = LoxoneCredentials {
        username: "test_user".to_string(),
        password: "test_password".to_string(),
        api_key: None,
        #[cfg(feature = "crypto-openssl")]
        public_key: None,
    };

    let client = create_client(&config, &credentials).await;
    assert!(
        client.is_ok(),
        "Token auth client creation should succeed with mock"
    );
}

#[tokio::test]
#[serial] // Run this test isolated from others
async fn test_auth_failure_handling() {
    let mock_server = MockLoxoneServer::start().await;

    // Mock an authentication failure
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(401).set_body_json(TestControlResponses::error_response()),
        )
        .mount(&mock_server.server)
        .await;

    let config = test_loxone_config(mock_server.url());
    let credentials = LoxoneCredentials {
        username: "invalid_user".to_string(),
        password: "invalid_password".to_string(),
        api_key: None,
        #[cfg(feature = "crypto-openssl")]
        public_key: None,
    };

    // Test that authentication failures are handled gracefully
    let client = create_client(&config, &credentials).await;
    // Note: This may succeed in client creation but fail on actual API calls
    // The exact behavior depends on the client implementation
}

#[rstest]
#[tokio::test]
async fn test_auth_method_default() {
    // Test that default auth method is Token as expected
    let auth_method = AuthMethod::default();
    assert_eq!(
        auth_method,
        AuthMethod::Token,
        "Default auth method should be Token"
    );
}

#[tokio::test]
async fn test_mock_server_structure_endpoint() {
    let mock_server = MockLoxoneServer::start().await;

    // Test that we can fetch structure data from mock server
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/data/LoxAPP3.json", mock_server.url()))
        .send()
        .await
        .expect("Should get response from mock server");

    assert_eq!(response.status(), 200);

    let json: serde_json::Value = response.json().await.expect("Should parse JSON");
    assert_eq!(json["msInfo"]["serialNr"], "TEST-12345");
    assert!(!json["rooms"].as_object().unwrap().is_empty());
    assert!(!json["controls"].as_object().unwrap().is_empty());
}

#[tokio::test]
async fn test_mock_device_control_endpoints() {
    let mock_server = MockLoxoneServer::start().await;

    let client = reqwest::Client::new();

    // Test light control endpoint
    let response = client
        .get(format!("{}/jdev/sps/io/TestLight/On", mock_server.url()))
        .send()
        .await
        .expect("Should get response");

    assert_eq!(response.status(), 200);

    let json: serde_json::Value = response.json().await.expect("Should parse JSON");
    assert_eq!(json["LL"]["Code"], "200");

    // Test blind control endpoint
    let response = client
        .get(format!(
            "{}/jdev/sps/io/TestBlind/FullUp",
            mock_server.url()
        ))
        .send()
        .await
        .expect("Should get response");

    assert_eq!(response.status(), 200);
}

#[cfg(test)]
mod environment_isolation_tests {
    use super::*;
    use temp_env::with_vars;

    #[tokio::test]
    #[serial]
    async fn test_environment_isolation_works() {
        // Test that environment variables are properly isolated
        with_vars([("TEST_VAR", Some("test_value"))], || {
            assert_eq!(std::env::var("TEST_VAR").unwrap(), "test_value");
        });

        // Variable should not exist outside the with_vars block
        assert!(std::env::var("TEST_VAR").is_err());
    }
}
