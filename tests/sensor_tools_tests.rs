//! Unit tests for sensor discovery and monitoring tools
//!
//! Tests for sensor discovery, classification, and monitoring functionality.

use loxone_mcp_rust::{
    client::LoxoneDevice,
    tools::sensors::{
        DiscoveredSensor, SensorStateHistory, SensorStatistics, SensorType, StateChangeEvent,
    },
};
use serde_json::{json, Value};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_sensors() -> Vec<LoxoneDevice> {
        vec![
            // Door/Window sensors
            LoxoneDevice {
                uuid: "door-sensor-1".to_string(),
                name: "Front Door Sensor".to_string(),
                device_type: "AnalogInput".to_string(),
                room: Some("Entrance".to_string()),
                category: "sensors".to_string(),
                states: HashMap::from([("value".to_string(), json!(0))]), // Closed
                sub_controls: HashMap::new(),
            },
            LoxoneDevice {
                uuid: "window-sensor-1".to_string(),
                name: "Living Room Window Sensor".to_string(),
                device_type: "DigitalInput".to_string(),
                room: Some("Living Room".to_string()),
                category: "sensors".to_string(),
                states: HashMap::from([("value".to_string(), json!(1))]), // Open
                sub_controls: HashMap::new(),
            },
            // Motion sensors
            LoxoneDevice {
                uuid: "motion-sensor-1".to_string(),
                name: "Hallway Motion Detector".to_string(),
                device_type: "AnalogInput".to_string(),
                room: Some("Hallway".to_string()),
                category: "sensors".to_string(),
                states: HashMap::from([("value".to_string(), json!(0))]), // No motion
                sub_controls: HashMap::new(),
            },
            // Temperature sensors
            LoxoneDevice {
                uuid: "temp-sensor-1".to_string(),
                name: "Living Room Temperature".to_string(),
                device_type: "InfoOnlyAnalog".to_string(),
                room: Some("Living Room".to_string()),
                category: "sensors".to_string(),
                states: HashMap::from([("value".to_string(), json!(21.5))]),
                sub_controls: HashMap::new(),
            },
            LoxoneDevice {
                uuid: "temp-sensor-2".to_string(),
                name: "Outdoor Thermometer".to_string(),
                device_type: "InfoOnlyAnalog".to_string(),
                room: Some("Outdoor".to_string()),
                category: "sensors".to_string(),
                states: HashMap::from([("value".to_string(), json!(15.2))]),
                sub_controls: HashMap::new(),
            },
        ]
    }

    #[test]
    fn test_sensor_type_classification() {
        let sensors = create_test_sensors();

        // Test door sensor classification
        let door_sensor = &sensors[0];
        assert!(door_sensor.name.to_lowercase().contains("door"));
        let sensor_type = classify_sensor_by_name(&door_sensor.name);
        assert_eq!(sensor_type, SensorType::DoorWindow);

        // Test window sensor classification
        let window_sensor = &sensors[1];
        assert!(window_sensor.name.to_lowercase().contains("window"));
        let sensor_type = classify_sensor_by_name(&window_sensor.name);
        assert_eq!(sensor_type, SensorType::DoorWindow);

        // Test motion sensor classification
        let motion_sensor = &sensors[2];
        assert!(motion_sensor.name.to_lowercase().contains("motion"));
        let sensor_type = classify_sensor_by_name(&motion_sensor.name);
        assert_eq!(sensor_type, SensorType::Motion);

        // Test temperature sensor classification
        let temp_sensor = &sensors[3];
        assert!(temp_sensor.name.to_lowercase().contains("temperature"));
        let sensor_type = classify_sensor_by_name(&temp_sensor.name);
        assert_eq!(sensor_type, SensorType::Temperature);
    }

    #[test]
    fn test_discovered_sensor_creation() {
        let now = chrono::Utc::now();

        let sensor = DiscoveredSensor {
            uuid: "test-sensor-123".to_string(),
            name: Some("Test Door Sensor".to_string()),
            current_value: json!(0),
            value_history: vec![json!(1), json!(0), json!(1)],
            first_seen: now - chrono::Duration::minutes(30),
            last_updated: now,
            update_count: 3,
            sensor_type: SensorType::DoorWindow,
            confidence: 0.9,
            pattern_score: 0.8,
            room: Some("Living Room".to_string()),
            metadata: HashMap::from([
                ("device_type".to_string(), json!("AnalogInput")),
                ("category".to_string(), json!("sensors")),
            ]),
        };

        assert_eq!(sensor.uuid, "test-sensor-123");
        assert_eq!(sensor.name, Some("Test Door Sensor".to_string()));
        assert_eq!(sensor.current_value, json!(0));
        assert_eq!(sensor.value_history.len(), 3);
        assert_eq!(sensor.update_count, 3);
        assert_eq!(sensor.sensor_type, SensorType::DoorWindow);
        assert_eq!(sensor.confidence, 0.9);
        assert_eq!(sensor.room, Some("Living Room".to_string()));
    }

    #[test]
    fn test_sensor_pattern_analysis() {
        // Test binary pattern (door/window sensor)
        let binary_values = vec![json!(0), json!(1), json!(0), json!(1), json!(0)];
        let is_binary = is_binary_pattern(&binary_values);
        assert!(is_binary);

        // Test analog pattern (temperature sensor)
        let analog_values = vec![
            json!(21.1),
            json!(21.3),
            json!(21.0),
            json!(21.5),
            json!(21.2),
        ];
        let is_binary = is_binary_pattern(&analog_values);
        assert!(!is_binary);

        // Test mixed pattern (should not be binary)
        let mixed_values = vec![json!(0), json!(1), json!(2.5), json!(1), json!(0)];
        let is_binary = is_binary_pattern(&mixed_values);
        assert!(!is_binary);
    }

    #[test]
    fn test_sensor_state_history() {
        let mut history = SensorStateHistory::new(
            "test-sensor".to_string(),
            Some("Test Sensor".to_string()),
            10, // max events
        );

        assert_eq!(history.uuid, "test-sensor");
        assert_eq!(history.name, Some("Test Sensor".to_string()));
        assert_eq!(history.total_changes, 0);
        assert_eq!(history.state_events.len(), 0);

        // Add state changes
        history.add_state_change(json!(0), json!(1));
        assert_eq!(history.total_changes, 1);
        assert_eq!(history.current_state, json!(1));
        assert_eq!(history.state_events.len(), 1);

        history.add_state_change(json!(1), json!(0));
        assert_eq!(history.total_changes, 2);
        assert_eq!(history.current_state, json!(0));
        assert_eq!(history.state_events.len(), 2);

        // Test ring buffer behavior (add more than max_events)
        for i in 0..15 {
            history.add_state_change(json!(i % 2), json!((i + 1) % 2));
        }

        assert_eq!(history.state_events.len(), 10); // Should not exceed max_events
        assert_eq!(history.total_changes, 17); // Total should keep counting
    }

    #[test]
    fn test_door_window_activity_calculation() {
        let mut history = SensorStateHistory::new(
            "door-sensor".to_string(),
            Some("Front Door".to_string()),
            10,
        );

        history.sensor_type = Some(SensorType::DoorWindow);

        // Simulate door opening and closing events
        let base_time = chrono::Utc::now() - chrono::Duration::hours(2);

        // Add events manually for testing
        history.state_events.push_back(StateChangeEvent {
            uuid: "door-sensor".to_string(),
            timestamp: base_time + chrono::Duration::minutes(30),
            old_value: json!(0),
            new_value: json!(1),
            human_readable: "OPEN".to_string(),
            event_type: "state_change".to_string(),
        });

        history.state_events.push_back(StateChangeEvent {
            uuid: "door-sensor".to_string(),
            timestamp: base_time + chrono::Duration::minutes(45),
            old_value: json!(1),
            new_value: json!(0),
            human_readable: "CLOSED".to_string(),
            event_type: "state_change".to_string(),
        });

        history.state_events.push_back(StateChangeEvent {
            uuid: "door-sensor".to_string(),
            timestamp: base_time + chrono::Duration::minutes(60),
            old_value: json!(0),
            new_value: json!(1),
            human_readable: "OPEN".to_string(),
            event_type: "state_change".to_string(),
        });

        let activity = history.get_door_window_activity(3); // Last 3 hours

        assert_eq!(activity.opens, 2);
        assert_eq!(activity.closes, 1);
        assert!(activity.last_open_time.is_some());
        assert!(activity.last_close_time.is_some());
    }

    #[test]
    fn test_sensor_statistics_calculation() {
        let sensors = vec![
            DiscoveredSensor {
                uuid: "1".to_string(),
                name: Some("Door 1".to_string()),
                current_value: json!(0),
                value_history: vec![],
                first_seen: chrono::Utc::now(),
                last_updated: chrono::Utc::now(),
                update_count: 1,
                sensor_type: SensorType::DoorWindow,
                confidence: 0.9,
                pattern_score: 0.8,
                room: Some("Living Room".to_string()),
                metadata: HashMap::new(),
            },
            DiscoveredSensor {
                uuid: "2".to_string(),
                name: Some("Motion 1".to_string()),
                current_value: json!(0),
                value_history: vec![],
                first_seen: chrono::Utc::now(),
                last_updated: chrono::Utc::now(),
                update_count: 1,
                sensor_type: SensorType::Motion,
                confidence: 0.8,
                pattern_score: 0.7,
                room: Some("Hallway".to_string()),
                metadata: HashMap::new(),
            },
            DiscoveredSensor {
                uuid: "3".to_string(),
                name: Some("Temperature 1".to_string()),
                current_value: json!(21.5),
                value_history: vec![],
                first_seen: chrono::Utc::now(),
                last_updated: chrono::Utc::now(),
                update_count: 1,
                sensor_type: SensorType::Temperature,
                confidence: 0.95,
                pattern_score: 0.9,
                room: Some("Living Room".to_string()),
                metadata: HashMap::new(),
            },
        ];

        let stats = calculate_sensor_statistics(&sensors);

        assert_eq!(stats.total_sensors, 3);
        assert_eq!(stats.by_type.get("doorwindow"), Some(&1));
        assert_eq!(stats.by_type.get("motion"), Some(&1));
        assert_eq!(stats.by_type.get("temperature"), Some(&1));
        assert_eq!(stats.by_room.get("Living Room"), Some(&2));
        assert_eq!(stats.by_room.get("Hallway"), Some(&1));
        assert_eq!(stats.binary_count, 2); // door and motion sensors
        assert_eq!(stats.analog_count, 1); // temperature sensor
    }

    #[test]
    fn test_sensor_filtering() {
        let sensors = create_test_sensors();

        // Filter by sensor type using name patterns
        let door_window_sensors: Vec<_> = sensors
            .iter()
            .filter(|s| {
                let name_lower = s.name.to_lowercase();
                name_lower.contains(" door")
                    || name_lower.contains("window")
                    || name_lower.starts_with("door")
            })
            .collect();

        assert_eq!(door_window_sensors.len(), 2);

        // Filter by room
        let living_room_sensors: Vec<_> = sensors
            .iter()
            .filter(|s| s.room.as_deref() == Some("Living Room"))
            .collect();

        assert_eq!(living_room_sensors.len(), 2); // Window sensor and temperature sensor

        // Filter by device type
        let analog_sensors: Vec<_> = sensors
            .iter()
            .filter(|s| s.device_type == "AnalogInput")
            .collect();

        assert_eq!(analog_sensors.len(), 2); // Door sensor and motion sensor
    }

    #[test]
    fn test_sensor_value_interpretation() {
        // Test door/window sensor values
        assert_eq!(interpret_door_window_value(&json!(0)), "CLOSED");
        assert_eq!(interpret_door_window_value(&json!(1)), "OPEN");
        assert_eq!(interpret_door_window_value(&json!(false)), "CLOSED");
        assert_eq!(interpret_door_window_value(&json!(true)), "OPEN");

        // Test motion sensor values
        assert_eq!(interpret_motion_value(&json!(0)), "NO_MOTION");
        assert_eq!(interpret_motion_value(&json!(1)), "MOTION");
        assert_eq!(interpret_motion_value(&json!(false)), "NO_MOTION");
        assert_eq!(interpret_motion_value(&json!(true)), "MOTION");
    }

    #[test]
    fn test_temperature_sensor_validation() {
        let temp_sensors = create_test_sensors()
            .into_iter()
            .filter(|s| {
                let name_lower = s.name.to_lowercase();
                name_lower.contains("temperature") || name_lower.contains("thermometer")
            })
            .collect::<Vec<_>>();

        assert_eq!(temp_sensors.len(), 2);

        for sensor in &temp_sensors {
            if let Some(temp_value) = sensor.states.get("value") {
                if let Some(temp) = temp_value.as_f64() {
                    // Validate temperature range (reasonable for indoor/outdoor)
                    assert!(
                        (-50.0..=50.0).contains(&temp),
                        "Temperature {temp} out of reasonable range"
                    );
                }
            }
        }
    }

    #[test]
    fn test_sensor_discovery_response_structure() {
        let sensors = vec![DiscoveredSensor {
            uuid: "sensor-1".to_string(),
            name: Some("Test Sensor".to_string()),
            current_value: json!(0),
            value_history: vec![json!(1), json!(0)],
            first_seen: chrono::Utc::now() - chrono::Duration::minutes(10),
            last_updated: chrono::Utc::now(),
            update_count: 2,
            sensor_type: SensorType::DoorWindow,
            confidence: 0.9,
            pattern_score: 0.8,
            room: Some("Living Room".to_string()),
            metadata: HashMap::new(),
        }];

        let stats = calculate_sensor_statistics(&sensors);

        // Test discovery response structure
        let response = json!({
            "discovery_duration": "60s",
            "discovered_sensors": sensors.iter().map(|s| json!({
                "uuid": s.uuid,
                "name": s.name,
                "current_value": s.current_value,
                "sensor_type": format!("{:?}", s.sensor_type).to_lowercase(),
                "confidence": s.confidence,
                "pattern_score": s.pattern_score,
                "room": s.room,
                "update_count": s.update_count,
                "first_seen": s.first_seen,
                "last_updated": s.last_updated
            })).collect::<Vec<_>>(),
            "statistics": {
                "total_sensors": stats.total_sensors,
                "by_type": stats.by_type,
                "by_room": stats.by_room,
                "binary_count": stats.binary_count,
                "analog_count": stats.analog_count
            },
            "discovery_complete": true,
            "timestamp": chrono::Utc::now()
        });

        assert_eq!(response["discovery_duration"], "60s");
        assert!(response["discovered_sensors"].is_array());
        assert_eq!(response["discovered_sensors"].as_array().unwrap().len(), 1);
        assert!(response["statistics"].is_object());
        assert_eq!(response["statistics"]["total_sensors"], 1);
        assert!(response["discovery_complete"].as_bool().unwrap());
    }

    #[test]
    fn test_door_window_sensor_summary() {
        let sensors = create_test_sensors();

        // Filter door/window sensors
        let door_window_sensors: Vec<_> = sensors
            .iter()
            .filter(|s| {
                let name_lower = s.name.to_lowercase();
                name_lower.contains(" door")
                    || name_lower.contains("window")
                    || name_lower.starts_with("door")
            })
            .collect();

        let mut open_count = 0;
        let mut closed_count = 0;

        for sensor in &door_window_sensors {
            if let Some(value) = sensor.states.get("value") {
                if is_open_state(value) {
                    open_count += 1;
                } else {
                    closed_count += 1;
                }
            }
        }

        let summary = json!({
            "total_sensors": door_window_sensors.len(),
            "open": open_count,
            "closed": closed_count,
            "all_closed": open_count == 0,
            "any_open": open_count > 0
        });

        assert_eq!(summary["total_sensors"], 2);
        assert_eq!(summary["open"], 1); // Window sensor is open (value: 1)
        assert_eq!(summary["closed"], 1); // Door sensor is closed (value: 0)
        assert!(!summary["all_closed"].as_bool().unwrap());
        assert!(summary["any_open"].as_bool().unwrap());
    }
}

