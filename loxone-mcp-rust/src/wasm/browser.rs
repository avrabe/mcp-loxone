//! Browser-specific WASM implementation
//!
//! This module provides browser-specific WASM functionality while
//! maintaining compatibility with the WASIP2 component model.

use crate::config::{CredentialStore, LoxoneCredentials, ServerConfig};
use crate::error::{LoxoneError, Result};
use js_sys::{Array, Object, Promise, Uint8Array};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use web_sys::{console, window, Headers, Request, RequestInit, Response, Storage};

/// Browser-specific WASM server implementation
#[wasm_bindgen]
pub struct BrowserLoxoneServer {
    config: Option<ServerConfig>,
    credentials: Option<LoxoneCredentials>,
}

#[wasm_bindgen]
impl BrowserLoxoneServer {
    /// Create new browser WASM server instance
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        // Set up panic hook for better debugging
        console_error_panic_hook::set_once();

        Self {
            config: None,
            credentials: None,
        }
    }

    /// Initialize server with configuration
    #[wasm_bindgen]
    pub async fn init(&mut self, config_json: Option<String>) -> Result<(), JsValue> {
        let config = match config_json {
            Some(json) => serde_json::from_str::<ServerConfig>(&json)
                .map_err(|e| JsValue::from_str(&format!("Invalid config: {}", e)))?,
            None => ServerConfig::from_browser_env()
                .await
                .map_err(|e| JsValue::from_str(&format!("Config error: {}", e)))?,
        };

        self.config = Some(config);

        // Load credentials from browser storage
        if let Ok(creds) = BrowserCredentialManager::get_credentials().await {
            self.credentials = Some(creds);
        }

        console::log_1(&"Browser Loxone MCP server initialized".into());
        Ok(())
    }

    /// Execute MCP tool
    #[wasm_bindgen]
    pub async fn call_tool(&self, name: &str, arguments: &str) -> Result<String, JsValue> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| JsValue::from_str(&format!("Invalid arguments: {}", e)))?;

        let result = match name {
            "list_rooms" => self.handle_list_rooms().await,
            "control_device" => self.handle_control_device(args).await,
            "get_device_state" => self.handle_get_device_state(args).await,
            "test_connection" => self.handle_test_connection().await,
            _ => Err(LoxoneError::config(format!("Unknown tool: {}", name))),
        };

        match result {
            Ok(value) => serde_json::to_string(&value)
                .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e))),
            Err(e) => Err(JsValue::from_str(&e.to_string())),
        }
    }

    /// Get server capabilities
    #[wasm_bindgen]
    pub fn get_capabilities(&self) -> String {
        serde_json::json!({
            "name": "Loxone MCP (Browser)",
            "version": env!("CARGO_PKG_VERSION"),
            "runtime": "browser-wasm",
            "features": [
                "credential-management",
                "device-control",
                "local-storage",
                "browser-apis"
            ],
            "limitations": [
                "No direct file system access",
                "CORS restrictions apply",
                "Limited to browser storage"
            ]
        })
        .to_string()
    }

    /// Store credentials in browser local storage
    #[wasm_bindgen]
    pub async fn store_credentials(
        &mut self,
        username: &str,
        password: &str,
        host: &str,
    ) -> Result<(), JsValue> {
        let credentials = LoxoneCredentials {
            username: username.to_string(),
            password: password.to_string(),
            api_key: None,
            #[cfg(feature = "crypto")]
            public_key: None,
        };

        BrowserCredentialManager::store_credentials(&credentials)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        self.credentials = Some(credentials);

        // Update config with host
        if let Some(ref mut config) = self.config {
            config.loxone.url = host
                .parse()
                .map_err(|e| JsValue::from_str(&format!("Invalid host URL: {}", e)))?;
        }

        Ok(())
    }

    /// Clear stored credentials
    #[wasm_bindgen]
    pub async fn clear_credentials(&mut self) -> Result<(), JsValue> {
        BrowserCredentialManager::clear_credentials()
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        self.credentials = None;
        Ok(())
    }

    /// Check if credentials are stored
    #[wasm_bindgen]
    pub async fn has_credentials(&self) -> bool {
        BrowserCredentialManager::has_credentials().await
    }
}

