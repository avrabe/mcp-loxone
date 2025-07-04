//! Loxone client implementations for HTTP and WebSocket communication

pub mod adaptive_pool;
#[cfg(feature = "crypto-openssl")]
pub mod auth;
pub mod client_factory;
pub mod command_queue;
pub mod connection_pool;
pub mod http_client;
pub mod load_balancer;
pub mod pool_health_monitor;
pub mod streaming_parser;
#[cfg(feature = "crypto-openssl")]
pub mod token_http_client;
#[cfg(feature = "websocket")]
pub mod websocket_client;
#[cfg(feature = "websocket")]
pub mod websocket_resilience;

pub use adaptive_pool::{
    AdaptiveConnectionGuard, AdaptiveConnectionPool, AdaptivePoolBuilder, PoolStatistics,
};
pub use client_factory::{
    AdaptiveClientFactory, ClientFactory, EncryptionLevel, ServerCapabilities, StaticClientFactory,
};
pub use http_client::LoxoneHttpClient;
pub use load_balancer::{
    LoadBalancer, LoadBalancingStatistics, LoadBalancingStrategy, WeightMethod,
};
pub use pool_health_monitor::{
    AlertThresholds, HealthAlert, HealthMetrics, HealthMonitorConfig, HealthStatus,
    PoolHealthMonitor,
};
#[cfg(feature = "crypto-openssl")]
pub use token_http_client::TokenHttpClient;
#[cfg(feature = "websocket")]
pub use websocket_client::LoxoneWebSocketClient;
#[cfg(feature = "websocket")]
pub use websocket_resilience::{
    MessagePriority, MessageType, ResilienceEvent, ResilienceStatistics, ResilientMessage,
    WebSocketResilienceConfig, WebSocketResilienceManager,
};

use crate::config::{credentials::LoxoneCredentials, LoxoneConfig};
use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Loxone device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneDevice {
    /// Device UUID
    pub uuid: String,
    /// Device name
    pub name: String,
    /// Device type (e.g., "LightController", "Jalousie")
    pub device_type: String,
    /// Room assignment
    pub room: Option<String>,
    /// Current states
    pub states: HashMap<String, serde_json::Value>,
    /// Category
    pub category: String,
    /// Sub-controls (for complex devices)
    pub sub_controls: HashMap<String, serde_json::Value>,
}

/// Loxone room information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneRoom {
    /// Room UUID
    pub uuid: String,
    /// Room name
    pub name: String,
    /// Device count in room
    pub device_count: usize,
}

/// Loxone structure file data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneStructure {
    /// Last modified timestamp
    #[serde(rename = "lastModified")]
    pub last_modified: String,
    /// All controls/devices
    pub controls: HashMap<String, serde_json::Value>,
    /// Room definitions
    pub rooms: HashMap<String, serde_json::Value>,
    /// Categories
    pub cats: HashMap<String, serde_json::Value>,
    /// Global states (optional, not present in all Loxone versions)
    #[serde(default)]
    pub global_states: HashMap<String, serde_json::Value>,
}

/// Command response from Loxone
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneResponse {
    /// Response code (200 = success)
    #[serde(rename = "LL")]
    pub code: i32,
    /// Response value
    pub value: serde_json::Value,
}

/// System capabilities detected from structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemCapabilities {
    pub has_lighting: bool,
    pub has_blinds: bool,
    pub has_weather: bool,
    pub has_security: bool,
    pub has_energy: bool,
    pub has_audio: bool,
    pub has_climate: bool,
    pub has_sensors: bool,

    // Detailed counts
    pub light_count: usize,
    pub blind_count: usize,
    pub sensor_count: usize,
    pub climate_count: usize,
}

/// Trait for Loxone client implementations
#[async_trait]
pub trait LoxoneClient: Send + Sync {
    /// Connect to the Loxone Miniserver
    async fn connect(&mut self) -> Result<()>;

    /// Check if client is connected
    async fn is_connected(&self) -> Result<bool>;

    /// Disconnect from the Miniserver
    async fn disconnect(&mut self) -> Result<()>;

    /// Send a command to a device
    async fn send_command(&self, uuid: &str, command: &str) -> Result<LoxoneResponse>;

    /// Get the structure file (LoxAPP3.json)
    async fn get_structure(&self) -> Result<LoxoneStructure>;

    /// Get current device states
    async fn get_device_states(
        &self,
        uuids: &[String],
    ) -> Result<HashMap<String, serde_json::Value>>;

    /// Get state values by state UUIDs (for resolving position values etc.)
    async fn get_state_values(
        &self,
        state_uuids: &[String],
    ) -> Result<HashMap<String, serde_json::Value>>;

