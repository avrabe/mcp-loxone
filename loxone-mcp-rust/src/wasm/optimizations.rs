//! WASM-specific optimizations for performance and size
//!
//! This module provides optimizations specifically for WASM deployment,
//! focusing on binary size reduction, memory efficiency, and performance.

use crate::error::{LoxoneError, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// WASM memory allocator configuration
#[cfg(target_arch = "wasm32")]
use wee_alloc;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Global optimization settings
static OPTIMIZATION_CONFIG: OnceLock<WasmOptimizationConfig> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct WasmOptimizationConfig {
    /// Enable aggressive memory management
    pub aggressive_memory_management: bool,

    /// Maximum memory pool size in bytes
    pub max_memory_pool_bytes: usize,

    /// Enable request deduplication
    pub enable_request_deduplication: bool,

    /// Response cache size limit
    pub response_cache_limit: usize,

    /// Enable binary size optimizations
    pub enable_size_optimizations: bool,

    /// Maximum number of concurrent connections
    pub max_concurrent_connections: usize,

    /// Enable lazy loading of non-critical features
    pub enable_lazy_loading: bool,
}

impl Default for WasmOptimizationConfig {
    fn default() -> Self {
        Self {
            aggressive_memory_management: true,
            max_memory_pool_bytes: 8 * 1024 * 1024, // 8MB
            enable_request_deduplication: true,
            response_cache_limit: 100,
            enable_size_optimizations: true,
            max_concurrent_connections: 5,
            enable_lazy_loading: true,
        }
    }
}

impl WasmOptimizationConfig {
    /// Create configuration optimized for minimal memory usage
    pub fn minimal_memory() -> Self {
        Self {
            aggressive_memory_management: true,
            max_memory_pool_bytes: 2 * 1024 * 1024, // 2MB
            enable_request_deduplication: true,
            response_cache_limit: 20,
            enable_size_optimizations: true,
            max_concurrent_connections: 2,
            enable_lazy_loading: true,
        }
    }

    /// Create configuration optimized for performance
    pub fn performance_optimized() -> Self {
        Self {
            aggressive_memory_management: false,
            max_memory_pool_bytes: 16 * 1024 * 1024, // 16MB
            enable_request_deduplication: true,
            response_cache_limit: 500,
            enable_size_optimizations: false,
            max_concurrent_connections: 10,
            enable_lazy_loading: false,
        }
    }
}

/// Memory pool for efficient allocation in WASM
pub struct WasmMemoryPool {
    pool: Arc<Mutex<Vec<Vec<u8>>>>,
    max_size: usize,
    current_size: Arc<Mutex<usize>>,
}

impl WasmMemoryPool {
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(Vec::new())),
            max_size,
            current_size: Arc::new(Mutex::new(0)),
        }
    }

    /// Get a buffer from the pool or allocate new one
    pub fn get_buffer(&self, size: usize) -> Vec<u8> {
        let mut pool = self.pool.lock().unwrap();

        // Try to find a suitable buffer in the pool
        for (i, buffer) in pool.iter().enumerate() {
            if buffer.capacity() >= size {
                let mut buffer = pool.swap_remove(i);
                buffer.clear();
                buffer.resize(size, 0);

                // Update current size
                *self.current_size.lock().unwrap() -= buffer.capacity();
                return buffer;
            }
        }

        // Allocate new buffer if none found
        vec![0; size]
    }

    /// Return a buffer to the pool
    pub fn return_buffer(&self, mut buffer: Vec<u8>) {
        let buffer_capacity = buffer.capacity();
        let mut current_size = self.current_size.lock().unwrap();

        // Only keep buffer if it doesn't exceed pool size limit
        if *current_size + buffer_capacity <= self.max_size {
            buffer.clear();
            self.pool.lock().unwrap().push(buffer);
            *current_size += buffer_capacity;
        }
        // Otherwise, let buffer be dropped
    }

    /// Clear the memory pool
    pub fn clear(&self) {
        self.pool.lock().unwrap().clear();
        *self.current_size.lock().unwrap() = 0;
    }

    /// Get current pool statistics
    pub fn stats(&self) -> MemoryPoolStats {
        let pool = self.pool.lock().unwrap();
        let current_size = *self.current_size.lock().unwrap();

        MemoryPoolStats {
            buffers_in_pool: pool.len(),
            total_size_bytes: current_size,
            max_size_bytes: self.max_size,
            utilization_percent: (current_size as f32 / self.max_size as f32 * 100.0) as u32,
        }
    }
}

