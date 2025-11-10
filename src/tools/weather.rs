//! Weather monitoring MCP tools
//!
//! This module provides weather data access using Loxone's native weather devices.
//! Weather data is sourced from connected Loxone weather stations and sensors,
//! not from external APIs. This ensures accurate local environmental readings.
//!
//! ## Available Tools:
//! - Control weather device settings
//! - Calibrate weather sensors
//! - Configure weather alerts
//!
//! ## Resources (Read-only data):
//! - loxone://weather/current - Current weather from native Loxone devices
//! - loxone://weather/outdoor-conditions - Environmental conditions with analysis
//! - loxone://sensors/weather-station - Weather station device data

use crate::tools::{ToolContext, ToolResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Loxone weather device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneWeatherConfig {
    /// Temperature calibration offset in Celsius
    pub temperature_offset: f64,
    /// Humidity calibration offset in percentage
    pub humidity_offset: f64,
    /// Pressure calibration offset in hPa
    pub pressure_offset: f64,
    /// Wind speed calibration factor
    pub wind_speed_factor: f64,
    /// Preferred temperature unit (celsius/fahrenheit)
    pub temperature_unit: TemperatureUnit,
    /// Update interval for weather readings in seconds
    pub update_interval: u64,
}

/// Temperature unit enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemperatureUnit {
    Celsius,
    Fahrenheit,
}

impl Default for LoxoneWeatherConfig {
    fn default() -> Self {
        Self {
            temperature_offset: 0.0,
            humidity_offset: 0.0,
            pressure_offset: 0.0,
            wind_speed_factor: 1.0,
            temperature_unit: TemperatureUnit::Celsius,
            update_interval: 60, // 1 minute default
        }
    }
}

/// Weather sensor types found in Loxone systems
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoxoneWeatherSensorType {
    /// Main weather station with multiple sensors
    WeatherStation,
    /// Temperature sensor only
    TemperatureSensor,
    /// Humidity sensor only
    HumiditySensor,
    /// Wind speed/direction sensor
    WindSensor,
    /// Rain gauge sensor
    RainSensor,
    /// Pressure sensor
    PressureSensor,
    /// UV index sensor
    UvSensor,
    /// Solar radiation sensor
    SolarSensor,
    /// Multi-purpose analog sensor with weather data
    AnalogWeatherSensor,
}

/// Loxone weather data from native devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneWeatherData {
    /// Location/site name from Loxone configuration
    pub location: String,
    /// Current weather conditions from devices
    pub current: WeatherData,
    /// Connected weather devices
    pub devices: Vec<WeatherDeviceInfo>,
    /// Data quality assessment
    pub data_quality: WeatherDataQuality,
    /// Last successful update from devices
    pub last_update: chrono::DateTime<chrono::Utc>,
}

/// Weather device information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherDeviceInfo {
    /// Device UUID
    pub uuid: String,
    /// Device name
    pub name: String,
    /// Device type
    pub device_type: String,
    /// Sensor type classification
    pub sensor_type: LoxoneWeatherSensorType,
    /// Room/location of device
    pub location: Option<String>,
    /// Device status (online/offline/error)
    pub status: String,
    /// Available weather parameters
    pub parameters: Vec<String>,
    /// Last successful reading timestamp
    pub last_reading: Option<chrono::DateTime<chrono::Utc>>,
    /// Device-specific calibration applied
    pub calibration: Option<serde_json::Value>,
}

/// Weather data quality assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherDataQuality {
    /// Overall quality score (0-100)
    pub score: u32,
    /// Number of active sensors
    pub active_sensors: u32,
    /// Number of sensors with recent data
    pub recent_data_count: u32,
    /// Number of sensors with stale data
    pub stale_data_count: u32,
    /// Quality issues detected
    pub issues: Vec<String>,
    /// Recommendations for improvement
    pub recommendations: Vec<String>,
}

/// Weather data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherData {
    /// Current temperature
    pub temperature: Option<f64>,

    /// Humidity percentage
    pub humidity: Option<f64>,

    /// Wind speed
    pub wind_speed: Option<f64>,

    /// Wind direction
    pub wind_direction: Option<f64>,

    /// Precipitation amount
    pub precipitation: Option<f64>,

    /// Atmospheric pressure
    pub pressure: Option<f64>,

    /// UV index
    pub uv_index: Option<f64>,

    /// Solar radiation
    pub solar_radiation: Option<f64>,

    /// Weather description
    pub description: Option<String>,

    /// Last update timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Historical weather data point from Loxone devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherHistoryPoint {
    /// Reading timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Device UUID that provided the reading
    pub device_uuid: String,
    /// Sensor parameter name
    pub parameter: String,
    /// Measured value
    pub value: f64,
    /// Unit of measurement
    pub unit: Option<String>,
    /// Data quality indicator
    pub quality: f64, // 0.0 to 1.0
}

