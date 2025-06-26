//! Proper MCP server implementation using 4t145/rmcp
//!
//! This module implements the MCP server using the correct API from the rmcp crate.

use crate::client::{create_client, ClientContext, LoxoneClient};
use crate::config::{LoxoneConfig, ServerConfig};
use crate::error::{LoxoneError, Result};

// Legacy removed - framework only
use std::sync::Arc;
use tracing::{error, info, warn};

use health_check::{HealthCheckConfig, HealthChecker};
use loxone_batch_executor::LoxoneBatchExecutor;
use rate_limiter::{RateLimitConfig, RateLimitMiddleware};
use request_coalescing::{CoalescingConfig, RequestCoalescer};
use response_cache::ToolResponseCache;
use schema_validation::SchemaValidator;

pub mod context_builders;
// Legacy modules temporarily disabled during framework migration
// pub mod handlers;
pub mod health_check;
pub mod loxone_batch_executor;
pub mod models;
pub mod rate_limiter;
pub mod request_coalescing;
pub mod request_context;
pub mod resource_monitor;
pub mod response_cache;
// pub mod response_optimization; // Disabled for legacy cleanup
// pub mod rmcp_impl;
pub mod schema_validation;
pub mod workflow_engine;

/// MCP Resources implementation for read-only data access
pub mod resources;

/// Real-time resource subscription system for MCP
pub mod subscription;

pub use models::*;
pub use request_context::*;

/// Main MCP server for Loxone control
#[derive(Clone)]
#[allow(dead_code)]
pub struct LoxoneMcpServer {
    /// Server configuration
    pub(crate) config: ServerConfig,

    /// Loxone client
    pub(crate) client: Arc<dyn LoxoneClient>,

    /// Client context for caching
    pub(crate) context: Arc<ClientContext>,

    /// Rate limiting middleware
    pub(crate) rate_limiter: Arc<RateLimitMiddleware>,

    /// Health checker for comprehensive monitoring
    pub(crate) health_checker: Arc<HealthChecker>,

    /// Request coalescer for performance optimization
    pub(crate) request_coalescer: Arc<RequestCoalescer>,

    /// Schema validator for parameter validation
    pub(crate) schema_validator: Arc<SchemaValidator>,

    /// Resource monitor for system resource management
    pub(crate) resource_monitor: Arc<resource_monitor::ResourceMonitor>,

    /// Response cache for MCP tools
    pub(crate) response_cache: Arc<ToolResponseCache>,

    /// Sampling protocol integration for MCP (optional)
    pub(crate) sampling_integration:
        Option<Arc<crate::sampling::protocol::SamplingProtocolIntegration>>,

    /// Resource subscription coordinator for real-time notifications
    pub(crate) subscription_coordinator: Arc<subscription::SubscriptionCoordinator>,

    // Removed history_store - unused module

    /// Loxone statistics collector (optional)
    #[cfg(feature = "influxdb")]
    #[allow(dead_code)]
    pub(crate) stats_collector: Option<Arc<crate::monitoring::loxone_stats::LoxoneStatsCollector>>,

    /// Unified value resolution service
    pub(crate) value_resolver: Arc<crate::services::UnifiedValueResolver>,

    /// Centralized state manager with change detection
    pub(crate) state_manager: Option<Arc<crate::services::StateManager>>,

    /// Server metrics collector for dashboard monitoring
    pub(crate) metrics_collector: Arc<crate::monitoring::server_metrics::ServerMetricsCollector>,
}

