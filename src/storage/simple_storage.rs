//! Simple in-memory weather data storage
//!
//! This provides a basic implementation for weather data storage without external database dependencies.
//! Data is stored in memory and will be lost when the application restarts.

use crate::client::LoxoneDevice;
use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Simple weather data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleWeatherDataPoint {
    pub device_uuid: String,
    pub parameter_name: String,
    pub value: f64,
    pub unit: Option<String>,
    pub timestamp: u32,
    pub quality_score: f64,
}

/// Simple weather storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleWeatherStorageConfig {
    /// Maximum number of data points to store per device
    pub max_points_per_device: usize,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
}

impl Default for SimpleWeatherStorageConfig {
    fn default() -> Self {
        Self {
            max_points_per_device: 1000,
            cache_ttl_seconds: 3600, // 1 hour
        }
    }
}

/// Cached UUID mapping entry
#[derive(Debug, Clone)]
struct CachedMapping {
    device_uuid: String,
    device_name: Option<String>,
    device_type: Option<String>,
    cached_at: DateTime<Utc>,
}

/// Simple in-memory weather data storage
pub struct SimpleWeatherStorage {
    config: SimpleWeatherStorageConfig,
    /// Weather data storage: device_uuid -> parameter_name -> Vec<data_points>
    data: Arc<RwLock<HashMap<String, HashMap<String, Vec<SimpleWeatherDataPoint>>>>>,
    /// Cache for UUID index to device UUID mapping
    uuid_cache: Arc<RwLock<HashMap<u32, CachedMapping>>>,
    /// Device structure cache for UUID resolution
    devices_cache: Arc<RwLock<HashMap<String, LoxoneDevice>>>,
}

