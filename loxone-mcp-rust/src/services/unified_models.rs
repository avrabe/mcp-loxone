//! Unified data models for device values across all layers
//!
//! This module provides consistent data structures for representing
//! device values throughout the system, eliminating inconsistencies
//! between dashboard, tools, resources, and state management layers.

use crate::services::sensor_registry::SensorType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unified device value representation for the entire system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedDeviceValue {
    /// Device UUID
    pub device_uuid: String,
    /// Human-readable device name
    pub device_name: String,
    /// Device type from Loxone structure
    pub device_type: String,
    /// Room where device is located
    pub room: Option<String>,
    /// Parsed value information
    pub value: UnifiedValue,
    /// Data quality assessment
    pub quality: DataQuality,
    /// When this value was last updated
    pub last_updated: DateTime<Utc>,
    /// Source of this data
    pub data_source: DataSource,
    /// Additional metadata for debugging and analytics
    pub metadata: ValueMetadata,
}

/// Unified value representation with type safety
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedValue {
    /// Numeric value (if applicable)
    pub numeric: Option<f64>,
    /// Text representation for display
    pub display_text: String,
    /// Unit of measurement
    pub unit: Option<String>,
    /// Sensor type classification
    pub sensor_type: Option<SensorType>,
    /// Boolean interpretation (for switches, contacts, etc.)
    pub boolean: Option<bool>,
    /// Parsed semantic meaning
    pub semantic: SemanticValue,
    /// Confidence in parsing accuracy (0.0-1.0)
    pub confidence: f32,
}

/// Semantic interpretation of device values
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SemanticValue {
    /// Temperature measurement
    Temperature {
        celsius: f64,
        fahrenheit: Option<f64>,
    },
    /// Humidity percentage
    Humidity { percentage: f64 },
    /// Light level
    Illuminance { lux: f64, percentage: Option<f64> },
    /// Binary state (on/off, open/closed)
    BinaryState {
        is_active: bool,
        active_text: String,
        inactive_text: String,
    },
    /// Position/percentage value
    Position {
        percentage: f64,
        min_value: f64,
        max_value: f64,
    },
    /// Power measurement
    Power { watts: f64, kilowatts: Option<f64> },
    /// Energy consumption
    Energy { kwh: f64, wh: Option<f64> },
    /// Motion detection
    Motion {
        detected: bool,
        last_motion: Option<DateTime<Utc>>,
    },
    /// Contact sensor state
    Contact { is_closed: bool, is_open: bool },
    /// Weather data
    Weather {
        value: f64,
        weather_type: WeatherType,
    },
    /// Raw/unknown value
    Raw {
        raw_value: String,
        parsed_numeric: Option<f64>,
    },
}

/// Weather measurement types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WeatherType {
    WindSpeed,
    Rainfall,
    AirPressure,
    AirQuality,
    UVIndex,
}

/// Data quality assessment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataQuality {
    /// Fresh, high-confidence data
    Excellent { last_update_age: chrono::Duration },
    /// Good quality, recent data
    Good {
        last_update_age: chrono::Duration,
        minor_issues: Vec<String>,
    },
    /// Acceptable but with some concerns
    Fair {
        last_update_age: chrono::Duration,
        concerns: Vec<String>,
    },
    /// Stale or low-confidence data
    Poor {
        last_update_age: chrono::Duration,
        issues: Vec<String>,
    },
    /// Failed to parse or obtain data
    Failed {
        error_message: String,
        last_successful_update: Option<DateTime<Utc>>,
    },
}

/// Source of the data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataSource {
    /// Real-time API call
    RealTimeApi {
        endpoint: String,
        response_time_ms: u64,
    },
    /// Cached from previous call
    Cache {
        cached_at: DateTime<Utc>,
        cache_hit_type: CacheHitType,
    },
    /// Structure data (static)
    Structure { loaded_at: DateTime<Utc> },
    /// State manager (centralized)
    StateManager {
        managed_since: DateTime<Utc>,
        change_count: u64,
    },
    /// Fallback or default value
    Fallback {
        fallback_reason: String,
        original_source: Option<Box<DataSource>>,
    },
}

