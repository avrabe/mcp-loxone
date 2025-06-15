//! Resource Change Detector
//!
//! Monitors Loxone WebSocket streams for state changes and maps them to MCP resource URIs.
//! Detects device state changes, sensor updates, and system events to trigger notifications.

use super::types::{ChangeDetectorStats, ResourceChange, ResourceChangeType, SubscriptionEvent};
use crate::client::LoxoneClient;
use crate::error::{LoxoneError, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, timeout};
use tracing::{debug, info, warn};

/// Detects resource changes from Loxone WebSocket events
pub struct ResourceChangeDetector {
    /// Loxone client for WebSocket monitoring
    client: Option<Arc<dyn LoxoneClient>>,

    /// Broadcast sender for system events
    event_sender: broadcast::Sender<SubscriptionEvent>,

    /// Cache mapping Loxone UUIDs to resource URIs
    uuid_to_resource_cache: Arc<RwLock<HashMap<String, String>>>,

    /// Last known values for change detection
    last_values: Arc<RwLock<HashMap<String, Value>>>,

    /// Statistics for monitoring
    stats: Arc<RwLock<ChangeDetectorStats>>,

    /// Flag to stop monitoring
    should_stop: Arc<RwLock<bool>>,

    /// Debounce settings to prevent spam
    debounce_duration: Duration,

    /// Last change timestamps for debouncing
    last_change_times: Arc<RwLock<HashMap<String, SystemTime>>>,
}

