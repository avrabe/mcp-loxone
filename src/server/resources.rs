//! MCP Resources implementation for read-only data access
//!
//! This module implements the MCP Resources protocol for exposing Loxone system data
//! as structured resources that can be accessed via URI patterns. Resources represent
//! read-only data that can be efficiently cached and accessed by MCP clients.
//!
//! ## Resource URI Scheme
//!
//! Available resource URIs:
//! - `loxone://rooms` - All rooms list
//! - `loxone://devices/all` - All devices
//! - `loxone://devices/category/blinds` - All blinds/rolladen with current positions
//! - `loxone://devices/category/lighting` - All lighting devices with states
//! - `loxone://devices/category/climate` - All climate devices and sensors
//! - `loxone://system/status` - System status
//! - `loxone://system/capabilities` - System capabilities
//! - `loxone://system/categories` - Category overview
//! - `loxone://audio/zones` - Audio zones
//! - `loxone://audio/sources` - Audio sources
//! - `loxone://sensors/door-window` - Door/window sensors
//! - `loxone://sensors/temperature` - Temperature sensors
//! - `loxone://sensors/motion` - Motion sensors
//! - `loxone://sensors/air-quality` - Air quality sensors (CO2, VOC, humidity, PM)
//! - `loxone://sensors/presence` - Presence detectors with room occupancy analytics
//! - `loxone://sensors/weather-station` - Weather station sensors (wind, rain, pressure, solar)
//! - `loxone://sensors/discovered` - Dynamically discovered sensors
//! - `loxone://weather/current` - Current weather data
//! - `loxone://weather/outdoor-conditions` - Outdoor conditions with comfort assessment
//! - `loxone://weather/forecast-daily` - Daily weather forecast
//! - `loxone://weather/forecast-hourly` - Hourly weather forecast
//! - `loxone://security/status` - Security system status
//! - `loxone://security/zones` - Security zones
//! - `loxone://energy/consumption` - Energy consumption data
//! - `loxone://energy/meters` - Energy meters
//! - `loxone://energy/usage-history` - Historical energy usage
//!
//! Note: For room-specific or device-type-specific queries, use the appropriate tools instead.

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::debug;

/// MCP Resource representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneResource {
    /// URI that identifies the resource
    pub uri: String,
    /// Human-readable name
    pub name: String,
    /// Resource description
    pub description: String,
    /// Optional MIME type for the resource content
    pub mime_type: Option<String>,
}

/// Resource content with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    /// The actual resource data
    pub data: serde_json::Value,
    /// Content metadata
    pub metadata: ResourceMetadata,
}

/// Resource metadata for caching and validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetadata {
    /// Content-Type/MIME type
    pub content_type: String,
    /// Last modified timestamp
    pub last_modified: chrono::DateTime<chrono::Utc>,
    /// ETag for cache validation
    pub etag: String,
    /// Cache TTL in seconds
    pub cache_ttl: Option<u64>,
    /// Content size in bytes
    pub size: usize,
}

/// Simple cache entry for resource content
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Cached content
    content: ResourceContent,
    /// When the entry was created
    created_at: Instant,
    /// Time-to-live duration
    ttl: Duration,
    /// Access count for statistics
    access_count: u64,
}

impl CacheEntry {
    fn new(content: ResourceContent, ttl: Duration) -> Self {
        Self {
            content,
            created_at: Instant::now(),
            ttl,
            access_count: 0,
        }
    }

    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }

    fn access(&mut self) -> &ResourceContent {
        self.access_count += 1;
        &self.content
    }
}

/// URI parameter extraction for parameterized resources
#[derive(Debug, Clone)]
pub struct ResourceParams {
    /// Path parameters extracted from URI
    pub path_params: HashMap<String, String>,
    /// Query parameters
    pub query_params: HashMap<String, String>,
}

/// Resource access context
#[derive(Debug, Clone)]
pub struct ResourceContext {
    /// Original URI requested
    pub uri: String,
    /// Extracted parameters
    pub params: ResourceParams,
    /// Request timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Resource categories for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceCategory {
    /// Room-related resources
    Rooms,
    /// Device-related resources
    Devices,
    /// System information resources
    System,
    /// Audio/multimedia resources
    Audio,
    /// Sensor data resources
    Sensors,
    /// Weather data resources
    Weather,
    /// Security system resources
    Security,
    /// Energy consumption resources
    Energy,
    /// Climate control resources
    Climate,
}