/// Cache hit classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CacheHitType {
    Fresh,      // Within optimal TTL
    Acceptable, // Beyond optimal but within max TTL
    Stale,      // Beyond max TTL but still usable
}

/// Additional metadata for debugging and analytics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValueMetadata {
    /// Processing time in milliseconds
    pub processing_time_ms: Option<u64>,
    /// Number of processing attempts
    pub processing_attempts: u32,
    /// Warning messages during processing
    pub warnings: Vec<String>,
    /// Debug information for troubleshooting
    pub debug_info: HashMap<String, String>,
    /// Performance metrics
    pub performance: PerformanceMetrics,
    /// Value change history (last few changes)
    pub recent_changes: Vec<ValueChange>,
}

/// Performance metrics for value processing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PerformanceMetrics {
    /// Time to fetch from source
    pub fetch_duration_ms: Option<u64>,
    /// Time to parse and process
    pub parse_duration_ms: Option<u64>,
    /// Cache lookup time
    pub cache_lookup_ms: Option<u64>,
    /// Total end-to-end processing time
    pub total_duration_ms: Option<u64>,
}

/// Value change record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValueChange {
    /// When the change occurred
    pub timestamp: DateTime<Utc>,
    /// Previous value
    pub old_value: Option<UnifiedValue>,
    /// New value
    pub new_value: UnifiedValue,
    /// Magnitude of change (for numeric values)
    pub change_magnitude: Option<f64>,
    /// Change type classification
    pub change_type: ChangeType,
}

/// Type of value change
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    /// Gradual change within normal parameters
    Gradual,
    /// Sudden significant change
    Sudden,
    /// State transition (on/off, open/closed)
    StateTransition,
    /// First time seeing this device
    Initial,
    /// Data quality change
    QualityChange,
    /// Error or parsing failure
    Error,
}

/// Batch of unified device values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedDeviceValueBatch {
    /// All device values in this batch
    pub values: HashMap<String, UnifiedDeviceValue>,
    /// When this batch was created
    pub batch_timestamp: DateTime<Utc>,
    /// Source that created this batch
    pub batch_source: BatchSource,
    /// Batch-level metadata
    pub metadata: BatchMetadata,
}

/// Source of a batch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchSource {
    /// Dashboard data request
    Dashboard,
    /// MCP tool execution
    Tool { tool_name: String },
    /// Resource subscription update
    ResourceUpdate,
    /// State manager sync
    StateSync,
    /// Manual refresh
    Manual,
}

/// Batch-level processing metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMetadata {
    /// Total processing time for the batch
    pub total_processing_ms: u64,
    /// Number of devices successfully processed
    pub successful_count: usize,
    /// Number of devices that failed processing
    pub failed_count: usize,
    /// Cache hit ratio for this batch
    pub cache_hit_ratio: f32,
    /// Warnings that apply to the entire batch
    pub batch_warnings: Vec<String>,
    /// Performance breakdown
    pub performance_breakdown: HashMap<String, u64>,
}

impl Default for ValueMetadata {
    fn default() -> Self {
        Self {
            processing_time_ms: None,
            processing_attempts: 1,
            warnings: Vec::new(),
            debug_info: HashMap::new(),
            performance: PerformanceMetrics::default(),
            recent_changes: Vec::new(),
        }
    }
}

impl UnifiedDeviceValue {
    /// Create a new unified device value with minimal required fields
    pub fn new(
        device_uuid: String,
        device_name: String,
        device_type: String,
        value: UnifiedValue,
    ) -> Self {
        Self {
            device_uuid,
            device_name,
            device_type,
            room: None,
            value,
            quality: DataQuality::Good {
                last_update_age: chrono::Duration::zero(),
                minor_issues: Vec::new(),
            },
            last_updated: Utc::now(),
            data_source: DataSource::RealTimeApi {
                endpoint: "unknown".to_string(),
                response_time_ms: 0,
            },
            metadata: ValueMetadata::default(),
        }
    }

