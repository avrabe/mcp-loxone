//! Advanced Multi-Zone HVAC Control and Climate Management
//!
//! Comprehensive climate control with:
//! - Multi-zone HVAC management with independent control
//! - Intelligent scheduling and occupancy-based optimization
//! - Zone synchronization and balancing
//! - Advanced air quality management
//! - Energy-efficient operation modes
//! - Seasonal adaptation and weather integration
//! - Ventilation and air circulation control
//!
//! For read-only climate data, use resources:
//! - loxone://climate/overview - Climate control overview
//! - loxone://climate/rooms/{room} - Room climate data
//! - loxone://climate/sensors - Temperature sensor readings
//! - loxone://climate/zones - Zone configuration and status
//! - loxone://climate/air_quality - Air quality metrics

use crate::tools::{DeviceFilter, ToolContext, ToolResponse};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::warn;

/// HVAC zone configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HvacZone {
    /// Zone identifier
    pub zone_id: String,
    /// Zone name
    pub name: String,
    /// Rooms included in this zone
    pub rooms: Vec<String>,
    /// Zone type (residential, commercial, etc.)
    pub zone_type: ZoneType,
    /// Current zone status
    pub status: ZoneStatus,
    /// Zone priority (1-10, 1 being highest)
    pub priority: u8,
    /// Zone schedule
    pub schedule: Option<ZoneSchedule>,
    /// Zone constraints
    pub constraints: ZoneConstraints,
}

/// Zone types for different usage patterns
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ZoneType {
    /// Living areas (living room, kitchen)
    Living,
    /// Sleeping areas (bedrooms)
    Sleeping,
    /// Working areas (office, study)
    Working,
    /// Utility areas (bathroom, laundry)
    Utility,
    /// Common areas (hallway, stairwell)
    Common,
    /// Special purpose (server room, wine cellar)
    Special(String),
}

/// Zone operational status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneStatus {
    /// Zone active/inactive
    pub active: bool,
    /// Current temperature average
    pub current_temperature: Option<f64>,
    /// Target temperature
    pub target_temperature: Option<f64>,
    /// Current humidity
    pub humidity: Option<f64>,
    /// Air quality index (0-500)
    pub air_quality_index: Option<u16>,
    /// CO2 level (ppm)
    pub co2_level: Option<u16>,
    /// Current mode
    pub mode: HvacMode,
    /// Fan speed
    pub fan_speed: FanSpeed,
    /// Damper position (0-100)
    pub damper_position: Option<u8>,
    /// Occupancy detected
    pub occupied: bool,
    /// Last update time
    pub last_update: DateTime<Utc>,
}

/// HVAC operating modes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HvacMode {
    /// Heating mode
    Heating,
    /// Cooling mode
    Cooling,
    /// Auto mode (heating/cooling as needed)
    Auto,
    /// Fan only (ventilation)
    FanOnly,
    /// Dehumidification mode
    Dehumidify,
    /// System off
    Off,
    /// Emergency heat
    EmergencyHeat,
    /// Eco mode
    Eco,
}

/// Fan speed settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FanSpeed {
    /// Automatic fan speed
    Auto,
    /// Low speed
    Low,
    /// Medium speed
    Medium,
    /// High speed
    High,
    /// Variable speed (0-100)
    Variable(u8),
}

/// Zone scheduling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneSchedule {
    /// Schedule entries
    pub entries: Vec<ScheduleEntry>,
    /// Override until time
    pub override_until: Option<DateTime<Utc>>,
    /// Holiday mode enabled
    pub holiday_mode: bool,
}

/// Individual schedule entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    /// Days of week (1=Monday, 7=Sunday)
    pub days: Vec<u8>,
    /// Start time (HH:MM)
    pub start_time: String,
    /// End time (HH:MM)
    pub end_time: String,
    /// Target temperature
    pub temperature: f64,
    /// Mode for this period
    pub mode: HvacMode,
    /// Fan mode
    pub fan_mode: FanSpeed,
}

/// Zone operational constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneConstraints {
    /// Minimum temperature allowed
    pub min_temperature: f64,
    /// Maximum temperature allowed
    pub max_temperature: f64,
    /// Maximum temperature change rate (°C/hour)
    pub max_rate_of_change: Option<f64>,
    /// Minimum fresh air percentage
    pub min_fresh_air_percent: Option<u8>,
    /// Quiet hours (reduced fan speed)
    pub quiet_hours: Option<(String, String)>,
}

/// Air quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirQualityMetrics {
    /// Zone identifier
    pub zone_id: String,
    /// Temperature
    pub temperature: Option<f64>,
    /// Relative humidity percentage
    pub humidity: Option<f64>,
    /// CO2 concentration (ppm)
    pub co2_ppm: Option<u16>,
    /// VOC level (ppb)
    pub voc_ppb: Option<u16>,
    /// PM2.5 level (μg/m³)
    pub pm25: Option<f64>,
    /// Air quality index (0-500)
    pub aqi: Option<u16>,
    /// Comfort index (0-100)
    pub comfort_index: Option<u8>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Multi-zone synchronization settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneSyncSettings {
    /// Enable zone synchronization
    pub enabled: bool,
    /// Master zone (others follow)
    pub master_zone: Option<String>,
    /// Temperature offset for slave zones
    pub slave_offset: f64,
    /// Synchronize schedules
    pub sync_schedules: bool,
    /// Synchronize modes
    pub sync_modes: bool,
}

