//! State management API endpoints
//!
//! This module provides HTTP endpoints for centralized state management,
//! change detection, and real-time state monitoring.

use crate::server::LoxoneMcpServer;
use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

/// Query parameters for state API endpoints
#[derive(Debug, Deserialize)]
pub struct StateQuery {
    /// Limit number of results
    pub limit: Option<usize>,
    /// Include history
    pub include_history: Option<bool>,
    /// Filter by room
    pub room: Option<String>,
    /// Filter by device type
    pub device_type: Option<String>,
}

/// Get all current device states
pub async fn get_all_states(
    State(server): State<Arc<LoxoneMcpServer>>,
    Query(query): Query<StateQuery>,
) -> Json<Value> {
    if let Some(state_manager) = server.get_state_manager() {
        let all_states = state_manager.get_all_device_states().await;

        // Apply filters
        let filtered_states: Vec<_> = all_states
            .into_iter()
            .filter(|(_, state)| {
                // Room filter
                if let Some(room_filter) = &query.room {
                    if state.room.as_ref() != Some(room_filter) {
                        return false;
                    }
                }

                // Device type filter
                if let Some(type_filter) = &query.device_type {
                    if state.device_type != *type_filter {
                        return false;
                    }
                }

                true
            })
            .take(query.limit.unwrap_or(1000))
            .collect();

        let states_vec: Vec<_> = filtered_states
            .into_iter()
            .map(|(_, state)| state)
            .collect();
        let total_count = states_vec.len();

        Json(json!({
            "states": states_vec,
            "total_devices": total_count,
            "filters_applied": {
                "room": query.room,
                "device_type": query.device_type,
                "limit": query.limit.unwrap_or(1000),
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    } else {
        Json(json!({
            "error": "State manager not available",
            "message": "Centralized state management is not initialized",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

/// Get state for a specific device
pub async fn get_device_state(
    State(server): State<Arc<LoxoneMcpServer>>,
    Path(device_uuid): Path<String>,
    Query(query): Query<StateQuery>,
) -> Json<Value> {
    if let Some(state_manager) = server.get_state_manager() {
        let current_state = state_manager.get_device_state(&device_uuid).await;

        let history = if query.include_history.unwrap_or(false) {
            Some(
                state_manager
                    .get_device_history(&device_uuid, query.limit)
                    .await,
            )
        } else {
            None
        };

        Json(json!({
            "device_uuid": device_uuid,
            "current_state": current_state,
            "history": history,
            "history_included": query.include_history.unwrap_or(false),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    } else {
        Json(json!({
            "error": "State manager not available",
            "device_uuid": device_uuid,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

/// Get change statistics
pub async fn get_change_statistics(State(server): State<Arc<LoxoneMcpServer>>) -> Json<Value> {
    if let Some(state_manager) = server.get_state_manager() {
        let stats = state_manager.get_change_statistics().await;

        Json(json!({
            "change_statistics": stats,
            "analysis": {
                "total_tracked_devices": stats.changes_by_device_type.len(),
                "most_active_room": stats.changes_by_room
                    .iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(room, count)| json!({"room": room, "changes": count})),
                "most_active_device_type": stats.changes_by_device_type
                    .iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(device_type, count)| json!({"type": device_type, "changes": count})),
                "change_frequency": if stats.total_changes > 0 {
                    format!("{:.1} changes/hour", stats.change_rate_per_hour)
                } else {
                    "No changes recorded".to_string()
                },
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    } else {
        Json(json!({
            "error": "State manager not available",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

/// Get recent changes across all devices
pub async fn get_recent_changes(
    State(server): State<Arc<LoxoneMcpServer>>,
    Query(query): Query<StateQuery>,
) -> Json<Value> {
    if let Some(state_manager) = server.get_state_manager() {
        let all_states = state_manager.get_all_device_states().await;
        let limit = query.limit.unwrap_or(100);

        // Collect recent changes from all devices
        let mut all_changes = Vec::new();
        for (uuid, _) in all_states {
            let device_changes = state_manager.get_device_history(&uuid, Some(10)).await;
            all_changes.extend(device_changes);
        }

        // Sort by timestamp (most recent first) and apply limit
        all_changes.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        all_changes.truncate(limit);

        // Apply filters
        let filtered_changes: Vec<_> = all_changes
            .into_iter()
            .filter(|change| {
                // Room filter
                if let Some(room_filter) = &query.room {
                    if change.room.as_ref() != Some(room_filter) {
                        return false;
                    }
                }

                // Device type filter
                if let Some(type_filter) = &query.device_type {
                    if change.device_type != *type_filter {
                        return false;
                    }
                }

                true
            })
            .collect();

        Json(json!({
            "recent_changes": filtered_changes,
            "total_changes": filtered_changes.len(),
            "filters_applied": {
                "room": query.room,
                "device_type": query.device_type,
                "limit": limit,
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    } else {
        Json(json!({
            "error": "State manager not available",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

/// Server-Sent Events endpoint for real-time state changes
pub async fn state_changes_stream(
    State(server): State<Arc<LoxoneMcpServer>>,
    Query(_query): Query<StateQuery>,
) -> Result<String, String> {
    if server.get_state_manager().is_some() {
        Ok("State change streaming not yet implemented - use polling endpoint".to_string())
    } else {
        Err("State manager not available".to_string())
    }
}

/// Server-Sent Events endpoint for device-specific state changes
pub async fn device_state_stream(
    State(server): State<Arc<LoxoneMcpServer>>,
    Path(device_uuid): Path<String>,
) -> Result<String, String> {
    if server.get_state_manager().is_some() {
        Ok(format!(
            "Device state streaming for {} not yet implemented - use polling endpoint",
            device_uuid
        ))
    } else {
        Err("State manager not available".to_string())
    }
}

/// Server-Sent Events endpoint for room-specific state changes
pub async fn room_state_stream(
    State(server): State<Arc<LoxoneMcpServer>>,
    Path(room_name): Path<String>,
) -> Result<String, String> {
    if server.get_state_manager().is_some() {
        Ok(format!(
            "Room state streaming for {} not yet implemented - use polling endpoint",
            room_name
        ))
    } else {
        Err("State manager not available".to_string())
    }
}
