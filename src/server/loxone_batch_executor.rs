//! Loxone-specific implementation of BatchExecutor for request coalescing

use super::request_coalescing::BatchExecutor;
use crate::client::LoxoneClient;
use crate::error::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

/// Loxone-specific batch executor implementation
pub struct LoxoneBatchExecutor {
    client: Arc<dyn LoxoneClient + Send + Sync>,
}

impl LoxoneBatchExecutor {
    /// Create a new Loxone batch executor
    pub fn new(client: Arc<dyn LoxoneClient + Send + Sync>) -> Self {
        Self { client }
    }

    /// Helper method to get device states efficiently
    async fn get_multiple_device_states(
        &self,
        device_uuids: &[String],
    ) -> Result<HashMap<String, Value>> {
        debug!("Fetching states for {} devices", device_uuids.len());

        // Get the structure first to validate devices exist
        let structure = self.client.get_structure().await?;

        // Use the batched get_device_states method
        match self.client.get_device_states(device_uuids).await {
            Ok(states) => {
                let mut results = HashMap::new();

                for uuid in device_uuids {
                    if let Some(device) = structure.controls.get(uuid) {
                        let state_value =
                            states.get(uuid).cloned().unwrap_or(serde_json::Value::Null);
                        results.insert(uuid.clone(), serde_json::json!({
                            "uuid": uuid,
                            "name": device.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                            "type": device.get("type").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                            "room": device.get("room").and_then(|v| v.as_str()).unwrap_or(""),
                            "state": state_value,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }));
                    } else {
                        // Device not found in structure
                        results.insert(
                            uuid.clone(),
                            serde_json::json!({
                                "uuid": uuid,
                                "state": null,
                                "error": "Device not found in structure",
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            }),
                        );
                    }
                }

                Ok(results)
            }
            Err(e) => {
                warn!("Failed to get device states: {}", e);
                // Fall back to individual error responses
                let mut results = HashMap::new();
                for uuid in device_uuids {
                    results.insert(
                        uuid.clone(),
                        serde_json::json!({
                            "uuid": uuid,
                            "state": null,
                            "error": e.to_string(),
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }),
                    );
                }
                Ok(results)
            }
        }
    }

    /// Helper method to get devices for multiple rooms
    async fn get_devices_for_rooms(
        &self,
        room_uuids: &[String],
    ) -> Result<HashMap<String, Vec<Value>>> {
        debug!("Fetching devices for {} rooms", room_uuids.len());

        let structure = self.client.get_structure().await?;
        let mut results = HashMap::new();

        for room_uuid in room_uuids {
            if let Some(_room) = structure.rooms.get(room_uuid) {
                // Find all devices in this room
                let room_devices: Vec<Value> = structure.controls
                    .values()
                    .filter(|device| {
                        device.get("room")
                            .and_then(|v| v.as_str())
                            .map(|r| r == room_uuid)
                            .unwrap_or(false)
                    })
                    .map(|device| serde_json::json!({
                        "uuid": device.get("uuid").and_then(|v| v.as_str()).unwrap_or(""),
                        "name": device.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                        "type": device.get("type").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                        "category": device.get("cat").and_then(|v| v.as_str()).unwrap_or(""),
                        "room": device.get("room").and_then(|v| v.as_str()).unwrap_or(""),
                        "defaultIcon": device.get("defaultIcon").and_then(|v| v.as_str()),
                        "isFavorite": device.get("isFavorite").and_then(|v| v.as_bool()).unwrap_or(false),
                        "isSecured": device.get("isSecured").and_then(|v| v.as_bool()).unwrap_or(false)
                    }))
                    .collect();

                results.insert(room_uuid.clone(), room_devices);
            } else {
                // Room not found
                results.insert(room_uuid.clone(), vec![]);
            }
        }

        Ok(results)
    }

