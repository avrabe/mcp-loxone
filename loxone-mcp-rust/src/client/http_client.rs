//! HTTP client implementation for Loxone Miniserver communication
//!
//! This module provides HTTP-based communication with Loxone Generation 1
//! Miniservers using basic authentication and REST API calls.

use crate::client::{
    connection_pool::{ConnectionPool, PoolBuilder},
    ClientContext, LoxoneClient, LoxoneDevice, LoxoneResponse, LoxoneStructure,
};
use crate::config::{credentials::LoxoneCredentials, LoxoneConfig};
use crate::error::{LoxoneError, Result};
use async_trait::async_trait;
use base64::Engine;
use reqwest::{Client, ClientBuilder};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use url::Url;

/// HTTP client for Loxone Miniserver
pub struct LoxoneHttpClient {
    /// HTTP client instance
    client: Client,

    /// Base URL for Miniserver
    base_url: Url,

    /// Authentication credentials
    #[allow(dead_code)]
    credentials: LoxoneCredentials,

    /// Configuration
    config: LoxoneConfig,

    /// Shared context for caching
    context: ClientContext,

    /// Connection state
    connected: bool,

    /// Connection pool for resource management
    connection_pool: Arc<ConnectionPool>,
}

impl LoxoneHttpClient {
    /// Create a new HTTP client
    pub async fn new(config: LoxoneConfig, credentials: LoxoneCredentials) -> Result<Self> {
        // Build HTTP client with appropriate settings
        let mut client_builder = ClientBuilder::new()
            .timeout(config.timeout)
            .user_agent(format!("loxone-mcp-rust/{}", env!("CARGO_PKG_VERSION")));

        // Handle SSL verification
        if !config.verify_ssl {
            warn!("SSL verification disabled - this is insecure for production use");
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }

        // Add basic authentication via header
        let auth_header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!(
                "{username}:{password}",
                username = credentials.username,
                password = credentials.password
            ))
        );
        let mut default_headers = reqwest::header::HeaderMap::new();
        let header_value = reqwest::header::HeaderValue::from_str(&auth_header).map_err(|e| {
            LoxoneError::invalid_input(format!("Invalid authorization header: {e}"))
        })?;
        default_headers.insert(reqwest::header::AUTHORIZATION, header_value);
        client_builder = client_builder.default_headers(default_headers);

        let client = client_builder
            .build()
            .map_err(|e| LoxoneError::connection(format!("Failed to build HTTP client: {e}")))?;

        // Create connection pool based on config
        let connection_pool = Arc::new(
            PoolBuilder::new()
                .max_connections(config.max_connections.unwrap_or(10))
                .connection_timeout(config.timeout)
                .idle_timeout(Duration::from_secs(300))
                .max_lifetime(Duration::from_secs(3600))
                .build(),
        );

        Ok(Self {
            client,
            base_url: config.url.clone(),
            credentials,
            config,
            context: ClientContext::new(),
            connected: false,
            connection_pool,
        })
    }

    /// Build URL for API endpoint
    fn build_url(&self, path: &str) -> Result<Url> {
        self.base_url
            .join(path)
            .map_err(|e| LoxoneError::connection(format!("Invalid URL path {path}: {e}")))
    }

    /// Execute HTTP request with retry logic and connection pooling
    async fn execute_request(&self, url: Url) -> Result<reqwest::Response> {
        // Acquire connection permit from pool
        let _permit = self.connection_pool.acquire().await?;

        let mut last_error = None;

        for attempt in 1..=self.config.max_retries {
            debug!("HTTP request attempt {attempt} to {url}");

            match self.client.get(url.clone()).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        debug!("HTTP request successful: {}", response.status());
                        return Ok(response);
                    }
                    let status = response.status();
                    let response_text = response.text().await.unwrap_or_default();
                    let error_msg = format!("HTTP error {status}: {response_text}");

                    last_error = Some(match status.as_u16() {
                        401 => LoxoneError::authentication(error_msg),
                        403 => LoxoneError::authentication("Access denied"),
                        404 => LoxoneError::connection("Endpoint not found"),
                        500..=599 => LoxoneError::connection(format!("Server error: {error_msg}")),
                        _ => LoxoneError::connection(error_msg),
                    });
                }
                Err(e) => {
                    let error_msg = format!("HTTP request failed: {e}");
                    last_error = Some(if e.is_timeout() {
                        LoxoneError::timeout(error_msg)
                    } else if e.is_connect() {
                        LoxoneError::connection(error_msg)
                    } else {
                        LoxoneError::Http(e)
                    });
                }
            }

            if attempt < self.config.max_retries {
                let delay = Duration::from_millis(100 * u64::from(attempt));
                debug!("Retrying HTTP request in {delay:?}");
                tokio::time::sleep(delay).await;
            }
        }

        // Record error in connection pool
        if last_error.is_some() {
            self.connection_pool.record_error().await;
        }

        Err(last_error.unwrap_or_else(|| LoxoneError::connection("All retry attempts failed")))
    }

    /// Parse Loxone response format
    fn parse_loxone_response(text: &str) -> LoxoneResponse {
        // Try parsing as JSON first
        if let Ok(json_response) = serde_json::from_str::<LoxoneResponse>(text) {
            return json_response;
        }

        // Try parsing as simple value response
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
            return LoxoneResponse { code: 200, value };
        }

        // Fallback to plain text response
        LoxoneResponse {
            code: 200,
            value: serde_json::Value::String(text.to_string()),
        }
    }

    /// Get public context for external access
    #[must_use]
    pub fn context(&self) -> &ClientContext {
        &self.context
    }

    /// Get connection pool statistics
    pub async fn pool_stats(&self) -> crate::client::connection_pool::PoolStats {
        self.connection_pool.stats().await
    }

    /// Get connection pool health
    pub async fn pool_health(&self) -> crate::client::connection_pool::PoolHealth {
        self.connection_pool.health_check().await
    }
}

