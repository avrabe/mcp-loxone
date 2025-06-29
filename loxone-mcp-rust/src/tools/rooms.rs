//! Room management MCP tools
//!
//! Tools for room-based device control operations.
//! For read-only room data, use resources:
//! - loxone://rooms - All rooms list
//! - loxone://rooms/{room}/devices - Devices in specific room
//! - loxone://rooms/{room}/overview - Room overview with statistics

use crate::tools::{ToolContext, ToolResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Room information response (used by resources)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    /// Room UUID
    pub uuid: String,

    /// Room name
    pub name: String,

    /// Number of devices in room
    pub device_count: usize,

    /// Device breakdown by category
    pub devices_by_category: HashMap<String, usize>,

    /// Sample device names (first 5)
    pub sample_devices: Vec<String>,
}

// READ-ONLY TOOLS REMOVED:
// The following tools were removed as they duplicate existing resources:
//
// - list_rooms() → Use resource: loxone://rooms
// - get_room_devices() → Use resource: loxone://rooms/{room}/devices  
// - get_room_overview() → Use resource: loxone://rooms/{room}/overview
//
// These functions provided read-only data access and violated MCP patterns.
// Use the corresponding resources for data retrieval instead.

// Future action-based room tools can be added here, such as:
// - set_room_mode() - Set room-wide climate/lighting mode
// - control_room_scene() - Activate room scenes
// - set_room_temperature() - Set target temperature for room