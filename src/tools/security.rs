//! Advanced Security System Control Tools
//!
//! Comprehensive security system management including:
//! - Alarm systems and zone control
//! - Door locks and access control
//! - Security cameras and monitoring
//! - Motion detectors and sensors
//! - Access codes and user management
//! - Security system integration
//!
//! For read-only security data, use resources:
//! - loxone://security/status - Security system status
//! - loxone://security/zones - Security zones
//! - loxone://security/access_logs - Access logs and events
//! - loxone://security/cameras - Camera status and feeds

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::client::LoxoneDevice;
use crate::error::LoxoneError;
use crate::tools::ToolContext;

/// Security device types supported by Loxone
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SecurityDeviceType {
    /// Door lock with smart access control
    DoorLock,
    /// Security alarm system
    AlarmSystem,
    /// Motion detector sensor
    MotionDetector,
    /// Door/window contact sensor
    ContactSensor,
    /// Security camera
    Camera,
    /// Access control panel
    AccessPanel,
    /// Keypad for code entry
    Keypad,
    /// RFID reader
    RfidReader,
    /// Smoke detector
    SmokeDetector,
    /// Glass break sensor
    GlassBreakSensor,
    /// Security siren/alarm
    SecuritySiren,
    /// Unknown security device
    Unknown(String),
}

/// Security zone configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityZone {
    /// Zone identifier
    pub id: String,
    /// Zone name
    pub name: String,
    /// Zone type (perimeter, interior, etc.)
    pub zone_type: String,
    /// Armed status
    pub is_armed: bool,
    /// Devices in this zone
    pub devices: Vec<String>,
    /// Bypass status
    pub is_bypassed: bool,
    /// Zone alarm status
    pub alarm_active: bool,
}

/// Access control entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessEntry {
    /// Entry timestamp
    pub timestamp: DateTime<Utc>,
    /// Device that granted/denied access
    pub device_id: String,
    /// User identifier (if known)
    pub user_id: Option<String>,
    /// Access method (code, rfid, app, etc.)
    pub access_method: String,
    /// Whether access was granted
    pub granted: bool,
    /// Additional context
    pub context: HashMap<String, String>,
}

/// Door lock status and capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoorLockStatus {
    /// Lock identifier
    pub lock_id: String,
    /// Lock name
    pub name: String,
    /// Current lock state
    pub is_locked: bool,
    /// Door open/closed status
    pub door_open: Option<bool>,
    /// Battery level (0.0-1.0)
    pub battery_level: Option<f64>,
    /// Auto-lock enabled
    pub auto_lock_enabled: bool,
    /// Auto-lock delay (seconds)
    pub auto_lock_delay: Option<u32>,
    /// Last access timestamp
    pub last_access: Option<DateTime<Utc>>,
    /// Lock capabilities
    pub capabilities: DoorLockCapabilities,
}

/// Door lock capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DoorLockCapabilities {
    /// Supports remote locking/unlocking
    pub remote_control: bool,
    /// Supports access codes
    pub access_codes: bool,
    /// Supports RFID cards/tags
    pub rfid_support: bool,
    /// Supports mobile app unlock
    pub mobile_unlock: bool,
    /// Supports auto-lock timing
    pub auto_lock: bool,
    /// Supports door status sensing
    pub door_sensor: bool,
    /// Supports battery monitoring
    pub battery_monitoring: bool,
}

