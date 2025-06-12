//! Unit tests for core components
//!
//! Tests individual modules and functions in isolation.

mod common;

use loxone_mcp_rust::{
    client::{ClientContext, LoxoneClient, LoxoneResponse},
    config::CredentialStore,
    error::LoxoneError,
    tools::{ToolResponse, DeviceFilter, ActionAliases},
};
use common::*;

// Configuration Tests
#[tokio::test]
async fn test_server_config_validation() {
    let mut config = create_test_config();
    
    // Valid configuration should pass
    assert!(config.validate().is_ok());
    
    // Invalid URL scheme should fail
    config.loxone.url = "ftp://invalid.url".parse().unwrap();
    assert!(config.validate().is_err());
    
    // Empty username should fail
    config.loxone.url = "http://valid.url".parse().unwrap();
    config.loxone.username = "".to_string();
    assert!(config.validate().is_err());
    
    // Zero timeout should fail
    config.loxone.username = "valid_user".to_string();
    config.loxone.timeout = std::time::Duration::from_secs(0);
    assert!(config.validate().is_err());
}

#[tokio::test]
async fn test_credential_store_types() {
    // Test different credential store types
    let env_store = CredentialStore::Environment;
    assert!(matches!(env_store, CredentialStore::Environment));
    
    let file_store = CredentialStore::FileSystem { 
        path: "/tmp/test_creds.json".to_string() 
    };
    assert!(matches!(file_store, CredentialStore::FileSystem { .. }));
}

// Error Handling Tests
#[tokio::test]
async fn test_error_types() {
    let connection_err = LoxoneError::connection("Test connection error");
    assert!(connection_err.is_retryable());
    assert!(!connection_err.is_auth_error());
    
    let auth_err = LoxoneError::authentication("Test auth error");
    assert!(!auth_err.is_retryable());
    assert!(auth_err.is_auth_error());
    
    let timeout_err = LoxoneError::timeout("Test timeout");
    assert!(timeout_err.is_retryable());
    assert!(!timeout_err.is_auth_error());
}

#[tokio::test]
async fn test_error_display() {
    let error = LoxoneError::device_control("Device not responding");
    let error_string = format!("{}", error);
    assert!(error_string.contains("Device control error"));
    assert!(error_string.contains("Device not responding"));
}

// Client Context Tests
#[tokio::test]
async fn test_client_context_device_management() {
    let context = ClientContext::new();
    
    // Initially empty
    let devices = context.devices.read().await;
    assert!(devices.is_empty());
    drop(devices);
    
    // Add test structure
    let structure = create_test_structure();
    context.update_structure(structure).await.unwrap();
    
    // Check devices were parsed
    let devices = context.devices.read().await;
    assert!(!devices.is_empty());
    assert!(devices.contains_key("light-1"));
    assert!(devices.contains_key("blind-1"));
    drop(devices);
    
    // Test device retrieval
    let device = context.get_device("light-1").await.unwrap();
    assert!(device.is_some());
    assert_eq!(device.unwrap().name, "Living Room Light");
    
    // Test device not found
    let missing = context.get_device("nonexistent").await.unwrap();
    assert!(missing.is_none());
}

#[tokio::test]
async fn test_client_context_room_filtering() {
    let context = ClientContext::new();
    let structure = create_test_structure();
    context.update_structure(structure).await.unwrap();
    
    // Test room filtering
    let living_room_devices = context.get_devices_by_room("Living Room").await.unwrap();
    assert_eq!(living_room_devices.len(), 2); // light and temperature
    
    let kitchen_devices = context.get_devices_by_room("Kitchen").await.unwrap();
    assert_eq!(kitchen_devices.len(), 1); // blind
    
    let empty_room = context.get_devices_by_room("Nonexistent Room").await.unwrap();
    assert!(empty_room.is_empty());
}

#[tokio::test]
async fn test_client_context_category_filtering() {
    let context = ClientContext::new();
    let structure = create_test_structure();
    context.update_structure(structure).await.unwrap();
    
    // Test category filtering
    let lighting_devices = context.get_devices_by_category("lighting").await.unwrap();
    assert_eq!(lighting_devices.len(), 1);
    
    let climate_devices = context.get_devices_by_category("climate").await.unwrap();
    assert_eq!(climate_devices.len(), 1);
    
    let unknown_category = context.get_devices_by_category("unknown").await.unwrap();
    assert!(unknown_category.is_empty());
}

// Tool Response Tests
#[tokio::test]
async fn test_tool_response_creation() {
    let data = serde_json::json!({"test": "data"});
    
    // Success response
    let success = ToolResponse::success(data.clone());
    assert_eq!(success.status, "success");
    assert_eq!(success.data, data);
    assert!(success.message.is_none());
    
    // Success with message
    let success_msg = ToolResponse::success_with_message(data.clone(), "Test message".to_string());
    assert_eq!(success_msg.status, "success");
    assert_eq!(success_msg.message, Some("Test message".to_string()));
    
    // Error response
    let error = ToolResponse::error("Test error".to_string());
    assert_eq!(error.status, "error");
    assert_eq!(error.data, serde_json::Value::Null);
    assert_eq!(error.message, Some("Test error".to_string()));
}

