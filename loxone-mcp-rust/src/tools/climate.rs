//! Climate control MCP tools
//!
//! Tools for HVAC control, temperature monitoring, and climate management.

use crate::tools::{DeviceFilter, ToolContext, ToolResponse};
// use rmcp::tool; // TODO: Re-enable when rmcp API is clarified
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Climate device information
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

    /// Current temperature (if available)
    pub current_temperature: Option<f64>,

    /// Target temperature (if available)
    pub target_temperature: Option<f64>,

    /// Operating mode (if available)
    pub mode: Option<String>,

    /// States from the device
    pub states: HashMap<String, serde_json::Value>,
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
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_climate_control(context: ToolContext) -> ToolResponse {
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

    let message = format!(
        "Climate system: {} devices ({} room controllers, {} sensors, {} HVAC devices)",
        overview.total_devices,
        overview.room_controllers.len(),
        overview.temperature_sensors.len(),
        overview.hvac_devices.len()
    );

    ToolResponse::success_with_message(serde_json::to_value(overview).unwrap(), message)
}

/// Get climate status for a specific room
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_room_climate(context: ToolContext, room_name: String) -> ToolResponse {
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
        return ToolResponse::error(format!("No climate devices found in room '{}'", room_name));
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
            format!(" (current: {:.1}°C)", temp)
        } else {
            String::new()
        };
        format!(
            "Room '{}' climate: 1 controller, {} sensors{}",
            room_name,
            sensors.len(),
            temp_info
        )
    } else {
        format!(
            "Room '{}' climate: {} sensors (no controller)",
            room_name,
            sensors.len()
        )
    };

    ToolResponse::success_with_message(response_data, message)
}

/// Set target temperature for a room
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn set_room_temperature(
    context: ToolContext,
    room_name: String,
    // #[description("Target temperature in Celsius")] // TODO: Re-enable when rmcp API is clarified
    temperature: f64,
) -> ToolResponse {
    // Validate temperature range
    if !(5.0..=35.0).contains(&temperature) {
        return ToolResponse::error(format!(
            "Invalid temperature {}°C. Must be between 5°C and 35°C",
            temperature
        ));
    }

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
            return ToolResponse::error(format!("No room controller found in room '{}'", room_name))
        }
    };

    // Send temperature set command
    let command = format!("setpoint/{}", temperature);

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
        Err(e) => return ToolResponse::error(format!("Failed to send command: {}", e)),
    };

    ToolResponse::success_with_message(
        result,
        format!(
            "Set target temperature to {:.1}°C for room '{}'",
            temperature, room_name
        ),
    )
}

/// Get temperature readings from all sensors
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_temperature_readings(context: ToolContext) -> ToolResponse {
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

/// Control heating/cooling mode for a room
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn set_room_mode(
    context: ToolContext,
    room_name: String,
    // #[description("Mode: heating, cooling, auto, off")] // TODO: Re-enable when rmcp API is clarified
    mode: String,
) -> ToolResponse {
    // Validate mode
    let valid_modes = ["heating", "cooling", "auto", "off"];
    if !valid_modes.contains(&mode.as_str()) {
        return ToolResponse::error(format!(
            "Invalid mode '{}'. Valid modes: {}",
            mode,
            valid_modes.join(", ")
        ));
    }

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
            return ToolResponse::error(format!("No room controller found in room '{}'", room_name))
        }
    };

    // Send mode command
    let command = format!("mode/{}", mode);

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
                    "mode": mode,
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
        Err(e) => return ToolResponse::error(format!("Failed to send command: {}", e)),
    };

    ToolResponse::success_with_message(
        result,
        format!("Set mode to '{}' for room '{}'", mode, room_name),
    )
}

/// Parse a device into climate-specific format
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

    let mode = device
        .states
        .get("mode")
        .or_else(|| device.states.get("activeOutput"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    ClimateDevice {
        uuid: device.uuid,
        name: device.name,
        device_type: device.device_type,
        room: device.room,
        current_temperature,
        target_temperature,
        mode,
        states: device.states,
    }
}