/// Calibrate weather sensor with offset values
pub async fn calibrate_weather_sensor(
    context: ToolContext,
    device_uuid: String,
    temperature_offset: Option<f64>,
    humidity_offset: Option<f64>,
    pressure_offset: Option<f64>,
    wind_speed_factor: Option<f64>,
) -> ToolResponse {
    // Validate device exists and is a weather device
    let device = match context.context.get_device(&device_uuid).await {
        Ok(Some(device)) => {
            if !is_weather_device(&device) {
                return ToolResponse::error(format!(
                    "Device {} is not a weather device",
                    device.name
                ));
            }
            device
        }
        Ok(None) => return ToolResponse::error(format!("Device {device_uuid} not found")),
        Err(e) => return ToolResponse::error(format!("Failed to get device: {e}")),
    };

    // Apply calibration offsets to device
    let mut calibration_commands = Vec::new();
    let mut applied_calibrations = HashMap::new();

    if let Some(temp_offset) = temperature_offset {
        let command = format!("calibrate/temperature/{temp_offset}");
        calibration_commands.push(command.clone());
        applied_calibrations.insert("temperature_offset".to_string(), temp_offset);
    }

    if let Some(humid_offset) = humidity_offset {
        let command = format!("calibrate/humidity/{humid_offset}");
        calibration_commands.push(command.clone());
        applied_calibrations.insert("humidity_offset".to_string(), humid_offset);
    }

    if let Some(press_offset) = pressure_offset {
        let command = format!("calibrate/pressure/{press_offset}");
        calibration_commands.push(command.clone());
        applied_calibrations.insert("pressure_offset".to_string(), press_offset);
    }

    if let Some(wind_factor) = wind_speed_factor {
        let command = format!("calibrate/wind_speed/{wind_factor}");
        calibration_commands.push(command.clone());
        applied_calibrations.insert("wind_speed_factor".to_string(), wind_factor);
    }

    if calibration_commands.is_empty() {
        return ToolResponse::error("No calibration parameters provided".to_string());
    }

    // Execute calibration commands
    let mut results = Vec::new();
    for command in &calibration_commands {
        match context.send_device_command(&device.uuid, command).await {
            Ok(_) => results.push(serde_json::json!({
                "command": command,
                "status": "success"
            })),
            Err(e) => results.push(serde_json::json!({
                "command": command,
                "status": "error",
                "error": e.to_string()
            })),
        }
    }

    let success_count = results
        .iter()
        .filter(|r| r.get("status").and_then(|s| s.as_str()) == Some("success"))
        .count();

    let response_data = serde_json::json!({
        "device": {
            "uuid": device.uuid,
            "name": device.name,
            "type": device.device_type
        },
        "calibration_applied": applied_calibrations,
        "command_results": results,
        "success_count": success_count,
        "total_commands": calibration_commands.len(),
        "timestamp": chrono::Utc::now()
    });

    if success_count == calibration_commands.len() {
        ToolResponse::success_with_message(
            response_data,
            format!("Successfully calibrated weather device '{}'", device.name),
        )
    } else {
        ToolResponse::error(format!(
            "Calibration partially failed: {}/{} commands succeeded",
            success_count,
            calibration_commands.len()
        ))
    }
}

/// Reset weather sensor calibration to defaults
pub async fn reset_weather_calibration(context: ToolContext, device_uuid: String) -> ToolResponse {
    // Validate device exists and is a weather device
    let device = match context.context.get_device(&device_uuid).await {
        Ok(Some(device)) => {
            if !is_weather_device(&device) {
                return ToolResponse::error(format!(
                    "Device {} is not a weather device",
                    device.name
                ));
            }
            device
        }
        Ok(None) => return ToolResponse::error(format!("Device {device_uuid} not found")),
        Err(e) => return ToolResponse::error(format!("Failed to get device: {e}")),
    };

    // Reset all calibrations to default values
    let reset_commands = vec![
        "calibrate/temperature/0.0",
        "calibrate/humidity/0.0",
        "calibrate/pressure/0.0",
        "calibrate/wind_speed/1.0",
    ];

    let mut results = Vec::new();
    for command in &reset_commands {
        match context.send_device_command(&device.uuid, command).await {
            Ok(_) => results.push(serde_json::json!({
                "command": command,
                "status": "success"
            })),
            Err(e) => results.push(serde_json::json!({
                "command": command,
                "status": "error",
                "error": e.to_string()
            })),
        }
    }

    let success_count = results
        .iter()
        .filter(|r| r.get("status").and_then(|s| s.as_str()) == Some("success"))
        .count();

    let response_data = serde_json::json!({
        "device": {
            "uuid": device.uuid,
            "name": device.name,
            "type": device.device_type
        },
        "reset_commands": results,
        "success_count": success_count,
        "total_commands": reset_commands.len(),
        "timestamp": chrono::Utc::now()
    });

    if success_count == reset_commands.len() {
        ToolResponse::success_with_message(
            response_data,
            format!(
                "Successfully reset calibration for weather device '{}'",
                device.name
            ),
        )
    } else {
        ToolResponse::error(format!(
            "Calibration reset partially failed: {}/{} commands succeeded",
            success_count,
            reset_commands.len()
        ))
    }
}

