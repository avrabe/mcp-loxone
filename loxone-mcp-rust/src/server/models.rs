//! Request and response models for MCP server

use schemars::JsonSchema;
use serde::Deserialize;

/// Device control request parameters
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeviceControlRequest {
    #[schemars(description = "Device UUID")]
    pub device_id: String,
    #[schemars(description = "Action to perform (on, off, up, down, stop)")]
    pub action: String,
}

/// Room control request parameters
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RoomControlRequest {
    #[schemars(description = "Room name")]
    pub room_name: String,
    #[schemars(description = "Action to perform")]
    pub action: String,
}

/// Temperature control request parameters
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TemperatureRequest {
    #[schemars(description = "Room name")]
    pub room_name: String,
    #[schemars(description = "Target temperature in Celsius")]
    pub temperature: f64,
}

/// Room devices request parameters
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RoomDevicesRequest {
    #[schemars(description = "Name of the room")]
    pub room_name: String,
}
