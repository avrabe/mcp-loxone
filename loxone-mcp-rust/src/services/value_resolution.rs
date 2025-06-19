//! Unified value resolution service
//!
//! This module provides the single source of truth for all device values,
//! consolidating the fragmented data access patterns across the codebase.

use crate::client::LoxoneClient;
use crate::error::{LoxoneError, Result};
use crate::services::cache_manager::{CacheConfig, EnhancedCacheManager, PrefetchHandler};
use crate::services::sensor_registry::{SensorType, SensorTypeRegistry};
use crate::services::value_parsers::{ParsedValue, ValueParserRegistry};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Unified value resolution service - single source of truth for all device values
#[derive(Clone)]
pub struct UnifiedValueResolver {
    client: Arc<dyn LoxoneClient>,
    enhanced_cache: Arc<EnhancedCacheManager>,
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
    pub fn new(client: Arc<dyn LoxoneClient>, sensor_registry: Arc<SensorTypeRegistry>) -> Self {
        let cache_config = CacheConfig::default();
        
        // Create a prefetch handler that uses the client
        let prefetch_client = client.clone();
        let prefetch_handler = Arc::new(ValueResolverPrefetchHandler {
            client: prefetch_client,
        });
        
        Self {
            client,
            enhanced_cache: Arc::new(EnhancedCacheManager::with_prefetch_handler(
                cache_config,
                prefetch_handler,
            )),
            sensor_registry,
            parsers: Arc::new(ValueParserRegistry::new()),
        }
    }

    /// Create resolver with custom cache configuration
    pub fn with_cache_config(
        client: Arc<dyn LoxoneClient>,
        sensor_registry: Arc<SensorTypeRegistry>,
        cache_config: CacheConfig,
    ) -> Self {
        // Create a prefetch handler that uses the client
        let prefetch_client = client.clone();
        let prefetch_handler = Arc::new(ValueResolverPrefetchHandler {
            client: prefetch_client,
        });
        
        Self {
            client,
            enhanced_cache: Arc::new(EnhancedCacheManager::with_prefetch_handler(
                cache_config,
                prefetch_handler,
            )),
            sensor_registry,
            parsers: Arc::new(ValueParserRegistry::new()),
        }
    }

    /// Resolve value for a single device
    pub async fn resolve_device_value(&self, uuid: &str) -> Result<ResolvedValue> {
        // Use enhanced cache for single device resolution via batch method
        let results = self.resolve_batch_values(&[uuid.to_string()]).await?;
        
        results.into_iter()
            .next()
            .map(|(_, value)| value)
            .ok_or_else(|| LoxoneError::not_found(format!("Device not found: {}", uuid)))
    }

