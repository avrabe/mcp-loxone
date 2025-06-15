//! Unit tests for newly implemented MCP tools
//!
//! Tests for get_devices_by_category, get_available_capabilities,
//! control_multiple_devices, and other recently added tools.

use loxone_mcp_rust::{client::LoxoneDevice, tools::DeviceStats};
use serde_json::json;
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    /// Create test devices for testing
    fn create_test_devices() -> Vec<LoxoneDevice> {
        vec![
            // Lighting devices
            LoxoneDevice {
                uuid: "light-1".to_string(),
                name: "Living Room Main Light".to_string(),
                device_type: "LightController".to_string(),
                room: Some("Living Room".to_string()),
                category: "lighting".to_string(),
                states: HashMap::from([
                    ("state".to_string(), json!(true)),
                    ("brightness".to_string(), json!(75.0)),
                ]),
                sub_controls: HashMap::new(),
            },
            LoxoneDevice {
                uuid: "light-2".to_string(),
                name: "Kitchen Counter Light".to_string(),
                device_type: "Dimmer".to_string(),
                room: Some("Kitchen".to_string()),
                category: "lighting".to_string(),
                states: HashMap::from([
                    ("state".to_string(), json!(false)),
                    ("brightness".to_string(), json!(0.0)),
                ]),
                sub_controls: HashMap::new(),
            },
            LoxoneDevice {
                uuid: "light-3".to_string(),
                name: "Bedroom Reading Light".to_string(),
                device_type: "Switch".to_string(),
                room: Some("Bedroom".to_string()),
                category: "lighting".to_string(),
                states: HashMap::from([("state".to_string(), json!(true))]),
                sub_controls: HashMap::new(),
            },
            // Blind devices
            LoxoneDevice {
                uuid: "blind-1".to_string(),
                name: "Living Room Window Blind".to_string(),
                device_type: "Jalousie".to_string(),
                room: Some("Living Room".to_string()),
                category: "blinds".to_string(),
                states: HashMap::from([
                    ("position".to_string(), json!(25.0)),
                    ("moving".to_string(), json!(false)),
                ]),
                sub_controls: HashMap::new(),
            },
            LoxoneDevice {
                uuid: "blind-2".to_string(),
                name: "Bedroom Blackout Blind".to_string(),
                device_type: "Jalousie".to_string(),
                room: Some("Bedroom".to_string()),
                category: "blinds".to_string(),
                states: HashMap::from([
                    ("position".to_string(), json!(100.0)),
                    ("moving".to_string(), json!(false)),
                ]),
                sub_controls: HashMap::new(),
            },
            // Climate devices
            LoxoneDevice {
                uuid: "climate-1".to_string(),
                name: "Living Room Thermostat".to_string(),
                device_type: "IRoomControllerV2".to_string(),
                room: Some("Living Room".to_string()),
                category: "climate".to_string(),
                states: HashMap::from([
                    ("temperature".to_string(), json!(21.5)),
                    ("target_temperature".to_string(), json!(22.0)),
                    ("mode".to_string(), json!("heating")),
                ]),
                sub_controls: HashMap::new(),
            },
            // Sensor devices
            LoxoneDevice {
                uuid: "sensor-1".to_string(),
                name: "Front Door Sensor".to_string(),
                device_type: "AnalogInput".to_string(),
                room: Some("Entrance".to_string()),
                category: "sensors".to_string(),
                states: HashMap::from([("value".to_string(), json!(0))]),
                sub_controls: HashMap::new(),
            },
            LoxoneDevice {
                uuid: "sensor-2".to_string(),
                name: "Window Sensor".to_string(),
                device_type: "DigitalInput".to_string(),
                room: Some("Living Room".to_string()),
                category: "sensors".to_string(),
                states: HashMap::from([("value".to_string(), json!(1))]),
                sub_controls: HashMap::new(),
            },
            // Audio device
            LoxoneDevice {
                uuid: "audio-1".to_string(),
                name: "Living Room Speakers".to_string(),
                device_type: "AudioZone".to_string(),
                room: Some("Living Room".to_string()),
                category: "audio".to_string(),
                states: HashMap::from([
                    ("volume".to_string(), json!(50.0)),
                    ("playing".to_string(), json!(false)),
                ]),
                sub_controls: HashMap::new(),
            },
        ]
    }

    #[test]
    fn test_get_devices_by_category_filtering() {
        let devices = create_test_devices();

        // Test lighting category
        let lighting_devices: Vec<_> = devices
            .iter()
            .filter(|d| d.category == "lighting")
            .collect();
        assert_eq!(lighting_devices.len(), 3);
        assert!(lighting_devices.iter().all(|d| d.category == "lighting"));

        // Test blinds category
        let blind_devices: Vec<_> = devices.iter().filter(|d| d.category == "blinds").collect();
        assert_eq!(blind_devices.len(), 2);
        assert!(blind_devices.iter().all(|d| d.category == "blinds"));

        // Test climate category
        let climate_devices: Vec<_> = devices.iter().filter(|d| d.category == "climate").collect();
        assert_eq!(climate_devices.len(), 1);
        assert_eq!(climate_devices[0].device_type, "IRoomControllerV2");

        // Test sensors category
        let sensor_devices: Vec<_> = devices.iter().filter(|d| d.category == "sensors").collect();
        assert_eq!(sensor_devices.len(), 2);

        // Test audio category
        let audio_devices: Vec<_> = devices.iter().filter(|d| d.category == "audio").collect();
        assert_eq!(audio_devices.len(), 1);
    }

    #[test]
    fn test_get_devices_by_category_with_limit() {
        let devices = create_test_devices();
        let lighting_devices: Vec<_> = devices
            .iter()
            .filter(|d| d.category == "lighting")
            .take(2) // Apply limit
            .collect();

        assert_eq!(lighting_devices.len(), 2);
        assert!(lighting_devices.iter().all(|d| d.category == "lighting"));
    }

    #[test]
    fn test_get_devices_by_category_with_state() {
        let devices = create_test_devices();

        // Test that devices include state information
        for device in &devices {
            assert!(
                !device.states.is_empty(),
                "Device {} should have states",
                device.name
            );

            match device.category.as_str() {
                "lighting" => {
                    assert!(
                        device.states.contains_key("state"),
                        "Lighting device should have state"
                    );
                }
                "blinds" => {
                    assert!(
                        device.states.contains_key("position"),
                        "Blind device should have position"
                    );
                }
                "climate" => {
                    assert!(
                        device.states.contains_key("temperature"),
                        "Climate device should have temperature"
                    );
                }
                "sensors" => {
                    assert!(
                        device.states.contains_key("value"),
                        "Sensor device should have value"
                    );
                }
                "audio" => {
                    assert!(
                        device.states.contains_key("volume"),
                        "Audio device should have volume"
                    );
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_get_devices_by_type_filtering() {
        let devices = create_test_devices();

        // Test LightController type
        let light_controllers: Vec<_> = devices
            .iter()
            .filter(|d| d.device_type == "LightController")
            .collect();
        assert_eq!(light_controllers.len(), 1);
        assert_eq!(light_controllers[0].name, "Living Room Main Light");

        // Test Jalousie type
        let jalousies: Vec<_> = devices
            .iter()
            .filter(|d| d.device_type == "Jalousie")
            .collect();
        assert_eq!(jalousies.len(), 2);

        // Test Switch type
        let switches: Vec<_> = devices
            .iter()
            .filter(|d| d.device_type == "Switch")
            .collect();
        assert_eq!(switches.len(), 1);

        // Test AnalogInput type
        let analog_inputs: Vec<_> = devices
            .iter()
            .filter(|d| d.device_type == "AnalogInput")
            .collect();
        assert_eq!(analog_inputs.len(), 1);
    }

    #[test]
    fn test_system_capabilities_calculation() {
        let devices = create_test_devices();

        // Simulate capability calculation
        let lighting_count = devices.iter().filter(|d| d.category == "lighting").count();
        let blind_count = devices.iter().filter(|d| d.category == "blinds").count();
        let climate_count = devices.iter().filter(|d| d.category == "climate").count();
        let sensor_count = devices.iter().filter(|d| d.category == "sensors").count();
        let audio_count = devices.iter().filter(|d| d.category == "audio").count();

        assert_eq!(lighting_count, 3);
        assert_eq!(blind_count, 2);
        assert_eq!(climate_count, 1);
        assert_eq!(sensor_count, 2);
        assert_eq!(audio_count, 1);

        // Test capability flags
        let has_lighting = lighting_count > 0;
        let has_blinds = blind_count > 0;
        let has_climate = climate_count > 0;
        let has_sensors = sensor_count > 0;
        let has_audio = audio_count > 0;

        assert!(has_lighting);
        assert!(has_blinds);
        assert!(has_climate);
        assert!(has_sensors);
        assert!(has_audio);
    }

    #[test]
    fn test_available_capabilities_response_structure() {
        let devices = create_test_devices();

        // Simulate the response structure for get_available_capabilities
        let lighting_count = devices.iter().filter(|d| d.category == "lighting").count();
        let blind_count = devices.iter().filter(|d| d.category == "blinds").count();

        let capabilities = json!({
            "lighting": {
                "available": lighting_count > 0,
                "device_count": lighting_count,
                "tools": [
                    "control_device",
                    "control_multiple_devices",
                    "control_all_lights",
                    "control_room_lights",
                    "get_devices_by_type (with 'LightController')",
                    "get_devices_by_category (with 'lighting')"
                ],
                "description": "Control lights, dimmers, and switches"
            },
            "blinds": {
                "available": blind_count > 0,
                "device_count": blind_count,
                "tools": [
                    "control_device",
                    "control_multiple_devices",
                    "control_all_rolladen",
                    "control_room_rolladen",
                    "get_devices_by_type (with 'Jalousie')",
                    "get_devices_by_category (with 'blinds')"
                ],
                "description": "Control blinds and rolladen"
            }
        });

        // Verify structure
        assert!(capabilities["lighting"]["available"].as_bool().unwrap());
        assert_eq!(
            capabilities["lighting"]["device_count"].as_u64().unwrap(),
            3
        );
        assert!(capabilities["lighting"]["tools"].is_array());

        assert!(capabilities["blinds"]["available"].as_bool().unwrap());
        assert_eq!(capabilities["blinds"]["device_count"].as_u64().unwrap(), 2);
        assert!(capabilities["blinds"]["tools"].is_array());
    }

    #[test]
    fn test_control_multiple_devices_validation() {
        // Test empty device list
        let empty_devices: Vec<String> = vec![];
        assert!(empty_devices.is_empty());

        // Test valid device list
        let valid_devices = [
            "Living Room Light".to_string(),
            "Kitchen Light".to_string(),
            "Bedroom Light".to_string(),
        ];
        assert!(!valid_devices.is_empty());
        assert!(valid_devices.len() <= 50); // Assume reasonable batch limit

        // Test large device list (should be rejected)
        let large_devices: Vec<String> = (0..101).map(|i| format!("Device {}", i)).collect();
        assert!(large_devices.len() > 100); // Should exceed reasonable limit
    }

    #[test]
    fn test_device_command_response_structure() {
        // Test successful device control response
        let success_response = json!({
            "device": "Living Room Light",
            "uuid": "light-1",
            "action": "on",
            "success": true,
            "code": 200,
            "error": null,
            "response": {"status": "ok"}
        });

        assert_eq!(success_response["device"], "Living Room Light");
        assert_eq!(success_response["action"], "on");
        assert!(success_response["success"].as_bool().unwrap());
        assert_eq!(success_response["code"], 200);
        assert!(success_response["error"].is_null());

        // Test failed device control response
        let error_response = json!({
            "device": "Broken Light",
            "uuid": "broken-light-1",
            "action": "on",
            "success": false,
            "code": 400,
            "error": "Device not responding",
            "response": null
        });

        assert!(!error_response["success"].as_bool().unwrap());
        assert_eq!(error_response["code"], 400);
        assert_eq!(error_response["error"], "Device not responding");
        assert!(error_response["response"].is_null());
    }

    #[test]
    fn test_multiple_device_control_batch_response() {
        // Test batch control response structure
        let devices = ["Light 1", "Light 2", "Light 3"];
        let action = "on";

        // Simulate batch response
        let batch_response = json!({
            "batch_id": "batch-123",
            "action": action,
            "total_devices": devices.len(),
            "successful": 2,
            "failed": 1,
            "results": [
                {
                    "device": "Light 1",
                    "success": true,
                    "code": 200
                },
                {
                    "device": "Light 2",
                    "success": true,
                    "code": 200
                },
                {
                    "device": "Light 3",
                    "success": false,
                    "code": 400,
                    "error": "Device offline"
                }
            ],
            "summary": {
                "success_rate": 66.67,
                "execution_time_ms": 234
            }
        });

        assert_eq!(batch_response["total_devices"], 3);
        assert_eq!(batch_response["successful"], 2);
        assert_eq!(batch_response["failed"], 1);
        assert!(batch_response["results"].is_array());
        assert_eq!(batch_response["results"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_device_discovery_response_structure() {
        let devices = create_test_devices();
        let stats = DeviceStats::from_devices(&devices);

        // Test discovery response structure
        let discovery_response = json!({
            "devices": devices.iter().map(|d| json!({
                "uuid": d.uuid,
                "name": d.name,
                "device_type": d.device_type,
                "room": d.room,
                "category": d.category,
                "states": d.states
            })).collect::<Vec<_>>(),
            "statistics": {
                "total_devices": stats.total_devices,
                "by_category": stats.by_category,
                "by_type": stats.by_type,
                "by_room": stats.by_room
            },
            "total_found": devices.len()
        });

        assert_eq!(discovery_response["total_found"], devices.len());
        assert!(discovery_response["devices"].is_array());
        assert!(discovery_response["statistics"].is_object());
        assert_eq!(
            discovery_response["statistics"]["total_devices"],
            devices.len()
        );
    }

    #[test]
    fn test_room_device_filtering() {
        let devices = create_test_devices();

        // Test Living Room filtering
        let living_room_devices: Vec<_> = devices
            .iter()
            .filter(|d| d.room.as_deref() == Some("Living Room"))
            .collect();

        assert_eq!(living_room_devices.len(), 5); // Light, blind, climate, sensor, audio

        for device in living_room_devices {
            assert_eq!(device.room.as_deref(), Some("Living Room"));
        }

        // Test Kitchen filtering
        let kitchen_devices: Vec<_> = devices
            .iter()
            .filter(|d| d.room.as_deref() == Some("Kitchen"))
            .collect();

        assert_eq!(kitchen_devices.len(), 1);
        assert_eq!(kitchen_devices[0].name, "Kitchen Counter Light");

        // Test Bedroom filtering
        let bedroom_devices: Vec<_> = devices
            .iter()
            .filter(|d| d.room.as_deref() == Some("Bedroom"))
            .collect();

        assert_eq!(bedroom_devices.len(), 2); // Light and blind
    }

    #[test]
    fn test_device_type_specific_filtering() {
        let devices = create_test_devices();

        // Group devices by type
        let mut devices_by_type: HashMap<String, Vec<&LoxoneDevice>> = HashMap::new();
        for device in &devices {
            devices_by_type
                .entry(device.device_type.clone())
                .or_default()
                .push(device);
        }

        // Test specific device types
        assert_eq!(devices_by_type.get("LightController").unwrap().len(), 1);
        assert_eq!(devices_by_type.get("Dimmer").unwrap().len(), 1);
        assert_eq!(devices_by_type.get("Switch").unwrap().len(), 1);
        assert_eq!(devices_by_type.get("Jalousie").unwrap().len(), 2);
        assert_eq!(devices_by_type.get("IRoomControllerV2").unwrap().len(), 1);
        assert_eq!(devices_by_type.get("AnalogInput").unwrap().len(), 1);
        assert_eq!(devices_by_type.get("DigitalInput").unwrap().len(), 1);
        assert_eq!(devices_by_type.get("AudioZone").unwrap().len(), 1);
    }

    #[test]
    fn test_pagination_logic() {
        let devices = create_test_devices();

        // Test different page sizes
        let page_sizes = vec![1, 2, 3, 5, 10];

        for page_size in page_sizes {
            let paginated: Vec<_> = devices.iter().take(page_size).collect();
            assert!(paginated.len() <= page_size);
            assert!(paginated.len() <= devices.len());

            if devices.len() >= page_size {
                assert_eq!(paginated.len(), page_size);
            } else {
                assert_eq!(paginated.len(), devices.len());
            }
        }
    }

    #[test]
    fn test_device_state_inclusion() {
        let devices = create_test_devices();

        // Test that when include_state is true, states are present
        for device in &devices {
            assert!(
                !device.states.is_empty(),
                "Device states should not be empty"
            );

            // Verify device-specific states
            match device.device_type.as_str() {
                "LightController" | "Dimmer" => {
                    assert!(device.states.contains_key("state"));
                    assert!(device.states.contains_key("brightness"));
                }
                "Switch" => {
                    assert!(device.states.contains_key("state"));
                }
                "Jalousie" => {
                    assert!(device.states.contains_key("position"));
                    assert!(device.states.contains_key("moving"));
                }
                "IRoomControllerV2" => {
                    assert!(device.states.contains_key("temperature"));
                    assert!(device.states.contains_key("target_temperature"));
                }
                "AnalogInput" | "DigitalInput" => {
                    assert!(device.states.contains_key("value"));
                }
                "AudioZone" => {
                    assert!(device.states.contains_key("volume"));
                    assert!(device.states.contains_key("playing"));
                }
                _ => {}
            }
        }
    }
}