#[tokio::test]
async fn test_tool_response_from_result() {
    // Success result
    let success_result: Result<String, LoxoneError> = Ok("success_data".to_string());
    let response = ToolResponse::from_result(success_result);
    assert_eq!(response.status, "success");
    
    // Error result
    let error_result: Result<String, LoxoneError> = Err(LoxoneError::device_control("Test error"));
    let response = ToolResponse::from_result(error_result);
    assert_eq!(response.status, "error");
    assert!(response.message.unwrap().contains("Test error"));
}

// Device Filter Tests
#[tokio::test]
async fn test_device_filter_matching() {
    let device = create_test_device("test-1", "Test Light", "LightController", Some("Living Room"));
    
    // No filter should match
    let no_filter = DeviceFilter {
        device_type: None,
        category: None,
        room: None,
        limit: None,
    };
    assert!(no_filter.matches(&device));
    
    // Matching device type
    let type_filter = DeviceFilter {
        device_type: Some("LightController".to_string()),
        category: None,
        room: None,
        limit: None,
    };
    assert!(type_filter.matches(&device));
    
    // Non-matching device type
    let wrong_type_filter = DeviceFilter {
        device_type: Some("Jalousie".to_string()),
        category: None,
        room: None,
        limit: None,
    };
    assert!(!wrong_type_filter.matches(&device));
    
    // Matching room
    let room_filter = DeviceFilter {
        device_type: None,
        category: None,
        room: Some("Living Room".to_string()),
        limit: None,
    };
    assert!(room_filter.matches(&device));
    
    // Non-matching room
    let wrong_room_filter = DeviceFilter {
        device_type: None,
        category: None,
        room: Some("Kitchen".to_string()),
        limit: None,
    };
    assert!(!wrong_room_filter.matches(&device));
}

// Action Aliases Tests
#[tokio::test]
async fn test_action_aliases_normalization() {
    // German to English
    assert_eq!(ActionAliases::normalize_action("hoch"), "up");
    assert_eq!(ActionAliases::normalize_action("runter"), "down");
    assert_eq!(ActionAliases::normalize_action("an"), "on");
    assert_eq!(ActionAliases::normalize_action("aus"), "off");
    
    // English passthrough
    assert_eq!(ActionAliases::normalize_action("on"), "on");
    assert_eq!(ActionAliases::normalize_action("OFF"), "off");
    assert_eq!(ActionAliases::normalize_action("Up"), "up");
    
    // Unknown actions passthrough
    assert_eq!(ActionAliases::normalize_action("custom"), "custom");
}

#[tokio::test]
async fn test_action_aliases_valid_actions() {
    let light_actions = ActionAliases::get_valid_actions("LightController");
    assert!(light_actions.contains(&"on"));
    assert!(light_actions.contains(&"off"));
    assert!(light_actions.contains(&"dim"));
    
    let blind_actions = ActionAliases::get_valid_actions("Jalousie");
    assert!(blind_actions.contains(&"up"));
    assert!(blind_actions.contains(&"down"));
    assert!(blind_actions.contains(&"stop"));
    
    let switch_actions = ActionAliases::get_valid_actions("Switch");
    assert!(switch_actions.contains(&"on"));
    assert!(switch_actions.contains(&"off"));
}

// Sensor Configuration Tests
// TODO: Re-enable when SensorConfig and ConfiguredSensor are implemented
/*
#[tokio::test]
async fn test_sensor_config_management() {
    let mut config = SensorConfig::new();
    assert!(config.sensors.is_empty());
    
    // Add sensor
    let sensor = ConfiguredSensor::new(
        "sensor-1".to_string(),
        "Test Sensor".to_string(),
        "door_window".to_string(),
    );
    
    config.add_sensor(sensor.clone()).unwrap();
    assert_eq!(config.sensors.len(), 1);
    
    // Duplicate UUID should fail
    let duplicate = ConfiguredSensor::new(
        "sensor-1".to_string(),
        "Duplicate Sensor".to_string(),
        "motion".to_string(),
    );
    assert!(config.add_sensor(duplicate).is_err());
    
    // Remove sensor
    let removed = config.remove_sensor("sensor-1").unwrap();
    assert!(removed);
    assert!(config.sensors.is_empty());
    
    // Remove non-existent sensor
    let not_removed = config.remove_sensor("nonexistent").unwrap();
    assert!(!not_removed);
}
*/

