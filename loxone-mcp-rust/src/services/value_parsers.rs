//! Value parsing system for different sensor types
//!
//! This module provides specialized parsers for extracting and formatting
//! sensor values from various Loxone API response formats.

use crate::error::{LoxoneError, Result};
use crate::services::sensor_registry::SensorType;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Registry for value parsers
pub struct ValueParserRegistry {
    parsers: HashMap<String, Arc<dyn ValueParser>>,
}

/// Trait for parsing sensor values
pub trait ValueParser: Send + Sync {
    /// Parse the raw value into a structured format
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue>;

    /// Calculate confidence in the parsed value (0.0-1.0)
    fn confidence(&self, raw_value: &Value) -> f32;
}

/// Parsed sensor value with metadata
#[derive(Debug, Clone)]
pub struct ParsedValue {
    pub numeric_value: Option<f64>,
    pub formatted_value: String,
    pub unit: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl ValueParserRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            parsers: HashMap::new(),
        };

        registry.register_default_parsers();
        registry
    }

    pub fn get_parser(&self, sensor_type: &SensorType) -> Option<&dyn ValueParser> {
        let key = self.sensor_type_to_key(sensor_type);
        self.parsers.get(&key).map(|p| p.as_ref())
    }

    fn register_default_parsers(&mut self) {
        // Environmental sensors
        self.parsers
            .insert("Temperature".to_string(), Arc::new(TemperatureParser));
        self.parsers
            .insert("Humidity".to_string(), Arc::new(HumidityParser));
        self.parsers
            .insert("AirPressure".to_string(), Arc::new(AirPressureParser));
        self.parsers
            .insert("AirQuality".to_string(), Arc::new(AirQualityParser));

        // Light sensors
        self.parsers
            .insert("Illuminance".to_string(), Arc::new(LightParser));

        // Energy monitoring
        self.parsers
            .insert("PowerMeter".to_string(), Arc::new(PowerParser));
        self.parsers
            .insert("EnergyConsumption".to_string(), Arc::new(EnergyParser));
        self.parsers
            .insert("Current".to_string(), Arc::new(CurrentParser));
        self.parsers
            .insert("Voltage".to_string(), Arc::new(VoltageParser));

        // Binary sensors
        self.parsers
            .insert("MotionDetector".to_string(), Arc::new(MotionParser));
        self.parsers
            .insert("DoorWindowContact".to_string(), Arc::new(ContactParser));

        // Position sensors
        self.parsers
            .insert("BlindPosition".to_string(), Arc::new(PositionParser));
        self.parsers
            .insert("WindowPosition".to_string(), Arc::new(PositionParser));

        // Weather sensors
        self.parsers
            .insert("WindSpeed".to_string(), Arc::new(WindSpeedParser));
        self.parsers
            .insert("Rainfall".to_string(), Arc::new(RainfallParser));
    }

    fn sensor_type_to_key(&self, sensor_type: &SensorType) -> String {
        format!("{:?}", sensor_type)
            .split('{')
            .next()
            .unwrap_or("Unknown")
            .to_string()
    }
}

impl Default for ValueParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Temperature sensor parser
pub struct TemperatureParser;

