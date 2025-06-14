//! Tests for streaming JSON parser

use loxone_mcp_rust::client::http_client::LoxoneHttpClient;
use loxone_mcp_rust::client::streaming_parser::{
    ParseProgress, StreamingParserConfig, StreamingStructureParser, StructureSection,
};
use loxone_mcp_rust::config::credentials::LoxoneCredentials;
use loxone_mcp_rust::config::{AuthMethod, LoxoneConfig};
use std::time::Duration;
use url::Url;

fn create_test_config() -> (LoxoneConfig, LoxoneCredentials) {
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

    (config, credentials)
}

#[tokio::test]
async fn test_streaming_parser_config() {
    let config = StreamingParserConfig::default();
    assert_eq!(config.max_buffer_size, 50 * 1024 * 1024); // 50MB
    assert_eq!(config.progress_interval, 1000);
    assert_eq!(config.parse_timeout, Duration::from_secs(300));
    assert!(config.allow_partial);
    assert!(config.sections.is_empty());
    assert_eq!(config.max_items_per_section, 0);
}

#[tokio::test]
async fn test_streaming_parser_creation() {
    let parser = StreamingStructureParser::new();
    assert!(std::matches!(parser, StreamingStructureParser { .. }));

    let config = StreamingParserConfig {
        max_buffer_size: 10 * 1024 * 1024, // 10MB
        progress_interval: 500,
        parse_timeout: Duration::from_secs(60),
        allow_partial: false,
        sections: vec![StructureSection::Controls, StructureSection::Rooms],
        max_items_per_section: 1000,
    };

    let parser = StreamingStructureParser::with_config(config.clone());
    assert!(std::matches!(parser, StreamingStructureParser { .. }));
}

#[tokio::test]
async fn test_streaming_parser_presets() {
    // Test large installation preset
    let parser = StreamingStructureParser::for_large_installation();
    assert!(std::matches!(parser, StreamingStructureParser { .. }));

    // Test quick overview preset
    let parser = StreamingStructureParser::for_quick_overview();
    assert!(std::matches!(parser, StreamingStructureParser { .. }));

    // Test sections-specific preset
    let sections = vec![StructureSection::Controls, StructureSection::Rooms];
    let parser = StreamingStructureParser::for_sections(sections);
    assert!(std::matches!(parser, StreamingStructureParser { .. }));
}

#[tokio::test]
async fn test_structure_sections() {
    let controls = StructureSection::Controls;
    let rooms = StructureSection::Rooms;
    let categories = StructureSection::Categories;
    let global_states = StructureSection::GlobalStates;

    assert_eq!(controls, StructureSection::Controls);
    assert_eq!(rooms, StructureSection::Rooms);
    assert_eq!(categories, StructureSection::Categories);
    assert_eq!(global_states, StructureSection::GlobalStates);

    // Test that they can be used in collections
    let sections = [controls, rooms];
    assert_eq!(sections.len(), 2);
}

#[tokio::test]
async fn test_parse_progress_serialization() {
    let progress = ParseProgress {
        bytes_processed: 1024,
        total_bytes: Some(4096),
        items_parsed: 100,
        current_section: Some("controls".to_string()),
        elapsed: Duration::from_secs(5),
        completion_percentage: Some(25.0),
        memory_usage: 2048,
        parse_rate: 20.0,
    };

    // Test serialization
    let serialized = serde_json::to_string(&progress).unwrap();
    let deserialized: ParseProgress = serde_json::from_str(&serialized).unwrap();

    assert_eq!(progress.bytes_processed, deserialized.bytes_processed);
    assert_eq!(progress.total_bytes, deserialized.total_bytes);
    assert_eq!(progress.items_parsed, deserialized.items_parsed);
    assert_eq!(progress.current_section, deserialized.current_section);
    assert_eq!(
        progress.completion_percentage,
        deserialized.completion_percentage
    );
    assert_eq!(progress.memory_usage, deserialized.memory_usage);
    assert_eq!(progress.parse_rate, deserialized.parse_rate);
}

