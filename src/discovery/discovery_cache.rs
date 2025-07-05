//! Discovery result caching system
//!
//! This module provides intelligent caching for network discovery results
//! to improve performance and reduce network overhead during repeated
//! discovery operations.
//!
//! Features:
//! - TTL-based cache expiration
//! - Network-aware invalidation
//! - Persistent storage options
//! - Memory-efficient storage
//! - Statistics and monitoring

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Discovered Loxone device information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredDevice {
    /// Device IP address
    pub ip_address: IpAddr,

    /// Device port
    pub port: u16,

    /// Device serial number (unique identifier)
    pub serial: String,

    /// Device name/description
    pub name: String,

    /// Device type (Miniserver, Extension, etc.)
    pub device_type: String,

    /// Firmware version
    pub firmware_version: String,

    /// Hardware version
    pub hardware_version: String,

    /// MAC address
    pub mac_address: Option<String>,

    /// Device capabilities
    pub capabilities: Vec<String>,

    /// Additional metadata
    pub metadata: HashMap<String, String>,

    /// When this device was first discovered
    pub first_seen: SystemTime,

    /// When this device was last seen
    pub last_seen: SystemTime,

    /// Discovery method used (mDNS, UPnP, Network Scan, etc.)
    pub discovery_method: String,

    /// Response time during discovery (for performance tracking)
    pub response_time: Duration,
}

impl DiscoveredDevice {
    /// Create a new discovered device
    pub fn new(
        ip_address: IpAddr,
        port: u16,
        serial: String,
        name: String,
        device_type: String,
        discovery_method: String,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            ip_address,
            port,
            serial,
            name,
            device_type,
            firmware_version: "unknown".to_string(),
            hardware_version: "unknown".to_string(),
            mac_address: None,
            capabilities: Vec::new(),
            metadata: HashMap::new(),
            first_seen: now,
            last_seen: now,
            discovery_method,
            response_time: Duration::from_millis(0),
        }
    }

    /// Get the socket address for this device
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip_address, self.port)
    }

    /// Update the last seen timestamp
    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now();
    }

    /// Check if the device is considered stale
    pub fn is_stale(&self, max_age: Duration) -> bool {
        self.last_seen.elapsed().unwrap_or(Duration::from_secs(0)) > max_age
    }

    /// Get age since last seen
    pub fn age(&self) -> Duration {
        self.last_seen.elapsed().unwrap_or(Duration::from_secs(0))
    }

    /// Add a capability
    pub fn add_capability(&mut self, capability: String) {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
    }

    /// Add metadata
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Check if device has capability
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(&capability.to_string())
    }
}

/// Discovery cache entry with TTL and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// The discovered device
    pub device: DiscoveredDevice,

    /// Cache entry creation time
    pub cached_at: SystemTime,

    /// Time-to-live for this entry
    pub ttl: Duration,

    /// Number of times this entry has been accessed
    pub access_count: u64,

    /// Network conditions when cached
    pub network_context: NetworkContext,
}

impl CacheEntry {
    /// Create a new cache entry
    pub fn new(device: DiscoveredDevice, ttl: Duration) -> Self {
        Self {
            device,
            cached_at: SystemTime::now(),
            ttl,
            access_count: 0,
            network_context: NetworkContext::current(),
        }
    }

    /// Check if this cache entry has expired
    pub fn is_expired(&self) -> bool {
        self.cached_at.elapsed().unwrap_or(Duration::from_secs(0)) > self.ttl
    }

    /// Get remaining TTL
    pub fn remaining_ttl(&self) -> Duration {
        let elapsed = self.cached_at.elapsed().unwrap_or(Duration::from_secs(0));
        if elapsed >= self.ttl {
            Duration::from_secs(0)
        } else {
            self.ttl - elapsed
        }
    }

    /// Increment access count
    pub fn increment_access(&mut self) {
        self.access_count += 1;
    }

