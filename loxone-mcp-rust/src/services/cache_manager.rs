//! Enhanced cache management for reducing redundant API calls
//!
//! This module provides intelligent caching strategies to minimize
//! API calls to the Loxone system while ensuring data freshness.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cache configuration for different data types
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// TTL for device state values (frequently changing)
    pub device_state_ttl: chrono::Duration,
    /// TTL for sensor readings (moderate change frequency)
    pub sensor_ttl: chrono::Duration,
    /// TTL for structure data (rarely changes)
    pub structure_ttl: chrono::Duration,
    /// TTL for room data (rarely changes)
    pub room_ttl: chrono::Duration,
    /// Maximum cache size before eviction
    pub max_cache_size: usize,
    /// Enable predictive prefetching
    pub enable_prefetch: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            device_state_ttl: chrono::Duration::seconds(30),
            sensor_ttl: chrono::Duration::seconds(60),
            structure_ttl: chrono::Duration::seconds(3600), // 1 hour
            room_ttl: chrono::Duration::seconds(3600),      // 1 hour
            max_cache_size: 10000,
            enable_prefetch: true,
        }
    }
}

/// Enhanced cache manager with intelligent eviction and prefetching
pub struct EnhancedCacheManager {
    /// Device state cache
    device_cache: Arc<RwLock<LruCache<String, CachedValue<serde_json::Value>>>>,
    /// Batch request cache (for reducing redundant batch calls)
    batch_cache: Arc<RwLock<HashMap<String, BatchCacheEntry>>>,
    /// Access pattern tracker for predictive prefetching
    access_tracker: Arc<RwLock<AccessPatternTracker>>,
    /// Cache configuration
    config: CacheConfig,
}

#[derive(Debug, Clone)]
struct CachedValue<T> {
    value: T,
    timestamp: DateTime<Utc>,
    #[allow(dead_code)]
    access_count: u64,
    #[allow(dead_code)]
    last_access: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct BatchCacheEntry {
    device_states: HashMap<String, serde_json::Value>,
    timestamp: DateTime<Utc>,
    #[allow(dead_code)]
    request_hash: String,
}

#[derive(Debug, Default)]
struct AccessPatternTracker {
    /// Track which devices are frequently accessed together
    co_access_patterns: HashMap<String, Vec<String>>,
    /// Track access frequency for each device
    access_frequency: HashMap<String, AccessFrequency>,
    /// Track typical access intervals
    access_intervals: HashMap<String, Vec<chrono::Duration>>,
}

#[derive(Debug, Clone)]
struct AccessFrequency {
    count: u64,
    last_access: DateTime<Utc>,
    average_interval: chrono::Duration,
}

/// Simple LRU cache implementation
pub struct LruCache<K, V> {
    capacity: usize,
    map: HashMap<K, V>,
    access_order: Vec<K>,
}

impl<K: Clone + Eq + std::hash::Hash, V> LruCache<K, V> {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            map: HashMap::with_capacity(capacity),
            access_order: Vec::with_capacity(capacity),
        }
    }

    fn get(&mut self, key: &K) -> Option<&V> {
        if self.map.contains_key(key) {
            // Move to end (most recently used)
            self.access_order.retain(|k| k != key);
            self.access_order.push(key.clone());
            self.map.get(key)
        } else {
            None
        }
    }

    fn insert(&mut self, key: K, value: V) {
        if self.map.len() >= self.capacity && !self.map.contains_key(&key) {
            // Evict least recently used
            if let Some(lru_key) = self.access_order.first().cloned() {
                self.access_order.remove(0);
                self.map.remove(&lru_key);
            }
        }

        self.map.insert(key.clone(), value);
        self.access_order.retain(|k| k != &key);
        self.access_order.push(key);
    }

    fn clear(&mut self) {
        self.map.clear();
        self.access_order.clear();
    }
}