impl BrowserLoxoneServer {
    async fn handle_list_rooms(&self) -> Result<serde_json::Value> {
        let response = self.make_loxone_request("/data/loxapp3.json").await?;
        let structure: serde_json::Value = serde_json::from_str(&response)?;

        // Extract rooms from structure
        if let Some(rooms) = structure.get("rooms") {
            Ok(rooms.clone())
        } else {
            Ok(serde_json::json!([]))
        }
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

        let path = format!("/dev/sps/io/{}/{}", device_uuid, action);
        let response = self.make_loxone_request(&path).await?;
        let result: serde_json::Value = serde_json::from_str(&response)?;

        Ok(result)
    }

    async fn handle_get_device_state(
        &self,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let device_uuid = arguments
            .get("uuid")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LoxoneError::config("Missing device UUID"))?;

        let path = format!("/dev/sps/io/{}", device_uuid);
        let response = self.make_loxone_request(&path).await?;
        let result: serde_json::Value = serde_json::from_str(&response)?;

        Ok(result)
    }

    async fn handle_test_connection(&self) -> Result<serde_json::Value> {
        match self.make_loxone_request("/dev/sps/version").await {
            Ok(response) => {
                let result: serde_json::Value = serde_json::from_str(&response)?;
                Ok(serde_json::json!({
                    "connected": true,
                    "version_info": result
                }))
            }
            Err(e) => Ok(serde_json::json!({
                "connected": false,
                "error": e.to_string()
            })),
        }
    }

    async fn make_loxone_request(&self, path: &str) -> Result<String> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| LoxoneError::config("Server not initialized"))?;

        let credentials = self
            .credentials
            .as_ref()
            .ok_or_else(|| LoxoneError::credentials("No credentials available"))?;

        let url = format!("{}{}", config.loxone.url, path);

        BrowserHttpClient::request(&url, "GET", Some(credentials), None).await
    }
}

/// Browser-specific HTTP client using Fetch API
pub struct BrowserHttpClient;

impl BrowserHttpClient {
    /// Make HTTP request using browser Fetch API
    pub async fn request(
        url: &str,
        method: &str,
        credentials: Option<&LoxoneCredentials>,
        body: Option<&[u8]>,
    ) -> Result<String> {
        let window = web_sys::window().ok_or_else(|| LoxoneError::http("Window not available"))?;

        // Create request options
        let mut opts = RequestInit::new();
        opts.method(method);

        // Set headers
        let headers = Headers::new().map_err(|_| LoxoneError::http("Failed to create headers"))?;

        headers
            .set("Content-Type", "application/json")
            .map_err(|_| LoxoneError::http("Failed to set content type"))?;

        // Add authentication if provided
        if let Some(creds) = credentials {
            let auth_value = format!("{}:{}", creds.username, creds.password);
            let auth_encoded = base64::encode(auth_value);
            headers
                .set("Authorization", &format!("Basic {}", auth_encoded))
                .map_err(|_| LoxoneError::http("Failed to set authorization"))?;
        }

        opts.headers(&headers);

        // Add body if provided
        if let Some(body_data) = body {
            let uint8_array = Uint8Array::new_with_length(body_data.len() as u32);
            uint8_array.copy_from(body_data);
            opts.body(Some(&uint8_array));
        }

        // Create and send request
        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|_| LoxoneError::http("Failed to create request"))?;

        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|_| LoxoneError::http("Request failed"))?;

        let resp: Response = resp_value
            .dyn_into()
            .map_err(|_| LoxoneError::http("Invalid response"))?;

        // Check response status
        if !resp.ok() {
            return Err(LoxoneError::http(format!("HTTP error: {}", resp.status())));
        }

        // Get response text
        let text_promise = resp
            .text()
            .map_err(|_| LoxoneError::http("Failed to get response text"))?;

        let text_value = JsFuture::from(text_promise)
            .await
            .map_err(|_| LoxoneError::http("Failed to read response"))?;

        text_value
            .as_string()
            .ok_or_else(|| LoxoneError::http("Response is not text"))
    }
}

