//! MCP tool implementations for Loxone device control and monitoring
//!
//! This module contains all 30+ MCP tools that provide device control,
//! room management, sensor monitoring, and system capabilities.

pub mod audio;
pub mod climate;
pub mod devices;
pub mod documentation;
pub mod energy;
pub mod lighting;
pub mod rolladen;
pub mod rooms;
pub mod security;
pub mod sensors;
pub mod sensors_unified;
pub mod value_helpers;
pub mod weather;
pub mod workflows;

use crate::client::{ClientContext, LoxoneClient};
use crate::error::{LoxoneError, Result};
// use rmcp::tool; // TODO: Re-enable when rmcp API is clarified
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Standard MCP tool response format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    /// Status of the operation
    pub status: String,

    /// Response data
    pub data: serde_json::Value,

    /// Optional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ToolResponse {
    /// Create successful response
    pub fn success(data: serde_json::Value) -> Self {
        Self {
            status: "success".to_string(),
            data,
            message: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create successful response with message
    pub fn success_with_message(data: serde_json::Value, message: String) -> Self {
        Self {
            status: "success".to_string(),
            data,
            message: Some(message),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create error response
    pub fn error(message: String) -> Self {
        Self {
            status: "error".to_string(),
            data: serde_json::Value::Null,
            message: Some(message),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create response from Result
    pub fn from_result<T: Serialize>(result: Result<T>) -> Self {
        match result {
            Ok(data) => {
                let json_data = serde_json::to_value(data).unwrap_or(serde_json::Value::Null);
                Self::success(json_data)
            }
            Err(e) => Self::error(e.to_string()),
        }
    }

    /// Create empty response (optimized for MCP best practices)
    pub fn empty() -> Self {
        Self {
            status: "success".to_string(),
            data: serde_json::json!([]),
            message: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create empty response with context message
    pub fn empty_with_context(context: &str) -> Self {
        Self {
            status: "success".to_string(),
            data: serde_json::json!({
                "context": context,
                "items": [],
                "count": 0
            }),
            message: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create not found response (optimized for MCP best practices)
    pub fn not_found(identifier: &str, suggestion: Option<&str>) -> Self {
        Self {
            status: "success".to_string(),
            data: serde_json::json!({
                "requested": identifier,
                "found": false,
                "suggestion": suggestion.unwrap_or("Check available items using discovery tools")
            }),
            message: None,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Shared tool context for all MCP tools
#[derive(Clone)]
pub struct ToolContext {
    /// Loxone client for API calls (legacy - prefer services)
    pub client: Arc<dyn LoxoneClient>,

    /// Client context for cached data (legacy - prefer services)
    pub context: Arc<ClientContext>,

    /// Unified value resolver for consistent value parsing
    pub value_resolver: Arc<crate::services::UnifiedValueResolver>,

    /// Centralized state manager with change detection
    pub state_manager: Option<Arc<crate::services::StateManager>>,
}

impl ToolContext {
    /// Create new tool context (legacy - use with_services instead)
    #[deprecated(note = "Use with_services for service-layer architecture")]
    pub fn new(_client: Arc<dyn LoxoneClient>, context: Arc<ClientContext>) -> Self {
        // This will panic in debug builds to encourage migration
        #[cfg(debug_assertions)]
        panic!("Use ToolContext::with_services instead of ::new");
        
        #[cfg(not(debug_assertions))]
        Self {
            client: _client,
            context,
            value_resolver: todo!("Missing value resolver - use with_services"),
            state_manager: None,
        }
    }

    /// Create tool context with unified services (recommended)
    pub fn with_services(
        client: Arc<dyn LoxoneClient>,
        context: Arc<ClientContext>,
        value_resolver: Arc<crate::services::UnifiedValueResolver>,
        state_manager: Option<Arc<crate::services::StateManager>>,
    ) -> Self {
        Self {
            client,
            context,
            value_resolver,
            state_manager,
        }
    }

    /// Create tool context with value resolver (legacy compatibility)
    #[deprecated(note = "Use with_services for full service-layer support")]
    pub fn with_resolver(
        client: Arc<dyn LoxoneClient>,
        context: Arc<ClientContext>,
        value_resolver: Arc<crate::services::UnifiedValueResolver>,
    ) -> Self {
        Self {
            client,
            context,
            value_resolver,
            state_manager: None,
        }
    }

    /// Ensure client is connected
    pub async fn ensure_connected(&self) -> Result<()> {
        if !self.client.is_connected().await? {
            return Err(LoxoneError::connection(
                "Not connected to Loxone Miniserver",
            ));
        }
        Ok(())
    }

    /// Get all devices with optional filtering
    pub async fn get_devices(
        &self,
        filter: Option<DeviceFilter>,
    ) -> Result<Vec<crate::client::LoxoneDevice>> {
        let devices = self.context.devices.read().await;
        let mut result: Vec<_> = devices.values().cloned().collect();

        if let Some(filter) = filter {
            result.retain(|device| filter.matches(device));
        }

        Ok(result)
    }

    /// Find device by name or UUID
    pub async fn find_device(&self, identifier: &str) -> Result<crate::client::LoxoneDevice> {
        self.context
            .get_device(identifier)
            .await?
            .ok_or_else(|| LoxoneError::not_found(format!("Device not found: {}", identifier)))
    }

    /// Send command to device
    pub async fn send_device_command(
        &self,
        uuid: &str,
        command: &str,
    ) -> Result<crate::client::LoxoneResponse> {
        self.ensure_connected().await?;
        self.client.send_command(uuid, command).await
    }

    /// Send commands to multiple devices in parallel
    pub async fn send_parallel_commands(
        &self,
        commands: Vec<(String, String)>,
    ) -> Result<Vec<Result<crate::client::LoxoneResponse>>> {
        use futures::future::join_all;

        self.ensure_connected().await?;

        // Create futures for all commands
        let futures: Vec<_> = commands
            .into_iter()
            .map(|(uuid, command)| {
                let client = self.client.clone();
                async move { client.send_command(&uuid, &command).await }
            })
            .collect();

        // Execute all commands in parallel
        let results = join_all(futures).await;

        Ok(results)
    }
}

/// Device filter for tool queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFilter {
    /// Filter by device type
    pub device_type: Option<String>,

    /// Filter by category
    pub category: Option<String>,

    /// Filter by room
    pub room: Option<String>,

    /// Maximum number of devices to return
    pub limit: Option<usize>,
}

impl DeviceFilter {
    /// Check if device matches filter criteria
    pub fn matches(&self, device: &crate::client::LoxoneDevice) -> bool {
        if let Some(ref device_type) = self.device_type {
            if device.device_type != *device_type {
                return false;
            }
        }

        if let Some(ref category) = self.category {
            if device.category != *category {
                return false;
            }
        }

        if let Some(ref room) = self.room {
            if device.room.as_ref() != Some(room) {
                return false;
            }
        }

        true
    }
}

/// Action aliases for multi-language support
pub struct ActionAliases;

impl ActionAliases {
    /// Get standardized action from user input (supports German/English)
    pub fn normalize_action(action: &str) -> String {
        match action.to_lowercase().as_str() {
            // German -> English mappings
            "hoch" | "rauf" | "öffnen" => "up".to_string(),
            "runter" | "zu" | "schließen" => "down".to_string(),
            "an" | "ein" | "einschalten" => "on".to_string(),
            "aus" | "ab" | "ausschalten" => "off".to_string(),
            "stopp" | "stop" | "halt" => "stop".to_string(),
            "dimmen" => "dim".to_string(),
            "hell" | "bright" => "bright".to_string(),

            // English actions (passthrough)
            "on" | "off" | "up" | "down" | "dim" => action.to_lowercase(),

            // Default passthrough
            _ => action.to_lowercase(),
        }
    }

    /// Get valid actions for device type
    pub fn get_valid_actions(device_type: &str) -> Vec<&'static str> {
        match device_type.to_lowercase().as_str() {
            t if t.contains("light") || t.contains("dimmer") => {
                vec!["on", "off", "dim", "bright"]
            }
            t if t.contains("jalousie") || t.contains("blind") => {
                vec!["up", "down", "stop"]
            }
            t if t.contains("switch") => {
                vec!["on", "off"]
            }
            _ => {
                vec!["on", "off", "up", "down", "stop"]
            }
        }
    }
}

/// Device statistics helper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStats {
    /// Total device count
    pub total_devices: usize,

    /// Devices by category
    pub by_category: HashMap<String, usize>,

    /// Devices by room
    pub by_room: HashMap<String, usize>,

    /// Devices by type
    pub by_type: HashMap<String, usize>,
}

impl DeviceStats {
    /// Calculate statistics from device list
    pub fn from_devices(devices: &[crate::client::LoxoneDevice]) -> Self {
        let mut by_category = HashMap::new();
        let mut by_room = HashMap::new();
        let mut by_type = HashMap::new();

        for device in devices {
            // Count by category
            *by_category.entry(device.category.clone()).or_insert(0) += 1;

            // Count by room
            if let Some(ref room) = device.room {
                *by_room.entry(room.clone()).or_insert(0) += 1;
            }

            // Count by type
            *by_type.entry(device.device_type.clone()).or_insert(0) += 1;
        }

        Self {
            total_devices: devices.len(),
            by_category,
            by_room,
            by_type,
        }
    }
}