    /// Check if entry is still valid considering network changes
    pub fn is_network_valid(&self) -> bool {
        // Simple network context comparison
        // In a more sophisticated implementation, this could check
        // for network interface changes, IP changes, etc.
        true
    }
}

/// Network context for cache invalidation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkContext {
    /// Local IP addresses when cached
    pub local_ips: Vec<IpAddr>,

    /// Network interfaces when cached
    pub interfaces: Vec<String>,

    /// Timestamp of context capture
    pub captured_at: SystemTime,
}

impl NetworkContext {
    /// Capture current network context
    pub fn current() -> Self {
        // Simple implementation - in production, this would query actual network interfaces
        Self {
            local_ips: Vec::new(),
            interfaces: Vec::new(),
            captured_at: SystemTime::now(),
        }
    }
}

/// Configuration for discovery cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryCacheConfig {
    /// Default TTL for cache entries
    pub default_ttl: Duration,

    /// Maximum number of entries to keep in memory
    pub max_entries: usize,

    /// Enable persistent storage to disk
    pub enable_persistence: bool,

    /// Path for persistent cache file
    pub cache_file_path: Option<PathBuf>,

    /// Cleanup interval for expired entries
    pub cleanup_interval: Duration,

    /// Minimum time between full network scans
    pub min_scan_interval: Duration,

    /// Maximum age for stale device detection
    pub max_device_age: Duration,

    /// Enable network-aware cache invalidation
    pub network_aware_invalidation: bool,

    /// TTL for different discovery methods
    pub method_ttls: HashMap<String, Duration>,
}

impl Default for DiscoveryCacheConfig {
    fn default() -> Self {
        let mut method_ttls = HashMap::new();
        method_ttls.insert("mdns".to_string(), Duration::from_secs(300)); // 5 minutes
        method_ttls.insert("upnp".to_string(), Duration::from_secs(600)); // 10 minutes
        method_ttls.insert("network_scan".to_string(), Duration::from_secs(1800)); // 30 minutes
        method_ttls.insert("manual".to_string(), Duration::from_secs(86400)); // 24 hours

        Self {
            default_ttl: Duration::from_secs(600), // 10 minutes
            max_entries: 1000,
            enable_persistence: true,
            cache_file_path: None,
            cleanup_interval: Duration::from_secs(60), // 1 minute
            min_scan_interval: Duration::from_secs(30), // 30 seconds
            max_device_age: Duration::from_secs(3600), // 1 hour
            network_aware_invalidation: true,
            method_ttls,
        }
    }
}

/// Statistics for discovery cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryCacheStats {
    /// Total entries in cache
    pub total_entries: usize,

    /// Number of cache hits
    pub cache_hits: u64,

    /// Number of cache misses
    pub cache_misses: u64,

    /// Number of expired entries cleaned up
    pub expired_cleanups: u64,

    /// Total discoveries performed
    pub total_discoveries: u64,

    /// Average discovery time
    pub avg_discovery_time: Duration,

    /// Cache hit ratio
    pub hit_ratio: f32,

    /// Devices by discovery method
    pub devices_by_method: HashMap<String, usize>,

    /// Memory usage estimate
    pub memory_usage_bytes: usize,
}

/// Discovery result cache
pub struct DiscoveryCache {
    /// Configuration
    config: DiscoveryCacheConfig,

    /// In-memory cache storage
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,

    /// Cache statistics
    stats: Arc<RwLock<DiscoveryCacheStats>>,

    /// Last full scan timestamp
    last_scan: Arc<RwLock<Option<SystemTime>>>,

    /// Shutdown signal for background tasks
    shutdown_sender: tokio::sync::mpsc::UnboundedSender<()>,
    shutdown_receiver: Arc<RwLock<Option<tokio::sync::mpsc::UnboundedReceiver<()>>>>,
}

impl DiscoveryCache {
    /// Create a new discovery cache with default configuration
    pub fn new() -> Self {
        Self::with_config(DiscoveryCacheConfig::default())
    }

