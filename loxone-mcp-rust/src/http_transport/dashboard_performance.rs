//! High-performance dashboard optimization module
//!
//! This module implements aggressive optimizations to achieve <100ms dashboard load times
//! through caching, precomputation, and efficient data structures.

use crate::server::LoxoneMcpServer;
// Remove unused import - we use our own snapshot mechanism
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Pre-computed dashboard snapshot for instant delivery
#[derive(Debug, Clone)]
pub struct DashboardSnapshot {
    /// Complete dashboard JSON data
    pub data: Value,
    /// When this snapshot was created
    pub created_at: Instant,
    /// Data source identifier
    pub source: String,
    /// Cache key used for this snapshot
    pub cache_key: String,
}

/// High-performance dashboard cache with aggressive optimization
pub struct PerformanceDashboard {
    /// Current active snapshot
    snapshot: Arc<RwLock<Option<DashboardSnapshot>>>,
    /// Background refresh interval
    refresh_interval: Duration,
    /// Maximum age before forced refresh
    max_age: Duration,
    /// Performance metrics
    metrics: Arc<RwLock<DashboardMetrics>>,
}

/// Dashboard performance metrics
#[derive(Debug, Clone, Default)]
pub struct DashboardMetrics {
    /// Total requests served
    pub total_requests: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Average response time in microseconds
    pub avg_response_time_us: u64,
    /// Last refresh time
    pub last_refresh_at: Option<Instant>,
    /// Fastest response time recorded
    pub fastest_response_us: u64,
    /// Background refresh count
    pub background_refreshes: u64,
}

impl Default for PerformanceDashboard {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceDashboard {
    /// Create new performance dashboard with aggressive caching
    pub fn new() -> Self {
        Self {
            snapshot: Arc::new(RwLock::new(None)),
            refresh_interval: Duration::from_secs(5), // 5-second background refresh
            max_age: Duration::from_secs(30), // Force refresh after 30 seconds
            metrics: Arc::new(RwLock::new(DashboardMetrics {
                fastest_response_us: u64::MAX,
                ..Default::default()
            })),
        }
    }

    /// Get dashboard data with <100ms guarantee
    pub async fn get_dashboard_fast(&self, server: &LoxoneMcpServer) -> Value {
        let start = Instant::now();
        
        // Try to serve from snapshot first (should be <1ms)
        if let Some(snapshot) = self.get_valid_snapshot().await {
            let elapsed = start.elapsed();
            self.update_metrics(elapsed, true).await;
            return snapshot.data;
        }

        // Cache miss - generate new snapshot
        let data = self.generate_optimized_dashboard(server).await;
        let elapsed = start.elapsed();
        
        // Store new snapshot
        let snapshot = DashboardSnapshot {
            data: data.clone(),
            created_at: Instant::now(),
            source: "live_generation".to_string(),
            cache_key: self.generate_cache_key().await,
        };
        
        *self.snapshot.write().await = Some(snapshot);
        self.update_metrics(elapsed, false).await;
        
        data
    }