/// Climate device information with enhanced capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimateDevice {
    /// Device UUID
    pub uuid: String,
    /// Device name
    pub name: String,
    /// Device type
    pub device_type: String,
    /// Room assignment
    pub room: Option<String>,
    /// Zone assignment
    pub zone: Option<String>,
    /// Current temperature
    pub current_temperature: Option<f64>,
    /// Target temperature
    pub target_temperature: Option<f64>,
    /// Current humidity
    pub humidity: Option<f64>,
    /// Operating mode
    pub mode: Option<String>,
    /// Fan speed
    pub fan_speed: Option<String>,
    /// Damper position (0-100)
    pub damper_position: Option<u8>,
    /// Device capabilities
    pub capabilities: DeviceCapabilities,
    /// States from the device
    pub states: HashMap<String, serde_json::Value>,
}

/// Device capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    /// Supports heating
    pub heating: bool,
    /// Supports cooling
    pub cooling: bool,
    /// Supports fan control
    pub fan_control: bool,
    /// Supports humidity control
    pub humidity_control: bool,
    /// Supports scheduling
    pub scheduling: bool,
    /// Supports zone control
    pub zone_control: bool,
    /// Has occupancy sensor
    pub occupancy_sensor: bool,
    /// Has air quality sensors
    pub air_quality_sensors: bool,
}

/// Climate system overview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimateOverview {
    /// Total climate devices
    pub total_devices: usize,

    /// Room controllers
    pub room_controllers: Vec<ClimateDevice>,

    /// Temperature sensors
    pub temperature_sensors: Vec<ClimateDevice>,

    /// Heating/cooling devices
    pub hvac_devices: Vec<ClimateDevice>,

    /// System-wide statistics
    pub statistics: ClimateStatistics,
}

/// Climate statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimateStatistics {
    /// Average temperature across all sensors
    pub average_temperature: Option<f64>,

    /// Temperature range (min, max)
    pub temperature_range: Option<(f64, f64)>,

    /// Rooms with climate control
    pub controlled_rooms: Vec<String>,

    /// Active heating/cooling zones
    pub active_zones: usize,
}

/// Get comprehensive climate control overview
// READ-ONLY TOOL REMOVED:
// get_climate_control() → Use resource: loxone://climate/overview
#[allow(dead_code)]
async fn _removed_get_climate_control(context: ToolContext) -> ToolResponse {
    // Get all climate-related devices
    let filter = DeviceFilter {
        category: Some("climate".to_string()),
        device_type: None,
        room: None,
        limit: None,
    };
    let devices = match context.get_devices(Some(filter)).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    if devices.is_empty() {
        return ToolResponse::error("No climate control devices found in the system".to_string());
    }

    // Categorize climate devices
    let mut room_controllers = Vec::new();
    let mut temperature_sensors = Vec::new();
    let mut hvac_devices = Vec::new();
    let mut controlled_rooms = std::collections::HashSet::new();

    let mut temperatures = Vec::new();

    for device in devices {
        let climate_device = parse_climate_device(device);

        // Collect temperatures for statistics
        if let Some(temp) = climate_device.current_temperature {
            temperatures.push(temp);
        }

        // Track controlled rooms
        if let Some(ref room) = climate_device.room {
            controlled_rooms.insert(room.clone());
        }

        // Categorize by device type
        match climate_device.device_type.to_lowercase().as_str() {
            t if t.contains("roomcontroller") || t.contains("controller") => {
                room_controllers.push(climate_device);
            }
            t if t.contains("temperature") || t.contains("sensor") => {
                temperature_sensors.push(climate_device);
            }
            _ => {
                hvac_devices.push(climate_device);
            }
        }
    }

    // Calculate statistics
    let average_temperature = if !temperatures.is_empty() {
        Some(temperatures.iter().sum::<f64>() / temperatures.len() as f64)
    } else {
        None
    };

    let temperature_range = if !temperatures.is_empty() {
        let min = temperatures.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = temperatures
            .iter()
            .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        Some((min, max))
    } else {
        None
    };

    let statistics = ClimateStatistics {
        average_temperature,
        temperature_range,
        controlled_rooms: controlled_rooms.into_iter().collect(),
        active_zones: room_controllers.len(),
    };

    let overview = ClimateOverview {
        total_devices: room_controllers.len() + temperature_sensors.len() + hvac_devices.len(),
        room_controllers,
        temperature_sensors,
        hvac_devices,
        statistics,
    };

    let room_count = overview.room_controllers.len();
    let sensor_count = overview.temperature_sensors.len();
    let hvac_count = overview.hvac_devices.len();
    let message = format!(
        "Climate system: {} devices ({room_count} room controllers, {sensor_count} sensors, {hvac_count} HVAC devices)",
        overview.total_devices
    );

    ToolResponse::success_with_message(serde_json::to_value(overview).unwrap(), message)
}

