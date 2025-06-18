//! Sensor type registry and detection system
//!
//! This module provides comprehensive sensor type classification and detection
//! for all Loxone devices, replacing the fragmented detection logic scattered
//! across the codebase.

use crate::client::LoxoneDevice;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Comprehensive sensor type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SensorType {
    // Environmental sensors
    Temperature {
        unit: TemperatureUnit,
        range: (f64, f64), // min, max expected values
    },
    Humidity {
        range: (f64, f64), // 0-100% typically
    },
    AirPressure {
        unit: PressureUnit,
        range: (f64, f64),
    },
    AirQuality {
        scale: AirQualityScale,
    },

    // Light sensors
    Illuminance {
        unit: LightUnit,
        range: (f64, f64), // 0-100000 Lx typically
    },
    UVIndex,

    // Motion and presence
    MotionDetector,
    PresenceSensor,

    // Contact and position
    DoorWindowContact,
    WindowPosition {
        range: (f64, f64),
    }, // 0-100%
    BlindPosition {
        range: (f64, f64),
    }, // 0-100%

    // Energy monitoring
    PowerMeter {
        unit: PowerUnit,
    },
    EnergyConsumption {
        unit: EnergyUnit,
    },
    Current {
        unit: CurrentUnit,
    },
    Voltage {
        unit: VoltageUnit,
    },

    // Weather
    WindSpeed {
        unit: SpeedUnit,
    },
    Rainfall {
        unit: VolumeUnit,
    },

    // Sound
    SoundLevel {
        unit: SoundUnit,
    },

    // Unknown with learning metadata
    Unknown {
        device_type: String,
        detected_patterns: Vec<String>,
        sample_values: Vec<String>,
        confidence_score: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TemperatureUnit {
    Celsius,
    Fahrenheit,
    Kelvin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LightUnit {
    Lux,
    FootCandles,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PowerUnit {
    Watts,
    Kilowatts,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EnergyUnit {
    WattHours,
    KilowattHours,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CurrentUnit {
    Amperes,
    Milliamperes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VoltageUnit {
    Volts,
    Millivolts,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PressureUnit {
    Hectopascals,
    MillimetersOfMercury,
    PoundsPerSquareInch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SpeedUnit {
    MetersPerSecond,
    KilometersPerHour,
    MilesPerHour,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VolumeUnit {
    Millimeters,
    Inches,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SoundUnit {
    Decibels,
    DecibelsAWeighted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AirQualityScale {
    AQI,    // Air Quality Index 0-500
    PM25,   // PM2.5 µg/m³
    CO2PPM, // CO2 parts per million
}

/// Detection rule for sensor type identification
pub struct SensorDetectionRule {
    pub name_patterns: Vec<String>,
    pub device_type_patterns: Vec<String>,
    pub sensor_type: SensorType,
    pub confidence: f32,
}

/// Registry for sensor type detection and management
pub struct SensorTypeRegistry {
    type_mappings: HashMap<String, SensorType>,
    detection_rules: Vec<SensorDetectionRule>,
    learned_types: HashMap<String, SensorType>,
}

impl SensorTypeRegistry {
    /// Create a new sensor type registry with default rules
    pub fn new() -> Self {
        let mut registry = Self {
            type_mappings: HashMap::new(),
            detection_rules: Vec::new(),
            learned_types: HashMap::new(),
        };

        registry.initialize_default_rules();
        registry
    }

    /// Detect sensor type for a device
    pub async fn detect_sensor_type(&self, device: &LoxoneDevice) -> Result<Option<SensorType>> {
        // Check explicit mappings first
        if let Some(sensor_type) = self.type_mappings.get(&device.uuid) {
            return Ok(Some(sensor_type.clone()));
        }

        // Check learned types
        if let Some(sensor_type) = self.learned_types.get(&device.uuid) {
            return Ok(Some(sensor_type.clone()));
        }

        // Apply detection rules
        let mut best_match: Option<(SensorType, f32)> = None;

        for rule in &self.detection_rules {
            let confidence = self.calculate_rule_confidence(rule, device);
            if confidence > 0.5 {
                // Minimum confidence threshold
                if let Some((_, best_confidence)) = &best_match {
                    if confidence > *best_confidence {
                        best_match = Some((rule.sensor_type.clone(), confidence));
                    }
                } else {
                    best_match = Some((rule.sensor_type.clone(), confidence));
                }
            }
        }

        Ok(best_match.map(|(sensor_type, _)| sensor_type))
    }

    /// Initialize default detection rules
    fn initialize_default_rules(&mut self) {
        // Temperature sensors
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "temperatur".to_string(),
                "temp".to_string(),
                "temperature".to_string(),
                "raumtemperatur".to_string(),
                "außentemperatur".to_string(),
            ],
            device_type_patterns: vec!["analog".to_string(), "sensor".to_string()],
            sensor_type: SensorType::Temperature {
                unit: TemperatureUnit::Celsius,
                range: (-40.0, 85.0),
            },
            confidence: 0.9,
        });

        // Humidity sensors
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "luftfeuchte".to_string(),
                "humidity".to_string(),
                "feuchte".to_string(),
                "feuchtigkeit".to_string(),
                "luftfeuchtigkeit".to_string(),
            ],
            device_type_patterns: vec!["analog".to_string(), "sensor".to_string()],
            sensor_type: SensorType::Humidity {
                range: (0.0, 100.0),
            },
            confidence: 0.9,
        });

        // Light sensors
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "helligkeit".to_string(),
                "light".to_string(),
                "brightness".to_string(),
                "lux".to_string(),
                "beleuchtung".to_string(),
                "lichtsensor".to_string(),
            ],
            device_type_patterns: vec!["analog".to_string(), "sensor".to_string()],
            sensor_type: SensorType::Illuminance {
                unit: LightUnit::Lux,
                range: (0.0, 100000.0),
            },
            confidence: 0.9,
        });

        // Motion sensors
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "motion".to_string(),
                "bewegung".to_string(),
                "pir".to_string(),
                "bewegungsmelder".to_string(),
                "präsenz".to_string(),
                "presence".to_string(),
            ],
            device_type_patterns: vec!["digital".to_string(), "binary".to_string()],
            sensor_type: SensorType::MotionDetector,
            confidence: 0.8,
        });

        // Door/Window contact sensors
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "kontakt".to_string(),
                "contact".to_string(),
                "fenster".to_string(),
                "window".to_string(),
                "tür".to_string(),
                "door".to_string(),
                "türkontakt".to_string(),
                "fensterkontakt".to_string(),
            ],
            device_type_patterns: vec!["digital".to_string(), "binary".to_string()],
            sensor_type: SensorType::DoorWindowContact,
            confidence: 0.8,
        });

        // Power meters
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "power".to_string(),
                "leistung".to_string(),
                "watt".to_string(),
                "stromverbrauch".to_string(),
                "verbrauch".to_string(),
            ],
            device_type_patterns: vec!["analog".to_string(), "meter".to_string()],
            sensor_type: SensorType::PowerMeter {
                unit: PowerUnit::Watts,
            },
            confidence: 0.8,
        });

        // Energy consumption
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "energy".to_string(),
                "energie".to_string(),
                "kwh".to_string(),
                "kilowattstunde".to_string(),
                "stromzähler".to_string(),
            ],
            device_type_patterns: vec!["analog".to_string(), "meter".to_string()],
            sensor_type: SensorType::EnergyConsumption {
                unit: EnergyUnit::KilowattHours,
            },
            confidence: 0.8,
        });

        // Wind speed sensors
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "wind".to_string(),
                "windgeschwindigkeit".to_string(),
                "windspeed".to_string(),
                "anemometer".to_string(),
            ],
            device_type_patterns: vec!["analog".to_string(), "weather".to_string()],
            sensor_type: SensorType::WindSpeed {
                unit: SpeedUnit::MetersPerSecond,
            },
            confidence: 0.8,
        });

        // Rainfall sensors
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "regen".to_string(),
                "rain".to_string(),
                "niederschlag".to_string(),
                "rainfall".to_string(),
                "regenmesser".to_string(),
            ],
            device_type_patterns: vec!["analog".to_string(), "weather".to_string()],
            sensor_type: SensorType::Rainfall {
                unit: VolumeUnit::Millimeters,
            },
            confidence: 0.8,
        });

        // Air pressure sensors
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "luftdruck".to_string(),
                "pressure".to_string(),
                "barometer".to_string(),
                "air pressure".to_string(),
            ],
            device_type_patterns: vec!["analog".to_string(), "weather".to_string()],
            sensor_type: SensorType::AirPressure {
                unit: PressureUnit::Hectopascals,
                range: (900.0, 1100.0),
            },
            confidence: 0.8,
        });

        // Air quality sensors
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "luftqualität".to_string(),
                "air quality".to_string(),
                "co2".to_string(),
                "voc".to_string(),
                "luftgüte".to_string(),
            ],
            device_type_patterns: vec!["analog".to_string(), "sensor".to_string()],
            sensor_type: SensorType::AirQuality {
                scale: AirQualityScale::CO2PPM,
            },
            confidence: 0.8,
        });

        // Blind/Shutter position
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "jalousie".to_string(),
                "rolladen".to_string(),
                "blind".to_string(),
                "shutter".to_string(),
                "position".to_string(),
                "fenster".to_string(),
                "beschattung".to_string(),
            ],
            device_type_patterns: vec![
                "analog".to_string(),
                "actuator".to_string(),
                "jalousie".to_string(),
            ],
            sensor_type: SensorType::BlindPosition {
                range: (0.0, 100.0),
            },
            confidence: 0.7,
        });

        // Light control sensors
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "lichtsteuerung".to_string(),
                "licht".to_string(),
                "beleuchtung".to_string(),
                "light control".to_string(),
                "dimmer".to_string(),
                "lamp".to_string(),
                "led".to_string(),
            ],
            device_type_patterns: vec![
                "lightcontroller".to_string(),
                "dimmer".to_string(),
                "switch".to_string(),
            ],
            sensor_type: SensorType::Illuminance {
                unit: LightUnit::Lux,
                range: (0.0, 100.0), // For dimmer percentage
            },
            confidence: 0.8,
        });

        // Ventilation and climate control
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "bwm".to_string(),
                "nachlauf".to_string(),
                "lüftung".to_string(),
                "ventilation".to_string(),
                "climate".to_string(),
                "klima".to_string(),
                "heizung".to_string(),
                "heating".to_string(),
                "stellantrieb".to_string(),
                "intelligente raumregelung".to_string(),
            ],
            device_type_patterns: vec![
                "analog".to_string(),
                "digital".to_string(),
                "timer".to_string(),
            ],
            sensor_type: SensorType::AirQuality {
                scale: AirQualityScale::CO2PPM,
            },
            confidence: 0.6,
        });

        // Water detection and management
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "wasser".to_string(),
                "water".to_string(),
                "wassermeldezentrale".to_string(),
                "leak".to_string(),
                "feuchtigkeit".to_string(),
                "moisture".to_string(),
            ],
            device_type_patterns: vec!["digital".to_string(), "sensor".to_string()],
            sensor_type: SensorType::DoorWindowContact, // Repurposed for water detection
            confidence: 0.7,
        });

        // Security and alarm systems
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "alarmanlage".to_string(),
                "alarm".to_string(),
                "sicherheit".to_string(),
                "security".to_string(),
                "brandzentrale".to_string(),
                "fire".to_string(),
                "smoke".to_string(),
                "rauch".to_string(),
                "überwachung".to_string(),
                "monitoring".to_string(),
            ],
            device_type_patterns: vec![
                "digital".to_string(),
                "security".to_string(),
                "alarm".to_string(),
            ],
            sensor_type: SensorType::MotionDetector, // Repurposed for security detection
            confidence: 0.8,
        });

        // Audio and entertainment control
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "audio".to_string(),
                "music".to_string(),
                "sound".to_string(),
                "speaker".to_string(),
                "zone".to_string(),
                "volume".to_string(),
                "lautstärke".to_string(),
                "media".to_string(),
            ],
            device_type_patterns: vec!["digital".to_string(), "analog".to_string()],
            sensor_type: SensorType::SoundLevel {
                unit: SoundUnit::Decibels,
            },
            confidence: 0.7,
        });

        // Smart home scenarios and automation
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "schlafen".to_string(),
                "essen".to_string(),
                "kochen".to_string(),
                "lesen".to_string(),
                "abwesend".to_string(),
                "besuch".to_string(),
                "tiefschlaf".to_string(),
                "scenario".to_string(),
                "scene".to_string(),
                "mood".to_string(),
                "stimmung".to_string(),
            ],
            device_type_patterns: vec![
                "digital".to_string(),
                "switch".to_string(),
                "pushbutton".to_string(),
            ],
            sensor_type: SensorType::MotionDetector, // Repurposed for presence/scenario detection
            confidence: 0.5,
        });

        // NFC and Touch controls
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "nfc".to_string(),
                "touch".to_string(),
                "code".to_string(),
                "card".to_string(),
                "rfid".to_string(),
                "key".to_string(),
                "schlüssel".to_string(),
            ],
            device_type_patterns: vec![
                "digital".to_string(),
                "nfc".to_string(),
                "touch".to_string(),
            ],
            sensor_type: SensorType::DoorWindowContact, // Repurposed for touch/access detection
            confidence: 0.7,
        });
    }

    /// Calculate confidence score for a detection rule
    fn calculate_rule_confidence(&self, rule: &SensorDetectionRule, device: &LoxoneDevice) -> f32 {
        let mut confidence: f32 = 0.0;
        let name_lower = device.name.to_lowercase();
        let type_lower = device.device_type.to_lowercase();

        // Check name patterns
        for pattern in &rule.name_patterns {
            if name_lower.contains(pattern) {
                confidence += 0.4;
                break;
            }
        }

        // Check device type patterns
        for pattern in &rule.device_type_patterns {
            if type_lower.contains(pattern) {
                confidence += 0.3;
                break;
            }
        }

        // Additional boost for exact matches
        if rule.name_patterns.contains(&name_lower) {
            confidence += 0.2;
        }

        confidence.min(rule.confidence)
    }

    /// Learn sensor type from user input or behavioral analysis
    pub async fn learn_sensor_type(
        &mut self,
        device_uuid: String,
        sensor_type: SensorType,
    ) -> Result<()> {
        self.learned_types.insert(device_uuid, sensor_type);
        // Could persist to file or database here
        Ok(())
    }

    /// Get all detected sensor types in the system
    pub async fn get_sensor_inventory(
        &self,
        devices: &HashMap<String, LoxoneDevice>,
    ) -> Result<SensorInventory> {
        let mut inventory = SensorInventory::new();

        for (uuid, device) in devices {
            if let Ok(Some(sensor_type)) = self.detect_sensor_type(device).await {
                inventory.add_sensor(uuid.clone(), device.clone(), sensor_type);
            } else {
                inventory.add_unknown_device(uuid.clone(), device.clone());
            }
        }

        Ok(inventory)
    }
}