/// Get comprehensive security system status
pub async fn get_security_system_status(_input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;
    let devices = ctx.context.devices.read().await;

    // Find all security-related devices
    let security_devices: Vec<&LoxoneDevice> = devices
        .values()
        .filter(|device| is_security_device(&device.device_type))
        .collect();

    let mut zones = Vec::new();
    let mut alarm_status = HashMap::new();
    let mut overall_armed = false;

    // Check alarm system status
    for device in &security_devices {
        if device.device_type.contains("Alarm") || device.device_type.contains("Security") {
            let states = client
                .get_device_states(std::slice::from_ref(&device.uuid))
                .await?;
            if let Some(state) = states.get(&device.uuid) {
                alarm_status.insert(
                    device.uuid.clone(),
                    json!({
                        "name": device.name,
                        "state": state,
                        "room": device.room
                    }),
                );

                // Check if any alarm is armed
                if let Some(armed) = state.get("armed").and_then(|v| v.as_bool()) {
                    overall_armed = overall_armed || armed;
                }
            }
        }
    }

    // Group devices by room to create zones
    let mut room_zones: HashMap<String, Vec<&LoxoneDevice>> = HashMap::new();
    for device in &security_devices {
        if let Some(room) = &device.room {
            room_zones.entry(room.clone()).or_default().push(device);
        }
    }

    for (room_name, room_devices) in room_zones {
        let zone_devices: Vec<String> = room_devices.iter().map(|d| d.uuid.clone()).collect();
        zones.push(SecurityZone {
            id: format!("zone_{}", room_name.to_lowercase().replace(' ', "_")),
            name: room_name,
            zone_type: "mixed".to_string(),
            is_armed: overall_armed,
            devices: zone_devices,
            is_bypassed: false,
            alarm_active: false,
        });
    }

    Ok(json!({
        "status": "success",
        "overall_armed": overall_armed,
        "zones": zones,
        "alarm_systems": alarm_status,
        "device_count": security_devices.len(),
        "timestamp": Utc::now()
    }))
}

/// Check if device type is security-related
fn is_security_device(device_type: &str) -> bool {
    let security_keywords = [
        "alarm", "security", "motion", "contact", "door", "window", "camera", "lock", "access",
        "keypad", "rfid", "smoke", "glass", "siren", "detector", "sensor",
    ];

    let device_lower = device_type.to_lowercase();
    security_keywords
        .iter()
        .any(|keyword| device_lower.contains(keyword))
}

/// Arm security system or specific zones
pub async fn arm_security_system(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;

    #[derive(Deserialize)]
    struct ArmRequest {
        #[serde(default)]
        zones: Option<Vec<String>>,
        #[serde(default)]
        arm_mode: Option<String>, // "full", "partial", "night"
        #[serde(default)]
        delay_seconds: Option<u32>,
    }

    let request: ArmRequest =
        serde_json::from_value(input).map_err(|e| anyhow!("Invalid arm request: {}", e))?;

    let arm_mode = request.arm_mode.unwrap_or_else(|| "full".to_string());
    let delay = request.delay_seconds.unwrap_or(0);

    info!(
        "Arming security system with mode: {}, delay: {}s",
        arm_mode, delay
    );

    let mut armed_zones = Vec::new();

    if let Some(zone_ids) = request.zones {
        // Arm specific zones
        for zone_id in zone_ids {
            match client
                .send_command(&zone_id, &format!("arm/{arm_mode}"))
                .await
            {
                Ok(_) => {
                    armed_zones.push(zone_id.clone());
                    info!("Armed zone: {}", zone_id);
                }
                Err(e) => {
                    warn!("Failed to arm zone {}: {}", zone_id, e);
                }
            }
        }
    } else {
        // Arm entire system
        client
            .send_command("security/arm", &format!("arm/{arm_mode}"))
            .await
            .map_err(|e| anyhow!("Failed to arm security system: {}", e))?;
        armed_zones.push("all".to_string());
    }

    Ok(json!({
        "status": "success",
        "message": format!("Security system armed in {} mode", arm_mode),
        "armed_zones": armed_zones,
        "arm_mode": arm_mode,
        "delay_seconds": delay,
        "timestamp": Utc::now()
    }))
}

/// Disarm security system or specific zones
pub async fn disarm_security_system(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;

    #[derive(Deserialize)]
    struct DisarmRequest {
        #[serde(default)]
        zones: Option<Vec<String>>,
        #[serde(default)]
        #[allow(dead_code)]
        access_code: Option<String>,
        #[serde(default)]
        user_id: Option<String>,
    }

    let request: DisarmRequest =
        serde_json::from_value(input).map_err(|e| anyhow!("Invalid disarm request: {}", e))?;

    info!("Disarming security system");

    let mut disarmed_zones = Vec::new();

    if let Some(zone_ids) = request.zones {
        // Disarm specific zones
        for zone_id in zone_ids {
            match client.send_command(&zone_id, "disarm").await {
                Ok(_) => {
                    disarmed_zones.push(zone_id.clone());
                    info!("Disarmed zone: {}", zone_id);
                }
                Err(e) => {
                    warn!("Failed to disarm zone {}: {}", zone_id, e);
                }
            }
        }
    } else {
        // Disarm entire system
        client
            .send_command("security/disarm", "disarm")
            .await
            .map_err(|e| anyhow!("Failed to disarm security system: {}", e))?;
        disarmed_zones.push("all".to_string());
    }

    Ok(json!({
        "status": "success",
        "message": "Security system disarmed",
        "disarmed_zones": disarmed_zones,
        "user_id": request.user_id,
        "timestamp": Utc::now()
    }))
}

