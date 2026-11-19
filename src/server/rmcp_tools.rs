//! MCP protocol surface for [`LoxoneMcpServer`], built on the official Rust
//! SDK (rmcp).
//!
//! Each tool takes one parameter struct, so every `inputSchema` is a JSON
//! object (issue #37). The wrappers only unpack parameters and call the tool
//! logic in `macro_backend`; success returns the JSON value as text plus
//! `structuredContent`, failure returns a tool result with `isError: true`.

use super::macro_backend::LoxoneMcpServer;
use rmcp::{
    ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Implementation, ServerCapabilities},
    schemars::JsonSchema,
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::Value;

/// Server name advertised in `initialize`.
pub const SERVER_NAME: &str = "Loxone MCP Server";
/// Server description advertised in `initialize`.
pub const SERVER_DESCRIPTION: &str = "High-performance MCP server for Loxone home automation";

type ToolResult = Result<CallToolResult, String>;

fn done(result: Result<Value, String>) -> ToolResult {
    result.map(CallToolResult::structured)
}

/// Parameters for a tool that takes none.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct NoParams {}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ControlLightsParams {
    pub scope: String,
    pub target: Option<String>,
    pub action: String,
    pub brightness: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetTemperatureParams {
    pub room: String,
    pub temperature: f64,
    pub mode: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ControlBlindsParams {
    pub target: String,
    pub action: Option<String>,
    pub position: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ListDevicesParams {
    pub room: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetDeviceInfoParams {
    pub device_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ControlAudioZoneParams {
    pub zone: String,
    pub action: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetAudioVolumeParams {
    pub zone: String,
    pub volume: u8,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ControlEvChargingParams {
    pub charger: String,
    pub action: String,
    pub limit_kwh: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetSecurityModeParams {
    pub mode: String,
    pub code: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ControlDoorLockParams {
    pub lock: String,
    pub action: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ControlIntercomParams {
    pub intercom: String,
    pub action: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ActivateSceneParams {
    pub scene: String,
    pub room: Option<String>,
}

#[tool_router(vis = "pub")]
impl LoxoneMcpServer {
    /// Control lights in a room, by device, or system-wide
    ///
    /// Unified lighting control with scope-based targeting. Supports:
    /// - scope: "device" (single light), "room" (all lights in room), "system" (all lights)
    /// - action: "on", "off", "dim", "bright"
    /// - brightness: 0-100 for dimming (optional)
    #[tool(name = "control_lights")]
    async fn tool_control_lights(
        &self,
        Parameters(p): Parameters<ControlLightsParams>,
    ) -> ToolResult {
        done(
            self.control_lights(p.scope, p.target, p.action, p.brightness)
                .await,
        )
    }

    /// Get the current state of all lights
    ///
    /// Returns a list of all lighting devices with their current state,
    /// brightness level, and room location.
    #[tool(name = "get_lights_status")]
    async fn tool_get_lights_status(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_lights_status().await)
    }

    /// Control room temperature settings
    ///
    /// Set target temperature for a room or zone. Supports heating and cooling modes.
    #[tool(name = "set_temperature")]
    async fn tool_set_temperature(
        &self,
        Parameters(p): Parameters<SetTemperatureParams>,
    ) -> ToolResult {
        done(self.set_temperature(p.room, p.temperature, p.mode).await)
    }

    /// Get current climate status for all rooms
    #[tool(name = "get_climate_status")]
    async fn tool_get_climate_status(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_climate_status().await)
    }

    /// Control blinds/rolladen position
    ///
    /// Set blind position (0=fully open, 100=fully closed) or use actions like up/down/stop.
    #[tool(name = "control_blinds")]
    async fn tool_control_blinds(
        &self,
        Parameters(p): Parameters<ControlBlindsParams>,
    ) -> ToolResult {
        done(self.control_blinds(p.target, p.action, p.position).await)
    }

    /// Get status of all blinds/rolladen
    #[tool(name = "get_blinds_status")]
    async fn tool_get_blinds_status(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_blinds_status().await)
    }

    /// List all rooms in the Loxone system
    #[tool(name = "list_rooms")]
    async fn tool_list_rooms(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.list_rooms().await)
    }

    /// List all devices in a specific room or system-wide
    #[tool(name = "list_devices")]
    async fn tool_list_devices(&self, Parameters(p): Parameters<ListDevicesParams>) -> ToolResult {
        done(self.list_devices(p.room).await)
    }

    /// Get detailed information about a specific device
    #[tool(name = "get_device_info")]
    async fn tool_get_device_info(
        &self,
        Parameters(p): Parameters<GetDeviceInfoParams>,
    ) -> ToolResult {
        done(self.get_device_info(p.device_id).await)
    }

    /// Get server status and health information
    #[tool(name = "get_server_status")]
    async fn tool_get_server_status(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_server_status().await)
    }

    /// Control an audio zone (play, pause, stop, next, previous)
    ///
    /// Manages playback in audio zones. Actions: play, pause, stop, next, previous, mute, unmute
    #[tool(name = "control_audio_zone")]
    async fn tool_control_audio_zone(
        &self,
        Parameters(p): Parameters<ControlAudioZoneParams>,
    ) -> ToolResult {
        done(self.control_audio_zone(p.zone, p.action).await)
    }

    /// Set volume for an audio zone
    ///
    /// Set volume level (0-100) for a specific audio zone
    #[tool(name = "set_audio_volume")]
    async fn tool_set_audio_volume(
        &self,
        Parameters(p): Parameters<SetAudioVolumeParams>,
    ) -> ToolResult {
        done(self.set_audio_volume(p.zone, p.volume).await)
    }

    /// Get status of all audio zones
    #[tool(name = "get_audio_status")]
    async fn tool_get_audio_status(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_audio_status().await)
    }

    /// Get all sensor readings
    ///
    /// Returns current values from all sensors (temperature, humidity, motion, etc.)
    #[tool(name = "get_sensor_readings")]
    async fn tool_get_sensor_readings(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_sensor_readings().await)
    }

    /// Get door and window sensor status
    ///
    /// Returns open/closed state of all door and window sensors
    #[tool(name = "get_door_window_status")]
    async fn tool_get_door_window_status(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_door_window_status().await)
    }

    /// Get motion detector status
    #[tool(name = "get_motion_status")]
    async fn tool_get_motion_status(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_motion_status().await)
    }

    /// Get current weather data
    ///
    /// Returns weather station readings (temperature, humidity, wind, rain)
    #[tool(name = "get_weather")]
    async fn tool_get_weather(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_weather().await)
    }

    /// Get energy consumption data
    ///
    /// Returns current power usage and energy meters
    #[tool(name = "get_energy_status")]
    async fn tool_get_energy_status(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_energy_status().await)
    }

    /// Control EV charging
    ///
    /// Start, stop, or set charging limits for electric vehicle chargers
    #[tool(name = "control_ev_charging")]
    async fn tool_control_ev_charging(
        &self,
        Parameters(p): Parameters<ControlEvChargingParams>,
    ) -> ToolResult {
        done(
            self.control_ev_charging(p.charger, p.action, p.limit_kwh)
                .await,
        )
    }

    /// Get security system status
    ///
    /// Returns alarm system state, door locks, and security sensors
    #[tool(name = "get_security_status")]
    async fn tool_get_security_status(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_security_status().await)
    }

    /// Arm or disarm security system
    ///
    /// Set security system mode: arm_away, arm_home, disarm
    #[tool(name = "set_security_mode")]
    async fn tool_set_security_mode(
        &self,
        Parameters(p): Parameters<SetSecurityModeParams>,
    ) -> ToolResult {
        done(self.set_security_mode(p.mode, p.code).await)
    }

    /// Control door lock
    ///
    /// Lock or unlock a smart door lock
    #[tool(name = "control_door_lock")]
    async fn tool_control_door_lock(
        &self,
        Parameters(p): Parameters<ControlDoorLockParams>,
    ) -> ToolResult {
        done(self.control_door_lock(p.lock, p.action).await)
    }

    /// Get camera/intercom status
    ///
    /// Returns list of cameras and video intercoms
    #[tool(name = "get_camera_status")]
    async fn tool_get_camera_status(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_camera_status().await)
    }

    /// Answer or control intercom
    ///
    /// Answer calls, open doors, or control intercom features
    #[tool(name = "control_intercom")]
    async fn tool_control_intercom(
        &self,
        Parameters(p): Parameters<ControlIntercomParams>,
    ) -> ToolResult {
        done(self.control_intercom(p.intercom, p.action).await)
    }

    /// Get intercom call history
    #[tool(name = "get_intercom_history")]
    async fn tool_get_intercom_history(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.get_intercom_history().await)
    }

    /// Activate a scene or mood
    ///
    /// Trigger predefined scenes (moods) for rooms or the whole house
    #[tool(name = "activate_scene")]
    async fn tool_activate_scene(
        &self,
        Parameters(p): Parameters<ActivateSceneParams>,
    ) -> ToolResult {
        done(self.activate_scene(p.scene, p.room).await)
    }

    /// List available scenes
    #[tool(name = "list_scenes")]
    async fn tool_list_scenes(&self, _: Parameters<NoParams>) -> ToolResult {
        done(self.list_scenes().await)
    }
}