    /// Check if this value is considered fresh
    pub fn is_fresh(&self) -> bool {
        matches!(
            &self.quality,
            DataQuality::Excellent { .. } | DataQuality::Good { .. }
        )
    }

    /// Get age of this value
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.last_updated
    }

    /// Get confidence score for this value
    pub fn confidence(&self) -> f32 {
        self.value.confidence
    }

    /// Check if this is a numeric value
    pub fn is_numeric(&self) -> bool {
        self.value.numeric.is_some()
    }

    /// Check if this is a boolean value
    pub fn is_boolean(&self) -> bool {
        self.value.boolean.is_some()
    }
}

impl UnifiedValue {
    /// Create a simple numeric value
    pub fn numeric(value: f64, unit: Option<String>) -> Self {
        Self {
            numeric: Some(value),
            display_text: if let Some(ref unit) = unit {
                format!("{value} {unit}")
            } else {
                value.to_string()
            },
            unit,
            sensor_type: None,
            boolean: None,
            semantic: SemanticValue::Raw {
                raw_value: value.to_string(),
                parsed_numeric: Some(value),
            },
            confidence: 0.8,
        }
    }

    /// Create a boolean value
    pub fn boolean(value: bool, active_text: String, inactive_text: String) -> Self {
        Self {
            numeric: Some(if value { 1.0 } else { 0.0 }),
            display_text: if value {
                active_text.clone()
            } else {
                inactive_text.clone()
            },
            unit: None,
            sensor_type: None,
            boolean: Some(value),
            semantic: SemanticValue::BinaryState {
                is_active: value,
                active_text,
                inactive_text,
            },
            confidence: 0.9,
        }
    }

    /// Create a temperature value
    pub fn temperature(celsius: f64) -> Self {
        Self {
            numeric: Some(celsius),
            display_text: format!("{celsius:.1}°C"),
            unit: Some("°C".to_string()),
            sensor_type: Some(SensorType::Temperature {
                unit: crate::services::sensor_registry::TemperatureUnit::Celsius,
                range: (-40.0, 85.0),
            }),
            boolean: None,
            semantic: SemanticValue::Temperature {
                celsius,
                fahrenheit: Some(celsius * 9.0 / 5.0 + 32.0),
            },
            confidence: 0.95,
        }
    }

    /// Create a humidity value
    pub fn humidity(percentage: f64) -> Self {
        Self {
            numeric: Some(percentage),
            display_text: format!("{percentage:.1}%"),
            unit: Some("%".to_string()),
            sensor_type: Some(SensorType::Humidity {
                range: (0.0, 100.0),
            }),
            boolean: None,
            semantic: SemanticValue::Humidity { percentage },
            confidence: 0.95,
        }
    }

    /// Create an unknown/raw value
    pub fn raw(raw_value: String) -> Self {
        let parsed_numeric = raw_value.parse::<f64>().ok();

        Self {
            numeric: parsed_numeric,
            display_text: raw_value.clone(),
            unit: None,
            sensor_type: None,
            boolean: None,
            semantic: SemanticValue::Raw {
                raw_value,
                parsed_numeric,
            },
            confidence: 0.3,
        }
    }
}

impl DataQuality {
    /// Assess data quality based on age and other factors
    pub fn assess(age: chrono::Duration, confidence: f32, issues: Vec<String>) -> Self {
        let age_score = if age < chrono::Duration::seconds(30) {
            1.0
        } else if age < chrono::Duration::minutes(5) {
            0.8
        } else if age < chrono::Duration::minutes(30) {
            0.5
        } else {
            0.2
        };

        let quality_score = (age_score + confidence) / 2.0;

        if quality_score > 0.8 && issues.is_empty() {
            DataQuality::Excellent {
                last_update_age: age,
            }
        } else if quality_score > 0.6 {
            DataQuality::Good {
                last_update_age: age,
                minor_issues: issues,
            }
        } else if quality_score > 0.4 {
            DataQuality::Fair {
                last_update_age: age,
                concerns: issues,
            }
        } else {
            DataQuality::Poor {
                last_update_age: age,
                issues,
            }
        }
    }
}
