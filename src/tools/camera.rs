//! Camera control and video streaming tools
//!
//! This module provides comprehensive camera management for Loxone systems,
//! including PTZ controls, streaming, recording, motion detection, and analytics.
//! For read-only camera data, use resources:
//! - loxone://camera/devices - Available camera devices
//! - loxone://camera/streams - Active video streams
//! - loxone://camera/recordings - Recording status and history

use crate::client::LoxoneDevice;
use crate::tools::{ToolContext, ToolResponse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::debug;

/// Camera type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CameraType {
    /// IP Camera
    IpCamera,
    /// Analog Camera
    AnalogCamera,
    /// PTZ Camera (Pan-Tilt-Zoom)
    PtzCamera,
    /// Dome Camera
    DomeCamera,
    /// Bullet Camera
    BulletCamera,
    /// Thermal Camera
    ThermalCamera,
    /// Webcam
    Webcam,
    /// Door Station Camera
    DoorStation,
    /// Unknown camera type
    Unknown,
}

impl From<&str> for CameraType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            s if s.contains("ip") => CameraType::IpCamera,
            s if s.contains("analog") => CameraType::AnalogCamera,
            s if s.contains("ptz") => CameraType::PtzCamera,
            s if s.contains("dome") => CameraType::DomeCamera,
            s if s.contains("bullet") => CameraType::BulletCamera,
            s if s.contains("thermal") => CameraType::ThermalCamera,
            s if s.contains("webcam") => CameraType::Webcam,
            s if s.contains("door") => CameraType::DoorStation,
            _ => CameraType::Unknown,
        }
    }
}

/// Camera recording state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RecordingState {
    Stopped,
    Recording,
    Paused,
    Scheduled,
    MotionTriggered,
    ManualTriggered,
    Error,
}

impl From<&str> for RecordingState {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "recording" | "aufzeichnung" => RecordingState::Recording,
            "paused" | "pausiert" => RecordingState::Paused,
            "scheduled" | "geplant" => RecordingState::Scheduled,
            "motion" | "bewegung" => RecordingState::MotionTriggered,
            "manual" | "manuell" => RecordingState::ManualTriggered,
            "error" | "fehler" => RecordingState::Error,
            _ => RecordingState::Stopped,
        }
    }
}

/// Camera device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraDevice {
    /// Device UUID
    pub uuid: String,
    /// Device name
    pub name: String,
    /// Camera type
    pub camera_type: CameraType,
    /// Location/room
    pub location: Option<String>,
    /// Camera resolution (e.g., "1920x1080")
    pub resolution: Option<String>,
    /// Current recording state
    pub recording_state: RecordingState,
    /// Pan-Tilt-Zoom capabilities
    pub has_ptz: bool,
    /// Motion detection enabled
    pub motion_detection: bool,
    /// Night vision available
    pub has_night_vision: bool,
    /// Audio recording capability
    pub has_audio: bool,
    /// Streaming URL (if available)
    pub stream_url: Option<String>,
    /// Camera status
    pub status: String,
    /// Current settings
    pub settings: HashMap<String, Value>,
}

/// Video stream information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStream {
    /// Stream ID
    pub stream_id: String,
    /// Camera UUID
    pub camera_uuid: String,
    /// Stream URL
    pub url: String,
    /// Stream format (RTSP, HTTP, WebRTC, etc.)
    pub format: String,
    /// Resolution
    pub resolution: String,
    /// Frame rate
    pub fps: u32,
    /// Bitrate (kbps)
    pub bitrate: u32,
    /// Stream quality
    pub quality: String,
    /// Active viewers count
    pub viewer_count: u32,
    /// Stream start time
    pub start_time: chrono::DateTime<chrono::Utc>,
}