impl LoxoneMcpServer {
    /// Create new MCP server instance
    pub async fn new(config: ServerConfig) -> Result<Self> {
        info!("🚀 Initializing Loxone MCP server...");

        // Create credential manager with proper async initialization
        info!("📋 Initializing credential manager...");
        let credential_manager =
            match crate::config::credentials::create_best_credential_manager().await {
                Ok(manager) => manager,
                Err(e) => {
                    error!(
                        "❌ Failed to create multi-backend credential manager: {}",
                        e
                    );
                    error!("");
                    error!("🚀 Quick Setup Guide:");
                    error!("");
                    error!("Option 1: Use environment variables (simplest):");
                    error!("  export LOXONE_USERNAME=<your-username>");
                    error!("  export LOXONE_PASSWORD=<your-password>");
                    error!("  export LOXONE_HOST=<miniserver-ip-or-hostname>");
                    error!("");
                    error!("Option 2: Use keychain (interactive setup):");
                    error!("  cargo run --bin setup");
                    error!("");
                    error!("Option 3: Use Infisical (for teams):");
                    error!("  export INFISICAL_PROJECT_ID=\"your-project-id\"");
                    error!("  export INFISICAL_CLIENT_ID=\"your-client-id\"");
                    error!("  export INFISICAL_CLIENT_SECRET=\"your-client-secret\"");
                    error!("");
                    error!("For complete setup instructions, run:");
                    error!("  cargo run --bin setup");
                    error!("");
                    return Err(e);
                }
            };

        // Get credentials with host
        info!("🔐 Loading credentials...");
        let credentials = credential_manager.get_credentials().await?;
        info!("✅ Credentials loaded successfully");

        // URL is already configured in config.loxone.url

        let url = &config.loxone.url;
        info!("🌐 Connecting to Loxone at {}...", url);

        // Create the appropriate client
        let loxone_config = config.loxone.clone();
        let mut client = create_client(&loxone_config, &credentials).await?;

        // Start async connection attempt (non-blocking)
        info!("🚀 Starting asynchronous Loxone connection...");
        info!("⚡ Server will start immediately - connection attempts continue in background");
        
        // Check if we're in dev mode to skip actual connection
        let should_skip_connection = std::env::var("DEV_MODE").is_ok();
        
        if should_skip_connection {
            info!("🚧 Development mode detected - skipping Loxone connection");
        } else {
            // Try to connect immediately without blocking server startup
            info!("🔌 Attempting initial Loxone connection (non-blocking)...");
            
            // Try initial connection once, but don't fail if it doesn't work
            match client.connect().await {
                Ok(()) => {
                    info!("✅ Initial connection successful");
                }
                Err(e) => {
                    warn!("⚠️ Initial connection failed: {}", e);
                    
                    // If using token auth and connection failed, try fallback to basic auth
                    if loxone_config.auth_method == crate::config::AuthMethod::Token {
                        warn!("🔄 Token authentication failed during connection, trying basic authentication fallback");
                        
                        // Create a new basic auth client and try to connect
                        let mut basic_config = loxone_config.clone();
                        basic_config.auth_method = crate::config::AuthMethod::Basic;
                        
                        match create_client(&basic_config, &credentials).await {
                            Ok(mut basic_client) => {
                                match basic_client.connect().await {
                                    Ok(()) => {
                                        info!("✅ Basic authentication fallback successful");
                                        // Replace the client with the successful basic auth client
                                        client = basic_client;
                                    }
                                    Err(e) => {
                                        warn!("⚠️ Basic authentication fallback also failed: {}", e);
                                        info!("🔄 Server will continue - connection attempts can be retried later...");
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("⚠️ Failed to create basic auth fallback client: {}", e);
                                info!("🔄 Server will continue - connection attempts can be retried later...");
                            }
                        }
                    } else {
                        info!("🔄 Server will continue - connection attempts can be retried later...");
                    }
                }
            }
        }

        // Create context for the client - handle both HTTP client types
        let context = if let Some(http_client) = client
            .as_any()
            .downcast_ref::<crate::client::http_client::LoxoneHttpClient>()
        {
            // If using basic auth HTTP client, share its context directly
            info!("📊 Using basic auth HTTP client context");
            http_client.context().clone()
        } else if let Some(token_client) = client
            .as_any()
            .downcast_ref::<crate::client::token_http_client::TokenHttpClient>()
        {
            // If using token auth HTTP client, share its context directly
            info!("📊 Using token auth HTTP client context");
            token_client.context().clone()
        } else {
            // For all other clients, create default context for now
            warn!("📊 Creating default context - no HTTP client detected");
            Arc::new(ClientContext::new())
        };

        {
            let capabilities = context.capabilities.read().await;
            info!("🏠 System capabilities:");
            info!("  - {} rooms", context.rooms.read().await.len());
            info!("  - {} devices", context.devices.read().await.len());
            info!("  - {} lights", capabilities.light_count);
            info!("  - {} blinds", capabilities.blind_count);
            info!("  - {} climate zones", capabilities.climate_count);
            info!("  - {} sensors", capabilities.sensor_count);
        }

        // Initialize rate limiter with sensible defaults
        info!("🛡️ Initializing rate limiter...");
        let rate_config = RateLimitConfig::default();
        let rate_limiter = Arc::new(RateLimitMiddleware::new(rate_config));
        info!("✅ Rate limiter initialized");

        // Convert client to Arc for sharing
        let client_arc: Arc<dyn LoxoneClient> = Arc::from(client);

        // Initialize health checker
        info!("🏥 Initializing health checker...");
        let health_config = HealthCheckConfig::default();
        let health_checker = Arc::new(HealthChecker::new(client_arc.clone(), health_config));
        info!("✅ Health checker initialized");

        // Initialize request coalescer
        info!("⚡ Initializing request coalescer...");
        let coalescing_config = CoalescingConfig::default();
        let batch_executor = Arc::new(LoxoneBatchExecutor::new(client_arc.clone()));
        let request_coalescer = Arc::new(RequestCoalescer::new(coalescing_config, batch_executor));
        info!("✅ Request coalescer initialized");

        // Initialize schema validator
        info!("📋 Initializing schema validator...");
        let schema_validator = Arc::new(SchemaValidator::new());
        info!("✅ Schema validator initialized with standard tool schemas");

        // Initialize resource monitor
        info!("📊 Initializing resource monitor...");
        let resource_limits = resource_monitor::ResourceLimits::default();
        let resource_monitor = Arc::new(resource_monitor::ResourceMonitor::new(resource_limits));
        info!("✅ Resource monitor initialized with default limits");

        // Initialize response cache
        info!("🗄️ Initializing response cache...");
        let response_cache = Arc::new(ToolResponseCache::new());
        info!("✅ Response cache initialized with TTL-based eviction");

        // Initialize Loxone statistics collector (if InfluxDB is enabled)
        #[cfg(feature = "influxdb")]
        let stats_collector = {
            if std::env::var("ENABLE_LOXONE_STATS").is_ok()
                || std::env::var("INFLUXDB_TOKEN").is_ok()
            {
                info!("📈 Initializing Loxone statistics collector...");

                // Create metrics collector and initialize default metrics
                let metrics_collector =
                    Arc::new(crate::monitoring::metrics::MetricsCollector::new());
                metrics_collector.init_default_metrics().await;

                // Optionally create InfluxDB manager
                let influx_manager = if let Ok(token) = std::env::var("INFLUXDB_TOKEN") {
                    let influx_config = crate::monitoring::influxdb::InfluxConfig {
                        token,
                        url: std::env::var("INFLUXDB_URL")
                            .unwrap_or_else(|_| "http://localhost:8086".to_string()),
                        org: std::env::var("INFLUXDB_ORG")
                            .unwrap_or_else(|_| "loxone-mcp".to_string()),
                        bucket: std::env::var("INFLUXDB_BUCKET")
                            .unwrap_or_else(|_| "loxone_metrics".to_string()),
                        ..Default::default()
                    };

                    match crate::monitoring::influxdb::InfluxManager::new(influx_config).await {
                        Ok(manager) => {
                            info!("✅ InfluxDB integration enabled for Loxone stats");
                            Some(Arc::new(manager))
                        }
                        Err(e) => {
                            warn!("⚠️ Failed to initialize InfluxDB for stats: {}", e);
                            None
                        }
                    }
                } else {
                    None
                };

                let collector =
                    Arc::new(crate::monitoring::loxone_stats::LoxoneStatsCollector::new(
                        client_arc.clone(),
                        context.clone(),
                        metrics_collector,
                        influx_manager,
                    ));

                // Start the collector
                if let Err(e) = collector.start().await {
                    warn!("⚠️ Failed to start Loxone statistics collector: {}", e);
                    None
                } else {
                    info!("✅ Loxone statistics collector started");
                    Some(collector)
                }
            } else {
                info!("📈 Loxone statistics collection disabled (set ENABLE_LOXONE_STATS=1 to enable)");
                None
            }
        };

        // Initialize MCP sampling protocol integration with environment-based provider configuration
        info!("🔄 Initializing MCP sampling protocol...");
        let sampling_integration = {
            // Load provider configuration from environment variables
            let provider_config = crate::sampling::config::ProviderFactoryConfig::from_env();

            // Log configuration summary
            info!("🧠 LLM Provider Configuration:");
            info!("  {}", provider_config.get_selection_summary());

            if provider_config.is_ollama_primary() {
                info!(
                    "  🦙 Ollama (PRIMARY): {} with model '{}'",
                    provider_config.ollama.base_url, provider_config.ollama.default_model
                );
            }

            if provider_config.openai.enabled {
                info!(
                    "  🤖 OpenAI (FALLBACK): enabled with model '{}'",
                    provider_config.openai.default_model
                );
            }

            if provider_config.anthropic.enabled {
                info!(
                    "  🏛️ Anthropic (FALLBACK): enabled with model '{}'",
                    provider_config.anthropic.default_model
                );
            }

            if !provider_config.has_fallback_providers() {
                info!("  ⚠️ No fallback providers configured - only Ollama will be available");
            }

            // Validate configuration
            match provider_config.validate() {
                Ok(()) => {
                    info!("✅ Provider configuration validated successfully");

                    // For now, use mock implementation with enhanced configuration awareness
                    // TODO: Implement real provider factory when provider module is available
                    info!("ℹ️ Using enhanced mock implementation with environment-based configuration");

                    // Create enhanced sampling client manager with the validated configuration
                    let sampling_manager =
                        crate::sampling::client::SamplingClientManager::new_with_config(
                            provider_config.clone(),
                        );

                    // Log initial provider status
                    info!(
                        "📊 Initial provider status: {}",
                        sampling_manager.get_provider_summary().await
                    );

                    let integration =
                        crate::sampling::protocol::SamplingProtocolIntegration::new_with_mock(true);
                    Some(Arc::new(integration))
                }
                Err(e) => {
                    warn!("⚠️ Provider configuration validation failed: {}", e);
                    warn!("🔄 Falling back to basic mock implementation");
                    let integration =
                        crate::sampling::protocol::SamplingProtocolIntegration::new_with_mock(true);
                    Some(Arc::new(integration))
                }
            }
        };

        // Initialize subscription coordinator for real-time resource notifications
        info!("🔔 Initializing subscription coordinator...");
        let subscription_coordinator = Arc::new(
            subscription::SubscriptionCoordinator::new()
                .await
                .map_err(|e| {
                    error!("❌ Failed to initialize subscription coordinator: {}", e);
                    e
                })?,
        );

        // Start subscription system background tasks
        subscription_coordinator.start().await.map_err(|e| {
            error!("❌ Failed to start subscription system: {}", e);
            e
        })?;
        info!("✅ Subscription coordinator initialized and started");

        // Initialize unified value resolver with enhanced caching
        info!("🔍 Initializing unified value resolver with enhanced caching...");
        let sensor_registry = Arc::new(crate::services::SensorTypeRegistry::new());
        
        // Configure enhanced cache for better performance
        let cache_config = crate::services::cache_manager::CacheConfig {
            device_state_ttl: chrono::Duration::seconds(30), // 30-second TTL for real-time data
            sensor_ttl: chrono::Duration::seconds(60),       // 1-minute TTL for sensors
            structure_ttl: chrono::Duration::seconds(3600),  // 1 hour for structure data
            room_ttl: chrono::Duration::seconds(3600),       // 1 hour for room data
            max_cache_size: 10000,                           // Support large device counts
            enable_prefetch: true,                           // Enable intelligent prefetching
        };
        
        let value_resolver = Arc::new(crate::services::UnifiedValueResolver::with_cache_config(
            client_arc.clone(),
            sensor_registry,
            cache_config,
        ));
        info!("✅ Unified value resolver initialized with enhanced caching");

        // Initialize centralized state manager
        info!("🔄 Initializing centralized state manager...");
        let mut state_manager = crate::services::StateManager::new(value_resolver.clone()).await?;
        
        // Start state manager background tasks
        state_manager.start().await?;
        let state_manager = Arc::new(state_manager);
        info!("✅ Centralized state manager initialized and started");

        // Initialize server metrics collector
        info!("📊 Initializing server metrics collector...");
        let metrics_collector =
            Arc::new(crate::monitoring::server_metrics::ServerMetricsCollector::new());
        info!("✅ Server metrics collector initialized");

        Ok(Self {
            config,
            client: client_arc,
            context,
            rate_limiter,
            health_checker,
            request_coalescer,
            schema_validator,
            resource_monitor,
            response_cache,
            sampling_integration,
            subscription_coordinator,
            // Removed history_store initialization - unused module
            #[cfg(feature = "influxdb")]
            stats_collector,
            value_resolver,
            state_manager: Some(state_manager),
            metrics_collector,
        })
    }

    /// Helper method to create server with specific client and config
    #[allow(dead_code)]
    async fn new_with_client(
        mut client: Box<dyn LoxoneClient>,
        config: LoxoneConfig,
    ) -> Result<Self> {
        info!("🚀 Initializing Loxone MCP server with fallback client...");

        // Ensure client is connected
        if !client.is_connected().await.unwrap_or(false) {
            info!("🔌 Connecting fallback client...");
            client.connect().await?;
        }

        // Load structure with the new client
        info!("📊 Loading Loxone structure...");
        let structure = client.get_structure().await?;
        info!("✅ Structure loaded successfully");

        // Create context
        let context = Arc::new(ClientContext::new());
        context.update_structure(structure).await?;

        Self::new_with_context(client, config, context).await
    }

    /// Helper method to create server with specific client, config, and context
    #[allow(dead_code)]
    async fn new_with_context(
        client: Box<dyn LoxoneClient>,
        config: LoxoneConfig,
        context: Arc<ClientContext>,
    ) -> Result<Self> {
        {
            let capabilities = context.capabilities.read().await;
            info!("🏠 System capabilities:");
            info!("  - {} rooms", context.rooms.read().await.len());
            info!("  - {} devices", context.devices.read().await.len());
            info!("  - {} lights", capabilities.light_count);
            info!("  - {} blinds", capabilities.blind_count);
            info!("  - {} climate zones", capabilities.climate_count);
            info!("  - {} sensors", capabilities.sensor_count);
        }

        // Initialize rate limiter with sensible defaults
        info!("🛡️ Initializing rate limiter...");
        let rate_config = RateLimitConfig::default();
        let rate_limiter = Arc::new(RateLimitMiddleware::new(rate_config));
        info!("✅ Rate limiter initialized");

        // Convert client to Arc for sharing
        let client_arc: Arc<dyn LoxoneClient> = Arc::from(client);

        // Initialize health checker
        info!("🏥 Initializing health checker...");
        let health_config = HealthCheckConfig::default();
        let health_checker = Arc::new(HealthChecker::new(client_arc.clone(), health_config));
        info!("✅ Health checker initialized");

        // Initialize request coalescer
        info!("⚡ Initializing request coalescer...");
        let coalescing_config = CoalescingConfig::default();
        let batch_executor = Arc::new(LoxoneBatchExecutor::new(client_arc.clone()));
        let request_coalescer = Arc::new(RequestCoalescer::new(coalescing_config, batch_executor));
        info!("✅ Request coalescer initialized");

        // Initialize schema validator
        info!("📋 Initializing schema validator...");
        let schema_validator = Arc::new(SchemaValidator::new());
        info!("✅ Schema validator initialized with standard tool schemas");

        // Initialize resource monitor
        info!("📊 Initializing resource monitor...");
        let resource_limits = resource_monitor::ResourceLimits::default();
        let resource_monitor = Arc::new(resource_monitor::ResourceMonitor::new(resource_limits));
        info!("✅ Resource monitor initialized with default limits");

        // Initialize response cache
        info!("🗄️ Initializing response cache...");
        let response_cache = Arc::new(ToolResponseCache::new());
        info!("✅ Response cache initialized with TTL-based eviction");

        // Initialize Loxone statistics collector (if InfluxDB is enabled)
        #[cfg(feature = "influxdb")]
        let stats_collector = {
            if std::env::var("ENABLE_LOXONE_STATS").is_ok()
                || std::env::var("INFLUXDB_TOKEN").is_ok()
            {
                info!("📈 Initializing Loxone statistics collector...");

                // Create metrics collector and initialize default metrics
                let metrics_collector =
                    Arc::new(crate::monitoring::metrics::MetricsCollector::new());
                metrics_collector.init_default_metrics().await;

                // Optionally create InfluxDB manager
                let influx_manager = if let Ok(token) = std::env::var("INFLUXDB_TOKEN") {
                    let influx_config = crate::monitoring::influxdb::InfluxConfig {
                        token,
                        url: std::env::var("INFLUXDB_URL")
                            .unwrap_or_else(|_| "http://localhost:8086".to_string()),
                        org: std::env::var("INFLUXDB_ORG")
                            .unwrap_or_else(|_| "loxone-mcp".to_string()),
                        bucket: std::env::var("INFLUXDB_BUCKET")
                            .unwrap_or_else(|_| "loxone_metrics".to_string()),
                        ..Default::default()
                    };

                    match crate::monitoring::influxdb::InfluxManager::new(influx_config).await {
                        Ok(manager) => {
                            info!("✅ InfluxDB integration enabled for Loxone stats");
                            Some(Arc::new(manager))
                        }
                        Err(e) => {
                            warn!("⚠️ Failed to initialize InfluxDB for stats: {}", e);
                            None
                        }
                    }
                } else {
                    None
                };

                let collector =
                    Arc::new(crate::monitoring::loxone_stats::LoxoneStatsCollector::new(
                        client_arc.clone(),
                        context.clone(),
                        metrics_collector,
                        influx_manager,
                    ));

                // Start the collector
                if let Err(e) = collector.start().await {
                    warn!("⚠️ Failed to start Loxone statistics collector: {}", e);
                    None
                } else {
                    info!("✅ Loxone statistics collector started");
                    Some(collector)
                }
            } else {
                info!("📈 Loxone statistics collection disabled (set ENABLE_LOXONE_STATS=1 to enable)");
                None
            }
        };

        // Initialize MCP sampling protocol integration with environment-based provider configuration
        let sampling_integration = {
            info!("🔄 Initializing MCP sampling protocol...");

            // Load provider configuration from environment variables
            let provider_config = crate::sampling::config::ProviderFactoryConfig::from_env();

            // Log configuration summary
            info!("🧠 LLM Provider Configuration:");
            info!("  {}", provider_config.get_selection_summary());

            if provider_config.is_ollama_primary() {
                info!(
                    "  🦙 Ollama (PRIMARY): {} with model '{}'",
                    provider_config.ollama.base_url, provider_config.ollama.default_model
                );
            }

            if provider_config.openai.enabled {
                info!(
                    "  🤖 OpenAI (FALLBACK): enabled with model '{}'",
                    provider_config.openai.default_model
                );
            }

            if provider_config.anthropic.enabled {
                info!(
                    "  🏛️ Anthropic (FALLBACK): enabled with model '{}'",
                    provider_config.anthropic.default_model
                );
            }

            if !provider_config.has_fallback_providers() {
                info!("  ⚠️ No fallback providers configured - only Ollama will be available");
            }

            // Validate configuration
            match provider_config.validate() {
                Ok(()) => {
                    info!("✅ Provider configuration validated successfully");

                    // For now, use mock implementation with enhanced configuration awareness
                    // TODO: Implement real provider factory when provider module is available
                    info!("ℹ️ Using enhanced mock implementation with environment-based configuration");

                    // Create enhanced sampling client manager with the validated configuration
                    let sampling_manager =
                        crate::sampling::client::SamplingClientManager::new_with_config(
                            provider_config.clone(),
                        );

                    // Log initial provider status
                    info!(
                        "📊 Initial provider status: {}",
                        sampling_manager.get_provider_summary().await
                    );

                    let integration =
                        crate::sampling::protocol::SamplingProtocolIntegration::new_with_mock(true);
                    Some(Arc::new(integration))
                }
                Err(e) => {
                    warn!("⚠️ Provider configuration validation failed: {}", e);
                    warn!("🔄 Falling back to basic mock implementation");
                    let integration =
                        crate::sampling::protocol::SamplingProtocolIntegration::new_with_mock(true);
                    Some(Arc::new(integration))
                }
            }
        };

        // Initialize subscription coordinator for real-time resource notifications
        info!("🔔 Initializing subscription coordinator...");
        let subscription_coordinator = Arc::new(
            subscription::SubscriptionCoordinator::new()
                .await
                .map_err(|e| {
                    error!("❌ Failed to initialize subscription coordinator: {}", e);
                    e
                })?,
        );

        // Start subscription system background tasks
        subscription_coordinator.start().await.map_err(|e| {
            error!("❌ Failed to start subscription system: {}", e);
            e
        })?;
        info!("✅ Subscription coordinator initialized and started");

        // Initialize unified value resolver with enhanced caching
        info!("🔍 Initializing unified value resolver with enhanced caching...");
        let sensor_registry = Arc::new(crate::services::SensorTypeRegistry::new());
        
        // Configure enhanced cache for better performance
        let cache_config = crate::services::cache_manager::CacheConfig {
            device_state_ttl: chrono::Duration::seconds(30), // 30-second TTL for real-time data
            sensor_ttl: chrono::Duration::seconds(60),       // 1-minute TTL for sensors
            structure_ttl: chrono::Duration::seconds(3600),  // 1 hour for structure data
            room_ttl: chrono::Duration::seconds(3600),       // 1 hour for room data
            max_cache_size: 10000,                           // Support large device counts
            enable_prefetch: true,                           // Enable intelligent prefetching
        };
        
        let value_resolver = Arc::new(crate::services::UnifiedValueResolver::with_cache_config(
            client_arc.clone(),
            sensor_registry,
            cache_config,
        ));
        info!("✅ Unified value resolver initialized with enhanced caching");

        // Initialize centralized state manager
        info!("🔄 Initializing centralized state manager...");
        let mut state_manager = crate::services::StateManager::new(value_resolver.clone()).await?;
        
        // Start state manager background tasks
        state_manager.start().await?;
        let state_manager = Arc::new(state_manager);
        info!("✅ Centralized state manager initialized and started");

        // Initialize server metrics collector
        info!("📊 Initializing server metrics collector...");
        let metrics_collector =
            Arc::new(crate::monitoring::server_metrics::ServerMetricsCollector::new());
        info!("✅ Server metrics collector initialized");

        Ok(Self {
            config: ServerConfig {
                loxone: config,
                ..Default::default()
            },
            client: client_arc,
            context,
            rate_limiter,
            health_checker,
            request_coalescer,
            schema_validator,
            resource_monitor,
            response_cache,
            sampling_integration,
            subscription_coordinator,
            // Removed history_store initialization - unused module
            #[cfg(feature = "influxdb")]
            stats_collector,
            value_resolver,
            state_manager: Some(state_manager),
            metrics_collector,
        })
    }

    /// Get the unified value resolver
    pub fn get_value_resolver(&self) -> &Arc<crate::services::UnifiedValueResolver> {
        &self.value_resolver
    }

    /// Get the state manager (if initialized)
    pub fn get_state_manager(&self) -> Option<&Arc<crate::services::StateManager>> {
        self.state_manager.as_ref()
    }

    /// Get the server metrics collector
    pub fn get_metrics_collector(
        &self,
    ) -> &Arc<crate::monitoring::server_metrics::ServerMetricsCollector> {
        &self.metrics_collector
    }

    /// Initialize the state manager with change detection
    pub async fn enable_state_management(&mut self) -> Result<()> {
        if self.state_manager.is_some() {
            info!("🔄 State manager already initialized");
            return Ok(());
        }

        info!("🎯 Initializing centralized state manager...");
        let mut state_manager =
            crate::services::StateManager::new(self.value_resolver.clone()).await?;

        // Start background tasks
        state_manager.start().await?;

        self.state_manager = Some(Arc::new(state_manager));
        info!("✅ State manager initialized and running");

        Ok(())
    }

    /// Run the MCP server (legacy - use framework instead)
    pub async fn run(self) -> Result<()> {
        Err(LoxoneError::config(
            "Legacy run method disabled - use framework implementation in main.rs".to_string()
        ))
    }

    /// Get request coalescer metrics
    pub fn get_coalescing_metrics(&self) -> request_coalescing::CoalescingMetrics {
        self.request_coalescer.get_metrics()
    }

    /// Check if a tool is read-only (safe to cache)
    pub fn is_read_only_tool(tool_name: &str) -> bool {
        matches!(
            tool_name,
            "list_rooms"
                | "list_devices"
                | "list_devices_in_room"
                | "get_device_state"
                | "get_room_devices"
                | "get_system_info"
                | "get_weather_info"
                | "get_energy_info"
                | "get_sensor_readings"
                | "health_check"
                | "test_connection"
                | "discover_rooms"
                | "discover_devices"
        )
    }

    /// Get system status for HTTP transport compatibility
    pub async fn get_system_status(&self) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "status": "ok",
            "uptime": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "version": "1.0.0"
        }))
    }

    /// Get server info for HTTP transport compatibility
    pub fn get_info(&self) -> serde_json::Value {
        serde_json::json!({
            "server_info": {
                "name": "Loxone MCP Server",
                "version": "1.0.0"
            },
            "instructions": "MCP server for Loxone home automation",
            "description": "MCP server for Loxone home automation"
        })
    }

    /// Call tool for HTTP transport compatibility
    pub async fn call_tool(&self, tool_name: &str, arguments: serde_json::Value) -> Result<serde_json::Value> {
        // Basic implementation - just return success for now
        Ok(serde_json::json!({
            "tool": tool_name,
            "arguments": arguments,
            "result": "Not implemented in legacy server - use framework backend"
        }))
    }


    // Prompt message methods for HTTP transport compatibility
    pub async fn get_cozy_prompt_messages(&self, _args: Option<serde_json::Value>) -> Result<Vec<serde_json::Value>> {
        Ok(vec![serde_json::json!({"role": "user", "content": "Make home cozy"})])
    }

    pub async fn get_event_prompt_messages(&self, _args: Option<serde_json::Value>) -> Result<Vec<serde_json::Value>> {
        Ok(vec![serde_json::json!({"role": "user", "content": "Prepare for event"})])
    }

    pub async fn get_energy_prompt_messages(&self, _args: Option<serde_json::Value>) -> Result<Vec<serde_json::Value>> {
        Ok(vec![serde_json::json!({"role": "user", "content": "Analyze energy usage"})])
    }

    pub async fn get_morning_prompt_messages(&self, _args: Option<serde_json::Value>) -> Result<Vec<serde_json::Value>> {
        Ok(vec![serde_json::json!({"role": "user", "content": "Good morning routine"})])
    }

    pub async fn get_night_prompt_messages(&self, _args: Option<serde_json::Value>) -> Result<Vec<serde_json::Value>> {
        Ok(vec![serde_json::json!({"role": "user", "content": "Good night routine"})])
    }

    pub async fn get_comfort_optimization_messages(&self, _args: Option<serde_json::Value>) -> Result<Vec<serde_json::Value>> {
        Ok(vec![serde_json::json!({"role": "user", "content": "Optimize comfort"})])
    }

    pub async fn get_seasonal_adjustment_messages(&self, _args: Option<serde_json::Value>) -> Result<Vec<serde_json::Value>> {
        Ok(vec![serde_json::json!({"role": "user", "content": "Seasonal adjustment"})])
    }

    pub async fn get_security_analysis_messages(&self, _args: Option<serde_json::Value>) -> Result<Vec<serde_json::Value>> {
        Ok(vec![serde_json::json!({"role": "user", "content": "Security analysis"})])
    }

    pub async fn get_troubleshooting_messages(&self, _args: Option<serde_json::Value>) -> Result<Vec<serde_json::Value>> {
        Ok(vec![serde_json::json!({"role": "user", "content": "Troubleshoot automation"})])
    }

    pub async fn get_custom_scene_messages(&self, _args: Option<serde_json::Value>) -> Result<Vec<serde_json::Value>> {
        Ok(vec![serde_json::json!({"role": "user", "content": "Create custom scene"})])
    }
}

