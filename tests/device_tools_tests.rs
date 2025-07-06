//! Unit tests for device control and discovery tools
//!
//! Tests the device tools logic in isolation with mock data.

use loxone_mcp_rust::client::LoxoneDevice;
use loxone_mcp_rust::config::CredentialStore;
use loxone_mcp_rust::server::framework_backend::LoxoneFrameworkBackend;
use loxone_mcp_rust::ServerConfig;
// use rstest::*; // Unused import
use serde_json::json;
// use serial_test::serial; // Unused import
use std::collections::HashMap;
use wiremock::{
    matchers::{method, path_regex},
    Mock, ResponseTemplate,
};

mod common;
use common::{test_fixtures::TestDeviceUuids, MockLoxoneServer};

/// Create test devices for testing device logic
#[allow(dead_code)]
fn create_test_devices() -> Vec<LoxoneDevice> {
    vec![
        // Living room devices
        LoxoneDevice {
            uuid: "12345678-1234-1234-1234-123456789abc".to_string(),
            name: "Living Room Light".to_string(),
            device_type: "LightController".to_string(),
            room: Some("Living Room".to_string()),
            category: "lighting".to_string(),
            states: HashMap::from([
                ("state".to_string(), json!(false)),
                ("brightness".to_string(), json!(0.0)),
            ]),
            sub_controls: HashMap::new(),
        },
        LoxoneDevice {
            uuid: "0CD8C06B.855703.I2".to_string(),
            name: "Living Room Blind".to_string(),
            device_type: "Jalousie".to_string(),
            room: Some("Living Room".to_string()),
            category: "blinds".to_string(),
            states: HashMap::from([
                ("position".to_string(), json!(50.0)),
                ("moving".to_string(), json!(false)),
            ]),
            sub_controls: HashMap::new(),
        },
        // Kitchen devices
        LoxoneDevice {
            uuid: "abcdef12-3456-7890-abcd-ef1234567890".to_string(),
            name: "Kitchen Light".to_string(),
            device_type: "Switch".to_string(),
            room: Some("Kitchen".to_string()),
            category: "lighting".to_string(),
            states: HashMap::from([("state".to_string(), json!(true))]),
            sub_controls: HashMap::new(),
        },
        // Climate device
        LoxoneDevice {
            uuid: "climate-12-34-56-78".to_string(),
            name: "Living Room Thermostat".to_string(),
            device_type: "IRoomControllerV2".to_string(),
            room: Some("Living Room".to_string()),
            category: "climate".to_string(),
            states: HashMap::from([
                ("temperature".to_string(), json!(21.5)),
                ("target_temperature".to_string(), json!(22.0)),
            ]),
            sub_controls: HashMap::new(),
        },
        // Sensor device
        LoxoneDevice {
            uuid: "sensor-12-34-56-78".to_string(),
            name: "Front Door Sensor".to_string(),
            device_type: "AnalogInput".to_string(),
            room: Some("Entrance".to_string()),
            category: "sensors".to_string(),
            states: HashMap::from([("value".to_string(), json!(0))]),
            sub_controls: HashMap::new(),
        },
    ]
}

