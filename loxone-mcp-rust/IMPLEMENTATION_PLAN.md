# Detailed Implementation Plan - Sensor Data Consolidation

## 🎯 Priority: Fix Dashboard Sensor Display Issues

Based on the architectural analysis, the immediate priority is consolidating the fragmented sensor data flows that are causing the dashboard to show "Off"/"Idle" instead of real sensor values.

## 📋 Phase 1: Unified Value Resolution Service (Weeks 1-2)

### **Task 1.1: Create Core Value Resolution Service**

**File: `src/services/value_resolution.rs`**

```rust
use crate::client::{LoxoneClient, ClientContext};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// Unified value resolution service - single source of truth for all device values
#[derive(Clone)]
pub struct UnifiedValueResolver {
    client: Arc<dyn LoxoneClient>,
    cache: Arc<ValueCache>,
    sensor_registry: Arc<SensorTypeRegistry>,
    parsers: Arc<ValueParserRegistry>,
}

/// Resolved device value with comprehensive metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedValue {
    pub uuid: String,
    pub device_name: String,
    pub raw_value: serde_json::Value,
    pub numeric_value: Option<f64>,
    pub formatted_value: String,
    pub unit: Option<String>,
    pub sensor_type: Option<SensorType>,
    pub room: Option<String>,
    pub source: ValueSource,
    pub timestamp: DateTime<Utc>,
    pub confidence: f32, // 0.0-1.0 confidence in this value
    pub validation_status: ValidationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueSource {
    RealTimeApi,    // From /jdev/sps/io/{uuid}/state
    StructureCache, // From cached structure file
    StateUuid,      // From /jdev/sps/status/{state_uuid}
    Computed,       // Calculated/derived value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationStatus {
    Valid,
    OutOfRange { min: f64, max: f64, actual: f64 },
    Stale { age_seconds: u64 },
    ParseError { error: String },
    Unknown,
}

impl UnifiedValueResolver {
    /// Create new resolver
    pub fn new(
        client: Arc<dyn LoxoneClient>,
        sensor_registry: Arc<SensorTypeRegistry>,
    ) -> Self {
        Self {
            client,
            cache: Arc::new(ValueCache::new()),
            sensor_registry,
            parsers: Arc::new(ValueParserRegistry::new()),
        }
    }

    /// Resolve value for a single device
    pub async fn resolve_device_value(&self, uuid: &str) -> Result<ResolvedValue> {
        // Try cache first
        if let Some(cached) = self.cache.get(uuid).await {
            if !cached.is_stale() {
                return Ok(cached);
            }
        }

        // Get device info from context
        let context = self.client.get_context().await?;
        let devices = context.devices.read().await;
        let device = devices.get(uuid)
            .ok_or_else(|| crate::error::LoxoneError::device_not_found(uuid))?;

        // Fetch real-time state
        let device_states = self.client.get_device_states(&[uuid.to_string()]).await?;
        let raw_state = device_states.get(uuid);

        // Resolve the value using multiple strategies
        let resolved = self.resolve_value_with_strategies(device, raw_state).await?;

        // Cache the result
        self.cache.set(uuid.to_string(), resolved.clone()).await;

        Ok(resolved)
    }

    /// Resolve values for multiple devices efficiently
    pub async fn resolve_batch_values(&self, uuids: &[String]) -> Result<HashMap<String, ResolvedValue>> {
        // Split into cached and uncached
        let mut results = HashMap::new();
        let mut uncached_uuids = Vec::new();

        for uuid in uuids {
            if let Some(cached) = self.cache.get(uuid).await {
                if !cached.is_stale() {
                    results.insert(uuid.clone(), cached);
                    continue;
                }
            }
            uncached_uuids.push(uuid.clone());
        }

        if uncached_uuids.is_empty() {
            return Ok(results);
        }

        // Batch fetch uncached values
        let device_states = self.client.get_device_states(&uncached_uuids).await?;
        let context = self.client.get_context().await?;
        let devices = context.devices.read().await;

        // Resolve each uncached value
        for uuid in uncached_uuids {
            if let Some(device) = devices.get(&uuid) {
                let raw_state = device_states.get(&uuid);
                match self.resolve_value_with_strategies(device, raw_state).await {
                    Ok(resolved) => {
                        self.cache.set(uuid.clone(), resolved.clone()).await;
                        results.insert(uuid, resolved);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to resolve value for {}: {}", uuid, e);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Internal: Resolve value using multiple strategies with fallback
    async fn resolve_value_with_strategies(
        &self,
        device: &crate::client::LoxoneDevice,
        raw_state: Option<&serde_json::Value>,
    ) -> Result<ResolvedValue> {
        let sensor_type = self.sensor_registry.detect_sensor_type(device).await?;
        
        // Strategy 1: Use appropriate parser based on sensor type
        if let Some(sensor_type) = &sensor_type {
            if let Some(parser) = self.parsers.get_parser(sensor_type) {
                if let Some(raw_state) = raw_state {
                    if let Ok(parsed) = parser.parse(raw_state) {
                        return Ok(ResolvedValue {
                            uuid: device.uuid.clone(),
                            device_name: device.name.clone(),
                            raw_value: raw_state.clone(),
                            numeric_value: parsed.numeric_value,
                            formatted_value: parsed.formatted_value,
                            unit: parsed.unit,
                            sensor_type: Some(sensor_type.clone()),
                            room: device.room.clone(),
                            source: ValueSource::RealTimeApi,
                            timestamp: Utc::now(),
                            confidence: parser.confidence(raw_state),
                            validation_status: self.validate_value(&parsed, &sensor_type),
                        });
                    }
                }
            }
        }

        // Strategy 2: Generic value extraction (current dashboard logic)
        if let Some(raw_state) = raw_state {
            if let Ok(parsed) = self.generic_value_extraction(raw_state) {
                return Ok(ResolvedValue {
                    uuid: device.uuid.clone(),
                    device_name: device.name.clone(),
                    raw_value: raw_state.clone(),
                    numeric_value: parsed.numeric_value,
                    formatted_value: parsed.formatted_value,
                    unit: parsed.unit,
                    sensor_type,
                    room: device.room.clone(),
                    source: ValueSource::RealTimeApi,
                    timestamp: Utc::now(),
                    confidence: 0.5, // Lower confidence for generic parsing
                    validation_status: ValidationStatus::Unknown,
                });
            }
        }

        // Strategy 3: Fallback to cached states
        if let Some(cached_value) = device.states.get("active") {
            if let Some(numeric) = cached_value.as_f64() {
                return Ok(ResolvedValue {
                    uuid: device.uuid.clone(),
                    device_name: device.name.clone(),
                    raw_value: cached_value.clone(),
                    numeric_value: Some(numeric),
                    formatted_value: numeric.to_string(),
                    unit: None,
                    sensor_type,
                    room: device.room.clone(),
                    source: ValueSource::StructureCache,
                    timestamp: Utc::now(),
                    confidence: 0.3, // Low confidence for cached data
                    validation_status: ValidationStatus::Stale { age_seconds: 3600 }, // Assume 1h old
                });
            }
        }

        // Strategy 4: Default unknown value
        Ok(ResolvedValue {
            uuid: device.uuid.clone(),
            device_name: device.name.clone(),
            raw_value: serde_json::Value::Null,
            numeric_value: None,
            formatted_value: "Unknown".to_string(),
            unit: None,
            sensor_type,
            room: device.room.clone(),
            source: ValueSource::StructureCache,
            timestamp: Utc::now(),
            confidence: 0.0,
            validation_status: ValidationStatus::Unknown,
        })
    }

    /// Generic value extraction (consolidation of current dashboard logic)
    fn generic_value_extraction(&self, raw_state: &serde_json::Value) -> Result<ParsedValue> {
        // Strategy 1: LL.value extraction (most reliable for sensors)
        if let Some(ll_obj) = raw_state.get("LL").and_then(|v| v.as_object()) {
            if let Some(value_str) = ll_obj.get("value").and_then(|v| v.as_str()) {
                if let Some(numeric) = extract_numeric_value(value_str) {
                    return Ok(ParsedValue {
                        numeric_value: Some(numeric),
                        formatted_value: format!("{:.1}", numeric),
                        unit: extract_unit(value_str),
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        // Strategy 2: Direct numeric value
        if let Some(numeric) = raw_state.as_f64() {
            return Ok(ParsedValue {
                numeric_value: Some(numeric),
                formatted_value: format!("{:.1}", numeric),
                unit: None,
                metadata: HashMap::new(),
            });
        }

        // Strategy 3: Direct string parsing
        if let Some(value_str) = raw_state.as_str() {
            if let Some(numeric) = extract_numeric_value(value_str) {
                return Ok(ParsedValue {
                    numeric_value: Some(numeric),
                    formatted_value: value_str.to_string(),
                    unit: extract_unit(value_str),
                    metadata: HashMap::new(),
                });
            }
        }

        Err(crate::error::LoxoneError::value_parse_error("Unable to extract numeric value"))
    }

    /// Validate parsed value against sensor type constraints
    fn validate_value(&self, parsed: &ParsedValue, sensor_type: &SensorType) -> ValidationStatus {
        if let Some(numeric) = parsed.numeric_value {
            match sensor_type {
                SensorType::Temperature { range: (min, max), .. } => {
                    if numeric < *min || numeric > *max {
                        return ValidationStatus::OutOfRange { 
                            min: *min, max: *max, actual: numeric 
                        };
                    }
                }
                SensorType::Humidity { range: (min, max) } => {
                    if numeric < *min || numeric > *max {
                        return ValidationStatus::OutOfRange { 
                            min: *min, max: *max, actual: numeric 
                        };
                    }
                }
                // Add more sensor type validations
                _ => {}
            }
        }
        ValidationStatus::Valid
    }
}

/// Value cache with TTL
pub struct ValueCache {
    cache: RwLock<HashMap<String, (ResolvedValue, DateTime<Utc>)>>,
    ttl: chrono::Duration,
}

impl ValueCache {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            ttl: chrono::Duration::seconds(30), // 30 second TTL
        }
    }

    pub async fn get(&self, uuid: &str) -> Option<ResolvedValue> {
        let cache = self.cache.read().await;
        if let Some((value, timestamp)) = cache.get(uuid) {
            if Utc::now() - *timestamp < self.ttl {
                return Some(value.clone());
            }
        }
        None
    }

    pub async fn set(&self, uuid: String, value: ResolvedValue) {
        let mut cache = self.cache.write().await;
        cache.insert(uuid, (value, Utc::now()));
    }
}

impl ResolvedValue {
    pub fn is_stale(&self) -> bool {
        match &self.validation_status {
            ValidationStatus::Stale { .. } => true,
            _ => false,
        }
    }
}

/// Helper functions (consolidation of current logic)
fn extract_numeric_value(value_str: &str) -> Option<f64> {
    let cleaned = value_str
        .replace("°", "")
        .replace("%", "")
        .replace("Lx", "")
        .replace("W", "")
        .replace("A", "")
        .replace("V", "")
        .trim()
        .to_string();
    
    cleaned.parse::<f64>().ok()
}

fn extract_unit(value_str: &str) -> Option<String> {
    if value_str.contains("°") { return Some("°C".to_string()); }
    if value_str.contains("%") { return Some("%".to_string()); }
    if value_str.contains("Lx") { return Some("Lx".to_string()); }
    if value_str.contains("W") { return Some("W".to_string()); }
    None
}

#[derive(Debug, Clone)]
pub struct ParsedValue {
    pub numeric_value: Option<f64>,
    pub formatted_value: String,
    pub unit: Option<String>,
    pub metadata: HashMap<String, String>,
}
```

