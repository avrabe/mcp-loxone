//! Core types for the resource subscription system

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Information about a connected MCP client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Unique client identifier
    pub id: String,

    /// Transport protocol the client is using
    pub transport: ClientTransport,

    /// Client capabilities
    pub capabilities: Vec<String>,

    /// When the client connected
    pub connected_at: SystemTime,
}

/// Transport protocol types supported for notifications
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClientTransport {
    /// Standard input/output transport (Claude Desktop)
    Stdio,

    /// HTTP with Server-Sent Events (web clients, n8n)
    HttpSse {
        /// Client's connection ID for SSE
        connection_id: String,
    },

    /// WebSocket transport (future extension)
    WebSocket {
        /// WebSocket connection ID
        connection_id: String,
    },
}

/// Client subscription to a specific resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientSubscription {
    /// Client information
    pub client: ClientInfo,

    /// Resource URI being monitored
    pub resource_uri: String,

    /// Optional filter for change events
    pub filter: Option<SubscriptionFilter>,

    /// When this subscription was created
    pub subscribed_at: SystemTime,

    /// Last notification sent to this client
    pub last_notification: Option<SystemTime>,
}

/// Filter criteria for subscription notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionFilter {
    /// Only notify for specific change types
    pub change_types: Option<Vec<ResourceChangeType>>,

    /// Minimum time between notifications (debouncing)
    pub min_interval: Option<Duration>,

    /// Only notify for changes above this threshold
    pub change_threshold: Option<f64>,

    /// Custom filter expression (future extension)
    pub custom_expression: Option<String>,
}

/// Types of resource changes that can trigger notifications
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResourceChangeType {
    /// Device state changed (on/off, position, etc.)
    DeviceState,

    /// Sensor value updated
    SensorValue,

    /// Room configuration changed
    RoomConfig,

    /// System status updated
    SystemStatus,

    /// Audio zone state changed
    AudioZone,

    /// Weather data updated
    Weather,

    /// Security status changed
    Security,

    /// Energy consumption data updated
    Energy,

    /// Resource was added to the system
    ResourceAdded,

    /// Resource was removed from the system
    ResourceRemoved,
}

/// A detected change to a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceChange {
    /// URI of the changed resource
    pub resource_uri: String,

    /// Type of change that occurred
    pub change_type: ResourceChangeType,

    /// When the change was detected
    pub timestamp: SystemTime,

    /// Previous value (if available)
    pub previous_value: Option<serde_json::Value>,

    /// New value after the change
    pub new_value: serde_json::Value,

    /// UUID of the Loxone device/sensor that changed
    pub loxone_uuid: Option<String>,

    /// Additional metadata about the change
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Events within the subscription system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubscriptionEvent {
    /// A client subscribed to a resource
    ClientSubscribed {
        client_id: String,
        resource_uri: String,
    },

    /// A client unsubscribed from a resource
    ClientUnsubscribed {
        client_id: String,
        resource_uri: Option<String>,
    },

    /// A client disconnected
    ClientDisconnected { client_id: String, reason: String },

    /// A resource change was detected
    ResourceChanged { change: ResourceChange },

    /// A notification was sent to a client
    NotificationSent {
        client_id: String,
        resource_uri: String,
        success: bool,
    },

    /// A system error occurred
    SystemError { error: String, component: String },

    /// System is shutting down
    SystemShutdown,
}

/// Target for sending notifications
#[derive(Debug, Clone)]
pub enum NotificationTarget {
    /// Send to specific client
    Client(ClientInfo),

    /// Broadcast to all clients subscribed to a resource
    Resource(String),

    /// Send to all clients matching criteria
    Filtered {
        transport: Option<ClientTransport>,
        capabilities: Option<Vec<String>>,
    },
}

/// MCP notification message for resource changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceChangeNotification {
    /// MCP method name
    pub method: String,

    /// Notification parameters
    pub params: ResourceChangeParams,
}

/// Parameters for resource change notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceChangeParams {
    /// URI of the changed resource
    pub uri: String,

    /// Type of change
    #[serde(rename = "changeType")]
    pub change_type: ResourceChangeType,

    /// Timestamp of the change
    pub timestamp: String,

    /// Optional updated data
    pub data: Option<serde_json::Value>,

    /// Additional metadata
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl ResourceChangeNotification {
    /// Create a new resource change notification
    pub fn new(change: ResourceChange) -> Self {
        Self {
            method: "notifications/resources/updated".to_string(),
            params: ResourceChangeParams {
                uri: change.resource_uri,
                change_type: change.change_type,
                timestamp: format!("{:?}", change.timestamp),
                data: Some(change.new_value),
                metadata: if change.metadata.is_empty() {
                    None
                } else {
                    Some(change.metadata)
                },
            },
        }
    }
}

/// Statistics for subscription management
#[derive(Debug, Clone, Default)]
pub struct SubscriptionManagerStats {
    pub total_subscriptions: usize,
    pub active_clients: usize,
    pub monitored_resources: usize,
    pub subscriptions_by_transport: HashMap<String, usize>,
}

/// Statistics for change detection
#[derive(Debug, Clone, Default)]
pub struct ChangeDetectorStats {
    pub changes_detected: u64,
    pub websocket_events_processed: u64,
    pub mapping_cache_hits: u64,
    pub mapping_cache_misses: u64,
}

/// Statistics for notification dispatch
#[derive(Debug, Clone, Default)]
pub struct NotificationDispatcherStats {
    pub notifications_sent: u64,
    pub failed_notifications: u64,
    pub retry_attempts: u64,
    pub average_dispatch_time_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_info_creation() {
        let client = ClientInfo {
            id: "test-client".to_string(),
            transport: ClientTransport::Stdio,
            capabilities: vec!["resources".to_string()],
            connected_at: SystemTime::now(),
        };

        assert_eq!(client.id, "test-client");
        assert_eq!(client.transport, ClientTransport::Stdio);
    }

    #[test]
    fn test_resource_change_creation() {
        let change = ResourceChange {
            resource_uri: "loxone://devices/all".to_string(),
            change_type: ResourceChangeType::DeviceState,
            timestamp: SystemTime::now(),
            previous_value: Some(serde_json::json!({"state": "off"})),
            new_value: serde_json::json!({"state": "on"}),
            loxone_uuid: Some("123-456-789".to_string()),
            metadata: HashMap::new(),
        };

        assert_eq!(change.resource_uri, "loxone://devices/all");
        assert_eq!(change.change_type, ResourceChangeType::DeviceState);
    }

    #[test]
    fn test_notification_creation() {
        let change = ResourceChange {
            resource_uri: "loxone://rooms/Kitchen/devices".to_string(),
            change_type: ResourceChangeType::DeviceState,
            timestamp: SystemTime::now(),
            previous_value: None,
            new_value: serde_json::json!({"devices": []}),
            loxone_uuid: None,
            metadata: HashMap::new(),
        };

        let notification = ResourceChangeNotification::new(change);
        assert_eq!(notification.method, "notifications/resources/updated");
        assert_eq!(notification.params.uri, "loxone://rooms/Kitchen/devices");
    }
}