impl EnhancedCacheManager {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            device_cache: Arc::new(RwLock::new(LruCache::new(config.max_cache_size))),
            batch_cache: Arc::new(RwLock::new(HashMap::new())),
            access_tracker: Arc::new(RwLock::new(AccessPatternTracker::default())),
            config,
        }
    }

    /// Get a single device value with intelligent caching
    pub async fn get_device_value(
        &self,
        uuid: &str,
        fetch_fn: impl std::future::Future<Output = Result<serde_json::Value>>,
    ) -> Result<serde_json::Value> {
        // Check cache first
        {
            let mut cache = self.device_cache.write().await;
            if let Some(cached) = cache.get(&uuid.to_string()) {
                let age = Utc::now() - cached.timestamp;
                if age < self.config.device_state_ttl {
                    // Update access tracking
                    self.track_access(uuid).await;
                    return Ok(cached.value.clone());
                }
            }
        }

        // Fetch new value
        let value = fetch_fn.await?;

        // Cache the result
        {
            let mut cache = self.device_cache.write().await;
            cache.insert(
                uuid.to_string(),
                CachedValue {
                    value: value.clone(),
                    timestamp: Utc::now(),
                    access_count: 1,
                    last_access: Utc::now(),
                },
            );
        }

        // Update access tracking
        self.track_access(uuid).await;

        // Predictive prefetch related devices
        if self.config.enable_prefetch {
            self.prefetch_related_devices(uuid).await;
        }

        Ok(value)
    }

    /// Get batch device values with deduplication
    pub async fn get_batch_device_values(
        &self,
        uuids: &[String],
        fetch_fn: impl std::future::Future<Output = Result<HashMap<String, serde_json::Value>>>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut result = HashMap::new();
        let mut uncached_uuids = Vec::new();

        // Check cache for each UUID
        {
            let mut cache = self.device_cache.write().await;
            for uuid in uuids {
                if let Some(cached) = cache.get(uuid) {
                    let age = Utc::now() - cached.timestamp;
                    if age < self.config.device_state_ttl {
                        result.insert(uuid.clone(), cached.value.clone());
                        continue;
                    }
                }
                uncached_uuids.push(uuid.clone());
            }
        }

        // If all values are cached, return immediately
        if uncached_uuids.is_empty() {
            return Ok(result);
        }

        // Check if we have a recent batch request that includes these UUIDs
        let batch_key = self.generate_batch_key(&uncached_uuids);
        {
            let batch_cache = self.batch_cache.read().await;
            if let Some(batch_entry) = batch_cache.get(&batch_key) {
                let age = Utc::now() - batch_entry.timestamp;
                if age < self.config.device_state_ttl {
                    // Use cached batch result
                    for uuid in &uncached_uuids {
                        if let Some(value) = batch_entry.device_states.get(uuid) {
                            result.insert(uuid.clone(), value.clone());
                        }
                    }
                    return Ok(result);
                }
            }
        }

        // Fetch new values
        let fetched_values = fetch_fn.await?;

        // Cache individual values and update result
        {
            let mut cache = self.device_cache.write().await;
            for (uuid, value) in &fetched_values {
                cache.insert(
                    uuid.clone(),
                    CachedValue {
                        value: value.clone(),
                        timestamp: Utc::now(),
                        access_count: 1,
                        last_access: Utc::now(),
                    },
                );
                result.insert(uuid.clone(), value.clone());
            }
        }

        // Cache the batch request
        {
            let mut batch_cache = self.batch_cache.write().await;
            batch_cache.insert(
                batch_key,
                BatchCacheEntry {
                    device_states: fetched_values,
                    timestamp: Utc::now(),
                    request_hash: self.generate_batch_key(&uncached_uuids),
                },
            );
        }

        // Track co-access patterns
        self.track_co_access(uuids).await;

        Ok(result)
    }

    /// Clear all caches
    pub async fn clear_all(&self) {
        self.device_cache.write().await.clear();
        self.batch_cache.write().await.clear();
        self.access_tracker.write().await.co_access_patterns.clear();
    }

    /// Get cache statistics
    pub async fn get_statistics(&self) -> CacheStatistics {
        let device_cache = self.device_cache.read().await;
        let batch_cache = self.batch_cache.read().await;
        let tracker = self.access_tracker.read().await;

        CacheStatistics {
            device_cache_size: device_cache.map.len(),
            batch_cache_size: batch_cache.len(),
            tracked_patterns: tracker.co_access_patterns.len(),
            total_access_count: tracker.access_frequency.values().map(|f| f.count).sum(),
        }
    }

    /// Track device access for pattern analysis
    async fn track_access(&self, uuid: &str) {
        let mut tracker = self.access_tracker.write().await;
        let now = Utc::now();

        if let Some(freq) = tracker.access_frequency.get_mut(uuid) {
            let interval = now - freq.last_access;
            freq.count += 1;
            freq.last_access = now;

            // Update average interval
            freq.average_interval = chrono::Duration::seconds(
                (freq.average_interval.num_seconds() * (freq.count as i64 - 1)
                    + interval.num_seconds())
                    / freq.count as i64,
            );

            // Track intervals
            if let Some(intervals) = tracker.access_intervals.get_mut(uuid) {
                intervals.push(interval);
                if intervals.len() > 10 {
                    intervals.remove(0);
                }
            }
        } else {
            tracker.access_frequency.insert(
                uuid.to_string(),
                AccessFrequency {
                    count: 1,
                    last_access: now,
                    average_interval: chrono::Duration::seconds(60),
                },
            );
        }
    }

    /// Track co-access patterns for predictive prefetching
    async fn track_co_access(&self, uuids: &[String]) {
        if uuids.len() < 2 {
            return;
        }

        let mut tracker = self.access_tracker.write().await;

        // Update co-access patterns
        for uuid in uuids {
            let others: Vec<String> = uuids.iter().filter(|u| *u != uuid).cloned().collect();

            tracker
                .co_access_patterns
                .entry(uuid.clone())
                .or_insert_with(Vec::new)
                .extend(others);
        }
    }

    /// Predictively prefetch related devices
    async fn prefetch_related_devices(&self, uuid: &str) {
        let tracker = self.access_tracker.read().await;

        if let Some(related) = tracker.co_access_patterns.get(uuid) {
            // Find frequently co-accessed devices
            let mut frequency_map: HashMap<&str, usize> = HashMap::new();
            for device in related {
                *frequency_map.entry(device).or_insert(0) += 1;
            }

            // Prefetch top co-accessed devices
            let mut top_devices: Vec<_> = frequency_map.into_iter().collect();
            top_devices.sort_by(|a, b| b.1.cmp(&a.1));

            for (device_uuid, _) in top_devices.iter().take(5) {
                // Check if device needs prefetching based on access pattern
                if let Some(freq) = tracker.access_frequency.get(*device_uuid) {
                    let time_since_last = Utc::now() - freq.last_access;
                    if time_since_last < freq.average_interval * 2 {
                        // This device is likely to be accessed soon
                        // TODO: Trigger background prefetch
                        tracing::debug!("Would prefetch device: {}", device_uuid);
                    }
                }
            }
        }
    }

    /// Generate a consistent key for batch requests
    fn generate_batch_key(&self, uuids: &[String]) -> String {
        let mut sorted_uuids = uuids.to_vec();
        sorted_uuids.sort();
        format!("batch:{}", sorted_uuids.join(","))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheStatistics {
    pub device_cache_size: usize,
    pub batch_cache_size: usize,
    pub tracked_patterns: usize,
    pub total_access_count: u64,
}