/// Get climate status for a specific room
// READ-ONLY TOOL REMOVED:
// get_room_climate() → Use resource: loxone://climate/rooms/{room}
#[allow(dead_code)]
async fn _removed_get_room_climate(context: ToolContext, room_name: String) -> ToolResponse {
    // Get climate devices in the specified room
    let filter = DeviceFilter {
        category: Some("climate".to_string()),
        device_type: None,
        room: None,
        limit: None,
    };
    let all_devices = match context.get_devices(Some(filter)).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    let room_devices: Vec<_> = all_devices
        .into_iter()
        .filter(|device| device.room.as_ref() == Some(&room_name))
        .collect();

    if room_devices.is_empty() {
        return ToolResponse::error(format!("No climate devices found in room '{room_name}'"));
    }

    // Parse climate devices
    let climate_devices: Vec<ClimateDevice> =
        room_devices.into_iter().map(parse_climate_device).collect();

    // Find room controller and sensors
    let mut room_controller = None;
    let mut sensors = Vec::new();

    for device in &climate_devices {
        if device.device_type.to_lowercase().contains("controller") {
            room_controller = Some(device.clone());
        } else {
            sensors.push(device.clone());
        }
    }

    let response_data = serde_json::json!({
        "room": room_name,
        "room_controller": room_controller,
        "sensors": sensors,
        "total_devices": climate_devices.len(),
        "has_controller": room_controller.is_some(),
        "sensor_count": sensors.len()
    });

    let message = if let Some(ref controller) = room_controller {
        let temp_info = if let Some(temp) = controller.current_temperature {
            format!(" (current: {temp:.1}°C)")
        } else {
            String::new()
        };
        let sensor_count = sensors.len();
        format!("Room '{room_name}' climate: 1 controller, {sensor_count} sensors{temp_info}")
    } else {
        let sensor_count = sensors.len();
        format!("Room '{room_name}' climate: {sensor_count} sensors (no controller)")
    };

    ToolResponse::success_with_message(response_data, message)
}

/// Get multi-zone HVAC system status
pub async fn get_multizone_status(context: ToolContext) -> ToolResponse {
    // Get all climate devices
    let filter = DeviceFilter {
        category: Some("climate".to_string()),
        device_type: None,
        room: None,
        limit: None,
    };
    let devices = match context.get_devices(Some(filter)).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    // Group devices by zones
    let mut zones: HashMap<String, Vec<ClimateDevice>> = HashMap::new();
    let mut unzoned_devices = Vec::new();

    for device in devices {
        let climate_device = parse_climate_device(device);

        if let Some(zone) = &climate_device.zone {
            zones
                .entry(zone.clone())
                .or_default()
                .push(climate_device);
        } else if let Some(room) = &climate_device.room {
            // Auto-assign to zone based on room
            let auto_zone = determine_zone_from_room(room);
            zones
                .entry(auto_zone)
                .or_default()
                .push(climate_device);
        } else {
            unzoned_devices.push(climate_device);
        }
    }

    // Calculate zone status
    let mut zone_statuses = Vec::new();

    for (zone_name, zone_devices) in zones {
        let zone_status = calculate_zone_status(&zone_name, &zone_devices);
        zone_statuses.push(zone_status);
    }

    let response_data = json!({
        "zones": zone_statuses,
        "unzoned_devices": unzoned_devices,
        "total_zones": zone_statuses.len(),
        "timestamp": Utc::now()
    });

    ToolResponse::success_with_message(
        response_data,
        format!(
            "Multi-zone HVAC status: {} zones active",
            zone_statuses.len()
        ),
    )
}

/// Determine zone from room name
fn determine_zone_from_room(room: &str) -> String {
    let room_lower = room.to_lowercase();

    if room_lower.contains("bedroom") || room_lower.contains("schlaf") {
        "sleeping_zone".to_string()
    } else if room_lower.contains("living")
        || room_lower.contains("wohn")
        || room_lower.contains("kitchen")
    {
        "living_zone".to_string()
    } else if room_lower.contains("office")
        || room_lower.contains("büro")
        || room_lower.contains("study")
    {
        "working_zone".to_string()
    } else if room_lower.contains("bath") || room_lower.contains("bad") || room_lower.contains("wc")
    {
        "utility_zone".to_string()
    } else {
        "common_zone".to_string()
    }
}

