//! Example implementation of the unified history architecture
//! This shows how the new system would work in practice

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Example usage of the unified history system
#[cfg(test)]
mod example_usage {
    use super::*;

    #[tokio::test]
    async fn test_unified_history() {
        // Initialize the unified store
        let config = HistoryConfig::default();
        let store = UnifiedHistoryStore::new(config).await.unwrap();

        // Example 1: Record a device state change
        let device_event = HistoricalEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            category: EventCategory::DeviceState(DeviceStateChange {
                device_uuid: "uuid-123".to_string(),
                device_name: "Living Room Light".to_string(),
                room: Some("Living Room".to_string()),
                previous_state: json!({"on": false, "brightness": 0}),
                new_state: json!({"on": true, "brightness": 75}),
                triggered_by: "user_action".to_string(),
            }),
            source: EventSource::Device("uuid-123".to_string()),
            data: EventData::Generic(json!({"action": "turn_on"})),
            metadata: HashMap::new(),
        };

        store.record(device_event).await.unwrap();

        // Example 2: Record a sensor reading
        let sensor_event = HistoricalEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            category: EventCategory::SensorReading(SensorData {
                sensor_uuid: "sensor-456".to_string(),
                sensor_name: "Kitchen Temperature".to_string(),
                value: 22.5,
                unit: "°C".to_string(),
                sensor_type: "temperature".to_string(),
            }),
            source: EventSource::Sensor("sensor-456".to_string()),
            data: EventData::Generic(json!({"quality": "good"})),
            metadata: HashMap::new(),
        };

        store.record(sensor_event).await.unwrap();

        // Example 3: Query recent device states
        let recent_events = store
            .query()
            .category(EventCategory::DeviceState)
            .limit(10)
            .execute()
            .await
            .unwrap();

        println!("Recent device events: {}", recent_events.len());

        // Example 4: Query time range for sensors
        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now();
        
        let sensor_history = store
            .query()
            .time_range(start, end)
            .source_type(EventSource::Sensor)
            .execute()
            .await
            .unwrap();

        println!("Sensor readings in last hour: {}", sensor_history.len());

        // Example 5: Get aggregated data for dashboard
        let dashboard_data = store
            .dashboard_provider()
            .get_device_activity_summary()
            .await
            .unwrap();

        println!("Active devices: {}", dashboard_data.active_count);
        println!("Total state changes: {}", dashboard_data.total_changes);

        // Example 6: Subscribe to real-time updates
        let mut event_stream = store.subscribe().await;
        
        tokio::spawn(async move {
            while let Ok(event) = event_stream.recv().await {
                match event.category {
                    EventCategory::DeviceState(ref change) => {
                        println!("Device {} changed state", change.device_name);
                    }
                    EventCategory::SensorReading(ref data) => {
                        println!("Sensor {} = {} {}", 
                            data.sensor_name, data.value, data.unit);
                    }
                    _ => {}
                }
            }
        });
    }
}

/// Configuration for the history store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    /// Maximum events to keep in hot storage per source
    pub hot_storage_limit: usize,
    
    /// How long to keep sensor data at full resolution
    pub sensor_retention_minutes: u32,
    
    /// Directory for cold storage
    pub cold_storage_path: String,
    
    /// Automatic tiering interval
    pub tiering_interval_seconds: u64,
    
    /// Query cache settings
    pub query_cache_size_mb: usize,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            hot_storage_limit: 100,
            sensor_retention_minutes: 60,
            cold_storage_path: "/var/lib/loxone-mcp/history".to_string(),
            tiering_interval_seconds: 300, // 5 minutes
            query_cache_size_mb: 100,
        }
    }
}

/// Main unified history store
pub struct UnifiedHistoryStore {
    hot_store: Arc<RwLock<HotDataStore>>,
    cold_store: Arc<ColdDataStore>,
    config: HistoryConfig,
    event_bus: broadcast::Sender<HistoricalEvent>,
}

impl UnifiedHistoryStore {
    pub async fn new(config: HistoryConfig) -> Result<Self> {
        let (tx, _) = broadcast::channel(1000);
        
        Ok(Self {
            hot_store: Arc::new(RwLock::new(HotDataStore::new(&config))),
            cold_store: Arc::new(ColdDataStore::new(&config)?),
            config,
            event_bus: tx,
        })
    }

    /// Record a new event
    pub async fn record(&self, event: HistoricalEvent) -> Result<()> {
        // Write to hot storage
        self.hot_store.write().await.insert(event.clone())?;
        
        // Broadcast to subscribers
        let _ = self.event_bus.send(event.clone());
        
        // Check if tiering needed
        self.check_tiering().await?;
        
        Ok(())
    }