/// Get available weather devices in the system
pub async fn get_weather_devices(context: ToolContext) -> ToolResponse {
    // Get all weather-related devices
    let devices = context.context.devices.read().await;
    let weather_devices: Vec<_> = devices
        .values()
        .filter(|device| is_weather_device(device))
        .map(|device| {
            serde_json::json!({
                "uuid": device.uuid,
                "name": device.name,
                "type": device.device_type,
                "room": device.room,
                "sensor_type": classify_weather_sensor_type(device),
                "available_parameters": get_weather_parameters(device),
                "current_states": device.states
            })
        })
        .collect();

    if weather_devices.is_empty() {
        return ToolResponse::error("No weather devices found in Loxone system".to_string());
    }

    ToolResponse::success(serde_json::json!({
        "weather_devices": weather_devices,
        "device_count": weather_devices.len(),
        "timestamp": chrono::Utc::now()
    }))
}

// Helper functions for weather device management

/// Check if device is a weather device
fn is_weather_device(device: &crate::client::LoxoneDevice) -> bool {
    let weather_types = ["WeatherStation", "Sensor", "TempSensor", "HumiditySensor"];
    let weather_keywords = ["weather", "temp", "humidity", "wind", "rain", "pressure"];

    // Check by type
    if weather_types
        .iter()
        .any(|&t| device.device_type.contains(t))
    {
        return true;
    }

    // Check by name keywords
    let device_name = device.name.to_lowercase();
    weather_keywords
        .iter()
        .any(|&keyword| device_name.contains(keyword))
}

/// Classify weather sensor type based on device properties
fn classify_weather_sensor_type(device: &crate::client::LoxoneDevice) -> LoxoneWeatherSensorType {
    let device_type = device.device_type.to_lowercase();
    let device_name = device.name.to_lowercase();

    if device_type.contains("weatherstation") || device_name.contains("weather") {
        LoxoneWeatherSensorType::WeatherStation
    } else if device_type.contains("tempsensor") || device_name.contains("temp") {
        LoxoneWeatherSensorType::TemperatureSensor
    } else if device_type.contains("humidity") || device_name.contains("humid") {
        LoxoneWeatherSensorType::HumiditySensor
    } else if device_name.contains("wind") {
        LoxoneWeatherSensorType::WindSensor
    } else if device_name.contains("rain") || device_name.contains("precipitation") {
        LoxoneWeatherSensorType::RainSensor
    } else if device_name.contains("pressure") || device_name.contains("baro") {
        LoxoneWeatherSensorType::PressureSensor
    } else if device_name.contains("uv") {
        LoxoneWeatherSensorType::UvSensor
    } else if device_name.contains("solar") {
        LoxoneWeatherSensorType::SolarSensor
    } else {
        LoxoneWeatherSensorType::AnalogWeatherSensor
    }
}

/// Get available weather parameters for a device
fn get_weather_parameters(device: &crate::client::LoxoneDevice) -> Vec<String> {
    let mut parameters = Vec::new();

    for state_name in device.states.keys() {
        let state_lower = state_name.to_lowercase();
        if state_lower.contains("temp") {
            parameters.push("temperature".to_string());
        } else if state_lower.contains("humid") {
            parameters.push("humidity".to_string());
        } else if state_lower.contains("pressure") || state_lower.contains("baro") {
            parameters.push("pressure".to_string());
        } else if state_lower.contains("wind") {
            if state_lower.contains("speed") {
                parameters.push("wind_speed".to_string());
            } else if state_lower.contains("direction") {
                parameters.push("wind_direction".to_string());
            } else {
                parameters.push("wind".to_string());
            }
        } else if state_lower.contains("rain") || state_lower.contains("precipitation") {
            parameters.push("precipitation".to_string());
        } else if state_lower.contains("uv") {
            parameters.push("uv_index".to_string());
        } else if state_lower.contains("solar") {
            parameters.push("solar_radiation".to_string());
        }
    }

    parameters.sort();
    parameters.dedup();
    parameters
}