/// Dummy metrics collector for HTTP transport compatibility
pub struct DummyMetricsCollector;

impl DummyMetricsCollector {
    pub fn record_prompt(&self) {}
    pub fn connection_opened(&self) {}
    pub fn connection_closed(&self) {}
    pub async fn get_metrics(&self) -> serde_json::Value {
        serde_json::json!({
            "requests": 0,
            "connections": 0
        })
    }
}

/// Try to connect with fallback authentication
#[allow(dead_code)]
async fn try_connect_with_fallback(
    mut client: Box<dyn LoxoneClient>,
    loxone_config: LoxoneConfig,
    credentials: crate::config::credentials::LoxoneCredentials,
) -> Result<()> {
    // Try initial connection
    match client.connect().await {
        Ok(()) => {
            info!("✅ Successfully connected to Loxone system");
            // Test connection health
            match client.health_check().await {
                Ok(true) => info!("✅ Health check passed"),
                Ok(false) => {
                    warn!("⚠️ Loxone connection established but health check shows issues");
                    info!("🔄 Continuing with degraded health status...");
                }
                Err(e) => {
                    warn!("⚠️ Health check failed after connection: {}", e);
                }
            }
            Ok(())
        }
        Err(e) => {
            warn!("⚠️ Connection failed: {}", e);

            // If using advanced auth, try fallback to basic auth
            if loxone_config.auth_method == crate::config::AuthMethod::Token 
                || (cfg!(feature = "websocket") && loxone_config.auth_method == crate::config::AuthMethod::WebSocket) {
                warn!("🔄 Advanced authentication failed, trying basic authentication fallback");
                
                let mut basic_config = loxone_config.clone();
                basic_config.auth_method = crate::config::AuthMethod::Basic;

                match create_client(&basic_config, &credentials).await {
                    Ok(mut basic_client) => {
                        info!("✅ Successfully created basic authentication client");
                        match basic_client.connect().await {
                            Ok(()) => {
                                info!("✅ Basic authentication connection successful");
                                Ok(())
                            }
                            Err(e) => {
                                error!("❌ Basic authentication connection also failed: {}", e);
                                Err(e)
                            }
                        }
                    }
                    Err(fallback_err) => {
                        error!("❌ Failed to create basic auth fallback client: {}", fallback_err);
                        Err(e)
                    }
                }
            } else {
                error!("❌ Failed to connect to Loxone: {}", e);
                Err(e)
            }
        }
    }
}

