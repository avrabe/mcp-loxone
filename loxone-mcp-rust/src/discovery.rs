#[cfg(feature = "websocket")]
use futures_util::{SinkExt, StreamExt};
#[cfg(feature = "websocket")]
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use url::Url;

use crate::client::{http_client::LoxoneHttpClient, LoxoneClient};

pub struct DeviceDiscovery {
    client: LoxoneHttpClient,
}

impl DeviceDiscovery {
    pub fn new(client: LoxoneHttpClient) -> Self {
        Self { client }
    }

    pub async fn discover_all_devices(&self) -> Result<Vec<Value>> {
        let structure = self.client.get_structure().await?;
        let mut devices = Vec::new();

        for (uuid, control) in &structure.controls {
            let device = json!({
                "uuid": uuid,
                "name": control.get("name").unwrap_or(&Value::String("Unknown".to_string())),
                "type": control.get("type").unwrap_or(&Value::String("Unknown".to_string())),
                "room": control.get("room").unwrap_or(&Value::String("".to_string())),
                "category": control.get("cat").unwrap_or(&Value::String("".to_string()))
            });
            devices.push(device);
        }

        Ok(devices)
    }

    pub async fn discover_sensors(&self) -> Result<Vec<Value>> {
        let devices = self.discover_all_devices().await?;

        let sensor_types = [
            "TemperatureSensor",
            "HumiditySensor",
            "MotionSensor",
            "DoorSensor",
            "WindowSensor",
            "SmokeSensor",
            "WaterSensor",
        ];

        let sensors: Vec<Value> = devices
            .into_iter()
            .filter(|device| {
                if let Some(device_type) = device.get("type").and_then(|t| t.as_str()) {
                    sensor_types.contains(&device_type)
                } else {
                    false
                }
            })
            .collect();

        Ok(sensors)
    }

    pub async fn test_device_connectivity(&self, uuid: &str) -> Result<bool> {
        match self.client.get_device_states(&[uuid.to_string()]).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub async fn discover_working_sensors(&self) -> Result<Vec<Value>> {
        let sensors = self.discover_sensors().await?;
        let mut working_sensors = Vec::new();

        for sensor in sensors {
            if let Some(uuid) = sensor.get("uuid").and_then(|u| u.as_str()) {
                if self.test_device_connectivity(uuid).await.unwrap_or(false) {
                    working_sensors.push(sensor);
                }
            }
        }

        Ok(working_sensors)
    }

    #[cfg(feature = "websocket")]
    pub async fn discover_via_websocket(&self, base_url: &str) -> Result<Vec<Value>> {
        let ws_url = format!("ws://{}/ws/rfc6455", base_url.trim_start_matches("http://"));
        let url = Url::parse(&ws_url)?;

        let (ws_stream, _) = connect_async(url).await?;
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Send discovery request
        let discovery_msg = json!({
            "LL": {
                "control": "jdev/sps/LoxAPPversion3",
                "value": "discover",
                "Code": "200"
            }
        });

        ws_sender
            .send(Message::Text(discovery_msg.to_string()))
            .await?;

        // Collect responses for a short time
        let mut devices = Vec::new();
        let mut count = 0;

        while let Some(msg) = ws_receiver.next().await {
            if count > 10 {
                break;
            } // Limit discovery time
            count += 1;

            if let Ok(Message::Text(text)) = msg {
                if let Ok(data) = serde_json::from_str::<Value>(&text) {
                    if let Some(controls) = data.get("LL").and_then(|ll| ll.get("controls")) {
                        if let Some(controls_obj) = controls.as_object() {
                            for (uuid, control) in controls_obj {
                                let device = json!({
                                    "uuid": uuid,
                                    "name": control.get("name"),
                                    "type": control.get("type"),
                                    "discovered_via": "websocket"
                                });
                                devices.push(device);
                            }
                        }
                    }
                }
            }
        }

        Ok(devices)
    }

    #[cfg(not(feature = "websocket"))]
    pub async fn discover_via_websocket(&self, _base_url: &str) -> Result<Vec<Value>> {
        Err(anyhow::anyhow!("WebSocket feature not enabled"))
    }

    pub async fn get_device_details(&self, uuid: &str) -> Result<Value> {
        let states = self.client.get_device_states(&[uuid.to_string()]).await?;
        let state = states.get(uuid).cloned().unwrap_or_default();
        let structure = self.client.get_structure().await?;

        let mut details = json!({
            "uuid": uuid,
            "state": state
        });

        if let Some(control) = structure.controls.get(uuid) {
            details["info"] = control.clone();
        }

        Ok(details)
    }

    pub async fn categorize_devices(&self) -> Result<HashMap<String, Vec<Value>>> {
        let devices = self.discover_all_devices().await?;
        let mut categorized = HashMap::new();

        for device in devices {
            let category = device
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("Unknown")
                .to_string();

            categorized
                .entry(category)
                .or_insert_with(Vec::new)
                .push(device);
        }

        Ok(categorized)
    }
}
