//! Response caching for MCP tools with TTL-based eviction
//!
//! Provides a generic response cache for MCP tool results to improve performance
//! and reduce load on the Loxone system. Supports TTL-based expiration and
//! automatic cleanup of stale entries.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, trace, warn};

/// Cache entry with TTL and metadata
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    /// Cached value
    pub value: T,

    /// When the entry was created
    pub created_at: Instant,

    /// Time-to-live duration
    pub ttl: Duration,

    /// Access count for LRU eviction
    pub access_count: u64,

    /// Last accessed timestamp
    pub last_accessed: Instant,

    /// Size hint for memory management
    pub size_hint: usize,
}

impl<T> CacheEntry<T> {
    /// Create a new cache entry
    pub fn new(value: T, ttl: Duration) -> Self {
        let now = Instant::now();
        let size_hint = std::mem::size_of::<T>();

        Self {
            value,
            created_at: now,
            ttl,
            access_count: 0,
            last_accessed: now,
            size_hint,
        }
    }

    /// Check if the entry has expired
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }

    /// Check if the entry is stale (older than 80% of TTL)
    pub fn is_stale(&self) -> bool {
        self.created_at.elapsed() > Duration::from_nanos((self.ttl.as_nanos() * 8 / 10) as u64)
    }

    /// Get the remaining TTL
    pub fn remaining_ttl(&self) -> Duration {
        self.ttl.saturating_sub(self.created_at.elapsed())
    }

    /// Mark as accessed and return the value reference
    pub fn access(&mut self) -> &T {
        self.access_count += 1;
        self.last_accessed = Instant::now();
        &self.value
    }

    /// Get value without updating access stats
    pub fn peek(&self) -> &T {
        &self.value
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total cache hits
    pub hits: u64,

    /// Total cache misses
    pub misses: u64,

    /// Total entries evicted due to TTL
    pub ttl_evictions: u64,

    /// Total entries evicted due to size limits
    pub size_evictions: u64,

    /// Current number of entries
    pub entry_count: usize,

    /// Estimated memory usage in bytes
    pub estimated_memory_bytes: usize,

    /// Average response time in microseconds
    pub avg_response_time_us: u64,
}

impl CacheStats {
    /// Calculate hit ratio
    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Check if cache is performing well
    pub fn is_healthy(&self) -> bool {
        self.hit_ratio() > 0.5 && self.entry_count > 0
    }
}

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries
    pub max_entries: usize,

    /// Maximum memory usage in bytes
    pub max_memory_bytes: usize,

    /// Default TTL for entries
    pub default_ttl: Duration,

    /// Cleanup interval
    pub cleanup_interval: Duration,

    /// Enable automatic background cleanup
    pub auto_cleanup: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            max_memory_bytes: 10 * 1024 * 1024,        // 10MB
            default_ttl: Duration::from_secs(300),     // 5 minutes
            cleanup_interval: Duration::from_secs(60), // 1 minute
            auto_cleanup: true,
        }
    }
}

/// Generic response cache with TTL-based eviction
#[derive(Debug)]
pub struct ResponseCache<K, V>
where
    K: Clone + Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Cache storage
    entries: Arc<RwLock<HashMap<K, CacheEntry<V>>>>,

    /// Cache configuration
    config: CacheConfig,

    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,

    /// Background cleanup task handle
    cleanup_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl<K, V> ResponseCache<K, V>
