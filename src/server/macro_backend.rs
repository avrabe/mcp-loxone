//! Macro-based MCP server implementation
//!
//! This module uses pulseengine-mcp-macros 0.17.0 to dramatically simplify
//! tool and resource definitions. The macros auto-generate:
//! - Tool registration and discovery
//! - JSON schema generation from Rust types
//! - Parameter validation
//! - Error handling

use crate::client::{ClientContext, LoxoneClient, LoxoneStructure};
use crate::config::ServerConfig;
use crate::services::{StateManager, UnifiedValueResolver};
use pulseengine_mcp_macros::{mcp_server, mcp_tools};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{info, warn};

/// Loxone MCP Server with macro-based tool definitions
///
/// This struct holds the context needed for tool execution and uses
/// the `#[mcp_server]` macro for automatic backend generation.
#[mcp_server(
    name = "Loxone MCP Server",
    description = "High-performance MCP server for Loxone home automation"
)]
#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct LoxoneMcpServer {
    /// Loxone client for API calls
    client: Option<Arc<dyn LoxoneClient>>,
    /// Client context for cached data
    context: Option<Arc<ClientContext>>,
    /// Unified value resolver (for future use)
    value_resolver: Option<Arc<UnifiedValueResolver>>,
    /// State manager for change detection (for future use)
    state_manager: Option<Arc<StateManager>>,
    /// Server configuration (for future use)
    config: Option<ServerConfig>,
}

impl LoxoneMcpServer {
    /// Create a new Loxone MCP server with all dependencies
    pub fn with_context(
        client: Arc<dyn LoxoneClient>,
        context: Arc<ClientContext>,
        value_resolver: Arc<UnifiedValueResolver>,
        state_manager: Option<Arc<StateManager>>,
        config: ServerConfig,
    ) -> Self {
        info!("Initializing Loxone MCP Server with macro-based tools");
        Self {
            client: Some(client),
            context: Some(context),
            value_resolver: Some(value_resolver),
            state_manager,
            config: Some(config),
        }
    }

    /// Check if connected to Loxone
    fn ensure_connected(&self) -> std::result::Result<(), String> {
        if self.client.is_none() {
            return Err("Server not initialized with Loxone client".to_string());
        }
        Ok(())
    }

    /// Get the Loxone client
    fn get_client(&self) -> std::result::Result<&Arc<dyn LoxoneClient>, String> {
        self.client
            .as_ref()
            .ok_or_else(|| "Client not initialized".to_string())
    }

    /// Resolve a room name to its UUID by searching the structure's rooms.
    /// Returns None if no matching room is found.
    fn resolve_room_uuid(structure: &LoxoneStructure, room_name: &str) -> Option<String> {
        let lower = room_name.to_lowercase();
        for (uuid, room) in &structure.rooms {
            if let Some(name) = room.get("name").and_then(|v| v.as_str())
                && (name.to_lowercase() == lower || name.to_lowercase().contains(&lower))
            {
                return Some(uuid.clone());
            }
        }
        None
    }

    /// Find controls matching the given types within a specific room (by room UUID).
    fn find_controls_by_type_in_room<'a>(
        structure: &'a LoxoneStructure,
        room_uuid: &str,
        types: &[&str],
    ) -> Vec<(&'a String, &'a Value)> {
        structure
            .controls
            .iter()
            .filter(|(_, control)| {
                let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let control_room = control.get("room").and_then(|v| v.as_str()).unwrap_or("");
                types.contains(&control_type) && control_room == room_uuid
            })
            .collect()
    }

    /// Find controls matching the given types across the entire system.
    fn find_controls_by_type<'a>(
        structure: &'a LoxoneStructure,
        types: &[&str],
    ) -> Vec<(&'a String, &'a Value)> {
        structure
            .controls
            .iter()
            .filter(|(_, control)| {
                let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
                types.contains(&control_type)
            })
            .collect()
    }

    /// Find a single control by UUID or by name (case-insensitive partial match).
    /// Returns the (uuid, control) pair.
    fn find_control_by_id_or_name<'a>(
        structure: &'a LoxoneStructure,
        identifier: &str,
    ) -> Option<(&'a String, &'a Value)> {
        // Try exact UUID match first
        if let Some(control) = structure.controls.get(identifier) {
            return structure
                .controls
                .get_key_value(identifier)
                .map(|(k, _)| (k, control));
        }
        // Fall back to name search
        let lower = identifier.to_lowercase();
        structure.controls.iter().find(|(_, control)| {
            control
                .get("name")
                .and_then(|v| v.as_str())
                .map(|n| n.to_lowercase() == lower || n.to_lowercase().contains(&lower))
                .unwrap_or(false)
        })
    }

    /// Search for climate controllers in a room by room name.
    fn find_climate_in_room<'a>(
        structure: &'a LoxoneStructure,
        room_name: &str,
        climate_types: &[&str],
    ) -> std::result::Result<Vec<(&'a String, &'a Value)>, String> {
        if let Some(room_uuid) = Self::resolve_room_uuid(structure, room_name) {
            Ok(Self::find_controls_by_type_in_room(
                structure,
                &room_uuid,
                climate_types,
            ))
        } else {
            // If room name didn't resolve, try to find climate controllers whose name contains the room
            let lower = room_name.to_lowercase();
            Ok(structure
                .controls
                .iter()
                .filter(|(_, control)| {
                    let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let name = control
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_lowercase();
                    climate_types.contains(&control_type) && name.contains(&lower)
                })
                .collect())
        }
    }

    /// Fetch live state for a list of UUIDs and return a mapping from UUID to state value.
    async fn fetch_live_states(
        client: &Arc<dyn LoxoneClient>,
        uuids: &[String],
    ) -> std::collections::HashMap<String, Value> {
        if uuids.is_empty() {
            return std::collections::HashMap::new();
        }
        match client.get_device_states(uuids).await {
            Ok(states) => states,
            Err(e) => {
                warn!("Failed to fetch live device states: {e}");
                std::collections::HashMap::new()
            }
        }
    }
}