    /// Helper method to get multiple sensor readings
    async fn get_multiple_sensor_readings(
        &self,
        sensor_uuids: &[String],
    ) -> Result<HashMap<String, Value>> {
        debug!("Fetching readings for {} sensors", sensor_uuids.len());

        let structure = self.client.get_structure().await?;

        // Use the batched get_device_states method for sensors too
        match self.client.get_device_states(sensor_uuids).await {
            Ok(states) => {
                let mut results = HashMap::new();

                for uuid in sensor_uuids {
                    if let Some(control) = structure.controls.get(uuid) {
                        let state_value =
                            states.get(uuid).cloned().unwrap_or(serde_json::Value::Null);
                        let device_type = control
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown");
                        results.insert(uuid.clone(), serde_json::json!({
                            "uuid": uuid,
                            "name": control.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                            "type": device_type,
                            "category": control.get("cat").and_then(|v| v.as_str()).unwrap_or(""),
                            "room": control.get("room").and_then(|v| v.as_str()).unwrap_or(""),
                            "value": state_value,
                            "unit": self.get_sensor_unit(device_type),
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }));
                    } else {
                        // Sensor not found
                        results.insert(
                            uuid.clone(),
                            serde_json::json!({
                                "uuid": uuid,
                                "value": null,
                                "error": "Sensor not found in structure",
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            }),
                        );
                    }
                }

                Ok(results)
            }
            Err(e) => {
                warn!("Failed to get sensor states: {}", e);
                // Fall back to individual error responses
                let mut results = HashMap::new();
                for uuid in sensor_uuids {
                    results.insert(
                        uuid.clone(),
                        serde_json::json!({
                            "uuid": uuid,
                            "value": null,
                            "error": e.to_string(),
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }),
                    );
                }
                Ok(results)
            }
        }
    }

    /// Get the appropriate unit for a sensor type
    fn get_sensor_unit(&self, sensor_type: &str) -> Option<String> {
        match sensor_type.to_lowercase().as_str() {
            "temperature" | "temperatur" => Some("°C".to_string()),
            "humidity" | "feuchtigkeit" => Some("%".to_string()),
            "brightness" | "helligkeit" => Some("lux".to_string()),
            "pressure" | "druck" => Some("hPa".to_string()),
            "windspeed" | "windgeschwindigkeit" => Some("km/h".to_string()),
            "precipitation" | "niederschlag" => Some("mm".to_string()),
            "energy" | "energie" => Some("kWh".to_string()),
            "power" | "leistung" => Some("W".to_string()),
            "voltage" | "spannung" => Some("V".to_string()),
            "current" | "strom" => Some("A".to_string()),
            _ => None,
        }
    }

    /// Helper method to get multiple structure information types
    async fn get_multiple_structure_info(
        &self,
        info_types: &[String],
    ) -> Result<HashMap<String, Value>> {
        debug!("Fetching structure info for {} types", info_types.len());

        let mut results = HashMap::new();

        for info_type in info_types {
            match info_type.as_str() {
                "rooms" => match self.client.get_structure().await {
                    Ok(structure) => {
                        let rooms: Vec<Value> = structure.rooms
                                .values()
                                .map(|room| serde_json::json!({
                                    "uuid": room.get("uuid").and_then(|v| v.as_str()).unwrap_or(""),
                                    "name": room.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                                    "image": room.get("image").and_then(|v| v.as_str()),
                                    "defaultRating": room.get("defaultRating").and_then(|v| v.as_i64()).unwrap_or(0),
                                    "isFavorite": room.get("isFavorite").and_then(|v| v.as_bool()).unwrap_or(false),
                                    "type": room.get("type").and_then(|v| v.as_i64()).unwrap_or(0)
                                }))
                                .collect();

                        results.insert(info_type.clone(), Value::Array(rooms));
                    }
                    Err(e) => {
                        results.insert(
                            info_type.clone(),
                            serde_json::json!({
                                "error": e.to_string()
                            }),
                        );
                    }
                },

                "categories" => match self.client.get_structure().await {
                    Ok(structure) => {
                        let categories: Vec<Value> = structure.cats
                                .values()
                                .map(|cat| serde_json::json!({
                                    "uuid": cat.get("uuid").and_then(|v| v.as_str()).unwrap_or(""),
                                    "name": cat.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                                    "image": cat.get("image").and_then(|v| v.as_str()),
                                    "defaultRating": cat.get("defaultRating").and_then(|v| v.as_i64()).unwrap_or(0),
                                    "isFavorite": cat.get("isFavorite").and_then(|v| v.as_bool()).unwrap_or(false),
                                    "type": cat.get("type").and_then(|v| v.as_i64()).unwrap_or(0),
                                    "color": cat.get("color").and_then(|v| v.as_str())
                                }))
                                .collect();

                        results.insert(info_type.clone(), Value::Array(categories));
                    }
                    Err(e) => {
                        results.insert(
                            info_type.clone(),
                            serde_json::json!({
                                "error": e.to_string()
                            }),
                        );
                    }
                },

                "controls" => match self.client.get_structure().await {
                    Ok(structure) => {
                        let controls: Vec<Value> = structure.controls
                                .values()
                                .map(|control| serde_json::json!({
                                    "uuid": control.get("uuid").and_then(|v| v.as_str()).unwrap_or(""),
                                    "name": control.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                                    "type": control.get("type").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                                    "category": control.get("cat").and_then(|v| v.as_str()).unwrap_or(""),
                                    "room": control.get("room").and_then(|v| v.as_str()).unwrap_or(""),
                                    "defaultIcon": control.get("defaultIcon").and_then(|v| v.as_str()),
                                    "isFavorite": control.get("isFavorite").and_then(|v| v.as_bool()).unwrap_or(false),
                                    "isSecured": control.get("isSecured").and_then(|v| v.as_bool()).unwrap_or(false)
                                }))
                                .collect();

                        results.insert(info_type.clone(), Value::Array(controls));
                    }
                    Err(e) => {
                        results.insert(
                            info_type.clone(),
                            serde_json::json!({
                                "error": e.to_string()
                            }),
                        );
                    }
                },

                "system_info" | "version" => match self.client.get_system_info().await {
                    Ok(system_info) => {
                        results.insert(info_type.clone(), system_info);
                    }
                    Err(e) => {
                        results.insert(
                            info_type.clone(),
                            serde_json::json!({
                                "error": e.to_string()
                            }),
                        );
                    }
                },

                _ => {
                    results.insert(
                        info_type.clone(),
                        serde_json::json!({
                            "error": format!("Unknown info type: {}", info_type)
                        }),
                    );
                }
            }
        }

        Ok(results)
    }
}