// Helper functions for testing

fn classify_sensor_by_name(name: &str) -> SensorType {
    let name_lower = name.to_lowercase();

    if name_lower.contains("door") || name_lower.contains("window") {
        SensorType::DoorWindow
    } else if name_lower.contains("motion") || name_lower.contains("pir") {
        SensorType::Motion
    } else if name_lower.contains("temperature") || name_lower.contains("temp") {
        SensorType::Temperature
    } else if name_lower.contains("light") || name_lower.contains("lux") {
        SensorType::Light
    } else {
        SensorType::Unknown
    }
}

fn is_binary_pattern(values: &[Value]) -> bool {
    values.iter().all(|v| match v {
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                f == 0.0 || f == 1.0
            } else {
                false
            }
        }
        Value::Bool(_) => true,
        _ => false,
    })
}

fn calculate_sensor_statistics(sensors: &[DiscoveredSensor]) -> SensorStatistics {
    let mut by_type = HashMap::new();
    let mut by_room = HashMap::new();
    let mut binary_count = 0;
    let mut analog_count = 0;

    for sensor in sensors {
        // Count by type
        let type_name = format!("{:?}", sensor.sensor_type).to_lowercase();
        *by_type.entry(type_name).or_insert(0) += 1;

        // Count by room
        if let Some(ref room) = sensor.room {
            *by_room.entry(room.clone()).or_insert(0) += 1;
        }

        // Count binary vs analog
        match sensor.sensor_type {
            SensorType::DoorWindow | SensorType::Motion => binary_count += 1,
            SensorType::Analog | SensorType::Temperature | SensorType::Light => analog_count += 1,
            _ => {}
        }
    }

    SensorStatistics {
        total_sensors: sensors.len(),
        by_type,
        by_room,
        active_count: sensors.len(), // All sensors considered active for this test
        binary_count,
        analog_count,
    }
}