### **Task 1.2: Sensor Type Registry**

**File: `src/services/sensor_registry.rs`**

```rust
use crate::client::LoxoneDevice;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Comprehensive sensor type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SensorType {
    // Environmental sensors
    Temperature { 
        unit: TemperatureUnit, 
        range: (f64, f64) // min, max expected values
    },
    Humidity { 
        range: (f64, f64) // 0-100% typically
    },
    AirPressure { 
        unit: PressureUnit,
        range: (f64, f64)
    },
    
    // Light sensors
    Illuminance { 
        unit: LightUnit,
        range: (f64, f64) // 0-100000 Lx typically
    },
    
    // Motion and presence
    MotionDetector,
    PresenceSensor,
    
    // Contact and position
    DoorWindowContact,
    WindowPosition { range: (f64, f64) }, // 0-100%
    BlindPosition { range: (f64, f64) },  // 0-100%
    
    // Energy monitoring
    PowerMeter { unit: PowerUnit },
    EnergyConsumption { unit: EnergyUnit },
    Current { unit: CurrentUnit },
    Voltage { unit: VoltageUnit },
    
    // Weather
    WindSpeed { unit: SpeedUnit },
    Rainfall { unit: VolumeUnit },
    
    // Sound
    SoundLevel { unit: SoundUnit },
    
    // Unknown with learning metadata
    Unknown { 
        device_type: String,
        detected_patterns: Vec<String>,
        sample_values: Vec<String>,
        confidence_score: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TemperatureUnit { Celsius, Fahrenheit, Kelvin }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LightUnit { Lux, FootCandles }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PowerUnit { Watts, Kilowatts }

// ... other unit enums

/// Registry for sensor type detection and management
pub struct SensorTypeRegistry {
    type_mappings: HashMap<String, SensorType>,
    detection_rules: Vec<SensorDetectionRule>,
    learned_types: HashMap<String, SensorType>,
}

pub struct SensorDetectionRule {
    pub name_patterns: Vec<String>,
    pub device_type_patterns: Vec<String>,
    pub sensor_type: SensorType,
    pub confidence: f32,
}

impl SensorTypeRegistry {
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
            if confidence > 0.5 { // Minimum confidence threshold
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
            ],
            device_type_patterns: vec!["analog".to_string()],
            sensor_type: SensorType::Temperature { 
                unit: TemperatureUnit::Celsius, 
                range: (-40.0, 85.0) 
            },
            confidence: 0.9,
        });

        // Humidity sensors  
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "luftfeuchte".to_string(),
                "humidity".to_string(),
                "feuchte".to_string(),
            ],
            device_type_patterns: vec!["analog".to_string()],
            sensor_type: SensorType::Humidity { range: (0.0, 100.0) },
            confidence: 0.9,
        });

        // Light sensors
        self.detection_rules.push(SensorDetectionRule {
            name_patterns: vec![
                "helligkeit".to_string(),
                "light".to_string(),
                "brightness".to_string(),
                "lux".to_string(),
            ],
            device_type_patterns: vec!["analog".to_string()],
            sensor_type: SensorType::Illuminance { 
                unit: LightUnit::Lux, 
                range: (0.0, 100000.0) 
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
            ],
            device_type_patterns: vec!["digital".to_string()],
            sensor_type: SensorType::MotionDetector,
            confidence: 0.8,
        });

        // Add more rules for other sensor types...
    }

    /// Calculate confidence score for a detection rule
    fn calculate_rule_confidence(&self, rule: &SensorDetectionRule, device: &LoxoneDevice) -> f32 {
        let mut confidence = 0.0;
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

        // Additional heuristics could be added here
        
        confidence.min(rule.confidence)
    }

    /// Learn sensor type from user input or behavioral analysis
    pub async fn learn_sensor_type(&mut self, device_uuid: String, sensor_type: SensorType) -> Result<()> {
        self.learned_types.insert(device_uuid, sensor_type);
        // Could persist to file or database here
        Ok(())
    }

    /// Get all detected sensor types in the system
    pub async fn get_sensor_inventory(&self, devices: &HashMap<String, LoxoneDevice>) -> Result<SensorInventory> {
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

#[derive(Debug, Serialize)]
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
        let type_key = format!("{:?}", sensor_type).split('{').next().unwrap_or("Unknown").to_string();
        self.sensors_by_type.entry(type_key).or_insert_with(Vec::new).push(sensor.clone());

        // Group by room
        if let Some(room) = &device.room {
            self.sensors_by_room.entry(room.clone()).or_insert_with(Vec::new).push(sensor);
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
```