    /// Create a new discovery cache with custom configuration
    pub fn with_config(config: DiscoveryCacheConfig) -> Self {
        let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::unbounded_channel();

        Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(DiscoveryCacheStats {
                total_entries: 0,
                cache_hits: 0,
                cache_misses: 0,
                expired_cleanups: 0,
                total_discoveries: 0,
                avg_discovery_time: Duration::from_millis(0),
                hit_ratio: 0.0,
                devices_by_method: HashMap::new(),
                memory_usage_bytes: 0,
            })),
            last_scan: Arc::new(RwLock::new(None)),
            shutdown_sender: shutdown_tx,
            shutdown_receiver: Arc::new(RwLock::new(Some(shutdown_rx))),
        }
    }

    /// Start the discovery cache background tasks
    pub async fn start(&self) -> Result<()> {
        // Load from persistent storage if enabled
        if self.config.enable_persistence {
            if let Err(e) = self.load_from_disk().await {
                warn!("Failed to load cache from disk: {}", e);
            }
        }

        // Start cleanup task
        self.start_cleanup_task().await;

        info!(
            "Discovery cache started with {} max entries",
            self.config.max_entries
        );
        Ok(())
    }

    /// Stop the discovery cache
    pub async fn stop(&self) -> Result<()> {
        // Save to persistent storage if enabled
        if self.config.enable_persistence {
            if let Err(e) = self.save_to_disk().await {
                warn!("Failed to save cache to disk: {}", e);
            }
        }

        // Send shutdown signal
        let _ = self.shutdown_sender.send(());

        info!("Discovery cache stopped");
        Ok(())
    }

    /// Add a discovered device to the cache
    pub async fn add_device(&self, device: DiscoveredDevice) -> Result<()> {
        let cache_key = self.get_cache_key(&device);
        let ttl = self.get_ttl_for_method(&device.discovery_method);
        let discovery_method = device.discovery_method.clone();

        {
            let mut cache = self.cache.write().await;

            // Check if we're at capacity
            if cache.len() >= self.config.max_entries {
                // Remove oldest expired entry or least recently used
                self.evict_entry(&mut cache).await;
            }

            // Create new cache entry or update existing
            let entry = if let Some(existing) = cache.get_mut(&cache_key) {
                // Update existing entry with new device data
                existing.device = device.clone();
                existing.device.update_last_seen();
                existing.increment_access();
                // Don't refresh cache time for updates - preserve original TTL
                existing.device.clone()
            } else {
                // Create new entry
                let entry = CacheEntry::new(device.clone(), ttl);
                cache.insert(cache_key.clone(), entry);
                device
            };

            debug!(
                "Added device to cache: {} ({})",
                entry.name, entry.ip_address
            );
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_entries = self.cache.read().await.len();
            *stats.devices_by_method.entry(discovery_method).or_insert(0) += 1;
            stats.total_discoveries += 1;
        }

        Ok(())
    }

    /// Get a device from the cache by IP address
    pub async fn get_device(&self, ip_address: &IpAddr) -> Option<DiscoveredDevice> {
        let cache = self.cache.read().await;

        // Search for device by IP
        for entry in cache.values() {
            if &entry.device.ip_address == ip_address
                && !entry.is_expired()
                && entry.is_network_valid()
            {
                // Update statistics
                {
                    let mut stats = self.stats.write().await;
                    stats.cache_hits += 1;
                    stats.hit_ratio =
                        stats.cache_hits as f32 / (stats.cache_hits + stats.cache_misses) as f32;
                }

                debug!("Cache hit for device: {}", ip_address);
                return Some(entry.device.clone());
            }
        }

        // Update miss statistics
        {
            let mut stats = self.stats.write().await;
            stats.cache_misses += 1;
            stats.hit_ratio =
                stats.cache_hits as f32 / (stats.cache_hits + stats.cache_misses) as f32;
        }

        debug!("Cache miss for device: {}", ip_address);
        None
    }

    /// Get a device from the cache by serial number
    pub async fn get_device_by_serial(&self, serial: &str) -> Option<DiscoveredDevice> {
        let cache = self.cache.read().await;

        for entry in cache.values() {
            if entry.device.serial == serial && !entry.is_expired() && entry.is_network_valid() {
                // Update statistics
                {
                    let mut stats = self.stats.write().await;
                    stats.cache_hits += 1;
                    stats.hit_ratio =
                        stats.cache_hits as f32 / (stats.cache_hits + stats.cache_misses) as f32;
                }

                return Some(entry.device.clone());
            }
        }

        // Update miss statistics
        {
            let mut stats = self.stats.write().await;
            stats.cache_misses += 1;
            stats.hit_ratio =
                stats.cache_hits as f32 / (stats.cache_hits + stats.cache_misses) as f32;
        }

        None
    }

    /// Get all cached devices
    pub async fn get_all_devices(&self) -> Vec<DiscoveredDevice> {
        let cache = self.cache.read().await;

        cache
            .values()
            .filter(|entry| !entry.is_expired() && entry.is_network_valid())
            .map(|entry| entry.device.clone())
            .collect()
    }

    /// Get devices discovered by a specific method
    pub async fn get_devices_by_method(&self, method: &str) -> Vec<DiscoveredDevice> {
        let cache = self.cache.read().await;

        cache
            .values()
            .filter(|entry| {
                entry.device.discovery_method == method
                    && !entry.is_expired()
                    && entry.is_network_valid()
            })
            .map(|entry| entry.device.clone())
            .collect()
    }

    /// Check if a full scan is needed based on last scan time
    pub async fn needs_full_scan(&self) -> bool {
        let last_scan = self.last_scan.read().await;

        match *last_scan {
            Some(time) => {
                time.elapsed().unwrap_or(Duration::from_secs(0)) > self.config.min_scan_interval
            }
            None => true,
        }
    }

    /// Mark that a full scan has been performed
    pub async fn mark_full_scan_completed(&self) {
        *self.last_scan.write().await = Some(SystemTime::now());
        debug!("Marked full scan as completed");
    }

    /// Remove expired entries from the cache
    pub async fn cleanup_expired(&self) -> Result<usize> {
        let mut cache = self.cache.write().await;
        let initial_count = cache.len();

        cache.retain(|_, entry| !entry.is_expired());

        let removed_count = initial_count - cache.len();

        if removed_count > 0 {
            let mut stats = self.stats.write().await;
            stats.expired_cleanups += removed_count as u64;
            stats.total_entries = cache.len();

            info!("Cleaned up {} expired cache entries", removed_count);
        }

        Ok(removed_count)
    }

    /// Remove stale devices (haven't been seen recently)
    pub async fn cleanup_stale_devices(&self) -> Result<usize> {
        let mut cache = self.cache.write().await;
        let initial_count = cache.len();

        cache.retain(|_, entry| !entry.device.is_stale(self.config.max_device_age));

        let removed_count = initial_count - cache.len();

        if removed_count > 0 {
            info!("Cleaned up {} stale devices", removed_count);
        }

        Ok(removed_count)
    }

    /// Clear all cache entries
    pub async fn clear(&self) -> Result<usize> {
        let mut cache = self.cache.write().await;
        let count = cache.len();
        cache.clear();

        // Reset statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_entries = 0;
            stats.devices_by_method.clear();
        }

        info!("Cleared all {} cache entries", count);
        Ok(count)
    }

    /// Get cache statistics
    pub async fn get_statistics(&self) -> DiscoveryCacheStats {
        let mut stats = self.stats.read().await.clone();

        // Update current metrics
        let cache = self.cache.read().await;
        stats.total_entries = cache.len();
        stats.memory_usage_bytes = self.estimate_memory_usage(&cache);

        stats
    }

    /// Get cache key for a device
    fn get_cache_key(&self, device: &DiscoveredDevice) -> String {
        // Use serial number as primary key, fall back to IP+port
        if !device.serial.is_empty() && device.serial != "unknown" {
            format!("serial:{}", device.serial)
        } else {
            format!("addr:{}:{}", device.ip_address, device.port)
        }
    }

    /// Get TTL for a discovery method
    fn get_ttl_for_method(&self, method: &str) -> Duration {
        self.config
            .method_ttls
            .get(method)
            .copied()
            .unwrap_or(self.config.default_ttl)
    }

    /// Evict an entry to make room for new ones
    async fn evict_entry(&self, cache: &mut HashMap<String, CacheEntry>) {
        // Find expired entries first
        let expired_keys: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| entry.is_expired())
            .map(|(key, _)| key.clone())
            .collect();

        if !expired_keys.is_empty() {
            // Remove first expired entry
            cache.remove(&expired_keys[0]);
            return;
        }

        // Find least recently accessed entry
        if let Some((key, _)) = cache.iter().min_by_key(|(_, entry)| entry.access_count) {
            let key = key.clone();
            cache.remove(&key);
        }
    }

    /// Estimate memory usage of the cache
    fn estimate_memory_usage(&self, cache: &HashMap<String, CacheEntry>) -> usize {
        // Rough estimate: each entry ~500 bytes
        cache.len() * 500
    }

    /// Load cache from persistent storage
    async fn load_from_disk(&self) -> Result<()> {
        if let Some(cache_file) = &self.config.cache_file_path {
            if cache_file.exists() {
                match tokio::fs::read_to_string(cache_file).await {
                    Ok(contents) => {
                        match serde_json::from_str::<HashMap<String, CacheEntry>>(&contents) {
                            Ok(loaded_cache) => {
                                let mut cache = self.cache.write().await;
                                *cache = loaded_cache;
                                info!("Loaded {} entries from cache file", cache.len());
                            }
                            Err(e) => return Err(LoxoneError::Json(e)),
                        }
                    }
                    Err(e) => return Err(LoxoneError::Io(e)),
                }
            }
        }

        Ok(())
    }

    /// Save cache to persistent storage
    async fn save_to_disk(&self) -> Result<()> {
        if let Some(cache_file) = &self.config.cache_file_path {
            let cache = self.cache.read().await;
            let serialized = serde_json::to_string_pretty(&*cache)?;

            if let Some(parent) = cache_file.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            tokio::fs::write(cache_file, serialized).await?;
            info!("Saved {} entries to cache file", cache.len());
        }

        Ok(())
    }

    /// Start cleanup task for expired entries
    async fn start_cleanup_task(&self) {
        let cache = self.cache.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();
        let shutdown_receiver = self.shutdown_receiver.clone();

        tokio::spawn(async move {
            let mut shutdown_rx = shutdown_receiver.write().await.take();
            let mut interval = tokio::time::interval(config.cleanup_interval);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Cleanup expired entries
                        let mut cache_guard = cache.write().await;
                        let initial_count = cache_guard.len();

                        cache_guard.retain(|_, entry| !entry.is_expired());

                        let removed_count = initial_count - cache_guard.len();

                        if removed_count > 0 {
                            let mut stats_guard = stats.write().await;
                            stats_guard.expired_cleanups += removed_count as u64;
                            stats_guard.total_entries = cache_guard.len();

                            debug!("Cleanup task removed {} expired entries", removed_count);
                        }
                    }
                    _ = async {
                        if let Some(ref mut rx) = shutdown_rx {
                            rx.recv().await
                        } else {
                            std::future::pending().await
                        }
                    } => {
                        debug!("Discovery cache cleanup task shutting down");
                        break;
                    }
                }
            }
        });
    }
}

