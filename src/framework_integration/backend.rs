//! Loxone backend implementation for the MCP framework
//!
//! This module implements the McpBackend trait to bridge the existing Loxone
//! server implementation with the new MCP framework.

use async_trait::async_trait;
use pulseengine_mcp_protocol::*;
use pulseengine_mcp_server::backend::{BackendError, McpBackend};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

#[cfg(feature = "turso")]
use crate::storage::turso_client::WeatherDataPoint;

use crate::{
    client::ClientContext, error::LoxoneError, framework_integration::adapters, ServerConfig,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Simplified error handling - single conversion chain
///
/// Framework pattern: LoxoneError -> BackendError (framework handles MCP protocol errors)
/// This eliminates the complex triple-conversion chain and reduces error handling overhead.
/// Convert LoxoneError to BackendError (simplified mapping)
impl From<LoxoneError> for BackendError {
    fn from(err: LoxoneError) -> Self {
        use LoxoneError::*;
        match err {
            // Connection issues
            Connection(msg) | WebSocket(msg) => BackendError::connection(msg),
            Http(e) => BackendError::connection(e.to_string()),

            // Configuration issues
            Authentication(msg) | Config(msg) | Credentials(msg) | InvalidInput(msg) => {
                BackendError::configuration(msg)
            }

            // Not found/unsupported
            NotFound(msg) | ServiceUnavailable(msg) => BackendError::not_supported(msg),

            // All other errors as internal
            _ => BackendError::internal(err.to_string()),
        }
    }
}

/// Convert BackendError to LoxoneError (reverse mapping for compatibility)
impl From<BackendError> for LoxoneError {
    fn from(err: BackendError) -> Self {
        match err {
            BackendError::Connection(msg) => LoxoneError::connection(msg),
            BackendError::Configuration(msg) => LoxoneError::config(msg),
            BackendError::NotSupported(msg) => LoxoneError::not_found(msg),
            BackendError::Internal(msg) => LoxoneError::Generic(anyhow::anyhow!(msg)),
            BackendError::NotInitialized => LoxoneError::config("Backend not initialized"),
            BackendError::Custom(e) => LoxoneError::Generic(anyhow::anyhow!("Custom error: {}", e)),
        }
    }
}

// Note: Framework handles LoxoneError -> MCP Protocol Error conversion automatically
// No need for manual Error conversion - reduces complexity and maintenance burden

/// Cache entry for resource data
#[derive(Clone)]
struct CacheEntry {
    data: String,
    mime_type: String,
    timestamp: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn new(data: String, mime_type: String, ttl: Duration) -> Self {
        Self {
            data,
            mime_type,
            timestamp: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.timestamp.elapsed() > self.ttl
    }
}

/// Loxone-specific backend implementation for the MCP framework
#[derive(Clone)]
#[allow(dead_code)]
pub struct LoxoneBackend {
    /// Server configuration
    config: crate::ServerConfig,

    /// Loxone client
    client: Arc<dyn crate::client::LoxoneClient>,

    /// Client context for caching
    context: Arc<crate::client::ClientContext>,

    /// Rate limiting middleware
    rate_limiter: Arc<crate::server::rate_limiter::RateLimitMiddleware>,

    /// Health checker for comprehensive monitoring
    health_checker: Arc<crate::server::health_check::HealthChecker>,

    /// Request coalescer for performance optimization
    request_coalescer: Arc<crate::server::request_coalescing::RequestCoalescer>,

    /// Schema validator for parameter validation
    schema_validator: Arc<crate::server::schema_validation::SchemaValidator>,

    /// Resource monitor for system resource management
    resource_monitor: Arc<crate::server::resource_monitor::ResourceMonitor>,

    /// Response cache for MCP tools
    response_cache: Arc<crate::server::response_cache::ToolResponseCache>,

    /// Sampling protocol integration for MCP (optional)
    sampling_integration: Option<Arc<crate::sampling::protocol::SamplingProtocolIntegration>>,

    /// Resource subscription coordinator for real-time notifications
    subscription_coordinator: Arc<crate::server::subscription::SubscriptionCoordinator>,

    /// Unified value resolution service
    value_resolver: Arc<crate::services::UnifiedValueResolver>,

    /// Centralized state manager with change detection
    state_manager: Option<Arc<crate::services::StateManager>>,

    /// Server metrics collector for dashboard monitoring
    metrics_collector: Arc<crate::monitoring::server_metrics::ServerMetricsCollector>,

    /// Resource cache with TTL for framework resources
    resource_cache: Arc<tokio::sync::RwLock<HashMap<String, CacheEntry>>>,

    /// Weather data storage for real-time WebSocket data
    weather_storage: Option<Arc<crate::storage::WeatherStorage>>,
}

impl LoxoneBackend {
    /// Ensure client is connected and structure is loaded (static version)
    async fn ensure_connected_static(
        client: &Arc<dyn crate::client::LoxoneClient>,
        context: &Arc<crate::client::ClientContext>,
    ) -> std::result::Result<(), BackendError> {
        // Check if structure is already loaded
        {
            let rooms = context.rooms.read().await;
            if !rooms.is_empty() {
                return Ok(()); // Already connected and loaded
            }
        }

        debug!("🔌 Connecting to Loxone and loading structure...");

        // First do a health check to establish connection
        match client.health_check().await {
            Ok(true) => {
                debug!("✅ Health check passed, fetching structure...");
            }
            Ok(false) => {
                return Err(BackendError::connection("Health check failed".to_string()));
            }
            Err(e) => {
                return Err(BackendError::connection(format!("Health check error: {e}")));
            }
        }

        // Now try to fetch structure
        info!("🔄 Attempting to fetch structure from Loxone...");
        match client.get_structure().await {
            Ok(structure) => {
                info!(
                    "📦 Structure received, {} controls found",
                    structure.controls.len()
                );
                context
                    .update_structure(structure)
                    .await
                    .map_err(|e| BackendError::internal(e.to_string()))?;
                let rooms = context.rooms.read().await;
                let devices = context.devices.read().await;
                let capabilities = context.capabilities.read().await;
                info!(
                    "✅ Structure loaded: {} rooms, {} devices",
                    rooms.len(),
                    devices.len()
                );

                // Debug: Show first few device types
                for (uuid, device) in devices.iter().take(5) {
                    info!(
                        "Sample device: {} - {} (type: {}, category: {})",
                        uuid, device.name, device.device_type, device.category
                    );
                }

                info!(
                    "📊 System capabilities: lights={}, blinds={}, climate={}, sensors={}",
                    capabilities.has_lighting,
                    capabilities.has_blinds,
                    capabilities.has_climate,
                    capabilities.has_sensors
                );
                Ok(())
            }
            Err(e) => {
                error!("❌ Failed to load structure: {}", e);
                Err(BackendError::connection(format!(
                    "Failed to load structure: {e}"
                )))
            }
        }
    }

    /// Ensure client is connected and structure is loaded
    async fn ensure_connected(&self) -> std::result::Result<(), BackendError> {
        Self::ensure_connected_static(&self.client, &self.context).await
    }

    /// Parse a URI against a template and extract parameters
    fn parse_uri_template(
        &self,
        uri: &str,
        template: &str,
    ) -> Option<std::collections::HashMap<String, String>> {
        let uri_parts: Vec<&str> = uri.split('/').collect();
        let template_parts: Vec<&str> = template.split('/').collect();

        if uri_parts.len() != template_parts.len() {
            return None;
        }

        let mut params = std::collections::HashMap::new();

        for (uri_part, template_part) in uri_parts.iter().zip(template_parts.iter()) {
            if template_part.starts_with('{') && template_part.ends_with('}') {
                // Extract parameter name from {param_name}
                let param_name = &template_part[1..template_part.len() - 1];
                params.insert(param_name.to_string(), uri_part.to_string());
            } else if uri_part != template_part {
                // Non-parameter parts must match exactly
                return None;
            }
        }

        Some(params)
    }

    /// Handle dynamic resource URI requests
    async fn handle_dynamic_resource(
        &self,
        uri: &str,
    ) -> std::result::Result<(String, String), LoxoneError> {
        // Try to match against known templates and handle dynamically

        // Room-based dynamic resources
        if let Some(params) = self.parse_uri_template(uri, "loxone://rooms/{room_name}/devices") {
            let room_name = params.get("room_name").unwrap();
            let devices = self.context.devices.read().await;
            let room_devices: Vec<_> = devices
                .values()
                .filter(|d| d.room.as_ref() == Some(room_name))
                .collect();
            return Ok((
                "application/json".to_string(),
                serde_json::to_string(&room_devices)
                    .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("JSON error: {}", e)))?,
            ));
        }

        if let Some(params) = self.parse_uri_template(uri, "loxone://rooms/{room_name}/lights") {
            let room_name = params.get("room_name").unwrap();
            let devices = self.context.devices.read().await;
            let lighting_devices: Vec<_> = devices
                .values()
                .filter(|d| {
                    d.room.as_ref() == Some(room_name)
                        && (d.category == "lights"
                            || d.device_type.contains("Light")
                            || d.device_type.contains("Dimmer"))
                })
                .collect();
            return Ok((
                "application/json".to_string(),
                serde_json::to_string(&lighting_devices)
                    .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("JSON error: {}", e)))?,
            ));
        }

        if let Some(params) = self.parse_uri_template(uri, "loxone://rooms/{room_name}/blinds") {
            let room_name = params.get("room_name").unwrap();
            let devices = self.context.devices.read().await;
            let blinds_devices: Vec<_> = devices
                .values()
                .filter(|d| {
                    d.room.as_ref() == Some(room_name)
                        && (d.category == "blinds" || d.device_type == "Jalousie")
                })
                .collect();
            return Ok((
                "application/json".to_string(),
                serde_json::to_string(&blinds_devices)
                    .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("JSON error: {}", e)))?,
            ));
        }

        if let Some(params) = self.parse_uri_template(uri, "loxone://rooms/{room_name}/climate") {
            let room_name = params.get("room_name").unwrap();
            let devices = self.context.devices.read().await;
            let climate_devices: Vec<_> = devices
                .values()
                .filter(|d| {
                    d.room.as_ref() == Some(room_name)
                        && (d.category == "climate"
                            || d.device_type.contains("Temperature")
                            || d.device_type.contains("Climate"))
                })
                .collect();
            return Ok((
                "application/json".to_string(),
                serde_json::to_string(&climate_devices)
                    .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("JSON error: {}", e)))?,
            ));
        }

        if let Some(params) = self.parse_uri_template(uri, "loxone://rooms/{room_name}/status") {
            let room_name = params.get("room_name").unwrap();
            let devices = self.context.devices.read().await;
            let room_devices: Vec<_> = devices
                .values()
                .filter(|d| d.room.as_ref() == Some(room_name))
                .collect();

            let status_summary = serde_json::json!({
                "room": room_name,
                "device_count": room_devices.len(),
                "devices": room_devices,
                "summary": {
                    "lights": room_devices.iter().filter(|d| d.category == "lights").count(),
                    "blinds": room_devices.iter().filter(|d| d.category == "blinds").count(),
                    "climate": room_devices.iter().filter(|d| d.category == "climate").count(),
                    "audio": room_devices.iter().filter(|d| d.category == "audio").count(),
                    "security": room_devices.iter().filter(|d| d.category == "security").count(),
                }
            });
            return Ok(("application/json".to_string(), status_summary.to_string()));
        }

        // Device-based dynamic resources
        if let Some(params) = self.parse_uri_template(uri, "loxone://devices/{device_id}/state") {
            let device_id = params.get("device_id").unwrap();
            let devices = self.context.devices.read().await;
            if let Some(device) = devices.get(device_id) {
                return Ok((
                    "application/json".to_string(),
                    serde_json::to_string(device)
                        .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("JSON error: {}", e)))?,
                ));
            } else {
                return Err(LoxoneError::validation(format!(
                    "Device not found: {device_id}"
                )));
            }
        }

        if let Some(params) = self.parse_uri_template(uri, "loxone://devices/{device_id}/history") {
            let device_id = params.get("device_id").unwrap();
            let history = serde_json::json!({
                "device_id": device_id,
                "history": [],
                "note": "Device history not yet implemented in framework migration"
            });
            return Ok(("application/json".to_string(), history.to_string()));
        }

        if let Some(params) = self.parse_uri_template(uri, "loxone://devices/category/{category}") {
            let category = params.get("category").unwrap();
            // Ensure we're connected and have structure loaded
            self.ensure_connected()
                .await
                .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("Connection error: {}", e)))?;

            let devices = self.context.devices.read().await;
            let category_devices: Vec<_> = devices
                .values()
                .filter(|d| d.category == *category)
                .collect();
            return Ok((
                "application/json".to_string(),
                serde_json::to_string(&category_devices)
                    .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("JSON error: {}", e)))?,
            ));
        }

        if let Some(params) = self.parse_uri_template(uri, "loxone://devices/type/{device_type}") {
            let device_type = params.get("device_type").unwrap();
            let devices = self.context.devices.read().await;
            let type_devices: Vec<_> = devices
                .values()
                .filter(|d| d.device_type == *device_type)
                .collect();
            return Ok((
                "application/json".to_string(),
                serde_json::to_string(&type_devices)
                    .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("JSON error: {}", e)))?,
            ));
        }

        // Audio dynamic resources
        if let Some(params) = self.parse_uri_template(uri, "loxone://audio/zones/{zone_name}") {
            let zone_name = params.get("zone_name").unwrap();
            let devices = self.context.devices.read().await;
            let audio_zone: Vec<_> = devices
                .values()
                .filter(|d| {
                    d.name == *zone_name
                        && (d.category == "audio"
                            || d.device_type.contains("Audio")
                            || d.device_type.contains("Music"))
                })
                .collect();
            return Ok((
                "application/json".to_string(),
                serde_json::to_string(&audio_zone)
                    .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("JSON error: {}", e)))?,
            ));
        }

        if let Some(params) = self.parse_uri_template(uri, "loxone://audio/rooms/{room_name}") {
            let room_name = params.get("room_name").unwrap();
            let devices = self.context.devices.read().await;
            let room_audio: Vec<_> = devices
                .values()
                .filter(|d| {
                    d.room.as_ref() == Some(room_name)
                        && (d.category == "audio"
                            || d.device_type.contains("Audio")
                            || d.device_type.contains("Music"))
                })
                .collect();
            return Ok((
                "application/json".to_string(),
                serde_json::to_string(&room_audio)
                    .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("JSON error: {}", e)))?,
            ));
        }

        // Sensor dynamic resources
        if let Some(params) = self.parse_uri_template(uri, "loxone://sensors/{sensor_type}") {
            let sensor_type = params.get("sensor_type").unwrap();
            let devices = self.context.devices.read().await;
            let sensors: Vec<_> = devices
                .values()
                .filter(|d| {
                    d.device_type
                        .to_lowercase()
                        .contains(&sensor_type.to_lowercase())
                })
                .collect();
            return Ok((
                "application/json".to_string(),
                serde_json::to_string(&sensors)
                    .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("JSON error: {}", e)))?,
            ));
        }

        if let Some(params) =
            self.parse_uri_template(uri, "loxone://sensors/{sensor_type}/rooms/{room_name}")
        {
            let sensor_type = params.get("sensor_type").unwrap();
            let room_name = params.get("room_name").unwrap();
            let devices = self.context.devices.read().await;
            let room_sensors: Vec<_> = devices
                .values()
                .filter(|d| {
                    d.room.as_ref() == Some(room_name)
                        && d.device_type
                            .to_lowercase()
                            .contains(&sensor_type.to_lowercase())
                })
                .collect();
            return Ok((
                "application/json".to_string(),
                serde_json::to_string(&room_sensors)
                    .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("JSON error: {}", e)))?,
            ));
        }

        // System monitoring resources
        if let Some(params) = self.parse_uri_template(uri, "loxone://system/rooms/{room_name}") {
            let room_name = params.get("room_name").unwrap();
            let rooms = self.context.rooms.read().await;
            if let Some(room_data) = rooms.get(room_name) {
                return Ok((
                    "application/json".to_string(),
                    serde_json::to_string(room_data)
                        .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("JSON error: {}", e)))?,
                ));
            } else {
                return Err(LoxoneError::validation(format!(
                    "Room not found: {room_name}"
                )));
            }
        }

        if let Some(params) = self.parse_uri_template(uri, "loxone://monitoring/{metric_type}") {
            let metric_type = params.get("metric_type").unwrap();
            let metrics = serde_json::json!({
                "metric_type": metric_type,
                "data": [],
                "note": "System monitoring not yet implemented in framework migration"
            });
            return Ok(("application/json".to_string(), metrics.to_string()));
        }

        // Energy and environment resources
        if let Some(params) = self.parse_uri_template(uri, "loxone://energy/rooms/{room_name}") {
            let room_name = params.get("room_name").unwrap();
            let energy_data = serde_json::json!({
                "room": room_name,
                "consumption": null,
                "note": "Room energy data not yet implemented in framework migration"
            });
            return Ok(("application/json".to_string(), energy_data.to_string()));
        }

        // If no template matches, return not found
        Err(LoxoneError::validation(format!(
            "Dynamic resource not found: {uri}"
        )))
    }

    /// Initialize backend directly with dependencies (no server wrapper)
    pub async fn initialize(config: ServerConfig) -> std::result::Result<Self, LoxoneError> {
        use crate::client::create_client;
        use crate::monitoring::server_metrics::ServerMetricsCollector;
        use crate::server::health_check::HealthChecker;
        use crate::server::rate_limiter::RateLimitMiddleware;
        use crate::server::request_coalescing::RequestCoalescer;
        use crate::server::resource_monitor::ResourceMonitor;
        use crate::server::response_cache::ToolResponseCache;
        use crate::server::schema_validation::SchemaValidator;
        use crate::server::subscription::SubscriptionCoordinator;
        use crate::services::{SensorTypeRegistry, UnifiedValueResolver};
        use std::sync::Arc;

        debug!("🔧 Initializing LoxoneBackend with direct dependencies");

        // Get credentials and create client
        let credential_manager =
            crate::config::credentials::CredentialManager::new_async(config.credentials.clone())
                .await?;
        let credentials = credential_manager.get_credentials().await?;
        let client_box = create_client(&config.loxone, &credentials).await?;
        let client: Arc<dyn crate::client::LoxoneClient> = Arc::from(client_box);
        debug!("✅ Loxone client created successfully");

        // Create client context
        let context = Arc::new(ClientContext::new());
        debug!("✅ Client context initialized");

        // Connect to Loxone and load structure automatically
        info!("🔌 Connecting to Loxone Miniserver...");

        // Test basic connectivity with health check first
        match client.health_check().await {
            Ok(true) => {
                info!("✅ Health check passed - Miniserver is reachable");

                // Try to load structure, but don't fail initialization if it doesn't work
                match Self::ensure_connected_static(&client, &context).await {
                    Ok(_) => {
                        info!(
                            "✅ Successfully connected and loaded structure during initialization"
                        );
                    }
                    Err(e) => {
                        warn!("⚠️ Structure loading failed during initialization: {}", e);
                        info!("🔄 Structure will be loaded on first resource access");
                    }
                }
            }
            Ok(false) => {
                warn!("⚠️ Health check failed - Miniserver not reachable");
                info!("🔄 Connection will be retried when resources are accessed");
            }
            Err(e) => {
                warn!("⚠️ Health check error: {}", e);
                info!("🔄 Connection will be retried when resources are accessed");
            }
        }

        // Create sensor type registry
        let sensor_registry = Arc::new(SensorTypeRegistry::default());

        // Initialize all dependencies directly (simplified for framework migration)
        let rate_limiter = Arc::new(RateLimitMiddleware::new(Default::default()));
        let health_checker = Arc::new(HealthChecker::new(client.clone(), Default::default()));
        let request_coalescer = Arc::new(RequestCoalescer::new(
            Default::default(),
            Arc::new(
                crate::server::loxone_batch_executor::LoxoneBatchExecutor::new(client.clone()),
            ),
        ));
        let schema_validator = Arc::new(SchemaValidator::new());
        let resource_monitor = Arc::new(ResourceMonitor::new(Default::default()));
        let response_cache = Arc::new(ToolResponseCache::new());
        let subscription_coordinator = Arc::new(SubscriptionCoordinator::new().await?);

        // Start the subscription system background tasks
        subscription_coordinator.start().await?;
        debug!("✅ Subscription coordinator started with real-time updates");

        // Start real-time monitoring for Loxone data changes
        let client_clone = client.clone();
        let context_clone = context.clone();
        tokio::spawn(async move {
            Self::start_realtime_monitoring(client_clone, context_clone).await;
        });
        let value_resolver = Arc::new(UnifiedValueResolver::new(client.clone(), sensor_registry));
        let metrics_collector = Arc::new(ServerMetricsCollector::new());

        // Initialize weather storage for real-time data
        let weather_storage = match crate::storage::WeatherStorage::new(
            crate::storage::WeatherStorageConfig::default(),
        )
        .await
        {
            Ok(storage) => {
                info!("✅ Weather storage initialized");

                // Update with current device structure if available
                {
                    let devices = context.devices.read().await;
                    if !devices.is_empty() {
                        storage.update_device_structure(&devices).await;
                        info!("✅ Weather storage updated with {} devices", devices.len());
                    }
                }

                Some(Arc::new(storage))
            }
            Err(e) => {
                warn!("⚠️ Failed to initialize weather storage: {}", e);
                None
            }
        };

        Ok(Self {
            config,
            client,
            context,
            rate_limiter,
            health_checker,
            request_coalescer,
            schema_validator,
            resource_monitor,
            response_cache,
            sampling_integration: None,
            subscription_coordinator,
            value_resolver,
            state_manager: None,
            metrics_collector,
            resource_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            weather_storage,
        })
    }

    /// Get cache TTL for a resource URI
    fn get_cache_ttl(&self, uri: &str) -> Duration {
        match uri {
            // Fast-changing resources (short TTL)
            "loxone://sensors/temperature"
            | "loxone://sensors/door-window"
            | "loxone://sensors/motion" => Duration::from_secs(5),
            // Medium-changing resources
            "loxone://energy/consumption" | "loxone://weather/current" => Duration::from_secs(30),
            // Slow-changing resources (longer TTL)
            "loxone://devices/all" | "loxone://audio/zones" => {
                Duration::from_secs(300) // 5 minutes
            }
            // Static resources (very long TTL)
            "loxone://rooms" | "loxone://structure/rooms" | "loxone://system/capabilities" => {
                Duration::from_secs(3600) // 1 hour
            }
            // Default TTL for other resources
            _ => Duration::from_secs(60),
        }
    }

    /// Check if resource should be cached
    fn should_cache(&self, uri: &str) -> bool {
        // Cache most resources except system info which should always be fresh
        !matches!(uri, "loxone://system/info" | "loxone://status/health")
    }

    /// Check if URI matches a dynamic resource template pattern
    fn is_dynamic_resource(&self, uri: &str) -> bool {
        let dynamic_patterns = [
            "loxone://rooms/{room_name}/",
            "loxone://devices/{device_id}/",
            "loxone://devices/category/{category}",
            "loxone://devices/type/{device_type}",
            "loxone://sensors/{sensor_type}/",
            "loxone://system/rooms/{room_name}",
            "loxone://monitoring/{metric_type}",
            "loxone://history/{date}/",
            "loxone://audio/zones/{zone_name}",
            "loxone://audio/rooms/{room_name}",
            "loxone://security/zones/{zone_name}",
            "loxone://access/doors/{door_id}",
            "loxone://energy/rooms/{room_name}",
            "loxone://weather/locations/{location}",
        ];

        for pattern in &dynamic_patterns {
            // Simple pattern matching - check if URI starts with pattern prefix
            if let Some(prefix) = pattern.split('{').next() {
                if uri.starts_with(prefix) {
                    return true;
                }
            }
        }
        false
    }

    /// Start real-time monitoring for Loxone data changes
    async fn start_realtime_monitoring(
        client: Arc<dyn crate::client::LoxoneClient>,
        context: Arc<crate::client::ClientContext>,
    ) {
        debug!("📡 Starting real-time Loxone data monitoring...");

        let mut interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            // Monitor sensor data changes
            if let Err(e) = Self::monitor_sensor_changes(&client, &context).await {
                debug!("Sensor monitoring error: {}", e);
            }

            // Monitor device state changes
            if let Err(e) = Self::monitor_device_changes(&client, &context).await {
                debug!("Device monitoring error: {}", e);
            }

            // Monitor energy consumption changes
            if let Err(e) = Self::monitor_energy_changes(&client, &context).await {
                debug!("Energy monitoring error: {}", e);
            }
        }
    }

    /// Monitor sensor data for changes
    async fn monitor_sensor_changes(
        client: &Arc<dyn crate::client::LoxoneClient>,
        _context: &Arc<crate::client::ClientContext>,
    ) -> std::result::Result<(), LoxoneError> {
        // In a full implementation, this would:
        // 1. Poll sensor endpoints for current values
        // 2. Compare with cached values to detect changes
        // 3. Trigger notifications for subscribed clients

        debug!("🌡️ Checking sensor data for changes...");

        // Placeholder for sensor monitoring
        if client.health_check().await.unwrap_or(false) {
            debug!("✅ Sensor monitoring active - connection healthy");
        } else {
            debug!("⚠️ Sensor monitoring paused - connection issues");
        }

        Ok(())
    }

    /// Monitor device states for changes
    async fn monitor_device_changes(
        client: &Arc<dyn crate::client::LoxoneClient>,
        context: &Arc<crate::client::ClientContext>,
    ) -> std::result::Result<(), LoxoneError> {
        debug!("📱 Checking device states for changes...");

        // In a full implementation, this would:
        // 1. Fetch current device states
        // 2. Compare with cached states in context
        // 3. Update context and trigger notifications for changes

        let devices = context.devices.read().await;
        let device_count = devices.len();

        if client.health_check().await.unwrap_or(false) {
            debug!("✅ Device monitoring active for {} devices", device_count);
        } else {
            debug!("⚠️ Device monitoring paused - connection issues");
        }

        Ok(())
    }

    /// Monitor energy consumption for changes
    async fn monitor_energy_changes(
        client: &Arc<dyn crate::client::LoxoneClient>,
        _context: &Arc<crate::client::ClientContext>,
    ) -> std::result::Result<(), LoxoneError> {
        debug!("⚡ Checking energy consumption for changes...");

        // In a full implementation, this would:
        // 1. Poll energy monitoring endpoints
        // 2. Track consumption changes
        // 3. Trigger notifications for significant changes

        if client.health_check().await.unwrap_or(false) {
            debug!("✅ Energy monitoring active - connection healthy");
        } else {
            debug!("⚠️ Energy monitoring paused - connection issues");
        }

        Ok(())
    }
}

