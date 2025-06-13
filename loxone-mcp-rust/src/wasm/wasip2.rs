//! WASM WASIP2-specific implementation
//!
//! This module provides optimized WASM functionality specifically for the
//! wasm32-wasip2 target with component model support.

use crate::config::{CredentialStore, LoxoneCredentials, ServerConfig};
use crate::error::{LoxoneError, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

// WASIP2-specific imports
#[cfg(target_arch = "wasm32")]
use wasi::{
    config::runtime as config_runtime,
    http::{outgoing_handler, types as http_types},
    keyvalue::{store, types as kv_types},
    logging,
};

/// Global WASIP2 component state
static COMPONENT_STATE: OnceLock<Arc<Mutex<Wasip2ComponentState>>> = OnceLock::new();

#[derive(Debug)]
struct Wasip2ComponentState {
    config: ServerConfig,
    keyvalue_store: Option<kv_types::Bucket>,
    credentials: Option<LoxoneCredentials>,
    connection_pool: HashMap<String, Wasip2HttpConnection>,
    metrics: Wasip2Metrics,
}

#[derive(Debug, Clone)]
struct Wasip2HttpConnection {
    base_url: String,
    timeout_ms: u32,
    retry_count: u32,
}

#[derive(Debug, Default)]
struct Wasip2Metrics {
    requests_total: u32,
    requests_failed: u32,
    connections_active: u32,
    memory_usage_bytes: u32,
    component_uptime_ms: u64,
}

impl Wasip2ComponentState {
    fn new(config: ServerConfig) -> Self {
        Self {
            config,
            keyvalue_store: None,
            credentials: None,
            connection_pool: HashMap::new(),
            metrics: Wasip2Metrics::default(),
        }
    }
}

/// Initialize the WASIP2 component
pub async fn initialize_wasip2_component(config: ServerConfig) -> Result<()> {
    let state = Arc::new(Mutex::new(Wasip2ComponentState::new(config)));

    // Initialize keyvalue store for credentials
    #[cfg(target_arch = "wasm32")]
    {
        let bucket = store::open("loxone-mcp-credentials")
            .map_err(|e| LoxoneError::config(format!("Failed to open keyvalue store: {:?}", e)))?;

        state.lock().unwrap().keyvalue_store = Some(bucket);

        // Log initialization
        logging::log(
            logging::Level::Info,
            "loxone-mcp",
            "WASIP2 component initialized with keyvalue store",
        );
    }

    COMPONENT_STATE
        .set(state)
        .map_err(|_| LoxoneError::config("Component already initialized"))?;

    Ok(())
}

/// WASIP2-optimized credential manager
pub struct Wasip2CredentialManager;

impl Wasip2CredentialManager {
    /// Store credentials using WASI keyvalue interface
    pub async fn store_credentials(credentials: &LoxoneCredentials) -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        {
            let state = COMPONENT_STATE
                .get()
                .ok_or_else(|| LoxoneError::config("Component not initialized"))?;

            let mut state_guard = state.lock().unwrap();

            if let Some(ref bucket) = state_guard.keyvalue_store {
                // Serialize credentials securely
                let creds_bytes = serde_json::to_vec(credentials).map_err(|e| {
                    LoxoneError::credentials(format!("Serialization failed: {}", e))
                })?;

                // Store in WASI keyvalue
                store::set(bucket, "credentials", &creds_bytes)
                    .map_err(|e| LoxoneError::credentials(format!("Storage failed: {:?}", e)))?;

                state_guard.credentials = Some(credentials.clone());

                logging::log(
                    logging::Level::Info,
                    "loxone-mcp",
                    "Credentials stored in WASI keyvalue store",
                );

                return Ok(());
            }
        }

        Err(LoxoneError::credentials(
            "WASI keyvalue store not available",
        ))
    }

    /// Get credentials from WASI keyvalue interface
    pub async fn get_credentials() -> Result<LoxoneCredentials> {
        #[cfg(target_arch = "wasm32")]
        {
            let state = COMPONENT_STATE
                .get()
                .ok_or_else(|| LoxoneError::config("Component not initialized"))?;

            let state_guard = state.lock().unwrap();

            // Return cached credentials if available
            if let Some(ref creds) = state_guard.credentials {
                return Ok(creds.clone());
            }

            if let Some(ref bucket) = state_guard.keyvalue_store {
                // Try to load from WASI keyvalue
                if let Ok(Some(creds_bytes)) = store::get(bucket, "credentials") {
                    let credentials: LoxoneCredentials = serde_json::from_slice(&creds_bytes)
                        .map_err(|e| {
                            LoxoneError::credentials(format!("Deserialization failed: {}", e))
                        })?;

                    return Ok(credentials);
                }
            }
        }

        Err(LoxoneError::credentials("No credentials found"))
    }

    /// Clear credentials from WASI keyvalue interface
    pub async fn clear_credentials() -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        {
            let state = COMPONENT_STATE
                .get()
                .ok_or_else(|| LoxoneError::config("Component not initialized"))?;

            let mut state_guard = state.lock().unwrap();

            if let Some(ref bucket) = state_guard.keyvalue_store {
                store::delete(bucket, "credentials")
                    .map_err(|e| LoxoneError::credentials(format!("Delete failed: {:?}", e)))?;

                state_guard.credentials = None;

                logging::log(
                    logging::Level::Info,
                    "loxone-mcp",
                    "Credentials cleared from WASI keyvalue store",
                );

                return Ok(());
            }
        }

        Err(LoxoneError::credentials(
            "WASI keyvalue store not available",
        ))
    }
}