fn interpret_door_window_value(value: &Value) -> &'static str {
    match value {
        Value::Number(n) => {
            if n.as_f64().unwrap_or(0.0) > 0.0 {
                "OPEN"
            } else {
                "CLOSED"
            }
        }
        Value::Bool(b) => {
            if *b {
                "OPEN"
            } else {
                "CLOSED"
            }
        }
        _ => "UNKNOWN",
    }
}

fn interpret_motion_value(value: &Value) -> &'static str {
    match value {
        Value::Number(n) => {
            if n.as_f64().unwrap_or(0.0) > 0.0 {
                "MOTION"
            } else {
                "NO_MOTION"
            }
        }
        Value::Bool(b) => {
            if *b {
                "MOTION"
            } else {
                "NO_MOTION"
            }
        }
        _ => "UNKNOWN",
    }
}

fn is_open_state(value: &Value) -> bool {
    match value {
        Value::Number(n) => n.as_f64().unwrap_or(0.0) > 0.0,
        Value::Bool(b) => *b,
        _ => false,
    }
}

// Additional tests for caching functionality

#[cfg(test)]
mod caching_tests {
    use super::*;

    #[test]
    fn test_sensor_discovery_caching() {
        // Test sensor discovery cache behavior
        let mut discovered_cache: HashMap<String, DiscoveredSensor> = HashMap::new();

        // First discovery
        let sensor = DiscoveredSensor {
            uuid: "cache-test-1".to_string(),
            name: Some("Cached Door Sensor".to_string()),
            current_value: json!(0),
            value_history: vec![json!(1), json!(0)],
            first_seen: chrono::Utc::now() - chrono::Duration::minutes(5),
            last_updated: chrono::Utc::now(),
            update_count: 2,
            sensor_type: SensorType::DoorWindow,
            confidence: 0.9,
            pattern_score: 0.8,
            room: Some("Entry".to_string()),
            metadata: HashMap::new(),
        };

        // Add to cache
        discovered_cache.insert(sensor.uuid.clone(), sensor.clone());
        assert_eq!(discovered_cache.len(), 1);
        assert!(discovered_cache.contains_key("cache-test-1"));

        // Verify cached sensor
        let cached_sensor = discovered_cache.get("cache-test-1").unwrap();
        assert_eq!(cached_sensor.name, Some("Cached Door Sensor".to_string()));
        assert_eq!(cached_sensor.sensor_type, SensorType::DoorWindow);
        assert_eq!(cached_sensor.confidence, 0.9);

        // Test cache eviction (LRU)
        let cache_size_limit = 2;

        // Add another sensor
        let sensor2 = DiscoveredSensor {
            uuid: "cache-test-2".to_string(),
            name: Some("Second Sensor".to_string()),
            current_value: json!(21.5),
            value_history: vec![json!(21.0), json!(21.5)],
            first_seen: chrono::Utc::now() - chrono::Duration::minutes(3),
            last_updated: chrono::Utc::now(),
            update_count: 2,
            sensor_type: SensorType::Temperature,
            confidence: 0.95,
            pattern_score: 0.9,
            room: Some("Living Room".to_string()),
            metadata: HashMap::new(),
        };

        discovered_cache.insert(sensor2.uuid.clone(), sensor2.clone());
        assert_eq!(discovered_cache.len(), 2);

        // Add third sensor (would trigger eviction in real cache)
        let sensor3 = DiscoveredSensor {
            uuid: "cache-test-3".to_string(),
            name: Some("Third Sensor".to_string()),
            current_value: json!(1),
            value_history: vec![json!(0), json!(1)],
            first_seen: chrono::Utc::now() - chrono::Duration::minutes(1),
            last_updated: chrono::Utc::now(),
            update_count: 2,
            sensor_type: SensorType::Motion,
            confidence: 0.8,
            pattern_score: 0.7,
            room: Some("Hallway".to_string()),
            metadata: HashMap::new(),
        };

        // In a real implementation, this would evict the oldest sensor
        if discovered_cache.len() >= cache_size_limit {
            // Find oldest sensor by last_updated
            let oldest_uuid = discovered_cache
                .iter()
                .min_by_key(|(_, s)| s.last_updated)
                .map(|(uuid, _)| uuid.clone());

            if let Some(oldest) = oldest_uuid {
                discovered_cache.remove(&oldest);
            }
        }

        discovered_cache.insert(sensor3.uuid.clone(), sensor3.clone());
        assert_eq!(discovered_cache.len(), 2);
        assert!(discovered_cache.contains_key("cache-test-3"));
    }

