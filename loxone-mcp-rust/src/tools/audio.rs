//! Audio system control tools
//!
//! This module provides tools for managing audio zones, music systems,
//! and speaker controls in Loxone systems.

use crate::client::LoxoneDevice;
use crate::tools::{ToolContext, ToolResponse};
use serde_json::{json, Value};
use tracing::debug;

/// Get audio system information including zones, sources, and playback status
pub async fn get_audio_zones(context: ToolContext) -> ToolResponse {
    debug!("Getting audio zone information");

    let _client = &context.client;
    let ctx = &context.context;

    // Get capabilities and devices
    let capabilities = ctx.capabilities.read().await;
    let devices = ctx.devices.read().await;

    // Check if audio capability is available
    if !capabilities.has_audio {
        return ToolResponse::success_with_message(
            json!({
                "error": "No audio devices available",
                "note": "Your Loxone system doesn't have audio zones or music systems configured"
            }),
            "No audio devices found in system".to_string(),
        );
    }

    // Look for audio-related devices
    let audio_categories = ["Audio", "Musik", "Music", "Multimedia"];
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

    let mut audio_devices = Vec::new();
    let mut seen_uuids = std::collections::HashSet::new();

    // Find audio devices by category, type, and keywords
    for device in devices.values() {
        let is_audio = audio_categories.contains(&device.category.as_str())
            || audio_types.contains(&device.device_type.as_str())
            || {
                let device_name = device.name.to_lowercase();
                audio_keywords
                    .iter()
                    .any(|keyword| device_name.contains(keyword))
            };

        if is_audio && seen_uuids.insert(device.uuid.clone()) {
            audio_devices.push(device.clone());
        }
    }

    if audio_devices.is_empty() {
        return ToolResponse::success_with_message(
            json!({
                "error": "No audio devices found",
                "note": "Searched for audio zones, music systems, and speakers",
                "suggestion": "Check if your Loxone system has audio components configured"
            }),
            "No audio devices found".to_string(),
        );
    }

    // Get audio status
    let mut zones = Vec::new();
    let playing_count = 0;
    let mut stopped_count = 0;

    for device in audio_devices {
        let mut zone_info = json!({
            "uuid": device.uuid,
            "name": device.name,
            "type": device.device_type,
            "room": device.room.as_deref().unwrap_or("Unknown")
        });

        // Try to get current state - this may not be available for all devices
        zone_info["status"] = json!("unknown");
        stopped_count += 1; // Default to stopped since we can't check state

        zones.push(zone_info);
    }

    ToolResponse::success(json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "zones": zones,
        "summary": {
            "total_zones": zones.len(),
            "playing": playing_count,
            "stopped": stopped_count
        }
    }))
}

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

/// Get available audio sources and their status
pub async fn get_audio_sources(context: ToolContext) -> ToolResponse {
    debug!("Getting audio sources information");

    let _client = &context.client;
    let devices = context.context.devices.read().await;

    // Look for audio source devices
    let source_types = ["Radio", "MediaPlayer", "AudioSource", "StreamingService"];
    let source_keywords = ["radio", "spotify", "stream", "input", "source"];

    let mut sources = Vec::new();
    let mut seen_uuids = std::collections::HashSet::new();

    // Find sources by type and keywords
    for device in devices.values() {
        let is_source = source_types.contains(&device.device_type.as_str()) || {
            let device_name = device.name.to_lowercase();
            source_keywords
                .iter()
                .any(|keyword| device_name.contains(keyword))
        };

        if is_source && seen_uuids.insert(device.uuid.clone()) {
            sources.push(create_audio_source_info(device, _client).await);
        }
    }

    ToolResponse::success(json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "sources": sources,
        "summary": {
            "total_sources": sources.len()
        }
    }))
}

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