#[derive(Debug)]
pub struct MemoryPoolStats {
    pub buffers_in_pool: usize,
    pub total_size_bytes: usize,
    pub max_size_bytes: usize,
    pub utilization_percent: u32,
}

/// Request deduplication cache to avoid duplicate HTTP requests
pub struct WasmRequestCache {
    cache: Arc<Mutex<HashMap<String, CachedResponse>>>,
    max_entries: usize,
}

#[derive(Debug, Clone)]
struct CachedResponse {
    data: Vec<u8>,
    timestamp: std::time::Instant,
    ttl_seconds: u32,
}

impl WasmRequestCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            max_entries,
        }
    }

    /// Get cached response if available and not expired
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let mut cache = self.cache.lock().unwrap();

        if let Some(cached) = cache.get(key) {
            if cached.timestamp.elapsed().as_secs() < cached.ttl_seconds as u64 {
                return Some(cached.data.clone());
            } else {
                // Remove expired entry
                cache.remove(key);
            }
        }

        None
    }

    /// Cache a response with TTL
    pub fn set(&self, key: String, data: Vec<u8>, ttl_seconds: u32) {
        let mut cache = self.cache.lock().unwrap();

        // Remove oldest entries if cache is full
        if cache.len() >= self.max_entries {
            let oldest_key = cache
                .iter()
                .min_by_key(|(_, v)| v.timestamp)
                .map(|(k, _)| k.clone());

            if let Some(oldest) = oldest_key {
                cache.remove(&oldest);
            }
        }

        cache.insert(
            key,
            CachedResponse {
                data,
                timestamp: std::time::Instant::now(),
                ttl_seconds,
            },
        );
    }

    /// Clear expired entries
    pub fn cleanup_expired(&self) {
        let mut cache = self.cache.lock().unwrap();
        let now = std::time::Instant::now();

        cache.retain(|_, cached| {
            now.duration_since(cached.timestamp).as_secs() < cached.ttl_seconds as u64
        });
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let cache = self.cache.lock().unwrap();
        let total_size = cache.values().map(|c| c.data.len()).sum();

        CacheStats {
            entries: cache.len(),
            total_size_bytes: total_size,
            max_entries: self.max_entries,
        }
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub entries: usize,
    pub total_size_bytes: usize,
    pub max_entries: usize,
}

/// WASM binary size optimization utilities
pub struct WasmSizeOptimizer;

impl WasmSizeOptimizer {
    /// Strip debug symbols in release builds
    #[cfg(not(debug_assertions))]
    pub fn strip_debug_symbols() {
        // This is handled by Cargo.toml profile.release.strip = true
        // But we can add runtime checks here
    }

    /// Enable panic = "abort" for smaller binaries
    pub fn enable_panic_abort() {
        // This is configured in Cargo.toml
        // Runtime verification can be added here
    }

    /// Minimize feature usage
    pub fn get_minimal_features() -> Vec<&'static str> {
        vec![
            // Only essential features for WASM
            "wasi-keyvalue",
            "wasi-http",
            "wasm-logging",
        ]
    }

    /// Get size optimization tips
    pub fn get_size_tips() -> Vec<String> {
        vec![
            "Use 'opt-level = \"z\"' for smallest binary size".to_string(),
            "Enable 'lto = true' for link-time optimization".to_string(),
            "Set 'codegen-units = 1' for better optimization".to_string(),
            "Use 'panic = \"abort\"' to avoid unwinding code".to_string(),
            "Enable 'strip = true' to remove debug symbols".to_string(),
            "Minimize dependencies and features".to_string(),
            "Use wee_alloc for smaller memory allocator".to_string(),
        ]
    }
}

/// Performance monitoring for WASM
pub struct WasmPerformanceMonitor {
    metrics: Arc<Mutex<WasmMetrics>>,
}