#[async_trait]
impl LoxoneClient for LoxoneHttpClient {
    async fn connect(&mut self) -> Result<()> {
        info!("Connecting to Loxone Miniserver at {}", self.base_url);

        // Test connection with a simple health check
        match self.health_check().await {
            Ok(true) => {
                self.connected = true;
                *self.context.connected.write().await = true;

                // Load structure on successful connection
                match self.get_structure().await {
                    Ok(structure) => {
                        info!("Structure loaded successfully");
                        self.context.update_structure(structure).await?;

                        let capabilities = self.context.capabilities.read().await;
                        info!("System capabilities detected:");
                        info!("  Lighting: {} devices", capabilities.light_count);
                        info!("  Blinds: {} devices", capabilities.blind_count);
                        info!("  Climate: {} devices", capabilities.climate_count);
                        info!("  Sensors: {} devices", capabilities.sensor_count);
                    }
                    Err(e) => {
                        warn!("Failed to load structure: {e}");
                    }
                }

                info!("✅ Connected to Loxone Miniserver");
                Ok(())
            }
            Ok(false) => Err(LoxoneError::connection("Health check failed")),
            Err(e) => {
                error!("Connection failed: {e}");
                Err(e)
            }
        }
    }

    async fn is_connected(&self) -> Result<bool> {
        Ok(self.connected && *self.context.connected.read().await)
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        *self.context.connected.write().await = false;
        info!("Disconnected from Loxone Miniserver");
        Ok(())
    }

    async fn send_command(&self, uuid: &str, command: &str) -> Result<LoxoneResponse> {
        if !self.connected {
            return Err(LoxoneError::connection("Not connected to Miniserver"));
        }

        debug!("Sending command '{command}' to device {uuid}");

        // Build command URL: /jdev/sps/io/{uuid}/{command}
        let url = self.build_url(&format!("jdev/sps/io/{uuid}/{command}"))?;

        let response = self.execute_request(url).await?;
        let text = response
            .text()
            .await
            .map_err(|e| LoxoneError::connection(format!("Failed to read response: {e}")))?;

        let loxone_response = Self::parse_loxone_response(&text);

        if loxone_response.code != 200 {
            return Err(LoxoneError::device_control(format!(
                "Command failed with code {}: {:?}",
                loxone_response.code, loxone_response.value
            )));
        }

        debug!("Command successful: {:?}", loxone_response.value);
        Ok(loxone_response)
    }

    async fn get_structure(&self) -> Result<LoxoneStructure> {
        debug!("Fetching structure file");

        // Get structure file: /data/LoxAPP3.json
        let url = self.build_url("data/LoxAPP3.json")?;

        let response = self.execute_request(url).await?;
        let text = response
            .text()
            .await
            .map_err(|e| LoxoneError::connection(format!("Failed to read structure: {e}")))?;

        // Parse structure JSON
        let structure: LoxoneStructure = serde_json::from_str(&text).map_err(LoxoneError::Json)?;

        debug!(
            "Structure loaded: {} controls, {} rooms",
            structure.controls.len(),
            structure.rooms.len()
        );

        Ok(structure)
    }

    async fn get_device_states(
        &self,
        uuids: &[String],
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut states = HashMap::new();

        // For HTTP client, we need to query each device individually
        // In a real implementation, this could be optimized with batch requests
        for uuid in uuids {
            match self.send_command(uuid, "state").await {
                Ok(response) => {
                    states.insert(uuid.clone(), response.value);
                }
                Err(e) => {
                    warn!("Failed to get state for device {uuid}: {e}");
                    // Continue with other devices
                }
            }
        }

        Ok(states)
    }