where
    K: Clone + Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Create a new response cache with default configuration
    pub fn new() -> Self {
        Self::with_config(CacheConfig::default())
    }

    /// Create a new response cache with custom configuration
    pub fn with_config(config: CacheConfig) -> Self {
        let cache = Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            config,
            stats: Arc::new(RwLock::new(CacheStats::default())),
            cleanup_handle: Arc::new(RwLock::new(None)),
        };

        if cache.config.auto_cleanup {
            cache.start_background_cleanup();
        }

        cache
    }

    /// Get a value from the cache
    pub async fn get(&self, key: &K) -> Option<V> {
        let start_time = Instant::now();

        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        if let Some(entry) = entries.get_mut(key) {
            if entry.is_expired() {
                // Remove expired entry
                entries.remove(key);
                stats.ttl_evictions += 1;
                stats.misses += 1;
                debug!("Cache entry expired and removed");
                None
            } else {
                // Return cached value
                let value = entry.access().clone();
                stats.hits += 1;

                // Update response time
                let response_time = start_time.elapsed().as_micros() as u64;
                stats.avg_response_time_us = if stats.hits == 1 {
                    response_time
                } else {
                    (stats.avg_response_time_us + response_time) / 2
                };

                trace!("Cache hit for key, access_count: {}", entry.access_count);
                Some(value)
            }
        } else {
            stats.misses += 1;
            debug!("Cache miss for key");
            None
        }
    }

    /// Put a value into the cache with default TTL
    pub async fn put(&self, key: K, value: V) {
        self.put_with_ttl(key, value, self.config.default_ttl).await;
    }

    /// Put a value into the cache with custom TTL
    pub async fn put_with_ttl(&self, key: K, value: V, ttl: Duration) {
        let entry = CacheEntry::new(value, ttl);

        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        // Check if we need to evict entries
        if entries.len() >= self.config.max_entries {
            self.evict_lru_entry(&mut entries, &mut stats).await;
        }

        // Check memory usage
        let estimated_size = entry.size_hint;
        if stats.estimated_memory_bytes + estimated_size > self.config.max_memory_bytes {
            self.evict_by_memory(&mut entries, &mut stats, estimated_size)
                .await;
        }

        // Insert the new entry
        entries.insert(key, entry);
        stats.entry_count = entries.len();
        stats.estimated_memory_bytes += estimated_size;

        trace!("Cache entry added, total entries: {}", entries.len());
    }

    /// Remove a specific key from the cache
    pub async fn remove(&self, key: &K) -> Option<V> {
        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        if let Some(entry) = entries.remove(key) {
            stats.entry_count = entries.len();
            stats.estimated_memory_bytes =
                stats.estimated_memory_bytes.saturating_sub(entry.size_hint);
            Some(entry.value)
        } else {
            None
        }
    }

    /// Clear all entries from the cache
    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        entries.clear();
        stats.entry_count = 0;
        stats.estimated_memory_bytes = 0;

        debug!("Cache cleared");
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// Manually trigger cleanup of expired entries
    pub async fn cleanup(&self) {
        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        let initial_count = entries.len();
        let mut evicted_size = 0;

        // Remove expired entries
        entries.retain(|_key, entry| {
            if entry.is_expired() {
                evicted_size += entry.size_hint;
                stats.ttl_evictions += 1;
                false
            } else {
                true
            }
        });

        let evicted_count = initial_count - entries.len();
        if evicted_count > 0 {
            stats.entry_count = entries.len();
            stats.estimated_memory_bytes =
                stats.estimated_memory_bytes.saturating_sub(evicted_size);
            debug!("Cache cleanup: removed {} expired entries", evicted_count);
        }
    }

    /// Check if a key exists and is not expired
    pub async fn contains_key(&self, key: &K) -> bool {
        let entries = self.entries.read().await;
        if let Some(entry) = entries.get(key) {
            !entry.is_expired()
        } else {
            false
        }
    }

    /// Get all keys in the cache (non-expired)
    pub async fn keys(&self) -> Vec<K> {
        let entries = self.entries.read().await;
        entries
            .iter()
            .filter(|(_, entry)| !entry.is_expired())
            .map(|(key, _)| key.clone())
            .collect()
    }

    /// Get cache size
    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Check if cache is empty
    pub async fn is_empty(&self) -> bool {
        self.entries.read().await.is_empty()
    }

    /// Start background cleanup task
    fn start_background_cleanup(&self) {
        let entries = self.entries.clone();
        let stats = self.stats.clone();
        let cleanup_interval = self.config.cleanup_interval;
        let handle = self.cleanup_handle.clone();

        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);

            loop {
                interval.tick().await;

                // Cleanup expired entries
                let mut entries_guard = entries.write().await;
                let mut stats_guard = stats.write().await;

                let initial_count = entries_guard.len();
                let mut evicted_size = 0;

                entries_guard.retain(|_key, entry| {
                    if entry.is_expired() {
                        evicted_size += entry.size_hint;
                        stats_guard.ttl_evictions += 1;
                        false
                    } else {
                        true
                    }
                });

                let evicted_count = initial_count - entries_guard.len();
                if evicted_count > 0 {
                    stats_guard.entry_count = entries_guard.len();
                    stats_guard.estimated_memory_bytes = stats_guard
                        .estimated_memory_bytes
                        .saturating_sub(evicted_size);
                    trace!(
                        "Background cleanup: removed {} expired entries",
                        evicted_count
                    );
                }
            }
        });

        tokio::spawn(async move {
            *handle.write().await = Some(task);
        });
    }

    /// Evict the least recently used entry
    async fn evict_lru_entry(
        &self,
        entries: &mut HashMap<K, CacheEntry<V>>,
        stats: &mut CacheStats,
    ) {
        if let Some((lru_key, lru_entry)) = entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(k, v)| (k.clone(), v.clone()))
        {
            entries.remove(&lru_key);
            stats.size_evictions += 1;
            stats.estimated_memory_bytes = stats
                .estimated_memory_bytes
                .saturating_sub(lru_entry.size_hint);
            debug!("Evicted LRU entry due to size limit");
        }
    }

    /// Evict entries to make room for new entry
    async fn evict_by_memory(
        &self,
        entries: &mut HashMap<K, CacheEntry<V>>,
        stats: &mut CacheStats,
        needed_size: usize,
    ) {
        let mut freed_size = 0;
        let target_size = needed_size + (self.config.max_memory_bytes / 10); // Free 10% extra

        // Sort by last accessed time (LRU first)
        let mut sorted_entries: Vec<_> = entries.iter().collect();
        sorted_entries.sort_by_key(|(_, entry)| entry.last_accessed);

        let mut keys_to_remove = Vec::new();

        for (key, entry) in sorted_entries {
            if freed_size >= target_size {
                break;
            }

            keys_to_remove.push(key.clone());
            freed_size += entry.size_hint;
        }

        for key in keys_to_remove {
            if let Some(entry) = entries.remove(&key) {
                stats.size_evictions += 1;
                stats.estimated_memory_bytes =
                    stats.estimated_memory_bytes.saturating_sub(entry.size_hint);
            }
        }

        if freed_size > 0 {
            warn!("Evicted entries to free {} bytes of memory", freed_size);
        }
    }
}

