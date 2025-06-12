//! Sensor configuration management
//!
//! Simplified sensor configuration without complex JSON mapping.
//! Uses direct device discovery and runtime configuration.

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Configured sensor entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfiguredSensor {
    /// Sensor UUID
    pub uuid: String,
    
    /// Human-readable name
    pub name: String,
    
    /// Sensor type classification
    pub sensor_type: String,
    
    /// Room assignment
    pub room: Option<String>,
    
    /// Description
    pub description: Option<String>,
    
    /// Whether sensor is enabled for monitoring
    pub enabled: bool,
    
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Sensor configuration manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorConfig {
    /// List of configured sensors
    pub sensors: Vec<ConfiguredSensor>,
    
    /// Configuration metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Default for SensorConfig {
    fn default() -> Self {
        Self {
            sensors: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

impl SensorConfig {
    /// Create new empty sensor configuration
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Load sensor configuration from file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| LoxoneError::config(format!("Failed to read sensor config: {}", e)))?;
        
        let config: SensorConfig = serde_json::from_str(&content)
            .map_err(|e| LoxoneError::config(format!("Failed to parse sensor config: {}", e)))?;
        
        Ok(config)
    }
    
    /// Save sensor configuration to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| LoxoneError::config(format!("Failed to serialize sensor config: {}", e)))?;
        
        fs::write(path, content)
            .map_err(|e| LoxoneError::config(format!("Failed to write sensor config: {}", e)))?;
        
        Ok(())
    }
    
    /// Add a new sensor to configuration
    pub fn add_sensor(&mut self, sensor: ConfiguredSensor) -> Result<()> {
        // Check for duplicate UUIDs
        if self.sensors.iter().any(|s| s.uuid == sensor.uuid) {
            return Err(LoxoneError::config(format!("Sensor {} already exists", sensor.uuid)));
        }
        
        self.sensors.push(sensor);
        Ok(())
    }
    
    /// Remove sensor by UUID
    pub fn remove_sensor(&mut self, uuid: &str) -> Result<bool> {
        let initial_len = self.sensors.len();
        self.sensors.retain(|s| s.uuid != uuid);
        Ok(self.sensors.len() < initial_len)
    }
    
    /// Get sensor by UUID
    pub fn get_sensor(&self, uuid: &str) -> Option<&ConfiguredSensor> {
        self.sensors.iter().find(|s| s.uuid == uuid)
    }
    
    /// Get mutable sensor by UUID
    pub fn get_sensor_mut(&mut self, uuid: &str) -> Option<&mut ConfiguredSensor> {
        self.sensors.iter_mut().find(|s| s.uuid == uuid)
    }
    
    /// List all enabled sensors
    pub fn enabled_sensors(&self) -> Vec<&ConfiguredSensor> {
        self.sensors.iter().filter(|s| s.enabled).collect()
    }
    
    /// List sensors by type
    pub fn sensors_by_type(&self, sensor_type: &str) -> Vec<&ConfiguredSensor> {
        self.sensors.iter()
            .filter(|s| s.sensor_type == sensor_type)
            .collect()
    }
    
    /// List sensors by room
    pub fn sensors_by_room(&self, room: &str) -> Vec<&ConfiguredSensor> {
        self.sensors.iter()
            .filter(|s| s.room.as_deref() == Some(room))
            .collect()
    }
    
    /// Enable/disable sensor
    pub fn set_sensor_enabled(&mut self, uuid: &str, enabled: bool) -> Result<()> {
        let sensor = self.get_sensor_mut(uuid)
            .ok_or_else(|| LoxoneError::not_found(format!("Sensor {} not found", uuid)))?;
        
        sensor.enabled = enabled;
        Ok(())
    }
    
    /// Update sensor metadata
    pub fn update_sensor_metadata(&mut self, uuid: &str, metadata: HashMap<String, serde_json::Value>) -> Result<()> {
        let sensor = self.get_sensor_mut(uuid)
            .ok_or_else(|| LoxoneError::not_found(format!("Sensor {} not found", uuid)))?;
        
        sensor.metadata = metadata;
        Ok(())
    }
    
    /// Get configuration statistics
    pub fn statistics(&self) -> SensorConfigStats {
        let total_sensors = self.sensors.len();
        let enabled_sensors = self.sensors.iter().filter(|s| s.enabled).count();
        
        let mut by_type = HashMap::new();
        let mut by_room = HashMap::new();
        
        for sensor in &self.sensors {
            *by_type.entry(sensor.sensor_type.clone()).or_insert(0) += 1;
            
            if let Some(ref room) = sensor.room {
                *by_room.entry(room.clone()).or_insert(0) += 1;
            }
        }
        
        SensorConfigStats {
            total_sensors,
            enabled_sensors,
            disabled_sensors: total_sensors - enabled_sensors,
            by_type,
            by_room,
            unique_types: by_type.len(),
            unique_rooms: by_room.len(),
        }
    }
}

/// Sensor configuration statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorConfigStats {
    pub total_sensors: usize,
    pub enabled_sensors: usize,
    pub disabled_sensors: usize,
    pub by_type: HashMap<String, usize>,
    pub by_room: HashMap<String, usize>,
    pub unique_types: usize,
    pub unique_rooms: usize,
}