### **Task 1.3: Value Parser Registry**

**File: `src/services/value_parsers.rs`**

```rust
use crate::services::sensor_registry::SensorType;
use crate::error::Result;
use serde_json::Value;
use std::collections::HashMap;

/// Registry for value parsers
pub struct ValueParserRegistry {
    parsers: HashMap<String, Box<dyn ValueParser>>,
}

/// Trait for parsing sensor values
pub trait ValueParser: Send + Sync {
    fn parse(&self, raw_value: &Value) -> Result<ParsedValue>;
    fn confidence(&self, raw_value: &Value) -> f32;
}

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
        self.parsers.insert("Temperature".to_string(), Box::new(TemperatureParser));
        self.parsers.insert("Humidity".to_string(), Box::new(HumidityParser));
        self.parsers.insert("Illuminance".to_string(), Box::new(LightParser));
        self.parsers.insert("PowerMeter".to_string(), Box::new(PowerParser));
        self.parsers.insert("MotionDetector".to_string(), Box::new(MotionParser));
        self.parsers.insert("DoorWindowContact".to_string(), Box::new(ContactParser));
    }

    fn sensor_type_to_key(&self, sensor_type: &SensorType) -> String {
        format!("{:?}", sensor_type).split('{').next().unwrap_or("Unknown").to_string()
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

        Err(crate::error::LoxoneError::value_parse_error("Unable to parse temperature"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() { 0.9 }
        else if raw_value.as_f64().is_some() { 0.7 }
        else { 0.0 }
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

        Err(crate::error::LoxoneError::value_parse_error("Unable to parse humidity"))
    }

    fn confidence(&self, raw_value: &Value) -> f32 {
        if raw_value.get("LL").and_then(|v| v.get("value")).is_some() { 0.9 }
        else if raw_value.as_f64().is_some() { 0.7 }
        else { 0.0 }
    }
}

// Additional parsers for other sensor types...

/// Helper functions for value extraction
fn extract_temperature(value_str: &str) -> Option<f64> {
    value_str.replace("°", "").replace("C", "").trim().parse().ok()
}

fn extract_percentage(value_str: &str) -> Option<f64> {
    value_str.replace("%", "").trim().parse().ok()
}
```