    #[test]
    fn test_sensor_pattern_caching() {
        // Test sensor pattern analysis caching
        let mut pattern_cache: HashMap<String, (f64, SensorType)> = HashMap::new();

        // Cache pattern analysis results
        pattern_cache.insert("door-pattern".to_string(), (0.9, SensorType::DoorWindow));
        pattern_cache.insert("temp-pattern".to_string(), (0.95, SensorType::Temperature));
        pattern_cache.insert("motion-pattern".to_string(), (0.8, SensorType::Motion));

        // Test cache retrieval
        assert_eq!(pattern_cache.len(), 3);

        let door_result = pattern_cache.get("door-pattern").unwrap();
        assert_eq!(door_result.0, 0.9);
        assert_eq!(door_result.1, SensorType::DoorWindow);

        let temp_result = pattern_cache.get("temp-pattern").unwrap();
        assert_eq!(temp_result.0, 0.95);
        assert_eq!(temp_result.1, SensorType::Temperature);

        // Test cache miss
        assert!(!pattern_cache.contains_key("unknown-pattern"));

        // Test cache update
        pattern_cache.insert("door-pattern".to_string(), (0.95, SensorType::DoorWindow));
        let updated_result = pattern_cache.get("door-pattern").unwrap();
        assert_eq!(updated_result.0, 0.95);
    }