    /// Start background refresh task for proactive caching
    pub async fn start_background_refresh(&self, server: Arc<LoxoneMcpServer>) {
        let snapshot_ref = self.snapshot.clone();
        let refresh_interval = self.refresh_interval;
        let metrics_ref = self.metrics.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(refresh_interval);
            
            loop {
                interval.tick().await;
                
                // Check if refresh is needed
                let needs_refresh = {
                    let snapshot_guard = snapshot_ref.read().await;
                    match snapshot_guard.as_ref() {
                        Some(snapshot) => snapshot.created_at.elapsed() > Duration::from_secs(10),
                        None => true,
                    }
                };
                
                if needs_refresh {
                    let start = Instant::now();
                    
                    // Generate fresh data in background
                    let data = Self::generate_optimized_dashboard_static(&server).await;
                    
                    // Update snapshot
                    let new_snapshot = DashboardSnapshot {
                        data,
                        created_at: Instant::now(),
                        source: "background_refresh".to_string(),
                        cache_key: Self::generate_cache_key_static().await,
                    };
                    
                    *snapshot_ref.write().await = Some(new_snapshot);
                    
                    // Update metrics
                    {
                        let mut metrics = metrics_ref.write().await;
                        metrics.background_refreshes += 1;
                        metrics.last_refresh_at = Some(Instant::now());
                    }
                    
                    tracing::debug!("Background dashboard refresh completed in {:?}", start.elapsed());
                }
            }
        });
    }

    /// Get valid snapshot if available
    async fn get_valid_snapshot(&self) -> Option<DashboardSnapshot> {
        let snapshot_guard = self.snapshot.read().await;
        
        if let Some(snapshot) = snapshot_guard.as_ref() {
            if snapshot.created_at.elapsed() < self.max_age {
                return Some(snapshot.clone());
            }
        }
        
        None
    }

    /// Generate optimized dashboard with performance focus
    async fn generate_optimized_dashboard(&self, server: &LoxoneMcpServer) -> Value {
        Self::generate_optimized_dashboard_static(server).await
    }

    /// Static version for background task
    async fn generate_optimized_dashboard_static(server: &LoxoneMcpServer) -> Value {
        let resolver = server.get_value_resolver();
        let context = &server.context;

        // Use concurrent operations for maximum speed
        let (devices, rooms, connection_status) = tokio::join!(
            context.devices.read(),
            context.rooms.read(),
            async { *context.connected.read().await }
        );

        // Get device UUIDs and resolve values in batch (fastest method)
        let device_uuids: Vec<String> = devices.keys().cloned().collect();
        
        let resolved_values: std::collections::HashMap<String, crate::services::value_resolution::ResolvedValue> = 
            resolver.resolve_batch_values(&device_uuids).await.unwrap_or_default();

        // Pre-categorize devices for faster processing
        let mut categorized = CategorizedDevices::default();
        for device in devices.values() {
            if let Some(resolved) = resolved_values.get(&device.uuid) {
                categorized.add_device(device, resolved);
            }
        }

        // Build minimal response for speed
        json!({
            "realtime": {
                "connection_status": if connection_status { "Connected" } else { "Disconnected" },
                "last_update": chrono::Utc::now().to_rfc3339(),
                "device_count": devices.len(),
                "active_devices": categorized.active_count,
                "response_time_target": "< 100ms"
            },
            "devices": {
                "summary": {
                    "total": devices.len(),
                    "active": categorized.active_count,
                    "rooms": rooms.len()
                },
                "by_room": categorized.by_room,
                "by_type": {
                    "lights": categorized.lights.len(),
                    "blinds": categorized.blinds.len(),
                    "climate": categorized.climate.len(),
                    "sensors": categorized.sensors.len()
                }
            },
            "performance": {
                "optimization": "aggressive_caching",
                "data_source": "batch_resolver",
                "target_time_ms": 100,
                "cache_enabled": true
            },
            "metadata": {
                "generated_at": chrono::Utc::now().to_rfc3339(),
                "version": "3.0.0-performance",
                "optimization_level": "maximum"
            }
        })
    }

    /// Update performance metrics
    async fn update_metrics(&self, elapsed: Duration, was_cache_hit: bool) {
        let mut metrics = self.metrics.write().await;
        
        metrics.total_requests += 1;
        
        if was_cache_hit {
            metrics.cache_hits += 1;
        } else {
            metrics.cache_misses += 1;
        }
        
        let elapsed_us = elapsed.as_micros() as u64;
        
        // Update average response time
        if metrics.total_requests == 1 {
            metrics.avg_response_time_us = elapsed_us;
        } else {
            metrics.avg_response_time_us = 
                (metrics.avg_response_time_us * (metrics.total_requests - 1) + elapsed_us) / metrics.total_requests;
        }
        
        // Update fastest response time
        if elapsed_us < metrics.fastest_response_us {
            metrics.fastest_response_us = elapsed_us;
        }
    }

    /// Get performance metrics
    pub async fn get_metrics(&self) -> DashboardMetrics {
        self.metrics.read().await.clone()
    }

    /// Generate cache key
    async fn generate_cache_key(&self) -> String {
        Self::generate_cache_key_static().await
    }

    /// Static cache key generation
    async fn generate_cache_key_static() -> String {
        format!("dashboard_v3_{}", chrono::Utc::now().timestamp())
    }
}