### **Task 1.4: Integration with Dashboard**

**File: `src/http_transport/dashboard_data_unified.rs`**

```rust
//! Unified dashboard data using the new value resolution service

use crate::server::LoxoneMcpServer;
use crate::services::value_resolution::UnifiedValueResolver;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Get dashboard data using unified value resolution
pub async fn get_unified_dashboard_data(server: &LoxoneMcpServer) -> Value {
    let resolver = server.get_value_resolver(); // New method to get resolver
    let context = &server.context;
    
    // Get all devices
    let devices = context.devices.read().await;
    let rooms = context.rooms.read().await;
    
    // Get all device UUIDs
    let all_device_uuids: Vec<String> = devices.keys().cloned().collect();
    
    // Resolve all values efficiently in batch
    let resolved_values = match resolver.resolve_batch_values(&all_device_uuids).await {
        Ok(values) => values,
        Err(e) => {
            tracing::error!("Failed to resolve device values: {}", e);
            HashMap::new()
        }
    };
    
    tracing::info!("Resolved values for {} devices", resolved_values.len());
    
    // Build rooms data with real-time sensor integration
    let mut rooms_data = Vec::new();
    for (room_uuid, room) in rooms.iter() {
        let room_devices: Vec<_> = devices.values()
            .filter(|device| device.room.as_ref() == Some(&room.name))
            .collect();
        
        // Get sensor readings for this room
        let mut room_temp: Option<f64> = None;
        let mut room_humidity: Option<f64> = None;
        let mut active_sensors = 0;
        
        for device in &room_devices {
            if let Some(resolved) = resolved_values.get(&device.uuid) {
                match &resolved.sensor_type {
                    Some(crate::services::sensor_registry::SensorType::Temperature { .. }) => {
                        if resolved.numeric_value.is_some() {
                            room_temp = resolved.numeric_value;
                            active_sensors += 1;
                        }
                    }
                    Some(crate::services::sensor_registry::SensorType::Humidity { .. }) => {
                        if resolved.numeric_value.is_some() {
                            room_humidity = resolved.numeric_value;
                            active_sensors += 1;
                        }
                    }
                    Some(_) => {
                        if resolved.numeric_value.is_some() {
                            active_sensors += 1;
                        }
                    }
                    None => {}
                }
            }
        }
        
        rooms_data.push(json!({
            "name": room.name,
            "uuid": room_uuid,
            "device_count": room_devices.len(),
            "active_sensors": active_sensors,
            "current_temp": room_temp,
            "current_humidity": room_humidity,
        }));
    }
    
    // Build devices data by category with resolved values
    let mut lights_data = Vec::new();
    let mut blinds_data = Vec::new();
    let mut climate_data = Vec::new();
    let mut other_data = Vec::new();
    
    for device in devices.values() {
        let resolved = resolved_values.get(&device.uuid);
        
        let device_json = build_device_json(device, resolved);
        
        match device.category.as_str() {
            "lights" => lights_data.push(device_json),
            "shading" => blinds_data.push(device_json),
            "climate" => climate_data.push(device_json),
            _ => {
                // Check if it's a sensor based on resolved type
                if let Some(resolved) = resolved {
                    if resolved.sensor_type.is_some() {
                        climate_data.push(device_json);
                    } else {
                        other_data.push(device_json);
                    }
                } else {
                    other_data.push(device_json);
                }
            }
        }
    }
    
    // Build device matrix for dashboard
    let mut device_matrix = Vec::new();
    for room in &rooms_data {
        if let Some(room_name) = room.get("name").and_then(|n| n.as_str()) {
            let mut all_room_devices = Vec::new();
            all_room_devices.extend(lights_data.iter().filter(|d| 
                d.get("room").and_then(|r| r.as_str()) == Some(room_name)
            ).cloned());
            all_room_devices.extend(blinds_data.iter().filter(|d| 
                d.get("room").and_then(|r| r.as_str()) == Some(room_name)
            ).cloned());
            all_room_devices.extend(climate_data.iter().filter(|d| 
                d.get("room").and_then(|r| r.as_str()) == Some(room_name)
            ).cloned());
            all_room_devices.extend(other_data.iter().filter(|d| 
                d.get("room").and_then(|r| r.as_str()) == Some(room_name)
            ).cloned());
            
            if !all_room_devices.is_empty() {
                device_matrix.push(json!({
                    "room_name": room_name,
                    "devices": all_room_devices
                }));
            }
        }
    }
    
    // Build final response
    json!({
        "realtime": {
            "system_health": {
                "connection_status": if *context.connected.read().await { "Connected" } else { "Disconnected" },
                "last_update": chrono::Utc::now().to_rfc3339(),
                "resolved_values": resolved_values.len(),
                "sensors_active": count_active_sensors(&resolved_values),
            },
            "data_source": "unified_value_resolver",
            "cache_status": "real_time"
        },
        "devices": {
            "device_matrix": device_matrix,
            "rooms": rooms_data,
            "lights": lights_data,
            "blinds": blinds_data,
            "climate": climate_data,
            "other": other_data,
            "summary": {
                "total_devices": devices.len(),
                "resolved_values": resolved_values.len(),
                "active_sensors": count_active_sensors(&resolved_values),
                "rooms": rooms.len()
            }
        },
        "metadata": {
            "last_update": chrono::Utc::now().to_rfc3339(),
            "data_age_seconds": 0,
            "cache_status": "live",
            "version": "2.0.0-unified"
        }
    })
}

/// Build device JSON with resolved value
fn build_device_json(
    device: &crate::client::LoxoneDevice, 
    resolved: Option<&crate::services::value_resolution::ResolvedValue>
) -> Value {
    let (status, status_color, state_display, numeric_value) = match resolved {
        Some(resolved) => {
            match &resolved.sensor_type {
                Some(_) => {
                    // This is a sensor - show the actual reading
                    if let Some(numeric) = resolved.numeric_value {
                        ("Active".to_string(), "green".to_string(), resolved.formatted_value.clone(), numeric)
                    } else {
                        ("Unknown".to_string(), "gray".to_string(), "No Data".to_string(), 0.0)
                    }
                }
                None => {
                    // Regular device - use device-specific logic
                    match device.category.as_str() {
                        "lights" => {
                            if let Some(numeric) = resolved.numeric_value {
                                if numeric > 0.0 {
                                    let brightness = (numeric * 100.0).round() as i32;
                                    ("On".to_string(), "green".to_string(), format!("On ({}%)", brightness), numeric)
                                } else {
                                    ("Off".to_string(), "gray".to_string(), "Off".to_string(), 0.0)
                                }
                            } else {
                                ("Unknown".to_string(), "gray".to_string(), "Unknown".to_string(), 0.0)
                            }
                        }
                        "shading" => {
                            if let Some(numeric) = resolved.numeric_value {
                                let position = (numeric * 100.0).round() as i32;
                                if position > 0 {
                                    ("Closed".to_string(), "blue".to_string(), format!("{}%", position), numeric)
                                } else {
                                    ("Open".to_string(), "gray".to_string(), "Open".to_string(), 0.0)
                                }
                            } else {
                                ("Unknown".to_string(), "gray".to_string(), "Unknown".to_string(), 0.0)
                            }
                        }
                        _ => {
                            if let Some(numeric) = resolved.numeric_value {
                                ("Active".to_string(), "green".to_string(), resolved.formatted_value.clone(), numeric)
                            } else {
                                ("Idle".to_string(), "gray".to_string(), "Idle".to_string(), 0.0)
                            }
                        }
                    }
                }
            }
        }
        None => {
            ("Unknown".to_string(), "red".to_string(), "No Data".to_string(), 0.0)
        }
    };
    
    json!({
        "uuid": device.uuid,
        "name": device.name,
        "device_type": device.device_type,
        "sensor_type": resolved.and_then(|r| r.sensor_type.as_ref()).map(|t| format!("{:?}", t)),
        "room": device.room,
        "status": status,
        "status_color": status_color,
        "state_display": state_display,
        "confidence": resolved.map(|r| r.confidence).unwrap_or(0.0),
        "validation_status": resolved.map(|r| format!("{:?}", r.validation_status)).unwrap_or("Unknown".to_string()),
        "source": resolved.map(|r| format!("{:?}", r.source)).unwrap_or("Unknown".to_string()),
        "states": {
            "active": if numeric_value > 0.0 { numeric_value } else { 0.0 },
            "value": numeric_value
        },
        "resolved_value": resolved.map(|r| json!({
            "numeric": r.numeric_value,
            "formatted": r.formatted_value,
            "unit": r.unit,
            "timestamp": r.timestamp,
        })),
        "cached_states": device.states,
    })
}

/// Count active sensors from resolved values
fn count_active_sensors(resolved_values: &HashMap<String, crate::services::value_resolution::ResolvedValue>) -> usize {
    resolved_values.values()
        .filter(|v| v.sensor_type.is_some() && v.numeric_value.is_some())
        .count()
}
```