    /// Create a query builder
    pub fn query(&self) -> QueryBuilder {
        QueryBuilder::new(self.hot_store.clone(), self.cold_store.clone())
    }

    /// Get dashboard data provider
    pub fn dashboard_provider(&self) -> DashboardProvider {
        DashboardProvider::new(self.hot_store.clone())
    }

    /// Subscribe to real-time events
    pub async fn subscribe(&self) -> broadcast::Receiver<HistoricalEvent> {
        self.event_bus.subscribe()
    }

    /// Check if data needs to be moved to cold storage
    async fn check_tiering(&self) -> Result<()> {
        let hot_store = self.hot_store.read().await;
        
        // Check each category for overflow
        if hot_store.needs_tiering(&self.config) {
            drop(hot_store); // Release read lock
            
            // Perform tiering in background
            let hot_store = self.hot_store.clone();
            let cold_store = self.cold_store.clone();
            let config = self.config.clone();
            
            tokio::spawn(async move {
                if let Err(e) = perform_tiering(hot_store, cold_store, config).await {
                    tracing::error!("Tiering failed: {}", e);
                }
            });
        }
        
        Ok(())
    }
}

/// Hot data store (in-memory)
pub struct HotDataStore {
    device_states: HashMap<String, VecDeque<HistoricalEvent>>,
    sensor_readings: HashMap<String, VecDeque<HistoricalEvent>>,
    system_metrics: VecDeque<HistoricalEvent>,
    audit_events: VecDeque<HistoricalEvent>,
}

impl HotDataStore {
    fn new(config: &HistoryConfig) -> Self {
        Self {
            device_states: HashMap::new(),
            sensor_readings: HashMap::new(),
            system_metrics: VecDeque::with_capacity(1000),
            audit_events: VecDeque::with_capacity(1000),
        }
    }

    fn insert(&mut self, event: HistoricalEvent) -> Result<()> {
        match &event.category {
            EventCategory::DeviceState(state) => {
                let queue = self.device_states
                    .entry(state.device_uuid.clone())
                    .or_insert_with(|| VecDeque::with_capacity(100));
                
                // Maintain size limit
                if queue.len() >= 100 {
                    queue.pop_front();
                }
                queue.push_back(event);
            }
            EventCategory::SensorReading(data) => {
                let queue = self.sensor_readings
                    .entry(data.sensor_uuid.clone())
                    .or_insert_with(|| VecDeque::with_capacity(3600)); // 1 hour at 1Hz
                
                if queue.len() >= 3600 {
                    queue.pop_front();
                }
                queue.push_back(event);
            }
            EventCategory::SystemMetric(_) => {
                if self.system_metrics.len() >= 1000 {
                    self.system_metrics.pop_front();
                }
                self.system_metrics.push_back(event);
            }
            EventCategory::AuditEvent(_) => {
                if self.audit_events.len() >= 1000 {
                    self.audit_events.pop_front();
                }
                self.audit_events.push_back(event);
            }
        }
        Ok(())
    }

    fn needs_tiering(&self, config: &HistoryConfig) -> bool {
        // Check if any category is near capacity
        self.device_states.values().any(|q| q.len() > 90) ||
        self.sensor_readings.values().any(|q| q.len() > 3000) ||
        self.system_metrics.len() > 900 ||
        self.audit_events.len() > 900
    }
}

/// Cold data store (persistent)
pub struct ColdDataStore {
    data_dir: PathBuf,
    index: Arc<RwLock<DataIndex>>,
}

impl ColdDataStore {
    fn new(config: &HistoryConfig) -> Result<Self> {
        let data_dir = PathBuf::from(&config.cold_storage_path);
        std::fs::create_dir_all(&data_dir)?;
        
        Ok(Self {
            data_dir,
            index: Arc::new(RwLock::new(DataIndex::load_or_create(&data_dir)?)),
        })
    }
}

// Additional types for completeness
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoricalEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub category: EventCategory,
    pub source: EventSource,
    pub data: EventData,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EventCategory {
    DeviceState(DeviceStateChange),
    SensorReading(SensorData),
    SystemMetric(MetricData),
    AuditEvent(AuditData),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceStateChange {
    pub device_uuid: String,
    pub device_name: String,
    pub room: Option<String>,
    pub previous_state: serde_json::Value,
    pub new_state: serde_json::Value,
    pub triggered_by: String,
}

// ... Additional type definitions ...