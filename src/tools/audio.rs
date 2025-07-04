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

/// Playlist management for audio zones
use serde::{Deserialize, Serialize};

/// Audio playlist information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPlaylist {
    /// Playlist ID
    pub playlist_id: String,
    /// Playlist name
    pub name: String,
    /// Audio zone UUID
    pub zone_uuid: String,
    /// Track list
    pub tracks: Vec<AudioTrack>,
    /// Current track index
    pub current_track: Option<usize>,
    /// Shuffle enabled
    pub shuffle: bool,
    /// Repeat mode
    pub repeat_mode: RepeatMode,
    /// Total duration in seconds
    pub total_duration: u64,
}

/// Audio track information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTrack {
    /// Track ID
    pub track_id: String,
    /// Track title
    pub title: String,
    /// Artist name
    pub artist: Option<String>,
    /// Album name
    pub album: Option<String>,
    /// Track duration in seconds
    pub duration: u64,
    /// Track URL or file path
    pub url: String,
}

/// Repeat mode enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    Off,
    Track,
    Playlist,
}

impl From<&str> for RepeatMode {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "track" | "single" | "eins" => RepeatMode::Track,
            "playlist" | "all" | "alle" => RepeatMode::Playlist,
            _ => RepeatMode::Off,
        }
    }
}

