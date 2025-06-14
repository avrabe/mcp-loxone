//! Tests for authentication method selection

use loxone_mcp_rust::client::create_client;
use loxone_mcp_rust::config::credentials::LoxoneCredentials;
use loxone_mcp_rust::config::{AuthMethod, LoxoneConfig};
use std::time::Duration;
use url::Url;

#[tokio::test]
async fn test_token_auth_selection() {
    let config = LoxoneConfig {
        url: Url::parse("http://192.168.1.100").unwrap(),
        username: "test".to_string(),
        verify_ssl: false,
        timeout: Duration::from_secs(30),
        max_retries: 3,
        max_connections: Some(10),
        #[cfg(feature = "websocket")]
        websocket: Default::default(),
        auth_method: AuthMethod::Token,
    };

    let credentials = LoxoneCredentials {
        username: "test".to_string(),
        password: "test".to_string(),
        api_key: None,
        #[cfg(feature = "crypto")]
        public_key: None,
    };

    let client = create_client(&config, &credentials).await;
    assert!(client.is_ok(), "Token client creation should succeed");

    // Verify it's the right type with crypto feature enabled
    #[cfg(feature = "crypto")]
    {
        let client = client.unwrap();
        let is_token_client = client
            .as_any()
            .is::<loxone_mcp_rust::client::token_http_client::TokenHttpClient>();
        assert!(
            is_token_client,
            "Should create TokenHttpClient when token auth is selected"
        );
    }
}

#[tokio::test]
async fn test_basic_auth_selection() {
    let config = LoxoneConfig {
        url: Url::parse("http://192.168.1.100").unwrap(),
        username: "test".to_string(),
        verify_ssl: false,
        timeout: Duration::from_secs(30),
        max_retries: 3,
        max_connections: Some(10),
        #[cfg(feature = "websocket")]
        websocket: Default::default(),
        auth_method: AuthMethod::Basic,
    };

    let credentials = LoxoneCredentials {
        username: "test".to_string(),
        password: "test".to_string(),
        api_key: None,
        #[cfg(feature = "crypto")]
        public_key: None,
    };

    let client = create_client(&config, &credentials).await;
    assert!(client.is_ok(), "Basic client creation should succeed");

    // Verify it's the basic HTTP client
    let client = client.unwrap();
    let is_basic_client = client
        .as_any()
        .is::<loxone_mcp_rust::client::http_client::LoxoneHttpClient>();
    assert!(
        is_basic_client,
        "Should create LoxoneHttpClient when basic auth is selected"
    );
}

#[tokio::test]
async fn test_default_auth_method() {
    // Test that default is Token
    let auth_method = AuthMethod::default();
    assert_eq!(
        auth_method,
        AuthMethod::Token,
        "Default auth method should be Token"
    );
}