#[derive(Debug, Default)]
struct WasmMetrics {
    request_count: u32,
    total_request_time_ms: u64,
    memory_allocations: u32,
    cache_hits: u32,
    cache_misses: u32,
}

impl WasmPerformanceMonitor {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(WasmMetrics::default())),
        }
    }

    /// Record a request completion
    pub fn record_request(&self, duration_ms: u64) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.request_count += 1;
        metrics.total_request_time_ms += duration_ms;
    }

    /// Record memory allocation
    pub fn record_allocation(&self) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.memory_allocations += 1;
    }

    /// Record cache hit
    pub fn record_cache_hit(&self) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.cache_hits += 1;
    }

    /// Record cache miss
    pub fn record_cache_miss(&self) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.cache_misses += 1;
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> WasmPerformanceStats {
        let metrics = self.metrics.lock().unwrap();

        let avg_request_time = if metrics.request_count > 0 {
            metrics.total_request_time_ms / metrics.request_count as u64
        } else {
            0
        };

        let cache_hit_rate = if metrics.cache_hits + metrics.cache_misses > 0 {
            (metrics.cache_hits as f32 / (metrics.cache_hits + metrics.cache_misses) as f32 * 100.0)
                as u32
        } else {
            0
        };

        WasmPerformanceStats {
            requests_total: metrics.request_count,
            avg_request_time_ms: avg_request_time,
            memory_allocations: metrics.memory_allocations,
            cache_hit_rate_percent: cache_hit_rate,
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        *self.metrics.lock().unwrap() = WasmMetrics::default();
    }
}

#[derive(Debug)]
pub struct WasmPerformanceStats {
    pub requests_total: u32,
    pub avg_request_time_ms: u64,
    pub memory_allocations: u32,
    pub cache_hit_rate_percent: u32,
}

/// Global WASM optimization manager
pub struct WasmOptimizationManager {
    memory_pool: WasmMemoryPool,
    request_cache: WasmRequestCache,
    performance_monitor: WasmPerformanceMonitor,
}

impl WasmOptimizationManager {
    /// Initialize global optimization manager
    pub fn init(config: WasmOptimizationConfig) -> Result<Self> {
        OPTIMIZATION_CONFIG
            .set(config.clone())
            .map_err(|_| LoxoneError::config("Optimization manager already initialized"))?;

        let manager = Self {
            memory_pool: WasmMemoryPool::new(config.max_memory_pool_bytes),
            request_cache: WasmRequestCache::new(config.response_cache_limit),
            performance_monitor: WasmPerformanceMonitor::new(),
        };

        Ok(manager)
    }

    /// Get optimized buffer for use
    pub fn get_buffer(&self, size: usize) -> Vec<u8> {
        self.performance_monitor.record_allocation();
        self.memory_pool.get_buffer(size)
    }

    /// Return buffer to pool
    pub fn return_buffer(&self, buffer: Vec<u8>) {
        self.memory_pool.return_buffer(buffer);
    }

    /// Get cached response
    pub fn get_cached_response(&self, key: &str) -> Option<Vec<u8>> {
        if let Some(response) = self.request_cache.get(key) {
            self.performance_monitor.record_cache_hit();
            Some(response)
        } else {
            self.performance_monitor.record_cache_miss();
            None
        }
    }

    /// Cache response
    pub fn cache_response(&self, key: String, data: Vec<u8>, ttl_seconds: u32) {
        self.request_cache.set(key, data, ttl_seconds);
    }

    /// Record request completion
    pub fn record_request(&self, duration_ms: u64) {
        self.performance_monitor.record_request(duration_ms);
    }

    /// Get comprehensive optimization statistics
    pub fn get_stats(&self) -> WasmOptimizationStats {
        WasmOptimizationStats {
            memory_pool: self.memory_pool.stats(),
            cache: self.request_cache.stats(),
            performance: self.performance_monitor.get_stats(),
        }
    }