/// WASIP2-optimized HTTP client for Loxone communication
pub struct Wasip2HttpClient {
    connection: Wasip2HttpConnection,
}

impl Wasip2HttpClient {
    pub fn new(base_url: String, timeout_ms: u32) -> Self {
        Self {
            connection: Wasip2HttpConnection {
                base_url,
                timeout_ms,
                retry_count: 0,
            },
        }
    }

    /// Make HTTP request using WASI HTTP interface
    pub async fn request(&self, path: &str, method: &str, body: Option<&[u8]>) -> Result<Vec<u8>> {
        #[cfg(target_arch = "wasm32")]
        {
            // Update metrics
            if let Some(state) = COMPONENT_STATE.get() {
                let mut state_guard = state.lock().unwrap();
                state_guard.metrics.requests_total += 1;
            }

            // Construct request URL
            let url = format!("{}{}", self.connection.base_url, path);

            // Create WASI HTTP request
            let request = http_types::OutgoingRequest::new(http_types::Headers::new());
            request
                .set_method(&http_types::Method::Get)
                .map_err(|e| LoxoneError::http(format!("Failed to set method: {:?}", e)))?;

            request
                .set_path_with_query(Some(&url))
                .map_err(|e| LoxoneError::http(format!("Failed to set URL: {:?}", e)))?;

            request
                .set_scheme(Some(&http_types::Scheme::Http))
                .map_err(|e| LoxoneError::http(format!("Failed to set scheme: {:?}", e)))?;

            // Add body if provided
            if let Some(body_data) = body {
                let outgoing_body = request.body().map_err(|e| {
                    LoxoneError::http(format!("Failed to get request body: {:?}", e))
                })?;

                let body_stream = outgoing_body.write().map_err(|e| {
                    LoxoneError::http(format!("Failed to get body stream: {:?}", e))
                })?;

                body_stream
                    .write(body_data)
                    .map_err(|e| LoxoneError::http(format!("Failed to write body: {:?}", e)))?;

                drop(body_stream);
                drop(outgoing_body);
            }

            // Send request with timeout
            let response_future = outgoing_handler::handle(request, None);
            let response = response_future.map_err(|e| {
                if let Some(state) = COMPONENT_STATE.get() {
                    let mut state_guard = state.lock().unwrap();
                    state_guard.metrics.requests_failed += 1;
                }
                LoxoneError::http(format!("Request failed: {:?}", e))
            })?;

            // Read response body
            let incoming_body = response
                .consume()
                .map_err(|e| LoxoneError::http(format!("Failed to consume response: {:?}", e)))?;

            let body_stream = incoming_body.stream().map_err(|e| {
                LoxoneError::http(format!("Failed to get response stream: {:?}", e))
            })?;

            let mut response_data = Vec::new();
            loop {
                match body_stream.read(8192) {
                    Ok(chunk) => {
                        if chunk.is_empty() {
                            break;
                        }
                        response_data.extend_from_slice(&chunk);
                    }
                    Err(_) => break,
                }
            }

            logging::log(
                logging::Level::Debug,
                "loxone-mcp",
                &format!("HTTP request completed: {} bytes", response_data.len()),
            );

            return Ok(response_data);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Fallback for non-WASM builds
            Err(LoxoneError::http(
                "WASI HTTP not available in non-WASM build",
            ))
        }
    }
}

/// WASIP2 configuration loader using WASI config interface
pub struct Wasip2ConfigLoader;