impl ResourceCategory {
    /// Get the URI prefix for this category
    pub fn uri_prefix(&self) -> &'static str {
        match self {
            ResourceCategory::Rooms => "loxone://rooms",
            ResourceCategory::Devices => "loxone://devices",
            ResourceCategory::System => "loxone://system",
            ResourceCategory::Audio => "loxone://audio",
            ResourceCategory::Sensors => "loxone://sensors",
            ResourceCategory::Weather => "loxone://weather",
            ResourceCategory::Security => "loxone://security",
            ResourceCategory::Energy => "loxone://energy",
            ResourceCategory::Climate => "loxone://climate",
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            ResourceCategory::Rooms => "Rooms",
            ResourceCategory::Devices => "Devices",
            ResourceCategory::System => "System",
            ResourceCategory::Audio => "Audio",
            ResourceCategory::Sensors => "Sensors",
            ResourceCategory::Weather => "Weather",
            ResourceCategory::Security => "Security",
            ResourceCategory::Energy => "Energy",
            ResourceCategory::Climate => "Climate",
        }
    }
}

/// Resource manager for handling MCP resource operations
pub struct ResourceManager {
    /// Available resources registry
    resources: HashMap<String, LoxoneResource>,
    /// Category mappings
    categories: HashMap<ResourceCategory, Vec<String>>,
    /// Resource content cache
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Cache statistics
    cache_hits: Arc<RwLock<u64>>,
    cache_misses: Arc<RwLock<u64>>,
}