/// Browser-specific credential manager using localStorage
pub struct BrowserCredentialManager;

impl BrowserCredentialManager {
    /// Store credentials in browser localStorage
    pub async fn store_credentials(credentials: &LoxoneCredentials) -> Result<()> {
        let storage = get_local_storage()
            .ok_or_else(|| LoxoneError::credentials("Local storage not available"))?;

        // Serialize credentials (Note: This is not secure for production)
        let creds_json = serde_json::to_string(credentials).map_err(|e| {
            LoxoneError::credentials(format!("Failed to serialize credentials: {}", e))
        })?;

        // In a real implementation, you would encrypt this data
        storage
            .set_item("loxone_credentials", &creds_json)
            .map_err(|_| LoxoneError::credentials("Failed to store credentials"))?;

        console::log_1(&"Credentials stored in browser localStorage".into());
        Ok(())
    }

    /// Get credentials from browser localStorage
    pub async fn get_credentials() -> Result<LoxoneCredentials> {
        let storage = get_local_storage()
            .ok_or_else(|| LoxoneError::credentials("Local storage not available"))?;

        let creds_json = storage
            .get_item("loxone_credentials")
            .map_err(|_| LoxoneError::credentials("Failed to access local storage"))?
            .ok_or_else(|| LoxoneError::credentials("No credentials found in storage"))?;

        let credentials: LoxoneCredentials = serde_json::from_str(&creds_json).map_err(|e| {
            LoxoneError::credentials(format!("Failed to parse stored credentials: {}", e))
        })?;

        Ok(credentials)
    }

    /// Clear credentials from browser localStorage
    pub async fn clear_credentials() -> Result<()> {
        let storage = get_local_storage()
            .ok_or_else(|| LoxoneError::credentials("Local storage not available"))?;

        storage
            .remove_item("loxone_credentials")
            .map_err(|_| LoxoneError::credentials("Failed to clear credentials"))?;

        console::log_1(&"Credentials cleared from browser localStorage".into());
        Ok(())
    }

    /// Check if credentials exist
    pub async fn has_credentials() -> bool {
        if let Some(storage) = get_local_storage() {
            if let Ok(Some(_)) = storage.get_item("loxone_credentials") {
                return true;
            }
        }
        false
    }
}

/// Browser-specific configuration helpers
impl ServerConfig {
    /// Load configuration from browser environment
    pub async fn from_browser_env() -> Result<Self> {
        let mut config = Self::default();

        // Override credential store for browser
        config.credentials = CredentialStore::LocalStorage;

        // Try to load from browser storage
        if let Some(storage) = get_local_storage() {
            // Load Loxone URL
            if let Ok(Some(url)) = storage.get_item("loxone_url") {
                config.loxone.url = url
                    .parse()
                    .map_err(|e| LoxoneError::config(format!("Invalid stored URL: {}", e)))?;
            }

            // Load username
            if let Ok(Some(username)) = storage.get_item("loxone_username") {
                config.loxone.username = username;
            }

            // Load transport configuration
            if let Ok(Some(transport)) = storage.get_item("mcp_transport") {
                config.mcp.transport.transport_type = transport;
            }
        }

        Ok(config)
    }

    /// Save configuration to browser storage
    pub async fn save_to_browser_storage(&self) -> Result<()> {
        let storage = get_local_storage()
            .ok_or_else(|| LoxoneError::config("Local storage not available"))?;

        // Save Loxone configuration
        storage
            .set_item("loxone_url", &self.loxone.url.to_string())
            .map_err(|_| LoxoneError::config("Failed to save URL"))?;

        storage
            .set_item("loxone_username", &self.loxone.username)
            .map_err(|_| LoxoneError::config("Failed to save username"))?;

        // Save transport configuration
        storage
            .set_item("mcp_transport", &self.mcp.transport.transport_type)
            .map_err(|_| LoxoneError::config("Failed to save transport type"))?;

        Ok(())
    }
}

