//! Integration tests for MCP tools and server functionality
//!
//! Tests the complete tool functionality with mock clients.

mod common;

use loxone_mcp_rust::{
    tools::ToolContext,
    client::{LoxoneResponse, ClientContext},
    tools::{rooms, devices, climate, ActionAliases},
};
use common::*;
use std::sync::Arc;

async_test!(test_room_management_tools, {
    let (mock_client, context) = setup_integration_test().await;
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Test list_rooms
    let response = rooms::list_rooms(tool_context.clone()).await;
    assert_eq!(response.status, "success");
    
    let rooms = response.data.as_array().unwrap();
    assert_eq!(rooms.len(), 2);
    
    // Verify room names
    let room_names: Vec<&str> = rooms.iter()
        .map(|r| r.get("name").unwrap().as_str().unwrap())
        .collect();
    assert!(room_names.contains(&"Living Room"));
    assert!(room_names.contains(&"Kitchen"));
});

async_test!(test_get_room_devices, {
    let (mock_client, context) = setup_integration_test().await;
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Test getting devices in Living Room
    let response = rooms::get_room_devices(
        tool_context.clone(),
        "Living Room".to_string(),
        None,
        None
    ).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    assert_eq!(data.get("room").unwrap().as_str().unwrap(), "Living Room");
    
    let devices = data.get("devices").unwrap().as_array().unwrap();
    assert_eq!(devices.len(), 2); // Light and temperature sensor
    
    // Test with category filter
    let lighting_response = rooms::get_room_devices(
        tool_context.clone(),
        "Living Room".to_string(),
        Some("lighting".to_string()),
        None
    ).await;
    
    assert_eq!(lighting_response.status, "success");
    let lighting_devices = lighting_response.data.get("devices").unwrap().as_array().unwrap();
    assert_eq!(lighting_devices.len(), 1);
});

async_test!(test_get_room_devices_nonexistent, {
    let (mock_client, context) = setup_integration_test().await;
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Test getting devices in non-existent room
    let response = rooms::get_room_devices(
        tool_context.clone(),
        "Nonexistent Room".to_string(),
        None,
        None
    ).await;
    
    assert_eq!(response.status, "error");
    assert!(response.message.unwrap().contains("not found"));
});

async_test!(test_control_room_lights, {
    let (mock_client, context) = setup_integration_test().await;
    let client_arc = Arc::new(mock_client);
    let tool_context = ToolContext::new(client_arc.clone(), Arc::new(context));
    
    // Test controlling lights in Living Room
    let response = rooms::control_room_lights(
        tool_context.clone(),
        "Living Room".to_string(),
        "on".to_string()
    ).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    assert_eq!(data.get("room").unwrap().as_str().unwrap(), "Living Room");
    assert_eq!(data.get("action").unwrap().as_str().unwrap(), "on");
    assert_eq!(data.get("successful").unwrap().as_u64().unwrap(), 1);
    assert_eq!(data.get("failed").unwrap().as_u64().unwrap(), 0);
    
    // Verify command was sent
    assertions::assert_command_sent(&client_arc, "light-1", "on").await;
});

async_test!(test_control_room_lights_no_lights, {
    let (mock_client, context) = setup_integration_test().await;
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Test controlling lights in room with no lights (Kitchen)
    let response = rooms::control_room_lights(
        tool_context.clone(),
        "Kitchen".to_string(),
        "on".to_string()
    ).await;
    
    assert_eq!(response.status, "error");
    assert!(response.message.unwrap().contains("No lights found"));
});

async_test!(test_control_room_rolladen, {
    let (mock_client, context) = setup_integration_test().await;
    let client_arc = Arc::new(mock_client);
    let tool_context = ToolContext::new(client_arc.clone(), Arc::new(context));
    
    // Test controlling blinds in Kitchen
    let response = rooms::control_room_rolladen(
        tool_context.clone(),
        "Kitchen".to_string(),
        "up".to_string()
    ).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    assert_eq!(data.get("action").unwrap().as_str().unwrap(), "up");
    assert_eq!(data.get("successful").unwrap().as_u64().unwrap(), 1);
    
    // Verify command was sent
    assertions::assert_command_sent(&client_arc, "blind-1", "up").await;
});

async_test!(test_discover_all_devices, {
    let (mock_client, context) = setup_integration_test().await;
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Test discovering all devices
    let response = devices::discover_all_devices(
        tool_context.clone(),
        None,
        None,
        None
    ).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    let devices = data.get("devices").unwrap().as_array().unwrap();
    assert_eq!(devices.len(), 3);
    
    let stats = data.get("statistics").unwrap();
    assert_eq!(stats.get("total_devices").unwrap().as_u64().unwrap(), 3);
});