impl Default for SensorTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive sensor inventory
#[derive(Debug, Serialize)]
pub struct SensorInventory {
    pub total_devices: usize,
    pub identified_sensors: usize,
    pub unknown_devices: usize,
    pub sensors_by_type: HashMap<String, Vec<IdentifiedSensor>>,
    pub sensors_by_room: HashMap<String, Vec<IdentifiedSensor>>,
    pub unknown_devices_list: Vec<UnknownDevice>,
    pub analysis_timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdentifiedSensor {
    pub uuid: String,
    pub name: String,
    pub sensor_type: SensorType,
    pub room: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Serialize)]
pub struct UnknownDevice {
    pub uuid: String,
    pub name: String,
    pub device_type: String,
    pub room: Option<String>,
    pub suggested_analysis: Vec<String>,
}

impl SensorInventory {
    pub fn new() -> Self {
        Self {
            total_devices: 0,
            identified_sensors: 0,
            unknown_devices: 0,
            sensors_by_type: HashMap::new(),
            sensors_by_room: HashMap::new(),
            unknown_devices_list: Vec::new(),
            analysis_timestamp: chrono::Utc::now(),
        }
    }

    pub fn add_sensor(&mut self, uuid: String, device: LoxoneDevice, sensor_type: SensorType) {
        let sensor = IdentifiedSensor {
            uuid: uuid.clone(),
            name: device.name.clone(),
            sensor_type: sensor_type.clone(),
            room: device.room.clone(),
            confidence: 0.9, // Would come from detection confidence
        };

        // Group by type
        let type_key = format!("{:?}", sensor_type)
            .split('{')
            .next()
            .unwrap_or("Unknown")
            .to_string();
        self.sensors_by_type
            .entry(type_key)
            .or_default()
            .push(sensor.clone());

        // Group by room
        if let Some(room) = &device.room {
            self.sensors_by_room
                .entry(room.clone())
                .or_default()
                .push(sensor);
        }

        self.identified_sensors += 1;
        self.total_devices += 1;
    }

    pub fn add_unknown_device(&mut self, uuid: String, device: LoxoneDevice) {
        self.unknown_devices_list.push(UnknownDevice {
            uuid,
            name: device.name.clone(),
            device_type: device.device_type.clone(),
            room: device.room.clone(),
            suggested_analysis: vec![
                "Run behavioral analysis".to_string(),
                "Check value patterns".to_string(),
                "Manual classification".to_string(),
            ],
        });

        self.unknown_devices += 1;
        self.total_devices += 1;
    }
}

impl Default for SensorInventory {
    fn default() -> Self {
        Self::new()
    }
}
