//! Cache monitoring and management API endpoints
//!
//! This module provides HTTP endpoints for monitoring cache performance
//! and managing cache state for better debugging and optimization.

use crate::server::LoxoneMcpServer;
use axum::{extract::State, response::Json};
use serde_json::{json, Value};
use std::sync::Arc;

/// Get cache statistics for monitoring
pub async fn get_cache_stats(State(server): State<Arc<LoxoneMcpServer>>) -> Json<Value> {
    let stats = server.value_resolver.get_cache_statistics().await;

    Json(json!({
        "cache_statistics": {
            "device_cache_size": stats.device_cache_size,
            "batch_cache_size": stats.batch_cache_size,
            "tracked_patterns": stats.tracked_patterns,
            "total_access_count": stats.total_access_count,
        },
        "cache_efficiency": {
            "hit_ratio_estimate": if stats.total_access_count > 0 {
                format!("{:.1}%", ((stats.device_cache_size as f64 / stats.total_access_count as f64) * 100.0).min(100.0))
            } else {
                "N/A".to_string()
            },
            "predictive_patterns": stats.tracked_patterns,
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "recommendations": generate_cache_recommendations(&stats),
    }))
}

/// Clear all caches (for maintenance)
pub async fn clear_caches(State(server): State<Arc<LoxoneMcpServer>>) -> Json<Value> {
    server.value_resolver.clear_caches().await;

    Json(json!({
        "status": "success",
        "message": "All caches cleared successfully",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Get detailed cache performance metrics
pub async fn get_cache_performance(State(server): State<Arc<LoxoneMcpServer>>) -> Json<Value> {
    let stats = server.value_resolver.get_cache_statistics().await;

    // Calculate performance metrics
    let cache_utilization = if stats.device_cache_size > 0 {
        (stats.device_cache_size as f64 / 10000.0) * 100.0 // Assuming max cache size of 10,000
    } else {
        0.0
    };

    let pattern_efficiency = if stats.tracked_patterns > 0 {
        format!("{} co-access patterns tracked", stats.tracked_patterns)
    } else {
        "No patterns detected yet".to_string()
    };

    Json(json!({
        "performance": {
            "cache_utilization_percent": format!("{:.1}%", cache_utilization),
            "total_cached_devices": stats.device_cache_size,
            "batch_cache_entries": stats.batch_cache_size,
            "access_pattern_analysis": pattern_efficiency,
            "estimated_api_call_reduction": estimate_api_reduction(&stats),
        },
        "health": {
            "status": if cache_utilization < 90.0 { "healthy" } else { "approaching_limit" },
            "recommendations": generate_performance_recommendations(&stats, cache_utilization),
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Generate cache optimization recommendations
fn generate_cache_recommendations(
    stats: &crate::services::cache_manager::CacheStatistics,
) -> Vec<String> {
    let mut recommendations = Vec::new();

    if stats.device_cache_size == 0 {
        recommendations
            .push("Cache is empty - consider warming up frequently accessed devices".to_string());
    } else if stats.device_cache_size > 8000 {
        recommendations.push(
            "Cache is near capacity - consider increasing cache size or reducing TTL".to_string(),
        );
    }

    if stats.tracked_patterns == 0 {
        recommendations
            .push("No access patterns detected - predictive prefetching is not active".to_string());
    } else if stats.tracked_patterns > 50 {
        recommendations.push(
            "Many access patterns detected - predictive caching is highly effective".to_string(),
        );
    }

    if stats.total_access_count > 1000 {
        recommendations.push(
            "High cache usage detected - consider monitoring for performance optimization"
                .to_string(),
        );
    }

    if recommendations.is_empty() {
        recommendations.push("Cache performance appears optimal".to_string());
    }

    recommendations
}

/// Generate performance-specific recommendations
fn generate_performance_recommendations(
    stats: &crate::services::cache_manager::CacheStatistics,
    utilization: f64,
) -> Vec<String> {
    let mut recommendations = Vec::new();

    if utilization > 90.0 {
        recommendations.push("Consider increasing cache size to prevent evictions".to_string());
    }

    if stats.batch_cache_size > stats.device_cache_size {
        recommendations
            .push("Batch cache is larger than device cache - this is unusual".to_string());
    }

    if stats.tracked_patterns > 100 {
        recommendations.push(
            "Many patterns tracked - predictive prefetching may be very effective".to_string(),
        );
    } else if stats.tracked_patterns < 5 && stats.total_access_count > 100 {
        recommendations.push("Few patterns despite high usage - check access patterns".to_string());
    }

    if recommendations.is_empty() {
        recommendations.push("Performance metrics are within normal ranges".to_string());
    }

    recommendations
}

/// Estimate API call reduction from caching
fn estimate_api_reduction(stats: &crate::services::cache_manager::CacheStatistics) -> String {
    if stats.total_access_count == 0 {
        return "No data available".to_string();
    }

    // Rough estimation: if we have cached values, we're likely saving API calls
    let estimated_saved_calls = stats.device_cache_size as f64 * 2.5; // Assume each cached value saves ~2.5 API calls
    let reduction_percentage = ((estimated_saved_calls
        / (stats.total_access_count as f64 + estimated_saved_calls))
        * 100.0)
        .min(95.0);

    format!(
        "~{:.0}% reduction (estimated {} API calls saved)",
        reduction_percentage, estimated_saved_calls as u64
    )
}