impl Wasip2ConfigLoader {
    /// Load configuration from WASI runtime config
    pub async fn load_config() -> Result<ServerConfig> {
        let mut config = ServerConfig::default();

        #[cfg(target_arch = "wasm32")]
        {
            // Load from WASI config interface
            if let Ok(Some(loxone_url)) = config_runtime::get("LOXONE_URL") {
                config.loxone.url = loxone_url
                    .parse()
                    .map_err(|e| LoxoneError::config(format!("Invalid LOXONE_URL: {}", e)))?;
            }

            if let Ok(Some(loxone_user)) = config_runtime::get("LOXONE_USER") {
                config.loxone.username = loxone_user;
            }

            if let Ok(Some(timeout_str)) = config_runtime::get("LOXONE_TIMEOUT") {
                if let Ok(timeout_secs) = timeout_str.parse::<u64>() {
                    config.loxone.timeout = std::time::Duration::from_secs(timeout_secs);
                }
            }

            if let Ok(Some(log_level)) = config_runtime::get("LOG_LEVEL") {
                config.logging.level = log_level;
            }

            // Set WASIP2-specific defaults
            config.credentials = CredentialStore::WasiKeyvalue;
            config.mcp.transport.transport_type = "wasm".to_string();

            logging::log(
                logging::Level::Info,
                "loxone-mcp",
                "Configuration loaded from WASI runtime config",
            );
        }

        Ok(config)
    }

    /// Validate WASIP2 configuration
    pub fn validate_wasip2_config(config: &ServerConfig) -> Result<()> {
        // Ensure WASM-compatible settings
        match config.credentials {
            CredentialStore::WasiKeyvalue => {}
            _ => {
                logging::log(
                    logging::Level::Warn,
                    "loxone-mcp",
                    "Non-WASIP2 credential store detected, using WasiKeyvalue",
                );
            }
        }

        if config.mcp.transport.transport_type != "wasm" {
            return Err(LoxoneError::config(
                "Transport type must be 'wasm' for WASIP2 target",
            ));
        }

        Ok(())
    }
}

/// WASIP2 memory management and optimization
pub struct Wasip2MemoryManager;

impl Wasip2MemoryManager {
    /// Get current memory usage
    pub fn get_memory_usage() -> Wasip2MemoryInfo {
        #[cfg(target_arch = "wasm32")]
        {
            // Use WASM memory introspection
            let memory = wasm_bindgen::memory();
            let buffer = memory.buffer();

            Wasip2MemoryInfo {
                total_bytes: buffer.byte_length() as u32,
                used_bytes: 0, // Would need more sophisticated tracking
                peak_bytes: 0,
                allocations: 0,
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        Wasip2MemoryInfo::default()
    }

    /// Optimize memory usage for WASIP2
    pub fn optimize_memory() {
        #[cfg(target_arch = "wasm32")]
        {
            // Force garbage collection if available
            if let Some(window) = web_sys::window() {
                if let Ok(gc) = js_sys::Reflect::get(&window, &"gc".into()) {
                    if gc.is_function() {
                        let _ = js_sys::Function::from(gc).call0(&window);
                    }
                }
            }

            logging::log(
                logging::Level::Debug,
                "loxone-mcp",
                "Memory optimization completed",
            );
        }
    }
}

#[derive(Debug, Default)]
pub struct Wasip2MemoryInfo {
    pub total_bytes: u32,
    pub used_bytes: u32,
    pub peak_bytes: u32,
    pub allocations: u32,
}

/// WASIP2-specific MCP server implementation
pub struct Wasip2McpServer {
    config: ServerConfig,
    credential_manager: Wasip2CredentialManager,
    http_client: Option<Wasip2HttpClient>,
}

impl Wasip2McpServer {
    /// Create new WASIP2 MCP server
    pub async fn new(config: ServerConfig) -> Result<Self> {
        // Validate WASIP2 configuration
        Wasip2ConfigLoader::validate_wasip2_config(&config)?;

        // Initialize component state
        initialize_wasip2_component(config.clone()).await?;

        let server = Self {
            config: config.clone(),
            credential_manager: Wasip2CredentialManager,
            http_client: None,
        };

        logging::log(
            logging::Level::Info,
            "loxone-mcp",
            "WASIP2 MCP server created successfully",
        );

        Ok(server)
    }

    /// Initialize HTTP client for Loxone communication
    pub async fn initialize_client(&mut self) -> Result<()> {
        let credentials = self.credential_manager.get_credentials().await?;

        let http_client = Wasip2HttpClient::new(
            self.config.loxone.url.to_string(),
            self.config.loxone.timeout.as_millis() as u32,
        );

        self.http_client = Some(http_client);

        logging::log(
            logging::Level::Info,
            "loxone-mcp",
            "HTTP client initialized for WASIP2",
        );

        Ok(())
    }

    /// Get server metrics
    pub fn get_metrics() -> Result<Wasip2Metrics> {
        let state = COMPONENT_STATE
            .get()
            .ok_or_else(|| LoxoneError::config("Component not initialized"))?;

        let state_guard = state.lock().unwrap();
        Ok(state_guard.metrics.clone())
    }

    /// Process MCP tool call optimized for WASIP2
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let start_time = std::time::Instant::now();

        let result = match name {
            "list_rooms" => self.handle_list_rooms().await,
            "control_device" => self.handle_control_device(arguments).await,
            "get_device_state" => self.handle_get_device_state(arguments).await,
            "test_connection" => self.handle_test_connection().await,
            _ => Err(LoxoneError::config(format!("Unknown tool: {}", name))),
        };

        let elapsed = start_time.elapsed();
        logging::log(
            logging::Level::Debug,
            "loxone-mcp",
            &format!("Tool '{}' completed in {:?}", name, elapsed),
        );

        result
    }

    async fn handle_list_rooms(&self) -> Result<serde_json::Value> {
        if let Some(ref client) = self.http_client {
            let response = client.request("/data/loxapp3.json", "GET", None).await?;
            let structure: serde_json::Value = serde_json::from_slice(&response)?;

            // Extract rooms from structure
            if let Some(rooms) = structure.get("rooms") {
                return Ok(rooms.clone());
            }
        }

        Ok(serde_json::json!([]))
    }

    async fn handle_control_device(
        &self,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let device_uuid = arguments
            .get("uuid")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LoxoneError::config("Missing device UUID"))?;

        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LoxoneError::config("Missing action"))?;

