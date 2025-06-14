//! Proper MCP server implementation using 4t145/rmcp
//!
//! This module implements the MCP server using the correct API from the rmcp crate.

use crate::client::{create_client, ClientContext, LoxoneClient};
use crate::config::{LoxoneConfig, ServerConfig};
use crate::error::{LoxoneError, Result};

use rmcp::ServiceExt;
use std::sync::Arc;
use tokio::io::{stdin, stdout};
use tracing::{error, info, warn};

use health_check::{HealthCheckConfig, HealthChecker};
use loxone_batch_executor::LoxoneBatchExecutor;
use rate_limiter::{RateLimitConfig, RateLimitMiddleware};
use request_coalescing::{CoalescingConfig, RequestCoalescer};
use response_cache::ToolResponseCache;
use schema_validation::SchemaValidator;

pub mod handlers;
pub mod health_check;
pub mod loxone_batch_executor;
pub mod models;
pub mod rate_limiter;
pub mod request_coalescing;
pub mod request_context;
pub mod resource_monitor;
pub mod response_cache;
pub mod response_optimization;
pub mod rmcp_impl;
pub mod schema_validation;
pub mod workflow_engine;

pub use models::*;
pub use request_context::*;

/// Main MCP server for Loxone control
#[derive(Clone)]
pub struct LoxoneMcpServer {
    /// Server configuration
    #[allow(dead_code)]
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
        let client = create_client(&loxone_config, &credentials).await?;

        // Test connection and load structure
        match client.health_check().await {
            Ok(true) => info!("✅ Connected to Loxone successfully"),
            Ok(false) => {
                warn!("⚠️ Loxone connection established but health check shows issues");
                info!("🔄 Attempting to continue with degraded health status...");
                // Don't fail here - let's try to continue and see if we can get structure
            }
            Err(e) => {
                warn!("⚠️ Health check failed: {}", e);

                // If we're using token auth and it's failing, try to fall back to basic auth
                if loxone_config.auth_method == crate::config::AuthMethod::Token {
                    warn!("🔄 Token authentication health check failed, attempting fallback to basic authentication");
                    let mut basic_config = loxone_config.clone();
                    basic_config.auth_method = crate::config::AuthMethod::Basic;

                    match create_client(&basic_config, &credentials).await {
                        Ok(basic_client) => {
                            info!("✅ Successfully fell back to basic authentication");
                            // Test the basic client
                            match basic_client.health_check().await {
                                Ok(true) => {
                                    info!("✅ Basic authentication health check passed");
                                    // Replace the client with the basic auth client
                                    return Self::new_with_client(basic_client, basic_config).await;
                                }
                                Ok(false) => {
                                    warn!("⚠️ Basic authentication health check shows issues, continuing anyway");
                                }
                                Err(e) => {
                                    error!("❌ Basic authentication also failed: {}", e);
                                    return Err(e);
                                }
                            }
                        }
                        Err(fallback_err) => {
                            error!(
                                "❌ Failed to create basic auth fallback client: {}",
                                fallback_err
                            );
                            error!("❌ Original token auth error: {}", e);
                            return Err(e);
                        }
                    }
                } else {
                    error!("❌ Failed to connect to Loxone: {}", e);
                    return Err(e);
                }
            }
        }

        // Load structure
        info!("📊 Loading Loxone structure...");
        let structure = match client.get_structure().await {
            Ok(structure) => {
                info!("✅ Structure loaded successfully");
                structure
            }
            Err(e) => {
                warn!("⚠️ Structure loading failed: {}", e);

                // If we're using token auth and structure loading fails, try basic auth fallback
                if loxone_config.auth_method == crate::config::AuthMethod::Token {
                    warn!("🔄 Token authentication structure loading failed, attempting fallback to basic authentication");
                    let mut basic_config = loxone_config.clone();
                    basic_config.auth_method = crate::config::AuthMethod::Basic;

                    match create_client(&basic_config, &credentials).await {
                        Ok(basic_client) => {
                            info!("✅ Successfully created basic authentication client");
                            // Try to load structure with basic client
                            match basic_client.get_structure().await {
                                Ok(structure) => {
                                    info!("✅ Structure loaded successfully with basic authentication");
                                    // Create context
                                    let context = Arc::new(ClientContext::new());
                                    context.update_structure(structure).await?;
                                    return Self::new_with_context(
                                        basic_client,
                                        basic_config,
                                        context,
                                    )
                                    .await;
                                }
                                Err(basic_err) => {
                                    error!(
                                        "❌ Basic authentication structure loading also failed: {}",
                                        basic_err
                                    );
                                    return Err(e);
                                }
                            }
                        }
                        Err(fallback_err) => {
                            error!(
                                "❌ Failed to create basic auth fallback client: {}",
                                fallback_err
                            );
                            return Err(e);
                        }
                    }
                } else {
                    error!("❌ Failed to load structure: {}", e);
                    return Err(e);
                }
            }
        };

        // Create context
        let context = Arc::new(ClientContext::new());
        context.update_structure(structure).await?;

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
        })
    }

    /// Helper method to create server with specific client and config
    async fn new_with_client(client: Box<dyn LoxoneClient>, config: LoxoneConfig) -> Result<Self> {
        info!("🚀 Initializing Loxone MCP server with fallback client...");

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
        })
    }

    /// Run the MCP server
    pub async fn run(self) -> Result<()> {
        info!("🔌 Starting MCP server on stdio transport...");

        // Start the request coalescer batch processor
        info!("🚀 Starting request coalescer batch processor...");
        let _batch_processor_handle = self.request_coalescer.clone().start_batch_processor();
        info!("✅ Batch processor started");

        let service = self
            .clone()
            .serve((stdin(), stdout()))
            .await
            .map_err(|e| LoxoneError::connection(format!("Failed to start server: {}", e)))?;

        info!("✅ MCP server started successfully");

        // Keep server running
        let quit_reason = service
            .waiting()
            .await
            .map_err(|e| LoxoneError::connection(format!("Server error: {}", e)))?;

        info!("🛑 Server stopped: {:?}", quit_reason);
        Ok(())
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
}