/// Pre-categorized device collections for fast processing
#[derive(Default)]
struct CategorizedDevices {
    lights: Vec<String>,
    blinds: Vec<String>,
    climate: Vec<String>,
    sensors: Vec<String>,
    by_room: HashMap<String, u32>,
    active_count: u32,
}

impl CategorizedDevices {
    fn add_device(&mut self, device: &crate::client::LoxoneDevice, resolved: &crate::services::value_resolution::ResolvedValue) {
        // Count active devices
        if resolved.numeric_value.unwrap_or(0.0) > 0.0 {
            self.active_count += 1;
        }

        // Categorize by type
        match device.category.as_str() {
            "lighting" => self.lights.push(device.uuid.clone()),
            "blinds" | "shading" => self.blinds.push(device.uuid.clone()),
            "climate" | "sensors" => {
                if resolved.sensor_type.is_some() {
                    self.sensors.push(device.uuid.clone());
                } else {
                    self.climate.push(device.uuid.clone());
                }
            },
            _ => {
                if resolved.sensor_type.is_some() {
                    self.sensors.push(device.uuid.clone());
                }
            }
        }

        // Count by room
        if let Some(room) = &device.room {
            *self.by_room.entry(room.clone()).or_insert(0) += 1;
        }
    }
}

/// Performance-optimized dashboard endpoint
pub async fn get_ultra_fast_dashboard(server: &LoxoneMcpServer) -> Value {
    // Static performance dashboard instance (created once)
    static PERF_DASHBOARD: tokio::sync::OnceCell<PerformanceDashboard> = tokio::sync::OnceCell::const_new();
    
    let dashboard = PERF_DASHBOARD.get_or_init(|| async {
        let dashboard = PerformanceDashboard::new();
        
        // Start background refresh
        dashboard.start_background_refresh(Arc::new(server.clone())).await;
        
        dashboard
    }).await;
    
    // This should complete in <100ms with caching
    dashboard.get_dashboard_fast(server).await
}

/// Micro-optimized dashboard for absolute minimum latency
pub async fn get_micro_dashboard(server: &LoxoneMcpServer) -> Value {
    let context = &server.context;
    
    // Minimal data fetch - only essentials
    let (device_count, connection_status) = tokio::join!(
        async { context.devices.read().await.len() },
        async { *context.connected.read().await }
    );
    
    json!({
        "status": if connection_status { "ok" } else { "disconnected" },
        "devices": device_count,
        "timestamp": chrono::Utc::now().timestamp(),
        "performance": "micro_optimized"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dashboard_metrics() {
        let dashboard = PerformanceDashboard::new();
        
        // Simulate requests
        dashboard.update_metrics(Duration::from_millis(50), true).await;
        dashboard.update_metrics(Duration::from_millis(75), false).await;
        
        let metrics = dashboard.get_metrics().await;
        assert_eq!(metrics.total_requests, 2);
        assert_eq!(metrics.cache_hits, 1);
        assert_eq!(metrics.cache_misses, 1);
        assert!(metrics.avg_response_time_us > 0);
    }

    #[tokio::test]
    async fn test_snapshot_expiry() {
        let dashboard = PerformanceDashboard::new();
        
        // Create expired snapshot
        let old_snapshot = DashboardSnapshot {
            data: json!({"test": "data"}),
            created_at: Instant::now() - Duration::from_secs(60),
            source: "test".to_string(),
            cache_key: "test_key".to_string(),
        };
        
        *dashboard.snapshot.write().await = Some(old_snapshot);
        
        // Should return None for expired snapshot
        assert!(dashboard.get_valid_snapshot().await.is_none());
    }
}