/// Start video streaming from a camera
pub async fn start_camera_stream(
    context: ToolContext,
    camera_name: String,
    quality: Option<String>,
    format: Option<String>,
) -> ToolResponse {
    debug!(
        "Starting video stream from camera '{}' with quality: {:?}, format: {:?}",
        camera_name, quality, format
    );

    match find_camera_device(&context, &camera_name).await {
        Ok(camera) => {
            let stream_quality = quality.unwrap_or_else(|| "medium".to_string());
            let stream_format = format.unwrap_or_else(|| "rtsp".to_string());

            let command = format!("stream/start/{stream_quality}/{stream_format}");

            match context.send_device_command(&camera.uuid, &command).await {
                Ok(response) => {
                    let stream_url =
                        extract_stream_url(&response.value, &camera.uuid, &stream_format);
                    ToolResponse::success(json!({
                        "camera": camera.name,
                        "uuid": camera.uuid,
                        "action": "start_stream",
                        "quality": stream_quality,
                        "format": stream_format,
                        "stream_url": stream_url,
                        "result": response.value,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }))
                }
                Err(e) => ToolResponse::error(format!(
                    "Failed to start stream on camera '{}': {}",
                    camera.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Stop video streaming from a camera
pub async fn stop_camera_stream(context: ToolContext, camera_name: String) -> ToolResponse {
    debug!("Stopping video stream from camera '{}'", camera_name);

    match find_camera_device(&context, &camera_name).await {
        Ok(camera) => {
            match context
                .send_device_command(&camera.uuid, "stream/stop")
                .await
            {
                Ok(response) => ToolResponse::success(json!({
                    "camera": camera.name,
                    "uuid": camera.uuid,
                    "action": "stop_stream",
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to stop stream on camera '{}': {}",
                    camera.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Control PTZ (Pan-Tilt-Zoom) camera movements
pub async fn control_ptz_camera(
    context: ToolContext,
    camera_name: String,
    action: String,
    speed: Option<u8>,
    position: Option<HashMap<String, f64>>,
) -> ToolResponse {
    debug!(
        "Controlling PTZ camera '{}' with action '{}', speed: {:?}, position: {:?}",
        camera_name, action, speed, position
    );

    match find_camera_device(&context, &camera_name).await {
        Ok(camera) => {
            if !camera.has_ptz {
                return ToolResponse::error(format!(
                    "Camera '{}' does not support PTZ controls",
                    camera.name
                ));
            }

            let normalized_action = normalize_ptz_action(&action);
            let ptz_speed = speed.unwrap_or(50).clamp(1, 100);

            let command = if let Some(ref pos) = position {
                format!(
                    "{}/{}/{}",
                    normalized_action,
                    ptz_speed,
                    serialize_position(pos.clone())
                )
            } else {
                format!("{normalized_action}/{ptz_speed}")
            };

            match context.send_device_command(&camera.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "camera": camera.name,
                    "uuid": camera.uuid,
                    "action": "ptz_control",
                    "command": normalized_action,
                    "speed": ptz_speed,
                    "position": position,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to control PTZ on camera '{}': {}",
                    camera.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Start/stop camera recording
pub async fn control_camera_recording(
    context: ToolContext,
    camera_name: String,
    action: String,
    duration: Option<u32>,
    quality: Option<String>,
) -> ToolResponse {
    debug!(
        "Controlling recording on camera '{}' with action '{}', duration: {:?}, quality: {:?}",
        camera_name, action, duration, quality
    );

    match find_camera_device(&context, &camera_name).await {
        Ok(camera) => {
            let normalized_action = normalize_recording_action(&action);
            let record_quality = quality.unwrap_or_else(|| "high".to_string());

            let command = match normalized_action.as_str() {
                "start" => {
                    if let Some(dur) = duration {
                        format!("record/start/{record_quality}/{dur}")
                    } else {
                        format!("record/start/{record_quality}")
                    }
                }
                "stop" => "record/stop".to_string(),
                "pause" => "record/pause".to_string(),
                "resume" => "record/resume".to_string(),
                _ => normalized_action.clone(),
            };

            match context.send_device_command(&camera.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "camera": camera.name,
                    "uuid": camera.uuid,
                    "action": "recording_control",
                    "command": normalized_action,
                    "duration": duration,
                    "quality": record_quality,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to control recording on camera '{}': {}",
                    camera.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Capture snapshot from camera
pub async fn capture_camera_snapshot(
    context: ToolContext,
    camera_name: String,
    quality: Option<String>,
    save_location: Option<String>,
) -> ToolResponse {
    debug!(
        "Capturing snapshot from camera '{}' with quality: {:?}, save location: {:?}",
        camera_name, quality, save_location
    );

    match find_camera_device(&context, &camera_name).await {
        Ok(camera) => {
            let snap_quality = quality.unwrap_or_else(|| "high".to_string());
            let location = save_location.unwrap_or_else(|| "default".to_string());

            let command = format!(
                "snapshot/{}/{}",
                snap_quality,
                urlencoding::encode(&location)
            );

            match context.send_device_command(&camera.uuid, &command).await {
                Ok(response) => {
                    let snapshot_url = extract_snapshot_url(&response.value, &camera.uuid);
                    ToolResponse::success(json!({
                        "camera": camera.name,
                        "uuid": camera.uuid,
                        "action": "capture_snapshot",
                        "quality": snap_quality,
                        "save_location": location,
                        "snapshot_url": snapshot_url,
                        "result": response.value,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }))
                }
                Err(e) => ToolResponse::error(format!(
                    "Failed to capture snapshot from camera '{}': {}",
                    camera.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Configure camera settings
pub async fn configure_camera_settings(
    context: ToolContext,
    camera_name: String,
    settings: HashMap<String, Value>,
) -> ToolResponse {
    debug!(
        "Configuring camera settings for '{}': {:?}",
        camera_name, settings
    );

    match find_camera_device(&context, &camera_name).await {
        Ok(camera) => {
            let mut results = Vec::new();
            let mut success_count = 0;

            for (setting, value) in settings.iter() {
                let command = format!("config/{}/{}", setting, serialize_setting_value(value));

                match context.send_device_command(&camera.uuid, &command).await {
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
                "camera": camera.name,
                "uuid": camera.uuid,
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

/// Enable/disable motion detection on camera
pub async fn control_motion_detection(
    context: ToolContext,
    camera_name: String,
    enabled: bool,
    sensitivity: Option<u8>,
    zones: Option<Vec<HashMap<String, Value>>>,
) -> ToolResponse {
    debug!(
        "Setting motion detection on camera '{}' to {} with sensitivity {:?} and zones: {:?}",
        camera_name, enabled, sensitivity, zones
    );

    match find_camera_device(&context, &camera_name).await {
        Ok(camera) => {
            let command = if enabled {
                let sens = sensitivity.unwrap_or(50).clamp(1, 100);
                if let Some(ref detection_zones) = zones {
                    format!(
                        "motion/enable/{}/{}",
                        sens,
                        serialize_motion_zones(detection_zones.clone())
                    )
                } else {
                    format!("motion/enable/{sens}")
                }
            } else {
                "motion/disable".to_string()
            };

            match context.send_device_command(&camera.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "camera": camera.name,
                    "uuid": camera.uuid,
                    "action": "motion_detection",
                    "enabled": enabled,
                    "sensitivity": sensitivity,
                    "zones": zones,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to control motion detection on camera '{}': {}",
                    camera.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Control camera night vision
pub async fn control_night_vision(
    context: ToolContext,
    camera_name: String,
    mode: String,
    sensitivity: Option<u8>,
) -> ToolResponse {
    debug!(
        "Setting night vision on camera '{}' to mode '{}' with sensitivity: {:?}",
        camera_name, mode, sensitivity
    );

    match find_camera_device(&context, &camera_name).await {
        Ok(camera) => {
            if !camera.has_night_vision {
                return ToolResponse::error(format!(
                    "Camera '{}' does not support night vision",
                    camera.name
                ));
            }

            let normalized_mode = normalize_night_vision_mode(&mode);
            let command = if let Some(sens) = sensitivity {
                format!("nightvision/{}/{}", normalized_mode, sens.clamp(1, 100))
            } else {
                format!("nightvision/{normalized_mode}")
            };

            match context.send_device_command(&camera.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "camera": camera.name,
                    "uuid": camera.uuid,
                    "action": "night_vision",
                    "mode": normalized_mode,
                    "sensitivity": sensitivity,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to control night vision on camera '{}': {}",
                    camera.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Get all available camera devices
pub async fn get_camera_devices(context: ToolContext) -> ToolResponse {
    debug!("Getting all camera devices");

    let devices = context.context.devices.read().await;
    let mut camera_devices = Vec::new();

    for device in devices.values() {
        if is_camera_device(device) {
            let camera_info = create_camera_device_info(device, &context).await;
            camera_devices.push(camera_info);
        }
    }

    ToolResponse::success(json!({
        "cameras": camera_devices,
        "count": camera_devices.len(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Get active video streams
pub async fn get_active_streams(context: ToolContext) -> ToolResponse {
    debug!("Getting active video streams");

    let devices = context.context.devices.read().await;
    let mut active_streams = Vec::new();

    for device in devices.values() {
        if is_camera_device(device) {
            // Check if camera has active stream
            if let Some(stream_state) = device.states.get("stream_state") {
                if stream_state.as_str() == Some("active") {
                    let stream_info = create_stream_info(device);
                    active_streams.push(stream_info);
                }
            }
        }
    }

    ToolResponse::success(json!({
        "active_streams": active_streams,
        "count": active_streams.len(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Set camera preset position
pub async fn set_camera_preset(
    context: ToolContext,
    camera_name: String,
    preset_name: String,
    position: Option<HashMap<String, f64>>,
) -> ToolResponse {
    debug!(
        "Setting camera preset '{}' on camera '{}' with position: {:?}",
        preset_name, camera_name, position
    );

    match find_camera_device(&context, &camera_name).await {
        Ok(camera) => {
            if !camera.has_ptz {
                return ToolResponse::error(format!(
                    "Camera '{}' does not support presets (no PTZ capability)",
                    camera.name
                ));
            }

            let command = if let Some(ref pos) = position {
                format!(
                    "preset/set/{}/{}",
                    urlencoding::encode(&preset_name),
                    serialize_position(pos.clone())
                )
            } else {
                format!("preset/set/{}/current", urlencoding::encode(&preset_name))
            };

            match context.send_device_command(&camera.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "camera": camera.name,
                    "uuid": camera.uuid,
                    "action": "set_preset",
                    "preset_name": preset_name,
                    "position": position,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to set preset on camera '{}': {}",
                    camera.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

/// Go to camera preset position
pub async fn goto_camera_preset(
    context: ToolContext,
    camera_name: String,
    preset_name: String,
    speed: Option<u8>,
) -> ToolResponse {
    debug!(
        "Moving camera '{}' to preset '{}' with speed: {:?}",
        camera_name, preset_name, speed
    );

    match find_camera_device(&context, &camera_name).await {
        Ok(camera) => {
            if !camera.has_ptz {
                return ToolResponse::error(format!(
                    "Camera '{}' does not support presets (no PTZ capability)",
                    camera.name
                ));
            }

            let preset_speed = speed.unwrap_or(50).clamp(1, 100);
            let command = format!(
                "preset/goto/{}/{}",
                urlencoding::encode(&preset_name),
                preset_speed
            );

            match context.send_device_command(&camera.uuid, &command).await {
                Ok(response) => ToolResponse::success(json!({
                    "camera": camera.name,
                    "uuid": camera.uuid,
                    "action": "goto_preset",
                    "preset_name": preset_name,
                    "speed": preset_speed,
                    "result": response.value,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })),
                Err(e) => ToolResponse::error(format!(
                    "Failed to go to preset on camera '{}': {}",
                    camera.name, e
                )),
            }
        }
        Err(e) => ToolResponse::error(e),
    }
}

// Helper functions

/// Find camera device by name or UUID
async fn find_camera_device(
    context: &ToolContext,
    identifier: &str,
) -> Result<CameraDevice, String> {
    let devices = context.context.devices.read().await;

    // First try exact UUID match
    if let Some(device) = devices.get(identifier) {
        if is_camera_device(device) {
            return Ok(create_camera_device_info(device, context).await);
        }
    }

    // Then try name matching
    for device in devices.values() {
        if is_camera_device(device)
            && device
                .name
                .to_lowercase()
                .contains(&identifier.to_lowercase())
        {
            return Ok(create_camera_device_info(device, context).await);
        }
    }

    Err(format!(
        "Camera device '{identifier}' not found. Use get_camera_devices to see available cameras"
    ))
}

/// Check if a device is a camera device
fn is_camera_device(device: &LoxoneDevice) -> bool {
    let camera_types = [
        "Camera",
        "IPCamera",
        "AnalogCamera",
        "PTZCamera",
        "DomeCamera",
        "BulletCamera",
        "ThermalCamera",
        "Webcam",
        "DoorStation",
        "VideoIntercom",
    ];
    let camera_keywords = [
        "camera",
        "kamera",
        "cam",
        "video",
        "surveillance",
        "überwachung",
        "security",
        "sicherheit",
        "ptz",
        "dome",
        "bullet",
        "thermal",
    ];

    // Check by type
    if camera_types.iter().any(|&t| device.device_type.contains(t)) {
        return true;
    }

    // Check by name keywords
    let device_name = device.name.to_lowercase();
    camera_keywords
        .iter()
        .any(|&keyword| device_name.contains(keyword))
}

/// Create camera device info from Loxone device
async fn create_camera_device_info(device: &LoxoneDevice, _context: &ToolContext) -> CameraDevice {
    // Determine camera type from device type and name
    let camera_type = CameraType::from(device.device_type.as_str());

    // Determine capabilities based on type and states
    let has_ptz = device.device_type.contains("PTZ")
        || device.states.contains_key("pan")
        || device.states.contains_key("tilt")
        || device.states.contains_key("zoom");

    let motion_detection = device
        .states
        .get("motion_detection")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let has_night_vision = device.states.contains_key("nightvision")
        || device.states.contains_key("ir")
        || device.device_type.contains("Night");

    let has_audio = device.states.contains_key("audio") || device.device_type.contains("Audio");

    // Extract current recording state
    let recording_state = device
        .states
        .get("recording_state")
        .or_else(|| device.states.get("recording"))
        .and_then(|v| v.as_str())
        .map(RecordingState::from)
        .unwrap_or(RecordingState::Stopped);

    // Extract resolution
    let resolution = device
        .states
        .get("resolution")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Extract stream URL
    let stream_url = device
        .states
        .get("stream_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Extract status
    let status = device
        .states
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // Create settings map from device states
    let mut settings = HashMap::new();
    for (key, value) in &device.states {
        if key.starts_with("setting_") || key.starts_with("config_") {
            settings.insert(key.clone(), value.clone());
        }
    }

    CameraDevice {
        uuid: device.uuid.clone(),
        name: device.name.clone(),
        camera_type,
        location: device.room.clone(),
        resolution,
        recording_state,
        has_ptz,
        motion_detection,
        has_night_vision,
        has_audio,
        stream_url,
        status,
        settings,
    }
}

/// Create stream info from device
fn create_stream_info(device: &LoxoneDevice) -> VideoStream {
    let stream_id = format!("stream_{}", device.uuid);
    let url = device
        .states
        .get("stream_url")
        .and_then(|v| v.as_str())
        .unwrap_or("rtsp://unknown")
        .to_string();

    let format = device
        .states
        .get("stream_format")
        .and_then(|v| v.as_str())
        .unwrap_or("rtsp")
        .to_string();

    let resolution = device
        .states
        .get("resolution")
        .and_then(|v| v.as_str())
        .unwrap_or("1920x1080")
        .to_string();

    let fps = device
        .states
        .get("fps")
        .and_then(|v| v.as_u64())
        .unwrap_or(30) as u32;

    let bitrate = device
        .states
        .get("bitrate")
        .and_then(|v| v.as_u64())
        .unwrap_or(2000) as u32;

    let quality = device
        .states
        .get("quality")
        .and_then(|v| v.as_str())
        .unwrap_or("medium")
        .to_string();

    let viewer_count = device
        .states
        .get("viewer_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    VideoStream {
        stream_id,
        camera_uuid: device.uuid.clone(),
        url,
        format,
        resolution,
        fps,
        bitrate,
        quality,
        viewer_count,
        start_time: chrono::Utc::now(), // Placeholder - would be actual start time
    }
}

/// Normalize PTZ action commands
fn normalize_ptz_action(action: &str) -> String {
    match action.to_lowercase().as_str() {
        // English commands
        "pan_left" | "left" => "pan_left".to_string(),
        "pan_right" | "right" => "pan_right".to_string(),
        "tilt_up" | "up" => "tilt_up".to_string(),
        "tilt_down" | "down" => "tilt_down".to_string(),
        "zoom_in" | "zoom+" => "zoom_in".to_string(),
        "zoom_out" | "zoom-" => "zoom_out".to_string(),
        "stop" => "stop".to_string(),
        "home" | "center" => "home".to_string(),

        // German commands
        "links" => "pan_left".to_string(),
        "rechts" => "pan_right".to_string(),
        "hoch" => "tilt_up".to_string(),
        "runter" => "tilt_down".to_string(),
        "zoom_rein" => "zoom_in".to_string(),
        "zoom_raus" => "zoom_out".to_string(),
        "stopp" => "stop".to_string(),
        "mitte" | "zentrum" => "home".to_string(),

        // Default passthrough
        _ => action.to_lowercase(),
    }
}

/// Normalize recording action commands
fn normalize_recording_action(action: &str) -> String {
    match action.to_lowercase().as_str() {
        // English commands
        "start" | "begin" | "record" => "start".to_string(),
        "stop" | "end" => "stop".to_string(),
        "pause" => "pause".to_string(),
        "resume" | "continue" => "resume".to_string(),

        // German commands
        "starten" | "beginnen" | "aufzeichnen" => "start".to_string(),
        "stoppen" | "beenden" => "stop".to_string(),
        "pausieren" => "pause".to_string(),
        "fortsetzen" | "weiter" => "resume".to_string(),

        // Default passthrough
        _ => action.to_lowercase(),
    }
}

/// Normalize night vision mode
fn normalize_night_vision_mode(mode: &str) -> String {
    match mode.to_lowercase().as_str() {
        "on" | "enable" | "manual" => "on".to_string(),
        "off" | "disable" => "off".to_string(),
        "auto" | "automatic" => "auto".to_string(),

        // German commands
        "an" | "ein" | "manuell" => "on".to_string(),
        "aus" => "off".to_string(),
        "automatisch" => "auto".to_string(),

        // Default passthrough
        _ => mode.to_lowercase(),
    }
}

/// Serialize position coordinates for PTZ commands
fn serialize_position(position: HashMap<String, f64>) -> String {
    position
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// Serialize motion detection zones
fn serialize_motion_zones(zones: Vec<HashMap<String, Value>>) -> String {
    zones
        .iter()
        .enumerate()
        .map(|(i, zone)| {
            format!(
                "zone{}={}",
                i,
                zone.iter()
                    .map(|(k, v)| format!("{}={}", k, serialize_setting_value(v)))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

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

/// Extract stream URL from response
fn extract_stream_url(response: &Value, camera_uuid: &str, format: &str) -> String {
    // Try to extract URL from response
    if let Some(url) = response.get("stream_url").and_then(|v| v.as_str()) {
        return url.to_string();
    }

    // Generate default stream URL based on format
    match format.to_lowercase().as_str() {
        "rtsp" => format!("rtsp://loxone/camera/{camera_uuid}/stream"),
        "http" => format!("http://loxone/camera/{camera_uuid}/stream.mjpeg"),
        "webrtc" => format!("webrtc://loxone/camera/{camera_uuid}/stream"),
        _ => format!("stream://loxone/camera/{camera_uuid}"),
    }
}

/// Extract snapshot URL from response
fn extract_snapshot_url(response: &Value, camera_uuid: &str) -> String {
    // Try to extract URL from response
    if let Some(url) = response.get("snapshot_url").and_then(|v| v.as_str()) {
        return url.to_string();
    }

    // Generate default snapshot URL
    format!("http://loxone/camera/{camera_uuid}/snapshot.jpg")
}