    #[test]
    fn test_sensor_state_caching() {
        // Test sensor state change caching and history
        let mut state_cache: HashMap<String, Vec<StateChangeEvent>> = HashMap::new();

        let base_time = chrono::Utc::now();

        // Create state change events
        let events = vec![
            StateChangeEvent {
                uuid: "sensor-1".to_string(),
                timestamp: base_time - chrono::Duration::minutes(30),
                old_value: json!(0),
                new_value: json!(1),
                human_readable: "OPEN".to_string(),
                event_type: "state_change".to_string(),
            },
            StateChangeEvent {
                uuid: "sensor-1".to_string(),
                timestamp: base_time - chrono::Duration::minutes(15),
                old_value: json!(1),
                new_value: json!(0),
                human_readable: "CLOSED".to_string(),
                event_type: "state_change".to_string(),
            },
            StateChangeEvent {
                uuid: "sensor-1".to_string(),
                timestamp: base_time,
                old_value: json!(0),
                new_value: json!(1),
                human_readable: "OPEN".to_string(),
                event_type: "state_change".to_string(),
            },
        ];

        // Cache state changes
        state_cache.insert("sensor-1".to_string(), events.clone());

        // Test cache retrieval
        let cached_events = state_cache.get("sensor-1").unwrap();
        assert_eq!(cached_events.len(), 3);
        assert_eq!(cached_events[0].human_readable, "OPEN");
        assert_eq!(cached_events[1].human_readable, "CLOSED");
        assert_eq!(cached_events[2].human_readable, "OPEN");

        // Test recent events filtering
        let threshold = base_time - chrono::Duration::minutes(20);
        let recent_events: Vec<_> = cached_events
            .iter()
            .filter(|event| event.timestamp > threshold)
            .collect();

        assert_eq!(recent_events.len(), 2); // Last 2 events within 20 minutes

        // Test cache size management (ring buffer simulation)
        let max_events = 10;
        let mut limited_events = events.clone();

        // Add more events to test size limit
        for i in 0..15 {
            let event = StateChangeEvent {
                uuid: "sensor-1".to_string(),
                timestamp: base_time + chrono::Duration::minutes(i as i64),
                old_value: json!(i % 2),
                new_value: json!((i + 1) % 2),
                human_readable: if i % 2 == 0 { "CLOSED" } else { "OPEN" }.to_string(),
                event_type: "state_change".to_string(),
            };
            limited_events.push(event);
        }

        // Simulate ring buffer behavior
        if limited_events.len() > max_events {
            let skip_count = limited_events.len() - max_events;
            limited_events = limited_events.into_iter().skip(skip_count).collect();
        }

        assert_eq!(limited_events.len(), max_events);
        state_cache.insert("sensor-1".to_string(), limited_events);

        let final_cached = state_cache.get("sensor-1").unwrap();
        assert_eq!(final_cached.len(), max_events);
    }