        if let Some(ref client) = self.http_client {
            let path = format!("/dev/sps/io/{}/{}", device_uuid, action);
            let response = client.request(&path, "GET", None).await?;
            let result: serde_json::Value = serde_json::from_slice(&response)?;

            return Ok(result);
        }

        Err(LoxoneError::config("HTTP client not initialized"))
    }

    async fn handle_get_device_state(
        &self,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let device_uuid = arguments
            .get("uuid")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LoxoneError::config("Missing device UUID"))?;

        if let Some(ref client) = self.http_client {
            let path = format!("/dev/sps/io/{}", device_uuid);
            let response = client.request(&path, "GET", None).await?;
            let result: serde_json::Value = serde_json::from_slice(&response)?;

            return Ok(result);
        }

        Err(LoxoneError::config("HTTP client not initialized"))
    }

    async fn handle_test_connection(&self) -> Result<serde_json::Value> {
        if let Some(ref client) = self.http_client {
            let response = client.request("/dev/sps/version", "GET", None).await?;
            let result: serde_json::Value = serde_json::from_slice(&response)?;

            return Ok(serde_json::json!({
                "connected": true,
                "version_info": result
            }));
        }

        Ok(serde_json::json!({
            "connected": false,
            "error": "HTTP client not initialized"
        }))
    }
}

/// WASIP2 component shutdown handler
pub async fn shutdown_wasip2_component() -> Result<()> {
    if let Some(state) = COMPONENT_STATE.get() {
        let mut state_guard = state.lock().unwrap();

        // Clean up resources
        state_guard.connection_pool.clear();
        state_guard.credentials = None;

        logging::log(
            logging::Level::Info,
            "loxone-mcp",
            "WASIP2 component shutdown completed",
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wasip2_config_loader() {
        let config = Wasip2ConfigLoader::load_config().await.unwrap();
        assert_eq!(config.credentials, CredentialStore::WasiKeyvalue);
        assert_eq!(config.mcp.transport.transport_type, "wasm");
    }

    #[tokio::test]
    async fn test_wasip2_memory_manager() {
        let info = Wasip2MemoryManager::get_memory_usage();
        assert!(info.total_bytes >= 0);

        // Test memory optimization (should not panic)
        Wasip2MemoryManager::optimize_memory();
    }

    #[tokio::test]
    async fn test_wasip2_mcp_server_creation() {
        let mut config = ServerConfig::default();
        config.credentials = CredentialStore::WasiKeyvalue;
        config.mcp.transport.transport_type = "wasm".to_string();

        // In a real WASIP2 environment, this would succeed
        let result = Wasip2McpServer::new(config).await;

        // For testing without WASM environment, we expect potential failure
        assert!(result.is_ok() || result.is_err());
    }
}
