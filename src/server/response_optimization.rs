//! Response optimization utilities for MCP best practices
//!
//! This module provides utilities to optimize response text according to MCP best practices,
//! returning empty results instead of "not found" error messages for better user experience.


// Use framework types instead of legacy mcp_foundation
use pulseengine_mcp_protocol::{CallToolResult, Content};
use serde_json::json;

/// Standard empty response patterns for different scenarios
pub struct OptimizedResponses;


impl OptimizedResponses {
    /// Create an empty room list response
    pub fn empty_rooms() -> CallToolResult {
        let result = json!({
            "total_rooms": 0,
            "rooms": [],
            "note": "No rooms match the current criteria"
        });
        CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "[]".to_string()),
        )])
    }

    /// Create an empty devices response for a room
    pub fn empty_room_devices(room_name: &str) -> CallToolResult {
        let result = json!({
            "room": room_name,
            "device_count": 0,
            "devices": []
        });
        CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()),
        )])
    }

    /// Create an empty devices response for system-wide queries
    pub fn empty_devices() -> CallToolResult {
        let result = json!({
            "total_devices": 0,
            "devices": []
        });
        CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()),
        )])
    }

    /// Create an empty lights response
    pub fn empty_lights(context: Option<&str>) -> CallToolResult {
        let result = if let Some(ctx) = context {
            json!({
                "context": ctx,
                "lights_count": 0,
                "lights_controlled": 0,
                "results": []
            })
        } else {
            json!({
                "lights_count": 0,
                "lights_controlled": 0,
                "results": []
            })
        };
        CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()),
        )])
    }

    /// Create an empty blinds/rolladen response
    pub fn empty_blinds(context: Option<&str>) -> CallToolResult {
        let result = if let Some(ctx) = context {
            json!({
                "context": ctx,
                "blinds_count": 0,
                "blinds_controlled": 0,
                "results": []
            })
        } else {
            json!({
                "blinds_count": 0,
                "blinds_controlled": 0,
                "results": []
            })
        };
        CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()),
        )])
    }

    /// Create an empty devices by type response
    pub fn empty_devices_by_type(device_type: Option<&str>) -> CallToolResult {
        let result = if let Some(dtype) = device_type {
            json!({
                "filter": dtype,
                "count": 0,
                "devices": []
            })
        } else {
            json!({
                "available_types": [],
                "note": "No device types found in the system"
            })
        };
        CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()),
        )])
    }

    /// Create an empty audio zones response
    pub fn empty_audio_zones() -> CallToolResult {
        let result = json!({
            "total_zones": 0,
            "zones": []
        });
        CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()),
        )])
    }

    /// Create an empty sensors response
    pub fn empty_sensors() -> CallToolResult {
        let result = json!({
            "total_sensors": 0,
            "sensors": []
        });
        CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()),
        )])
    }

    /// Create a device not found response (returns empty device info rather than error)
    pub fn device_not_found(device_identifier: &str) -> CallToolResult {
        let result = json!({
            "device_requested": device_identifier,
            "found": false,
            "suggestion": "Use discover_all_devices or get_devices_by_type to find available devices"
        });
        CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()),
        )])
    }

    /// Create a room not found response (returns empty room info rather than error)
    pub fn room_not_found(room_name: &str) -> CallToolResult {
        let result = json!({
            "room_requested": room_name,
            "found": false,
            "suggestion": "Use list_rooms to see available rooms"
        });
        CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()),
        )])
    }

    /// Create a successful operation response with empty affected items
    pub fn empty_operation_result(operation: &str, context: Option<&str>) -> CallToolResult {
        let result = if let Some(ctx) = context {
            json!({
                "operation": operation,
                "context": ctx,
                "items_affected": 0,
                "items_processed": [],
                "status": "completed"
            })
        } else {
            json!({
                "operation": operation,
                "items_affected": 0,
                "items_processed": [],
                "status": "completed"
            })
        };
        CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()),
        )])
    }
}

/// Response optimization helper functions

pub trait ResponseOptimizer {
    /// Convert a potential error response to an optimized empty response
    fn optimize_empty_result(self) -> CallToolResult;

    /// Convert a not found error to an optimized suggestion response
    fn optimize_not_found(self, identifier: &str, suggestion: Option<&str>) -> CallToolResult;
}


impl ResponseOptimizer for Result<CallToolResult, pulseengine_mcp_protocol::Error> {
    fn optimize_empty_result(self) -> CallToolResult {
        match self {
            Ok(result) => result,
            Err(_) => OptimizedResponses::empty_devices(),
        }
    }

    fn optimize_not_found(self, identifier: &str, suggestion: Option<&str>) -> CallToolResult {
        match self {
            Ok(result) => result,
            Err(_) => {
                let result = json!({
                    "requested": identifier,
                    "found": false,
                    "suggestion": suggestion.unwrap_or("Check available items using discovery tools")
                });
                CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()),
                )])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_rooms_response() {
        let response = OptimizedResponses::empty_rooms();
        assert!(!response.is_error.unwrap_or(false));
        assert_eq!(response.content.len(), 1);
    }

    #[test]
    fn test_empty_room_devices_response() {
        let response = OptimizedResponses::empty_room_devices("Kitchen");
        assert!(!response.is_error.unwrap_or(false));
        assert_eq!(response.content.len(), 1);
    }

    #[test]
    fn test_device_not_found_response() {
        let response = OptimizedResponses::device_not_found("invalid-device");
        assert!(!response.is_error.unwrap_or(false));
        assert_eq!(response.content.len(), 1);
    }

    #[test]
    fn test_room_not_found_response() {
        let response = OptimizedResponses::room_not_found("Invalid Room");
        assert!(!response.is_error.unwrap_or(false));
        assert_eq!(response.content.len(), 1);
    }

    #[test]
    fn test_empty_operation_result() {
        let response =
            OptimizedResponses::empty_operation_result("control_lights", Some("Living Room"));
        assert!(!response.is_error.unwrap_or(false));
        assert_eq!(response.content.len(), 1);
    }
}