## 📋 Implementation Checklist for Week 1-2

### **Week 1:**
- [ ] Create `src/services/` directory structure
- [ ] Implement `UnifiedValueResolver` with basic functionality
- [ ] **Ensure incremental compilation succeeds - no build errors**
- [ ] Implement `SensorTypeRegistry` with detection rules for:
  - [ ] Temperature sensors  
  - [ ] Humidity sensors
  - [ ] Light sensors
  - [ ] Motion sensors
  - [ ] Contact sensors
- [ ] Create basic value parsers for each sensor type
- [ ] **Fix any clippy warnings introduced by new code**
- [ ] Add comprehensive tests for value resolution
- [ ] **Verify all existing tests still pass**

### **Week 2:**  
- [ ] Integrate `UnifiedValueResolver` into `LoxoneMcpServer`
- [ ] Create `get_unified_dashboard_data()` function
- [ ] Replace complex dashboard fallback logic with unified resolver
- [ ] **Ensure no new build warnings are introduced**
- [ ] Add behavioral analysis for unknown devices
- [ ] Performance testing and optimization
- [ ] Create sensor discovery tool for real environment testing
- [ ] **Run clippy and fix any lints in new code**

## 🔨 Code Quality Requirements

### **Throughout Implementation (All Phases):**
- ✅ **No Build Errors:** Every commit must compile successfully
- ✅ **Incremental Progress:** Each feature added must not break existing functionality
- ✅ **Test Coverage:** New code must have tests that pass
- ✅ **Clippy Compliance:** Fix clippy warnings as you go in new code