impl ResourceManager {
    /// Create new resource manager with default Loxone resources
    pub fn new() -> Self {
        let mut manager = Self {
            resources: HashMap::new(),
            categories: HashMap::new(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_hits: Arc::new(RwLock::new(0)),
            cache_misses: Arc::new(RwLock::new(0)),
        };

        manager.register_default_resources();
        manager
    }

    /// Register all default Loxone resources
    fn register_default_resources(&mut self) {
        // Room resources
        self.register_resource(
            LoxoneResource {
                uri: "loxone://rooms".to_string(),
                name: "All Rooms".to_string(),
                description: "List of all rooms with device counts and information".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Rooms,
        );

        // Room-specific device resources (template)
        self.register_resource(
            LoxoneResource {
                uri: "loxone://rooms/{roomName}/devices".to_string(),
                name: "Room Devices".to_string(),
                description: "All devices in a specific room".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Rooms,
        );

        // Device resources
        self.register_resource(
            LoxoneResource {
                uri: "loxone://devices/all".to_string(),
                name: "All Devices".to_string(),
                description: "Complete list of all devices in the system".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Devices,
        );

        // Device type resources - template for any device type
        self.register_resource(
            LoxoneResource {
                uri: "loxone://devices/type/{deviceType}".to_string(),
                name: "Type Devices".to_string(),
                description: "All devices of a specific type".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Devices,
        );

        // Device category resources - template for any category
        self.register_resource(
            LoxoneResource {
                uri: "loxone://devices/category/{category}".to_string(),
                name: "Category Devices".to_string(),
                description: "All devices in a specific category".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Devices,
        );

        // Note: Specific category resources removed in favor of the template above

        // Note: Additional device categories can be added as needed

        // System resources
        self.register_resource(
            LoxoneResource {
                uri: "loxone://system/status".to_string(),
                name: "System Status".to_string(),
                description: "Overall system status and health information".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::System,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://system/capabilities".to_string(),
                name: "System Capabilities".to_string(),
                description: "Available system capabilities and features".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::System,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://system/categories".to_string(),
                name: "Device Categories Overview".to_string(),
                description: "Overview of all device categories with counts and examples"
                    .to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::System,
        );

        // Audio resources
        self.register_resource(
            LoxoneResource {
                uri: "loxone://audio/zones".to_string(),
                name: "Audio Zones".to_string(),
                description: "All audio zones and their current status".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Audio,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://audio/sources".to_string(),
                name: "Audio Sources".to_string(),
                description: "Available audio sources and their status".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Audio,
        );

        // Sensor resources
        self.register_resource(
            LoxoneResource {
                uri: "loxone://sensors/door-window".to_string(),
                name: "Door/Window Sensors".to_string(),
                description: "All door and window sensors with current state".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Sensors,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://sensors/temperature".to_string(),
                name: "Temperature Sensors".to_string(),
                description: "All temperature sensors and their current readings".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Sensors,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://sensors/discovered".to_string(),
                name: "Discovered Sensors".to_string(),
                description: "Dynamically discovered sensors with metadata".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Sensors,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://sensors/motion".to_string(),
                name: "Motion Sensors".to_string(),
                description: "All motion and presence sensors with current state".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Sensors,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://sensors/air-quality".to_string(),
                name: "Air Quality Sensors".to_string(),
                description:
                    "All air quality sensors including CO2, VOC, humidity, and particulate matter"
                        .to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Sensors,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://sensors/presence".to_string(),
                name: "Presence Detectors".to_string(),
                description:
                    "All presence and occupancy detectors with room-level occupancy analytics"
                        .to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Sensors,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://sensors/weather-station".to_string(),
                name: "Weather Station Sensors".to_string(),
                description: "All weather station sensors including temperature, wind, rain, pressure, humidity, and solar radiation".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Sensors,
        );

        // Weather resources
        self.register_resource(
            LoxoneResource {
                uri: "loxone://weather/current".to_string(),
                name: "Current Weather".to_string(),
                description: "Current weather data from all weather sensors".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Weather,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://weather/outdoor-conditions".to_string(),
                name: "Outdoor Conditions".to_string(),
                description: "Outdoor environmental conditions with comfort assessment".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Weather,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://weather/forecast-daily".to_string(),
                name: "Daily Weather Forecast".to_string(),
                description: "Multi-day weather forecast data".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Weather,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://weather/forecast-hourly".to_string(),
                name: "Hourly Weather Forecast".to_string(),
                description: "Hourly weather forecast data".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Weather,
        );

        // Security resources
        self.register_resource(
            LoxoneResource {
                uri: "loxone://security/status".to_string(),
                name: "Security System Status".to_string(),
                description: "Current security system status and alarm states".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Security,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://security/zones".to_string(),
                name: "Security Zones".to_string(),
                description: "All security zones and their current states".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Security,
        );

        // Energy resources
        self.register_resource(
            LoxoneResource {
                uri: "loxone://energy/consumption".to_string(),
                name: "Energy Consumption".to_string(),
                description: "Current energy consumption and usage metrics".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Energy,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://energy/meters".to_string(),
                name: "Energy Meters".to_string(),
                description: "All energy meters and their current readings".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Energy,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://energy/usage-history".to_string(),
                name: "Energy Usage History".to_string(),
                description: "Historical energy usage data and trends".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Energy,
        );

        // Additional resources for tools that were converted from read-only tools

        // Room-specific resources
        // Note: These use templated URIs - handler must parse {room} parameter
        self.register_resource(
            LoxoneResource {
                uri: "loxone://rooms/{room}/devices".to_string(),
                name: "Room Devices".to_string(),
                description: "All devices in a specific room".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Rooms,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://rooms/{room}/overview".to_string(),
                name: "Room Overview".to_string(),
                description: "Complete overview of a room including devices and statistics"
                    .to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Rooms,
        );

        // Climate resources
        self.register_resource(
            LoxoneResource {
                uri: "loxone://climate/overview".to_string(),
                name: "Climate System Overview".to_string(),
                description: "Overview of the climate control system".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Climate,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://climate/rooms/{room}".to_string(),
                name: "Room Climate".to_string(),
                description: "Climate data for a specific room".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Climate,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://climate/sensors".to_string(),
                name: "Temperature Sensors".to_string(),
                description: "All temperature sensor readings".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Climate,
        );

        // Note: LLM-focused resources could be added here in future versions
    }

    /// Register a resource
    pub fn register_resource(&mut self, resource: LoxoneResource, category: ResourceCategory) {
        let uri = resource.uri.clone();
        self.resources.insert(uri.clone(), resource);

        self.categories.entry(category).or_default().push(uri);
    }

    /// List all available resources
    pub fn list_resources(&self) -> Vec<&LoxoneResource> {
        self.resources.values().collect()
    }

    /// List resources by category
    pub fn list_resources_by_category(&self, category: ResourceCategory) -> Vec<&LoxoneResource> {
        if let Some(uris) = self.categories.get(&category) {
            uris.iter()
                .filter_map(|uri| self.resources.get(uri))
                .filter(|resource| {
                    // Filter out template resources (with placeholders)
                    !(resource.uri.contains('{') && resource.uri.contains('}'))
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get resource by URI
    pub fn get_resource(&self, uri: &str) -> Option<&LoxoneResource> {
        // Don't return template resources (with placeholders) via direct lookup
        if uri.contains('{') && uri.contains('}') {
            return None;
        }
        self.resources.get(uri)
    }

    /// Parse URI and extract parameters with comprehensive validation
    pub fn parse_uri(&self, uri: &str) -> Result<ResourceContext> {
        // First validate the URI format
        self.validate_uri_format(uri)?;

        let mut query_params = HashMap::new();

        // Split URI and query string
        let (uri_path, query_string) = if let Some(pos) = uri.find('?') {
            let (path, query) = uri.split_at(pos);
            (path, Some(&query[1..]))
        } else {
            (uri, None)
        };

        // Validate the URI path
        self.validate_uri_path(uri_path)?;

        // Parse and validate query parameters
        if let Some(query) = query_string {
            query_params = self.parse_query_parameters(query)?;
        }

        // First try exact matching
        if let Some(_resource) = self.get_resource(uri_path) {
            // Exact match found
            Ok(ResourceContext {
                uri: uri.to_string(),
                params: ResourceParams {
                    path_params: HashMap::new(), // No path params for exact matches
                    query_params,
                },
                timestamp: chrono::Utc::now(),
            })
        } else {
            // Try template matching
            let (matching_resource, path_params) =
                self.find_matching_resource_and_extract_params(uri_path)?;

            // Validate extracted parameters
            self.validate_extracted_parameters(&path_params, &query_params, &matching_resource)?;

            Ok(ResourceContext {
                uri: uri.to_string(),
                params: ResourceParams {
                    path_params,
                    query_params,
                },
                timestamp: chrono::Utc::now(),
            })
        }
    }

    /// Validate URI format and basic structure
    pub fn validate_uri_format(&self, uri: &str) -> Result<()> {
        // Check if URI is empty
        if uri.is_empty() {
            return Err(LoxoneError::invalid_input("URI cannot be empty"));
        }

        // Check URI length (reasonable limit)
        if uri.len() > 2048 {
            return Err(LoxoneError::invalid_input(
                "URI too long (max 2048 characters)",
            ));
        }

        // Check for valid scheme
        if !uri.starts_with("loxone://") {
            return Err(LoxoneError::invalid_input(
                "URI must use 'loxone://' scheme",
            ));
        }

        // Check for template placeholders (curly braces)
        if uri.contains('{') || uri.contains('}') {
            return Err(LoxoneError::invalid_input(
                "URI contains template placeholders (curly braces). Use concrete values instead of template URIs. For example, use 'loxone://rooms/Kitchen/devices' instead of 'loxone://rooms/{roomName}/devices'"
            ));
        }

        // Check for other invalid characters
        let invalid_chars = ['<', '>', '"', '|', '\\', '^', '`', ' '];
        for char in invalid_chars {
            if uri.contains(char) {
                return Err(LoxoneError::invalid_input(format!(
                    "URI contains invalid character: '{char}'"
                )));
            }
        }

        // Check for proper URI structure
        let path_part = &uri[9..]; // Skip "loxone://"
        if path_part.is_empty() {
            return Err(LoxoneError::invalid_input("URI path cannot be empty"));
        }

        // Check for double slashes (except after scheme)
        if path_part.contains("//") {
            return Err(LoxoneError::invalid_input(
                "URI contains invalid double slashes",
            ));
        }

        Ok(())
    }

    /// Validate URI path components
    pub fn validate_uri_path(&self, path: &str) -> Result<()> {
        let path_without_scheme = &path[9..]; // Remove "loxone://"
        let path_parts: Vec<&str> = path_without_scheme.split('/').collect();

        // Check for empty path components
        for (i, part) in path_parts.iter().enumerate() {
            if part.is_empty() && i != path_parts.len() - 1 {
                return Err(LoxoneError::invalid_input(
                    "URI contains empty path component",
                ));
            }
        }

        // Validate known categories
        if !path_parts.is_empty() {
            let category = path_parts[0];
            let valid_categories = [
                "rooms", "devices", "system", "audio", "sensors", "weather", "security", "energy",
            ];
            if !valid_categories.contains(&category) {
                return Err(LoxoneError::invalid_input(format!(
                    "Unknown resource category: '{}'. Valid categories: {}",
                    category,
                    valid_categories.join(", ")
                )));
            }
        }

        Ok(())
    }

    /// Parse and validate query parameters
    pub fn parse_query_parameters(&self, query: &str) -> Result<HashMap<String, String>> {
        let mut params = HashMap::new();

        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }

            if let Some(pos) = pair.find('=') {
                let key = &pair[..pos];
                let value = &pair[pos + 1..];

                // Validate key
                if key.is_empty() {
                    return Err(LoxoneError::invalid_input(
                        "Query parameter key cannot be empty",
                    ));
                }

                // Decode key and value
                let decoded_key = urlencoding::decode(key)
                    .map_err(|e| {
                        LoxoneError::invalid_input(format!(
                            "Invalid URL encoding in key '{key}': {e}"
                        ))
                    })?
                    .to_string();

                let decoded_value = urlencoding::decode(value)
                    .map_err(|e| {
                        LoxoneError::invalid_input(format!(
                            "Invalid URL encoding in value '{value}': {e}"
                        ))
                    })?
                    .to_string();

                // Validate parameter names
                self.validate_query_parameter(&decoded_key, &decoded_value)?;

                params.insert(decoded_key, decoded_value);
            } else {
                // Handle flag parameters (no value)
                let decoded_key = urlencoding::decode(pair)
                    .map_err(|e| {
                        LoxoneError::invalid_input(format!(
                            "Invalid URL encoding in parameter '{pair}': {e}"
                        ))
                    })?
                    .to_string();
                params.insert(decoded_key, "true".to_string());
            }
        }

        Ok(params)
    }

    /// Validate individual query parameter
    pub fn validate_query_parameter(&self, key: &str, value: &str) -> Result<()> {
        // Check parameter name
        if key.len() > 100 {
            return Err(LoxoneError::invalid_input(
                "Query parameter name too long (max 100 characters)",
            ));
        }

        // Check parameter value
        if value.len() > 1000 {
            return Err(LoxoneError::invalid_input(
                "Query parameter value too long (max 1000 characters)",
            ));
        }

        // Validate known parameter names and their values
        match key {
            "include_state" => {
                if !matches!(value, "true" | "false") {
                    return Err(LoxoneError::invalid_input(
                        "Parameter 'include_state' must be 'true' or 'false'",
                    ));
                }
            }
            "limit" => {
                let limit: u32 = value.parse::<u32>().map_err(|_| {
                    LoxoneError::invalid_input("Parameter 'limit' must be a positive integer")
                })?;
                if limit == 0 || limit > 1000 {
                    return Err(LoxoneError::invalid_input(
                        "Parameter 'limit' must be between 1 and 1000",
                    ));
                }
            }
            "offset" => {
                if value.parse::<u32>().is_err() {
                    return Err(LoxoneError::invalid_input(
                        "Parameter 'offset' must be a non-negative integer",
                    ));
                }
            }
            "sort" => {
                let valid_sorts = ["name", "type", "room", "category", "created", "modified"];
                let sort_value = value.trim_start_matches('-'); // Remove descending prefix
                if !valid_sorts.contains(&sort_value) {
                    return Err(LoxoneError::invalid_input(format!(
                        "Parameter 'sort' must be one of: {}. Use '-' prefix for descending order.",
                        valid_sorts.join(", ")
                    )));
                }
            }
            "filter" => {
                // Basic filter validation - could be expanded
                if value.len() < 3 {
                    return Err(LoxoneError::invalid_input(
                        "Parameter 'filter' must be at least 3 characters",
                    ));
                }
            }
            _ => {
                // Allow unknown parameters but log them
                debug!("Unknown query parameter: {} = {}", key, value);
            }
        }

        Ok(())
    }

    /// Find matching resource template and extract parameters
    pub fn find_matching_resource_and_extract_params(
        &self,
        uri_path: &str,
    ) -> Result<(LoxoneResource, HashMap<String, String>)> {
        for resource in self.resources.values() {
            if let Some(params) = self.extract_path_params(&resource.uri, uri_path) {
                return Ok((resource.clone(), params));
            }
        }

        Err(LoxoneError::invalid_input(format!(
            "No matching resource found for URI path: {uri_path}"
        )))
    }

    /// Validate extracted parameters against resource requirements
    pub fn validate_extracted_parameters(
        &self,
        path_params: &HashMap<String, String>,
        query_params: &HashMap<String, String>,
        resource: &LoxoneResource,
    ) -> Result<()> {
        // Validate path parameters based on resource URI template
        if resource.uri.contains("{roomName}") {
            if let Some(room_name) = path_params.get("roomName") {
                self.validate_room_name(room_name)?;
            }
        }

        if resource.uri.contains("{deviceType}") {
            if let Some(device_type) = path_params.get("deviceType") {
                self.validate_device_type(device_type)?;
            }
        }

        if resource.uri.contains("{category}") {
            if let Some(category) = path_params.get("category") {
                self.validate_device_category(category)?;
            }
        }

        // Validate query parameter combinations
        if let (Some(limit), Some(offset)) = (query_params.get("limit"), query_params.get("offset"))
        {
            let limit_val: u32 = limit.parse().unwrap_or(0);
            let offset_val: u32 = offset.parse().unwrap_or(0);

            if offset_val > 0 && limit_val == 0 {
                return Err(LoxoneError::invalid_input(
                    "Cannot use 'offset' without 'limit'",
                ));
            }
        }

        Ok(())
    }

    /// Validate room name parameter
    pub fn validate_room_name(&self, room_name: &str) -> Result<()> {
        if room_name.is_empty() {
            return Err(LoxoneError::invalid_input("Room name cannot be empty"));
        }

        if room_name.len() > 100 {
            return Err(LoxoneError::invalid_input(
                "Room name too long (max 100 characters)",
            ));
        }

        // Check for potentially dangerous characters
        let dangerous_chars = ['<', '>', '"', '\'', '&', '\0'];
        for char in dangerous_chars {
            if room_name.contains(char) {
                return Err(LoxoneError::invalid_input(format!(
                    "Room name contains invalid character: '{char}'"
                )));
            }
        }

        Ok(())
    }

    /// Validate device type parameter
    pub fn validate_device_type(&self, device_type: &str) -> Result<()> {
        if device_type.is_empty() {
            return Err(LoxoneError::invalid_input("Device type cannot be empty"));
        }

        if device_type.len() > 50 {
            return Err(LoxoneError::invalid_input(
                "Device type too long (max 50 characters)",
            ));
        }

        // Validate known device types
        let valid_types = [
            "Switch",
            "Dimmer",
            "LightControllerV2",
            "CentralLightController",
            "Jalousie",
            "Gate",
            "Window",
            "Pushbutton",
            "AnalogInput",
            "DigitalInput",
            "IRoomControllerV2",
            "AudioZone",
            "TimedSwitch",
            "Tracker",
        ];

        if !valid_types.contains(&device_type) {
            debug!("Unknown device type requested: {}", device_type);
            // Allow unknown device types but log them for monitoring
        }

        Ok(())
    }

    /// Validate device category parameter
    pub fn validate_device_category(&self, category: &str) -> Result<()> {
        if category.is_empty() {
            return Err(LoxoneError::invalid_input(
                "Device category cannot be empty",
            ));
        }

        if category.len() > 50 {
            return Err(LoxoneError::invalid_input(
                "Device category too long (max 50 characters)",
            ));
        }

        // Validate known categories
        let valid_categories = [
            "lighting",
            "blinds",
            "climate",
            "security",
            "audio",
            "sensors",
            "energy",
            "irrigation",
            "ventilation",
            "access",
        ];

        if !valid_categories.contains(&category) {
            debug!("Unknown device category requested: {}", category);
            // Allow unknown categories but log them for monitoring
        }

        Ok(())
    }

    /// Extract path parameters from URI template
    fn extract_path_params(
        &self,
        template: &str,
        actual_uri: &str,
    ) -> Option<HashMap<String, String>> {
        let template_parts: Vec<&str> = template.split('/').collect();
        let actual_parts: Vec<&str> = actual_uri.split('/').collect();

        if template_parts.len() != actual_parts.len() {
            return None;
        }

        let mut params = HashMap::new();
        let mut has_params = false;

        for (template_part, actual_part) in template_parts.iter().zip(actual_parts.iter()) {
            if template_part.starts_with('{') && template_part.ends_with('}') {
                // Extract parameter name
                let param_name = &template_part[1..template_part.len() - 1];
                params.insert(param_name.to_string(), actual_part.to_string());
                has_params = true;
            } else if template_part != actual_part {
                // Parts don't match and it's not a parameter
                return None;
            }
        }

        if has_params || template == actual_uri {
            Some(params)
        } else {
            None
        }
    }

    /// Read resource with caching
    pub async fn read_resource_cached<T: ResourceHandler>(
        &self,
        handler: &T,
        context: ResourceContext,
    ) -> Result<ResourceContent> {
        let cache_key = self.create_cache_key(&context);

        // Check cache first and clean up expired entries
        {
            let mut cache = self.cache.write().await;

            // Remove expired entries
            cache.retain(|_, entry| !entry.is_expired());

            // Check for valid cached entry
            if let Some(entry) = cache.get_mut(&cache_key) {
                if !entry.is_expired() {
                    debug!("Cache hit for resource: {}", context.uri);
                    *self.cache_hits.write().await += 1;
                    return Ok(entry.access().clone());
                }
            }
        }

        debug!("Cache miss for resource: {}", context.uri);
        *self.cache_misses.write().await += 1;

        // Fetch from handler
        let content = handler.read_resource(context.clone()).await?;

        // Store in cache with appropriate TTL
        let ttl_seconds = ResourceManager::get_resource_cache_ttl(&context.uri).unwrap_or(120);
        let ttl = Duration::from_secs(ttl_seconds);

        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, CacheEntry::new(content.clone(), ttl));
        }

        Ok(content)
    }

    /// Create cache key for resource context
    pub fn create_cache_key(&self, context: &ResourceContext) -> String {
        // Include URI and relevant parameters in cache key
        let mut key = context.uri.clone();

        // Add path parameters to cache key
        if !context.params.path_params.is_empty() {
            let mut path_params: Vec<_> = context.params.path_params.iter().collect();
            path_params.sort_by_key(|&(k, _)| k);

            key.push_str("?path=");
            for (k, v) in path_params {
                key.push_str(&format!("{k}:{v},"));
            }
        }

        // Add relevant query parameters to cache key
        if !context.params.query_params.is_empty() {
            // Only include parameters that affect content
            let relevant_params = ["include_state", "limit", "offset", "filter", "sort"];
            let mut query_params: Vec<_> = context
                .params
                .query_params
                .iter()
                .filter(|(k, _)| relevant_params.contains(&k.as_str()))
                .collect();
            query_params.sort_by_key(|&(k, _)| k);

            if !query_params.is_empty() {
                key.push_str("&query=");
                for (k, v) in query_params {
                    key.push_str(&format!("{k}:{v},"));
                }
            }
        }

        key
    }

    /// Clear cache for specific resource pattern
    pub async fn invalidate_cache(&self, uri_pattern: &str) {
        let mut cache = self.cache.write().await;
        cache.retain(|key, _| !key.starts_with(uri_pattern));
        debug!("Invalidated cache for pattern: {}", uri_pattern);
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (usize, u64, u64, f64) {
        let cache = self.cache.read().await;
        let hits = *self.cache_hits.read().await;
        let misses = *self.cache_misses.read().await;
        let hit_ratio = if hits + misses > 0 {
            hits as f64 / (hits + misses) as f64
        } else {
            0.0
        };

        (cache.len(), hits, misses, hit_ratio)
    }

    /// Clear all cached resources
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        *self.cache_hits.write().await = 0;
        *self.cache_misses.write().await = 0;
        debug!("Cleared all resource cache");
    }

    /// Cleanup expired cache entries
    pub async fn cleanup_cache(&self) {
        let mut cache = self.cache.write().await;
        let before_count = cache.len();
        cache.retain(|_, entry| !entry.is_expired());
        let after_count = cache.len();

        if before_count > after_count {
            debug!(
                "Cleaned up {} expired cache entries",
                before_count - after_count
            );
        }
    }

    /// Get cache TTL for resource URI
    pub fn get_resource_cache_ttl(uri: &str) -> Option<u64> {
        match uri {
            // Static structure data - cache longer
            uri if uri.starts_with("loxone://rooms")
                || uri.starts_with("loxone://devices")
                || uri == "loxone://system/capabilities"
                || uri == "loxone://system/categories" =>
            {
                Some(600)
            } // 10 minutes

            // Dynamic status data - shorter cache
            "loxone://system/status" => Some(60), // 1 minute

            // Audio and sensor data - very short cache
            uri if uri.starts_with("loxone://audio") || uri.starts_with("loxone://sensors") => {
                Some(30)
            } // 30 seconds

            // Weather data - very short cache since it changes frequently
            uri if uri.starts_with("loxone://weather") => Some(30), // 30 seconds

            // Security data - short cache for real-time security status
            uri if uri.starts_with("loxone://security") => Some(10), // 10 seconds for security

            // Energy data - medium cache for power consumption data
            uri if uri.starts_with("loxone://energy") => Some(60), // 1 minute for energy data

            _ => Some(120), // Default 2 minutes
        }
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource handler trait for implementing resource data providers
#[allow(async_fn_in_trait)]
pub trait ResourceHandler {
    /// Handle resource read request
    async fn read_resource(&self, context: ResourceContext) -> Result<ResourceContent>;
}

/// Implementation of ResourceHandler for ResourceManager  
// Temporarily disabled - resource handlers need to be reimplemented
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_manager_creation() {
        let manager = ResourceManager::new();
        assert!(!manager.resources.is_empty());
        assert!(!manager.categories.is_empty());
    }

    #[test]
    fn test_resource_registration() {
        let mut manager = ResourceManager::new();
        let initial_count = manager.resources.len();

        let test_resource = LoxoneResource {
            uri: "loxone://test/resource".to_string(),
            name: "Test Resource".to_string(),
            description: "A test resource".to_string(),
            mime_type: Some("application/json".to_string()),
        };

        manager.register_resource(test_resource.clone(), ResourceCategory::System);

        assert_eq!(manager.resources.len(), initial_count + 1);
        assert!(manager.get_resource("loxone://test/resource").is_some());
    }

    #[test]
    fn test_uri_parameter_extraction() {
        let manager = ResourceManager::new();

        // Test simple parameter extraction
        let params = manager
            .extract_path_params(
                "loxone://rooms/{roomName}/devices",
                "loxone://rooms/LivingRoom/devices",
            )
            .unwrap();

        assert_eq!(params.get("roomName"), Some(&"LivingRoom".to_string()));
    }

    #[test]
    fn test_resource_categories() {
        assert_eq!(ResourceCategory::Rooms.uri_prefix(), "loxone://rooms");
        assert_eq!(ResourceCategory::Devices.uri_prefix(), "loxone://devices");
        assert_eq!(ResourceCategory::System.uri_prefix(), "loxone://system");
        assert_eq!(ResourceCategory::Audio.uri_prefix(), "loxone://audio");
        assert_eq!(ResourceCategory::Sensors.uri_prefix(), "loxone://sensors");
        assert_eq!(ResourceCategory::Weather.uri_prefix(), "loxone://weather");
    }

    #[test]
    fn test_resource_context_creation() {
        let manager = ResourceManager::new();

        // Use a registered concrete URI
        let context = manager
            .parse_uri("loxone://rooms?include_state=true")
            .unwrap();

        assert_eq!(context.uri, "loxone://rooms?include_state=true");
        // Concrete URIs don't have path parameters
        assert!(context.params.path_params.is_empty());
        assert_eq!(
            context.params.query_params.get("include_state"),
            Some(&"true".to_string())
        );
    }
}
