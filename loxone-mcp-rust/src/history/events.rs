//! Event types and data models for the unified history system

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A historical event in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalEvent {
    /// Unique event ID
    pub id: Uuid,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Event category
    pub category: EventCategory,

    /// Event source
    pub source: EventSource,

    /// Event-specific data
    pub data: EventData,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Categories of events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EventCategory {
    /// Device state change
    DeviceState(DeviceStateChange),

    /// Sensor reading
    SensorReading(SensorData),

    /// System metric
    SystemMetric(MetricData),

    /// Audit event
    AuditEvent(AuditData),

    /// Discovery event
    DiscoveryEvent(DiscoveryData),

    /// Response cache entry
    ResponseCache(ResponseCacheData),
}

/// Source of an event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "id")]
pub enum EventSource {
    /// Device-generated event
    Device(String),

    /// Sensor-generated event
    Sensor(String),

    /// System-generated event
    System,

    /// User action
    User(String),

    /// Automation/scene
    Automation(String),

    /// API request
    Api(String),
}

/// Generic event data wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EventData {
    /// Generic JSON data
    Generic(serde_json::Value),

    /// Structured command data
    Command(CommandData),

    /// Error data
    Error(ErrorData),
}

/// Device state change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStateChange {
    /// Device UUID
    pub device_uuid: String,

    /// Device name
    pub device_name: String,

    /// Device type
    pub device_type: String,

    /// Room assignment
    pub room: Option<String>,

    /// Previous state
    pub previous_state: serde_json::Value,

    /// New state
    pub new_state: serde_json::Value,

    /// What triggered the change
    pub triggered_by: String,

    /// Energy consumption if available
    pub energy_delta: Option<f64>,
}

/// Sensor data reading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    /// Sensor UUID
    pub sensor_uuid: String,

    /// Sensor name
    pub sensor_name: String,

    /// Sensor type
    pub sensor_type: String,

    /// Reading value
    pub value: f64,

    /// Unit of measurement
    pub unit: String,

    /// Quality indicator
    pub quality: Option<SensorQuality>,

    /// Room location
    pub room: Option<String>,
}

/// Sensor data quality
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SensorQuality {
    Good,
    Fair,
    Poor,
    Unknown,
}

/// System metric data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricData {
    /// Metric name
    pub metric_name: String,

    /// Metric value
    pub value: f64,

    /// Unit
    pub unit: String,

    /// Tags for categorization
    pub tags: HashMap<String, String>,
}

/// Audit event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditData {
    /// Action performed
    pub action: String,

    /// Actor (user/system)
    pub actor: String,

    /// Target of the action
    pub target: Option<String>,

    /// Result of the action
    pub result: AuditResult,

    /// Additional details
    pub details: serde_json::Value,
}

/// Audit action result
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditResult {
    Success,
    Failure,
    Partial,
}

/// Discovery event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryData {
    /// Discovered entity type
    pub entity_type: String,

    /// Entity ID
    pub entity_id: String,

    /// Discovery method
    pub method: String,

    /// Entity details
    pub details: serde_json::Value,

    /// Is this a new discovery
    pub is_new: bool,
}

/// Response cache data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseCacheData {
    /// Tool/resource name
    pub key: String,

    /// Cached response
    pub response: serde_json::Value,

    /// Cache hit/miss
    pub cache_hit: bool,

    /// TTL in seconds
    pub ttl: u64,
}

/// Command data for structured commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandData {
    /// Command type
    pub command: String,

    /// Command parameters
    pub parameters: serde_json::Value,

    /// Execution result
    pub result: Option<serde_json::Value>,
}

/// Error data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorData {
    /// Error code
    pub code: String,

    /// Error message
    pub message: String,

    /// Stack trace if available
    pub stack_trace: Option<String>,

    /// Context
    pub context: serde_json::Value,
}

impl HistoricalEvent {
    /// Create a new device state event
    pub fn device_state(change: DeviceStateChange) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            category: EventCategory::DeviceState(change.clone()),
            source: EventSource::Device(change.device_uuid.clone()),
            data: EventData::Generic(serde_json::json!({
                "action": "state_change"
            })),
            metadata: HashMap::new(),
        }
    }

    /// Create a new sensor reading event
    pub fn sensor_reading(data: SensorData) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            category: EventCategory::SensorReading(data.clone()),
            source: EventSource::Sensor(data.sensor_uuid.clone()),
            data: EventData::Generic(serde_json::json!({
                "reading_type": "periodic"
            })),
            metadata: HashMap::new(),
        }
    }

    /// Create a new system metric event
    pub fn system_metric(metric: MetricData) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            category: EventCategory::SystemMetric(metric),
            source: EventSource::System,
            data: EventData::Generic(serde_json::json!({
                "collector": "system"
            })),
            metadata: HashMap::new(),
        }
    }

    /// Create a new audit event
    pub fn audit(audit: AuditData, actor_id: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            category: EventCategory::AuditEvent(audit),
            source: EventSource::User(actor_id),
            data: EventData::Generic(serde_json::json!({
                "audit_version": 1
            })),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the event
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}