#[tool_handler]
impl ServerHandler for LoxoneMcpServer {
    fn get_info(&self) -> rmcp::model::ServerConfig {
        rmcp::model::ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new(SERVER_NAME, env!("CARGO_PKG_VERSION"))
                    .with_description(SERVER_DESCRIPTION),
            )
            .with_instructions(SERVER_DESCRIPTION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue #37: strict MCP clients reject a server if any tool's
    /// `inputSchema.type` is not "object".
    #[test]
    fn every_tool_input_schema_is_an_object() {
        let tools = LoxoneMcpServer::tool_router().list_all();
        assert_eq!(tools.len(), 27, "tool count changed");
        for tool in &tools {
            assert_eq!(
                tool.input_schema.get("type").and_then(|t| t.as_str()),
                Some("object"),
                "tool {} has a non-object inputSchema: {:?}",
                tool.name,
                tool.input_schema
            );
        }
    }

    #[test]
    fn single_parameter_tools_keep_their_argument_names() {
        let tools = LoxoneMcpServer::tool_router().list_all();
        let props = |name: &str| -> Vec<String> {
            let tool = tools.iter().find(|t| t.name == name).expect(name);
            let mut keys: Vec<String> = tool
                .input_schema
                .get("properties")
                .and_then(|p| p.as_object())
                .map(|o| o.keys().cloned().collect())
                .unwrap_or_default();
            keys.sort();
            keys
        };
        assert_eq!(props("list_devices"), vec!["room"]);
        assert_eq!(props("get_device_info"), vec!["device_id"]);
    }
}