    /// Get all device states in a single batch request (if supported)
    /// Falls back to individual requests if batch endpoint is not available
    async fn get_all_device_states_batch(&self) -> Result<HashMap<String, serde_json::Value>> {
        // Default implementation calls get_device_states with all known UUIDs
        // Implementations can override this for true batch support
        let devices = self.get_structure().await?.controls;
        let uuids: Vec<String> = devices.keys().cloned().collect();
        self.get_device_states(&uuids).await
    }

    /// Get system information
    async fn get_system_info(&self) -> Result<serde_json::Value>;

    /// Health check
    async fn health_check(&self) -> Result<bool>;

    /// Cast to Any for type checking
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Shared client context for caching and state management
#[derive(Debug, Clone)]
pub struct ClientContext {
    /// Cached structure data
    pub structure: Arc<RwLock<Option<LoxoneStructure>>>,

    /// Parsed devices
    pub devices: Arc<RwLock<HashMap<String, LoxoneDevice>>>,

    /// Parsed rooms
    pub rooms: Arc<RwLock<HashMap<String, LoxoneRoom>>>,

    /// System capabilities
    pub capabilities: Arc<RwLock<SystemCapabilities>>,

    /// Connection state
    pub connected: Arc<RwLock<bool>>,

    /// Last structure update
    pub last_update: Arc<RwLock<Option<chrono::DateTime<chrono::Utc>>>>,

    /// Sensor state logger (optional)
    pub sensor_logger: Arc<RwLock<Option<Arc<crate::tools::sensors::SensorStateLogger>>>>,
}

impl Default for ClientContext {
    fn default() -> Self {
        Self {
            structure: Arc::new(RwLock::new(None)),
            devices: Arc::new(RwLock::new(HashMap::new())),
            rooms: Arc::new(RwLock::new(HashMap::new())),
            capabilities: Arc::new(RwLock::new(SystemCapabilities::default())),
            connected: Arc::new(RwLock::new(false)),
            last_update: Arc::new(RwLock::new(None)),
            sensor_logger: Arc::new(RwLock::new(None)),
        }
    }
}

impl ClientContext {
    /// Create new client context
    pub fn new() -> Self {
        Self::default()
    }

    /// Update structure and parse devices/rooms
    pub async fn update_structure(&self, structure: LoxoneStructure) -> Result<()> {
        // Parse devices from structure
        let mut devices = HashMap::new();
        let mut rooms = HashMap::new();
        let mut capabilities = SystemCapabilities::default();

        // Parse rooms first
        for (uuid, room_data) in &structure.rooms {
            if let Ok(name) =
                serde_json::from_value::<String>(room_data.get("name").cloned().unwrap_or_default())
            {
                rooms.insert(
                    uuid.clone(),
                    LoxoneRoom {
                        uuid: uuid.clone(),
                        name,
                        device_count: 0, // Will be updated when parsing devices
                    },
                );
            }
        }

        // Parse devices from controls
        for (uuid, control_data) in &structure.controls {
            if let Some(control_obj) = control_data.as_object() {
                let name = control_obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let device_type = control_obj
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let room_uuid = control_obj
                    .get("room")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Get room name if room UUID is available
                let room_name = if let Some(ref room_uuid) = room_uuid {
                    rooms.get(room_uuid).map(|r| r.name.clone())
                } else {
                    None
                };

                // Parse states
                let states = control_obj
                    .get("states")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect::<HashMap<String, serde_json::Value>>()
                    })
                    .unwrap_or_default();

                // Parse sub-controls
                let sub_controls = control_obj
                    .get("subControls")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect::<HashMap<String, serde_json::Value>>()
                    })
                    .unwrap_or_default();

                // Determine category based on type
                let category = self.categorize_device(&device_type);

                // Update capabilities
                self.update_capabilities(&mut capabilities, &device_type, &category);

                // Update room device count
                if let Some(room_uuid) = &room_uuid {
                    if let Some(room) = rooms.get_mut(room_uuid) {
                        room.device_count += 1;
                    }
                }