impl SimpleWeatherStorage {
    /// Create new simple weather storage
    pub async fn new(config: SimpleWeatherStorageConfig) -> Result<Self> {
        info!("Initializing simple weather storage (in-memory)");

        Ok(Self {
            config,
            data: Arc::new(RwLock::new(HashMap::new())),
            uuid_cache: Arc::new(RwLock::new(HashMap::new())),
            devices_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Update device structure cache from Loxone structure
    pub async fn update_device_structure(&self, devices: &HashMap<String, LoxoneDevice>) {
        let mut cache = self.devices_cache.write().await;
        cache.clear();
        cache.extend(devices.iter().map(|(k, v)| (k.clone(), v.clone())));
        info!(
            "Updated device structure cache with {} devices",
            cache.len()
        );
    }

    /// Store weather data from WebSocket stream
    pub async fn store_weather_update(
        &self,
        uuid_index: u32,
        value: f64,
        timestamp: u32,
        parameter_name: Option<&str>,
        unit: Option<&str>,
        quality_score: Option<f64>,
    ) -> Result<()> {
        // Resolve device UUID from index
        let device_uuid = self.resolve_device_uuid(uuid_index).await?;

        // Determine parameter name if not provided
        let param_name = parameter_name.unwrap_or("weather_value");

        debug!(
            "Storing weather data: device={}, param={}, value={:.2}, timestamp={}",
            device_uuid, param_name, value, timestamp
        );

        // Create data point
        let data_point = SimpleWeatherDataPoint {
            device_uuid: device_uuid.clone(),
            parameter_name: param_name.to_string(),
            value,
            unit: unit.map(|s| s.to_string()),
            timestamp,
            quality_score: quality_score.unwrap_or(1.0),
        };

        // Store in memory
        let mut data = self.data.write().await;
        let device_data = data.entry(device_uuid).or_insert_with(HashMap::new);
        let param_data = device_data
            .entry(param_name.to_string())
            .or_insert_with(Vec::new);

        // Add new data point
        param_data.push(data_point);

        // Limit storage size
        if param_data.len() > self.config.max_points_per_device {
            param_data.remove(0); // Remove oldest
        }

        Ok(())
    }

    /// Resolve device UUID from UUID index using cache and fallback to structure
    async fn resolve_device_uuid(&self, uuid_index: u32) -> Result<String> {
        // Check cache first
        {
            let cache = self.uuid_cache.read().await;
            if let Some(cached) = cache.get(&uuid_index) {
                let age = Utc::now().timestamp() as u64 - cached.cached_at.timestamp() as u64;
                if age < self.config.cache_ttl_seconds {
                    debug!(
                        "UUID cache hit for index {}: {} ({:?})",
                        uuid_index,
                        cached.device_uuid,
                        cached.device_name.as_deref().unwrap_or("unknown")
                    );
                    return Ok(cached.device_uuid.clone());
                }
            }
        }

        // Try to resolve from device structure using index patterns
        let device_uuid = self
            .resolve_from_structure(uuid_index)
            .await
            .unwrap_or_else(|| format!("unknown_device_{}", uuid_index));

        // Update cache
        self.update_uuid_cache(uuid_index, &device_uuid).await;

        Ok(device_uuid)
    }

    /// Try to resolve UUID from device structure cache
    async fn resolve_from_structure(&self, uuid_index: u32) -> Option<String> {
        let devices = self.devices_cache.read().await;

        // Look for weather devices and try to match by index patterns
        for (uuid, device) in devices.iter() {
            if self.is_weather_device(device) {
                // Try various heuristics to match UUID index to device
                if let Some(device_index) = self.extract_device_index_from_uuid(uuid) {
                    if device_index == uuid_index {
                        debug!(
                            "Resolved UUID {} from structure for index {}",
                            uuid, uuid_index
                        );
                        return Some(uuid.clone());
                    }
                }
            }
        }

        None
    }

    /// Extract potential device index from UUID (Loxone-specific logic)
    fn extract_device_index_from_uuid(&self, uuid: &str) -> Option<u32> {
        // Loxone UUIDs often follow patterns like "0F1A2B3C-0001-4567-8901-234567890123"
        // The index might be embedded in specific parts of the UUID
        if let Some(parts) = uuid.split('-').nth(1) {
            if let Ok(index) = u32::from_str_radix(parts, 16) {
                return Some(index);
            }
        }

        // Alternative: hash-based mapping (fallback)
        None
    }

    /// Check if device is a weather device
    fn is_weather_device(&self, device: &LoxoneDevice) -> bool {
        let weather_types = ["WeatherStation", "Sensor", "TempSensor", "HumiditySensor"];
        let weather_keywords = ["weather", "temp", "humidity", "wind", "rain", "pressure"];

        // Check by type
        if weather_types
            .iter()
            .any(|&t| device.device_type.contains(t))
        {
            return true;
        }

        // Check by name keywords
        let device_name = device.name.to_lowercase();
        weather_keywords
            .iter()
            .any(|&keyword| device_name.contains(keyword))
    }

    /// Update UUID cache with new mapping
    async fn update_uuid_cache(&self, uuid_index: u32, device_uuid: &str) {
        let mut cache = self.uuid_cache.write().await;

        // Get device details from structure cache
        let (device_name, device_type) = {
            let devices = self.devices_cache.read().await;
            if let Some(device) = devices.get(device_uuid) {
                (Some(device.name.clone()), Some(device.device_type.clone()))
            } else {
                (None, None)
            }
        };

        // Add to cache
        cache.insert(
            uuid_index,
            CachedMapping {
                device_uuid: device_uuid.to_string(),
                device_name,
                device_type,
                cached_at: Utc::now(),
            },
        );

        debug!(
            "Updated UUID cache for index {} -> {}",
            uuid_index, device_uuid
        );
    }

    /// Get recent weather data for resources
    pub async fn get_current_weather_data(
        &self,
        device_uuid: &str,
    ) -> Result<Vec<SimpleWeatherDataPoint>> {
        let data = self.data.read().await;
        if let Some(device_data) = data.get(device_uuid) {
            let mut all_points = Vec::new();
            for param_data in device_data.values() {
                all_points.extend(param_data.iter().cloned());
            }
            // Sort by timestamp (newest first)
            all_points.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            Ok(all_points.into_iter().take(50).collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Get cached device information
    pub async fn get_device_info(
        &self,
        uuid_index: u32,
    ) -> Option<(String, Option<String>, Option<String>)> {
        let cache = self.uuid_cache.read().await;
        cache.get(&uuid_index).map(|c| {
            (
                c.device_uuid.clone(),
                c.device_name.clone(),
                c.device_type.clone(),
            )
        })
    }

    /// Get weather data for specific parameter
    pub async fn get_weather_parameter(
        &self,
        device_uuid: &str,
        parameter_name: &str,
        limit: usize,
    ) -> Result<Vec<SimpleWeatherDataPoint>> {
        let data = self.data.read().await;
        if let Some(device_data) = data.get(device_uuid) {
            if let Some(param_data) = device_data.get(parameter_name) {
                let mut points = param_data.clone();
                points.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                Ok(points.into_iter().take(limit).collect())
            } else {
                Ok(Vec::new())
            }
        } else {
            Ok(Vec::new())
        }
    }

    /// Get all weather devices with recent data
    pub async fn get_weather_devices_with_data(&self) -> Result<Vec<String>> {
        let data = self.data.read().await;
        Ok(data.keys().cloned().collect())
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<SimpleWeatherStorageStats> {
        let data = self.data.read().await;
        let total_points: usize = data
            .values()
            .map(|device_data| device_data.values().map(|v| v.len()).sum::<usize>())
            .sum();

        Ok(SimpleWeatherStorageStats {
            total_devices: data.len(),
            total_data_points: total_points,
            cached_uuid_mappings: self.uuid_cache.read().await.len(),
            cached_devices: self.devices_cache.read().await.len(),
            max_points_per_device: self.config.max_points_per_device,
        })
    }
}

/// Simple weather storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleWeatherStorageStats {
    pub total_devices: usize,
    pub total_data_points: usize,
    pub cached_uuid_mappings: usize,
    pub cached_devices: usize,
    pub max_points_per_device: usize,
}
