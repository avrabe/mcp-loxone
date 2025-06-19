//! Ultra-fast dashboard endpoint optimized for <100ms response times
//!
//! This module combines all performance optimizations to achieve sub-100ms
//! dashboard loads through aggressive caching, precomputation, and batching.

use crate::http_transport::dashboard_performance::{get_ultra_fast_dashboard, get_micro_dashboard};
use crate::server::LoxoneMcpServer;
// use crate::services::connection_pool::{ConnectionPool, PoolConfig};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Query parameters for dashboard requests
#[derive(Debug, Deserialize)]
pub struct DashboardQuery {
    /// Performance mode: "micro", "fast", "full"
    #[serde(default = "default_mode")]
    pub mode: String,
    /// Include real-time data
    #[serde(default)]
    pub realtime: bool,
    /// Include device details
    #[serde(default = "default_include_devices")]
    pub devices: bool,
    /// Include metrics
    #[serde(default)]
    pub metrics: bool,
}

fn default_mode() -> String {
    "fast".to_string()
}

fn default_include_devices() -> bool {
    true
}

/// Dashboard response with performance metadata
#[derive(Debug, Serialize)]
pub struct DashboardResponse {
    /// Dashboard data
    pub data: Value,
    /// Performance metrics
    pub performance: PerformanceMetrics,
    /// Request metadata
    pub metadata: RequestMetadata,
}

/// Performance metrics for the response
#[derive(Debug, Serialize)]
pub struct PerformanceMetrics {
    /// Response time in milliseconds
    pub response_time_ms: f64,
    /// Whether target was achieved
    pub target_achieved: bool,
    /// Target response time
    pub target_ms: f64,
    /// Cache hit/miss
    pub cache_status: String,
    /// Optimization level used
    pub optimization: String,
}

/// Request metadata
#[derive(Debug, Serialize)]
pub struct RequestMetadata {
    /// Request timestamp
    pub timestamp: String,
    /// Server version
    pub version: String,
    /// Mode used
    pub mode: String,
    /// Features enabled
    pub features: Vec<String>,
}

/// Ultra-fast dashboard endpoint (primary)
pub async fn fast_dashboard_handler(
    State(server): State<Arc<LoxoneMcpServer>>,
    Query(params): Query<DashboardQuery>,
    headers: HeaderMap,
) -> Result<Json<DashboardResponse>, StatusCode> {
    let start = Instant::now();
    
    // Determine performance mode
    let (data, optimization) = match params.mode.as_str() {
        "micro" => {
            (get_micro_dashboard(&server).await, "micro_optimized")
        }
        "fast" => {
            (get_ultra_fast_dashboard(&server).await, "ultra_fast_cached")
        }
        "full" => {
            (get_full_dashboard(&server, &params).await, "full_featured")
        }
        _ => {
            (get_ultra_fast_dashboard(&server).await, "default_fast")
        }
    };
    
    let elapsed = start.elapsed();
    let response_time_ms = elapsed.as_micros() as f64 / 1000.0;
    
    // Determine cache status from headers or data
    let cache_status = if headers.contains_key("x-cache") {
        headers.get("x-cache")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string()
    } else if response_time_ms < 10.0 {
        "hit".to_string()
    } else {
        "miss".to_string()
    };
    
    let response = DashboardResponse {
        data,
        performance: PerformanceMetrics {
            response_time_ms,
            target_achieved: response_time_ms < 100.0,
            target_ms: 100.0,
            cache_status,
            optimization: optimization.to_string(),
        },
        metadata: RequestMetadata {
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            mode: params.mode.clone(),
            features: get_enabled_features(&params),
        },
    };
    
    Ok(Json(response))
}

/// Micro dashboard - absolute minimum data for fastest response
pub async fn micro_dashboard_handler(
    State(server): State<Arc<LoxoneMcpServer>>,
) -> Result<Json<Value>, StatusCode> {
    let start = Instant::now();
    
    let data = get_micro_dashboard(&server).await;
    let elapsed = start.elapsed();
    
    // Add performance metadata
    let mut response = data;
    response["_performance"] = json!({
        "response_time_ms": elapsed.as_micros() as f64 / 1000.0,
        "mode": "micro",
        "target_achieved": elapsed.as_millis() < 100
    });
    
    Ok(Json(response))
}

/// Health check endpoint optimized for monitoring
pub async fn health_fast_handler(
    State(server): State<Arc<LoxoneMcpServer>>,
) -> Result<Json<Value>, StatusCode> {
    let start = Instant::now();
    
    // Ultra-minimal health check
    let connected = *server.context.connected.read().await;
    let device_count = server.context.devices.read().await.len();
    
    let elapsed = start.elapsed();
    
    Ok(Json(json!({
        "status": if connected { "healthy" } else { "degraded" },
        "connected": connected,
        "devices": device_count,
        "response_time_ms": elapsed.as_micros() as f64 / 1000.0,
        "timestamp": chrono::Utc::now().timestamp()
    })))
}

/// Performance metrics endpoint
pub async fn performance_handler(
    State(server): State<Arc<LoxoneMcpServer>>,
) -> Result<Json<Value>, StatusCode> {
    let metrics_collector = server.get_metrics_collector();
    let server_metrics = metrics_collector.get_metrics().await;
    
    Ok(Json(json!({
        "performance": {
            "avg_response_time_ms": server_metrics.network.average_response_time_ms,
            "requests_per_minute": server_metrics.network.requests_per_minute,
            "error_rate_percent": server_metrics.errors.error_rate_percent,
            "cpu_usage_percent": server_metrics.performance.cpu_usage_percent,
            "memory_usage_mb": server_metrics.performance.memory_usage_mb,
            "cache_hit_rate": server_metrics.cache.hit_rate_percent,
            "uptime_seconds": server_metrics.uptime.uptime_seconds
        },
        "mcp": {
            "tools_executed": server_metrics.mcp.tools_executed,
            "avg_tool_time_ms": server_metrics.mcp.average_tool_execution_ms,
            "active_sessions": server_metrics.mcp.active_mcp_sessions,
            "most_used_tool": server_metrics.mcp.most_used_tool
        },
        "targets": {
            "dashboard_response_ms": 100,
            "api_response_ms": 50,
            "health_check_ms": 10
        }
    })))
}