/// Calculate zone status from devices
fn calculate_zone_status(zone_name: &str, devices: &[ClimateDevice]) -> HvacZone {
    let rooms: Vec<String> = devices
        .iter()
        .filter_map(|d| d.room.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let temperatures: Vec<f64> = devices
        .iter()
        .filter_map(|d| d.current_temperature)
        .collect();

    let avg_temp = if !temperatures.is_empty() {
        Some(temperatures.iter().sum::<f64>() / temperatures.len() as f64)
    } else {
        None
    };

    let zone_type = match zone_name {
        "sleeping_zone" => ZoneType::Sleeping,
        "living_zone" => ZoneType::Living,
        "working_zone" => ZoneType::Working,
        "utility_zone" => ZoneType::Utility,
        _ => ZoneType::Common,
    };

    let status = ZoneStatus {
        active: true,
        current_temperature: avg_temp,
        target_temperature: Some(21.0), // Default
        humidity: None,
        air_quality_index: None,
        co2_level: None,
        mode: HvacMode::Auto,
        fan_speed: FanSpeed::Auto,
        damper_position: None,
        occupied: true, // Would need occupancy sensors
        last_update: Utc::now(),
    };

    HvacZone {
        zone_id: zone_name.to_string(),
        name: zone_name.replace('_', " ").to_title_case(),
        rooms,
        zone_type,
        status,
        priority: 5,
        schedule: None,
        constraints: ZoneConstraints {
            min_temperature: 16.0,
            max_temperature: 28.0,
            max_rate_of_change: Some(2.0),
            min_fresh_air_percent: Some(20),
            quiet_hours: None,
        },
    }
}

/// Trait for title case conversion
trait ToTitleCase {
    fn to_title_case(&self) -> String;
}

impl ToTitleCase for str {
    fn to_title_case(&self) -> String {
        self.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Set target temperature for a room or zone
pub async fn set_temperature(
    context: ToolContext,
    target: String,
    temperature: f64,
    zone_mode: Option<bool>,
) -> ToolResponse {
    // Validate temperature range
    if !(5.0..=35.0).contains(&temperature) {
        return ToolResponse::error(format!(
            "Invalid temperature {temperature}°C. Must be between 5°C and 35°C"
        ));
    }

    let is_zone = zone_mode.unwrap_or(false);

    if is_zone {
        // Zone temperature control
        set_zone_temperature(context, target, temperature).await
    } else {
        // Room temperature control (existing logic)
        set_room_temperature_internal(context, target, temperature).await
    }
}

/// Set temperature for an entire zone
async fn set_zone_temperature(
    context: ToolContext,
    zone_name: String,
    temperature: f64,
) -> ToolResponse {
    let client = &context.client;

    // Get all climate devices
    let filter = DeviceFilter {
        category: Some("climate".to_string()),
        device_type: None,
        room: None,
        limit: None,
    };
    let devices = match context.get_devices(Some(filter)).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    // Find devices in the target zone
    let mut zone_controllers = Vec::new();

    for device in devices {
        let climate_device = parse_climate_device(device.clone());
        let device_zone = climate_device.zone.or_else(|| {
            climate_device
                .room
                .as_ref()
                .map(|r| determine_zone_from_room(r))
        });

        if device_zone.as_ref() == Some(&zone_name)
            && device.device_type.to_lowercase().contains("controller")
        {
            zone_controllers.push(device);
        }
    }

    if zone_controllers.is_empty() {
        return ToolResponse::error(format!("No controllers found in zone '{zone_name}'"));
    }

    // Send temperature command to all controllers in the zone
    let command = format!("setpoint/{temperature}");
    let mut results = Vec::new();
    let mut failures = 0;

    for controller in &zone_controllers {
        match client.send_command(&controller.uuid, &command).await {
            Ok(response) => {
                if response.code == 200 {
                    results.push(json!({
                        "controller": controller.name,
                        "room": controller.room,
                        "success": true
                    }));
                } else {
                    failures += 1;
                    results.push(json!({
                        "controller": controller.name,
                        "room": controller.room,
                        "success": false,
                        "error": format!("Response code: {}", response.code)
                    }));
                }
            }
            Err(e) => {
                failures += 1;
                results.push(json!({
                    "controller": controller.name,
                    "room": controller.room,
                    "success": false,
                    "error": e.to_string()
                }));
            }
        }
    }

    let response_data = json!({
        "zone": zone_name,
        "target_temperature": temperature,
        "controllers_updated": zone_controllers.len() - failures,
        "controllers_failed": failures,
        "results": results
    });

    if failures == 0 {
        ToolResponse::success_with_message(
            response_data,
            format!(
                "Set zone '{zone_name}' temperature to {temperature:.1}°C ({} controllers updated)",
                zone_controllers.len()
            ),
        )
    } else {
        ToolResponse::success_with_message(
            response_data,
            format!("Set zone '{zone_name}' temperature to {temperature:.1}°C ({}/{} controllers updated)", 
                    zone_controllers.len() - failures, zone_controllers.len())
        )
    }
}

/// Internal function for room temperature setting
async fn set_room_temperature_internal(
    context: ToolContext,
    room_name: String,
    temperature: f64,
) -> ToolResponse {
    // Find room controller
    let filter = DeviceFilter {
        category: Some("climate".to_string()),
        device_type: None,
        room: None,
        limit: None,
    };
    let all_devices = match context.get_devices(Some(filter)).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    let room_controller = all_devices.into_iter().find(|device| {
        device.room.as_ref() == Some(&room_name)
            && device.device_type.to_lowercase().contains("controller")
    });

    let controller = match room_controller {
        Some(device) => device,
        None => {
            return ToolResponse::error(format!("No room controller found in room '{room_name}'"))
        }
    };

    // Send temperature set command
    let command = format!("setpoint/{temperature}");

    let result = match context
        .client
        .send_command(&controller.uuid, &command)
        .await
    {
        Ok(response) => {
            if response.code == 200 {
                serde_json::json!({
                    "room": room_name,
                    "controller": controller.name,
                    "target_temperature": temperature,
                    "command_sent": command,
                    "response": response.value,
                    "success": true
                })
            } else {
                return ToolResponse::error(format!(
                    "Failed to set temperature: response code {}",
                    response.code
                ));
            }
        }
        Err(e) => return ToolResponse::error(format!("Failed to send command: {e}")),
    };

    ToolResponse::success_with_message(
        result,
        format!("Set target temperature to {temperature:.1}°C for room '{room_name}'"),
    )
}

/// Get temperature readings from all sensors
// READ-ONLY TOOL REMOVED:
// get_temperature_readings() → Use resource: loxone://climate/sensors
#[allow(dead_code)]
async fn _removed_get_temperature_readings(context: ToolContext) -> ToolResponse {
    // Get all climate devices
    let filter = DeviceFilter {
        category: Some("climate".to_string()),
        device_type: None,
        room: None,
        limit: None,
    };
    let devices = match context.get_devices(Some(filter)).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    if devices.is_empty() {
        return ToolResponse::error("No climate devices found".to_string());
    }

    // Parse and collect temperature readings
    let mut readings = Vec::new();
    let mut room_temperatures = HashMap::new();

    for device in devices {
        let climate_device = parse_climate_device(device);

        if let Some(temp) = climate_device.current_temperature {
            let reading = serde_json::json!({
                "device": climate_device.name,
                "room": climate_device.room,
                "temperature": temp,
                "device_type": climate_device.device_type,
                "timestamp": chrono::Utc::now()
            });
            readings.push(reading);

            // Track highest temperature per room
            if let Some(ref room) = climate_device.room {
                let current_max = room_temperatures
                    .get(room)
                    .copied()
                    .unwrap_or(f64::NEG_INFINITY);
                if temp > current_max {
                    room_temperatures.insert(room.clone(), temp);
                }
            }
        }
    }

    if readings.is_empty() {
        return ToolResponse::error("No temperature readings available".to_string());
    }

    // Calculate statistics
    let temperatures: Vec<f64> = readings
        .iter()
        .filter_map(|r| r.get("temperature").and_then(|v| v.as_f64()))
        .collect();

    let avg_temp = temperatures.iter().sum::<f64>() / temperatures.len() as f64;
    let min_temp = temperatures.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max_temp = temperatures
        .iter()
        .fold(f64::NEG_INFINITY, |a, &b| a.max(b));

    let response_data = serde_json::json!({
        "readings": readings,
        "room_temperatures": room_temperatures,
        "statistics": {
            "total_sensors": readings.len(),
            "average_temperature": avg_temp,
            "min_temperature": min_temp,
            "max_temperature": max_temp,
            "rooms_monitored": room_temperatures.len()
        },
        "timestamp": chrono::Utc::now()
    });

    ToolResponse::success_with_message(
        response_data,
        format!(
            "Temperature readings from {} sensors (avg: {:.1}°C, range: {:.1}°C - {:.1}°C)",
            readings.len(),
            avg_temp,
            min_temp,
            max_temp
        ),
    )
}

/// Control HVAC mode for room or zone
pub async fn set_hvac_mode(
    context: ToolContext,
    target: String,
    mode: String,
    zone_mode: Option<bool>,
    fan_speed: Option<String>,
) -> ToolResponse {
    // Validate mode
    let valid_modes = [
        "heating",
        "cooling",
        "auto",
        "off",
        "fan_only",
        "dehumidify",
        "eco",
    ];
    if !valid_modes.contains(&mode.as_str()) {
        return ToolResponse::error(format!(
            "Invalid mode '{}'. Valid modes: {}",
            mode,
            valid_modes.join(", ")
        ));
    }

    // Validate fan speed if provided
    if let Some(ref speed) = fan_speed {
        let valid_speeds = ["auto", "low", "medium", "high"];
        if !valid_speeds.contains(&speed.as_str()) {
            return ToolResponse::error(format!(
                "Invalid fan speed '{}'. Valid speeds: {}",
                speed,
                valid_speeds.join(", ")
            ));
        }
    }

    let is_zone = zone_mode.unwrap_or(false);

    if is_zone {
        set_zone_mode(context, target, mode, fan_speed).await
    } else {
        set_room_mode_internal(context, target, mode, fan_speed).await
    }
}

/// Set mode for an entire zone
async fn set_zone_mode(
    context: ToolContext,
    zone_name: String,
    mode: String,
    fan_speed: Option<String>,
) -> ToolResponse {
    let client = &context.client;

    // Get all climate devices
    let filter = DeviceFilter {
        category: Some("climate".to_string()),
        device_type: None,
        room: None,
        limit: None,
    };
    let devices = match context.get_devices(Some(filter)).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    // Find devices in the target zone
    let mut zone_controllers = Vec::new();

    for device in devices {
        let climate_device = parse_climate_device(device.clone());
        let device_zone = climate_device.zone.or_else(|| {
            climate_device
                .room
                .as_ref()
                .map(|r| determine_zone_from_room(r))
        });

        if device_zone.as_ref() == Some(&zone_name)
            && device.device_type.to_lowercase().contains("controller")
        {
            zone_controllers.push(device);
        }
    }

    if zone_controllers.is_empty() {
        return ToolResponse::error(format!("No controllers found in zone '{zone_name}'"));
    }

    // Send commands to all controllers in the zone
    let mode_command = format!("mode/{mode}");
    let mut results = Vec::new();
    let mut failures = 0;

    for controller in &zone_controllers {
        // Send mode command
        match client.send_command(&controller.uuid, &mode_command).await {
            Ok(response) => {
                if response.code == 200 {
                    // Send fan speed command if provided
                    if let Some(ref speed) = fan_speed {
                        let fan_command = format!("fan/{speed}");
                        let _ = client.send_command(&controller.uuid, &fan_command).await;
                    }

                    results.push(json!({
                        "controller": controller.name,
                        "room": controller.room,
                        "success": true
                    }));
                } else {
                    failures += 1;
                    results.push(json!({
                        "controller": controller.name,
                        "room": controller.room,
                        "success": false,
                        "error": format!("Response code: {}", response.code)
                    }));
                }
            }
            Err(e) => {
                failures += 1;
                results.push(json!({
                    "controller": controller.name,
                    "room": controller.room,
                    "success": false,
                    "error": e.to_string()
                }));
            }
        }
    }

    let response_data = json!({
        "zone": zone_name,
        "mode": mode,
        "fan_speed": fan_speed,
        "controllers_updated": zone_controllers.len() - failures,
        "controllers_failed": failures,
        "results": results
    });

    if failures == 0 {
        ToolResponse::success_with_message(
            response_data,
            format!(
                "Set zone '{zone_name}' to {mode} mode ({} controllers updated)",
                zone_controllers.len()
            ),
        )
    } else {
        ToolResponse::success_with_message(
            response_data,
            format!(
                "Set zone '{zone_name}' to {mode} mode ({}/{} controllers updated)",
                zone_controllers.len() - failures,
                zone_controllers.len()
            ),
        )
    }
}

/// Internal function for room mode setting
async fn set_room_mode_internal(
    context: ToolContext,
    room_name: String,
    mode: String,
    fan_speed: Option<String>,
) -> ToolResponse {
    // Find room controller
    let filter = DeviceFilter {
        category: Some("climate".to_string()),
        device_type: None,
        room: None,
        limit: None,
    };
    let all_devices = match context.get_devices(Some(filter)).await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    let room_controller = all_devices.into_iter().find(|device| {
        device.room.as_ref() == Some(&room_name)
            && device.device_type.to_lowercase().contains("controller")
    });

    let controller = match room_controller {
        Some(device) => device,
        None => {
            return ToolResponse::error(format!("No room controller found in room '{room_name}'"))
        }
    };

    // Send mode command
    let command = format!("mode/{mode}");

    let result = match context
        .client
        .send_command(&controller.uuid, &command)
        .await
    {
        Ok(response) => {
            if response.code == 200 {
                // Send fan speed command if provided
                if let Some(ref speed) = fan_speed {
                    let fan_command = format!("fan/{speed}");
                    let _ = context
                        .client
                        .send_command(&controller.uuid, &fan_command)
                        .await;
                }

                serde_json::json!({
                    "room": room_name,
                    "controller": controller.name,
                    "mode": mode,
                    "fan_speed": fan_speed,
                    "command_sent": command,
                    "response": response.value,
                    "success": true
                })
            } else {
                return ToolResponse::error(format!(
                    "Failed to set mode: response code {}",
                    response.code
                ));
            }
        }
        Err(e) => return ToolResponse::error(format!("Failed to send command: {e}")),
    };

    ToolResponse::success_with_message(
        result,
        format!("Set mode to '{mode}' for room '{room_name}'"),
    )
}

/// Create zone schedule
pub async fn create_zone_schedule(
    context: ToolContext,
    zone_name: String,
    schedule_entries: Vec<ScheduleEntry>,
) -> ToolResponse {
    let client = &context.client;

    // Validate schedule entries
    for entry in &schedule_entries {
        // Validate days
        if entry.days.iter().any(|&d| !(1..=7).contains(&d)) {
            return ToolResponse::error(
                "Invalid day in schedule. Days must be 1-7 (Mon-Sun)".to_string(),
            );
        }

        // Validate time format
        if !is_valid_time_format(&entry.start_time) || !is_valid_time_format(&entry.end_time) {
            return ToolResponse::error(
                "Invalid time format. Use HH:MM (24-hour format)".to_string(),
            );
        }

        // Validate temperature
        if !(5.0..=35.0).contains(&entry.temperature) {
            return ToolResponse::error("Temperature must be between 5°C and 35°C".to_string());
        }
    }

    // Create schedule object
    let schedule = ZoneSchedule {
        entries: schedule_entries.clone(),
        override_until: None,
        holiday_mode: false,
    };

    // Send schedule to zone controllers
    let schedule_json = serde_json::to_string(&schedule).unwrap();
    let command = format!("schedule/set/{}", urlencoding::encode(&schedule_json));

    match client
        .send_command(&format!("zone/{zone_name}"), &command)
        .await
    {
        Ok(_) => {
            let response_data = json!({
                "zone": zone_name,
                "schedule": schedule,
                "entries_count": schedule_entries.len(),
                "timestamp": Utc::now()
            });

            ToolResponse::success_with_message(
                response_data,
                format!(
                    "Created schedule for zone '{}' with {} entries",
                    zone_name,
                    schedule_entries.len()
                ),
            )
        }
        Err(e) => ToolResponse::error(format!("Failed to create schedule: {e}")),
    }
}

/// Check if time format is valid HH:MM
fn is_valid_time_format(time: &str) -> bool {
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() != 2 {
        return false;
    }

    match (parts[0].parse::<u8>(), parts[1].parse::<u8>()) {
        (Ok(h), Ok(m)) => h < 24 && m < 60,
        _ => false,
    }
}

/// Synchronize multiple zones
pub async fn synchronize_zones(
    context: ToolContext,
    master_zone: String,
    slave_zones: Vec<String>,
    sync_settings: ZoneSyncSettings,
) -> ToolResponse {
    let client = &context.client;

    // Get master zone settings
    let master_state = match get_zone_state(&context, &master_zone).await {
        Ok(state) => state,
        Err(e) => return ToolResponse::error(format!("Failed to get master zone state: {e}")),
    };

    let mut sync_results = Vec::new();

    for slave_zone in &slave_zones {
        let mut sync_commands = Vec::new();

        // Sync temperature with offset
        if let Some(master_temp) = master_state.target_temperature {
            let slave_temp = master_temp + sync_settings.slave_offset;
            sync_commands.push(("temperature", format!("setpoint/{slave_temp}")));
        }

        // Sync mode
        if sync_settings.sync_modes {
            let mode_str = format!("{:?}", master_state.mode).to_lowercase();
            sync_commands.push(("mode", format!("mode/{mode_str}")));
        }

        // Apply sync commands
        let mut success = true;
        for (cmd_type, command) in sync_commands {
            if let Err(e) = client
                .send_command(&format!("zone/{slave_zone}"), &command)
                .await
            {
                success = false;
                warn!("Failed to sync {} for zone {}: {}", cmd_type, slave_zone, e);
            }
        }

        sync_results.push(json!({
            "zone": slave_zone,
            "synced": success,
            "settings_applied": {
                "temperature_offset": sync_settings.slave_offset,
                "mode_synced": sync_settings.sync_modes,
            }
        }));
    }

    let response_data = json!({
        "master_zone": master_zone,
        "slave_zones": slave_zones,
        "sync_results": sync_results,
        "sync_settings": sync_settings,
        "timestamp": Utc::now()
    });

    ToolResponse::success_with_message(
        response_data,
        format!(
            "Synchronized {} zones with master zone '{}'",
            slave_zones.len(),
            master_zone
        ),
    )
}

/// Get zone state helper
async fn get_zone_state(context: &ToolContext, zone_name: &str) -> Result<ZoneStatus> {
    // Get all climate devices for this zone
    let filter = DeviceFilter {
        category: Some("climate".to_string()),
        device_type: None,
        room: None,
        limit: None,
    };

    let devices = context
        .get_devices(Some(filter))
        .await
        .map_err(|e| anyhow!("Failed to get devices: {}", e))?;

    // Filter devices for this zone
    let zone_devices: Vec<_> = devices
        .into_iter()
        .filter(|device| {
            let climate_device = parse_climate_device(device.clone());
            let device_zone = climate_device.zone.or_else(|| {
                climate_device
                    .room
                    .as_ref()
                    .map(|r| determine_zone_from_room(r))
            });
            device_zone.as_ref() == Some(&zone_name.to_string())
        })
        .collect();

    if zone_devices.is_empty() {
        return Err(anyhow!("No devices found in zone '{}'", zone_name));
    }

    // Get current states for all zone devices
    let device_uuids: Vec<String> = zone_devices.iter().map(|d| d.uuid.clone()).collect();
    let states = context
        .client
        .get_device_states(&device_uuids)
        .await
        .map_err(|e| anyhow!("Failed to get device states: {}", e))?;

    // Calculate zone status from device states
    let mut temperatures = Vec::new();
    let mut humidities = Vec::new();
    let mut co2_levels = Vec::new();
    let mut occupied = false;

    for device in &zone_devices {
        if let Some(state) = states.get(&device.uuid) {
            if let Some(temp) = state.get("temperature").and_then(|v| v.as_f64()) {
                temperatures.push(temp);
            }
            if let Some(humidity) = state.get("humidity").and_then(|v| v.as_f64()) {
                humidities.push(humidity);
            }
            if let Some(co2) = state.get("co2").and_then(|v| v.as_u64()) {
                co2_levels.push(co2 as u16);
            }
            if let Some(occ) = state.get("occupied").and_then(|v| v.as_bool()) {
                occupied = occupied || occ;
            }
        }
    }

    Ok(ZoneStatus {
        active: !zone_devices.is_empty(),
        current_temperature: if temperatures.is_empty() {
            None
        } else {
            Some(temperatures.iter().sum::<f64>() / temperatures.len() as f64)
        },
        target_temperature: Some(22.0), // Would need to query from zone controller
        humidity: if humidities.is_empty() {
            None
        } else {
            Some(humidities.iter().sum::<f64>() / humidities.len() as f64)
        },
        air_quality_index: None, // Would need AQI calculation
        co2_level: if co2_levels.is_empty() {
            None
        } else {
            Some(
                (co2_levels.iter().map(|&x| x as u32).sum::<u32>() / co2_levels.len() as u32)
                    as u16,
            )
        },
        mode: HvacMode::Auto,
        fan_speed: FanSpeed::Auto,
        damper_position: None,
        occupied,
        last_update: Utc::now(),
    })
}

/// Control air quality and ventilation
pub async fn control_air_quality(
    context: ToolContext,
    zone_name: String,
    target_co2: Option<u16>,
    min_fresh_air_percent: Option<u8>,
    boost_mode: Option<bool>,
) -> ToolResponse {
    let client = &context.client;

    let mut commands = Vec::new();

    // Set target CO2 level
    if let Some(co2) = target_co2 {
        if !(400..=2000).contains(&co2) {
            return ToolResponse::error("CO2 target must be between 400-2000 ppm".to_string());
        }
        commands.push(("co2_target", format!("ventilation/co2/{co2}")));
    }

    // Set minimum fresh air percentage
    if let Some(fresh_air) = min_fresh_air_percent {
        if fresh_air > 100 {
            return ToolResponse::error("Fresh air percentage must be 0-100".to_string());
        }
        commands.push(("fresh_air", format!("ventilation/fresh_air/{fresh_air}")));
    }

    // Enable/disable boost mode
    if let Some(boost) = boost_mode {
        let boost_cmd = if boost {
            "ventilation/boost/on"
        } else {
            "ventilation/boost/off"
        };
        commands.push(("boost", boost_cmd.to_string()));
    }

    // Send commands
    let mut results = Vec::new();
    for (cmd_type, command) in &commands {
        match client
            .send_command(&format!("zone/{zone_name}"), command)
            .await
        {
            Ok(_) => results.push(json!({
                "command_type": cmd_type,
                "status": "success"
            })),
            Err(e) => results.push(json!({
                "command_type": cmd_type,
                "status": "failed",
                "error": e.to_string()
            })),
        }
    }

    let response_data = json!({
        "zone": zone_name,
        "air_quality_settings": {
            "target_co2_ppm": target_co2,
            "min_fresh_air_percent": min_fresh_air_percent,
            "boost_mode": boost_mode,
        },
        "commands_sent": results,
        "timestamp": Utc::now()
    });

    ToolResponse::success_with_message(
        response_data,
        format!("Updated air quality settings for zone '{zone_name}'"),
    )
}

/// Parse a device into climate-specific format with enhanced capabilities
fn parse_climate_device(device: crate::client::LoxoneDevice) -> ClimateDevice {
    // Extract temperature values from states
    let current_temperature = device
        .states
        .get("tempActual")
        .or_else(|| device.states.get("temperature"))
        .or_else(|| device.states.get("value"))
        .and_then(|v| v.as_f64());

    let target_temperature = device
        .states
        .get("tempTarget")
        .or_else(|| device.states.get("setpoint"))
        .and_then(|v| v.as_f64());

    let humidity = device.states.get("humidity").and_then(|v| v.as_f64());

    let mode = device
        .states
        .get("mode")
        .or_else(|| device.states.get("activeOutput"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let fan_speed = device
        .states
        .get("fanSpeed")
        .or_else(|| device.states.get("fan"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let damper_position = device
        .states
        .get("damperPosition")
        .or_else(|| device.states.get("damper"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u8);

    // Determine zone from room or device name
    let zone = device.room.as_ref().map(|r| determine_zone_from_room(r));

    // Determine capabilities based on device type and states
    let capabilities = DeviceCapabilities {
        heating: device.states.contains_key("heatingOutput")
            || device.device_type.to_lowercase().contains("heat"),
        cooling: device.states.contains_key("coolingOutput")
            || device.device_type.to_lowercase().contains("cool"),
        fan_control: device.states.contains_key("fanSpeed") || device.states.contains_key("fan"),
        humidity_control: device.states.contains_key("humidityTarget"),
        scheduling: device.states.contains_key("schedule"),
        zone_control: device.states.contains_key("zone"),
        occupancy_sensor: device.states.contains_key("occupied")
            || device.states.contains_key("presence"),
        air_quality_sensors: device.states.contains_key("co2") || device.states.contains_key("voc"),
    };

    ClimateDevice {
        uuid: device.uuid,
        name: device.name,
        device_type: device.device_type,
        room: device.room,
        zone,
        current_temperature,
        target_temperature,
        humidity,
        mode,
        fan_speed,
        damper_position,
        capabilities,
        states: device.states,
    }
}