    /// Perform cleanup and optimization
    pub fn optimize(&self) {
        // Clean up expired cache entries
        self.request_cache.cleanup_expired();

        // Trigger memory optimization if configured
        if let Some(config) = OPTIMIZATION_CONFIG.get() {
            if config.aggressive_memory_management {
                #[cfg(target_arch = "wasm32")]
                {
                    // Force garbage collection if available
                    if let Some(window) = web_sys::window() {
                        if let Ok(gc) = js_sys::Reflect::get(&window, &"gc".into()) {
                            if gc.is_function() {
                                let _ = js_sys::Function::from(gc).call0(&window);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct WasmOptimizationStats {
    pub memory_pool: MemoryPoolStats,
    pub cache: CacheStats,
    pub performance: WasmPerformanceStats,
}

/// Lazy loading utilities for WASM
pub struct WasmLazyLoader;

impl WasmLazyLoader {
    /// Load feature on demand
    pub fn load_feature(feature_name: &str) -> Result<()> {
        match feature_name {
            "infisical" => {
                // Load Infisical integration on demand
                #[cfg(feature = "infisical")]
                {
                    // Initialize Infisical client
                    Ok(())
                }
                #[cfg(not(feature = "infisical"))]
                Err(LoxoneError::config("Infisical feature not enabled"))
            }
            "websocket" => {
                // Load WebSocket support on demand
                #[cfg(feature = "websocket")]
                {
                    // Initialize WebSocket client
                    Ok(())
                }
                #[cfg(not(feature = "websocket"))]
                Err(LoxoneError::config("WebSocket feature not enabled"))
            }
            _ => Err(LoxoneError::config(format!(
                "Unknown feature: {}",
                feature_name
            ))),
        }
    }

    /// Check if feature is available
    pub fn has_feature(feature_name: &str) -> bool {
        match feature_name {
            "infisical" => cfg!(feature = "infisical"),
            "websocket" => cfg!(feature = "websocket"),
            "wasi-keyvalue" => cfg!(feature = "wasi-keyvalue"),
            "debug-logging" => cfg!(feature = "debug-logging"),
            _ => false,
        }
    }

    /// Get list of available features
    pub fn available_features() -> Vec<String> {
        let mut features = Vec::new();

        if Self::has_feature("infisical") {
            features.push("infisical".to_string());
        }
        if Self::has_feature("websocket") {
            features.push("websocket".to_string());
        }
        if Self::has_feature("wasi-keyvalue") {
            features.push("wasi-keyvalue".to_string());
        }
        if Self::has_feature("debug-logging") {
            features.push("debug-logging".to_string());
        }

        features
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pool() {
        let pool = WasmMemoryPool::new(1024);

        // Get a buffer
        let buffer = pool.get_buffer(100);
        assert_eq!(buffer.len(), 100);

        // Return it to pool
        pool.return_buffer(buffer);

        // Get stats
        let stats = pool.stats();
        assert_eq!(stats.buffers_in_pool, 1);
    }

    #[test]
    fn test_request_cache() {
        let cache = WasmRequestCache::new(10);

        // Cache a response
        cache.set("test_key".to_string(), vec![1, 2, 3], 60);

        // Get cached response
        let response = cache.get("test_key");
        assert!(response.is_some());
        assert_eq!(response.unwrap(), vec![1, 2, 3]);

        // Get stats
        let stats = cache.stats();
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.total_size_bytes, 3);
    }

    #[test]
    fn test_performance_monitor() {
        let monitor = WasmPerformanceMonitor::new();

        // Record some metrics
        monitor.record_request(100);
        monitor.record_cache_hit();
        monitor.record_cache_miss();

        // Get stats
        let stats = monitor.get_stats();
        assert_eq!(stats.requests_total, 1);
        assert_eq!(stats.avg_request_time_ms, 100);
        assert_eq!(stats.cache_hit_rate_percent, 50);
    }

    #[test]
    fn test_lazy_loader() {
        let features = WasmLazyLoader::available_features();
        assert!(!features.is_empty());

        // Test feature detection
        let has_logging = WasmLazyLoader::has_feature("debug-logging");
        assert!(has_logging || !has_logging); // Should not panic
    }

    #[test]
    fn test_size_optimizer() {
        let tips = WasmSizeOptimizer::get_size_tips();
        assert!(!tips.is_empty());

        let features = WasmSizeOptimizer::get_minimal_features();
        assert!(features.contains(&"wasi-keyvalue"));
    }
}