/// Control door locks - lock, unlock, set access codes
pub async fn control_door_lock(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;

    #[derive(Deserialize)]
    struct LockRequest {
        lock_id: String,
        action: String, // "lock", "unlock", "toggle"
        #[serde(default)]
        #[allow(dead_code)]
        access_code: Option<String>,
        #[serde(default)]
        user_id: Option<String>,
        #[serde(default)]
        auto_lock_delay: Option<u32>,
    }

    let request: LockRequest =
        serde_json::from_value(input).map_err(|e| anyhow!("Invalid lock request: {}", e))?;

    info!(
        "Controlling door lock {}: {}",
        request.lock_id, request.action
    );

    // Get current lock status
    let lock_states = client
        .get_device_states(std::slice::from_ref(&request.lock_id))
        .await?;
    let current_state = lock_states
        .get(&request.lock_id)
        .ok_or_else(|| anyhow!("Lock not found: {}", request.lock_id))?;

    let command = match request.action.as_str() {
        "lock" => "lock",
        "unlock" => "unlock",
        "toggle" => {
            // Determine current state and toggle
            let is_locked = current_state
                .get("locked")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if is_locked {
                "unlock"
            } else {
                "lock"
            }
        }
        _ => {
            return Err(anyhow!(
                "Invalid action: {}. Use 'lock', 'unlock', or 'toggle'",
                request.action
            ))
        }
    };

    // Send lock command
    client
        .send_command(&request.lock_id, command)
        .await
        .map_err(|e| anyhow!("Failed to {} lock {}: {}", command, request.lock_id, e))?;

    // If auto-lock delay is specified, set it
    if let Some(delay) = request.auto_lock_delay {
        let delay_command = format!("autolock/{delay}");
        if let Err(e) = client.send_command(&request.lock_id, &delay_command).await {
            warn!("Failed to set auto-lock delay: {}", e);
        }
    }

    Ok(json!({
        "status": "success",
        "message": format!("Door lock {} {}", request.lock_id, command),
        "lock_id": request.lock_id,
        "action": command,
        "user_id": request.user_id,
        "timestamp": Utc::now()
    }))
}