    /// Resolve values for multiple devices efficiently with enhanced caching
    pub async fn resolve_batch_values(
        &self,
        uuids: &[String],
    ) -> Result<HashMap<String, ResolvedValue>> {
        let client_clone = self.client.clone();

        // Use enhanced cache manager for intelligent batch fetching
        // Try to use batch API if available for better performance
        let device_states = if uuids.len() > 5 {
            // For many devices, try batch endpoint first
            self.enhanced_cache
                .get_batch_device_values(uuids, async move {
                    // Try batch endpoint first, fallback to individual
                    match client_clone.get_all_device_states_batch().await {
                        Ok(all_states) => {
                            // Filter to only requested UUIDs
                            let mut filtered = HashMap::new();
                            for uuid in uuids {
                                if let Some(state) = all_states.get(uuid) {
                                    filtered.insert(uuid.clone(), state.clone());
                                }
                            }
                            Ok(filtered)
                        }
                        Err(_) => {
                            // Fallback to individual requests
                            client_clone.get_device_states(uuids).await
                        }
                    }
                })
                .await?
        } else {
            // For few devices, use individual requests
            self.enhanced_cache
                .get_batch_device_values(uuids, async move {
                    client_clone.get_device_states(uuids).await
                })
                .await?
        };

        // Convert raw device states to resolved values
        let mut results = HashMap::new();

        // Get device info from context
        let context = self
            .client
            .as_any()
            .downcast_ref::<crate::client::LoxoneHttpClient>()
            .map(|c| c.context())
            .or_else(|| {
                #[cfg(feature = "crypto-openssl")]
                {
                    self.client
                        .as_any()
                        .downcast_ref::<crate::client::TokenHttpClient>()
                        .map(|c| c.context())
                }
                #[cfg(not(feature = "crypto-openssl"))]
                {
                    None
                }
            })
            .ok_or_else(|| LoxoneError::config("Unable to access client context"))?;

        let devices = context.devices.read().await;

        // Resolve each device value
        for uuid in uuids {
            if let Some(device) = devices.get(uuid) {
                let raw_state = device_states.get(uuid);
                match self.resolve_value_with_strategies(device, raw_state).await {
                    Ok(resolved) => {
                        // Note: Caching is handled automatically by enhanced_cache in get_batch_device_values
                        results.insert(uuid.clone(), resolved);
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
                        let validation_status = self.validate_value(&parsed, sensor_type);
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
                            validation_status,
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
        if let Some(cached_value) = device.states.get("active").or(device.states.get("value")) {
            if let Some(numeric) = cached_value.as_f64() {
                return Ok(ResolvedValue {
                    uuid: device.uuid.clone(),
                    device_name: device.name.clone(),
                    raw_value: cached_value.clone(),
                    numeric_value: Some(numeric),
                    formatted_value: format!("{:.1}", numeric),
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

    /// Enhanced value extraction with device-specific patterns
    fn generic_value_extraction(&self, raw_state: &serde_json::Value) -> Result<ParsedValue> {
        // Strategy 1: LL.value extraction (most reliable for all devices)
        if let Some(ll_obj) = raw_state.get("LL").and_then(|v| v.as_object()) {
            if let Some(value_str) = ll_obj.get("value").and_then(|v| v.as_str()) {
                // Enhanced value string parsing
                if let Some(result) = self.parse_loxone_value_string(value_str) {
                    return Ok(result);
                }
            }

            // Check for other LL fields
            if let Some(position) = ll_obj.get("position").and_then(|v| v.as_f64()) {
                return Ok(ParsedValue {
                    numeric_value: Some(position),
                    formatted_value: if position > 0.0 {
                        format!("{}%", (position * 100.0).round() as i32)
                    } else {
                        "Closed".to_string()
                    },
                    unit: Some("%".to_string()),
                    metadata: HashMap::new(),
                });
            }

            if let Some(active) = ll_obj.get("active").and_then(|v| v.as_f64()) {
                return Ok(ParsedValue {
                    numeric_value: Some(active),
                    formatted_value: if active > 0.0 {
                        "On".to_string()
                    } else {
                        "Off".to_string()
                    },
                    unit: None,
                    metadata: HashMap::new(),
                });
            }
        }

        // Strategy 2: Check for state-specific fields
        if let Some(state_obj) = raw_state.as_object() {
            // Look for common state fields
            for field_name in ["position", "value", "dimmer", "active", "switch", "level"] {
                if let Some(value) = state_obj.get(field_name) {
                    if let Some(result) = self.extract_from_state_field(field_name, value) {
                        return Ok(result);
                    }
                }
            }
        }

        // Strategy 3: Direct numeric value
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

        Err(LoxoneError::parsing_error(
            "Unable to extract numeric value",
        ))
    }

    /// Parse Loxone value strings with device-specific logic
    fn parse_loxone_value_string(&self, value_str: &str) -> Option<ParsedValue> {
        let value_lower = value_str.to_lowercase();

        // Handle common device state strings
        if value_lower.contains("closed") || value_lower == "0" {
            return Some(ParsedValue {
                numeric_value: Some(0.0),
                formatted_value: "Closed".to_string(),
                unit: None,
                metadata: HashMap::new(),
            });
        }

        if value_lower.contains("open") {
            return Some(ParsedValue {
                numeric_value: Some(1.0),
                formatted_value: "Open".to_string(),
                unit: None,
                metadata: HashMap::new(),
            });
        }

        if value_lower.contains("on") || value_lower.contains("active") {
            return Some(ParsedValue {
                numeric_value: Some(1.0),
                formatted_value: "On".to_string(),
                unit: None,
                metadata: HashMap::new(),
            });
        }

        if value_lower.contains("off") || value_lower.contains("inactive") {
            return Some(ParsedValue {
                numeric_value: Some(0.0),
                formatted_value: "Off".to_string(),
                unit: None,
                metadata: HashMap::new(),
            });
        }

        // Handle position/percentage values
        if let Some(numeric) = extract_numeric_value(value_str) {
            let unit = extract_unit(value_str);

            // Format based on the unit or value type
            let formatted = if unit.as_ref().is_some_and(|u| u.contains('%')) {
                format!("{}%", numeric.round() as i32)
            } else if unit.as_ref().is_some_and(|u| u.contains('°')) {
                format!("{:.1}°C", numeric)
            } else if (0.0..=1.0).contains(&numeric) && !value_str.contains('.') {
                // Likely a binary state encoded as 0/1
                if numeric > 0.0 {
                    "On".to_string()
                } else {
                    "Off".to_string()
                }
            } else {
                // Preserve original formatting for other values
                value_str.to_string()
            };

            return Some(ParsedValue {
                numeric_value: Some(numeric),
                formatted_value: formatted,
                unit,
                metadata: HashMap::new(),
            });
        }

        // For non-numeric strings, try to interpret meaningful states
        if !value_str.trim().is_empty() && value_str.to_lowercase() != "idle" {
            return Some(ParsedValue {
                numeric_value: None,
                formatted_value: value_str.to_string(),
                unit: None,
                metadata: HashMap::new(),
            });
        }

        None
    }

    /// Extract values from specific state fields
    fn extract_from_state_field(
        &self,
        field_name: &str,
        value: &serde_json::Value,
    ) -> Option<ParsedValue> {
        match field_name {
            "position" => {
                if let Some(pos) = value.as_f64() {
                    return Some(ParsedValue {
                        numeric_value: Some(pos),
                        formatted_value: if pos > 0.0 {
                            format!("{}%", (pos * 100.0).round() as i32)
                        } else {
                            "Closed".to_string()
                        },
                        unit: Some("%".to_string()),
                        metadata: HashMap::new(),
                    });
                }
            }
            "dimmer" | "level" => {
                if let Some(level) = value.as_f64() {
                    return Some(ParsedValue {
                        numeric_value: Some(level),
                        formatted_value: if level > 0.0 {
                            format!("{}%", (level * 100.0).round() as i32)
                        } else {
                            "Off".to_string()
                        },
                        unit: Some("%".to_string()),
                        metadata: HashMap::new(),
                    });
                }
            }
            "active" | "switch" => {
                if let Some(active) = value.as_f64() {
                    return Some(ParsedValue {
                        numeric_value: Some(active),
                        formatted_value: if active > 0.0 {
                            "On".to_string()
                        } else {
                            "Off".to_string()
                        },
                        unit: None,
                        metadata: HashMap::new(),
                    });
                }
            }
            "value" => {
                if let Some(val) = value.as_f64() {
                    return Some(ParsedValue {
                        numeric_value: Some(val),
                        formatted_value: format!("{:.1}", val),
                        unit: None,
                        metadata: HashMap::new(),
                    });
                }
                if let Some(val_str) = value.as_str() {
                    return self.parse_loxone_value_string(val_str);
                }
            }
            _ => {}
        }
        None
    }

    /// Validate parsed value against sensor type constraints
    fn validate_value(&self, parsed: &ParsedValue, sensor_type: &SensorType) -> ValidationStatus {
        if let Some(numeric) = parsed.numeric_value {
            match sensor_type {
                SensorType::Temperature {
                    range: (min, max), ..
                } => {
                    if numeric < *min || numeric > *max {
                        return ValidationStatus::OutOfRange {
                            min: *min,
                            max: *max,
                            actual: numeric,
                        };
                    }
                }
                SensorType::Humidity { range: (min, max) } => {
                    if numeric < *min || numeric > *max {
                        return ValidationStatus::OutOfRange {
                            min: *min,
                            max: *max,
                            actual: numeric,
                        };
                    }
                }
                SensorType::Illuminance {
                    range: (min, max), ..
                } => {
                    if numeric < *min || numeric > *max {
                        return ValidationStatus::OutOfRange {
                            min: *min,
                            max: *max,
                            actual: numeric,
                        };
                    }
                }
                SensorType::BlindPosition { range: (min, max) }
                | SensorType::WindowPosition { range: (min, max) } => {
                    if numeric < *min || numeric > *max {
                        return ValidationStatus::OutOfRange {
                            min: *min,
                            max: *max,
                            actual: numeric,
                        };
                    }
                }
                SensorType::AirPressure {
                    range: (min, max), ..
                } => {
                    if numeric < *min || numeric > *max {
                        return ValidationStatus::OutOfRange {
                            min: *min,
                            max: *max,
                            actual: numeric,
                        };
                    }
                }
                // Binary sensors
                SensorType::MotionDetector | SensorType::DoorWindowContact => {
                    if numeric != 0.0 && numeric != 1.0 {
                        return ValidationStatus::OutOfRange {
                            min: 0.0,
                            max: 1.0,
                            actual: numeric,
                        };
                    }
                }
                _ => {}
            }
        }
        ValidationStatus::Valid
    }

    /// Discover all sensor types in the system
    pub async fn discover_all_sensor_types(
        &self,
    ) -> Result<crate::services::sensor_registry::SensorInventory> {
        let context = self
            .client
            .as_any()
            .downcast_ref::<crate::client::LoxoneHttpClient>()
            .map(|c| c.context())
            .or_else(|| {
                #[cfg(feature = "crypto-openssl")]
                {
                    self.client
                        .as_any()
                        .downcast_ref::<crate::client::TokenHttpClient>()
                        .map(|c| c.context())
                }
                #[cfg(not(feature = "crypto-openssl"))]
                {
                    None
                }
            })
            .ok_or_else(|| LoxoneError::config("Unable to access client context"))?;

        let devices = context.devices.read().await;
        self.sensor_registry.get_sensor_inventory(&devices).await
    }

    /// Get cache statistics for monitoring
    pub async fn get_cache_statistics(&self) -> crate::services::cache_manager::CacheStatistics {
        self.enhanced_cache.get_statistics().await
    }

    /// Clear all caches (for maintenance or testing)
    pub async fn clear_caches(&self) {
        self.enhanced_cache.clear_all().await;
    }
}


impl ResolvedValue {
    pub fn is_stale(&self) -> bool {
        matches!(&self.validation_status, ValidationStatus::Stale { .. })
    }

    pub fn is_valid(&self) -> bool {
        matches!(&self.validation_status, ValidationStatus::Valid)
    }

    pub fn has_numeric_value(&self) -> bool {
        self.numeric_value.is_some()
    }
}

/// Helper functions (consolidation of current logic)
fn extract_numeric_value(value_str: &str) -> Option<f64> {
    let cleaned = value_str
        .replace(['°', '%', 'W', 'A', 'V'], "")
        .replace("Lx", "")
        .replace("hPa", "")
        .replace("ppm", "")
        .trim()
        .to_string();

    cleaned.parse::<f64>().ok()
}

fn extract_unit(value_str: &str) -> Option<String> {
    if value_str.contains('°') {
        return Some("°C".to_string());
    }
    if value_str.contains('%') {
        return Some("%".to_string());
    }
    if value_str.contains("Lx") {
        return Some("Lx".to_string());
    }
    if value_str.contains('W') && !value_str.contains("Wh") {
        return Some("W".to_string());
    }
    if value_str.contains("kWh") {
        return Some("kWh".to_string());
    }
    if value_str.contains('A') && !value_str.contains("AQI") {
        return Some("A".to_string());
    }
    if value_str.contains('V') {
        return Some("V".to_string());
    }
    if value_str.contains("hPa") {
        return Some("hPa".to_string());
    }
    if value_str.contains("ppm") {
        return Some("ppm".to_string());
    }
    None
}

/// Prefetch handler implementation for the UnifiedValueResolver
struct ValueResolverPrefetchHandler {
    client: Arc<dyn LoxoneClient>,
}

#[async_trait::async_trait]
impl PrefetchHandler for ValueResolverPrefetchHandler {
    async fn prefetch_devices(&self, device_uuids: Vec<String>) -> Result<HashMap<String, serde_json::Value>> {
        // Use the client's batch method to fetch device states
        self.client.get_device_states(&device_uuids).await
    }
}
