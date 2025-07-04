//! Comprehensive intercom system control tools
//!
//! This module provides complete intercom functionality for Loxone systems,
//! including call management, door access control, audio features, and monitoring.
//! For read-only intercom data, use resources:
//! - loxone://intercom/devices - Available intercom devices
//! - loxone://intercom/calls - Active and recent calls
//! - loxone://intercom/settings - System configuration

use crate::client::LoxoneDevice;
use crate::tools::{ToolContext, ToolResponse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::debug;

/// Intercom call state enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CallState {
    Idle,
    Incoming,
    Outgoing,
    Connected,
    Ringing,
    Busy,
    Ended,
    Missed,
}

impl From<&str> for CallState {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "idle" => CallState::Idle,
            "incoming" | "eingehend" => CallState::Incoming,
            "outgoing" | "ausgehend" => CallState::Outgoing,
            "connected" | "verbunden" => CallState::Connected,
            "ringing" | "klingelt" => CallState::Ringing,
            "busy" | "besetzt" => CallState::Busy,
            "ended" | "beendet" => CallState::Ended,
            "missed" | "verpasst" => CallState::Missed,
            _ => CallState::Idle,
        }
    }
}

/// Intercom device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntercomDevice {
    /// Device UUID
    pub uuid: String,
    /// Device name
    pub name: String,
    /// Device type (Intercom, DoorStation, etc.)
    pub device_type: String,
    /// Location/room
    pub location: Option<String>,
    /// Current call state
    pub call_state: CallState,
    /// Audio volume (0-100)
    pub volume: Option<f64>,
    /// Camera available
    pub has_camera: bool,
    /// Door lock control available
    pub has_door_control: bool,
    /// Motion detection available
    pub has_motion_detection: bool,
    /// Night vision available
    pub has_night_vision: bool,
}

/// Intercom call information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntercomCall {
    /// Call ID
    pub call_id: String,
    /// Source device UUID
    pub source_device: String,
    /// Target device UUID (if applicable)
    pub target_device: Option<String>,
    /// Call state
    pub state: CallState,
    /// Call start time
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// Call duration (for ended calls)
    pub duration: Option<u64>,
    /// Caller information
    pub caller_info: Option<String>,
    /// Has video
    pub has_video: bool,
}