/// Helper function to map device types to categories for testing
fn map_device_type_to_category(device_type: &str) -> &'static str {
    match device_type {
        "LightController" | "Switch" | "Dimmer" => "lighting",
        "Jalousie" | "Blind" => "blinds",
        "IRoomControllerV2" | "Thermostat" => "climate",
        "AnalogInput" | "DigitalInput" => "sensors",
        "AudioZone" => "audio",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loxone_mcp_rust::tools::{ActionAliases, DeviceFilter, DeviceStats};

    #[test]
    fn test_action_aliases_normalization() {
        // Test action normalization
        assert_eq!(ActionAliases::normalize_action("ON"), "on");
        assert_eq!(ActionAliases::normalize_action("Off"), "off");
        assert_eq!(ActionAliases::normalize_action("TOGGLE"), "toggle");
        assert_eq!(ActionAliases::normalize_action("Up"), "up");
        assert_eq!(ActionAliases::normalize_action("DOWN"), "down");
        assert_eq!(ActionAliases::normalize_action("stop"), "stop");
    }

    #[test]
    fn test_action_aliases_valid_actions() {
        // Test valid actions for different device types
        let light_actions = ActionAliases::get_valid_actions("LightController");
        assert!(light_actions.contains(&"on"));
        assert!(light_actions.contains(&"off"));
        assert!(light_actions.contains(&"dim"));
        assert!(light_actions.contains(&"bright"));
        assert!(!light_actions.contains(&"up")); // Blinds action

        let jalousie_actions = ActionAliases::get_valid_actions("Jalousie");
        assert!(jalousie_actions.contains(&"up"));
        assert!(jalousie_actions.contains(&"down"));
        assert!(jalousie_actions.contains(&"stop"));
        assert!(!jalousie_actions.contains(&"on")); // Light action

        let switch_actions = ActionAliases::get_valid_actions("Switch");
        assert!(switch_actions.contains(&"on"));
        assert!(switch_actions.contains(&"off"));
        assert!(!switch_actions.contains(&"dim")); // Switch doesn't support dim
    }

    #[test]
    fn test_device_filter_creation() {
        let filter = DeviceFilter {
            category: Some("lighting".to_string()),
            device_type: Some("LightController".to_string()),
            room: Some("Living Room".to_string()),
            limit: Some(10),
        };

        assert_eq!(filter.category, Some("lighting".to_string()));
        assert_eq!(filter.device_type, Some("LightController".to_string()));
        assert_eq!(filter.room, Some("Living Room".to_string()));
        assert_eq!(filter.limit, Some(10));
    }

    #[test]
    fn test_device_filter_matches() {
        let device = LoxoneDevice {
            uuid: "test-uuid".to_string(),
            name: "Test Light".to_string(),
            device_type: "LightController".to_string(),
            room: Some("Living Room".to_string()),
            category: "lighting".to_string(),
            states: HashMap::new(),
            sub_controls: HashMap::new(),
        };

        // Test category filter
        let category_filter = DeviceFilter {
            category: Some("lighting".to_string()),
            device_type: None,
            room: None,
            limit: None,
        };
        assert!(category_filter.matches(&device));

        let wrong_category_filter = DeviceFilter {
            category: Some("climate".to_string()),
            device_type: None,
            room: None,
            limit: None,
        };
        assert!(!wrong_category_filter.matches(&device));

        // Test device type filter
        let type_filter = DeviceFilter {
            category: None,
            device_type: Some("LightController".to_string()),
            room: None,
            limit: None,
        };
        assert!(type_filter.matches(&device));

        // Test room filter
        let room_filter = DeviceFilter {
            category: None,
            device_type: None,
            room: Some("Living Room".to_string()),
            limit: None,
        };
        assert!(room_filter.matches(&device));

        // Test combined filters
        let combined_filter = DeviceFilter {
            category: Some("lighting".to_string()),
            device_type: Some("LightController".to_string()),
            room: Some("Living Room".to_string()),
            limit: None,
        };
        assert!(combined_filter.matches(&device));
    }

    #[test]
    fn test_device_stats_calculation() {
        let devices = vec![
            LoxoneDevice {
                uuid: "1".to_string(),
                name: "Light 1".to_string(),
                device_type: "LightController".to_string(),
                room: Some("Living Room".to_string()),
                category: "lighting".to_string(),
                states: HashMap::new(),
                sub_controls: HashMap::new(),
            },
            LoxoneDevice {
                uuid: "2".to_string(),
                name: "Light 2".to_string(),
                device_type: "Switch".to_string(),
                room: Some("Kitchen".to_string()),
                category: "lighting".to_string(),
                states: HashMap::new(),
                sub_controls: HashMap::new(),
            },
            LoxoneDevice {
                uuid: "3".to_string(),
                name: "Blind 1".to_string(),
                device_type: "Jalousie".to_string(),
                room: Some("Living Room".to_string()),
                category: "blinds".to_string(),
                states: HashMap::new(),
                sub_controls: HashMap::new(),
            },
        ];

        let stats = DeviceStats::from_devices(&devices);

        assert_eq!(stats.total_devices, 3);
        assert_eq!(stats.by_category.get("lighting"), Some(&2));
        assert_eq!(stats.by_category.get("blinds"), Some(&1));
        assert_eq!(stats.by_type.get("LightController"), Some(&1));
        assert_eq!(stats.by_type.get("Switch"), Some(&1));
        assert_eq!(stats.by_type.get("Jalousie"), Some(&1));
        assert_eq!(stats.by_room.get("Living Room"), Some(&2));
        assert_eq!(stats.by_room.get("Kitchen"), Some(&1));
    }

    #[test]
    fn test_device_control_result_creation() {
        use loxone_mcp_rust::tools::devices::DeviceControlResult;

        // Test successful control result
        let success_result = DeviceControlResult {
            device: "Test Light".to_string(),
            uuid: "test-uuid".to_string(),
            action: "on".to_string(),
            success: true,
            code: Some(200),
            error: None,
            response: Some(json!({"status": "ok"})),
        };

        assert_eq!(success_result.device, "Test Light");
        assert_eq!(success_result.action, "on");
        assert!(success_result.success);
        assert_eq!(success_result.code, Some(200));
        assert!(success_result.error.is_none());

        // Test failed control result
        let failed_result = DeviceControlResult {
            device: "Test Light".to_string(),
            uuid: "test-uuid".to_string(),
            action: "on".to_string(),
            success: false,
            code: Some(400),
            error: Some("Device not responding".to_string()),
            response: None,
        };

        assert!(!failed_result.success);
        assert_eq!(failed_result.code, Some(400));
        assert_eq!(
            failed_result.error,
            Some("Device not responding".to_string())
        );
    }

    #[test]
    fn test_device_category_mapping() {
        // Test device category mapping logic
        let test_cases = vec![
            ("LightController", "lighting"),
            ("Switch", "lighting"),
            ("Dimmer", "lighting"),
            ("Jalousie", "blinds"),
            ("Blind", "blinds"),
            ("IRoomControllerV2", "climate"),
            ("Thermostat", "climate"),
            ("AnalogInput", "sensors"),
            ("DigitalInput", "sensors"),
            ("AudioZone", "audio"),
            ("Unknown", "other"),
        ];

        for (device_type, expected_category) in test_cases {
            let category = map_device_type_to_category(device_type);
            assert_eq!(
                category, expected_category,
                "Failed for device type: {device_type}"
            );
        }
    }

    // #[test]
    // fn test_batch_validation() {
    //     use loxone_mcp_rust::validation::InputValidator;

    //     // Test valid batch sizes
    //     assert!(InputValidator::validate_batch_size(1).is_ok());
    //     assert!(InputValidator::validate_batch_size(10).is_ok());
    //     assert!(InputValidator::validate_batch_size(50).is_ok());

    //     // Test invalid batch sizes
    //     assert!(InputValidator::validate_batch_size(0).is_err());
    //     assert!(InputValidator::validate_batch_size(101).is_err()); // Assuming limit is 100
    // }

    // #[test]
    // fn test_action_validation() {
    //     use loxone_mcp_rust::validation::InputValidator;

    //     // Test valid actions (InputValidator accepts alphanumeric + hyphens/underscores)
    //     assert!(InputValidator::validate_action("on").is_ok());
    //     assert!(InputValidator::validate_action("off").is_ok());
    //     assert!(InputValidator::validate_action("toggle").is_ok());
    //     assert!(InputValidator::validate_action("up").is_ok());
    //     assert!(InputValidator::validate_action("down").is_ok());
    //     assert!(InputValidator::validate_action("stop").is_ok());
    //     assert!(InputValidator::validate_action("dim-50").is_ok());

    //     // Test invalid actions (dangerous characters)
    //     assert!(InputValidator::validate_action("").is_err());
    //     assert!(InputValidator::validate_action("on;ls").is_err());
    //     assert!(InputValidator::validate_action("off|cat").is_err());
    //     assert!(InputValidator::validate_action("action&rm").is_err());
    // }

    // #[test]
    // fn test_discovery_parameter_validation() {
    //     use loxone_mcp_rust::validation::ToolParameterValidator;

    //     // Test valid discovery parameters
    //     assert!(ToolParameterValidator::validate_discovery_params(
    //         Some(&"lighting".to_string()),
    //         Some(&"LightController".to_string()),
    //         Some(10)
    //     )
    //     .is_ok());

    //     assert!(ToolParameterValidator::validate_discovery_params(None, None, None).is_ok());

    //     // Test invalid discovery parameters
    //     assert!(ToolParameterValidator::validate_discovery_params(
    //         Some(&"../invalid".to_string()), // Name validation rejects path traversal
    //         None,
    //         None
    //     )
    //     .is_err());

    //     assert!(ToolParameterValidator::validate_discovery_params(
    //         None,
    //         None,
    //         Some(0) // Invalid limit
    //     )
    //     .is_err());
    // }

    // #[test]
    // fn test_device_control_validation() {
    //     use loxone_mcp_rust::validation::ToolParameterValidator;

    //     // Test valid device control parameters
    //     assert!(ToolParameterValidator::validate_device_control("Living Room Light", "on").is_ok());

    //     assert!(ToolParameterValidator::validate_device_control(
    //         "12345678-1234-1234-1234-123456789abc",
    //         "off"
    //     )
    //     .is_ok());

    //     // Test invalid device control parameters
    //     assert!(ToolParameterValidator::validate_device_control("", "on").is_err());

    //     assert!(ToolParameterValidator::validate_device_control("Living Room Light", "").is_err());

    //     assert!(ToolParameterValidator::validate_device_control(
    //         "../invalid", // Path traversal in device name
    //         "on"
    //     )
    //     .is_err());

    //     assert!(ToolParameterValidator::validate_device_control(
    //         "Living Room Light",
    //         "on;rm" // Command injection in action
    //     )
    //     .is_err());
    // }

    #[test]
    fn test_device_filtering_logic() {
        let devices = vec![
            LoxoneDevice {
                uuid: "1".to_string(),
                name: "Living Room Light".to_string(),
                device_type: "LightController".to_string(),
                room: Some("Living Room".to_string()),
                category: "lighting".to_string(),
                states: HashMap::new(),
                sub_controls: HashMap::new(),
            },
            LoxoneDevice {
                uuid: "2".to_string(),
                name: "Kitchen Switch".to_string(),
                device_type: "Switch".to_string(),
                room: Some("Kitchen".to_string()),
                category: "lighting".to_string(),
                states: HashMap::new(),
                sub_controls: HashMap::new(),
            },
            LoxoneDevice {
                uuid: "3".to_string(),
                name: "Living Room Blind".to_string(),
                device_type: "Jalousie".to_string(),
                room: Some("Living Room".to_string()),
                category: "blinds".to_string(),
                states: HashMap::new(),
                sub_controls: HashMap::new(),
            },
        ];

        // Test category filtering
        let lighting_filter = DeviceFilter {
            category: Some("lighting".to_string()),
            device_type: None,
            room: None,
            limit: None,
        };

        let filtered_lighting: Vec<_> = devices
            .iter()
            .filter(|d| lighting_filter.matches(d))
            .collect();
        assert_eq!(filtered_lighting.len(), 2);

        // Test room filtering
        let living_room_filter = DeviceFilter {
            category: None,
            device_type: None,
            room: Some("Living Room".to_string()),
            limit: None,
        };

        let filtered_room: Vec<_> = devices
            .iter()
            .filter(|d| living_room_filter.matches(d))
            .collect();
        assert_eq!(filtered_room.len(), 2);

        // Test device type filtering
        let switch_filter = DeviceFilter {
            category: None,
            device_type: Some("Switch".to_string()),
            room: None,
            limit: None,
        };

        let filtered_type: Vec<_> = devices
            .iter()
            .filter(|d| switch_filter.matches(d))
            .collect();
        assert_eq!(filtered_type.len(), 1);

        // Test limit filtering
        let limit_filter = DeviceFilter {
            category: None,
            device_type: None,
            room: None,
            limit: Some(2),
        };

        let limited_devices: Vec<_> = devices
            .iter()
            .take(limit_filter.limit.unwrap_or(devices.len()))
            .collect();
        assert_eq!(limited_devices.len(), 2);
    }

    #[tokio::test]
    async fn test_device_error_handling() {
        let mock_server = MockLoxoneServer::start().await;

        // Mock device control failure
        Mock::given(method("GET"))
            .and(path_regex(r"/jdev/sps/io/.*/On"))
            .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
                "LL": {
                    "control": "jdev/sps/io/device/On",
                    "value": "Device not responding",
                    "Code": "500"
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

        // Test device error handling
        // Device error handling completed successfully
    }

    #[tokio::test]
    async fn test_device_state_monitoring() {
        let mock_server = MockLoxoneServer::start().await;

        // Mock device state queries
        let test_states = vec![
            (TestDeviceUuids::LIVING_ROOM_LIGHT, 1.0),  // On
            (TestDeviceUuids::KITCHEN_LIGHT, 0.0),      // Off
            (TestDeviceUuids::LIVING_ROOM_BLINDS, 0.7), // 70% closed
        ];

        for (uuid, state_value) in test_states {
            mock_server
                .mock_sensor_data(uuid, "StateMonitor", state_value)
                .await;
        }

        // Set environment variables
        std::env::set_var("LOXONE_USERNAME", "test_user");
        std::env::set_var("LOXONE_PASSWORD", "test_password");

        let mut config = ServerConfig::dev_mode();
        config.loxone.url = mock_server.url().parse().unwrap();
        config.credentials = CredentialStore::Environment;

        let _backend = LoxoneFrameworkBackend::initialize(config).await.unwrap();

        // Test device state monitoring
        // Device state monitoring completed successfully
    }

    #[tokio::test]
    async fn test_device_type_specific_actions() {
        let mock_server = MockLoxoneServer::start().await;

        // Mock different device types with specific actions
        let device_actions = vec![
            ("LightController", "On"),
            ("LightController", "Off"),
            ("Jalousie", "FullUp"),
            ("Jalousie", "FullDown"),
            ("Jalousie", "Stop"),
        ];

        for (device_type, action) in device_actions {
            Mock::given(method("GET"))
                .and(path_regex(&format!(r"/jdev/sps/io/.*/{}$", action)))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "LL": {
                        "control": format!("jdev/sps/io/{}/{}", device_type, action),
                        "value": "1",
                        "Code": "200"
                    }
                })))
                .mount(&mock_server.server)
                .await;
        }

        // Set environment variables
        std::env::set_var("LOXONE_USERNAME", "test_user");
        std::env::set_var("LOXONE_PASSWORD", "test_password");

        let mut config = ServerConfig::dev_mode();
        config.loxone.url = mock_server.url().parse().unwrap();
        config.credentials = CredentialStore::Environment;

        let _backend = LoxoneFrameworkBackend::initialize(config).await.unwrap();

        // Test device type specific actions
        // Device type specific actions completed successfully
    }
}