/// Get detailed door lock status
pub async fn get_door_lock_status(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;
    let devices = ctx.context.devices.read().await;

    #[derive(Deserialize)]
    struct StatusRequest {
        #[serde(default)]
        lock_id: Option<String>,
    }

    let request: StatusRequest =
        serde_json::from_value(input).map_err(|e| anyhow!("Invalid status request: {}", e))?;

    let locks: Vec<&LoxoneDevice> = if let Some(lock_id) = &request.lock_id {
        // Get specific lock
        devices
            .get(lock_id)
            .map(|device| vec![device])
            .unwrap_or_default()
    } else {
        // Get all door locks
        devices
            .values()
            .filter(|device| {
                device.device_type.to_lowercase().contains("lock")
                    || device.device_type.to_lowercase().contains("door")
            })
            .collect()
    };

    if locks.is_empty() {
        return Ok(json!({
            "status": "success",
            "message": "No door locks found",
            "locks": []
        }));
    }

    let lock_uuids: Vec<String> = locks.iter().map(|l| l.uuid.clone()).collect();
    let lock_states = client.get_device_states(&lock_uuids).await?;

    let mut lock_statuses = Vec::new();

    for lock in locks {
        if let Some(state) = lock_states.get(&lock.uuid) {
            let is_locked = state
                .get("locked")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let door_open = state.get("door_open").and_then(|v| v.as_bool());
            let battery_level = state.get("battery").and_then(|v| v.as_f64());

            let capabilities = DoorLockCapabilities {
                remote_control: true,
                access_codes: state
                    .get("supports_codes")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                rfid_support: state
                    .get("supports_rfid")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                mobile_unlock: true,
                auto_lock: state
                    .get("supports_autolock")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                door_sensor: door_open.is_some(),
                battery_monitoring: battery_level.is_some(),
            };

            let status = DoorLockStatus {
                lock_id: lock.uuid.clone(),
                name: lock.name.clone(),
                is_locked,
                door_open,
                battery_level,
                auto_lock_enabled: state
                    .get("autolock_enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                auto_lock_delay: state
                    .get("autolock_delay")
                    .and_then(|v| v.as_u64())
                    .map(|d| d as u32),
                last_access: None, // Would need to be tracked separately
                capabilities,
            };

            lock_statuses.push(status);
        }
    }

    Ok(json!({
        "status": "success",
        "locks": lock_statuses,
        "count": lock_statuses.len(),
        "timestamp": Utc::now()
    }))
}

/// Manage access codes for door locks
pub async fn manage_access_codes(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;

    #[derive(Deserialize)]
    struct AccessCodeRequest {
        lock_id: String,
        action: String, // "add", "remove", "list", "modify"
        #[serde(default)]
        code: Option<String>,
        #[serde(default)]
        user_name: Option<String>,
        #[serde(default)]
        code_slot: Option<u32>,
        #[serde(default)]
        #[allow(dead_code)]
        expires_at: Option<DateTime<Utc>>,
        #[serde(default)]
        #[allow(dead_code)]
        permanent: Option<bool>,
    }

    let request: AccessCodeRequest =
        serde_json::from_value(input).map_err(|e| anyhow!("Invalid access code request: {}", e))?;

    info!(
        "Managing access code for lock {}: {}",
        request.lock_id, request.action
    );

    match request.action.as_str() {
        "add" => {
            let code = request
                .code
                .ok_or_else(|| anyhow!("Code required for add action"))?;
            let slot = request.code_slot.unwrap_or(1);
            let command = format!("addcode/{slot}/{code}");

            client
                .send_command(&request.lock_id, &command)
                .await
                .map_err(|e| anyhow!("Failed to add access code: {}", e))?;

            Ok(json!({
                "status": "success",
                "message": "Access code added",
                "lock_id": request.lock_id,
                "code_slot": slot,
                "user_name": request.user_name
            }))
        }
        "remove" => {
            let slot = request
                .code_slot
                .ok_or_else(|| anyhow!("Code slot required for remove action"))?;
            let command = format!("removecode/{slot}");

            client
                .send_command(&request.lock_id, &command)
                .await
                .map_err(|e| anyhow!("Failed to remove access code: {}", e))?;

            Ok(json!({
                "status": "success",
                "message": "Access code removed",
                "lock_id": request.lock_id,
                "code_slot": slot
            }))
        }
        "list" => {
            // This would typically require a specific Loxone command to list codes
            // For now, return a placeholder response
            Ok(json!({
                "status": "success",
                "message": "Access code listing not implemented yet",
                "lock_id": request.lock_id,
                "codes": []
            }))
        }
        _ => Err(anyhow!(
            "Invalid action: {}. Use 'add', 'remove', or 'list'",
            request.action
        )),
    }
}

/// Control motion detectors and sensors
pub async fn control_motion_detectors(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;
    let devices = ctx.context.devices.read().await;

    #[derive(Deserialize)]
    struct MotionRequest {
        #[serde(default)]
        detector_id: Option<String>,
        action: String, // "enable", "disable", "test", "status"
        #[serde(default)]
        #[allow(dead_code)]
        sensitivity: Option<u32>, // 1-100
    }

    let request: MotionRequest = serde_json::from_value(input)
        .map_err(|e| anyhow!("Invalid motion detector request: {}", e))?;

    let detectors: Vec<&LoxoneDevice> = if let Some(detector_id) = &request.detector_id {
        devices
            .get(detector_id)
            .map(|device| vec![device])
            .unwrap_or_default()
    } else {
        devices
            .values()
            .filter(|device| {
                device.device_type.to_lowercase().contains("motion")
                    || device.device_type.to_lowercase().contains("detector")
                    || device.device_type.to_lowercase().contains("sensor")
            })
            .collect()
    };

    if detectors.is_empty() {
        return Ok(json!({
            "status": "error",
            "message": "No motion detectors found"
        }));
    }

    let mut results = Vec::new();

    for detector in detectors {
        let result = match request.action.as_str() {
            "enable" => client
                .send_command(&detector.uuid, "enable")
                .await
                .map(|_| format!("Enabled motion detector: {}", detector.name)),
            "disable" => client
                .send_command(&detector.uuid, "disable")
                .await
                .map(|_| format!("Disabled motion detector: {}", detector.name)),
            "test" => client
                .send_command(&detector.uuid, "test")
                .await
                .map(|_| format!("Test signal sent to: {}", detector.name)),
            "status" => {
                let states = client
                    .get_device_states(std::slice::from_ref(&detector.uuid))
                    .await?;
                if let Some(state) = states.get(&detector.uuid) {
                    Ok(format!("Status for {}: {:?}", detector.name, state))
                } else {
                    Ok(format!("No status available for: {}", detector.name))
                }
            }
            _ => Err(LoxoneError::validation(format!(
                "Invalid action: {}",
                request.action
            ))),
        };

        match result {
            Ok(message) => results.push(json!({
                "detector_id": detector.uuid,
                "name": detector.name,
                "status": "success",
                "message": message
            })),
            Err(e) => results.push(json!({
                "detector_id": detector.uuid,
                "name": detector.name,
                "status": "error",
                "message": e.to_string()
            })),
        }
    }

    Ok(json!({
        "status": "success",
        "action": request.action,
        "detectors": results,
        "timestamp": Utc::now()
    }))
}

/// Emergency security actions
pub async fn emergency_security_action(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;

    #[derive(Deserialize)]
    struct EmergencyRequest {
        action: String, // "panic", "silent_alarm", "lockdown", "evacuate"
        #[serde(default)]
        zone: Option<String>,
        #[serde(default)]
        reason: Option<String>,
    }

    let request: EmergencyRequest =
        serde_json::from_value(input).map_err(|e| anyhow!("Invalid emergency request: {}", e))?;

    error!(
        "EMERGENCY SECURITY ACTION: {} - Reason: {:?}",
        request.action, request.reason
    );

    let mut actions_taken = Vec::new();

    match request.action.as_str() {
        "panic" => {
            // Trigger all alarms and sirens
            if let Err(e) = client.send_command("security/panic", "activate").await {
                warn!("Failed to trigger panic alarm: {}", e);
            } else {
                actions_taken.push("Panic alarm activated".to_string());
            }

            // Lock all doors
            if let Err(e) = client.send_command("security/lockdown", "all").await {
                warn!("Failed to lock all doors: {}", e);
            } else {
                actions_taken.push("All doors locked".to_string());
            }
        }
        "silent_alarm" => {
            // Trigger silent alarm to monitoring center
            if let Err(e) = client
                .send_command("security/silent_alarm", "activate")
                .await
            {
                warn!("Failed to trigger silent alarm: {}", e);
            } else {
                actions_taken.push("Silent alarm activated".to_string());
            }
        }
        "lockdown" => {
            // Lock all access points
            if let Err(e) = client.send_command("security/lockdown", "engage").await {
                warn!("Failed to engage lockdown: {}", e);
            } else {
                actions_taken.push("Lockdown engaged".to_string());
            }
        }
        "evacuate" => {
            // Unlock all exit doors and disable alarms
            if let Err(e) = client.send_command("security/evacuate", "activate").await {
                warn!("Failed to activate evacuation mode: {}", e);
            } else {
                actions_taken.push("Evacuation mode activated".to_string());
            }
        }
        _ => return Err(anyhow!("Invalid emergency action: {}", request.action)),
    }

    Ok(json!({
        "status": "success",
        "emergency_action": request.action,
        "actions_taken": actions_taken,
        "zone": request.zone,
        "reason": request.reason,
        "timestamp": Utc::now(),
        "alert_level": "CRITICAL"
    }))
}