/// Helper functions for sensor configuration
impl ConfiguredSensor {
    /// Create new configured sensor
    pub fn new(uuid: String, name: String, sensor_type: String) -> Self {
        Self {
            uuid,
            name,
            sensor_type,
            room: None,
            description: None,
            enabled: true,
            metadata: HashMap::new(),
        }
    }
    
    /// Create from device discovery
    pub fn from_device(device: &crate::client::LoxoneDevice, sensor_type: String) -> Self {
        Self {
            uuid: device.uuid.clone(),
            name: device.name.clone(),
            sensor_type,
            room: device.room.clone(),
            description: Some(format!("Auto-discovered {} sensor", device.device_type)),
            enabled: true,
            metadata: device.states.clone(),
        }
    }
    
    /// Set room assignment
    pub fn with_room(mut self, room: String) -> Self {
        self.room = Some(room);
        self
    }
    
    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
    
    /// Set enabled state
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    
    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Create default sensor configuration
pub fn create_default_config() -> SensorConfig {
    let mut config = SensorConfig::new();
    
    // Add some example sensors (would be discovered in real usage)
    config.metadata.insert(
        "version".to_string(), 
        serde_json::Value::String("1.0".to_string())
    );
    
    config.metadata.insert(
        "created".to_string(),
        serde_json::Value::String(chrono::Utc::now().to_rfc3339())
    );
    
    config
}

/// Auto-discover sensors from device list
pub fn discover_sensors_from_devices(devices: &[crate::client::LoxoneDevice]) -> Vec<ConfiguredSensor> {
    let mut sensors = Vec::new();
    
    for device in devices {
        // Only include devices that look like sensors
        if device.category == "sensors" || is_sensor_device(device) {
            let sensor_type = classify_device_as_sensor(device);
            let configured_sensor = ConfiguredSensor::from_device(device, sensor_type);
            sensors.push(configured_sensor);
        }
    }
    
    sensors
}

/// Check if device appears to be a sensor
fn is_sensor_device(device: &crate::client::LoxoneDevice) -> bool {
    let name_lower = device.name.to_lowercase();
    let type_lower = device.device_type.to_lowercase();
    
    // Check for sensor-like names or types
    name_lower.contains("sensor") ||
    name_lower.contains("detector") ||
    name_lower.contains("monitor") ||
    type_lower.contains("sensor") ||
    type_lower.contains("analog") ||
    
    // Specific sensor types
    name_lower.contains("temperature") ||
    name_lower.contains("humidity") ||
    name_lower.contains("motion") ||
    name_lower.contains("door") ||
    name_lower.contains("window")
}

/// Classify device as sensor type
fn classify_device_as_sensor(device: &crate::client::LoxoneDevice) -> String {
    let name_lower = device.name.to_lowercase();
    let type_lower = device.device_type.to_lowercase();
    
    if name_lower.contains("door") || name_lower.contains("window") {
        "door_window".to_string()
    } else if name_lower.contains("motion") || name_lower.contains("pir") {
        "motion".to_string()
    } else if name_lower.contains("temperature") || name_lower.contains("temp") {
        "temperature".to_string()
    } else if name_lower.contains("humidity") || name_lower.contains("humid") {
        "humidity".to_string()
    } else if name_lower.contains("light") || name_lower.contains("lux") {
        "light".to_string()
    } else if type_lower.contains("analog") {
        "analog".to_string()
    } else {
        "unknown".to_string()
    }
}