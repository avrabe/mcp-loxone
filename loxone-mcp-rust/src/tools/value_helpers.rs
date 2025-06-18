//! Helper functions for consistent value parsing across all MCP tools
//!
//! This module provides unified value parsing and formatting utilities
//! to ensure consistency across tools, resources, and the dashboard.

use crate::client::LoxoneDevice;
use crate::services::sensor_registry::SensorType;
use crate::services::value_resolution::UnifiedValueResolver;
use serde_json::{json, Value};
use std::sync::Arc;

/// Tool-specific value resolution result
#[derive(Debug, Clone)]
pub struct ToolValue {
    pub numeric_value: Option<f64>,
    pub display_value: String,
    pub unit: Option<String>,
    pub sensor_type: Option<SensorType>,
    pub confidence: f32,
    pub raw_value: Value,
}

/// Resolve device value using the unified resolver
pub async fn resolve_device_value_for_tool(
    resolver: &Arc<UnifiedValueResolver>,
    device: &LoxoneDevice,
    raw_state: Option<&Value>,
) -> ToolValue {
    // Try to get resolved value from unified resolver
    match resolver.resolve_device_value(&device.uuid).await {
        Ok(resolved) => ToolValue {
            numeric_value: resolved.numeric_value,
            display_value: resolved.formatted_value,
            unit: resolved.unit,
            sensor_type: resolved.sensor_type,
            confidence: resolved.confidence,
            raw_value: resolved.raw_value,
        },
        Err(_) => {
            // Fallback to basic parsing if resolver fails
            fallback_parse_value(device, raw_state)
        }
    }
}

/// Batch resolve multiple device values
pub async fn resolve_batch_values_for_tools(
    resolver: &Arc<UnifiedValueResolver>,
    devices: &[LoxoneDevice],
) -> Vec<(String, ToolValue)> {
    let uuids: Vec<String> = devices.iter().map(|d| d.uuid.clone()).collect();

    match resolver.resolve_batch_values(&uuids).await {
        Ok(resolved_map) => devices
            .iter()
            .map(|device| {
                let tool_value = if let Some(resolved) = resolved_map.get(&device.uuid) {
                    ToolValue {
                        numeric_value: resolved.numeric_value,
                        display_value: resolved.formatted_value.clone(),
                        unit: resolved.unit.clone(),
                        sensor_type: resolved.sensor_type.clone(),
                        confidence: resolved.confidence,
                        raw_value: resolved.raw_value.clone(),
                    }
                } else {
                    fallback_parse_value(device, None)
                };
                (device.uuid.clone(), tool_value)
            })
            .collect(),
        Err(_) => {
            // Fallback to individual parsing
            devices
                .iter()
                .map(|device| {
                    let tool_value = fallback_parse_value(device, None);
                    (device.uuid.clone(), tool_value)
                })
                .collect()
        }
    }
}

/// Format sensor value for human-readable display
pub fn format_sensor_value_display(value: &ToolValue) -> String {
    match &value.sensor_type {
        Some(SensorType::DoorWindowContact) => {
            if let Some(num) = value.numeric_value {
                if num > 0.0 { "OPEN" } else { "CLOSED" }.to_string()
            } else {
                value.display_value.clone()
            }
        }
        Some(SensorType::MotionDetector) => {
            if let Some(num) = value.numeric_value {
                if num > 0.0 {
                    "MOTION DETECTED"
                } else {
                    "NO MOTION"
                }
                .to_string()
            } else {
                value.display_value.clone()
            }
        }
        Some(SensorType::Temperature { .. }) => {
            if let Some(num) = value.numeric_value {
                format!("{:.1}°C", num)
            } else {
                value.display_value.clone()
            }
        }
        Some(SensorType::Humidity { .. }) => {
            if let Some(num) = value.numeric_value {
                format!("{:.0}%", num)
            } else {
                value.display_value.clone()
            }
        }
        Some(SensorType::Illuminance { .. }) => {
            if let Some(num) = value.numeric_value {
                format!("{:.0} Lx", num)
            } else {
                value.display_value.clone()
            }
        }
        _ => value.display_value.clone(),
    }
}

/// Create standardized sensor JSON representation
pub fn create_sensor_json(device: &LoxoneDevice, value: &ToolValue) -> Value {
    json!({
        "uuid": device.uuid,
        "name": device.name,
        "room": device.room.as_deref().unwrap_or("Unknown"),
        "type": device.device_type,
        "category": device.category,
        "sensor_type": value.sensor_type.as_ref().map(|t| {
            let type_str = format!("{:?}", t);
            type_str.split('{').next().unwrap_or("Unknown").to_string()
        }),
        "value": value.numeric_value,
        "display_value": format_sensor_value_display(value),
        "unit": value.unit,
        "raw_value": value.raw_value,
        "confidence": value.confidence,
        "states": device.states,
    })
}

/// Fallback value parsing when unified resolver is not available
fn fallback_parse_value(device: &LoxoneDevice, raw_state: Option<&Value>) -> ToolValue {
    // Try raw state first
    if let Some(state) = raw_state {
        if let Some(numeric) = extract_numeric_from_value(state) {
            return ToolValue {
                numeric_value: Some(numeric),
                display_value: format!("{:.1}", numeric),
                unit: extract_unit_from_value(state),
                sensor_type: None,
                confidence: 0.5,
                raw_value: state.clone(),
            };
        }
    }

    // Try device states
    if let Some(value_state) = device.states.get("value").or(device.states.get("active")) {
        if let Some(numeric) = extract_numeric_from_value(value_state) {
            return ToolValue {
                numeric_value: Some(numeric),
                display_value: format!("{:.1}", numeric),
                unit: device
                    .states
                    .get("unit")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                sensor_type: None,
                confidence: 0.3,
                raw_value: value_state.clone(),
            };
        }
    }

    // Default unknown value
    ToolValue {
        numeric_value: None,
        display_value: "Unknown".to_string(),
        unit: None,
        sensor_type: None,
        confidence: 0.0,
        raw_value: Value::Null,
    }
}

/// Extract numeric value from JSON value
fn extract_numeric_from_value(value: &Value) -> Option<f64> {
    // Direct numeric
    if let Some(num) = value.as_f64() {
        return Some(num);
    }

    // String parsing
    if let Some(str_val) = value.as_str() {
        let cleaned = str_val
            .replace(['°', '%', 'W', 'A', 'V'], "")
            .replace("Lx", "")
            .replace("hPa", "")
            .replace("ppm", "")
            .trim()
            .to_string();
        return cleaned.parse::<f64>().ok();
    }

    // LL.value extraction
    if let Some(ll_obj) = value.get("LL").and_then(|v| v.as_object()) {
        if let Some(value_str) = ll_obj.get("value").and_then(|v| v.as_str()) {
            return extract_numeric_from_value(&Value::String(value_str.to_string()));
        }
    }

    None
}

/// Extract unit from value
fn extract_unit_from_value(value: &Value) -> Option<String> {
    if let Some(str_val) = value.as_str() {
        if str_val.contains('°') {
            return Some("°C".to_string());
        }
        if str_val.contains('%') {
            return Some("%".to_string());
        }
        if str_val.contains("Lx") {
            return Some("Lx".to_string());
        }
        if str_val.contains("hPa") {
            return Some("hPa".to_string());
        }
        if str_val.contains("ppm") {
            return Some("ppm".to_string());
        }
    }
    None
}
