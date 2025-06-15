//! Adapter for existing sensor history implementation

use crate::error::Result;
use crate::history::events::{SensorData, SensorQuality};
use crate::history::{EventCategory, HistoricalEvent, UnifiedHistoryStore};
use crate::tools::sensors::StateChangeEvent;
use std::sync::Arc;
use uuid::Uuid;

/// Adapter that bridges old sensor history API to unified history
pub struct SensorHistoryAdapter {
    unified_store: Arc<UnifiedHistoryStore>,
    // Keep a reference to original implementation for gradual migration
    _original_enabled: bool,
}

impl SensorHistoryAdapter {
    /// Create new adapter
    pub fn new(unified_store: Arc<UnifiedHistoryStore>) -> Self {
        Self {
            unified_store,
            _original_enabled: false, // Disable original implementation
        }
    }

    /// Log a sensor state change (adapts old API)
    pub async fn log_state_change(&self, event: StateChangeEvent) -> Result<()> {
        // Convert old StateChangeEvent to new HistoricalEvent
        let quality = Some(SensorQuality::Good); // Default to good quality

        let sensor_type_str = match event.event_type.as_str() {
            "door_window" => "door_window",
            "motion" => "motion",
            "temperature" => "temperature",
            "analog" => "analog",
            "light" => "light",
            _ => "unknown",
        }
        .to_string();

        let value = match &event.new_value {
            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
            serde_json::Value::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            serde_json::Value::String(s) => {
                s.parse()
                    .unwrap_or(if s.to_lowercase() == "true" || s == "1" {
                        1.0
                    } else {
                        0.0
                    })
            }
            _ => 0.0,
        };

        let unified_event = HistoricalEvent {
            id: Uuid::new_v4(),
            timestamp: event.timestamp,
            category: EventCategory::SensorReading(SensorData {
                sensor_uuid: event.uuid.clone(),
                sensor_name: "Unknown".to_string(), // StateChangeEvent doesn't have name
                sensor_type: sensor_type_str,
                value,
                unit: match event.event_type.as_str() {
                    "temperature" => "°C".to_string(),
                    "light" => "lux".to_string(),
                    "door_window" | "motion" => "state".to_string(),
                    _ => "value".to_string(),
                },
                quality,
                room: None, // StateChangeEvent doesn't have room
            }),
            source: crate::history::EventSource::Sensor(event.uuid.clone()),
            data: crate::history::EventData::Generic(serde_json::json!({
                "legacy_event": true,
                "previous_state": event.old_value,
                "new_state": event.new_value,
                "human_readable": event.human_readable
            })),
            metadata: std::collections::HashMap::new(),
        };

        self.unified_store.record(unified_event).await
    }

    /// Get sensor state history (adapts old API)
    pub async fn get_state_history(
        &self,
        sensor_uuid: &str,
        limit: Option<usize>,
    ) -> Result<Vec<StateChangeEvent>> {
        let events = self
            .unified_store
            .query()
            .category("sensor_reading")
            .entity_id(sensor_uuid)
            .limit(limit.unwrap_or(100))
            .execute()
            .await?
            .events;

        // Convert back to old format
        let mut state_events = Vec::new();

        for event in events {
            if let EventCategory::SensorReading(ref data) = event.category {
                // Extract legacy data from metadata if available
                let (previous_state, new_state, human_readable) =
                    if let crate::history::EventData::Generic(ref generic) = event.data {
                        (
                            generic
                                .get("previous_state")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null),
                            generic
                                .get("new_state")
                                .cloned()
                                .unwrap_or(serde_json::json!(data.value)),
                            generic
                                .get("human_readable")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        )
                    } else {
                        (serde_json::Value::Null, serde_json::json!(data.value), None)
                    };

                state_events.push(StateChangeEvent {
                    uuid: data.sensor_uuid.clone(),
                    timestamp: event.timestamp,
                    old_value: previous_state,
                    new_value: new_state,
                    human_readable: human_readable.unwrap_or_else(|| "Unknown state".to_string()),
                    event_type: data.sensor_type.clone(),
                });
            }
        }

        Ok(state_events)
    }

    /// Get recent sensor changes across all sensors
    pub async fn get_recent_changes(&self, limit: Option<usize>) -> Result<Vec<StateChangeEvent>> {
        let events = self
            .unified_store
            .query()
            .category("sensor_reading")
            .limit(limit.unwrap_or(50))
            .execute()
            .await?
            .events;

        // Convert to old format (similar to get_state_history)
        let mut state_events = Vec::new();

        for event in events {
            if let EventCategory::SensorReading(ref data) = event.category {
                let (previous_state, new_state, human_readable) =
                    if let crate::history::EventData::Generic(ref generic) = event.data {
                        (
                            generic
                                .get("previous_state")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null),
                            generic
                                .get("new_state")
                                .cloned()
                                .unwrap_or(serde_json::json!(data.value)),
                            generic
                                .get("human_readable")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        )
                    } else {
                        (serde_json::Value::Null, serde_json::json!(data.value), None)
                    };

                state_events.push(StateChangeEvent {
                    uuid: data.sensor_uuid.clone(),
                    timestamp: event.timestamp,
                    old_value: previous_state,
                    new_value: new_state,
                    human_readable: human_readable.unwrap_or_else(|| "Unknown state".to_string()),
                    event_type: data.sensor_type.clone(),
                });
            }
        }

        Ok(state_events)
    }
}