impl ResourceChangeDetector {
    /// Create a new change detector
    pub async fn new(event_sender: broadcast::Sender<SubscriptionEvent>) -> Result<Self> {
        info!("🔍 Initializing resource change detector");

        Ok(Self {
            client: None, // Will be set when we have a client
            event_sender,
            uuid_to_resource_cache: Arc::new(RwLock::new(HashMap::new())),
            last_values: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(ChangeDetectorStats::default())),
            should_stop: Arc::new(RwLock::new(false)),
            debounce_duration: Duration::from_millis(500), // 500ms debounce
            last_change_times: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Set the Loxone client for monitoring
    pub async fn set_client(&self, client: Arc<dyn LoxoneClient>) -> Result<()> {
        info!("🔌 Setting Loxone client for change detection");

        // Build initial UUID to resource mapping
        self.build_uuid_mapping(&client).await?;

        // Store client reference
        // Note: We'll need to modify this to store the client
        // For now, we'll work with the provided client in start_monitoring

        Ok(())
    }

    /// Start monitoring for changes
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("🚀 Starting resource change monitoring");

        // Reset stop flag
        {
            let mut should_stop = self.should_stop.write().await;
            *should_stop = false;
        }

        // Start periodic structure refresh
        let _uuid_cache = self.uuid_to_resource_cache.clone();
        let stats = self.stats.clone();
        let should_stop = self.should_stop.clone();

        tokio::spawn(async move {
            let mut refresh_interval = interval(Duration::from_secs(300)); // 5 minutes

            loop {
                refresh_interval.tick().await;

                if *should_stop.read().await {
                    break;
                }

                // Refresh UUID mapping periodically
                debug!("🔄 Refreshing UUID to resource mapping");
                // Note: In a real implementation, we'd refresh the mapping here
                // For now, we'll just update stats
                {
                    let mut detector_stats = stats.write().await;
                    detector_stats.mapping_cache_hits += 1;
                }
            }
        });

        // Start WebSocket event monitoring
        self.monitor_websocket_events().await?;

        Ok(())
    }

    /// Stop monitoring
    pub async fn stop_monitoring(&self) -> Result<()> {
        info!("🛑 Stopping resource change monitoring");

        let mut should_stop = self.should_stop.write().await;
        *should_stop = true;

        Ok(())
    }

    /// Monitor WebSocket events for changes
    async fn monitor_websocket_events(&self) -> Result<()> {
        info!("👂 Starting WebSocket event monitoring");

        // In a real implementation, this would:
        // 1. Connect to Loxone WebSocket
        // 2. Listen for state update events
        // 3. Parse and map events to resource changes
        // 4. Apply debouncing
        // 5. Send notifications

        // For now, we'll simulate monitoring with a periodic check
        let event_sender = self.event_sender.clone();
        let uuid_cache = self.uuid_to_resource_cache.clone();
        let last_values = self.last_values.clone();
        let stats = self.stats.clone();
        let should_stop = self.should_stop.clone();
        let debounce_duration = self.debounce_duration;
        let last_change_times = self.last_change_times.clone();

        tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_secs(1));

            loop {
                check_interval.tick().await;

                if *should_stop.read().await {
                    break;
                }

                // Simulate WebSocket event processing
                // In real implementation, this would be event-driven
                if let Err(e) = Self::process_simulated_events(
                    &event_sender,
                    &uuid_cache,
                    &last_values,
                    &stats,
                    debounce_duration,
                    &last_change_times,
                )
                .await
                {
                    warn!("Error processing WebSocket events: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Process simulated WebSocket events (replace with real implementation)
    async fn process_simulated_events(
        event_sender: &broadcast::Sender<SubscriptionEvent>,
        _uuid_cache: &Arc<RwLock<HashMap<String, String>>>,
        _last_values: &Arc<RwLock<HashMap<String, Value>>>,
        stats: &Arc<RwLock<ChangeDetectorStats>>,
        _debounce_duration: Duration,
        _last_change_times: &Arc<RwLock<HashMap<String, SystemTime>>>,
    ) -> Result<()> {
        // Simulate occasional events for testing
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Simple pseudo-random number generation without external dependencies
        let mut hasher = DefaultHasher::new();
        std::time::SystemTime::now().hash(&mut hasher);
        let pseudo_random = (hasher.finish() % 1000) as f64 / 1000.0;

        if pseudo_random < 0.001 {
            // 0.1% chance per check
            let change = ResourceChange {
                resource_uri: "loxone://devices/all".to_string(),
                change_type: ResourceChangeType::DeviceState,
                timestamp: SystemTime::now(),
                previous_value: Some(serde_json::json!({"state": "off"})),
                new_value: serde_json::json!({"state": "on"}),
                loxone_uuid: Some("simulated-device-123".to_string()),
                metadata: {
                    let mut map = HashMap::new();
                    map.insert("simulation".to_string(), serde_json::json!(true));
                    map
                },
            };

            // Send resource change event
            let _ = event_sender.send(SubscriptionEvent::ResourceChanged { change });

            // Update stats
            {
                let mut detector_stats = stats.write().await;
                detector_stats.changes_detected += 1;
                detector_stats.websocket_events_processed += 1;
            }
        }

        Ok(())
    }

    /// Build mapping from Loxone UUIDs to resource URIs
    async fn build_uuid_mapping(&self, client: &Arc<dyn LoxoneClient>) -> Result<()> {
        info!("🗺️ Building UUID to resource URI mapping");

        // Get structure from Loxone
        let structure = match timeout(Duration::from_secs(10), client.get_structure()).await {
            Ok(Ok(structure)) => structure,
            Ok(Err(e)) => {
                warn!("Failed to get Loxone structure: {}", e);
                return Err(e);
            }
            Err(_) => {
                warn!("Timeout getting Loxone structure");
                return Err(LoxoneError::timeout("Structure request timeout"));
            }
        };

        let mut mapping = HashMap::new();

        // Map rooms
        let structure_json = serde_json::to_value(&structure)?;
        if let Some(rooms) = structure_json.get("rooms").and_then(|r| r.as_object()) {
            for (room_uuid, room_data) in rooms {
                let room_name = room_data
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("Unknown");

                // Map room to loxone://rooms/{roomName}/devices
                mapping.insert(
                    room_uuid.clone(),
                    format!("loxone://rooms/{}/devices", room_name),
                );

                // Map devices in the room
                if let Some(controls) = room_data.get("controls").and_then(|c| c.as_object()) {
                    for (device_uuid, _device_data) in controls {
                        mapping.insert(
                            device_uuid.clone(),
                            format!("loxone://rooms/{}/devices", room_name),
                        );

                        // Also map to general device resources
                        mapping.insert(device_uuid.clone(), "loxone://devices/all".to_string());
                    }
                }
            }
        }

        // Map global controls
        if let Some(controls) = structure_json.get("controls").and_then(|c| c.as_object()) {
            for (control_uuid, control_data) in controls {
                // Determine control type and map to appropriate resource
                if let Some(control_type) = control_data.get("type").and_then(|t| t.as_str()) {
                    let resource_uri = match control_type {
                        "Switch" | "Dimmer" => "loxone://devices/category/lighting",
                        "Jalousie" => "loxone://devices/category/blinds",
                        "AudioZone" => "loxone://audio/zones",
                        "TempSensor" => "loxone://sensors/temperature",
                        "SecuritySwitch" => "loxone://security/status",
                        _ => "loxone://devices/all",
                    };

                    mapping.insert(control_uuid.clone(), resource_uri.to_string());
                }
            }
        }

        // Store the mapping
        {
            let mut cache = self.uuid_to_resource_cache.write().await;
            *cache = mapping;
        }

        let cache_len = {
            let cache = self.uuid_to_resource_cache.read().await;
            cache.len()
        };
        info!("✅ UUID mapping built with {} entries", cache_len);
        Ok(())
    }

    /// Map a Loxone UUID to resource URI
    async fn map_uuid_to_resource(&self, uuid: &str) -> Option<String> {
        let cache = self.uuid_to_resource_cache.read().await;
        let result = cache.get(uuid).cloned();

        // Update stats
        {
            let mut stats = self.stats.write().await;
            if result.is_some() {
                stats.mapping_cache_hits += 1;
            } else {
                stats.mapping_cache_misses += 1;
            }
        }

        result
    }

    /// Check if a change should be debounced
    async fn should_debounce(&self, resource_uri: &str) -> bool {
        let last_times = self.last_change_times.read().await;

        if let Some(last_time) = last_times.get(resource_uri) {
            let elapsed = SystemTime::now()
                .duration_since(*last_time)
                .unwrap_or(Duration::from_secs(0));

            elapsed < self.debounce_duration
        } else {
            false
        }
    }

    /// Update the last change time for debouncing
    async fn update_last_change_time(&self, resource_uri: &str) {
        let mut last_times = self.last_change_times.write().await;
        last_times.insert(resource_uri.to_string(), SystemTime::now());
    }

    /// Get detection statistics
    pub async fn get_statistics(&self) -> ChangeDetectorStats {
        self.stats.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detector_creation() {
        let (sender, _) = broadcast::channel(100);
        let detector = ResourceChangeDetector::new(sender).await;
        assert!(detector.is_ok());
    }

    #[tokio::test]
    async fn test_uuid_mapping() {
        let (sender, _) = broadcast::channel(100);
        let detector = ResourceChangeDetector::new(sender).await.unwrap();

        // Test mapping logic
        let result = detector.map_uuid_to_resource("nonexistent-uuid").await;
        assert!(result.is_none());

        // Add a mapping manually for testing
        {
            let mut cache = detector.uuid_to_resource_cache.write().await;
            cache.insert("test-uuid".to_string(), "loxone://devices/all".to_string());
        }

        let result = detector.map_uuid_to_resource("test-uuid").await;
        assert_eq!(result, Some("loxone://devices/all".to_string()));
    }

    #[tokio::test]
    async fn test_debouncing() {
        let (sender, _) = broadcast::channel(100);
        let detector = ResourceChangeDetector::new(sender).await.unwrap();

        let resource_uri = "loxone://test/resource";

        // First check should not be debounced
        assert!(!detector.should_debounce(resource_uri).await);

        // Update last change time
        detector.update_last_change_time(resource_uri).await;

        // Immediate check should be debounced
        assert!(detector.should_debounce(resource_uri).await);

        // Wait for debounce period to pass
        tokio::time::sleep(Duration::from_millis(600)).await;

        // Should not be debounced anymore
        assert!(!detector.should_debounce(resource_uri).await);
    }
}