#[async_trait::async_trait]
impl BatchExecutor for LoxoneBatchExecutor {
    /// Execute a batch of device state requests
    async fn execute_device_state_batch(
        &self,
        device_uuids: Vec<String>,
    ) -> Result<HashMap<String, Value>> {
        if device_uuids.is_empty() {
            return Ok(HashMap::new());
        }

        debug!(
            "Executing device state batch for {} devices",
            device_uuids.len()
        );
        self.get_multiple_device_states(&device_uuids).await
    }

    /// Execute a batch of room device queries
    async fn execute_room_devices_batch(
        &self,
        room_uuids: Vec<String>,
    ) -> Result<HashMap<String, Vec<Value>>> {
        if room_uuids.is_empty() {
            return Ok(HashMap::new());
        }

        debug!(
            "Executing room devices batch for {} rooms",
            room_uuids.len()
        );
        self.get_devices_for_rooms(&room_uuids).await
    }

    /// Execute a batch of sensor readings
    async fn execute_sensor_batch(
        &self,
        sensor_uuids: Vec<String>,
    ) -> Result<HashMap<String, Value>> {
        if sensor_uuids.is_empty() {
            return Ok(HashMap::new());
        }

        debug!("Executing sensor batch for {} sensors", sensor_uuids.len());
        self.get_multiple_sensor_readings(&sensor_uuids).await
    }