/// All MCP tools defined in a single impl block
#[mcp_tools]
impl LoxoneMcpServer {
    // ========================================================================
    // LIGHTING TOOLS
    // ========================================================================

    /// Control lights in a room, by device, or system-wide
    ///
    /// Unified lighting control with scope-based targeting. Supports:
    /// - scope: "device" (single light), "room" (all lights in room), "system" (all lights)
    /// - action: "on", "off", "dim", "bright"
    /// - brightness: 0-100 for dimming (optional)
    pub async fn control_lights(
        &self,
        scope: String,
        target: Option<String>,
        action: String,
        brightness: Option<u8>,
    ) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        // Normalize action (multi-language support)
        let normalized_action = match action.to_lowercase().as_str() {
            "on" | "ein" | "an" | "einschalten" => "on",
            "off" | "aus" | "ab" | "ausschalten" => "off",
            "dim" | "dimmen" => "dim",
            "bright" | "hell" => "bright",
            _ => {
                return Err(format!(
                    "Invalid action '{action}'. Supported: on, off, dim, bright"
                ));
            }
        };

        // Validate brightness
        if let Some(level) = brightness
            && level > 100
        {
            return Err("Brightness must be between 0-100".to_string());
        }

        // Build the Loxone command string from the normalized action + brightness
        let command = match (normalized_action, brightness) {
            (_, Some(level)) => format!("{level}"),
            ("on", None) => "on".to_string(),
            ("off", None) => "off".to_string(),
            ("dim", None) => "25".to_string(), // default dim level
            ("bright", None) => "100".to_string(), // full brightness
            _ => "on".to_string(),
        };

        let client = self.get_client()?;
        let light_types = &["Switch", "Dimmer", "LightController", "ColorPicker"];