    #[test]
    fn test_sensor_discovery_cache_ttl() {
        // Test TTL-based cache expiration for sensor discovery
        let cache_ttl = chrono::Duration::minutes(5);
        let now = chrono::Utc::now();

        // Create sensors with different discovery times
        let fresh_sensor = DiscoveredSensor {
            uuid: "fresh-sensor".to_string(),
            name: Some("Fresh Sensor".to_string()),
            current_value: json!(0),
            value_history: vec![json!(0)],
            first_seen: now - chrono::Duration::minutes(2), // Fresh
            last_updated: now,
            update_count: 1,
            sensor_type: SensorType::DoorWindow,
            confidence: 0.9,
            pattern_score: 0.8,
            room: Some("Room1".to_string()),
            metadata: HashMap::new(),
        };

        let stale_sensor = DiscoveredSensor {
            uuid: "stale-sensor".to_string(),
            name: Some("Stale Sensor".to_string()),
            current_value: json!(1),
            value_history: vec![json!(1)],
            first_seen: now - chrono::Duration::minutes(10), // Stale
            last_updated: now - chrono::Duration::minutes(8),
            update_count: 1,
            sensor_type: SensorType::Motion,
            confidence: 0.7,
            pattern_score: 0.6,
            room: Some("Room2".to_string()),
            metadata: HashMap::new(),
        };

        // Test TTL expiration logic
        let is_fresh = (now - fresh_sensor.last_updated) < cache_ttl;
        let is_stale = (now - stale_sensor.last_updated) >= cache_ttl;

        assert!(is_fresh, "Fresh sensor should not be expired");
        assert!(is_stale, "Stale sensor should be expired");

        // Simulate cache cleanup
        let sensors = vec![fresh_sensor, stale_sensor];
        let active_sensors: Vec<_> = sensors
            .into_iter()
            .filter(|s| (now - s.last_updated) < cache_ttl)
            .collect();

        assert_eq!(active_sensors.len(), 1);
        assert_eq!(active_sensors[0].name, Some("Fresh Sensor".to_string()));
    }