impl ValueParser for TemperatureParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        // Try LL.value first (most reliable for sensors)
        if let Some(ll_obj) = raw_value.get("LL").and_then(|v| v.as_object()) {
            if let Some(value_str) = ll_obj.get("value").and_then(|v| v.as_str()) {
                if let Some(numeric) = extract_temperature(value_str) {
                    return Ok(ParsedValue {
                        numeric_value: Some(numeric),
                        formatted_value: format!("{:.1}°C", numeric),
                        unit: Some("°C".to_string()),
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        // Fallback to direct value
        if let Some(numeric) = raw_value.as_f64() {
            return Ok(ParsedValue {
                numeric_value: Some(numeric),
                formatted_value: format!("{:.1}°C", numeric),
                unit: Some("°C".to_string()),
                metadata: HashMap::new(),
            });
        }

        // Try parsing as string
        if let Some(value_str) = raw_value.as_str() {
            if let Some(numeric) = extract_temperature(value_str) {
                return Ok(ParsedValue {
                    numeric_value: Some(numeric),
                    formatted_value: format!("{:.1}°C", numeric),
                    unit: Some("°C".to_string()),
                    metadata: HashMap::new(),
                });
            }
        }

        Err(LoxoneError::parsing_error("Unable to parse temperature"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() {
            0.9
        } else if raw_value.as_f64().is_some() {
            0.7
        } else if raw_value.as_str().is_some() {
            0.5
        } else {
            0.0
        }
    }
}

/// Humidity sensor parser
pub struct HumidityParser;

impl ValueParser for HumidityParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        if let Some(ll_obj) = raw_value.get("LL").and_then(|v| v.as_object()) {
            if let Some(value_str) = ll_obj.get("value").and_then(|v| v.as_str()) {
                if let Some(numeric) = extract_percentage(value_str) {
                    return Ok(ParsedValue {
                        numeric_value: Some(numeric),
                        formatted_value: format!("{}%", numeric.round() as i32),
                        unit: Some("%".to_string()),
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        if let Some(numeric) = raw_value.as_f64() {
            return Ok(ParsedValue {
                numeric_value: Some(numeric),
                formatted_value: format!("{}%", numeric.round() as i32),
                unit: Some("%".to_string()),
                metadata: HashMap::new(),
            });
        }

        if let Some(value_str) = raw_value.as_str() {
            if let Some(numeric) = extract_percentage(value_str) {
                return Ok(ParsedValue {
                    numeric_value: Some(numeric),
                    formatted_value: format!("{}%", numeric.round() as i32),
                    unit: Some("%".to_string()),
                    metadata: HashMap::new(),
                });
            }
        }

        Err(LoxoneError::parsing_error("Unable to parse humidity"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() {
            0.9
        } else if raw_value.as_f64().is_some() {
            0.7
        } else if raw_value.as_str().is_some() {
            0.5
        } else {
            0.0
        }
    }
}

/// Light/Illuminance sensor parser
pub struct LightParser;

impl ValueParser for LightParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        if let Some(ll_obj) = raw_value.get("LL").and_then(|v| v.as_object()) {
            if let Some(value_str) = ll_obj.get("value").and_then(|v| v.as_str()) {
                if let Some(numeric) = extract_lux(value_str) {
                    return Ok(ParsedValue {
                        numeric_value: Some(numeric),
                        formatted_value: format!("{:.0} Lx", numeric),
                        unit: Some("Lx".to_string()),
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        if let Some(numeric) = raw_value.as_f64() {
            return Ok(ParsedValue {
                numeric_value: Some(numeric),
                formatted_value: format!("{:.0} Lx", numeric),
                unit: Some("Lx".to_string()),
                metadata: HashMap::new(),
            });
        }

        if let Some(value_str) = raw_value.as_str() {
            if let Some(numeric) = extract_lux(value_str) {
                return Ok(ParsedValue {
                    numeric_value: Some(numeric),
                    formatted_value: format!("{:.0} Lx", numeric),
                    unit: Some("Lx".to_string()),
                    metadata: HashMap::new(),
                });
            }
        }

        Err(LoxoneError::parsing_error("Unable to parse illuminance"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() {
            0.9
        } else if raw_value.as_f64().is_some() {
            0.7
        } else {
            0.0
        }
    }
}

/// Motion detector parser (binary sensor)
pub struct MotionParser;

impl ValueParser for MotionParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        let detected = if let Some(bool_val) = raw_value.as_bool() {
            bool_val
        } else if let Some(num_val) = raw_value.as_f64() {
            num_val > 0.0
        } else if let Some(str_val) = raw_value.as_str() {
            str_val == "1" || str_val.to_lowercase() == "true" || str_val.to_lowercase() == "on"
        } else {
            false
        };

        Ok(ParsedValue {
            numeric_value: Some(if detected { 1.0 } else { 0.0 }),
            formatted_value: if detected {
                "Motion Detected".to_string()
            } else {
                "No Motion".to_string()
            },
            unit: None,
            metadata: HashMap::new(),
        })
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.is_boolean() || raw_value.is_number() {
            0.9
        } else if raw_value.is_string() {
            0.7
        } else {
            0.0
        }
    }
}

/// Door/Window contact parser (binary sensor)
pub struct ContactParser;

impl ValueParser for ContactParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        let open = if let Some(bool_val) = raw_value.as_bool() {
            bool_val
        } else if let Some(num_val) = raw_value.as_f64() {
            num_val > 0.0
        } else if let Some(str_val) = raw_value.as_str() {
            str_val == "1" || str_val.to_lowercase() == "true" || str_val.to_lowercase() == "open"
        } else {
            false
        };

        Ok(ParsedValue {
            numeric_value: Some(if open { 1.0 } else { 0.0 }),
            formatted_value: if open {
                "Open".to_string()
            } else {
                "Closed".to_string()
            },
            unit: None,
            metadata: HashMap::new(),
        })
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.is_boolean() || raw_value.is_number() {
            0.9
        } else if raw_value.is_string() {
            0.7
        } else {
            0.0
        }
    }
}

/// Power meter parser
pub struct PowerParser;

impl ValueParser for PowerParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        if let Some(numeric) = extract_numeric_with_unit(raw_value, &["W", "kW"]) {
            let (value, unit) = numeric;
            let watts = if unit == "kW" { value * 1000.0 } else { value };

            return Ok(ParsedValue {
                numeric_value: Some(watts),
                formatted_value: if watts >= 1000.0 {
                    format!("{:.1} kW", watts / 1000.0)
                } else {
                    format!("{:.0} W", watts)
                },
                unit: Some("W".to_string()),
                metadata: HashMap::new(),
            });
        }

        Err(LoxoneError::parsing_error("Unable to parse power"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() {
            0.9
        } else if raw_value.as_f64().is_some() {
            0.7
        } else {
            0.0
        }
    }
}

/// Energy consumption parser
pub struct EnergyParser;

impl ValueParser for EnergyParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        if let Some(numeric) = extract_numeric_with_unit(raw_value, &["Wh", "kWh", "MWh"]) {
            let (value, unit) = numeric;
            let kwh = match unit.as_str() {
                "Wh" => value / 1000.0,
                "MWh" => value * 1000.0,
                _ => value, // kWh
            };

            return Ok(ParsedValue {
                numeric_value: Some(kwh),
                formatted_value: format!("{:.2} kWh", kwh),
                unit: Some("kWh".to_string()),
                metadata: HashMap::new(),
            });
        }

        Err(LoxoneError::parsing_error("Unable to parse energy"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() {
            0.9
        } else if raw_value.as_f64().is_some() {
            0.7
        } else {
            0.0
        }
    }
}

/// Current parser
pub struct CurrentParser;

impl ValueParser for CurrentParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        if let Some(numeric) = extract_numeric_with_unit(raw_value, &["A", "mA"]) {
            let (value, unit) = numeric;
            let amps = if unit == "mA" { value / 1000.0 } else { value };

            return Ok(ParsedValue {
                numeric_value: Some(amps),
                formatted_value: format!("{:.2} A", amps),
                unit: Some("A".to_string()),
                metadata: HashMap::new(),
            });
        }

        Err(LoxoneError::parsing_error("Unable to parse current"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() {
            0.9
        } else if raw_value.as_f64().is_some() {
            0.7
        } else {
            0.0
        }
    }
}

/// Voltage parser
pub struct VoltageParser;

impl ValueParser for VoltageParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        if let Some(numeric) = extract_numeric_with_unit(raw_value, &["V", "mV", "kV"]) {
            let (value, unit) = numeric;
            let volts = match unit.as_str() {
                "mV" => value / 1000.0,
                "kV" => value * 1000.0,
                _ => value, // V
            };

            return Ok(ParsedValue {
                numeric_value: Some(volts),
                formatted_value: format!("{:.1} V", volts),
                unit: Some("V".to_string()),
                metadata: HashMap::new(),
            });
        }

        Err(LoxoneError::parsing_error("Unable to parse voltage"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() {
            0.9
        } else if raw_value.as_f64().is_some() {
            0.7
        } else {
            0.0
        }
    }
}

/// Position parser (for blinds, shutters, etc.)
pub struct PositionParser;

impl ValueParser for PositionParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        if let Some(numeric) = extract_percentage_or_number(raw_value) {
            let percentage = numeric.clamp(0.0, 100.0);

            return Ok(ParsedValue {
                numeric_value: Some(percentage),
                formatted_value: format!("{}%", percentage.round() as i32),
                unit: Some("%".to_string()),
                metadata: HashMap::new(),
            });
        }

        Err(LoxoneError::parsing_error("Unable to parse position"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.as_f64().is_some() {
            0.9
        } else if raw_value.as_str().is_some() {
            0.7
        } else {
            0.0
        }
    }
}

/// Air pressure parser
pub struct AirPressureParser;

impl ValueParser for AirPressureParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        if let Some(numeric) = extract_numeric_with_unit(raw_value, &["hPa", "mbar", "mmHg", "PSI"])
        {
            let (value, unit) = numeric;

            return Ok(ParsedValue {
                numeric_value: Some(value),
                formatted_value: format!("{:.0} {}", value, unit),
                unit: Some(unit),
                metadata: HashMap::new(),
            });
        }

        // Default to hPa if no unit
        if let Some(numeric) = raw_value.as_f64() {
            return Ok(ParsedValue {
                numeric_value: Some(numeric),
                formatted_value: format!("{:.0} hPa", numeric),
                unit: Some("hPa".to_string()),
                metadata: HashMap::new(),
            });
        }

        Err(LoxoneError::parsing_error("Unable to parse air pressure"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() {
            0.9
        } else if raw_value.as_f64().is_some() {
            0.7
        } else {
            0.0
        }
    }
}

/// Air quality parser
pub struct AirQualityParser;

impl ValueParser for AirQualityParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        if let Some(numeric) = extract_numeric_with_unit(raw_value, &["ppm", "µg/m³", "AQI"]) {
            let (value, unit) = numeric;

            return Ok(ParsedValue {
                numeric_value: Some(value),
                formatted_value: format!("{:.0} {}", value, unit),
                unit: Some(unit),
                metadata: HashMap::new(),
            });
        }

        // Default to ppm for CO2
        if let Some(numeric) = raw_value.as_f64() {
            return Ok(ParsedValue {
                numeric_value: Some(numeric),
                formatted_value: format!("{:.0} ppm", numeric),
                unit: Some("ppm".to_string()),
                metadata: HashMap::new(),
            });
        }

        Err(LoxoneError::parsing_error("Unable to parse air quality"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() {
            0.9
        } else if raw_value.as_f64().is_some() {
            0.7
        } else {
            0.0
        }
    }
}

/// Wind speed parser
pub struct WindSpeedParser;

impl ValueParser for WindSpeedParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        if let Some(numeric) = extract_numeric_with_unit(raw_value, &["m/s", "km/h", "mph"]) {
            let (value, unit) = numeric;

            return Ok(ParsedValue {
                numeric_value: Some(value),
                formatted_value: format!("{:.1} {}", value, unit),
                unit: Some(unit),
                metadata: HashMap::new(),
            });
        }

        // Default to m/s
        if let Some(numeric) = raw_value.as_f64() {
            return Ok(ParsedValue {
                numeric_value: Some(numeric),
                formatted_value: format!("{:.1} m/s", numeric),
                unit: Some("m/s".to_string()),
                metadata: HashMap::new(),
            });
        }

        Err(LoxoneError::parsing_error("Unable to parse wind speed"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() {
            0.9
        } else if raw_value.as_f64().is_some() {
            0.7
        } else {
            0.0
        }
    }
}

/// Rainfall parser
pub struct RainfallParser;

impl ValueParser for RainfallParser {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue> {
        if let Some(numeric) = extract_numeric_with_unit(raw_value, &["mm", "cm", "in"]) {
            let (value, unit) = numeric;

            return Ok(ParsedValue {
                numeric_value: Some(value),
                formatted_value: format!("{:.1} {}", value, unit),
                unit: Some(unit),
                metadata: HashMap::new(),
            });
        }

        // Default to mm
        if let Some(numeric) = raw_value.as_f64() {
            return Ok(ParsedValue {
                numeric_value: Some(numeric),
                formatted_value: format!("{:.1} mm", numeric),
                unit: Some("mm".to_string()),
                metadata: HashMap::new(),
            });
        }

        Err(LoxoneError::parsing_error("Unable to parse rainfall"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() {
            0.9
        } else if raw_value.as_f64().is_some() {
            0.7
        } else {
            0.0
        }
    }
}

// Helper functions for value extraction

fn extract_temperature(value_str: &str) -> Option<f64> {
    value_str
        .replace("°C", "")
        .replace("°F", "")
        .replace("°", "")
        .replace("C", "")
        .trim()
        .parse()
        .ok()
}

fn extract_percentage(value_str: &str) -> Option<f64> {
    value_str.replace('%', "").trim().parse().ok()
}

fn extract_lux(value_str: &str) -> Option<f64> {
    value_str
        .replace("Lx", "")
        .replace("lx", "")
        .replace("Lux", "")
        .trim()
        .parse()
        .ok()
}

fn extract_numeric_with_unit(raw_value: &Value, units: &[&str]) -> Option<(f64, String)> {
    // Try LL.value first
    if let Some(ll_obj) = raw_value.get("LL").and_then(|v| v.as_object()) {
        if let Some(value_str) = ll_obj.get("value").and_then(|v| v.as_str()) {
            return parse_value_with_unit(value_str, units);
        }
    }

    // Try direct string
    if let Some(value_str) = raw_value.as_str() {
        return parse_value_with_unit(value_str, units);
    }

    // Try direct number (no unit)
    if let Some(numeric) = raw_value.as_f64() {
        return Some((numeric, units.first()?.to_string()));
    }

    None
}

fn parse_value_with_unit(value_str: &str, units: &[&str]) -> Option<(f64, String)> {
    for unit in units {
        if value_str.contains(unit) {
            let numeric_str = value_str.replace(unit, "").trim().to_string();
            if let Ok(value) = numeric_str.parse::<f64>() {
                return Some((value, unit.to_string()));
            }
        }
    }

    // Try parsing without unit
    if let Ok(value) = value_str.trim().parse::<f64>() {
        return Some((value, units.first()?.to_string()));
    }

    None
}

fn extract_percentage_or_number(raw_value: &Value) -> Option<f64> {
    if let Some(ll_obj) = raw_value.get("LL").and_then(|v| v.as_object()) {
        if let Some(value_str) = ll_obj.get("value").and_then(|v| v.as_str()) {
            return extract_percentage(value_str);
        }
    }

    if let Some(numeric) = raw_value.as_f64() {
        return Some(numeric);
    }

    if let Some(value_str) = raw_value.as_str() {
        return extract_percentage(value_str);
    }

    None
}