/*
#[tokio::test]
async fn test_sensor_config_filtering() {
    let mut config = SensorConfig::new();
    
    // Add test sensors
    let sensors = vec![
        ConfiguredSensor::new("sensor-1".to_string(), "Door Sensor".to_string(), "door_window".to_string())
            .with_room("Living Room".to_string()),
        ConfiguredSensor::new("sensor-2".to_string(), "Motion Sensor".to_string(), "motion".to_string())
            .with_room("Kitchen".to_string()),
        ConfiguredSensor::new("sensor-3".to_string(), "Window Sensor".to_string(), "door_window".to_string())
            .with_room("Living Room".to_string())
            .with_enabled(false),
    ];
    
    for sensor in sensors {
        config.add_sensor(sensor).unwrap();
    }
    
    // Test enabled sensors
    let enabled = config.enabled_sensors();
    assert_eq!(enabled.len(), 2);
    
    // Test by type
    let door_window_sensors = config.sensors_by_type("door_window");
    assert_eq!(door_window_sensors.len(), 2);
    
    let motion_sensors = config.sensors_by_type("motion");
    assert_eq!(motion_sensors.len(), 1);
    
    // Test by room
    let living_room_sensors = config.sensors_by_room("Living Room");
    assert_eq!(living_room_sensors.len(), 2);
    
    let kitchen_sensors = config.sensors_by_room("Kitchen");
    assert_eq!(kitchen_sensors.len(), 1);
}

#[tokio::test]
async fn test_sensor_config_statistics() {
    let mut config = SensorConfig::new();
    
    // Add test sensors
    let sensors = vec![
        ConfiguredSensor::new("s1".to_string(), "Sensor 1".to_string(), "door_window".to_string())
            .with_room("Room 1".to_string()),
        ConfiguredSensor::new("s2".to_string(), "Sensor 2".to_string(), "motion".to_string())
            .with_room("Room 1".to_string()),
        ConfiguredSensor::new("s3".to_string(), "Sensor 3".to_string(), "door_window".to_string())
            .with_room("Room 2".to_string())
            .with_enabled(false),
    ];
    
    for sensor in sensors {
        config.add_sensor(sensor).unwrap();
    }
    
    let stats = config.statistics();
    assert_eq!(stats.total_sensors, 3);
    assert_eq!(stats.enabled_sensors, 2);
    assert_eq!(stats.disabled_sensors, 1);
    assert_eq!(stats.unique_types, 2);
    assert_eq!(stats.unique_rooms, 2);
    assert_eq!(stats.by_type.get("door_window"), Some(&2));
    assert_eq!(stats.by_type.get("motion"), Some(&1));
    assert_eq!(stats.by_room.get("Room 1"), Some(&2));
    assert_eq!(stats.by_room.get("Room 2"), Some(&1));
}
*/

// Mock Client Tests
#[tokio::test]
async fn test_mock_client_basic_operations() {
    let mut mock_client = MockLoxoneClient::new();
    
    // Initially not connected
    assert!(!mock_client.is_connected().await.unwrap());
    
    // Connect
    mock_client.connect().await.unwrap();
    assert!(mock_client.is_connected().await.unwrap());
    
    // Send command
    let response = mock_client.send_command("test-uuid", "on").await.unwrap();
    assert_eq!(response.code, 200);
    
    // Check command history
    let history = mock_client.get_command_history().await;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0], ("test-uuid".to_string(), "on".to_string()));
    
    // Disconnect
    mock_client.disconnect().await.unwrap();
    assert!(!mock_client.is_connected().await.unwrap());
}

#[tokio::test]
async fn test_mock_client_failure_simulation() {
    let mut mock_client = MockLoxoneClient::new();
    
    // Enable failure simulation
    mock_client.simulate_failures(true).await;
    
    // Connection should fail
    assert!(mock_client.connect().await.is_err());
    
    // Manually connect for command testing
    *mock_client.connected.write().await = true;
    
    // Commands should fail
    assert!(mock_client.send_command("test", "on").await.is_err());
    
    // Health check should fail
    assert!(!mock_client.health_check().await.unwrap());
    
    // Disable failure simulation
    mock_client.simulate_failures(false).await;
    
    // Health check should succeed
    assert!(mock_client.health_check().await.unwrap());
}

#[tokio::test]
async fn test_mock_client_response_overrides() {
    let mock_client = MockLoxoneClient::new();
    *mock_client.connected.write().await = true;
    
    // Set custom response
    let custom_response = LoxoneResponse {
        code: 404,
        value: serde_json::json!({"error": "Device not found"}),
    };
    
    mock_client.set_response_override("test-uuid:on".to_string(), custom_response.clone()).await;
    
    // Command should return custom response
    let response = mock_client.send_command("test-uuid", "on").await.unwrap();
    assert_eq!(response.code, 404);
    assert_eq!(response.value, custom_response.value);
    
    // Different command should return default response
    let default_response = mock_client.send_command("test-uuid", "off").await.unwrap();
    assert_eq!(default_response.code, 200);
}