    /// Execute a batch of structure info requests
    async fn execute_structure_batch(
        &self,
        info_types: Vec<String>,
    ) -> Result<HashMap<String, Value>> {
        if info_types.is_empty() {
            return Ok(HashMap::new());
        }

        debug!(
            "Executing structure batch for {} info types",
            info_types.len()
        );
        self.get_multiple_structure_info(&info_types).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{LoxoneResponse, LoxoneStructure};
    use async_trait::async_trait;
    use std::collections::HashMap;

    struct MockLoxoneClient {
        structure: LoxoneStructure,
    }

    impl MockLoxoneClient {
        fn new() -> Self {
            let mut controls = HashMap::new();
            controls.insert(
                "device1".to_string(),
                serde_json::json!({
                    "uuid": "device1",
                    "name": "Test Device 1",
                    "type": "Switch",
                    "cat": "cat1",
                    "room": "room1",
                    "defaultIcon": "icon1",
                    "isFavorite": false,
                    "isSecured": false,
                    "uuidAction": "action1"
                }),
            );

            let mut rooms = HashMap::new();
            rooms.insert(
                "room1".to_string(),
                serde_json::json!({
                    "uuid": "room1",
                    "name": "Test Room",
                    "image": "room_image",
                    "defaultRating": 0,
                    "isFavorite": false,
                    "type": 0
                }),
            );

            Self {
                structure: LoxoneStructure {
                    last_modified: "2024-01-01T00:00:00Z".to_string(),
                    controls,
                    rooms,
                    cats: HashMap::new(),
                    global_states: HashMap::new(),
                },
            }
        }
    }

    #[async_trait]
    impl LoxoneClient for MockLoxoneClient {
        async fn connect(&mut self) -> Result<()> {
            Ok(())
        }

        async fn is_connected(&self) -> Result<bool> {
            Ok(true)
        }

        async fn disconnect(&mut self) -> Result<()> {
            Ok(())
        }

        async fn get_structure(&self) -> Result<LoxoneStructure> {
            Ok(self.structure.clone())
        }

        async fn send_command(&self, _uuid: &str, _command: &str) -> Result<LoxoneResponse> {
            Ok(LoxoneResponse {
                code: 200,
                value: serde_json::json!({"value": 1}),
            })
        }

        async fn get_device_states(
            &self,
            uuids: &[String],
        ) -> Result<HashMap<String, serde_json::Value>> {
            let mut results = HashMap::new();
            for uuid in uuids {
                results.insert(
                    uuid.clone(),
                    serde_json::json!({
                        "uuid": uuid,
                        "value": 1.0,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }),
                );
            }
            Ok(results)
        }

        async fn get_system_info(&self) -> Result<serde_json::Value> {
            Ok(serde_json::json!({
                "version": "12.3.4.5",
                "serialNumber": "12345678",
                "macAddress": "AA:BB:CC:DD:EE:FF"
            }))
        }

        async fn get_state_values(
            &self,
            state_uuids: &[String],
        ) -> Result<HashMap<String, serde_json::Value>> {
            let mut results = HashMap::new();
            for state_uuid in state_uuids {
                results.insert(
                    state_uuid.clone(),
                    serde_json::json!(0.5), // Mock value
                );
            }
            Ok(results)
        }

        async fn health_check(&self) -> Result<bool> {
            Ok(true)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_device_state_batch() {
        let client = Arc::new(MockLoxoneClient::new());
        let executor = LoxoneBatchExecutor::new(client);

        let device_uuids = vec!["device1".to_string(), "nonexistent".to_string()];
        let results = executor
            .execute_device_state_batch(device_uuids)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.contains_key("device1"));
        assert!(results.contains_key("nonexistent"));

        let device1_result = &results["device1"];
        assert_eq!(device1_result["name"], "Test Device 1");
        assert_eq!(device1_result["type"], "Switch");
    }

    #[tokio::test]
    async fn test_room_devices_batch() {
        let client = Arc::new(MockLoxoneClient::new());
        let executor = LoxoneBatchExecutor::new(client);

        let room_uuids = vec!["room1".to_string()];
        let results = executor
            .execute_room_devices_batch(room_uuids)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results.contains_key("room1"));

        let room_devices = &results["room1"];
        assert_eq!(room_devices.len(), 1);
        assert_eq!(room_devices[0]["name"], "Test Device 1");
    }

    #[tokio::test]
    async fn test_structure_batch() {
        let client = Arc::new(MockLoxoneClient::new());
        let executor = LoxoneBatchExecutor::new(client);

        let info_types = vec!["rooms".to_string(), "system_info".to_string()];
        let results = executor.execute_structure_batch(info_types).await.unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.contains_key("rooms"));
        assert!(results.contains_key("system_info"));

        let rooms = &results["rooms"];
        assert!(rooms.is_array());
        assert_eq!(rooms.as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_sensor_unit_detection() {
        let client = Arc::new(MockLoxoneClient::new());
        let executor = LoxoneBatchExecutor::new(client);

        assert_eq!(
            executor.get_sensor_unit("temperature"),
            Some("°C".to_string())
        );
        assert_eq!(executor.get_sensor_unit("humidity"), Some("%".to_string()));
        assert_eq!(executor.get_sensor_unit("unknown"), None);
    }
}
