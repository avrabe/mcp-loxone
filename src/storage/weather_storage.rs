//! Weather data storage layer
//!
//! Provides high-level interface for storing and retrieving weather data from WebSocket streams,
//! with automatic UUID resolution and caching for optimal performance.

use super::turso_client::{TursoClient, TursoConfig, WeatherAggregation, WeatherDataPoint};
use crate::client::LoxoneDevice;
use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Weather storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherStorageConfig {
    /// Turso database configuration
    pub turso: TursoConfig,
    /// Maximum cache size for UUID mappings
    pub cache_size: usize,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Data retention in days
    pub retention_days: u32,
    /// Enable automatic cleanup
    pub auto_cleanup: bool,
    /// Cleanup interval in hours
    pub cleanup_interval_hours: u32,
}

impl Default for WeatherStorageConfig {
    fn default() -> Self {
        Self {
            turso: TursoConfig::default(),
            cache_size: 1000,
            cache_ttl_seconds: 3600, // 1 hour
            retention_days: 90,      // 3 months
            auto_cleanup: true,
            cleanup_interval_hours: 24, // Daily cleanup
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

/// Weather data storage with caching and automatic UUID resolution
pub struct WeatherStorage {
    client: Arc<TursoClient>,
    config: WeatherStorageConfig,
    /// Cache for UUID index to device UUID mapping
    uuid_cache: Arc<RwLock<HashMap<u32, CachedMapping>>>,
    /// Device structure cache for UUID resolution
    devices_cache: Arc<RwLock<HashMap<String, LoxoneDevice>>>,
}

impl WeatherStorage {
    /// Create new weather storage with configuration
    pub async fn new(config: WeatherStorageConfig) -> Result<Self> {
        info!("Initializing weather storage");

        let client = Arc::new(TursoClient::new(config.turso.clone()).await?);

        let storage = Self {
            client,
            config,
            uuid_cache: Arc::new(RwLock::new(HashMap::new())),
            devices_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        // Start background cleanup if enabled
        if storage.config.auto_cleanup {
            storage.start_cleanup_task().await;
        }

        info!("Weather storage initialized successfully");
        Ok(storage)
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

        // Store in database
        self.client
            .store_weather_data(
                &device_uuid,
                uuid_index,
                param_name,
                value,
                unit,
                timestamp,
                quality_score,
            )
            .await?;

        Ok(())
    }

    /// Resolve device UUID from UUID index using cache and fallback to database
    async fn resolve_device_uuid(&self, uuid_index: u32) -> Result<String> {
        // Check cache first
        {
            let cache = self.uuid_cache.read().await;
            if let Some(cached) = cache.get(&uuid_index) {
                let age = Utc::now().timestamp() as u64 - cached.cached_at.timestamp() as u64;
                if age < self.config.cache_ttl_seconds {
                    debug!(
                        "UUID cache hit for index {}: {} ({:?} - {:?})",
                        uuid_index,
                        cached.device_uuid,
                        cached.device_name.as_deref().unwrap_or("unknown"),
                        cached.device_type.as_deref().unwrap_or("unknown")
                    );
                    return Ok(cached.device_uuid.clone());
                }
            }
        }

        // Try to resolve from device structure using index patterns
        let device_uuid = if let Some(uuid) = self.resolve_from_structure(uuid_index).await {
            uuid
        } else if let Ok(Some(uuid)) = self.resolve_from_database(uuid_index).await {
            uuid
        } else {
            format!("unknown_device_{uuid_index}")
        };

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
                // This is implementation-specific to Loxone's UUID index system
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

    /// Try to resolve UUID from database
    async fn resolve_from_database(&self, uuid_index: u32) -> Result<Option<String>> {
        self.client.get_device_uuid(uuid_index).await
    }

    /// Update UUID cache with new mapping
    async fn update_uuid_cache(&self, uuid_index: u32, device_uuid: &str) {
        let mut cache = self.uuid_cache.write().await;

        // Remove old entries if cache is full
        if cache.len() >= self.config.cache_size {
            // Remove oldest entries (simple FIFO)
            let oldest_key = cache
                .iter()
                .min_by_key(|(_, v)| v.cached_at)
                .map(|(k, _)| *k);

            if let Some(key) = oldest_key {
                cache.remove(&key);
            }
        }

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
                device_name: device_name.clone(),
                device_type: device_type.clone(),
                cached_at: Utc::now(),
            },
        );

        // Also store in database for persistence
        if let Err(e) = self
            .client
            .store_device_mapping(
                uuid_index,
                device_uuid,
                device_name.as_deref(),
                device_type.as_deref(),
            )
            .await
        {
            warn!("Failed to store device mapping in database: {}", e);
        }

        debug!(
            "Updated UUID cache for index {} -> {}",
            uuid_index, device_uuid
        );
    }

    /// Get recent weather data for resources
    pub async fn get_current_weather_data(
        &self,
        device_uuid: &str,
    ) -> Result<Vec<WeatherDataPoint>> {
        self.client
            .get_recent_weather_data(device_uuid, None, 50)
            .await
    }

    /// Get weather data for specific parameter
    pub async fn get_weather_parameter(
        &self,
        device_uuid: &str,
        parameter_name: &str,
        limit: usize,
    ) -> Result<Vec<WeatherDataPoint>> {
        self.client
            .get_recent_weather_data(device_uuid, Some(parameter_name), limit)
            .await
    }

    /// Get aggregated weather data for time range
    pub async fn get_weather_aggregation(
        &self,
        device_uuid: &str,
        parameter_name: &str,
        start_time: u32,
        end_time: u32,
    ) -> Result<Vec<WeatherAggregation>> {
        self.client
            .get_aggregated_weather_data(device_uuid, parameter_name, start_time, end_time)
            .await
    }

    /// Get all weather devices with recent data
    pub async fn get_weather_devices_with_data(&self) -> Result<Vec<String>> {
        // This would require a more complex query to get unique device UUIDs
        // For now, return devices from structure cache that are weather devices
        let devices = self.devices_cache.read().await;
        let weather_device_uuids: Vec<String> = devices
            .iter()
            .filter(|(_, device)| self.is_weather_device(device))
            .map(|(uuid, _)| uuid.clone())
            .collect();

        Ok(weather_device_uuids)
    }

    /// Start background cleanup task
    async fn start_cleanup_task(&self) {
        let client = self.client.clone();
        let retention_days = self.config.retention_days;
        let interval_hours = self.config.cleanup_interval_hours;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(
                interval_hours as u64 * 3600,
            ));

            loop {
                interval.tick().await;

                match client.cleanup_old_data(retention_days).await {
                    Ok(cleaned_count) => {
                        if cleaned_count > 0 {
                            info!("Cleaned up {} old weather records", cleaned_count);
                        }
                    }
                    Err(e) => {
                        error!("Failed to cleanup old weather data: {}", e);
                    }
                }

                // Trigger sync if enabled
                if let Err(e) = client.sync().await {
                    warn!("Database sync failed: {}", e);
                }
            }
        });

        debug!("Started background cleanup task");
    }

    /// Force cleanup of old data
    pub async fn cleanup_now(&self) -> Result<u64> {
        self.client
            .cleanup_old_data(self.config.retention_days)
            .await
    }

    /// Force database sync
    pub async fn sync_now(&self) -> Result<()> {
        self.client.sync().await
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<WeatherStorageStats> {
        // This would require additional queries to get comprehensive statistics
        // For now, return basic information
        Ok(WeatherStorageStats {
            cached_uuid_mappings: self.uuid_cache.read().await.len(),
            cached_devices: self.devices_cache.read().await.len(),
            cache_size_limit: self.config.cache_size,
            retention_days: self.config.retention_days,
            auto_cleanup_enabled: self.config.auto_cleanup,
        })
    }
}

/// Weather storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherStorageStats {
    pub cached_uuid_mappings: usize,
    pub cached_devices: usize,
    pub cache_size_limit: usize,
    pub retention_days: u32,
    pub auto_cleanup_enabled: bool,
}
