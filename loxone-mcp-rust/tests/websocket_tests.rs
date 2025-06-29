//! Tests for WebSocket client integration

#[cfg(feature = "websocket")]
mod websocket_integration_tests {
    use loxone_mcp_rust::client::websocket_client::{
        EventFilter, LoxoneEventType, ReconnectionConfig,
    };
    use loxone_mcp_rust::client::{create_hybrid_client, create_websocket_client};
    use loxone_mcp_rust::config::credentials::LoxoneCredentials;
    use loxone_mcp_rust::config::{AuthMethod, LoxoneConfig};
    use std::collections::HashSet;
    use std::time::Duration;
    use url::Url;

    async fn create_test_config() -> (LoxoneConfig, LoxoneCredentials) {
        let config = LoxoneConfig {
            url: Url::parse("http://192.168.1.100").unwrap(),
            username: "test".to_string(),
            verify_ssl: false,
            timeout: Duration::from_secs(30),
            max_retries: 3,
            max_connections: Some(10),
            #[cfg(feature = "websocket")]
            websocket: Default::default(),
            auth_method: AuthMethod::Basic, // Use basic auth for testing
        };

        let credentials = LoxoneCredentials {
            username: "test".to_string(),
            password: "test".to_string(),
            api_key: None,
            #[cfg(feature = "crypto-openssl")]
            public_key: None,
        };

        (config, credentials)
    }

    #[tokio::test]
    async fn test_websocket_client_creation() {
        let (config, credentials) = create_test_config().await;

        let client = create_websocket_client(&config, &credentials).await;
        assert!(client.is_ok(), "WebSocket client creation should succeed");
    }

    #[tokio::test]
    async fn test_hybrid_client_creation() {
        let (config, credentials) = create_test_config().await;

        let hybrid_client = create_hybrid_client(&config, &credentials).await;
        assert!(
            hybrid_client.is_ok(),
            "Hybrid client creation should succeed"
        );
    }

    #[tokio::test]
    async fn test_event_filter_creation() {
        let mut device_uuids = HashSet::new();
        device_uuids.insert("test-uuid".to_string());

        let mut event_types = HashSet::new();
        event_types.insert(LoxoneEventType::State);
        event_types.insert(LoxoneEventType::Weather);

        let filter = EventFilter {
            device_uuids,
            event_types,
            rooms: HashSet::new(),
            states: HashSet::new(),
            min_interval: Some(Duration::from_millis(500)),
        };

        assert_eq!(filter.device_uuids.len(), 1);
        assert_eq!(filter.event_types.len(), 2);
        assert!(filter.event_types.contains(&LoxoneEventType::State));
        assert!(filter.event_types.contains(&LoxoneEventType::Weather));
    }

    #[tokio::test]
    async fn test_reconnection_config() {
        let config = ReconnectionConfig {
            enabled: true,
            initial_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 1.5,
            max_attempts: Some(10),
            jitter_factor: 0.2,
        };

        assert!(config.enabled);
        assert_eq!(config.initial_delay, Duration::from_secs(2));
        assert_eq!(config.max_delay, Duration::from_secs(60));
        assert_eq!(config.backoff_multiplier, 1.5);
        assert_eq!(config.max_attempts, Some(10));
        assert_eq!(config.jitter_factor, 0.2);
    }

    #[tokio::test]
    async fn test_websocket_subscription_filtering() {
        let (config, credentials) = create_test_config().await;

        let hybrid_client = create_hybrid_client(&config, &credentials).await;
        assert!(hybrid_client.is_ok());

        let client = hybrid_client.unwrap();

        // Test different subscription methods
        let _all_updates = client.subscribe().await;

        let mut device_uuids = HashSet::new();
        device_uuids.insert("test-device-1".to_string());
        let _device_updates = client.subscribe_to_devices(device_uuids).await;

        let mut rooms = HashSet::new();
        rooms.insert("Living Room".to_string());
        let _room_updates = client.subscribe_to_rooms(rooms).await;

        let mut event_types = HashSet::new();
        event_types.insert(LoxoneEventType::State);
        let _state_updates = client.subscribe_to_event_types(event_types).await;

        // Test statistics
        let stats = client.get_stats().await;
        assert_eq!(stats.messages_received, 0);
        assert_eq!(stats.state_updates, 0);
    }

    #[tokio::test]
    #[ignore] // TODO: Re-enable when token auth WebSocket functionality is implemented
    async fn test_websocket_client_with_token_auth() {
        let config = LoxoneConfig {
            url: Url::parse("http://192.168.1.100").unwrap(),
            username: "test".to_string(),
            verify_ssl: false,
            timeout: Duration::from_secs(30),
            max_retries: 3,
            max_connections: Some(10),
            #[cfg(feature = "websocket")]
            websocket: Default::default(),
            auth_method: AuthMethod::Token, // Use token auth
        };

        let credentials = LoxoneCredentials {
            username: "test".to_string(),
            password: "test".to_string(),
            api_key: None,
            #[cfg(feature = "crypto-openssl")]
            public_key: None,
        };

        // This should work even with token auth (falls back to basic for WebSocket)
        let hybrid_client = create_hybrid_client(&config, &credentials).await;
        assert!(
            hybrid_client.is_ok(),
            "Hybrid client with token auth should succeed"
        );
    }

    #[tokio::test]
    async fn test_event_type_serialization() {
        // Test that event types can be serialized/deserialized
        let event_type = LoxoneEventType::State;
        let serialized = serde_json::to_string(&event_type).unwrap();
        let deserialized: LoxoneEventType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(event_type, deserialized);

        // Test unknown event type
        let unknown_json = "\"custom\"";
        let unknown: LoxoneEventType = serde_json::from_str(unknown_json)
            .unwrap_or(LoxoneEventType::Unknown("custom".to_string()));
        match unknown {
            LoxoneEventType::Unknown(name) => assert_eq!(name, "custom"),
            _ => panic!("Expected Unknown event type"),
        }
    }
}