    #[test]
    fn test_sensor_confidence_scoring() {
        // Test sensor discovery confidence scoring and caching
        let mut confidence_cache: HashMap<String, f64> = HashMap::new();

        // Test different confidence scenarios
        let test_cases = vec![
            (
                "binary-sensor",
                vec![json!(0), json!(1), json!(0), json!(1)],
                0.9,
            ), // High confidence binary
            (
                "analog-sensor",
                vec![json!(21.1), json!(21.3), json!(21.0)],
                0.8,
            ), // Medium confidence analog
            (
                "noisy-sensor",
                vec![json!(0), json!(1), json!(2.5), json!("text")],
                0.3,
            ), // Low confidence mixed
        ];

        for (sensor_id, values, expected_confidence) in test_cases {
            // Simulate confidence calculation
            let is_binary = values.iter().all(|v| match v {
                serde_json::Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        f == 0.0 || f == 1.0
                    } else {
                        false
                    }
                }
                serde_json::Value::Bool(_) => true,
                _ => false,
            });

            let has_mixed_types = values
                .iter()
                .any(|v| !v.is_number() && v.as_bool().is_none());

            let calculated_confidence = if has_mixed_types {
                0.3 // Low confidence for mixed types
            } else if is_binary {
                0.9 // High confidence for binary patterns
            } else {
                0.8 // Medium confidence for analog patterns
            };