/// Answer an incoming intercom call
pub async fn answer_call(
    context: ToolContext,
    device_name: String,
    enable_video: Option<bool>,
) -> ToolResponse {
    debug!(
        "Answering intercom call on device '{}' with video: {:?}",
        device_name, enable_video
    );

    match find_intercom_device(&context, &device_name).await {
        Ok(device) => {
            let video_flag = enable_video.unwrap_or(true);
            let command = if video_flag {
                "answer_video"
            } else {
                "answer_audio"
            };

            match context.send_device_command(&device.uuid, command).await {
                Ok(response) => ToolResponse::success(json!({
                    "device": device.name,
                    "uuid": device.uuid,
                    "action": "answer_call",
                    "video_enabled": video_flag,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to answer call on '{}': {}",
                    device.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// End/reject an active intercom call
pub async fn end_call(context: ToolContext, device_name: String) -> ToolResponse {
    debug!("Ending intercom call on device '{}'", device_name);

    match find_intercom_device(&context, &device_name).await {
        Ok(device) => match context.send_device_command(&device.uuid, "hangup").await {
            Ok(response) => ToolResponse::success(json!({
                "device": device.name,
                "uuid": device.uuid,
                "action": "end_call",
                "result": response.value,
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),
            Err(e) => {
                ToolResponse::error(format!("Failed to end call on '{}': {}", device.name, e))
            }
        },
        Err(e) => ToolResponse::error(e),
    }
}

/// Control door lock through intercom system
pub async fn control_door_lock(
    context: ToolContext,
    device_name: String,
    action: String,
    duration: Option<u32>,
) -> ToolResponse {
    debug!(
        "Controlling door lock on '{}' with action '{}' for {:?} seconds",
        device_name, action, duration
    );

    match find_intercom_device(&context, &device_name).await {
        Ok(device) => {
            if !device.has_door_control {
                return ToolResponse::error(format!(
                    "Device '{}' does not support door control",
                    device.name
                ));
            }

            let normalized_action = normalize_door_action(&action);
            let command = if let Some(dur) = duration {
                format!("{normalized_action}/{dur}")
            } else {
                normalized_action
            };

            match context.send_device_command(&device.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "device": device.name,
                    "uuid": device.uuid,
                    "action": "door_control",
                    "command": command,
                    "duration": duration,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to control door lock on '{}': {}",
                    device.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Set intercom audio volume
pub async fn set_intercom_volume(
    context: ToolContext,
    device_name: String,
    volume: f64,
) -> ToolResponse {
    debug!(
        "Setting intercom volume for '{}' to {}",
        device_name, volume
    );

    // Validate volume range
    if !(0.0..=100.0).contains(&volume) {
        return ToolResponse::error(format!(
            "Invalid volume level: {volume}. Valid range is 0-100"
        ));
    }

    match find_intercom_device(&context, &device_name).await {
        Ok(device) => {
            let command = format!("volume/{volume}");
            match context.send_device_command(&device.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "device": device.name,
                    "uuid": device.uuid,
                    "action": "set_volume",
                    "volume": volume,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => {
                    ToolResponse::error(format!("Failed to set volume on '{}': {}", device.name, e))
                }
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Start intercom call to specific device or room
pub async fn start_intercom_call(
    context: ToolContext,
    source_device: String,
    target: String,
    enable_video: Option<bool>,
) -> ToolResponse {
    debug!(
        "Starting intercom call from '{}' to '{}' with video: {:?}",
        source_device, target, enable_video
    );

    match find_intercom_device(&context, &source_device).await {
        Ok(device) => {
            let video_flag = enable_video.unwrap_or(true);
            let command = if video_flag {
                format!("call_video/{target}")
            } else {
                format!("call_audio/{target}")
            };

            match context.send_device_command(&device.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "source_device": device.name,
                    "source_uuid": device.uuid,
                    "target": target,
                    "action": "start_call",
                    "video_enabled": video_flag,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to start call from '{}' to '{}': {}",
                    device.name, target, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Control intercom camera settings
pub async fn control_intercom_camera(
    context: ToolContext,
    device_name: String,
    action: String,
    parameters: Option<HashMap<String, Value>>,
) -> ToolResponse {
    debug!(
        "Controlling intercom camera on '{}' with action '{}' and parameters: {:?}",
        device_name, action, parameters
    );

    match find_intercom_device(&context, &device_name).await {
        Ok(device) => {
            if !device.has_camera {
                return ToolResponse::error(format!(
                    "Device '{}' does not have camera functionality",
                    device.name
                ));
            }

            let normalized_action = normalize_camera_action(&action);
            let command = if let Some(ref params) = parameters {
                format!(
                    "{}/{}",
                    normalized_action,
                    serialize_camera_params(params.clone())
                )
            } else {
                normalized_action
            };

            match context.send_device_command(&device.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "device": device.name,
                    "uuid": device.uuid,
                    "action": "camera_control",
                    "command": command,
                    "parameters": parameters,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to control camera on '{}': {}",
                    device.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Enable/disable motion detection on intercom device
pub async fn control_motion_detection(
    context: ToolContext,
    device_name: String,
    enabled: bool,
    sensitivity: Option<u8>,
) -> ToolResponse {
    debug!(
        "Setting motion detection on '{}' to {} with sensitivity {:?}",
        device_name, enabled, sensitivity
    );

    match find_intercom_device(&context, &device_name).await {
        Ok(device) => {
            if !device.has_motion_detection {
                return ToolResponse::error(format!(
                    "Device '{}' does not support motion detection",
                    device.name
                ));
            }

            let command = if enabled {
                if let Some(sens) = sensitivity {
                    format!("motion_on/{}", sens.clamp(1, 100))
                } else {
                    "motion_on".to_string()
                }
            } else {
                "motion_off".to_string()
            };

            match context.send_device_command(&device.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "device": device.name,
                    "uuid": device.uuid,
                    "action": "motion_detection",
                    "enabled": enabled,
                    "sensitivity": sensitivity,
                    "command": command,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to control motion detection on '{}': {}",
                    device.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Get all available intercom devices with their current status
pub async fn get_intercom_devices(context: ToolContext) -> ToolResponse {
    debug!("Getting all intercom devices");

    let devices = context.context.devices.read().await;
    let mut intercom_devices = Vec::new();

    for device in devices.values() {
        if is_intercom_device(device) {
            let intercom_info = create_intercom_device_info(device, &context).await;
            intercom_devices.push(intercom_info);
        }
    }

    ToolResponse::success(json!({
        "devices": intercom_devices,
        "count": intercom_devices.len(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Enable/disable night vision on intercom camera
pub async fn control_night_vision(
    context: ToolContext,
    device_name: String,
    enabled: bool,
    auto_mode: Option<bool>,
) -> ToolResponse {
    debug!(
        "Setting night vision on '{}' to {} with auto mode: {:?}",
        device_name, enabled, auto_mode
    );

    match find_intercom_device(&context, &device_name).await {
        Ok(device) => {
            if !device.has_night_vision {
                return ToolResponse::error(format!(
                    "Device '{}' does not support night vision",
                    device.name
                ));
            }

            let command = match (enabled, auto_mode.unwrap_or(false)) {
                (true, true) => "nightvision_auto".to_string(),
                (true, false) => "nightvision_on".to_string(),
                (false, _) => "nightvision_off".to_string(),
            };

            match context.send_device_command(&device.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "device": device.name,
                    "uuid": device.uuid,
                    "action": "night_vision",
                    "enabled": enabled,
                    "auto_mode": auto_mode,
                    "command": command,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to control night vision on '{}': {}",
                    device.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

// Helper functions

/// Find intercom device by name or UUID
async fn find_intercom_device(
    context: &ToolContext,
    identifier: &str,
) -> Result<IntercomDevice, String> {
    let devices = context.context.devices.read().await;

    // First try exact UUID match
    if let Some(device) = devices.get(identifier) {
        if is_intercom_device(device) {
            return Ok(create_intercom_device_info(device, context).await);
        }
    }

    // Then try name matching
    for device in devices.values() {
        if is_intercom_device(device)
            && device
                .name
                .to_lowercase()
                .contains(&identifier.to_lowercase())
        {
            return Ok(create_intercom_device_info(device, context).await);
        }
    }

    Err(format!(
        "Intercom device '{identifier}' not found. Use get_intercom_devices to see available devices"
    ))
}

/// Check if a device is an intercom device
fn is_intercom_device(device: &LoxoneDevice) -> bool {
    let intercom_types = [
        "Intercom",
        "DoorStation",
        "VideoIntercom",
        "AudioIntercom",
        "Door",
        "Gate",
        "Entrance",
    ];
    let intercom_keywords = [
        "intercom",
        "sprechanlage",
        "türstation",
        "door",
        "tür",
        "entrance",
        "eingang",
        "gate",
        "tor",
        "video",
        "kamera",
    ];

    // Check by type
    if intercom_types
        .iter()
        .any(|&t| device.device_type.contains(t))
    {
        return true;
    }

    // Check by name keywords
    let device_name = device.name.to_lowercase();
    intercom_keywords
        .iter()
        .any(|&keyword| device_name.contains(keyword))
}

/// Create intercom device info from Loxone device
async fn create_intercom_device_info(
    device: &LoxoneDevice,
    _context: &ToolContext,
) -> IntercomDevice {
    // Determine device capabilities based on type and states
    let has_camera = device.device_type.contains("Video")
        || device.states.contains_key("camera")
        || device.name.to_lowercase().contains("video");

    let has_door_control = device.states.contains_key("lock")
        || device.states.contains_key("door")
        || device.device_type.contains("Door");

    let has_motion_detection =
        device.states.contains_key("motion") || device.states.contains_key("pir");

    let has_night_vision = has_camera
        && (device.states.contains_key("nightvision") || device.states.contains_key("ir"));

    // Extract current state information
    let call_state = device
        .states
        .get("state")
        .or_else(|| device.states.get("call_state"))
        .and_then(|v| v.as_str())
        .map(CallState::from)
        .unwrap_or(CallState::Idle);

    let volume = device.states.get("volume").and_then(|v| v.as_f64());

    IntercomDevice {
        uuid: device.uuid.clone(),
        name: device.name.clone(),
        device_type: device.device_type.clone(),
        location: device.room.clone(),
        call_state,
        volume,
        has_camera,
        has_door_control,
        has_motion_detection,
        has_night_vision,
    }
}

/// Normalize door action commands
fn normalize_door_action(action: &str) -> String {
    match action.to_lowercase().as_str() {
        // English commands
        "unlock" | "open" => "unlock".to_string(),
        "lock" | "close" => "lock".to_string(),
        "toggle" => "toggle".to_string(),
        "pulse" | "trigger" => "pulse".to_string(),

        // German commands
        "öffnen" | "aufschließen" | "entriegeln" => "unlock".to_string(),
        "schließen" | "abschließen" | "verriegeln" => "lock".to_string(),
        "umschalten" => "toggle".to_string(),
        "auslösen" | "impuls" => "pulse".to_string(),

        // Default passthrough
        _ => action.to_lowercase(),
    }
}

/// Normalize camera action commands
fn normalize_camera_action(action: &str) -> String {
    match action.to_lowercase().as_str() {
        // English commands
        "start" | "on" => "camera_on".to_string(),
        "stop" | "off" => "camera_off".to_string(),
        "snapshot" | "capture" => "snapshot".to_string(),
        "record" => "record".to_string(),
        "zoom_in" => "zoom_in".to_string(),
        "zoom_out" => "zoom_out".to_string(),
        "pan_left" => "pan_left".to_string(),
        "pan_right" => "pan_right".to_string(),
        "tilt_up" => "tilt_up".to_string(),
        "tilt_down" => "tilt_down".to_string(),

        // German commands
        "einschalten" | "an" => "camera_on".to_string(),
        "ausschalten" | "aus" => "camera_off".to_string(),
        "schnappschuss" | "aufnahme" => "snapshot".to_string(),
        "aufzeichnen" => "record".to_string(),
        "zoom_rein" => "zoom_in".to_string(),
        "zoom_raus" => "zoom_out".to_string(),
        "links" => "pan_left".to_string(),
        "rechts" => "pan_right".to_string(),
        "hoch" => "tilt_up".to_string(),
        "runter" => "tilt_down".to_string(),

        // Default passthrough
        _ => action.to_lowercase(),
    }
}

/// Serialize camera parameters for command
fn serialize_camera_params(params: HashMap<String, Value>) -> String {
    params
        .iter()
        .map(|(k, v)| {
            let value_str = match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => serde_json::to_string(v).unwrap_or_default(),
            };
            format!("{k}={value_str}")
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// Broadcast announcement to all intercom devices
pub async fn broadcast_announcement(
    context: ToolContext,
    message: String,
    duration: Option<u32>,
    priority: Option<String>,
) -> ToolResponse {
    debug!(
        "Broadcasting announcement: '{}' for {:?} seconds with priority {:?}",
        message, duration, priority
    );

    let devices = context.context.devices.read().await;
    let mut intercom_devices = Vec::new();
    let mut results = Vec::new();

    // Find all intercom devices
    for device in devices.values() {
        if is_intercom_device(device) {
            intercom_devices.push(device.clone());
        }
    }

    if intercom_devices.is_empty() {
        return ToolResponse::error("No intercom devices found for broadcast".to_string());
    }

    // Prepare broadcast command
    let priority_level = match priority.as_deref() {
        Some("high") | Some("urgent") => "urgent",
        Some("medium") | Some("normal") => "normal",
        Some("low") => "low",
        _ => "normal",
    };

    let duration_secs = duration.unwrap_or(10);
    let command = format!(
        "announce/{}/{}/{}",
        urlencoding::encode(&message),
        duration_secs,
        priority_level
    );

    // Send to all devices in parallel
    let commands: Vec<(String, String)> = intercom_devices
        .iter()
        .map(|device| (device.uuid.clone(), command.clone()))
        .collect();

    match context.send_parallel_commands(commands).await {
        Ok(responses) => {
            for (device, result) in intercom_devices.iter().zip(responses.iter()) {
                results.push(json!({
                    "device": device.name,
                    "uuid": device.uuid,
                    "success": result.is_ok(),
                    "result": match result {
                        Ok(resp) => resp.value.clone(),
                        Err(e) => json!(format!("Error: {}", e)),
                    }
                }));
            }

            ToolResponse::success(json!({
                "action": "broadcast_announcement",
                "message": message,
                "duration": duration_secs,
                "priority": priority_level,
                "devices_count": intercom_devices.len(),
                "results": results,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))
        }
        Err(e) => ToolResponse::error(format!("Failed to broadcast announcement: {e}")),
    }
}

/// Get intercom call history and statistics
pub async fn get_intercom_call_history(
    context: ToolContext,
    device_name: Option<String>,
    limit: Option<usize>,
) -> ToolResponse {
    debug!(
        "Getting intercom call history for device: {:?}, limit: {:?}",
        device_name, limit
    );

    // This is a placeholder implementation - in a real system, call history
    // would be retrieved from the Loxone system or a separate logging service
    let call_limit = limit.unwrap_or(50);

    if let Some(device) = device_name {
        match find_intercom_device(&context, &device).await {
            Ok(intercom_device) => {
                // In a real implementation, this would query the actual call history
                ToolResponse::success(json!({
                    "device": intercom_device.name,
                    "uuid": intercom_device.uuid,
                    "call_history": [],
                    "total_calls": 0,
                    "limit": call_limit,
                    "message": "Call history feature requires Loxone logging service integration",
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))
            }
            Err(e) => ToolResponse::error(e),
        }
    } else {
        // Get history for all devices
        ToolResponse::success(json!({
            "call_history": [],
            "total_calls": 0,
            "limit": call_limit,
            "message": "Call history feature requires Loxone logging service integration",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }
}

/// Configure intercom system settings
pub async fn configure_intercom_settings(
    context: ToolContext,
    device_name: String,
    settings: HashMap<String, Value>,
) -> ToolResponse {
    debug!(
        "Configuring intercom settings for '{}': {:?}",
        device_name, settings
    );

    match find_intercom_device(&context, &device_name).await {
        Ok(device) => {
            let mut results = Vec::new();
            let mut success_count = 0;

            for (setting, value) in settings.iter() {
                let command = format!("config/{}/{}", setting, serialize_setting_value(value));

                match context.send_device_command(&device.uuid, &command).await {
                    Ok(response) => {
                        results.push(json!({
                            "setting": setting,
                            "value": value,
                            "success": true,
                            "result": response.value
                        }));
                        success_count += 1;
                    }
                    Err(e) => {
                        results.push(json!({
                            "setting": setting,
                            "value": value,
                            "success": false,
                            "error": e.to_string()
                        }));
                    }
                }
            }

            ToolResponse::success(json!({
                "device": device.name,
                "uuid": device.uuid,
                "action": "configure_settings",
                "total_settings": settings.len(),
                "successful": success_count,
                "failed": settings.len() - success_count,
                "results": results,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Emergency intercom functions - panic button, emergency calls
pub async fn trigger_emergency_call(
    context: ToolContext,
    device_name: String,
    emergency_type: String,
    message: Option<String>,
) -> ToolResponse {
    debug!(
        "Triggering emergency call from '{}' type '{}' with message: {:?}",
        device_name, emergency_type, message
    );

    match find_intercom_device(&context, &device_name).await {
        Ok(device) => {
            let emergency_msg =
                message.unwrap_or_else(|| "Emergency assistance required".to_string());
            let command = format!(
                "emergency/{}/{}",
                emergency_type.to_lowercase(),
                urlencoding::encode(&emergency_msg)
            );

            match context.send_device_command(&device.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "device": device.name,
                    "uuid": device.uuid,
                    "action": "emergency_call",
                    "emergency_type": emergency_type,
                    "message": emergency_msg,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "priority": "urgent"
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to trigger emergency call on '{}': {}",
                    device.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

// Additional helper functions

/// Serialize setting value for configuration commands
fn serialize_setting_value(value: &Value) -> String {
    match value {
        Value::String(s) => urlencoding::encode(s).to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(arr) => arr
            .iter()
            .map(serialize_setting_value)
            .collect::<Vec<_>>()
            .join(","),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

/// Create comprehensive intercom system status
pub async fn get_intercom_system_status(context: ToolContext) -> ToolResponse {
    debug!("Getting comprehensive intercom system status");

    let devices = context.context.devices.read().await;
    let mut device_stats = HashMap::new();
    let mut total_devices = 0;
    let mut active_calls = 0;
    let mut devices_with_camera = 0;
    let mut devices_with_door_control = 0;

    for device in devices.values() {
        if is_intercom_device(device) {
            total_devices += 1;
            let intercom_info = create_intercom_device_info(device, &context).await;

            if intercom_info.call_state != CallState::Idle {
                active_calls += 1;
            }
            if intercom_info.has_camera {
                devices_with_camera += 1;
            }
            if intercom_info.has_door_control {
                devices_with_door_control += 1;
            }

            // Count by location
            let location = intercom_info
                .location
                .unwrap_or_else(|| "Unknown".to_string());
            *device_stats.entry(location).or_insert(0) += 1;
        }
    }

    ToolResponse::success(json!({
        "system_status": {
            "total_devices": total_devices,
            "active_calls": active_calls,
            "devices_with_camera": devices_with_camera,
            "devices_with_door_control": devices_with_door_control,
            "devices_by_location": device_stats,
            "system_health": if total_devices > 0 { "operational" } else { "no_devices" }
        },
        "capabilities": {
            "video_calling": devices_with_camera > 0,
            "door_access_control": devices_with_door_control > 0,
            "multi_location": device_stats.len() > 1,
            "broadcast_support": total_devices > 1
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