impl<K, V> Drop for ResponseCache<K, V>
where
    K: Clone + Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn drop(&mut self) {
        // Cancel background cleanup task
        if let Ok(mut handle_guard) = self.cleanup_handle.try_write() {
            if let Some(handle) = handle_guard.take() {
                handle.abort();
            }
        }
    }
}

impl<K, V> Default for ResponseCache<K, V>
where
    K: Clone + Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Type alias for MCP tool response cache
pub type ToolResponseCache = ResponseCache<String, serde_json::Value>;

/// Helper function to create a cache key from tool name and parameters
pub fn create_cache_key(tool_name: &str, params: &serde_json::Value) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    tool_name.hash(&mut hasher);
    params.to_string().hash(&mut hasher);

    format!("{}:{:x}", tool_name, hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let cache: ResponseCache<String, String> = ResponseCache::new();

        // Test put and get
        cache.put("key1".to_string(), "value1".to_string()).await;
        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, Some("value1".to_string()));

        // Test cache miss
        let missing = cache.get(&"nonexistent".to_string()).await;
        assert_eq!(missing, None);

        // Test remove
        let removed = cache.remove(&"key1".to_string()).await;
        assert_eq!(removed, Some("value1".to_string()));

        let after_remove = cache.get(&"key1".to_string()).await;
        assert_eq!(after_remove, None);
    }

    #[tokio::test]
    async fn test_cache_ttl_expiration() {
        let cache: ResponseCache<String, String> = ResponseCache::new();

        // Put with short TTL
        cache
            .put_with_ttl(
                "key1".to_string(),
                "value1".to_string(),
                Duration::from_millis(50),
            )
            .await;

        // Should be available immediately
        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, Some("value1".to_string()));

        // Wait for expiration
        sleep(Duration::from_millis(100)).await;

        // Should be expired now
        let expired = cache.get(&"key1".to_string()).await;
        assert_eq!(expired, None);
    }

    #[tokio::test]
    async fn test_cache_statistics() {
        let cache: ResponseCache<String, String> = ResponseCache::new();

        // Initially empty
        let initial_stats = cache.stats().await;
        assert_eq!(initial_stats.hits, 0);
        assert_eq!(initial_stats.misses, 0);

        // Add entry
        cache.put("key1".to_string(), "value1".to_string()).await;

        // Hit
        cache.get(&"key1".to_string()).await;
        let stats_after_hit = cache.stats().await;
        assert_eq!(stats_after_hit.hits, 1);
        assert_eq!(stats_after_hit.misses, 0);

        // Miss
        cache.get(&"nonexistent".to_string()).await;
        let stats_after_miss = cache.stats().await;
        assert_eq!(stats_after_miss.hits, 1);
        assert_eq!(stats_after_miss.misses, 1);

        // Check hit ratio
        assert_eq!(stats_after_miss.hit_ratio(), 0.5);
    }

    #[tokio::test]
    async fn test_cache_size_limit() {
        let config = CacheConfig {
            max_entries: 2,
            auto_cleanup: false,
            ..Default::default()
        };
        let cache: ResponseCache<String, String> = ResponseCache::with_config(config);

        // Add entries up to limit
        cache.put("key1".to_string(), "value1".to_string()).await;
        cache.put("key2".to_string(), "value2".to_string()).await;

        // Access key1 to make it more recently used
        cache.get(&"key1".to_string()).await;

        // Add third entry, should evict key2 (LRU)
        cache.put("key3".to_string(), "value3".to_string()).await;

        assert_eq!(
            cache.get(&"key1".to_string()).await,
            Some("value1".to_string())
        );
        assert_eq!(cache.get(&"key2".to_string()).await, None); // Evicted
        assert_eq!(
            cache.get(&"key3".to_string()).await,
            Some("value3".to_string())
        );
    }

    #[tokio::test]
    async fn test_cache_cleanup() {
        let cache: ResponseCache<String, String> = ResponseCache::new();

        // Add entries with different TTLs
        cache
            .put_with_ttl(
                "short".to_string(),
                "value".to_string(),
                Duration::from_millis(50),
            )
            .await;
        cache
            .put_with_ttl(
                "long".to_string(),
                "value".to_string(),
                Duration::from_secs(10),
            )
            .await;

        assert_eq!(cache.len().await, 2);

        // Wait for short TTL to expire
        sleep(Duration::from_millis(100)).await;

        // Manual cleanup
        cache.cleanup().await;

        assert_eq!(cache.len().await, 1);
        assert_eq!(cache.get(&"short".to_string()).await, None);
        assert_eq!(
            cache.get(&"long".to_string()).await,
            Some("value".to_string())
        );
    }

    #[test]
    fn test_create_cache_key() {
        let params1 = serde_json::json!({"param": "value1"});
        let params2 = serde_json::json!({"param": "value2"});

        let key1 = create_cache_key("test_tool", &params1);
        let key2 = create_cache_key("test_tool", &params2);
        let key3 = create_cache_key("test_tool", &params1);

        // Same parameters should produce same key
        assert_eq!(key1, key3);

        // Different parameters should produce different keys
        assert_ne!(key1, key2);

        // Keys should contain tool name
        assert!(key1.contains("test_tool"));
    }
}