                devices.insert(
                    uuid.clone(),
                    LoxoneDevice {
                        uuid: uuid.clone(),
                        name,
                        device_type,
                        room: room_name,
                        states,
                        category,
                        sub_controls,
                    },
                );
            }
        }

        // Update context
        *self.structure.write().await = Some(structure);
        *self.devices.write().await = devices;
        *self.rooms.write().await = rooms;
        *self.capabilities.write().await = capabilities;
        *self.last_update.write().await = Some(chrono::Utc::now());

        Ok(())
    }

    /// Categorize device based on type
    fn categorize_device(&self, device_type: &str) -> String {
        match device_type.to_lowercase().as_str() {
            t if t.contains("light") || t.contains("dimmer") => "lights".to_string(),
            t if t.contains("jalousie") || t.contains("blind") => "blinds".to_string(),
            t if t.contains("climate") || t.contains("heating") || t.contains("temperature") => {
                "climate".to_string()
            }
            t if t.contains("sensor") || t.contains("analog") => "sensors".to_string(),
            t if t.contains("weather") => "weather".to_string(),
            t if t.contains("security") || t.contains("alarm") => "security".to_string(),
            t if t.contains("energy") || t.contains("meter") => "energy".to_string(),
            t if t.contains("audio") || t.contains("music") => "audio".to_string(),
            _ => "other".to_string(),
        }
    }

    /// Update system capabilities based on device
    fn update_capabilities(
        &self,
        capabilities: &mut SystemCapabilities,
        _device_type: &str,
        category: &str,
    ) {
        match category {
            "lights" => {
                capabilities.has_lighting = true;
                capabilities.light_count += 1;
            }
            "blinds" => {
                capabilities.has_blinds = true;
                capabilities.blind_count += 1;
            }
            "climate" => {
                capabilities.has_climate = true;
                capabilities.climate_count += 1;
            }
            "sensors" => {
                capabilities.has_sensors = true;
                capabilities.sensor_count += 1;
            }
            "weather" => capabilities.has_weather = true,
            "security" => capabilities.has_security = true,
            "energy" => capabilities.has_energy = true,
            "audio" => capabilities.has_audio = true,
            _ => {}
        }
    }

    /// Get devices by category
    pub async fn get_devices_by_category(&self, category: &str) -> Result<Vec<LoxoneDevice>> {
        let devices = self.devices.read().await;
        Ok(devices
            .values()
            .filter(|device| device.category == category)
            .cloned()
            .collect())
    }

    /// Get devices by room
    pub async fn get_devices_by_room(&self, room_name: &str) -> Result<Vec<LoxoneDevice>> {
        let devices = self.devices.read().await;
        Ok(devices
            .values()
            .filter(|device| {
                device
                    .room
                    .as_ref()
                    .map(|r| r == room_name)
                    .unwrap_or(false)
            })
            .cloned()
            .collect())
    }

    /// Get device by name or UUID
    pub async fn get_device(&self, identifier: &str) -> Result<Option<LoxoneDevice>> {
        let devices = self.devices.read().await;

        // Try by UUID first
        if let Some(device) = devices.get(identifier) {
            return Ok(Some(device.clone()));
        }

        // Try by name
        for device in devices.values() {
            if device.name.to_lowercase() == identifier.to_lowercase() {
                return Ok(Some(device.clone()));
            }
        }

        Ok(None)
    }

    /// Check if structure needs refresh (older than cache TTL)
    pub async fn needs_refresh(&self, cache_ttl: std::time::Duration) -> bool {
        let last_update = self.last_update.read().await;
        match *last_update {
            Some(timestamp) => {
                let elapsed = chrono::Utc::now() - timestamp;
                elapsed.to_std().unwrap_or_default() > cache_ttl
            }
            None => true,
        }
    }

    /// Initialize sensor state logger
    pub async fn initialize_sensor_logger(&self, log_file: std::path::PathBuf) -> Result<()> {
        let logger = Arc::new(crate::tools::sensors::SensorStateLogger::new(log_file));

        // Load existing history from disk
        if let Err(e) = logger.load_from_disk().await {
            tracing::warn!("Failed to load sensor history from disk: {}", e);
        }

        // Start periodic sync task
        logger.start_periodic_sync();

        // Store in context
        *self.sensor_logger.write().await = Some(logger);

        tracing::info!("Sensor state logger initialized");
        Ok(())
    }

    /// Get sensor state logger
    pub async fn get_sensor_logger(&self) -> Option<Arc<crate::tools::sensors::SensorStateLogger>> {
        self.sensor_logger.read().await.clone()
    }

    /// Log sensor state change
    pub async fn log_sensor_state_change(
        &self,
        uuid: String,
        old_value: serde_json::Value,
        new_value: serde_json::Value,
        sensor_name: Option<String>,
        sensor_type: Option<crate::tools::sensors::SensorType>,
        room: Option<String>,
    ) {
        if let Some(logger) = self.get_sensor_logger().await {
            logger
                .log_state_change(uuid, old_value, new_value, sensor_name, sensor_type, room)
                .await;
        }
    }
}