    async fn get_system_info(&self) -> Result<serde_json::Value> {
        debug!("Fetching system information");

        // Get system info from API config: /jdev/cfg/api
        let url = self.build_url("jdev/cfg/api")?;

        let response = self.execute_request(url).await?;
        let text = response
            .text()
            .await
            .map_err(|e| LoxoneError::connection(format!("Failed to read system info: {e}")))?;

        let loxone_response = Self::parse_loxone_response(&text);

        if loxone_response.code != 200 {
            return Err(LoxoneError::connection(format!(
                "System info request failed: {:?}",
                loxone_response.value
            )));
        }

        Ok(loxone_response.value)
    }

    async fn health_check(&self) -> Result<bool> {
        debug!("Performing health check");

        match self.get_system_info().await {
            Ok(_) => {
                debug!("Health check passed");
                Ok(true)
            }
            Err(e) => {
                debug!("Health check failed: {e}");
                // Don't propagate error for health checks
                Ok(false)
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl LoxoneHttpClient {
    /// Get all devices from cache or fresh from server
    pub async fn get_all_devices(&self) -> Result<Vec<LoxoneDevice>> {
        // Check if we need to refresh the structure
        if self.context.needs_refresh(self.config.timeout).await {
            warn!("Structure cache expired, refreshing...");
            // In a mutable reference context, we would refresh here
            // For now, return cached data
        }

        let devices = self.context.devices.read().await;
        Ok(devices.values().cloned().collect())
    }

    /// Get devices by type (e.g., `LightController`, `Jalousie`)
    pub async fn get_devices_by_type(&self, device_type: &str) -> Result<Vec<LoxoneDevice>> {
        let devices = self.context.devices.read().await;
        Ok(devices
            .values()
            .filter(|device| device.device_type == device_type)
            .cloned()
            .collect())
    }

    /// Control multiple devices in parallel
    pub async fn control_devices_parallel(
        &self,
        commands: Vec<(String, String)>, // (uuid, command) pairs
    ) -> Result<Vec<Result<LoxoneResponse>>> {
        if !self.connected {
            return Err(LoxoneError::connection("Not connected to Miniserver"));
        }

        // Execute commands sequentially (parallel would need Arc<Self>)
        let mut results = Vec::new();
        for (uuid, command) in commands {
            let result = self.send_command(&uuid, &command).await;
            results.push(result);
        }
        Ok(results)
    }

    /// Get structure using streaming parser for large files
    pub async fn get_structure_streaming(&self) -> Result<LoxoneStructure> {
        use crate::client::streaming_parser::StreamingStructureParser;

        debug!("Fetching structure file with streaming parser");

        // Get structure file: /data/LoxAPP3.json
        let url = self.build_url("data/LoxAPP3.json")?;

        let response = self.execute_request(url).await?;

        // Use streaming parser
        let mut parser = StreamingStructureParser::new();
        let structure = parser.parse_from_response(response).await?;

        debug!(
            "Structure loaded via streaming: {} controls, {} rooms",
            structure.controls.len(),
            structure.rooms.len()
        );

        Ok(structure)
    }

    /// Get structure with custom streaming configuration
    pub async fn get_structure_streaming_with_config(
        &self,
        config: crate::client::streaming_parser::StreamingParserConfig,
    ) -> Result<LoxoneStructure> {
        use crate::client::streaming_parser::StreamingStructureParser;

        debug!("Fetching structure file with custom streaming config");

        let url = self.build_url("data/LoxAPP3.json")?;
        let response = self.execute_request(url).await?;

        let mut parser = StreamingStructureParser::with_config(config);
        let structure = parser.parse_from_response(response).await?;

        debug!(
            "Structure loaded via custom streaming: {} controls, {} rooms",
            structure.controls.len(),
            structure.rooms.len()
        );

        Ok(structure)
    }

    /// Get structure with progress reporting
    pub async fn get_structure_with_progress(
        &self,
    ) -> Result<(
        LoxoneStructure,
        tokio::sync::mpsc::UnboundedReceiver<crate::client::streaming_parser::ParseProgress>,
    )> {
        use crate::client::streaming_parser::StreamingStructureParser;

        debug!("Fetching structure file with progress reporting");

        let url = self.build_url("data/LoxAPP3.json")?;
        let response = self.execute_request(url).await?;

        let mut parser = StreamingStructureParser::new();
        let progress_rx = parser.with_progress_reporting();
        let structure = parser.parse_from_response(response).await?;

        Ok((structure, progress_rx))
    }

    /// Refresh structure cache
    pub async fn refresh_structure(&self) -> Result<()> {
        let structure = self.get_structure().await?;
        self.context.update_structure(structure).await?;
        info!("Structure cache refreshed");
        Ok(())
    }

    /// Refresh structure cache with streaming parser
    pub async fn refresh_structure_streaming(&self) -> Result<()> {
        let structure = self.get_structure_streaming().await?;
        self.context.update_structure(structure).await?;
        info!("Structure cache refreshed with streaming parser");
        Ok(())
    }
}
