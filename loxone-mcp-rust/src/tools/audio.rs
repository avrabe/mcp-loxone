//! Audio system control tools
//!
//! This module provides action tools for controlling audio zones, music systems,
//! and speaker controls in Loxone systems.
//! For read-only audio data, use resources:
//! - loxone://audio/zones - Audio zones list
//! - loxone://audio/sources - Audio sources list

use crate::client::LoxoneDevice;
use crate::tools::{ToolContext, ToolResponse};
use serde_json::{json, Value};
use tracing::debug;

// READ-ONLY TOOL REMOVED:
// get_audio_zones() → Use resource: loxone://audio/zones
// This function provided read-only data access and violated MCP patterns.

/// Control an audio zone (play, stop, volume control)
pub async fn control_audio_zone(
    context: ToolContext,
    zone_name: String,
    action: String,
    value: Option<f64>,
) -> ToolResponse {
    debug!(
        "Controlling audio zone '{}' with action '{}'",
        zone_name, action
    );

    let _client = &context.client;
    let devices = context.context.devices.read().await;

    // Find the audio zone
    let audio_device = devices.values().find(|device| {
        let name_match = device
            .name
            .to_lowercase()
            .contains(&zone_name.to_lowercase());
        let is_audio = is_audio_device(device);
        name_match && is_audio
    });

    let device = match audio_device {
        Some(device) => device,
        None => {
            return ToolResponse::error(format!(
                "Audio zone '{zone_name}' not found. Use get_audio_zones to see available zones"
            ));
        }
    };

    // Normalize action
    let normalized_action = normalize_audio_action(&action);

    // Execute the command
    let command = if let Some(val) = value {
        format!("{normalized_action}/{val}")
    } else {
        normalized_action
    };

    match _client.send_command(&device.uuid, &command).await {
        Ok(response) => ToolResponse::success(json!({
            "zone": device.name,
            "uuid": device.uuid,
            "action": action,
            "command": command,
            "result": response.value,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
        Err(e) => ToolResponse::error(format!(
            "Failed to control audio zone '{}': {}",
            device.name, e
        )),
    }
}

// READ-ONLY TOOL REMOVED:
// get_audio_sources() → Use resource: loxone://audio/sources
// This function provided read-only data access and violated MCP patterns.

/// Set volume for an audio zone
pub async fn set_audio_volume(
    context: ToolContext,
    zone_name: String,
    volume: f64,
) -> ToolResponse {
    debug!(
        "Setting volume for audio zone '{}' to {}",
        zone_name, volume
    );

    // Validate volume range
    if !(0.0..=100.0).contains(&volume) {
        return ToolResponse::error(format!(
            "Invalid volume level: {volume}. Valid range is 0-100"
        ));
    }

    control_audio_zone(context, zone_name, "volume".to_string(), Some(volume)).await
}

/// Helper function to check if a device is an audio device
fn is_audio_device(device: &LoxoneDevice) -> bool {
    let audio_types = ["AudioZone", "Radio", "MediaPlayer", "Intercom"];
    let audio_keywords = [
        "audio",
        "musik",
        "music",
        "radio",
        "speaker",
        "lautsprecher",
        "zone",
    ];

    // Check by type
    if audio_types.contains(&device.device_type.as_str()) {
        return true;
    }

    // Check by name keywords
    let device_name = device.name.to_lowercase();
    audio_keywords
        .iter()
        .any(|keyword| device_name.contains(keyword))
}

/// Helper function to interpret audio device state
#[allow(dead_code)]
fn interpret_audio_state(state: &Value) -> String {
    match state {
        Value::Number(n) => {
            if let Some(val) = n.as_f64() {
                if val > 0.0 {
                    "playing".to_string()
                } else {
                    "stopped".to_string()
                }
            } else {
                "unknown".to_string()
            }
        }
        Value::String(s) => s.to_lowercase(),
        Value::Bool(b) => {
            if *b {
                "playing".to_string()
            } else {
                "stopped".to_string()
            }
        }
        _ => "unknown".to_string(),
    }
}

/// Helper function to normalize audio actions
fn normalize_audio_action(action: &str) -> String {
    match action.to_lowercase().as_str() {
        // English commands
        "play" | "start" => "on".to_string(),
        "stop" | "pause" => "off".to_string(),
        "volume" | "vol" => "volume".to_string(),
        "mute" => "mute".to_string(),
        "unmute" => "unmute".to_string(),
        "next" => "next".to_string(),
        "previous" | "prev" => "previous".to_string(),

        // German commands
        "spielen" | "abspielen" => "on".to_string(),
        "stoppen" | "anhalten" => "off".to_string(),
        "lautstärke" => "volume".to_string(),
        "stumm" => "mute".to_string(),
        "entstummen" => "unmute".to_string(),
        "weiter" | "nächster" => "next".to_string(),
        "zurück" | "vorheriger" => "previous".to_string(),

        // Direct passthrough for unknown commands
        _ => action.to_lowercase(),
    }
}

/// Helper function to create audio source information
async fn create_audio_source_info(
    device: &LoxoneDevice,
    _client: &std::sync::Arc<dyn crate::client::LoxoneClient>,
) -> Value {
    json!({
        "uuid": device.uuid,
        "name": device.name,
        "type": device.device_type,
        "room": device.room.as_deref().unwrap_or("Unknown"),
        "status": "unknown"
    })
}
