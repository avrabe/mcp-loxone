//! Loxone client implementations for HTTP and WebSocket communication

pub mod http_client;
#[cfg(feature = "websocket")]
pub mod websocket_client;
#[cfg(feature = "crypto")]
pub mod auth;

use crate::error::Result;
use crate::config::{credentials::LoxoneCredentials, LoxoneConfig};
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
    async fn get_device_states(&self, uuids: &[String]) -> Result<HashMap<String, serde_json::Value>>;
    
    /// Get system information
    async fn get_system_info(&self) -> Result<serde_json::Value>;
    
    /// Health check
    async fn health_check(&self) -> Result<bool>;
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
            if let Ok(name) = serde_json::from_value::<String>(
                room_data.get("name").cloned().unwrap_or_default()
            ) {
                rooms.insert(uuid.clone(), LoxoneRoom {
                    uuid: uuid.clone(),
                    name,
                    device_count: 0, // Will be updated when parsing devices
                });
            }
        }
        
        // Parse devices from controls
        for (uuid, control_data) in &structure.controls {
            if let Some(control_obj) = control_data.as_object() {
                let name = control_obj.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();
                
                let device_type = control_obj.get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();
                
                let room_uuid = control_obj.get("room")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                
                // Get room name if room UUID is available
                let room_name = if let Some(ref room_uuid) = room_uuid {
                    rooms.get(room_uuid).map(|r| r.name.clone())
                } else {
                    None
                };
                
                // Parse states
                let states = control_obj.get("states")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect::<HashMap<String, serde_json::Value>>()
                    })
                    .unwrap_or_default();
                
                // Parse sub-controls
                let sub_controls = control_obj.get("subControls")
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
                
                devices.insert(uuid.clone(), LoxoneDevice {
                    uuid: uuid.clone(),
                    name,
                    device_type,
                    room: room_name,
                    states,
                    category,
                    sub_controls,
                });
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
            t if t.contains("light") || t.contains("dimmer") => "lighting".to_string(),
            t if t.contains("jalousie") || t.contains("blind") => "blinds".to_string(),
            t if t.contains("climate") || t.contains("heating") || t.contains("temperature") => "climate".to_string(),
            t if t.contains("sensor") || t.contains("analog") => "sensors".to_string(),
            t if t.contains("weather") => "weather".to_string(),
            t if t.contains("security") || t.contains("alarm") => "security".to_string(),
            t if t.contains("energy") || t.contains("meter") => "energy".to_string(),
            t if t.contains("audio") || t.contains("music") => "audio".to_string(),
            _ => "other".to_string(),
        }
    }
    
    /// Update system capabilities based on device
    fn update_capabilities(&self, capabilities: &mut SystemCapabilities, _device_type: &str, category: &str) {
        match category {
            "lighting" => {
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
                device.room.as_ref().map(|r| r == room_name).unwrap_or(false)
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
}

/// Create appropriate client based on configuration
pub async fn create_client(
    config: &LoxoneConfig,
    credentials: &LoxoneCredentials,
) -> Result<Box<dyn LoxoneClient>> {
    // For now, always use HTTP client
    // WebSocket client can be added later for real-time features
    let client = http_client::LoxoneHttpClient::new(config.clone(), credentials.clone()).await?;
    Ok(Box::new(client))
}