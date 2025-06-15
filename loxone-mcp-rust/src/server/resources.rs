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

use crate::{
    error::{LoxoneError, Result},
    server::LoxoneMcpServer,
};
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

        // Note: Room-specific device resources would need to be generated dynamically
        // based on actual rooms discovered in the system. For now, use tools instead.

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

        // Note: Device type filtering would need concrete resources or use tools instead

        // Device category resources - register common categories
        self.register_resource(
            LoxoneResource {
                uri: "loxone://devices/category/blinds".to_string(),
                name: "Blinds/Rolladen Devices".to_string(),
                description: "All blinds and rolladen devices with current positions".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Devices,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://devices/category/lighting".to_string(),
                name: "Lighting Devices".to_string(),
                description: "All lighting devices and their current states".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Devices,
        );

        self.register_resource(
            LoxoneResource {
                uri: "loxone://devices/category/climate".to_string(),
                name: "Climate Devices".to_string(),
                description: "All climate control devices and sensors".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceCategory::Devices,
        );

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
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get resource by URI
    pub fn get_resource(&self, uri: &str) -> Option<&LoxoneResource> {
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

        // Find exact matching resource (no templating)
        let _matching_resource = self.get_resource(uri_path).ok_or_else(|| {
            LoxoneError::invalid_input(format!("Resource not found: {}", uri_path))
        })?;

        Ok(ResourceContext {
            uri: uri.to_string(),
            params: ResourceParams {
                path_params: HashMap::new(), // No path params for concrete resources
                query_params,
            },
            timestamp: chrono::Utc::now(),
        })
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
                    "URI contains invalid character: '{}'",
                    char
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
                            "Invalid URL encoding in key '{}': {}",
                            key, e
                        ))
                    })?
                    .to_string();

                let decoded_value = urlencoding::decode(value)
                    .map_err(|e| {
                        LoxoneError::invalid_input(format!(
                            "Invalid URL encoding in value '{}': {}",
                            value, e
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
                            "Invalid URL encoding in parameter '{}': {}",
                            pair, e
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
                if value.parse::<u32>().is_err() {
                    return Err(LoxoneError::invalid_input(
                        "Parameter 'limit' must be a positive integer",
                    ));
                }
                let limit: u32 = value.parse().unwrap();
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
            "No matching resource found for URI path: {}",
            uri_path
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
                    "Room name contains invalid character: '{}'",
                    char
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
                key.push_str(&format!("{}:{},", k, v));
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
                    key.push_str(&format!("{}:{},", k, v));
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

/// Implementation of ResourceHandler for LoxoneMcpServer
impl ResourceHandler for LoxoneMcpServer {
    async fn read_resource(&self, context: ResourceContext) -> Result<ResourceContent> {
        debug!("Reading resource: {}", context.uri);

        let data = match context.uri.as_str() {
            // Route to appropriate handler based on exact URI match
            "loxone://rooms" => self.read_rooms_resource().await?,
            "loxone://devices/all" => self.read_all_devices_resource().await?,
            "loxone://system/status" => self.read_system_status_resource().await?,
            "loxone://system/capabilities" => self.read_system_capabilities_resource().await?,
            "loxone://system/categories" => self.read_system_categories_resource().await?,
            "loxone://audio/zones" => self.read_audio_zones_resource().await?,
            "loxone://audio/sources" => self.read_audio_sources_resource().await?,
            "loxone://sensors/door-window" => self.read_door_window_sensors_resource().await?,
            "loxone://sensors/temperature" => self.read_temperature_sensors_resource().await?,
            "loxone://sensors/discovered" => self.read_discovered_sensors_resource().await?,
            "loxone://weather/current" => self.read_weather_current_resource().await?,
            "loxone://weather/outdoor-conditions" => {
                self.read_weather_outdoor_conditions_resource().await?
            }
            "loxone://weather/forecast-daily" => {
                self.read_weather_forecast_daily_resource().await?
            }
            "loxone://weather/forecast-hourly" => {
                self.read_weather_forecast_hourly_resource().await?
            }
            "loxone://security/status" => self.read_security_status_resource().await?,
            "loxone://security/zones" => self.read_security_zones_resource().await?,
            "loxone://energy/consumption" => self.read_energy_consumption_resource().await?,
            "loxone://energy/meters" => self.read_energy_meters_resource().await?,
            "loxone://energy/usage-history" => self.read_energy_usage_history_resource().await?,
            // Device category resources
            "loxone://devices/category/blinds" => {
                self.read_devices_category_blinds_resource().await?
            }
            "loxone://devices/category/lighting" => {
                self.read_devices_category_lighting_resource().await?
            }
            "loxone://devices/category/climate" => {
                self.read_devices_category_climate_resource().await?
            }
            _ => {
                return Err(LoxoneError::invalid_input(format!(
                    "Unknown resource URI: {}. Available resources: loxone://rooms, loxone://devices/*, loxone://system/*, loxone://audio/*, loxone://sensors/*, loxone://weather/*, loxone://security/*, loxone://energy/*",
                    context.uri
                )));
            }
        };

        let content_str = serde_json::to_string(&data)?;
        let metadata = ResourceMetadata {
            content_type: "application/json".to_string(),
            last_modified: chrono::Utc::now(),
            etag: format!("{:x}", md5::compute(&content_str)),
            cache_ttl: ResourceManager::get_resource_cache_ttl(&context.uri),
            size: content_str.len(),
        };

        Ok(ResourceContent { data, metadata })
    }
}

impl LoxoneMcpServer {
    /// Resource handlers - implement the actual data retrieval
    async fn read_rooms_resource(&self) -> Result<serde_json::Value> {
        let rooms = self.context.rooms.read().await;
        let rooms_data: Vec<_> = rooms
            .iter()
            .map(|(uuid, room)| {
                serde_json::json!({
                    "uuid": uuid,
                    "name": room.name,
                    "device_count": room.device_count
                })
            })
            .collect();

        Ok(serde_json::json!({
            "total_rooms": rooms.len(),
            "rooms": rooms_data,
            "uri": "loxone://rooms"
        }))
    }

    async fn read_all_devices_resource(&self) -> Result<serde_json::Value> {
        let devices = self.context.devices.read().await;
        let rooms = self.context.rooms.read().await;

        let device_list: Vec<_> = devices
            .values()
            .map(|device| {
                let room_name = device
                    .room
                    .as_ref()
                    .and_then(|room_uuid| rooms.get(room_uuid))
                    .map(|room| room.name.clone())
                    .unwrap_or_else(|| "No Room".to_string());

                serde_json::json!({
                    "uuid": device.uuid,
                    "name": device.name,
                    "type": device.device_type,
                    "category": device.category,
                    "room": room_name,
                    "states": device.states
                })
            })
            .collect();

        Ok(serde_json::json!({
            "total_devices": devices.len(),
            "devices": device_list,
            "uri": "loxone://devices/all"
        }))
    }

    async fn read_system_status_resource(&self) -> Result<serde_json::Value> {
        let health_status = match self.client.health_check().await {
            Ok(is_healthy) => {
                if is_healthy {
                    "healthy"
                } else {
                    "degraded"
                }
            }
            Err(_) => "unhealthy",
        };

        let capabilities = self.context.capabilities.read().await;
        let rooms = self.context.rooms.read().await;
        let devices = self.context.devices.read().await;

        Ok(serde_json::json!({
            "status": health_status,
            "timestamp": chrono::Utc::now(),
            "statistics": {
                "total_rooms": rooms.len(),
                "total_devices": devices.len(),
                "lighting_devices": capabilities.light_count,
                "blind_devices": capabilities.blind_count,
                "sensor_devices": capabilities.sensor_count,
                "climate_devices": capabilities.climate_count
            },
            "capabilities": {
                "has_lighting": capabilities.has_lighting,
                "has_blinds": capabilities.has_blinds,
                "has_sensors": capabilities.has_sensors,
                "has_climate": capabilities.has_climate,
                "has_audio": capabilities.has_audio
            },
            "uri": "loxone://system/status"
        }))
    }

    async fn read_system_capabilities_resource(&self) -> Result<serde_json::Value> {
        // Use existing get_available_capabilities logic
        use crate::tools::{devices::get_available_capabilities, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());
        let response = get_available_capabilities(tool_context).await;

        if response.status == "success" {
            Ok(response.data)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Failed to get capabilities: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            )))
        }
    }

    async fn read_system_categories_resource(&self) -> Result<serde_json::Value> {
        // Use existing get_all_categories_overview logic
        use crate::tools::{devices::get_all_categories_overview, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());
        let response = get_all_categories_overview(tool_context).await;

        if response.status == "success" {
            Ok(response.data)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Failed to get categories: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            )))
        }
    }

    async fn read_audio_zones_resource(&self) -> Result<serde_json::Value> {
        use crate::tools::{audio::get_audio_zones, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());
        let response = get_audio_zones(tool_context).await;

        Ok(response.data)
    }

    async fn read_audio_sources_resource(&self) -> Result<serde_json::Value> {
        use crate::tools::{audio::get_audio_sources, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());
        let response = get_audio_sources(tool_context).await;

        Ok(response.data)
    }

    async fn read_door_window_sensors_resource(&self) -> Result<serde_json::Value> {
        use crate::tools::{sensors::get_all_door_window_sensors, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());
        let response = get_all_door_window_sensors(tool_context).await;

        if response.status == "success" {
            Ok(response.data)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Failed to get door/window sensors: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            )))
        }
    }

    async fn read_temperature_sensors_resource(&self) -> Result<serde_json::Value> {
        use crate::tools::{sensors::get_temperature_sensors, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());
        let response = get_temperature_sensors(tool_context).await;

        if response.status == "success" {
            Ok(response.data)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Failed to get temperature sensors: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            )))
        }
    }

    async fn read_discovered_sensors_resource(&self) -> Result<serde_json::Value> {
        use crate::tools::{sensors::list_discovered_sensors, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());
        let response = list_discovered_sensors(tool_context, None, None).await;

        if response.status == "success" {
            Ok(response.data)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Failed to get discovered sensors: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            )))
        }
    }

    /// Weather resource handlers
    async fn read_weather_current_resource(&self) -> Result<serde_json::Value> {
        use crate::tools::{weather::get_weather_data, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());
        let response = get_weather_data(tool_context).await;

        if response.status == "success" {
            Ok(response.data)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Failed to get weather data: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            )))
        }
    }

    async fn read_weather_outdoor_conditions_resource(&self) -> Result<serde_json::Value> {
        use crate::tools::{weather::get_outdoor_conditions, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());
        let response = get_outdoor_conditions(tool_context).await;

        if response.status == "success" {
            Ok(response.data)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Failed to get outdoor conditions: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            )))
        }
    }

    async fn read_weather_forecast_daily_resource(&self) -> Result<serde_json::Value> {
        use crate::tools::{weather::get_weather_forecast_daily, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());
        let response = get_weather_forecast_daily(tool_context, None).await;

        if response.status == "success" {
            Ok(response.data)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Failed to get daily forecast: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            )))
        }
    }

    async fn read_weather_forecast_hourly_resource(&self) -> Result<serde_json::Value> {
        use crate::tools::{weather::get_weather_forecast_hourly, ToolContext};

        let tool_context = ToolContext::new(self.client.clone(), self.context.clone());
        let response = get_weather_forecast_hourly(tool_context, None).await;

        if response.status == "success" {
            Ok(response.data)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Failed to get hourly forecast: {}",
                response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            )))
        }
    }

    /// Security resource handlers
    async fn read_security_status_resource(&self) -> Result<serde_json::Value> {
        // Get security-related devices
        let devices = match self.context.get_devices_by_category("security").await {
            Ok(devices) => devices,
            Err(_) => {
                // If no security category devices, return basic security status
                return Ok(serde_json::json!({
                    "status": "no_security_devices",
                    "message": "No security devices found in the system",
                    "timestamp": chrono::Utc::now(),
                    "uri": "loxone://security/status"
                }));
            }
        };

        let mut security_devices = Vec::new();
        let mut zones_armed = 0;
        let mut zones_total = 0;

        for device in devices {
            zones_total += 1;

            // Check device state to determine if it's armed/active
            let is_active = device
                .states
                .get("active")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if is_active {
                zones_armed += 1;
            }

            security_devices.push(serde_json::json!({
                "uuid": device.uuid,
                "name": device.name,
                "type": device.device_type,
                "room": device.room,
                "active": is_active,
                "states": device.states
            }));
        }

        // Determine overall alarm status
        let alarm_status = if zones_total == 0 {
            "no_zones"
        } else if zones_armed == 0 {
            "disarmed"
        } else if zones_armed == zones_total {
            "fully_armed"
        } else {
            "partially_armed"
        };

        Ok(serde_json::json!({
            "status": alarm_status,
            "zones_armed": zones_armed,
            "zones_total": zones_total,
            "devices": security_devices,
            "timestamp": chrono::Utc::now(),
            "uri": "loxone://security/status"
        }))
    }

    async fn read_security_zones_resource(&self) -> Result<serde_json::Value> {
        // Use the same logic as security status but focus on zone details
        let security_status = self.read_security_status_resource().await?;

        if let Some(devices) = security_status.get("devices") {
            Ok(serde_json::json!({
                "zones": devices,
                "zone_count": security_status.get("zones_total").unwrap_or(&serde_json::json!(0)),
                "armed_count": security_status.get("zones_armed").unwrap_or(&serde_json::json!(0)),
                "timestamp": chrono::Utc::now(),
                "uri": "loxone://security/zones"
            }))
        } else {
            Ok(serde_json::json!({
                "zones": [],
                "zone_count": 0,
                "armed_count": 0,
                "message": "No security zones found",
                "timestamp": chrono::Utc::now(),
                "uri": "loxone://security/zones"
            }))
        }
    }

    /// Energy resource handlers
    async fn read_energy_consumption_resource(&self) -> Result<serde_json::Value> {
        // Get energy-related devices (meters, power monitoring devices)
        let devices = match self.context.get_devices_by_category("energy").await {
            Ok(devices) => devices,
            Err(_) => {
                // If no energy category devices, return basic consumption info
                return Ok(serde_json::json!({
                    "total_consumption": 0.0,
                    "current_power": 0.0,
                    "message": "No energy devices found in the system",
                    "timestamp": chrono::Utc::now(),
                    "uri": "loxone://energy/consumption"
                }));
            }
        };

        let mut total_power = 0.0;
        let mut total_consumption = 0.0;
        let mut energy_devices = Vec::new();
        let mut room_consumption = std::collections::HashMap::new();

        for device in devices {
            let current_power = device
                .states
                .get("power")
                .or_else(|| device.states.get("value"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let consumption = device
                .states
                .get("consumption")
                .or_else(|| device.states.get("total"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            total_power += current_power;
            total_consumption += consumption;

            // Track consumption by room
            if let Some(ref room) = device.room {
                let room_total = room_consumption.get(room).unwrap_or(&0.0);
                room_consumption.insert(room.clone(), room_total + current_power);
            }

            energy_devices.push(serde_json::json!({
                "uuid": device.uuid,
                "name": device.name,
                "type": device.device_type,
                "room": device.room,
                "current_power": current_power,
                "total_consumption": consumption,
                "states": device.states
            }));
        }

        Ok(serde_json::json!({
            "total_consumption": total_consumption,
            "current_power": total_power,
            "device_count": energy_devices.len(),
            "devices": energy_devices,
            "room_breakdown": room_consumption,
            "timestamp": chrono::Utc::now(),
            "uri": "loxone://energy/consumption"
        }))
    }

    async fn read_energy_meters_resource(&self) -> Result<serde_json::Value> {
        // Get all energy meter devices specifically
        let all_devices = self.context.devices.read().await;
        let energy_meters: Vec<_> = all_devices
            .values()
            .filter(|device| {
                device.device_type.to_lowercase().contains("meter")
                    || device.device_type.to_lowercase().contains("energy")
                    || device.device_type.to_lowercase().contains("power")
            })
            .collect();

        let mut meters = Vec::new();
        for device in energy_meters {
            let reading = device
                .states
                .get("value")
                .or_else(|| device.states.get("power"))
                .or_else(|| device.states.get("consumption"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            meters.push(serde_json::json!({
                "uuid": device.uuid,
                "name": device.name,
                "type": device.device_type,
                "room": device.room,
                "reading": reading,
                "unit": "kWh", // Default unit
                "states": device.states
            }));
        }

        Ok(serde_json::json!({
            "meters": meters,
            "meter_count": meters.len(),
            "timestamp": chrono::Utc::now(),
            "uri": "loxone://energy/meters"
        }))
    }

    async fn read_energy_usage_history_resource(&self) -> Result<serde_json::Value> {
        // For now, return placeholder historical data
        // In a real implementation, this would query historical energy data
        let now = chrono::Utc::now();
        let mut history = Vec::new();

        // Generate sample historical data for the last 24 hours
        for hour in 0..24 {
            let timestamp = now - chrono::Duration::hours(hour as i64);
            history.push(serde_json::json!({
                "timestamp": timestamp,
                "consumption": 2.5 + (hour as f64 * 0.1), // Sample data
                "cost": 0.12 * (2.5 + (hour as f64 * 0.1)), // Sample cost calculation
            }));
        }

        history.reverse(); // Chronological order

        Ok(serde_json::json!({
            "history": history,
            "period": "24_hours",
            "total_consumption": history.iter()
                .filter_map(|h| h.get("consumption").and_then(|v| v.as_f64()))
                .sum::<f64>(),
            "estimated_cost": history.iter()
                .filter_map(|h| h.get("cost").and_then(|v| v.as_f64()))
                .sum::<f64>(),
            "note": "Sample historical data - integrate with energy monitoring system for real data",
            "timestamp": chrono::Utc::now(),
            "uri": "loxone://energy/usage-history"
        }))
    }

    /// Device category resource handlers
    async fn read_devices_category_blinds_resource(&self) -> Result<serde_json::Value> {
        let devices = match self.context.get_devices_by_category("blinds").await {
            Ok(devices) => devices,
            Err(e) => {
                return Err(LoxoneError::invalid_input(format!(
                    "Failed to get blinds devices: {}",
                    e
                )));
            }
        };

        // Collect all state UUIDs for batch resolution
        let mut all_state_uuids = Vec::new();
        for device in &devices {
            if let Some(position_uuid) = device.states.get("position").and_then(|v| v.as_str()) {
                all_state_uuids.push(position_uuid.to_string());
            }
            if let Some(shade_position_uuid) =
                device.states.get("shadePosition").and_then(|v| v.as_str())
            {
                all_state_uuids.push(shade_position_uuid.to_string());
            }
            if let Some(target_position_uuid) =
                device.states.get("targetPosition").and_then(|v| v.as_str())
            {
                all_state_uuids.push(target_position_uuid.to_string());
            }
        }

        // Try to get resolved state values using new state UUID resolution
        let resolved_state_values = match self.client.get_state_values(&all_state_uuids).await {
            Ok(values) => {
                tracing::info!("Successfully resolved {} state UUIDs", values.len());
                values
            }
            Err(e) => {
                tracing::warn!(
                    "Could not resolve state UUIDs: {}, falling back to device states",
                    e
                );
                HashMap::new()
            }
        };

        // Also get current device states as fallback
        let uuids: Vec<String> = devices.iter().map(|d| d.uuid.clone()).collect();
        let device_states = match self.client.get_device_states(&uuids).await {
            Ok(states) => states,
            Err(e) => {
                tracing::warn!("Could not retrieve device states: {}", e);
                HashMap::new()
            }
        };

        let mut blinds_with_states = Vec::new();

        for device in devices {
            // Extract state UUIDs
            let position_uuid = device.states.get("position").and_then(|v| v.as_str());
            let shade_position_uuid = device.states.get("shadePosition").and_then(|v| v.as_str());
            let target_position_uuid = device.states.get("targetPosition").and_then(|v| v.as_str());

            // Try to get position value from resolved state UUIDs first
            let position_value = if let Some(position_uuid) = position_uuid {
                if let Some(resolved_value) = resolved_state_values.get(position_uuid) {
                    resolved_value.as_f64().unwrap_or(-1.0)
                } else {
                    // Fallback to device state
                    device_states
                        .get(&device.uuid)
                        .and_then(|state| state.as_f64())
                        .unwrap_or(-1.0)
                }
            } else {
                // No position UUID, try device state
                device_states
                    .get(&device.uuid)
                    .and_then(|state| state.as_f64())
                    .unwrap_or(-1.0)
            };

            // Also get shade position if available
            let shade_position_value = if let Some(shade_uuid) = shade_position_uuid {
                resolved_state_values
                    .get(shade_uuid)
                    .and_then(|v| v.as_f64())
                    .unwrap_or(-1.0)
            } else {
                -1.0
            };

            // Also get target position if available
            let target_position_value = if let Some(target_uuid) = target_position_uuid {
                resolved_state_values
                    .get(target_uuid)
                    .and_then(|v| v.as_f64())
                    .unwrap_or(-1.0)
            } else {
                -1.0
            };

            let (position_desc, position_percent, status) = if position_value < 0.0 {
                ("unknown".to_string(), None, "no_data")
            } else if position_value == 0.0 {
                ("closed".to_string(), Some(0), "closed")
            } else if position_value == 1.0 {
                ("open".to_string(), Some(100), "open")
            } else {
                let percent = (position_value * 100.0).round() as i32;
                (format!("{}% open", percent), Some(percent), "partial")
            };

            // Check for moving state
            let is_moving = device
                .states
                .get("moving")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Determine data source based on what we successfully retrieved
            let data_source = if position_uuid.is_some()
                && resolved_state_values.contains_key(position_uuid.unwrap())
            {
                "resolved_state_uuid"
            } else if device_states.contains_key(&device.uuid) {
                "device_state"
            } else {
                "cached"
            };

            blinds_with_states.push(serde_json::json!({
                "uuid": device.uuid,
                "name": device.name,
                "type": device.device_type,
                "room": device.room,
                "position": position_desc,
                "position_percent": position_percent,
                "position_value": if position_value < 0.0 { serde_json::Value::Null } else { serde_json::Value::from(position_value) },
                "shade_position_value": if shade_position_value < 0.0 { serde_json::Value::Null } else { serde_json::Value::from(shade_position_value) },
                "target_position_value": if target_position_value < 0.0 { serde_json::Value::Null } else { serde_json::Value::from(target_position_value) },
                "status": status,
                "is_moving": is_moving,
                "state_uuids": {
                    "position": position_uuid,
                    "shade_position": shade_position_uuid,
                    "target_position": target_position_uuid
                },
                "resolved_values": {
                    "position": if let Some(uuid) = position_uuid { resolved_state_values.get(uuid).cloned() } else { None },
                    "shade_position": if let Some(uuid) = shade_position_uuid { resolved_state_values.get(uuid).cloned() } else { None },
                    "target_position": if let Some(uuid) = target_position_uuid { resolved_state_values.get(uuid).cloned() } else { None }
                },
                "available_states": device.states.keys().cloned().collect::<Vec<String>>(),
                "data_source": data_source,
                "note": if resolved_state_values.is_empty() { 
                    "Using fallback device states. State UUID resolution failed - may need WebSocket connection or different API endpoints." 
                } else {
                    "Using resolved state UUID values for accurate position data." 
                }
            }));
        }

        // Calculate summary statistics
        let total_devices = blinds_with_states.len();
        let closed_count = blinds_with_states
            .iter()
            .filter(|d| d["status"] == "closed")
            .count();
        let open_count = blinds_with_states
            .iter()
            .filter(|d| d["status"] == "open")
            .count();
        let partial_count = blinds_with_states
            .iter()
            .filter(|d| d["status"] == "partial")
            .count();
        let unknown_count = blinds_with_states
            .iter()
            .filter(|d| d["status"] == "no_data")
            .count();

        Ok(serde_json::json!({
            "devices": blinds_with_states,
            "summary": {
                "total_devices": total_devices,
                "closed": closed_count,
                "open": open_count,
                "partial": partial_count,
                "unknown": unknown_count,
                "problem": if unknown_count > 0 {
                    format!("{} devices have unknown status because the current API implementation returns '0' for all position states", unknown_count)
                } else {
                    "All devices have known status".to_string()
                }
            },
            "next_steps": {
                "to_get_real_positions": [
                    "Use WebSocket connection for real-time state updates (like the Loxone web interface)",
                    "Implement state UUID resolution to convert state UUIDs to actual values",
                    "Use different Loxone API endpoints that return actual position values"
                ]
            },
            "timestamp": chrono::Utc::now(),
            "uri": "loxone://devices/category/blinds"
        }))
    }

    async fn read_devices_category_lighting_resource(&self) -> Result<serde_json::Value> {
        let devices = match self.context.get_devices_by_category("lighting").await {
            Ok(devices) => devices,
            Err(e) => {
                return Err(LoxoneError::invalid_input(format!(
                    "Failed to get lighting devices: {}",
                    e
                )));
            }
        };

        // Get current states for all lighting devices
        let uuids: Vec<String> = devices.iter().map(|d| d.uuid.clone()).collect();
        let states = self
            .client
            .get_device_states(&uuids)
            .await
            .unwrap_or_default();

        let lighting_devices: Vec<_> = devices
            .iter()
            .map(|device| {
                let device_state = states.get(&device.uuid);
                let is_on = device_state
                    .and_then(|state| state.as_f64())
                    .map(|v| v > 0.0)
                    .unwrap_or(false);

                let brightness = device_state
                    .and_then(|state| state.as_f64())
                    .map(|v| (v * 100.0).round() as i32)
                    .unwrap_or(0);

                serde_json::json!({
                    "uuid": device.uuid,
                    "name": device.name,
                    "type": device.device_type,
                    "room": device.room,
                    "status": if is_on { "on" } else { "off" },
                    "brightness_percent": brightness,
                    "raw_state": device_state,
                    "cached_states": device.states
                })
            })
            .collect();

        Ok(serde_json::json!({
            "devices": lighting_devices,
            "total_devices": lighting_devices.len(),
            "timestamp": chrono::Utc::now(),
            "uri": "loxone://devices/category/lighting"
        }))
    }

    async fn read_devices_category_climate_resource(&self) -> Result<serde_json::Value> {
        let devices = match self.context.get_devices_by_category("climate").await {
            Ok(devices) => devices,
            Err(e) => {
                return Err(LoxoneError::invalid_input(format!(
                    "Failed to get climate devices: {}",
                    e
                )));
            }
        };

        let climate_devices: Vec<_> = devices
            .iter()
            .map(|device| {
                serde_json::json!({
                    "uuid": device.uuid,
                    "name": device.name,
                    "type": device.device_type,
                    "room": device.room,
                    "states": device.states
                })
            })
            .collect();

        Ok(serde_json::json!({
            "devices": climate_devices,
            "total_devices": climate_devices.len(),
            "timestamp": chrono::Utc::now(),
            "uri": "loxone://devices/category/climate"
        }))
    }
}

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