        match scope.to_lowercase().as_str() {
            "device" => {
                let target_id = target
                    .as_deref()
                    .ok_or_else(|| "target is required when scope is 'device'".to_string())?;
                let response = client
                    .send_command(target_id, &command)
                    .await
                    .map_err(|e| format!("Failed to send command to device {target_id}: {e}"))?;
                Ok(json!({
                    "scope": "device",
                    "target": target_id,
                    "action": normalized_action,
                    "brightness": brightness,
                    "command_sent": command,
                    "status": "executed",
                    "miniserver_response": response.value
                }))
            }
            "room" => {
                let room_name = target.as_deref().ok_or_else(|| {
                    "target (room name) is required when scope is 'room'".to_string()
                })?;
                let structure = client
                    .get_structure()
                    .await
                    .map_err(|e| format!("Failed to get structure: {e}"))?;
                let room_uuid = Self::resolve_room_uuid(&structure, room_name)
                    .ok_or_else(|| format!("Room '{room_name}' not found"))?;
                let controls =
                    Self::find_controls_by_type_in_room(&structure, &room_uuid, light_types);
                if controls.is_empty() {
                    return Err(format!("No lights found in room '{room_name}'"));
                }
                let mut results = Vec::new();
                for (uuid, control) in &controls {
                    let name = control
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    match client.send_command(uuid, &command).await {
                        Ok(response) => {
                            results.push(json!({
                                "uuid": uuid,
                                "name": name,
                                "status": "executed",
                                "miniserver_response": response.value
                            }));
                        }
                        Err(e) => {
                            results.push(json!({
                                "uuid": uuid,
                                "name": name,
                                "status": "error",
                                "error": format!("{e}")
                            }));
                        }
                    }
                }
                Ok(json!({
                    "scope": "room",
                    "target": room_name,
                    "action": normalized_action,
                    "brightness": brightness,
                    "command_sent": command,
                    "devices_affected": results.len(),
                    "results": results
                }))
            }
            "system" => {
                let structure = client
                    .get_structure()
                    .await
                    .map_err(|e| format!("Failed to get structure: {e}"))?;
                let controls = Self::find_controls_by_type(&structure, light_types);
                if controls.is_empty() {
                    return Err("No lights found in the system".to_string());
                }
                let mut results = Vec::new();
                for (uuid, control) in &controls {
                    let name = control
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    match client.send_command(uuid, &command).await {
                        Ok(response) => {
                            results.push(json!({
                                "uuid": uuid,
                                "name": name,
                                "status": "executed",
                                "miniserver_response": response.value
                            }));
                        }
                        Err(e) => {
                            results.push(json!({
                                "uuid": uuid,
                                "name": name,
                                "status": "error",
                                "error": format!("{e}")
                            }));
                        }
                    }
                }
                Ok(json!({
                    "scope": "system",
                    "action": normalized_action,
                    "brightness": brightness,
                    "command_sent": command,
                    "devices_affected": results.len(),
                    "results": results
                }))
            }
            _ => Err(format!(
                "Invalid scope '{scope}'. Use: device, room, system"
            )),
        }
    }

    /// Get the current state of all lights
    ///
    /// Returns a list of all lighting devices with their current state,
    /// brightness level, and room location.
    pub async fn get_lights_status(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let mut light_uuids = Vec::new();
        let mut light_info = Vec::new();

        for (uuid, control) in &structure.controls {
            let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");

            if matches!(
                control_type,
                "Switch" | "Dimmer" | "LightController" | "ColorPicker"
            ) {
                light_uuids.push(uuid.clone());
                light_info.push((uuid.clone(), control.clone()));
            }
        }

        // Fetch live states for all lights
        let live_states = Self::fetch_live_states(client, &light_uuids).await;

        let lights: Vec<Value> = light_info
            .iter()
            .map(|(uuid, control)| {
                let name = control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let room = control
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let state = live_states.get(uuid).cloned().unwrap_or(Value::Null);

                json!({
                    "uuid": uuid,
                    "name": name,
                    "type": control_type,
                    "room": room,
                    "state": state
                })
            })
            .collect();

        Ok(json!({
            "lights": lights,
            "count": lights.len()
        }))
    }

    // ========================================================================
    // CLIMATE TOOLS
    // ========================================================================

    /// Control room temperature settings
    ///
    /// Set target temperature for a room or zone. Supports heating and cooling modes.
    pub async fn set_temperature(
        &self,
        room: String,
        temperature: f64,
        mode: Option<String>,
    ) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        if !(5.0..=35.0).contains(&temperature) {
            return Err("Temperature must be between 5°C and 35°C".to_string());
        }

        let mode = mode.unwrap_or_else(|| "auto".to_string());
        if !["heat", "cool", "auto", "off"].contains(&mode.as_str()) {
            return Err(format!("Invalid mode '{mode}'. Use: heat, cool, auto, off"));
        }

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let climate_types = &["IRoomController", "Intelligent Room Controller"];

        // Try to find the thermostat: first by direct UUID/name, then by room
        let thermostat = Self::find_control_by_id_or_name(&structure, &room);
        let targets: Vec<(&String, &Value)> = if let Some(target) = thermostat {
            // Check it's actually a climate controller
            let control_type = target.1.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if climate_types.contains(&control_type) {
                vec![target]
            } else {
                // Not a climate controller, search by room
                Self::find_climate_in_room(&structure, &room, climate_types)?
            }
        } else {
            Self::find_climate_in_room(&structure, &room, climate_types)?
        };

        if targets.is_empty() {
            return Err(format!("No climate controller found for room '{room}'"));
        }

        let command = format!("settemp/{temperature}");
        let mut results = Vec::new();
        for (uuid, control) in &targets {
            let name = control
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            match client.send_command(uuid, &command).await {
                Ok(response) => {
                    results.push(json!({
                        "uuid": uuid,
                        "name": name,
                        "status": "executed",
                        "miniserver_response": response.value
                    }));
                }
                Err(e) => {
                    results.push(json!({
                        "uuid": uuid,
                        "name": name,
                        "status": "error",
                        "error": format!("{e}")
                    }));
                }
            }
        }

        // Also send mode command if not "auto" (the default)
        if mode != "auto" {
            let mode_command = match mode.as_str() {
                "heat" => "setmode/1",
                "cool" => "setmode/2",
                "off" => "setmode/0",
                _ => "setmode/3", // auto
            };
            for (uuid, _) in &targets {
                if let Err(e) = client.send_command(uuid, mode_command).await {
                    warn!("Failed to set mode on {uuid}: {e}");
                }
            }
        }

        Ok(json!({
            "room": room,
            "target_temperature": temperature,
            "mode": mode,
            "command_sent": command,
            "controllers_affected": results.len(),
            "results": results
        }))
    }

    /// Get current climate status for all rooms
    pub async fn get_climate_status(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let mut climate_uuids = Vec::new();
        let mut climate_info = Vec::new();

        for (uuid, control) in &structure.controls {
            let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");

            if matches!(
                control_type,
                "IRoomController" | "Intelligent Room Controller"
            ) {
                climate_uuids.push(uuid.clone());
                climate_info.push((uuid.clone(), control.clone()));
            }
        }

        let live_states = Self::fetch_live_states(client, &climate_uuids).await;

        let climate_data: Vec<Value> = climate_info
            .iter()
            .map(|(uuid, control)| {
                let name = control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let room = control
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let state = live_states.get(uuid).cloned().unwrap_or(Value::Null);

                json!({
                    "uuid": uuid,
                    "name": name,
                    "room": room,
                    "state": state
                })
            })
            .collect();

        Ok(json!({
            "climate_controllers": climate_data,
            "count": climate_data.len()
        }))
    }

    // ========================================================================
    // BLINDS/ROLLADEN TOOLS
    // ========================================================================

    /// Control blinds/rolladen position
    ///
    /// Set blind position (0=fully open, 100=fully closed) or use actions like up/down/stop.
    pub async fn control_blinds(
        &self,
        target: String,
        action: Option<String>,
        position: Option<u8>,
    ) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        // Determine command based on action or position
        let command = if let Some(pos) = position {
            if pos > 100 {
                return Err("Position must be between 0-100".to_string());
            }
            format!("ManualPosition/{pos}")
        } else if let Some(ref act) = action {
            match act.to_lowercase().as_str() {
                "up" | "open" | "auf" => "FullUp".to_string(),
                "down" | "close" | "ab" | "zu" => "FullDown".to_string(),
                "stop" | "halt" => "Stop".to_string(),
                "shade" | "schatten" => "Shade".to_string(),
                _ => {
                    return Err(format!(
                        "Invalid action '{act}'. Use: up, down, stop, shade"
                    ));
                }
            }
        } else {
            return Err("Either action or position must be provided".to_string());
        };

        let client = self.get_client()?;

        // Target can be a UUID or a device name; send command directly
        let response = client
            .send_command(&target, &command)
            .await
            .map_err(|e| format!("Failed to send blinds command to {target}: {e}"))?;

        Ok(json!({
            "target": target,
            "action": action,
            "position": position,
            "command_sent": command,
            "status": "executed",
            "miniserver_response": response.value
        }))
    }

    /// Get status of all blinds/rolladen
    pub async fn get_blinds_status(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let mut blind_uuids = Vec::new();
        let mut blind_info = Vec::new();

        for (uuid, control) in &structure.controls {
            let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");

            if matches!(control_type, "Jalousie" | "Blinds" | "Rolladen") {
                blind_uuids.push(uuid.clone());
                blind_info.push((uuid.clone(), control.clone()));
            }
        }

        let live_states = Self::fetch_live_states(client, &blind_uuids).await;

        let blinds: Vec<Value> = blind_info
            .iter()
            .map(|(uuid, control)| {
                let name = control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let room = control
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let state = live_states.get(uuid).cloned().unwrap_or(Value::Null);

                json!({
                    "uuid": uuid,
                    "name": name,
                    "room": room,
                    "state": state
                })
            })
            .collect();

        Ok(json!({
            "blinds": blinds,
            "count": blinds.len()
        }))
    }

    // ========================================================================
    // DISCOVERY TOOLS
    // ========================================================================

    /// List all rooms in the Loxone system
    pub async fn list_rooms(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let rooms: Vec<_> = structure
            .rooms
            .iter()
            .map(|(uuid, room)| {
                json!({
                    "uuid": uuid,
                    "name": room.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                    "type": room.get("type").and_then(|v| v.as_str()).unwrap_or("Room")
                })
            })
            .collect();

        Ok(json!({
            "rooms": rooms,
            "count": rooms.len()
        }))
    }

    /// List all devices in a specific room or system-wide
    pub async fn list_devices(
        &self,
        room: Option<String>,
        _filter: Option<String>,
    ) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let devices: Vec<_> = structure
            .controls
            .iter()
            .filter(|(_, control)| {
                if let Some(ref room_filter) = room {
                    control
                        .get("room")
                        .and_then(|v| v.as_str())
                        .map(|r| r.to_lowercase().contains(&room_filter.to_lowercase()))
                        .unwrap_or(false)
                } else {
                    true
                }
            })
            .map(|(uuid, control)| {
                json!({
                    "uuid": uuid,
                    "name": control.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                    "type": control.get("type").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                    "room": control.get("room").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                    "category": control.get("cat").and_then(|v| v.as_str()).unwrap_or("Unknown")
                })
            })
            .collect();

        Ok(json!({
            "devices": devices,
            "count": devices.len(),
            "filter": room
        }))
    }

    /// Get detailed information about a specific device
    pub async fn get_device_info(
        &self,
        device_id: String,
        _detail: Option<String>,
    ) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        // Find device by UUID or name
        let device = structure.controls.iter().find(|(uuid, control)| {
            uuid.to_string() == device_id
                || control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|n| n.to_lowercase().contains(&device_id.to_lowercase()))
                    .unwrap_or(false)
        });

        match device {
            Some((uuid, control)) => Ok(json!({
                "uuid": uuid,
                "control": control
            })),
            None => Err(format!("Device '{device_id}' not found")),
        }
    }

    // ========================================================================
    // SYSTEM TOOLS
    // ========================================================================

    /// Get server status and health information
    pub async fn get_server_status(&self) -> std::result::Result<serde_json::Value, String> {
        let connected = self.context.is_some() && self.client.is_some();

        Ok(json!({
            "connected": connected,
            "version": env!("CARGO_PKG_VERSION"),
            "name": "Loxone MCP Server"
        }))
    }

    // ========================================================================
    // AUDIO TOOLS
    // ========================================================================

    /// Control an audio zone (play, pause, stop, next, previous)
    ///
    /// Manages playback in audio zones. Actions: play, pause, stop, next, previous, mute, unmute
    pub async fn control_audio_zone(
        &self,
        zone: String,
        action: String,
    ) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let normalized_action = match action.to_lowercase().as_str() {
            "play" | "abspielen" | "start" => "play",
            "pause" | "pausieren" => "pause",
            "stop" | "stopp" | "anhalten" => "stop",
            "next" | "weiter" | "nächster" => "next",
            "previous" | "zurück" | "vorheriger" => "previous",
            "mute" | "stumm" => "mute",
            "unmute" | "laut" => "unmute",
            _ => {
                return Err(format!(
                    "Invalid action '{action}'. Use: play, pause, stop, next, previous, mute, unmute"
                ));
            }
        };

        let client = self.get_client()?;

        // Map normalized actions to Loxone audio commands
        let command = match normalized_action {
            "play" => "play",
            "pause" => "pause",
            "stop" => "stop",
            "next" => "queueplus",
            "previous" => "queueminus",
            "mute" => "mute",
            "unmute" => "unmute",
            _ => normalized_action,
        };

        let response = client
            .send_command(&zone, command)
            .await
            .map_err(|e| format!("Failed to control audio zone {zone}: {e}"))?;

        Ok(json!({
            "zone": zone,
            "action": normalized_action,
            "command_sent": command,
            "status": "executed",
            "miniserver_response": response.value
        }))
    }

    /// Set volume for an audio zone
    ///
    /// Set volume level (0-100) for a specific audio zone
    pub async fn set_audio_volume(
        &self,
        zone: String,
        volume: u8,
    ) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        if volume > 100 {
            return Err("Volume must be between 0-100".to_string());
        }

        let client = self.get_client()?;
        let command = format!("volume/{volume}");
        let response = client
            .send_command(&zone, &command)
            .await
            .map_err(|e| format!("Failed to set volume on zone {zone}: {e}"))?;

        Ok(json!({
            "zone": zone,
            "volume": volume,
            "command_sent": command,
            "status": "executed",
            "miniserver_response": response.value
        }))
    }

    /// Get status of all audio zones
    pub async fn get_audio_status(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let mut audio_uuids = Vec::new();
        let mut audio_info = Vec::new();

        for (uuid, control) in &structure.controls {
            let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");

            if control_type.contains("Audio") || control_type == "MediaController" {
                audio_uuids.push(uuid.clone());
                audio_info.push((uuid.clone(), control.clone()));
            }
        }

        let live_states = Self::fetch_live_states(client, &audio_uuids).await;

        let audio_zones: Vec<Value> = audio_info
            .iter()
            .map(|(uuid, control)| {
                let name = control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let room = control
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let state = live_states.get(uuid).cloned().unwrap_or(Value::Null);

                json!({
                    "uuid": uuid,
                    "name": name,
                    "type": control_type,
                    "room": room,
                    "state": state
                })
            })
            .collect();

        Ok(json!({
            "audio_zones": audio_zones,
            "count": audio_zones.len()
        }))
    }

    // ========================================================================
    // SENSOR TOOLS
    // ========================================================================

    /// Get all sensor readings
    ///
    /// Returns current values from all sensors (temperature, humidity, motion, etc.)
    pub async fn get_sensor_readings(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let mut sensor_uuids = Vec::new();
        let mut sensor_info = Vec::new();

        for (uuid, control) in &structure.controls {
            let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");

            // Match sensor types
            if matches!(
                control_type,
                "InfoOnlyAnalog"
                    | "InfoOnlyDigital"
                    | "PresenceDetector"
                    | "MotionSensor"
                    | "SmokeAlarm"
                    | "Meter"
                    | "Sensor"
            ) {
                sensor_uuids.push(uuid.clone());
                sensor_info.push((uuid.clone(), control.clone()));
            }
        }

        let live_states = Self::fetch_live_states(client, &sensor_uuids).await;

        let sensors: Vec<Value> = sensor_info
            .iter()
            .map(|(uuid, control)| {
                let name = control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let room = control
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let state = live_states.get(uuid).cloned().unwrap_or(Value::Null);

                json!({
                    "uuid": uuid,
                    "name": name,
                    "type": control_type,
                    "room": room,
                    "value": state
                })
            })
            .collect();

        Ok(json!({
            "sensors": sensors,
            "count": sensors.len()
        }))
    }

    /// Get door and window sensor status
    ///
    /// Returns open/closed state of all door and window sensors
    pub async fn get_door_window_status(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let mut dw_uuids = Vec::new();
        let mut dw_info = Vec::new();

        for (uuid, control) in &structure.controls {
            let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let name = control
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();

            // Match door/window sensors
            if control_type == "InfoOnlyDigital"
                && (name.contains("door")
                    || name.contains("window")
                    || name.contains("tür")
                    || name.contains("fenster"))
            {
                dw_uuids.push(uuid.clone());
                dw_info.push((uuid.clone(), control.clone()));
            }
        }

        let live_states = Self::fetch_live_states(client, &dw_uuids).await;

        let door_windows: Vec<Value> = dw_info
            .iter()
            .map(|(uuid, control)| {
                let display_name = control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let room = control
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let state = live_states.get(uuid).cloned().unwrap_or(Value::Null);

                json!({
                    "uuid": uuid,
                    "name": display_name,
                    "room": room,
                    "state": state
                })
            })
            .collect();

        Ok(json!({
            "door_window_sensors": door_windows,
            "count": door_windows.len()
        }))
    }

    /// Get motion detector status
    pub async fn get_motion_status(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let mut motion_uuids = Vec::new();
        let mut motion_info = Vec::new();

        for (uuid, control) in &structure.controls {
            let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");

            if matches!(control_type, "PresenceDetector" | "MotionSensor") {
                motion_uuids.push(uuid.clone());
                motion_info.push((uuid.clone(), control.clone()));
            }
        }

        let live_states = Self::fetch_live_states(client, &motion_uuids).await;

        let motion_sensors: Vec<Value> = motion_info
            .iter()
            .map(|(uuid, control)| {
                let name = control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let room = control
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let state = live_states.get(uuid).cloned().unwrap_or(Value::Null);

                json!({
                    "uuid": uuid,
                    "name": name,
                    "room": room,
                    "state": state
                })
            })
            .collect();

        Ok(json!({
            "motion_sensors": motion_sensors,
            "count": motion_sensors.len()
        }))
    }

    // ========================================================================
    // WEATHER TOOLS
    // ========================================================================

    /// Get current weather data
    ///
    /// Returns weather station readings (temperature, humidity, wind, rain)
    pub async fn get_weather(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let mut weather_uuids = Vec::new();
        let mut weather_info = Vec::new();

        for (uuid, control) in &structure.controls {
            let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");

            if control_type.contains("Weather") {
                weather_uuids.push(uuid.clone());
                weather_info.push((uuid.clone(), control.clone()));
            }
        }

        let live_states = Self::fetch_live_states(client, &weather_uuids).await;

        let weather_devices: Vec<Value> = weather_info
            .iter()
            .map(|(uuid, control)| {
                let name = control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let state = live_states.get(uuid).cloned().unwrap_or(Value::Null);

                json!({
                    "uuid": uuid,
                    "name": name,
                    "type": control_type,
                    "state": state
                })
            })
            .collect();

        Ok(json!({
            "weather_devices": weather_devices,
            "count": weather_devices.len()
        }))
    }

    // ========================================================================
    // ENERGY TOOLS
    // ========================================================================

    /// Get energy consumption data
    ///
    /// Returns current power usage and energy meters
    pub async fn get_energy_status(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let mut energy_uuids = Vec::new();
        let mut energy_info = Vec::new();

        for (uuid, control) in &structure.controls {
            let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");

            if matches!(control_type, "Meter" | "EnergyManager" | "EnergyMonitor")
                || control_type.contains("Energy")
            {
                energy_uuids.push(uuid.clone());
                energy_info.push((uuid.clone(), control.clone()));
            }
        }

        let live_states = Self::fetch_live_states(client, &energy_uuids).await;

        let energy_devices: Vec<Value> = energy_info
            .iter()
            .map(|(uuid, control)| {
                let name = control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let room = control
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let state = live_states.get(uuid).cloned().unwrap_or(Value::Null);

                json!({
                    "uuid": uuid,
                    "name": name,
                    "type": control_type,
                    "room": room,
                    "state": state
                })
            })
            .collect();

        Ok(json!({
            "energy_devices": energy_devices,
            "count": energy_devices.len()
        }))
    }

    /// Control EV charging
    ///
    /// Start, stop, or set charging limits for electric vehicle chargers
    pub async fn control_ev_charging(
        &self,
        charger: String,
        action: String,
        limit_kwh: Option<f64>,
    ) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let normalized_action = match action.to_lowercase().as_str() {
            "start" | "laden" => "start",
            "stop" | "stoppen" => "stop",
            "pause" | "pausieren" => "pause",
            _ => {
                return Err(format!(
                    "Invalid action '{action}'. Use: start, stop, pause"
                ));
            }
        };

        let client = self.get_client()?;

        // Map actions to Loxone commands
        let command = match normalized_action {
            "start" => "on".to_string(),
            "stop" => "off".to_string(),
            "pause" => "off".to_string(),
            _ => "off".to_string(),
        };

        let response = client
            .send_command(&charger, &command)
            .await
            .map_err(|e| format!("Failed to control EV charger {charger}: {e}"))?;

        // If a limit was specified, try to send it as well
        let limit_response = if let Some(limit) = limit_kwh {
            let limit_cmd = format!("setlimit/{limit}");
            match client.send_command(&charger, &limit_cmd).await {
                Ok(resp) => Some(resp.value),
                Err(e) => {
                    warn!("Failed to set charging limit on {charger}: {e}");
                    None
                }
            }
        } else {
            None
        };

        Ok(json!({
            "charger": charger,
            "action": normalized_action,
            "command_sent": command,
            "limit_kwh": limit_kwh,
            "status": "executed",
            "miniserver_response": response.value,
            "limit_response": limit_response
        }))
    }

    // ========================================================================
    // SECURITY TOOLS
    // ========================================================================

    /// Get security system status
    ///
    /// Returns alarm system state, door locks, and security sensors
    pub async fn get_security_status(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let mut security_uuids = Vec::new();
        let mut security_info = Vec::new();

        for (uuid, control) in &structure.controls {
            let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");

            if matches!(
                control_type,
                "Alarm" | "SmokeAlarm" | "Gate" | "DoorLock" | "AccessControl"
            ) || control_type.contains("Security")
            {
                security_uuids.push(uuid.clone());
                security_info.push((uuid.clone(), control.clone()));
            }
        }

        let live_states = Self::fetch_live_states(client, &security_uuids).await;

        let security_devices: Vec<Value> = security_info
            .iter()
            .map(|(uuid, control)| {
                let name = control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let room = control
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let state = live_states.get(uuid).cloned().unwrap_or(Value::Null);

                json!({
                    "uuid": uuid,
                    "name": name,
                    "type": control_type,
                    "room": room,
                    "state": state
                })
            })
            .collect();

        Ok(json!({
            "security_devices": security_devices,
            "count": security_devices.len()
        }))
    }

    /// Arm or disarm security system
    ///
    /// Set security system mode: arm_away, arm_home, disarm
    pub async fn set_security_mode(
        &self,
        mode: String,
        code: Option<String>,
    ) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let normalized_mode = match mode.to_lowercase().as_str() {
            "arm" | "arm_away" | "scharf" | "abwesend" => "arm_away",
            "arm_home" | "arm_stay" | "zuhause" => "arm_home",
            "disarm" | "unscharf" | "aus" => "disarm",
            _ => {
                return Err(format!(
                    "Invalid mode '{mode}'. Use: arm_away, arm_home, disarm"
                ));
            }
        };

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        // Find alarm/security controls
        let security_controls: Vec<(&String, &Value)> = structure
            .controls
            .iter()
            .filter(|(_, control)| {
                let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
                matches!(control_type, "Alarm" | "AccessControl")
                    || control_type.contains("Security")
            })
            .collect();

        if security_controls.is_empty() {
            return Err("No security/alarm devices found in the system".to_string());
        }

        // Build command - include code if provided
        let command = match (normalized_mode, &code) {
            ("arm_away", Some(c)) => format!("on/{c}"),
            ("arm_away", None) => "on".to_string(),
            ("arm_home", Some(c)) => format!("on/{c}"),
            ("arm_home", None) => "on".to_string(),
            ("disarm", Some(c)) => format!("off/{c}"),
            ("disarm", None) => "off".to_string(),
            _ => "off".to_string(),
        };

        let mut results = Vec::new();
        for (uuid, control) in &security_controls {
            let name = control
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            match client.send_command(uuid, &command).await {
                Ok(response) => {
                    results.push(json!({
                        "uuid": uuid,
                        "name": name,
                        "status": "executed",
                        "miniserver_response": response.value
                    }));
                }
                Err(e) => {
                    results.push(json!({
                        "uuid": uuid,
                        "name": name,
                        "status": "error",
                        "error": format!("{e}")
                    }));
                }
            }
        }

        Ok(json!({
            "mode": normalized_mode,
            "code_provided": code.is_some(),
            "command_sent": command,
            "devices_affected": results.len(),
            "results": results
        }))
    }

    /// Control door lock
    ///
    /// Lock or unlock a smart door lock
    pub async fn control_door_lock(
        &self,
        lock: String,
        action: String,
    ) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let normalized_action = match action.to_lowercase().as_str() {
            "lock" | "abschließen" | "zu" => "lock",
            "unlock" | "aufschließen" | "auf" => "unlock",
            _ => return Err(format!("Invalid action '{action}'. Use: lock, unlock")),
        };

        let client = self.get_client()?;
        let command = match normalized_action {
            "lock" => "on",
            "unlock" => "off",
            _ => "off",
        };

        let response = client
            .send_command(&lock, command)
            .await
            .map_err(|e| format!("Failed to control door lock {lock}: {e}"))?;

        Ok(json!({
            "lock": lock,
            "action": normalized_action,
            "command_sent": command,
            "status": "executed",
            "miniserver_response": response.value
        }))
    }

    // ========================================================================
    // CAMERA TOOLS
    // ========================================================================

    /// Get camera/intercom status
    ///
    /// Returns list of cameras and video intercoms
    pub async fn get_camera_status(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let mut camera_uuids = Vec::new();
        let mut camera_info = Vec::new();

        for (uuid, control) in &structure.controls {
            let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");

            if matches!(control_type, "Intercom" | "Camera" | "Doorbell")
                || control_type.contains("Camera")
            {
                camera_uuids.push(uuid.clone());
                camera_info.push((uuid.clone(), control.clone()));
            }
        }

        let live_states = Self::fetch_live_states(client, &camera_uuids).await;

        let cameras: Vec<Value> = camera_info
            .iter()
            .map(|(uuid, control)| {
                let name = control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let room = control
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let state = live_states.get(uuid).cloned().unwrap_or(Value::Null);

                json!({
                    "uuid": uuid,
                    "name": name,
                    "type": control_type,
                    "room": room,
                    "state": state
                })
            })
            .collect();

        Ok(json!({
            "cameras": cameras,
            "count": cameras.len()
        }))
    }

    // ========================================================================
    // INTERCOM TOOLS
    // ========================================================================

    /// Answer or control intercom
    ///
    /// Answer calls, open doors, or control intercom features
    pub async fn control_intercom(
        &self,
        intercom: String,
        action: String,
    ) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let normalized_action = match action.to_lowercase().as_str() {
            "answer" | "annehmen" | "abheben" => "answer",
            "hangup" | "auflegen" | "beenden" => "hangup",
            "open" | "öffnen" | "tür" => "open_door",
            "talk" | "sprechen" => "talk",
            "mute" | "stumm" => "mute",
            _ => {
                return Err(format!(
                    "Invalid action '{action}'. Use: answer, hangup, open, talk, mute"
                ));
            }
        };

        let client = self.get_client()?;

        // Map intercom actions to Loxone commands
        let command = match normalized_action {
            "answer" => "answer",
            "hangup" => "hangup",
            "open_door" => "open",
            "talk" => "talk",
            "mute" => "mute",
            _ => normalized_action,
        };

        let response = client
            .send_command(&intercom, command)
            .await
            .map_err(|e| format!("Failed to control intercom {intercom}: {e}"))?;

        Ok(json!({
            "intercom": intercom,
            "action": normalized_action,
            "command_sent": command,
            "status": "executed",
            "miniserver_response": response.value
        }))
    }

    /// Get intercom call history
    pub async fn get_intercom_history(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        Ok(json!({
            "history": [],
            "message": "Call history requires active connection to Loxone"
        }))
    }

    // ========================================================================
    // SCENE/MOOD TOOLS
    // ========================================================================

    /// Activate a scene or mood
    ///
    /// Trigger predefined scenes (moods) for rooms or the whole house
    pub async fn activate_scene(
        &self,
        scene: String,
        room: Option<String>,
    ) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let scene_types = &["LightController", "MoodSwitch"];

        // If scene looks like a UUID, send command directly
        if scene.contains('-') && scene.len() > 30 {
            let command = format!("changeTo/{scene}");
            let response = client
                .send_command(&scene, "on")
                .await
                .map_err(|e| format!("Failed to activate scene {scene}: {e}"))?;
            return Ok(json!({
                "scene": scene,
                "room": room,
                "command_sent": command,
                "status": "activated",
                "miniserver_response": response.value
            }));
        }

        // Search for matching scene controllers
        let controllers: Vec<(&String, &Value)> = if let Some(ref room_name) = room {
            if let Some(room_uuid) = Self::resolve_room_uuid(&structure, room_name) {
                Self::find_controls_by_type_in_room(&structure, &room_uuid, scene_types)
            } else {
                // Try matching by name
                structure
                    .controls
                    .iter()
                    .filter(|(_, control)| {
                        let ct = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        let name = control
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_lowercase();
                        scene_types.contains(&ct) && name.contains(&room_name.to_lowercase())
                    })
                    .collect()
            }
        } else {
            Self::find_controls_by_type(&structure, scene_types)
        };

        if controllers.is_empty() {
            return Err(format!(
                "No scene controllers found{}",
                room.as_ref()
                    .map(|r| format!(" in room '{r}'"))
                    .unwrap_or_default()
            ));
        }

        // Try to match the scene name to a mood ID, or use the scene value directly
        let scene_lower = scene.to_lowercase();
        let mut results = Vec::new();
        for (uuid, control) in &controllers {
            let name = control
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");

            // Check if there are moods defined that match the scene name
            let mood_id = if let Some(moods) = control.get("moods") {
                if let Some(moods_obj) = moods.as_object() {
                    moods_obj
                        .iter()
                        .find(|(_, v)| {
                            v.as_str()
                                .map(|s| s.to_lowercase().contains(&scene_lower))
                                .unwrap_or(false)
                        })
                        .map(|(id, _)| id.clone())
                } else {
                    None
                }
            } else {
                None
            };

            let command = if let Some(ref id) = mood_id {
                format!("changeTo/{id}")
            } else {
                // Try the scene string as a direct command (could be a mood number)
                format!("changeTo/{scene}")
            };

            match client.send_command(uuid, &command).await {
                Ok(response) => {
                    results.push(json!({
                        "uuid": uuid,
                        "name": name,
                        "command_sent": command,
                        "mood_id": mood_id,
                        "status": "activated",
                        "miniserver_response": response.value
                    }));
                }
                Err(e) => {
                    results.push(json!({
                        "uuid": uuid,
                        "name": name,
                        "command_sent": command,
                        "status": "error",
                        "error": format!("{e}")
                    }));
                }
            }
        }

        Ok(json!({
            "scene": scene,
            "room": room,
            "controllers_affected": results.len(),
            "results": results
        }))
    }

    /// List available scenes
    pub async fn list_scenes(&self) -> std::result::Result<serde_json::Value, String> {
        self.ensure_connected()?;

        let client = self.get_client()?;
        let structure = client
            .get_structure()
            .await
            .map_err(|e| format!("Failed to get structure: {e}"))?;

        let mut scenes = Vec::new();

        for (uuid, control) in &structure.controls {
            let control_type = control.get("type").and_then(|v| v.as_str()).unwrap_or("");

            if matches!(control_type, "LightController" | "MoodSwitch") {
                let name = control
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let room = control
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");

                // Extract moods if available
                let moods = control.get("moods").cloned().unwrap_or(json!([]));

                scenes.push(json!({
                    "uuid": uuid,
                    "name": name,
                    "room": room,
                    "moods": moods
                }));
            }
        }

        Ok(json!({
            "scene_controllers": scenes,
            "count": scenes.len()
        }))
    }
}