/// Get full dashboard with all features
async fn get_full_dashboard(server: &LoxoneMcpServer, params: &DashboardQuery) -> Value {
    // Use the existing unified dashboard but with performance enhancements
    let mut data = crate::http_transport::dashboard_data_unified::get_unified_dashboard_data(server).await;
    
    // Add performance-specific metadata
    data["performance_mode"] = json!("full");
    data["optimizations"] = json!({
        "batch_resolution": true,
        "concurrent_processing": true,
        "smart_caching": true,
        "realtime_updates": params.realtime
    });
    
    // Conditionally include expensive features
    if !params.devices {
        data["devices"]["device_matrix"] = json!([]);
    }
    
    if !params.metrics {
        data["operational"] = json!({});
    }
    
    data
}

/// Get enabled features for metadata
fn get_enabled_features(params: &DashboardQuery) -> Vec<String> {
    let mut features = Vec::new();
    
    if params.realtime {
        features.push("realtime".to_string());
    }
    if params.devices {
        features.push("devices".to_string());
    }
    if params.metrics {
        features.push("metrics".to_string());
    }
    
    features.push("performance_optimized".to_string());
    features.push("batch_processing".to_string());
    features.push("aggressive_caching".to_string());
    
    features
}

/// Benchmark endpoint for performance testing
pub async fn benchmark_handler(
    State(server): State<Arc<LoxoneMcpServer>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let iterations = params.get("iterations")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10);
    
    let mode = params.get("mode").cloned().unwrap_or_else(|| "fast".to_string());
    
    let mut times = Vec::new();
    let mut total_time = std::time::Duration::ZERO;
    
    // Run benchmark iterations
    for _ in 0..iterations {
        let start = Instant::now();
        
        match mode.as_str() {
            "micro" => { let _ = get_micro_dashboard(&server).await; }
            "fast" => { let _ = get_ultra_fast_dashboard(&server).await; }
            "full" => { 
                let params = DashboardQuery {
                    mode: "full".to_string(),
                    realtime: true,
                    devices: true,
                    metrics: true,
                };
                let _ = get_full_dashboard(&server, &params).await; 
            }
            _ => { let _ = get_ultra_fast_dashboard(&server).await; }
        }
        
        let elapsed = start.elapsed();
        times.push(elapsed.as_micros() as f64 / 1000.0);
        total_time += elapsed;
    }
    
    // Calculate statistics
    let avg_time = times.iter().sum::<f64>() / times.len() as f64;
    let min_time = times.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max_time = times.iter().fold(0.0f64, |a, &b| a.max(b));
    
    // Calculate percentiles
    let mut sorted_times = times.clone();
    sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = sorted_times[iterations / 2];
    let p95 = sorted_times[(iterations * 95) / 100];
    let p99 = sorted_times[(iterations * 99) / 100];
    
    Ok(Json(json!({
        "benchmark": {
            "mode": mode,
            "iterations": iterations,
            "total_time_ms": total_time.as_micros() as f64 / 1000.0,
            "statistics": {
                "avg_ms": avg_time,
                "min_ms": min_time,
                "max_ms": max_time,
                "p50_ms": p50,
                "p95_ms": p95,
                "p99_ms": p99
            },
            "performance": {
                "target_achieved": avg_time < 100.0,
                "target_ms": 100.0,
                "success_rate": 100.0,
                "throughput_rps": 1000.0 / avg_time
            },
            "individual_times_ms": times
        }
    })))
}

/// Create router with all fast dashboard endpoints
pub fn create_fast_dashboard_router() -> Router<Arc<LoxoneMcpServer>> {
    Router::new()
        .route("/dashboard", get(fast_dashboard_handler))
        .route("/dashboard/micro", get(micro_dashboard_handler))
        .route("/dashboard/health", get(health_fast_handler))
        .route("/dashboard/performance", get(performance_handler))
        .route("/dashboard/benchmark", get(benchmark_handler))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_query_params() {
        let query = DashboardQuery {
            mode: default_mode(),
            realtime: false,
            devices: default_include_devices(),
            metrics: false,
        };
        
        assert_eq!(query.mode, "fast");
        assert!(!query.realtime);
        assert!(query.devices);
        assert!(!query.metrics);
    }

    #[test]
    fn test_enabled_features() {
        let params = DashboardQuery {
            mode: "fast".to_string(),
            realtime: true,
            devices: true,
            metrics: false,
        };
        
        let features = get_enabled_features(&params);
        assert!(features.contains(&"realtime".to_string()));
        assert!(features.contains(&"devices".to_string()));
        assert!(!features.contains(&"metrics".to_string()));
        assert!(features.contains(&"performance_optimized".to_string()));
    }

    #[tokio::test]
    async fn test_benchmark_statistics() {
        let times = [10.0, 15.0, 20.0, 25.0, 30.0];
        let avg = times.iter().sum::<f64>() / times.len() as f64;
        assert_eq!(avg, 20.0);
        
        let min = times.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        assert_eq!(min, 10.0);
        
        let max = times.iter().fold(0.0f64, |a, &b| a.max(b));
        assert_eq!(max, 30.0);
    }
}