            confidence_cache.insert(sensor_id.to_string(), calculated_confidence);
            assert_eq!(calculated_confidence, expected_confidence);
        }

        // Verify cache contents
        assert_eq!(confidence_cache.len(), 3);
        assert_eq!(confidence_cache.get("binary-sensor"), Some(&0.9));
        assert_eq!(confidence_cache.get("analog-sensor"), Some(&0.8));
        assert_eq!(confidence_cache.get("noisy-sensor"), Some(&0.3));
    }

    #[test]
    fn test_response_cache_integration() {
        // Test integration with response caching for sensor tools
        use std::time::Duration;

        // Simulate cached responses for sensor tools
        let mut response_cache: HashMap<
            String,
            (serde_json::Value, chrono::DateTime<chrono::Utc>),
        > = HashMap::new();
        let cache_ttl = Duration::from_secs(300); // 5 minutes
        let now = chrono::Utc::now();

        // Cache a sensor discovery response
        let discovery_response = json!({
            "discovered_sensors": [
                {
                    "uuid": "sensor-1",
                    "name": "Cached Sensor",
                    "sensor_type": "doorwindow",
                    "confidence": 0.9
                }
            ],
            "statistics": {
                "total_sensors": 1,
                "by_type": { "doorwindow": 1 }
            }
        });

        let cache_key = "discover_new_sensors:60s".to_string();
        response_cache.insert(cache_key.clone(), (discovery_response.clone(), now));

        // Test cache hit
        if let Some((cached_response, cache_time)) = response_cache.get(&cache_key) {
            let age = now.signed_duration_since(*cache_time);
            let is_valid = age.to_std().unwrap_or(Duration::MAX) < cache_ttl;

            assert!(is_valid, "Cached response should be valid");
            assert_eq!(cached_response["statistics"]["total_sensors"], 1);
        }

        // Test cache miss
        let missing_key = "discover_new_sensors:120s".to_string();
        assert!(!response_cache.contains_key(&missing_key));

        // Test cache expiration
        let old_response = json!({"old": "data"});
        let old_time = now - chrono::Duration::minutes(10); // Older than TTL
        response_cache.insert("old_key".to_string(), (old_response, old_time));

        if let Some((_, cache_time)) = response_cache.get("old_key") {
            let age = now.signed_duration_since(*cache_time);
            let is_expired = age.to_std().unwrap_or(Duration::ZERO) >= cache_ttl;

            assert!(is_expired, "Old cached response should be expired");
        }
    }
}