async_test!(test_discover_devices_with_filter, {
    let (mock_client, context) = setup_integration_test().await;
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Test discovering devices with category filter
    let response = devices::discover_all_devices(
        tool_context.clone(),
        Some("lighting".to_string()),
        None,
        None
    ).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    let devices = data.get("devices").unwrap().as_array().unwrap();
    assert_eq!(devices.len(), 1);
    
    // Verify it's the light device
    let device = &devices[0];
    assert_eq!(device.get("name").unwrap().as_str().unwrap(), "Living Room Light");
});

async_test!(test_control_device, {
    let (mock_client, context) = setup_integration_test().await;
    let client_arc = Arc::new(mock_client);
    let tool_context = ToolContext::new(client_arc.clone(), Arc::new(context));
    
    // Test controlling device by name
    let response = devices::control_device(
        tool_context.clone(),
        "Living Room Light".to_string(),
        "on".to_string()
    ).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    assert_eq!(data.get("device").unwrap().as_str().unwrap(), "Living Room Light");
    assert_eq!(data.get("action").unwrap().as_str().unwrap(), "on");
    assert!(data.get("success").unwrap().as_bool().unwrap());
    
    // Verify command was sent
    assertions::assert_command_sent(&client_arc, "light-1", "on").await;
});

async_test!(test_control_device_by_uuid, {
    let (mock_client, context) = setup_integration_test().await;
    let client_arc = Arc::new(mock_client);
    let tool_context = ToolContext::new(client_arc.clone(), Arc::new(context));
    
    // Test controlling device by UUID
    let response = devices::control_device(
        tool_context.clone(),
        "light-1".to_string(),
        "off".to_string()
    ).await;
    
    assert_eq!(response.status, "success");
    assertions::assert_command_sent(&client_arc, "light-1", "off").await;
});

async_test!(test_control_device_not_found, {
    let (mock_client, context) = setup_integration_test().await;
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Test controlling non-existent device
    let response = devices::control_device(
        tool_context.clone(),
        "Nonexistent Device".to_string(),
        "on".to_string()
    ).await;
    
    assert_eq!(response.status, "error");
    assert!(response.message.unwrap().contains("not found"));
});

async_test!(test_control_device_invalid_action, {
    let (mock_client, context) = setup_integration_test().await;
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Test invalid action for light device
    let response = devices::control_device(
        tool_context.clone(),
        "Living Room Light".to_string(),
        "invalid_action".to_string()
    ).await;
    
    assert_eq!(response.status, "error");
    assert!(response.message.unwrap().contains("Invalid action"));
});

async_test!(test_control_multiple_devices, {
    let (mock_client, context) = setup_integration_test().await;
    let client_arc = Arc::new(mock_client);
    let tool_context = ToolContext::new(client_arc.clone(), Arc::new(context));
    
    // Test controlling multiple devices
    let response = devices::control_multiple_devices(
        tool_context.clone(),
        vec!["Living Room Light".to_string(), "Kitchen Blind".to_string()],
        "on".to_string()
    ).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    assert_eq!(data.get("total_devices").unwrap().as_u64().unwrap(), 2);
    assert_eq!(data.get("successful").unwrap().as_u64().unwrap(), 2);
    assert_eq!(data.get("failed").unwrap().as_u64().unwrap(), 0);
    
    // Verify commands were sent
    assertions::assert_command_sent(&client_arc, "light-1", "on").await;
    assertions::assert_command_sent(&client_arc, "blind-1", "on").await;
});

async_test!(test_control_all_lights, {
    let (mock_client, context) = setup_integration_test().await;
    let client_arc = Arc::new(mock_client);
    let tool_context = ToolContext::new(client_arc.clone(), Arc::new(context));
    
    // Test controlling all lights
    let response = devices::control_all_lights(
        tool_context.clone(),
        "on".to_string()
    ).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    assert_eq!(data.get("action").unwrap().as_str().unwrap(), "on");
    assert_eq!(data.get("successful").unwrap().as_u64().unwrap(), 1);
    assert_eq!(data.get("failed").unwrap().as_u64().unwrap(), 0);
    
    // Verify command was sent to light
    assertions::assert_command_sent(&client_arc, "light-1", "on").await;
});

async_test!(test_control_all_rolladen, {
    let (mock_client, context) = setup_integration_test().await;
    let client_arc = Arc::new(mock_client);
    let tool_context = ToolContext::new(client_arc.clone(), Arc::new(context));
    
    // Test controlling all blinds
    let response = devices::control_all_rolladen(
        tool_context.clone(),
        "down".to_string()
    ).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    assert_eq!(data.get("action").unwrap().as_str().unwrap(), "down");
    assert_eq!(data.get("successful").unwrap().as_u64().unwrap(), 1);
    
    // Verify command was sent to blind
    assertions::assert_command_sent(&client_arc, "blind-1", "down").await;
});