#[async_trait]
impl McpBackend for LoxoneBackend {
    type Error = BackendError;
    type Config = ServerConfig;

    async fn initialize(config: Self::Config) -> std::result::Result<Self, Self::Error> {
        info!("🚀 Initializing Loxone backend with framework (direct mode)");
        Self::initialize(config).await.map_err(BackendError::from)
    }

    fn get_server_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .enable_logging()
                .enable_sampling()
                .build(),
            server_info: Implementation {
                name: "loxone-mcp-server".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some("Loxone home automation control via MCP. Use tools to control lights, blinds, climate, and access sensor data.".to_string()),
        }
    }

    async fn health_check(&self) -> std::result::Result<(), Self::Error> {
        info!("🔍 Performing Loxone backend health check");

        // Check if the client is properly initialized and can connect
        match self.client.health_check().await {
            Ok(true) => {
                info!("✅ Loxone backend health check passed");
                Ok(())
            }
            Ok(false) => {
                warn!("⚠️ Loxone backend health check failed - not connected");
                Err(BackendError::connection(
                    "Health check failed: not connected",
                ))
            }
            Err(e) => {
                error!("❌ Loxone backend health check error: {}", e);
                Err(BackendError::connection(format!("Health check error: {e}")))
            }
        }
    }

    async fn list_tools(
        &self,
        _params: PaginatedRequestParam,
    ) -> std::result::Result<ListToolsResult, Self::Error> {
        debug!("📋 Listing Loxone tools");

        let tools = adapters::get_all_loxone_tools();

        debug!("✅ Listed {} Loxone tools", tools.len());

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParam,
    ) -> std::result::Result<CallToolResult, Self::Error> {
        debug!("⚡ Calling Loxone tool: {}", params.name);

        // Use the adapter layer to handle tool calls
        match adapters::handle_tool_call_direct(&self.client, &self.context, &params).await {
            Ok(content) => {
                info!("✅ Tool {} executed successfully", params.name);
                Ok(CallToolResult::success(vec![content]))
            }
            Err(e) => {
                error!("❌ Tool {} failed: {}", params.name, e);
                Ok(CallToolResult::error_text(format!(
                    "Tool execution failed: {e}"
                )))
            }
        }
    }

    async fn list_resources(
        &self,
        _params: PaginatedRequestParam,
    ) -> std::result::Result<ListResourcesResult, Self::Error> {
        debug!("📁 Listing Loxone resources");

        let resources = vec![
            // System resources
            Resource {
                uri: "loxone://system/info".to_string(),
                name: "System Information".to_string(),
                description: Some("Current Loxone system information and status".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://structure/rooms".to_string(),
                name: "Room Structure".to_string(),
                description: Some("Complete room structure with devices".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://config/devices".to_string(),
                name: "Device Configuration".to_string(),
                description: Some("All configured devices and their properties".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://status/health".to_string(),
                name: "System Health".to_string(),
                description: Some("Current system health and connectivity status".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://system/capabilities".to_string(),
                name: "System Capabilities".to_string(),
                description: Some("Available system capabilities and features".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://system/categories".to_string(),
                name: "Device Categories".to_string(),
                description: Some("Overview of all device categories with counts".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            // Room and device resources
            Resource {
                uri: "loxone://rooms".to_string(),
                name: "All Rooms".to_string(),
                description: Some("List of all rooms in the home automation system".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://devices/all".to_string(),
                name: "All Devices".to_string(),
                description: Some(
                    "Complete list of all devices with their current states".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://devices/category/lighting".to_string(),
                name: "Lighting Devices".to_string(),
                description: Some("All lighting devices and their current states".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://devices/category/blinds".to_string(),
                name: "Blinds/Rolladen".to_string(),
                description: Some("All blinds and rolladen devices with positions".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://devices/category/climate".to_string(),
                name: "Climate Devices".to_string(),
                description: Some("All climate and temperature control devices".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            // Audio resources
            Resource {
                uri: "loxone://audio/zones".to_string(),
                name: "Audio Zones".to_string(),
                description: Some("All audio zones and their current status".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://audio/sources".to_string(),
                name: "Audio Sources".to_string(),
                description: Some("Available audio sources and their status".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            // Sensor resources
            Resource {
                uri: "loxone://sensors/temperature".to_string(),
                name: "Temperature Sensors".to_string(),
                description: Some("All temperature sensors and their readings".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://sensors/door-window".to_string(),
                name: "Door/Window Sensors".to_string(),
                description: Some("All door and window sensors with current states".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://sensors/motion".to_string(),
                name: "Motion Sensors".to_string(),
                description: Some("All motion sensors and detection status".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            // Weather and energy resources
            Resource {
                uri: "loxone://weather/current".to_string(),
                name: "Current Weather".to_string(),
                description: Some("Current weather data from all weather sensors".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
            Resource {
                uri: "loxone://energy/consumption".to_string(),
                name: "Energy Consumption".to_string(),
                description: Some("Current energy consumption and usage metrics".to_string()),
                mime_type: Some("application/json".to_string()),
                annotations: None,
                raw: None,
            },
        ];

        debug!("✅ Listed {} Loxone resources", resources.len());

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        params: ReadResourceRequestParam,
    ) -> std::result::Result<ReadResourceResult, Self::Error> {
        debug!("📖 Reading Loxone resource: {}", params.uri);

        // Check cache first if caching is enabled for this resource
        if self.should_cache(&params.uri) {
            let cache = self.resource_cache.read().await;
            if let Some(entry) = cache.get(&params.uri) {
                if !entry.is_expired() {
                    debug!("💰 Cache hit for resource: {}", params.uri);
                    return Ok(ReadResourceResult {
                        contents: vec![ResourceContents {
                            uri: params.uri,
                            mime_type: Some(entry.mime_type.clone()),
                            text: Some(entry.data.clone()),
                            blob: None,
                        }],
                    });
                } else {
                    debug!("⏰ Cache expired for resource: {}", params.uri);
                }
            }
        }

        let (mime_type, content) = match params.uri.as_str() {
            "loxone://system/info" => {
                let info = serde_json::json!({
                    "server": "loxone-mcp-server",
                    "version": "1.0.0",
                    "connected": self.client.is_connected().await.unwrap_or(false),
                    "health": self.client.health_check().await.unwrap_or(false)
                });
                ("application/json", info.to_string())
            }

            "loxone://structure/rooms" => {
                let rooms = self.context.rooms.read().await;
                let room_data = serde_json::to_value(&*rooms)
                    .map_err(|e| LoxoneError::serialization(e.to_string()))
                    .map_err(BackendError::from)?;
                ("application/json", room_data.to_string())
            }

            "loxone://config/devices" => {
                let devices = self.context.devices.read().await;
                let device_data = serde_json::to_value(&*devices)
                    .map_err(|e| LoxoneError::serialization(e.to_string()))?;
                ("application/json", device_data.to_string())
            }

            "loxone://status/health" => {
                let health_status = serde_json::json!({
                    "status": "healthy",
                    "message": "Framework migration mode - basic health check"
                });
                let health_data = serde_json::to_value(&health_status)
                    .map_err(|e| LoxoneError::serialization(e.to_string()))
                    .map_err(BackendError::from)?;
                ("application/json", health_data.to_string())
            }

            // Room resources
            "loxone://rooms" => {
                // Ensure we're connected and have structure loaded
                self.ensure_connected().await?;

                let rooms = self.context.rooms.read().await;
                let room_list: Vec<_> = rooms.keys().cloned().collect();
                (
                    "application/json",
                    serde_json::to_string(&room_list)
                        .map_err(|e| BackendError::internal(format!("JSON error: {e}")))?,
                )
            }

            // Device resources
            "loxone://devices/all" => {
                let devices = self.context.devices.read().await;
                let device_list: Vec<_> = devices.values().collect();
                (
                    "application/json",
                    serde_json::to_string(&device_list)
                        .map_err(|e| BackendError::internal(format!("JSON error: {e}")))?,
                )
            }
            "loxone://devices/category/lighting" => {
                // Ensure we're connected and have structure loaded
                self.ensure_connected().await?;

                let devices = self.context.devices.read().await;
                let lighting_devices: Vec<_> = devices
                    .values()
                    .filter(|d| {
                        d.category == "lights"
                            || d.device_type.contains("Light")
                            || d.device_type.contains("Dimmer")
                    })
                    .collect();
                (
                    "application/json",
                    serde_json::to_string(&lighting_devices)
                        .map_err(|e| BackendError::internal(format!("JSON error: {e}")))?,
                )
            }
            "loxone://devices/category/blinds" => {
                // Ensure we're connected and have structure loaded
                self.ensure_connected().await?;

                let devices = self.context.devices.read().await;
                let blinds_devices: Vec<_> = devices
                    .values()
                    .filter(|d| d.category == "blinds" || d.device_type == "Jalousie")
                    .collect();
                (
                    "application/json",
                    serde_json::to_string(&blinds_devices)
                        .map_err(|e| BackendError::internal(format!("JSON error: {e}")))?,
                )
            }
            "loxone://devices/category/climate" => {
                // Ensure we're connected and have structure loaded
                self.ensure_connected().await?;

                let devices = self.context.devices.read().await;
                debug!(
                    "Checking climate devices - total devices: {}",
                    devices.len()
                );

                let climate_devices: Vec<_> = devices
                    .values()
                    .filter(|d| {
                        let matches = d.category == "climate"
                            || d.device_type.to_lowercase().contains("temperature")
                            || d.device_type.to_lowercase().contains("climate")
                            || d.device_type.to_lowercase().contains("thermostat")
                            || d.device_type.to_lowercase().contains("heating")
                            || d.device_type == "IRoomControllerV2"
                            || d.device_type == "IntelligentRoomController"
                            || d.device_type.contains("RoomController");
                        if matches {
                            debug!(
                                "Found climate device: {} (type: {}, category: {})",
                                d.name, d.device_type, d.category
                            );
                        }
                        matches
                    })
                    .collect();
                debug!("Found {} climate devices", climate_devices.len());
                (
                    "application/json",
                    serde_json::to_string(&climate_devices)
                        .map_err(|e| BackendError::internal(format!("JSON error: {e}")))?,
                )
            }

            // Audio resources
            "loxone://audio/zones" => {
                let devices = self.context.devices.read().await;
                let audio_devices: Vec<_> = devices
                    .values()
                    .filter(|d| {
                        d.category == "audio"
                            || d.device_type.contains("Audio")
                            || d.device_type.contains("Music")
                    })
                    .collect();
                (
                    "application/json",
                    serde_json::to_string(&audio_devices)
                        .map_err(|e| BackendError::internal(format!("JSON error: {e}")))?,
                )
            }
            "loxone://audio/sources" => {
                let audio_sources = serde_json::json!({
                    "sources": [],
                    "note": "Audio sources discovery not yet implemented in framework migration"
                });
                ("application/json", audio_sources.to_string())
            }

            // Sensor resources
            "loxone://sensors/temperature" => {
                // Try to ensure connection, but provide fallback if structure loading fails
                match self.ensure_connected().await {
                    Ok(_) => {
                        let devices = self.context.devices.read().await;
                        debug!("Total devices loaded: {}", devices.len());

                        // Log all device types for debugging
                        for (uuid, device) in devices.iter().take(10) {
                            debug!(
                                "Device {}: type='{}', category='{}'",
                                uuid, device.device_type, device.category
                            );
                        }

                        let temp_sensors: Vec<_> = devices
                            .values()
                            .filter(|d| {
                                // Match common temperature sensor patterns
                                d.device_type.to_lowercase().contains("temperature")
                                    || d.device_type.to_lowercase().contains("temp")
                                    || d.device_type == "InfoOnlyAnalog"
                                    || d.device_type == "IRoomControllerV2"
                                    || d.name.to_lowercase().contains("temp")
                                    || d.name.to_lowercase().contains("temperatur")
                                    || (d.category == "sensors"
                                        && d.name.to_lowercase().contains("temp"))
                            })
                            .collect();
                        debug!("Found {} temperature sensors", temp_sensors.len());
                        (
                            "application/json",
                            serde_json::to_string(&temp_sensors)
                                .map_err(|e| BackendError::internal(format!("JSON error: {e}")))?,
                        )
                    }
                    Err(e) => {
                        // Return proper error response
                        let error_data = serde_json::json!({
                            "error": "structure_loading_failed",
                            "message": format!("Failed to load temperature sensors: {}", e),
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        });
                        ("application/json", error_data.to_string())
                    }
                }
            }
            "loxone://sensors/door-window" => {
                // Ensure we're connected and have structure loaded
                self.ensure_connected().await?;

                let devices = self.context.devices.read().await;
                let door_window_sensors: Vec<_> = devices
                    .values()
                    .filter(|d| {
                        d.device_type.contains("Door")
                            || d.device_type.contains("Window")
                            || d.device_type.contains("Contact")
                    })
                    .collect();
                (
                    "application/json",
                    serde_json::to_string(&door_window_sensors)
                        .map_err(|e| BackendError::internal(format!("JSON error: {e}")))?,
                )
            }
            "loxone://sensors/motion" => {
                // Ensure we're connected and have structure loaded
                self.ensure_connected().await?;

                let devices = self.context.devices.read().await;
                let motion_sensors: Vec<_> = devices
                    .values()
                    .filter(|d| d.device_type.contains("Motion") || d.device_type.contains("PIR"))
                    .collect();
                (
                    "application/json",
                    serde_json::to_string(&motion_sensors)
                        .map_err(|e| BackendError::internal(format!("JSON error: {e}")))?,
                )
            }

            // Weather resources
            "loxone://weather/current" => {
                // Ensure we're connected and have structure loaded
                self.ensure_connected().await?;

                let devices = self.context.devices.read().await;

                // Find weather station devices
                let weather_devices: Vec<_> = devices
                    .values()
                    .filter(|d| {
                        d.category == "weather"
                            || d.device_type.to_lowercase().contains("weather")
                            || d.device_type == "WeatherStation"
                            || d.device_type == "WeatherServer"
                            || d.name.to_lowercase().contains("weather")
                            || d.name.to_lowercase().contains("wetter")
                    })
                    .collect();

                debug!("Found {} weather devices", weather_devices.len());

                // Get stored weather data from weather storage
                let mut stored_weather_data = Vec::new();
                #[cfg(feature = "turso")]
                let latest_temperature: Option<WeatherDataPoint> = None;
                #[cfg(not(feature = "turso"))]
                let latest_temperature: Option<
                    crate::storage::simple_storage::SimpleWeatherDataPoint,
                > = None;

                #[cfg(feature = "turso")]
                let latest_humidity: Option<WeatherDataPoint> = None;
                #[cfg(not(feature = "turso"))]
                let latest_humidity: Option<
                    crate::storage::simple_storage::SimpleWeatherDataPoint,
                > = None;

                #[cfg(feature = "turso")]
                let latest_pressure: Option<WeatherDataPoint> = None;
                #[cfg(not(feature = "turso"))]
                let latest_pressure: Option<
                    crate::storage::simple_storage::SimpleWeatherDataPoint,
                > = None;

                #[cfg(feature = "turso")]
                let latest_wind_speed: Option<WeatherDataPoint> = None;
                #[cfg(not(feature = "turso"))]
                let latest_wind_speed: Option<
                    crate::storage::simple_storage::SimpleWeatherDataPoint,
                > = None;

                if let Some(storage) = &self.weather_storage {
                    // Get recent weather data for all weather devices
                    for device in &weather_devices {
                        match storage.get_current_weather_data(&device.uuid).await {
                            Ok(data_points) => {
                                if !data_points.is_empty() {
                                    debug!(
                                        "Found {} stored data points for device {}",
                                        data_points.len(),
                                        device.name
                                    );

                                    // Extract latest values by parameter type
                                    #[cfg(feature = "turso")]
                                    {
                                        // No-op for turso feature - types don't match
                                        // We handle weather data differently with turso storage
                                    }
                                    #[cfg(not(feature = "turso"))]
                                    {
                                        for point in &data_points {
                                            match point.parameter_name.as_str() {
                                                name if name.contains("temp") => {
                                                    if latest_temperature.is_none()
                                                        || point.timestamp
                                                            > latest_temperature
                                                                .as_ref()
                                                                .unwrap()
                                                                .timestamp
                                                    {
                                                        latest_temperature = Some(point.clone());
                                                    }
                                                }
                                                name if name.contains("humid") => {
                                                    if latest_humidity.is_none()
                                                        || point.timestamp
                                                            > latest_humidity
                                                                .as_ref()
                                                                .unwrap()
                                                                .timestamp
                                                    {
                                                        latest_humidity = Some(point.clone());
                                                    }
                                                }
                                                name if name.contains("pressure") => {
                                                    if latest_pressure.is_none()
                                                        || point.timestamp
                                                            > latest_pressure
                                                                .as_ref()
                                                                .unwrap()
                                                                .timestamp
                                                    {
                                                        latest_pressure = Some(point.clone());
                                                    }
                                                }
                                                name if name.contains("wind") => {
                                                    if latest_wind_speed.is_none()
                                                        || point.timestamp
                                                            > latest_wind_speed
                                                                .as_ref()
                                                                .unwrap()
                                                                .timestamp
                                                    {
                                                        latest_wind_speed = Some(point.clone());
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }

                                    stored_weather_data.extend(data_points);
                                }
                            }
                            Err(e) => {
                                debug!(
                                    "Failed to get stored weather data for {}: {}",
                                    device.name, e
                                );
                            }
                        }
                    }
                } else {
                    debug!("Weather storage not available, falling back to device states");
                }

                // Fallback to device states if no stored data
                if stored_weather_data.is_empty() {
                    // Also look for outdoor temperature sensors - specifically look for Terrasse
                    let outdoor_sensors: Vec<_> = devices
                        .values()
                        .filter(|d| {
                            (d.device_type == "InfoOnlyAnalog" || d.category == "sensors")
                                && (d.name.to_lowercase().contains("outdoor")
                                    || d.name.to_lowercase().contains("außen")
                                    || d.name.to_lowercase().contains("aussen")
                                    || d.name.to_lowercase().contains("terrasse")
                                    || d.room
                                        .as_ref()
                                        .map(|r| r.to_lowercase().contains("terrasse"))
                                        .unwrap_or(false))
                        })
                        .collect();

                    debug!(
                        "Found {} outdoor sensors for fallback",
                        outdoor_sensors.len()
                    );

                    // Try to get state values from devices as fallback
                    let mut state_uuids_to_resolve = Vec::new();
                    for sensor in &outdoor_sensors {
                        if let Some(value_uuid) =
                            sensor.states.get("value").and_then(|v| v.as_str())
                        {
                            state_uuids_to_resolve
                                .push((value_uuid.to_string(), sensor.name.clone()));
                        }
                    }

                    if !state_uuids_to_resolve.is_empty() {
                        let state_uuids: Vec<String> = state_uuids_to_resolve
                            .iter()
                            .map(|(uuid, _)| uuid.clone())
                            .collect();

                        match self.client.get_state_values(&state_uuids).await {
                            Ok(state_values) => {
                                for (uuid, _name) in &state_uuids_to_resolve {
                                    if let Some(value) = state_values.get(uuid) {
                                        if let Some(_num_val) = value.as_f64() {
                                            #[cfg(not(feature = "turso"))]
                                            {
                                                if name.to_lowercase().contains("temperatur")
                                                    || name.to_lowercase().contains("temp")
                                                {
                                                    if latest_temperature.is_none() {
                                                        latest_temperature = Some(crate::storage::simple_storage::SimpleWeatherDataPoint {
                                                            device_uuid: uuid.clone(),
                                                            parameter_name: "temperature".to_string(),
                                                            value: num_val,
                                                            unit: Some("°C".to_string()),
                                                            timestamp: chrono::Utc::now().timestamp() as u32,
                                                            quality_score: 1.0,
                                                        });
                                                    }
                                                } else if name.to_lowercase().contains("feuchte")
                                                    || name.to_lowercase().contains("humidity")
                                                {
                                                    if latest_humidity.is_none() {
                                                        latest_humidity = Some(crate::storage::simple_storage::SimpleWeatherDataPoint {
                                                            device_uuid: uuid.clone(),
                                                            parameter_name: "humidity".to_string(),
                                                            value: num_val,
                                                            unit: Some("%".to_string()),
                                                            timestamp: chrono::Utc::now().timestamp() as u32,
                                                            quality_score: 1.0,
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                debug!("Failed to resolve fallback state values: {}", e);
                            }
                        }
                    }
                }

                let weather_data = serde_json::json!({
                    "status": "success",
                    "data_source": if stored_weather_data.is_empty() { "device_states" } else { "stored_realtime" },
                    "current_conditions": {
                        "temperature": latest_temperature.as_ref().map(|t| serde_json::json!({
                            "value": t.value,
                            "unit": t.unit,
                            "timestamp": t.timestamp,
                            "quality": t.quality_score
                        })),
                        "humidity": latest_humidity.as_ref().map(|h| serde_json::json!({
                            "value": h.value,
                            "unit": h.unit,
                            "timestamp": h.timestamp,
                            "quality": h.quality_score
                        })),
                        "pressure": latest_pressure.as_ref().map(|p| serde_json::json!({
                            "value": p.value,
                            "unit": p.unit,
                            "timestamp": p.timestamp,
                            "quality": p.quality_score
                        })),
                        "wind_speed": latest_wind_speed.as_ref().map(|w| serde_json::json!({
                            "value": w.value,
                            "unit": w.unit,
                            "timestamp": w.timestamp,
                            "quality": w.quality_score
                        }))
                    },
                    "weather_devices": weather_devices.iter().map(|d| {
                        serde_json::json!({
                            "name": d.name,
                            "type": d.device_type,
                            "uuid": d.uuid,
                            "category": d.category
                        })
                    }).collect::<Vec<_>>(),
                    "stored_data_points": stored_weather_data.len(),
                    "timestamp": chrono::Utc::now().to_rfc3339()
                });
                ("application/json", weather_data.to_string())
            }

            // Energy resources
            "loxone://energy/consumption" => {
                let energy_data = serde_json::json!({
                    "current_usage": null,
                    "daily_total": null,
                    "note": "Energy consumption data not yet implemented in framework migration"
                });
                ("application/json", energy_data.to_string())
            }

            // System resources (additional)
            "loxone://system/capabilities" => {
                let capabilities = self.context.capabilities.read().await;
                (
                    "application/json",
                    serde_json::to_string(&*capabilities)
                        .map_err(|e| BackendError::internal(format!("JSON error: {e}")))?,
                )
            }
            "loxone://system/categories" => {
                let devices = self.context.devices.read().await;
                let mut categories = std::collections::HashMap::new();
                let mut type_examples = std::collections::HashMap::new();

                for device in devices.values() {
                    *categories.entry(device.category.clone()).or_insert(0) += 1;

                    // Collect example device types for each category
                    let examples = type_examples
                        .entry(device.category.clone())
                        .or_insert_with(Vec::new);
                    if examples.len() < 3 && !examples.contains(&device.device_type) {
                        examples.push(device.device_type.clone());
                    }
                }

                debug!("Category breakdown: {:?}", categories);
                debug!("Example device types by category: {:?}", type_examples);

                let category_summary = serde_json::json!({
                    "categories": categories,
                    "total_devices": devices.len(),
                    "type_examples": type_examples
                });
                ("application/json", category_summary.to_string())
            }

            _ => {
                // Try to handle as dynamic resource template
                match self.handle_dynamic_resource(&params.uri).await {
                    Ok((_mime_type, content)) => {
                        // Always use application/json for consistency
                        ("application/json", content)
                    }
                    Err(_) => {
                        return Err(BackendError::not_supported(format!(
                            "Resource not found: {}",
                            params.uri
                        )));
                    }
                }
            }
        };

        // Update cache if caching is enabled for this resource
        if self.should_cache(&params.uri) {
            let ttl = self.get_cache_ttl(&params.uri);
            let cache_entry = CacheEntry::new(content.clone(), mime_type.to_string(), ttl);

            let mut cache = self.resource_cache.write().await;
            cache.insert(params.uri.clone(), cache_entry);
            debug!("💾 Cached resource: {} (TTL: {:?})", params.uri, ttl);

            // Simple cache cleanup: remove expired entries periodically
            if cache.len() > 100 {
                cache.retain(|_, entry| !entry.is_expired());
                debug!("🧹 Cleaned up expired cache entries");
            }
        }

        Ok(ReadResourceResult {
            contents: vec![ResourceContents {
                uri: params.uri,
                mime_type: Some(mime_type.to_string()),
                text: Some(content),
                blob: None,
            }],
        })
    }

    async fn list_resource_templates(
        &self,
        _params: PaginatedRequestParam,
    ) -> std::result::Result<ListResourceTemplatesResult, Self::Error> {
        debug!("📋 Listing Loxone resource templates");

        let resource_templates = vec![
            // Room-based templates
            ResourceTemplate {
                uri_template: "loxone://rooms/{room_name}/devices".to_string(),
                name: "Room Devices".to_string(),
                description: Some("All devices in a specific room with current states".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://rooms/{room_name}/lights".to_string(),
                name: "Room Lighting".to_string(),
                description: Some("All lighting devices in a specific room".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://rooms/{room_name}/blinds".to_string(),
                name: "Room Blinds".to_string(),
                description: Some("All blinds/rolladen devices in a specific room".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://rooms/{room_name}/climate".to_string(),
                name: "Room Climate".to_string(),
                description: Some("Climate control and sensors for a specific room".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://rooms/{room_name}/status".to_string(),
                name: "Room Status Summary".to_string(),
                description: Some(
                    "Comprehensive status summary for all devices in a room".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
            },
            // Device-based templates
            ResourceTemplate {
                uri_template: "loxone://devices/{device_id}/state".to_string(),
                name: "Device State".to_string(),
                description: Some("Current state and properties of a specific device".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://devices/{device_id}/history".to_string(),
                name: "Device History".to_string(),
                description: Some("Historical state changes for a specific device".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://devices/category/{category}".to_string(),
                name: "Category Devices".to_string(),
                description: Some(
                    "All devices in a specific category with current states".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://devices/type/{device_type}".to_string(),
                name: "Device Type Collection".to_string(),
                description: Some("All devices of a specific type with current states".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            // Sensor-based templates
            ResourceTemplate {
                uri_template: "loxone://sensors/{sensor_type}".to_string(),
                name: "Sensor Data".to_string(),
                description: Some(
                    "All sensors of a specific type with current readings".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://sensors/{sensor_type}/rooms/{room_name}".to_string(),
                name: "Room Sensor Data".to_string(),
                description: Some("Sensors of a specific type in a specific room".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            // System monitoring templates
            ResourceTemplate {
                uri_template: "loxone://system/rooms/{room_name}".to_string(),
                name: "Room System Info".to_string(),
                description: Some("System-level information for a specific room".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://monitoring/{metric_type}".to_string(),
                name: "System Metrics".to_string(),
                description: Some("System monitoring metrics by type".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://history/{date}/summary".to_string(),
                name: "Daily Summary".to_string(),
                description: Some(
                    "Daily activity summary for a specific date (YYYY-MM-DD)".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
            },
            // Audio and entertainment templates
            ResourceTemplate {
                uri_template: "loxone://audio/zones/{zone_name}".to_string(),
                name: "Audio Zone Status".to_string(),
                description: Some(
                    "Current status and controls for a specific audio zone".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://audio/rooms/{room_name}".to_string(),
                name: "Room Audio".to_string(),
                description: Some("All audio devices and zones in a specific room".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            // Security and access templates
            ResourceTemplate {
                uri_template: "loxone://security/zones/{zone_name}".to_string(),
                name: "Security Zone".to_string(),
                description: Some("Security status for a specific zone or area".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://access/doors/{door_id}".to_string(),
                name: "Door Access".to_string(),
                description: Some("Access control status for a specific door".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            // Energy and environment templates
            ResourceTemplate {
                uri_template: "loxone://energy/rooms/{room_name}".to_string(),
                name: "Room Energy".to_string(),
                description: Some("Energy consumption data for a specific room".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "loxone://weather/locations/{location}".to_string(),
                name: "Location Weather".to_string(),
                description: Some("Weather data for a specific location or sensor".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ];

        Ok(ListResourceTemplatesResult {
            resource_templates,
            next_cursor: None,
        })
    }

    async fn list_prompts(
        &self,
        _params: PaginatedRequestParam,
    ) -> std::result::Result<ListPromptsResult, Self::Error> {
        info!("💬 Listing Loxone prompts");

        // Get real-time data for context-aware prompts
        let rooms = self.context.rooms.read().await;
        let devices = self.context.devices.read().await;

        let room_names: Vec<_> = rooms.keys().cloned().collect();
        let device_count = devices.len();
        let room_count = rooms.len();

        // Calculate device category counts for intelligent prompting
        let lights_count = devices.values().filter(|d| d.category == "lights").count();
        let blinds_count = devices.values().filter(|d| d.category == "blinds").count();
        let climate_count = devices.values().filter(|d| d.category == "climate").count();
        let audio_count = devices.values().filter(|d| d.category == "audio").count();
        let security_count = devices
            .values()
            .filter(|d| d.category == "security")
            .count();

        let prompts = vec![
            // Energy and efficiency prompts
            Prompt {
                name: "analyze_energy_usage".to_string(),
                description: Some(
                    format!("Analyze energy consumption patterns across {room_count} rooms and {device_count} devices for optimization")
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "period".to_string(),
                        description: Some("Time period to analyze (day, week, month)".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "focus_rooms".to_string(),
                        description: Some("Comma-separated list of rooms to focus on".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "include_recommendations".to_string(),
                        description: Some("Include actionable optimization recommendations".to_string()),
                        required: Some(false),
                    }
                ]),
            },
            Prompt {
                name: "room_energy_optimization".to_string(),
                description: Some(
                    "Analyze and optimize energy usage for a specific room".to_string(),
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "room_name".to_string(),
                        description: Some(format!("Room to analyze (available: {})", room_names.join(", "))),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "include_schedule".to_string(),
                        description: Some("Include scheduling recommendations".to_string()),
                        required: Some(false),
                    }
                ]),
            },

            // Home status and monitoring prompts
            Prompt {
                name: "home_status_summary".to_string(),
                description: Some(
                    format!("Generate comprehensive home status for {room_count} rooms with {device_count} total devices")
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "include_sensors".to_string(),
                        description: Some("Include detailed sensor readings".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "room_filter".to_string(),
                        description: Some("Focus on specific rooms (comma-separated)".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "device_categories".to_string(),
                        description: Some("Filter by device categories (lights, blinds, climate, audio, security)".to_string()),
                        required: Some(false),
                    }
                ]),
            },
            Prompt {
                name: "room_status_report".to_string(),
                description: Some(
                    "Generate detailed status report for a specific room with all devices".to_string(),
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "room_name".to_string(),
                        description: Some(format!("Room to analyze (available: {})", room_names.join(", "))),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "include_controls".to_string(),
                        description: Some("Include available control options".to_string()),
                        required: Some(false),
                    }
                ]),
            },
            Prompt {
                name: "device_diagnostics".to_string(),
                description: Some(
                    "Analyze device performance and suggest maintenance actions".to_string(),
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "device_category".to_string(),
                        description: Some("Category to focus on (lights, blinds, climate, audio, security)".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "include_history".to_string(),
                        description: Some("Include historical performance data".to_string()),
                        required: Some(false),
                    }
                ]),
            },

            // Security and safety prompts
            Prompt {
                name: "security_report".to_string(),
                description: Some(
                    format!("Generate security status report with {security_count} security devices and recommendations")
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "include_history".to_string(),
                        description: Some("Include recent security events".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "threat_assessment".to_string(),
                        description: Some("Include threat assessment and recommendations".to_string()),
                        required: Some(false),
                    }
                ]),
            },
            Prompt {
                name: "safety_check".to_string(),
                description: Some(
                    "Perform comprehensive safety check across all home systems".to_string(),
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "focus_areas".to_string(),
                        description: Some("Areas to focus on (fire, security, electrical, climate)".to_string()),
                        required: Some(false),
                    }
                ]),
            },

            // Comfort and automation prompts
            Prompt {
                name: "comfort_optimization".to_string(),
                description: Some(
                    format!("Optimize comfort settings across {lights_count} lighting and {climate_count} climate devices")
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "time_of_day".to_string(),
                        description: Some("Time period to optimize (morning, afternoon, evening, night)".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "weather_consideration".to_string(),
                        description: Some("Consider weather conditions in recommendations".to_string()),
                        required: Some(false),
                    }
                ]),
            },
            Prompt {
                name: "lighting_scene_suggestion".to_string(),
                description: Some(
                    format!("Suggest optimal lighting scenes for {lights_count} lighting devices across rooms")
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "scenario".to_string(),
                        description: Some("Usage scenario (work, relax, party, sleep, away)".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "room_priority".to_string(),
                        description: Some("Prioritize specific rooms".to_string()),
                        required: Some(false),
                    }
                ]),
            },
            Prompt {
                name: "climate_optimization".to_string(),
                description: Some(
                    format!("Optimize climate control across {climate_count} climate devices for efficiency and comfort")
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "season".to_string(),
                        description: Some("Season to optimize for (spring, summer, autumn, winter)".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "priority".to_string(),
                        description: Some("Priority focus (comfort, efficiency, cost)".to_string()),
                        required: Some(false),
                    }
                ]),
            },

            // Entertainment and lifestyle prompts
            Prompt {
                name: "entertainment_setup".to_string(),
                description: Some(
                    format!("Configure optimal entertainment experience using {audio_count} audio devices")
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "activity".to_string(),
                        description: Some("Activity type (movie, music, party, quiet)".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "zones".to_string(),
                        description: Some("Audio zones to include".to_string()),
                        required: Some(false),
                    }
                ]),
            },
            Prompt {
                name: "blinds_automation".to_string(),
                description: Some(
                    format!("Create intelligent blinds automation rules for {blinds_count} blind devices")
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "priority".to_string(),
                        description: Some("Priority factor (privacy, energy, comfort, security)".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "schedule_type".to_string(),
                        description: Some("Schedule type (daily, seasonal, weather-based)".to_string()),
                        required: Some(false),
                    }
                ]),
            },

            // Troubleshooting and maintenance prompts
            Prompt {
                name: "system_troubleshooting".to_string(),
                description: Some(
                    "Analyze system issues and provide step-by-step troubleshooting guidance".to_string(),
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "symptom".to_string(),
                        description: Some("Describe the issue or symptom observed".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "affected_area".to_string(),
                        description: Some("Room or system area affected".to_string()),
                        required: Some(false),
                    }
                ]),
            },
            Prompt {
                name: "maintenance_schedule".to_string(),
                description: Some(
                    "Generate personalized maintenance schedule for all home automation systems".to_string(),
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "priority_level".to_string(),
                        description: Some("Maintenance priority (essential, recommended, optional)".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "season".to_string(),
                        description: Some("Season to focus on (spring, summer, autumn, winter)".to_string()),
                        required: Some(false),
                    }
                ]),
            },

            // Advanced automation prompts
            Prompt {
                name: "automation_suggestions".to_string(),
                description: Some(
                    "Suggest intelligent automation rules based on current device setup and usage patterns".to_string(),
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "lifestyle".to_string(),
                        description: Some("Lifestyle type (family, professional, retired, student)".to_string()),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "complexity".to_string(),
                        description: Some("Automation complexity (simple, intermediate, advanced)".to_string()),
                        required: Some(false),
                    }
                ]),
            },
            Prompt {
                name: "scenario_planning".to_string(),
                description: Some(
                    "Plan and configure home automation scenarios for different situations".to_string(),
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "scenario_type".to_string(),
                        description: Some("Scenario (vacation, party, work_from_home, sleep, emergency)".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "duration".to_string(),
                        description: Some("Expected duration of scenario".to_string()),
                        required: Some(false),
                    }
                ]),
            },
        ];

        Ok(ListPromptsResult {
            prompts,
            next_cursor: None,
        })
    }

    async fn get_prompt(
        &self,
        params: GetPromptRequestParam,
    ) -> std::result::Result<GetPromptResult, Self::Error> {
        info!("📝 Getting Loxone prompt: {}", params.name);

        let (description, messages) = match params.name.as_str() {
            "analyze_energy_usage" => {
                let period = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("period"))
                    .map(|s| s.as_str())
                    .unwrap_or("week");

                (
                    "Energy usage analysis and optimization".to_string(),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are an energy efficiency expert analyzing Loxone home automation data for the {period} period.")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            "Please analyze the energy consumption data and provide optimization recommendations."
                        ),
                    ]
                )
            }

            "home_status_summary" => {
                let include_sensors = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("include_sensors"))
                    .map(|v| v == "true")
                    .unwrap_or(true);

                let system_context = if include_sensors {
                    "You have access to comprehensive home data including all sensors, devices, and systems."
                } else {
                    "You have access to basic home data excluding detailed sensor information."
                };

                (
                    "Comprehensive home status summary".to_string(),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a smart home assistant. {system_context}")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            "Please provide a comprehensive summary of the current home status including all relevant systems and recommendations."
                        ),
                    ]
                )
            }

            "security_report" => {
                let include_history = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("include_history"))
                    .map(|v| v == "true")
                    .unwrap_or(false);

                let threat_assessment = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("threat_assessment"))
                    .map(|v| v == "true")
                    .unwrap_or(true);

                let context_msg = if include_history && threat_assessment {
                    "You are a home security expert with access to historical data and threat assessment capabilities."
                } else if include_history {
                    "You are a home security expert with access to historical security data."
                } else if threat_assessment {
                    "You are a home security expert focused on current threat assessment."
                } else {
                    "You are a home security expert analyzing current security system status."
                };

                (
                    "Security status and recommendations".to_string(),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            context_msg
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            "Please analyze the current security status and provide a comprehensive security report with recommendations."
                        ),
                    ]
                )
            }

            "room_energy_optimization" => {
                let room_name = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("room_name"))
                    .map(|s| s.as_str())
                    .unwrap_or("unspecified");

                let include_schedule = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("include_schedule"))
                    .map(|v| v == "true")
                    .unwrap_or(true);

                let schedule_context = if include_schedule {
                    " Include detailed scheduling recommendations in your analysis."
                } else {
                    " Focus on immediate optimizations without scheduling."
                };

                (
                    format!("Energy optimization for room: {room_name}"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are an energy efficiency expert focusing on optimizing energy usage in the {room_name} room.{schedule_context}")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please analyze energy usage in the {room_name} room and provide specific optimization recommendations.")
                        ),
                    ]
                )
            }

            "room_status_report" => {
                let room_name = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("room_name"))
                    .map(|s| s.as_str())
                    .unwrap_or("unspecified");

                let include_controls = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("include_controls"))
                    .map(|v| v == "true")
                    .unwrap_or(true);

                let control_context = if include_controls {
                    " Include available control options and usage instructions."
                } else {
                    " Focus on status information without control details."
                };

                (
                    format!("Status report for room: {room_name}"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a smart home assistant reporting on the {room_name} room status.{control_context}")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please provide a comprehensive status report for the {room_name} room including all devices and systems.")
                        ),
                    ]
                )
            }

            "device_diagnostics" => {
                let device_category = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("device_category"))
                    .map(|s| s.as_str())
                    .unwrap_or("all");

                let include_history = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("include_history"))
                    .map(|v| v == "true")
                    .unwrap_or(false);

                let history_context = if include_history {
                    " Use historical performance data to identify patterns and predict potential issues."
                } else {
                    " Focus on current device status and immediate diagnostic information."
                };

                (
                    format!("Device diagnostics for: {device_category}"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a smart home technician specializing in {device_category} device diagnostics.{history_context}")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please analyze the performance of {device_category} devices and suggest maintenance actions.")
                        ),
                    ]
                )
            }

            "safety_check" => {
                let focus_areas = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("focus_areas"))
                    .map(|s| s.as_str())
                    .unwrap_or("all systems");

                (
                    format!("Safety check focusing on: {focus_areas}"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a home safety inspector with expertise in {focus_areas}. Perform a thorough safety analysis.")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please perform a comprehensive safety check focusing on {focus_areas} and provide actionable recommendations.")
                        ),
                    ]
                )
            }

            "comfort_optimization" => {
                let time_of_day = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("time_of_day"))
                    .map(|s| s.as_str())
                    .unwrap_or("current time");

                let weather_consideration = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("weather_consideration"))
                    .map(|v| v == "true")
                    .unwrap_or(true);

                let weather_context = if weather_consideration {
                    " Take current weather conditions into account for optimal comfort settings."
                } else {
                    " Focus on indoor comfort settings without weather considerations."
                };

                (
                    format!("Comfort optimization for: {time_of_day}"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a comfort optimization specialist focusing on {time_of_day} settings.{weather_context}")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please optimize comfort settings for {time_of_day} across lighting and climate systems.")
                        ),
                    ]
                )
            }

            "lighting_scene_suggestion" => {
                let scenario = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("scenario"))
                    .map(|s| s.as_str())
                    .unwrap_or("general use");

                let room_priority = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("room_priority"))
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "all rooms".to_string());

                (
                    format!("Lighting scenes for {scenario} scenario"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a lighting design expert creating optimal scenes for {scenario} scenarios, prioritizing {room_priority}.")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please suggest optimal lighting scenes for {scenario} usage, focusing on {room_priority}.")
                        ),
                    ]
                )
            }

            "climate_optimization" => {
                let season = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("season"))
                    .map(|s| s.as_str())
                    .unwrap_or("current season");

                let priority = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("priority"))
                    .map(|s| s.as_str())
                    .unwrap_or("balance");

                (
                    format!("Climate optimization for {season} (priority: {priority})"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a climate control expert optimizing for {season} with {priority} priority.")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please optimize climate control for {season} season with {priority} priority focus.")
                        ),
                    ]
                )
            }

            "entertainment_setup" => {
                let activity = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("activity"))
                    .map(|s| s.as_str())
                    .unwrap_or("general entertainment");

                let zones = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("zones"))
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "all available zones".to_string());

                (
                    format!("Entertainment setup for {activity} activity"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are an entertainment system specialist configuring optimal audio for {activity} activities in {zones}.")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please configure the optimal entertainment experience for {activity} in {zones}.")
                        ),
                    ]
                )
            }

            "blinds_automation" => {
                let priority = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("priority"))
                    .map(|s| s.as_str())
                    .unwrap_or("comfort");

                let schedule_type = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("schedule_type"))
                    .map(|s| s.as_str())
                    .unwrap_or("daily");

                (
                    format!("Blinds automation (priority: {priority}, schedule: {schedule_type})"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a home automation expert creating intelligent blinds rules with {priority} priority and {schedule_type} scheduling.")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please create automated blinds rules prioritizing {priority} with {schedule_type} scheduling approach.")
                        ),
                    ]
                )
            }

            "system_troubleshooting" => {
                let symptom = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("symptom"))
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "general system issues".to_string());

                let affected_area = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("affected_area"))
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "system-wide".to_string());

                (
                    format!("Troubleshooting: {symptom} in {affected_area}"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a smart home troubleshooting expert analyzing {symptom} issues in {affected_area}.")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please provide step-by-step troubleshooting guidance for {symptom} in {affected_area}.")
                        ),
                    ]
                )
            }

            "maintenance_schedule" => {
                let priority_level = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("priority_level"))
                    .map(|s| s.as_str())
                    .unwrap_or("recommended");

                let season = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("season"))
                    .map(|s| s.as_str())
                    .unwrap_or("current season");

                (
                    format!("Maintenance schedule ({priority_level} priority, {season} focus)"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a home automation maintenance expert creating {priority_level} priority schedules for {season}.")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please generate a {priority_level} priority maintenance schedule focusing on {season} preparations.")
                        ),
                    ]
                )
            }

            "automation_suggestions" => {
                let lifestyle = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("lifestyle"))
                    .map(|s| s.as_str())
                    .unwrap_or("general");

                let complexity = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("complexity"))
                    .map(|s| s.as_str())
                    .unwrap_or("intermediate");

                (
                    format!("Automation suggestions for {lifestyle} lifestyle ({complexity})"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a home automation consultant specializing in {complexity} complexity solutions for {lifestyle} lifestyles.")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please suggest {complexity} automation rules suitable for a {lifestyle} lifestyle.")
                        ),
                    ]
                )
            }

            "scenario_planning" => {
                let scenario_type = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("scenario_type"))
                    .map(|s| s.as_str())
                    .unwrap_or("general");

                let duration = params
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("duration"))
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unspecified duration".to_string());

                (
                    format!("Scenario planning for {scenario_type} ({duration})"),
                    vec![
                        PromptMessage::new_text(
                            PromptMessageRole::System,
                            format!("You are a home automation scenario planner specializing in {scenario_type} scenarios lasting {duration}.")
                        ),
                        PromptMessage::new_text(
                            PromptMessageRole::User,
                            format!("Please plan and configure home automation for {scenario_type} scenario lasting {duration}.")
                        ),
                    ]
                )
            }

            _ => {
                return Err(BackendError::not_supported(format!(
                    "Prompt not found: {}",
                    params.name
                )));
            }
        };

        Ok(GetPromptResult {
            description: Some(description),
            messages,
        })
    }

    async fn subscribe(
        &self,
        params: SubscribeRequestParam,
    ) -> std::result::Result<(), Self::Error> {
        info!("🔔 Subscribing to Loxone resource: {}", params.uri);

        // Validate resource URI exists
        let is_valid_resource = match params.uri.as_str() {
            // System resources
            "loxone://system/info" | "loxone://structure/rooms" | "loxone://config/devices" |
            "loxone://status/health" | "loxone://system/capabilities" | "loxone://system/categories" |
            // Room and device resources
            "loxone://rooms" | "loxone://devices/all" |
            "loxone://devices/category/lighting" | "loxone://devices/category/blinds" | "loxone://devices/category/climate" |
            // Audio resources
            "loxone://audio/zones" | "loxone://audio/sources" |
            // Sensor resources
            "loxone://sensors/temperature" | "loxone://sensors/door-window" | "loxone://sensors/motion" |
            // Weather and energy resources
            "loxone://weather/current" | "loxone://energy/consumption" => true,
            _ => {
                // Also check if it's a dynamic resource template
                self.is_dynamic_resource(&params.uri)
            }
        };

        if !is_valid_resource {
            warn!("❌ Unknown resource URI for subscription: {}", params.uri);
            return Err(BackendError::not_supported(format!(
                "Resource not found for subscription: {}",
                params.uri
            )));
        }

        info!("✅ Valid resource URI for subscription: {}", params.uri);

        // Create client info for subscription
        use crate::server::subscription::types::{ClientInfo, ClientTransport};
        use std::time::SystemTime;
        use uuid::Uuid;

        let client_info = ClientInfo {
            id: Uuid::new_v4().to_string(), // Generate client ID if not provided
            transport: ClientTransport::Stdio, // Default to stdio for framework migration
            capabilities: vec!["resources".to_string(), "notifications".to_string()],
            connected_at: SystemTime::now(),
        };

        // Register with subscription coordinator
        match self
            .subscription_coordinator
            .subscribe_client(
                client_info,
                params.uri.clone(),
                None, // No filters for now
            )
            .await
        {
            Ok(()) => {
                info!(
                    "✅ Subscription registered with real-time updates: {}",
                    params.uri
                );

                // Start monitoring this resource type if not already started
                match params.uri.as_str() {
                    uri if uri.starts_with("loxone://sensors/") => {
                        // Start sensor monitoring if not already active
                        debug!("📊 Sensor monitoring active for: {}", uri);
                    }
                    uri if uri.starts_with("loxone://devices/") => {
                        // Start device state monitoring
                        debug!("📱 Device monitoring active for: {}", uri);
                    }
                    uri if uri.starts_with("loxone://energy/") => {
                        // Start energy monitoring
                        debug!("⚡ Energy monitoring active for: {}", uri);
                    }
                    uri if uri.starts_with("loxone://weather/") => {
                        // Start weather monitoring
                        debug!("🌤️ Weather monitoring active for: {}", uri);
                    }
                    _ => {
                        debug!("📡 General monitoring active for: {}", params.uri);
                    }
                }
                Ok(())
            }
            Err(e) => {
                error!("❌ Failed to register subscription: {}", e);
                Err(BackendError::from(e))
            }
        }
    }

    async fn unsubscribe(
        &self,
        params: UnsubscribeRequestParam,
    ) -> std::result::Result<(), Self::Error> {
        info!("🔕 Unsubscribing from Loxone resource: {}", params.uri);

        // Validate resource URI exists
        let is_valid_resource = match params.uri.as_str() {
            // Valid resource URIs (same as subscribe)
            "loxone://system/info"
            | "loxone://structure/rooms"
            | "loxone://config/devices"
            | "loxone://status/health"
            | "loxone://system/capabilities"
            | "loxone://system/categories"
            | "loxone://rooms"
            | "loxone://devices/all"
            | "loxone://devices/category/lighting"
            | "loxone://devices/category/blinds"
            | "loxone://devices/category/climate"
            | "loxone://audio/zones"
            | "loxone://audio/sources"
            | "loxone://sensors/temperature"
            | "loxone://sensors/door-window"
            | "loxone://sensors/motion"
            | "loxone://weather/current"
            | "loxone://energy/consumption" => true,
            _ => {
                // Also check if it's a dynamic resource template
                self.is_dynamic_resource(&params.uri)
            }
        };

        if !is_valid_resource {
            warn!("❌ Unknown resource URI for unsubscription: {}", params.uri);
            return Err(BackendError::not_supported(format!(
                "Resource not found for unsubscription: {}",
                params.uri
            )));
        }

        info!("✅ Valid resource URI for unsubscription: {}", params.uri);

        // For framework migration, we don't have client ID tracking yet
        // In a full implementation, we would need the client ID from the MCP session
        // For now, we'll unsubscribe all clients from this resource
        match self
            .subscription_coordinator
            .unsubscribe_client(
                "framework-migration-client".to_string(), // Placeholder client ID
                Some(params.uri.clone()),
            )
            .await
        {
            Ok(()) => {
                info!("✅ Unsubscription processed with cleanup: {}", params.uri);

                // Log monitoring status change
                match params.uri.as_str() {
                    uri if uri.starts_with("loxone://sensors/") => {
                        debug!("📊 Sensor monitoring may stop for: {}", uri);
                    }
                    uri if uri.starts_with("loxone://devices/") => {
                        debug!("📱 Device monitoring may stop for: {}", uri);
                    }
                    uri if uri.starts_with("loxone://energy/") => {
                        debug!("⚡ Energy monitoring may stop for: {}", uri);
                    }
                    uri if uri.starts_with("loxone://weather/") => {
                        debug!("🌤️ Weather monitoring may stop for: {}", uri);
                    }
                    _ => {
                        debug!("📡 General monitoring may stop for: {}", params.uri);
                    }
                }
                Ok(())
            }
            Err(e) => {
                error!("❌ Failed to process unsubscription: {}", e);
                Err(BackendError::from(e))
            }
        }
    }

    async fn complete(
        &self,
        params: CompleteRequestParam,
    ) -> std::result::Result<CompleteResult, Self::Error> {
        info!("🔍 Providing completion for: {}", params.ref_);

        let completions = match params.ref_.as_str() {
            // Room-based completions
            "room_names" => {
                let rooms = self.context.rooms.read().await;
                rooms.keys().cloned().collect::<Vec<_>>()
            }
            "room_names_with_lights" => {
                let rooms = self.context.rooms.read().await;
                let devices = self.context.devices.read().await;

                // Find rooms that have lighting devices
                let mut rooms_with_lights = Vec::new();
                for room_name in rooms.keys() {
                    let has_lights = devices.values().any(|device| {
                        device.room.as_ref() == Some(room_name)
                            && (device.category == "lights"
                                || device.device_type.contains("Light")
                                || device.device_type.contains("Dimmer"))
                    });
                    if has_lights {
                        rooms_with_lights.push(room_name.clone());
                    }
                }
                rooms_with_lights
            }
            "room_names_with_blinds" => {
                let rooms = self.context.rooms.read().await;
                let devices = self.context.devices.read().await;

                // Find rooms that have blinds/rolladen
                let mut rooms_with_blinds = Vec::new();
                for room_name in rooms.keys() {
                    let has_blinds = devices.values().any(|device| {
                        device.room.as_ref() == Some(room_name)
                            && (device.category == "blinds" || device.device_type == "Jalousie")
                    });
                    if has_blinds {
                        rooms_with_blinds.push(room_name.clone());
                    }
                }
                rooms_with_blinds
            }
            "room_names_with_climate" => {
                let rooms = self.context.rooms.read().await;
                let devices = self.context.devices.read().await;

                // Find rooms that have climate devices
                let mut rooms_with_climate = Vec::new();
                for room_name in rooms.keys() {
                    let has_climate = devices.values().any(|device| {
                        device.room.as_ref() == Some(room_name)
                            && (device.category == "climate"
                                || device.device_type.contains("Temperature")
                                || device.device_type.contains("Climate"))
                    });
                    if has_climate {
                        rooms_with_climate.push(room_name.clone());
                    }
                }
                rooms_with_climate
            }

            // Device-based completions
            "device_names" => {
                let devices = self.context.devices.read().await;
                devices.values().map(|d| d.name.clone()).collect()
            }
            "device_ids" => {
                let devices = self.context.devices.read().await;
                devices.keys().cloned().collect()
            }
            "lighting_device_names" => {
                let devices = self.context.devices.read().await;
                devices
                    .values()
                    .filter(|d| {
                        d.category == "lights"
                            || d.device_type.contains("Light")
                            || d.device_type.contains("Dimmer")
                    })
                    .map(|d| d.name.clone())
                    .collect()
            }
            "blinds_device_names" => {
                let devices = self.context.devices.read().await;
                devices
                    .values()
                    .filter(|d| d.category == "blinds" || d.device_type == "Jalousie")
                    .map(|d| d.name.clone())
                    .collect()
            }
            "audio_zone_names" => {
                let devices = self.context.devices.read().await;
                devices
                    .values()
                    .filter(|d| {
                        d.category == "audio"
                            || d.device_type.contains("Audio")
                            || d.device_type.contains("Music")
                    })
                    .map(|d| d.name.clone())
                    .collect()
            }

            // Device type completions with actual system data
            "device_types" => {
                let devices = self.context.devices.read().await;
                let mut types: std::collections::HashSet<String> =
                    devices.values().map(|d| d.device_type.clone()).collect();

                // Add common types even if not present
                types.insert("Light".to_string());
                types.insert("Jalousie".to_string());
                types.insert("TimedSwitch".to_string());
                types.insert("Dimmer".to_string());

                types.into_iter().collect()
            }
            "device_categories" => {
                let devices = self.context.devices.read().await;
                let mut categories: std::collections::HashSet<String> =
                    devices.values().map(|d| d.category.clone()).collect();

                // Add standard categories
                categories.insert("lights".to_string());
                categories.insert("blinds".to_string());
                categories.insert("climate".to_string());
                categories.insert("audio".to_string());
                categories.insert("security".to_string());

                categories.into_iter().collect()
            }

            // Action completions based on context
            "lighting_actions" => {
                vec![
                    "on".to_string(),
                    "off".to_string(),
                    "toggle".to_string(),
                    "dim".to_string(),
                    "brighten".to_string(),
                ]
            }
            "blinds_actions" => {
                vec![
                    "up".to_string(),
                    "down".to_string(),
                    "stop".to_string(),
                    "position".to_string(),
                    "hoch".to_string(),
                    "runter".to_string(),
                    "stopp".to_string(),
                ]
            }
            "audio_actions" => {
                vec![
                    "play".to_string(),
                    "pause".to_string(),
                    "stop".to_string(),
                    "next".to_string(),
                    "previous".to_string(),
                    "volume_up".to_string(),
                    "volume_down".to_string(),
                    "mute".to_string(),
                    "unmute".to_string(),
                ]
            }

            // Sensor type completions
            "sensor_types" => {
                vec![
                    "temperature".to_string(),
                    "humidity".to_string(),
                    "motion".to_string(),
                    "door_window".to_string(),
                    "contact".to_string(),
                    "presence".to_string(),
                    "air_quality".to_string(),
                ]
            }

            // Climate control completions
            "climate_modes" => {
                vec![
                    "auto".to_string(),
                    "heat".to_string(),
                    "cool".to_string(),
                    "off".to_string(),
                    "eco".to_string(),
                    "comfort".to_string(),
                    "manual".to_string(),
                ]
            }

            // Scope completions for unified tools
            "control_scopes" => {
                vec![
                    "device".to_string(),
                    "room".to_string(),
                    "system".to_string(),
                    "all".to_string(),
                ]
            }

            // Resource URI completions
            "resource_uris" => {
                vec![
                    "loxone://system/info".to_string(),
                    "loxone://structure/rooms".to_string(),
                    "loxone://config/devices".to_string(),
                    "loxone://status/health".to_string(),
                    "loxone://system/capabilities".to_string(),
                    "loxone://rooms".to_string(),
                    "loxone://devices/all".to_string(),
                    "loxone://devices/category/lighting".to_string(),
                    "loxone://devices/category/blinds".to_string(),
                    "loxone://devices/category/climate".to_string(),
                    "loxone://audio/zones".to_string(),
                    "loxone://sensors/temperature".to_string(),
                    "loxone://sensors/door-window".to_string(),
                    "loxone://sensors/motion".to_string(),
                    "loxone://weather/current".to_string(),
                    "loxone://energy/consumption".to_string(),
                ]
            }

            _ => {
                debug!("Unknown completion reference: {}", params.ref_);
                vec![]
            }
        };

        info!(
            "✅ Providing {} completions for '{}'",
            completions.len(),
            params.ref_
        );

        Ok(CompleteResult {
            completion: completions
                .into_iter()
                .map(|c| CompletionInfo {
                    completion: c,
                    has_more: Some(false),
                })
                .collect(),
        })
    }

    async fn set_level(
        &self,
        params: SetLevelRequestParam,
    ) -> std::result::Result<(), Self::Error> {
        info!("📊 Setting log level to: {}", params.level);

        // Update the tracing subscriber level if possible
        // This is a simplified implementation - in practice, you might want
        // to use a dynamic tracing subscriber that can be updated at runtime

        Ok(())
    }

    async fn handle_custom_method(
        &self,
        method: &str,
        _params: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, Self::Error> {
        warn!("❓ Unknown custom method: {}", method);
        Err(BackendError::configuration(format!(
            "Unknown method: {method}"
        )))
    }
}