### **Week 8: Major Cleanup Phase**
- [ ] **Eliminate 80%+ of existing build warnings**
- [ ] **Fix majority of clippy warnings across codebase**
- [ ] **Ensure all test suites are passing**
- [ ] **Document any remaining technical debt**

### **Week 9-10: Final Polish**
- [ ] **100% elimination of all build errors**
- [ ] **100% elimination of all build warnings**
- [ ] **100% elimination of all clippy errors**
- [ ] **100% elimination of all clippy warnings**
- [ ] **All tests passing with no failures**
- [ ] **Run `cargo fmt` on entire codebase**
- [ ] **Final code review and documentation**

### **Testing Strategy:**
1. **Unit Tests:** Test each parser and detection rule individually
2. **Integration Tests:** Test with real Loxone structure data
3. **Performance Tests:** Measure improvement in dashboard load times
4. **Real Environment:** Test with actual sensor data to validate all types
5. **Regression Tests:** Ensure existing functionality remains intact

### **Success Metrics:**
- Dashboard shows real sensor values instead of "Off"/"Idle"
- 80% reduction in dashboard data fetching complexity
- All temperature, humidity, light sensors properly detected
- Unknown sensors identified for manual classification
- Sub-second dashboard load times
- **Zero build errors/warnings**
- **Zero clippy errors/warnings**
- **100% test suite passing**
- **Consistent code formatting**

This plan provides a solid foundation for consolidating the fragmented sensor data architecture while ensuring comprehensive sensor type coverage.