async_test!(test_get_climate_control, {
    let (mock_client, context) = setup_integration_test().await;
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Test getting climate control overview
    let response = climate::get_climate_control(tool_context.clone()).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    assert_eq!(data.get("total_devices").unwrap().as_u64().unwrap(), 1);
    
    let room_controllers = data.get("room_controllers").unwrap().as_array().unwrap();
    assert_eq!(room_controllers.len(), 1);
    
    let controller = &room_controllers[0];
    assert_eq!(controller.get("name").unwrap().as_str().unwrap(), "Living Room Temperature");
    assert!(controller.get("current_temperature").is_some());
    assert!(controller.get("target_temperature").is_some());
});

async_test!(test_get_room_climate, {
    let (mock_client, context) = setup_integration_test().await;
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Test getting climate for specific room
    let response = climate::get_room_climate(
        tool_context.clone(),
        "Living Room".to_string()
    ).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    assert_eq!(data.get("room").unwrap().as_str().unwrap(), "Living Room");
    assert!(data.get("has_controller").unwrap().as_bool().unwrap());
    assert!(data.get("room_controller").is_some());
});

async_test!(test_set_room_temperature, {
    let (mock_client, context) = setup_integration_test().await;
    let client_arc = Arc::new(mock_client);
    let tool_context = ToolContext::new(client_arc.clone(), Arc::new(context));
    
    // Test setting room temperature
    let response = climate::set_room_temperature(
        tool_context.clone(),
        "Living Room".to_string(),
        22.5
    ).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    assert_eq!(data.get("room").unwrap().as_str().unwrap(), "Living Room");
    assert_eq!(data.get("target_temperature").unwrap().as_f64().unwrap(), 22.5);
    
    // Verify command was sent
    assertions::assert_command_sent(&client_arc, "temp-1", "setpoint/22.5").await;
});

async_test!(test_set_room_temperature_invalid, {
    let (mock_client, context) = setup_integration_test().await;
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Test invalid temperature (too high)
    let response = climate::set_room_temperature(
        tool_context.clone(),
        "Living Room".to_string(),
        50.0
    ).await;
    
    assert_eq!(response.status, "error");
    assert!(response.message.unwrap().contains("Invalid temperature"));
});

async_test!(test_get_temperature_readings, {
    let (mock_client, context) = setup_integration_test().await;
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Test getting temperature readings
    let response = climate::get_temperature_readings(tool_context.clone()).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    let readings = data.get("readings").unwrap().as_array().unwrap();
    assert_eq!(readings.len(), 1);
    
    let reading = &readings[0];
    assert_eq!(reading.get("device").unwrap().as_str().unwrap(), "Living Room Temperature");
    assert!(reading.get("temperature").is_some());
    
    let stats = data.get("statistics").unwrap();
    assert_eq!(stats.get("total_sensors").unwrap().as_u64().unwrap(), 1);
});

async_test!(test_action_normalization, {
    // Test German action normalization
    assert_eq!(
        ActionAliases::normalize_action("hoch"),
        "up"
    );
    assert_eq!(
        ActionAliases::normalize_action("runter"),
        "down"
    );
    assert_eq!(
        ActionAliases::normalize_action("an"),
        "on"
    );
    
    // Test that tools use normalized actions
    let (mock_client, context) = setup_integration_test().await;
    let client_arc = Arc::new(mock_client);
    let tool_context = ToolContext::new(client_arc.clone(), Arc::new(context));
    
    // Use German action
    let response = devices::control_device(
        tool_context.clone(),
        "Living Room Light".to_string(),
        "an".to_string()  // German for "on"
    ).await;
    
    assert_eq!(response.status, "success");
    
    // Verify normalized action was sent
    assertions::assert_command_sent(&client_arc, "light-1", "on").await;
});

async_test!(test_error_handling_disconnected_client, {
    let mock_client = MockLoxoneClient::new();
    let context = ClientContext::new();
    
    // Don't connect the client
    let tool_context = ToolContext::new(Arc::new(mock_client), Arc::new(context));
    
    // Tool should fail when client is not connected
    let response = devices::control_device(
        tool_context.clone(),
        "test".to_string(),
        "on".to_string()
    ).await;
    
    assert_eq!(response.status, "error");
    assert!(response.message.unwrap().contains("not found"));
});

async_test!(test_command_failure_handling, {
    let (mock_client, context) = setup_integration_test().await;
    let client_arc = Arc::new(mock_client);
    
    // Set up command failure
    let error_response = LoxoneResponse {
        code: 500,
        value: serde_json::json!({"error": "Device error"}),
    };
    client_arc.set_response_override("light-1:on".to_string(), error_response).await;
    
    let tool_context = ToolContext::new(client_arc.clone(), Arc::new(context));
    
    // Command should show failure but tool should still return success with error details
    let response = devices::control_device(
        tool_context.clone(),
        "Living Room Light".to_string(),
        "on".to_string()
    ).await;
    
    assert_eq!(response.status, "success");
    
    let data = &response.data;
    assert!(!data.get("success").unwrap().as_bool().unwrap());
    assert!(data.get("error").is_some());
});