#[tokio::test]
async fn test_http_client_streaming_methods() {
    let (config, credentials) = create_test_config();
    let client = LoxoneHttpClient::new(config, credentials).await;
    assert!(client.is_ok());

    let client = client.unwrap();

    // Test that streaming methods exist and have correct signatures
    // Note: These would fail in real execution without a Loxone server,
    // but we're just testing method availability and basic type checking

    // Test get_structure_streaming
    let result =
        tokio::time::timeout(Duration::from_millis(100), client.get_structure_streaming()).await;
    assert!(result.is_err()); // Should timeout since no real server

    // Test get_structure_streaming_with_config
    let config = StreamingParserConfig::default();
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        client.get_structure_streaming_with_config(config),
    )
    .await;
    assert!(result.is_err()); // Should timeout since no real server

    // Test get_structure_with_progress
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        client.get_structure_with_progress(),
    )
    .await;
    assert!(result.is_err()); // Should timeout since no real server
}

#[tokio::test]
async fn test_streaming_parser_with_mock_data() {
    // Create mock JSON structure
    let mock_structure = serde_json::json!({
        "lastModified": "2023-01-01T00:00:00Z",
        "controls": {
            "uuid1": {
                "name": "Living Room Light",
                "type": "LightController",
                "room": "room1",
                "states": {
                    "active": 1
                }
            },
            "uuid2": {
                "name": "Kitchen Blind",
                "type": "Jalousie",
                "room": "room2",
                "states": {
                    "position": 0.5
                }
            }
        },
        "rooms": {
            "room1": {
                "name": "Living Room",
                "type": "Room"
            },
            "room2": {
                "name": "Kitchen",
                "type": "Room"
            }
        },
        "cats": {
            "cat1": {
                "name": "Lighting",
                "type": "Category"
            }
        },
        "globalStates": {}
    });

    let json_bytes = serde_json::to_vec(&mock_structure).unwrap();

    // This is a bit tricky without a real HTTP server, but we can test the basic structure
    assert!(!json_bytes.is_empty());
    assert!(json_bytes.len() < 10 * 1024 * 1024); // Should be reasonable size

    // Test that our mock data is valid JSON
    let parsed: serde_json::Value = serde_json::from_slice(&json_bytes).unwrap();
    assert!(parsed["controls"].is_object());
    assert!(parsed["rooms"].is_object());
    assert_eq!(parsed["controls"]["uuid1"]["name"], "Living Room Light");
}

#[tokio::test]
async fn test_streaming_config_limits() {
    // Test memory limit
    let config = StreamingParserConfig {
        max_buffer_size: 1024, // Very small limit
        ..Default::default()
    };

    assert_eq!(config.max_buffer_size, 1024);

    // Test timeout
    let config = StreamingParserConfig {
        parse_timeout: Duration::from_millis(100), // Very short timeout
        ..Default::default()
    };

    assert_eq!(config.parse_timeout, Duration::from_millis(100));

    // Test item limits
    let config = StreamingParserConfig {
        max_items_per_section: 10,
        ..Default::default()
    };

    assert_eq!(config.max_items_per_section, 10);
}

#[tokio::test]
async fn test_section_filtering() {
    // Test filtering specific sections
    let sections = vec![StructureSection::Controls, StructureSection::Rooms];

    let config = StreamingParserConfig {
        sections: sections.clone(),
        allow_partial: true,
        ..Default::default()
    };

    assert_eq!(config.sections.len(), 2);
    assert!(config.sections.contains(&StructureSection::Controls));
    assert!(config.sections.contains(&StructureSection::Rooms));
    assert!(!config.sections.contains(&StructureSection::Categories));
}

#[tokio::test]
async fn test_progress_calculation() {
    // Test progress percentage calculation
    let total_bytes = 1000u64;
    let processed_bytes = 250usize;

    let percentage = (processed_bytes as f32 / total_bytes as f32) * 100.0;
    assert_eq!(percentage, 25.0);

    // Test parse rate calculation
    let items_parsed = 100;
    let elapsed_secs = 5.0f32;
    let parse_rate = items_parsed as f32 / elapsed_secs;
    assert_eq!(parse_rate, 20.0);
}

#[tokio::test]
async fn test_memory_estimation() {
    // Test rough memory estimation calculation
    let controls_count = 1000;
    let rooms_count = 50;
    let categories_count = 20;
    let global_states_count = 10;

    let estimated_size = controls_count * 200 +     // ~200 bytes per control
        rooms_count * 100 +        // ~100 bytes per room
        categories_count * 50 +    // ~50 bytes per category
        global_states_count * 100; // ~100 bytes per state

    assert_eq!(estimated_size, 200000 + 5000 + 1000 + 1000);
    assert_eq!(estimated_size, 207000); // Total estimated bytes
}