/// Create appropriate client based on configuration
pub async fn create_client(
    config: &LoxoneConfig,
    credentials: &LoxoneCredentials,
) -> Result<Box<dyn LoxoneClient>> {
    use crate::config::AuthMethod;

    match config.auth_method {
        AuthMethod::Token => {
            #[cfg(feature = "crypto-openssl")]
            {
                use crate::client::token_http_client::TokenHttpClient;
                tracing::info!("🔐 Attempting token authentication...");
                match TokenHttpClient::new(config.clone(), credentials.clone()).await {
                    Ok(client) => {
                        tracing::info!("✅ Token authentication initialized successfully");
                        Ok(Box::new(client))
                    }
                    Err(e) => {
                        tracing::warn!("⚠️ Token authentication failed: {}", e);
                        tracing::info!("🔄 Falling back to basic authentication");
                        let client =
                            http_client::LoxoneHttpClient::new(config.clone(), credentials.clone())
                                .await?;
                        tracing::info!("✅ Basic authentication client created successfully");
                        Ok(Box::new(client))
                    }
                }
            }
            #[cfg(not(feature = "crypto-openssl"))]
            {
                tracing::warn!("Token authentication requested but crypto feature is disabled, falling back to basic auth");
                let client =
                    http_client::LoxoneHttpClient::new(config.clone(), credentials.clone()).await?;
                Ok(Box::new(client))
            }
        }
        AuthMethod::Basic => {
            let client =
                http_client::LoxoneHttpClient::new(config.clone(), credentials.clone()).await?;
            Ok(Box::new(client))
        }
        #[cfg(feature = "websocket")]
        AuthMethod::WebSocket => {
            tracing::info!("🔌 Initializing WebSocket client for real-time communication");
            match websocket_client::LoxoneWebSocketClient::new(config.clone(), credentials.clone())
                .await
            {
                Ok(client) => {
                    tracing::info!("✅ WebSocket client initialized successfully");
                    Ok(Box::new(client))
                }
                Err(e) => {
                    tracing::warn!("⚠️ WebSocket client initialization failed: {}", e);
                    tracing::info!("🔄 Falling back to HTTP client for compatibility");
                    let client =
                        http_client::LoxoneHttpClient::new(config.clone(), credentials.clone())
                            .await?;
                    Ok(Box::new(client))
                }
            }
        }
    }
}

/// Create hybrid client with WebSocket for real-time updates and HTTP for commands/structure
#[cfg(feature = "websocket")]
pub async fn create_hybrid_client(
    config: &LoxoneConfig,
    credentials: &LoxoneCredentials,
) -> Result<crate::client::websocket_client::LoxoneWebSocketClient> {
    use crate::client::websocket_client::LoxoneWebSocketClient;
    use std::sync::Arc;

    // We need to create the concrete HTTP client directly to avoid Box<dyn> issues
    let http_client: Arc<dyn LoxoneClient> = match config.auth_method {
        crate::config::AuthMethod::Token => {
            #[cfg(feature = "crypto-openssl")]
            {
                use crate::client::token_http_client::TokenHttpClient;
                Arc::new(TokenHttpClient::new(config.clone(), credentials.clone()).await?)
            }
            #[cfg(not(feature = "crypto-openssl"))]
            {
                tracing::warn!("Token authentication requested but crypto feature is disabled, falling back to basic auth");
                Arc::new(
                    http_client::LoxoneHttpClient::new(config.clone(), credentials.clone()).await?,
                )
            }
        }
        crate::config::AuthMethod::Basic => {
            Arc::new(http_client::LoxoneHttpClient::new(config.clone(), credentials.clone()).await?)
        }
        #[cfg(feature = "websocket")]
        crate::config::AuthMethod::WebSocket => {
            // For hybrid client, use basic HTTP for structure loading
            Arc::new(http_client::LoxoneHttpClient::new(config.clone(), credentials.clone()).await?)
        }
    };

    // Create WebSocket client with HTTP client for hybrid operation
    LoxoneWebSocketClient::new_with_http_client(config.clone(), credentials.clone(), http_client)
        .await
}

/// Create standalone WebSocket client (for real-time monitoring only)
#[cfg(feature = "websocket")]
pub async fn create_websocket_client(
    config: &LoxoneConfig,
    credentials: &LoxoneCredentials,
) -> Result<Box<dyn LoxoneClient>> {
    use crate::client::websocket_client::LoxoneWebSocketClient;
    let client = LoxoneWebSocketClient::new(config.clone(), credentials.clone()).await?;
    Ok(Box::new(client))
}