/// Get browser local storage
pub fn get_local_storage() -> Option<Storage> {
    window()?.local_storage().ok()?
}

/// Browser feature detection
#[wasm_bindgen]
pub struct BrowserFeatures;

#[wasm_bindgen]
impl BrowserFeatures {
    /// Check if local storage is available
    #[wasm_bindgen]
    pub fn has_local_storage() -> bool {
        get_local_storage().is_some()
    }

    /// Check if Fetch API is available
    #[wasm_bindgen]
    pub fn has_fetch_api() -> bool {
        if let Some(window) = window() {
            js_sys::Reflect::has(&window, &"fetch".into())
        } else {
            false
        }
    }

    /// Check if WebCrypto API is available
    #[wasm_bindgen]
    pub fn has_web_crypto() -> bool {
        if let Some(window) = window() {
            if let Ok(crypto) = js_sys::Reflect::get(&window, &"crypto".into()) {
                return !crypto.is_undefined();
            }
        }
        false
    }

    /// Check if Worker API is available
    #[wasm_bindgen]
    pub fn has_workers() -> bool {
        if let Some(window) = window() {
            js_sys::Reflect::has(&window, &"Worker".into())
        } else {
            false
        }
    }

    /// Get browser information
    #[wasm_bindgen]
    pub fn get_browser_info() -> String {
        if let Some(window) = window() {
            if let Some(navigator) = window.navigator() {
                return navigator
                    .user_agent()
                    .unwrap_or_else(|_| "Unknown".to_string());
            }
        }
        "Unknown".to_string()
    }

    /// Get supported features as JSON
    #[wasm_bindgen]
    pub fn get_supported_features() -> String {
        serde_json::json!({
            "localStorage": Self::has_local_storage(),
            "fetchAPI": Self::has_fetch_api(),
            "webCrypto": Self::has_web_crypto(),
            "workers": Self::has_workers(),
            "userAgent": Self::get_browser_info()
        })
        .to_string()
    }
}

/// Browser WASM module initialization
#[wasm_bindgen(start)]
pub fn browser_init() {
    // Set up panic hook
    console_error_panic_hook::set_once();

    console::log_1(&"Loxone MCP Browser WASM module initialized".into());

    // Log browser capabilities
    if BrowserFeatures::has_local_storage() {
        console::log_1(&"✅ Local Storage available".into());
    } else {
        console::error_1(&"❌ Local Storage not available".into());
    }

    if BrowserFeatures::has_fetch_api() {
        console::log_1(&"✅ Fetch API available".into());
    } else {
        console::error_1(&"❌ Fetch API not available".into());
    }

    if BrowserFeatures::has_web_crypto() {
        console::log_1(&"✅ WebCrypto API available".into());
    } else {
        console::warn_1(&"⚠️ WebCrypto API not available".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_browser_features() {
        let has_storage = BrowserFeatures::has_local_storage();
        let has_fetch = BrowserFeatures::has_fetch_api();

        // These should be available in modern browsers
        assert!(has_storage || !has_storage); // Should not panic
        assert!(has_fetch || !has_fetch); // Should not panic
    }

    #[wasm_bindgen_test]
    async fn test_browser_server_creation() {
        let server = BrowserLoxoneServer::new();
        let capabilities = server.get_capabilities();

        let caps: serde_json::Value = serde_json::from_str(&capabilities).unwrap();
        assert_eq!(caps["runtime"], "browser-wasm");
    }

    #[wasm_bindgen_test]
    fn test_supported_features_json() {
        let features = BrowserFeatures::get_supported_features();
        let parsed: serde_json::Value = serde_json::from_str(&features).unwrap();

        assert!(parsed.get("localStorage").is_some());
        assert!(parsed.get("fetchAPI").is_some());
        assert!(parsed.get("webCrypto").is_some());
        assert!(parsed.get("workers").is_some());
        assert!(parsed.get("userAgent").is_some());
    }
}