/// Spawn a background reconnection task
#[allow(dead_code)]
async fn spawn_reconnection_task(
    loxone_config: LoxoneConfig,
    credentials: crate::config::credentials::LoxoneCredentials,
) {
    tokio::spawn(async move {
        let mut retry_count = 0;
        let max_retries = 10; // Limit retries to avoid infinite loops
        let base_delay = std::time::Duration::from_secs(30);
        
        loop {
            if retry_count >= max_retries {
                error!("❌ Max reconnection attempts reached. Giving up.");
                break;
            }
            
            retry_count += 1;
            let delay = base_delay * retry_count.min(5); // Cap delay at 5x base (2.5 minutes)
            
            info!("⏰ Sleeping for {} seconds before retry #{}", delay.as_secs(), retry_count);
            tokio::time::sleep(delay).await;
            
            info!("🔌 Reconnection attempt #{}/{}", retry_count, max_retries);
            
            match create_client(&loxone_config, &credentials).await {
                Ok(client) => {
                    match try_connect_with_fallback(client, loxone_config.clone(), credentials.clone()).await {
                        Ok(()) => {
                            info!("✅ Reconnection successful after {} attempts", retry_count);
                            break;
                        }
                        Err(e) => {
                            warn!("⚠️ Reconnection attempt #{} failed: {}", retry_count, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("⚠️ Failed to create client for reconnection attempt #{}: {}", retry_count, e);
                }
            }
        }
    });
}