/// Create playlist for audio zone
pub async fn create_playlist(
    context: ToolContext,
    zone_name: String,
    playlist_name: String,
    tracks: Vec<AudioTrack>,
) -> ToolResponse {
    debug!(
        "Creating playlist '{}' for audio zone '{}' with {} tracks",
        playlist_name,
        zone_name,
        tracks.len()
    );

    let devices = context.context.devices.read().await;
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

    let playlist_id = format!(
        "playlist_{}_{}",
        device.uuid,
        chrono::Utc::now().timestamp()
    );
    let total_duration = tracks.iter().map(|t| t.duration).sum();

    let playlist = AudioPlaylist {
        playlist_id: playlist_id.clone(),
        name: playlist_name.clone(),
        zone_uuid: device.uuid.clone(),
        tracks,
        current_track: None,
        shuffle: false,
        repeat_mode: RepeatMode::Off,
        total_duration,
    };

    let command = format!(
        "playlist/create/{}/{}",
        urlencoding::encode(&playlist_name),
        serialize_playlist(&playlist)
    );

    match context.send_device_command(&device.uuid, &command).await {
        Ok(response) => ToolResponse::success(json!({
            "zone": device.name,
            "uuid": device.uuid,
            "action": "create_playlist",
            "playlist": playlist,
            "result": response.value,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
        Err(e) => ToolResponse::error(format!(
            "Failed to create playlist for audio zone '{}': {}",
            device.name, e
        )),
    }
}

/// Load and play playlist in audio zone
pub async fn load_playlist(
    context: ToolContext,
    zone_name: String,
    playlist_name: String,
    start_track: Option<usize>,
) -> ToolResponse {
    debug!(
        "Loading playlist '{}' in audio zone '{}' starting at track {:?}",
        playlist_name, zone_name, start_track
    );

    let devices = context.context.devices.read().await;
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

    let command = if let Some(track_index) = start_track {
        format!(
            "playlist/load/{}/{}",
            urlencoding::encode(&playlist_name),
            track_index
        )
    } else {
        format!("playlist/load/{}", urlencoding::encode(&playlist_name))
    };

    match context.send_device_command(&device.uuid, &command).await {
        Ok(response) => ToolResponse::success(json!({
            "zone": device.name,
            "uuid": device.uuid,
            "action": "load_playlist",
            "playlist_name": playlist_name,
            "start_track": start_track,
            "result": response.value,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
        Err(e) => ToolResponse::error(format!(
            "Failed to load playlist in audio zone '{}': {}",
            device.name, e
        )),
    }
}

/// Control playlist playback (next, previous, shuffle, repeat)
pub async fn control_playlist(
    context: ToolContext,
    zone_name: String,
    action: String,
    value: Option<String>,
) -> ToolResponse {
    debug!(
        "Controlling playlist in audio zone '{}' with action '{}' and value: {:?}",
        zone_name, action, value
    );

    let devices = context.context.devices.read().await;
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

    let normalized_action = normalize_playlist_action(&action);
    let command = if let Some(ref val) = value {
        format!(
            "playlist/{}/{}",
            normalized_action,
            urlencoding::encode(val)
        )
    } else {
        format!("playlist/{}", normalized_action)
    };

    match context.send_device_command(&device.uuid, &command).await {
        Ok(response) => ToolResponse::success(json!({
            "zone": device.name,
            "uuid": device.uuid,
            "action": "playlist_control",
            "command": normalized_action,
            "value": value,
            "result": response.value,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
        Err(e) => ToolResponse::error(format!(
            "Failed to control playlist in audio zone '{}': {}",
            device.name, e
        )),
    }
}

/// Add track to existing playlist
pub async fn add_track_to_playlist(
    context: ToolContext,
    zone_name: String,
    playlist_name: String,
    track: AudioTrack,
    position: Option<usize>,
) -> ToolResponse {
    debug!(
        "Adding track '{}' to playlist '{}' in zone '{}' at position {:?}",
        track.title, playlist_name, zone_name, position
    );

    let devices = context.context.devices.read().await;
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

    let command = if let Some(pos) = position {
        format!(
            "playlist/add/{}/{}/{}",
            urlencoding::encode(&playlist_name),
            pos,
            serialize_track(&track)
        )
    } else {
        format!(
            "playlist/add/{}/{}",
            urlencoding::encode(&playlist_name),
            serialize_track(&track)
        )
    };

    match context.send_device_command(&device.uuid, &command).await {
        Ok(response) => ToolResponse::success(json!({
            "zone": device.name,
            "uuid": device.uuid,
            "action": "add_track",
            "playlist_name": playlist_name,
            "track": track,
            "position": position,
            "result": response.value,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
        Err(e) => ToolResponse::error(format!(
            "Failed to add track to playlist in audio zone '{}': {}",
            device.name, e
        )),
    }
}

/// Remove track from playlist
pub async fn remove_track_from_playlist(
    context: ToolContext,
    zone_name: String,
    playlist_name: String,
    track_identifier: String,
) -> ToolResponse {
    debug!(
        "Removing track '{}' from playlist '{}' in zone '{}'",
        track_identifier, playlist_name, zone_name
    );

    let devices = context.context.devices.read().await;
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

    let command = format!(
        "playlist/remove/{}/{}",
        urlencoding::encode(&playlist_name),
        urlencoding::encode(&track_identifier)
    );

    match context.send_device_command(&device.uuid, &command).await {
        Ok(response) => ToolResponse::success(json!({
            "zone": device.name,
            "uuid": device.uuid,
            "action": "remove_track",
            "playlist_name": playlist_name,
            "track_identifier": track_identifier,
            "result": response.value,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
        Err(e) => ToolResponse::error(format!(
            "Failed to remove track from playlist in audio zone '{}': {}",
            device.name, e
        )),
    }
}

/// Get current playlist status and track information
pub async fn get_playlist_status(context: ToolContext, zone_name: String) -> ToolResponse {
    debug!("Getting playlist status for audio zone '{}'", zone_name);

    let devices = context.context.devices.read().await;
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

    // Extract playlist information from device states
    let current_playlist = device
        .states
        .get("current_playlist")
        .and_then(|v| v.as_str())
        .unwrap_or("none");

    let current_track = device
        .states
        .get("current_track")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let shuffle = device
        .states
        .get("shuffle")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let repeat_mode = device
        .states
        .get("repeat_mode")
        .and_then(|v| v.as_str())
        .map(RepeatMode::from)
        .unwrap_or(RepeatMode::Off);

    let playing = device
        .states
        .get("playing")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    ToolResponse::success(json!({
        "zone": device.name,
        "uuid": device.uuid,
        "current_playlist": current_playlist,
        "current_track": current_track,
        "shuffle": shuffle,
        "repeat_mode": repeat_mode,
        "playing": playing,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Helper function to normalize playlist action commands
fn normalize_playlist_action(action: &str) -> String {
    match action.to_lowercase().as_str() {
        // English commands
        "next" | "skip" => "next".to_string(),
        "previous" | "prev" | "back" => "previous".to_string(),
        "shuffle" | "random" => "shuffle".to_string(),
        "repeat" | "loop" => "repeat".to_string(),
        "clear" | "empty" => "clear".to_string(),
        "save" => "save".to_string(),
        "delete" | "remove" => "delete".to_string(),

        // German commands
        "weiter" | "nächster" => "next".to_string(),
        "zurück" | "vorheriger" => "previous".to_string(),
        "mischen" | "zufall" => "shuffle".to_string(),
        "wiederholen" | "schleife" => "repeat".to_string(),
        "leeren" | "löschen" => "clear".to_string(),
        "speichern" => "save".to_string(),
        "entfernen" => "delete".to_string(),

        // Direct passthrough for unknown commands
        _ => action.to_lowercase(),
    }
}

/// Serialize playlist data for commands
fn serialize_playlist(playlist: &AudioPlaylist) -> String {
    serde_json::to_string(playlist).unwrap_or_default()
}

/// Serialize track data for commands
fn serialize_track(track: &AudioTrack) -> String {
    serde_json::to_string(track).unwrap_or_default()
}

/// Helper function to create audio source information
#[allow(dead_code)]
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