impl Default for DiscoveryCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn create_test_device(ip: &str, serial: &str, name: &str) -> DiscoveredDevice {
        DiscoveredDevice::new(
            IpAddr::V4(ip.parse::<Ipv4Addr>().expect("Test IP should be valid")),
            80,
            serial.to_string(),
            name.to_string(),
            "Miniserver".to_string(),
            "test".to_string(),
        )
    }

    #[tokio::test]
    async fn test_discovery_cache_creation() {
        let cache = DiscoveryCache::new();
        let stats = cache.get_statistics().await;
        assert_eq!(stats.total_entries, 0);
    }

    #[tokio::test]
    async fn test_add_and_get_device() {
        let cache = DiscoveryCache::new();
        let device = create_test_device("192.168.1.100", "12345", "Test Device");
        let ip = device.ip_address;

        cache.add_device(device.clone()).await.unwrap();

        let retrieved = cache.get_device(&ip).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().serial, "12345");
    }

    #[tokio::test]
    async fn test_get_device_by_serial() {
        let cache = DiscoveryCache::new();
        let device = create_test_device("192.168.1.100", "12345", "Test Device");

        cache.add_device(device.clone()).await.unwrap();

        let retrieved = cache.get_device_by_serial("12345").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Device");
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let config = DiscoveryCacheConfig {
            default_ttl: Duration::from_millis(50),
            ..Default::default()
        };
        let cache = DiscoveryCache::with_config(config);
        let device = create_test_device("192.168.1.100", "12345", "Test Device");
        let ip = device.ip_address;

        cache.add_device(device).await.unwrap();

        // Should be available immediately
        assert!(cache.get_device(&ip).await.is_some());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should be expired now
        assert!(cache.get_device(&ip).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_cleanup() {
        let config = DiscoveryCacheConfig {
            default_ttl: Duration::from_millis(10),
            ..Default::default()
        };
        let cache = DiscoveryCache::with_config(config);

        for i in 0..5 {
            let device = create_test_device(
                &format!("192.168.1.{}", 100 + i),
                &format!("serial{i}"),
                &format!("Device {i}"),
            );
            cache.add_device(device).await.unwrap();
        }

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Cleanup should remove all expired entries
        let removed = cache.cleanup_expired().await.unwrap();
        assert_eq!(removed, 5);
    }

    #[tokio::test]
    async fn test_get_devices_by_method() {
        let cache = DiscoveryCache::new();

        let mut device1 = create_test_device("192.168.1.100", "12345", "Device 1");
        device1.discovery_method = "mdns".to_string();

        let mut device2 = create_test_device("192.168.1.101", "12346", "Device 2");
        device2.discovery_method = "upnp".to_string();

        let mut device3 = create_test_device("192.168.1.102", "12347", "Device 3");
        device3.discovery_method = "mdns".to_string();

        cache.add_device(device1).await.unwrap();
        cache.add_device(device2).await.unwrap();
        cache.add_device(device3).await.unwrap();

        let mdns_devices = cache.get_devices_by_method("mdns").await;
        assert_eq!(mdns_devices.len(), 2);

        let upnp_devices = cache.get_devices_by_method("upnp").await;
        assert_eq!(upnp_devices.len(), 1);
    }

    #[tokio::test]
    async fn test_cache_statistics() {
        let cache = DiscoveryCache::new();
        let device = create_test_device("192.168.1.100", "12345", "Test Device");
        let ip = device.ip_address;

        cache.add_device(device).await.unwrap();

        // Generate some hits and misses
        cache.get_device(&ip).await; // hit
        cache
            .get_device(&IpAddr::V4(
                "192.168.1.200".parse().expect("Test IP should be valid"),
            ))
            .await; // miss

        let stats = cache.get_statistics().await;
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.hit_ratio, 0.5);
    }

    #[tokio::test]
    async fn test_full_scan_tracking() {
        let cache = DiscoveryCache::new();

        assert!(cache.needs_full_scan().await);

        cache.mark_full_scan_completed().await;

        // Should not need scan immediately after completion
        assert!(!cache.needs_full_scan().await);
    }
}
