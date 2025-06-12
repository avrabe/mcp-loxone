//! WASM-specific functionality and browser integration
//!
//! This module provides WASM-specific utilities, browser storage integration,
//! and WASM runtime optimizations.

use crate::config::{ServerConfig, LoxoneCredentials, CredentialStore};
use crate::error::{LoxoneError, Result};
use wasm_bindgen::prelude::*;
use web_sys::{console, window, Storage};
use serde_json;

/// WASM-specific server entry point
#[wasm_bindgen]
pub struct WasmLoxoneServer {
    inner: Option<crate::LoxoneMcpServer>,
}

#[wasm_bindgen]
impl WasmLoxoneServer {
    /// Create new WASM server instance
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        // Set up panic hook for better debugging
        console_error_panic_hook::set_once();
        
        Self {
            inner: None,
        }
    }
    
    /// Initialize server with configuration
    #[wasm_bindgen]
    pub async fn init(&mut self, config_json: Option<String>) -> Result<(), JsValue> {
        let config = match config_json {
            Some(json) => {
                serde_json::from_str::<ServerConfig>(&json)
                    .map_err(|e| JsValue::from_str(&format!("Invalid config: {}", e)))?
            }
            None => {
                ServerConfig::from_wasm_env().await
                    .map_err(|e| JsValue::from_str(&format!("Config error: {}", e)))?
            }
        };
        
        let server = crate::LoxoneMcpServer::new(config).await
            .map_err(|e| JsValue::from_str(&format!("Server init error: {}", e)))?;
        
        self.inner = Some(server);
        Ok(())
    }
    
    /// Start the server
    #[wasm_bindgen]
    pub async fn start(&self) -> Result<(), JsValue> {
        match &self.inner {
            Some(server) => {
                server.run().await
                    .map_err(|e| JsValue::from_str(&format!("Server run error: {}", e)))
            }
            None => Err(JsValue::from_str("Server not initialized"))
        }
    }
    
    /// Get server statistics
    #[wasm_bindgen]
    pub async fn get_stats(&self) -> Result<String, JsValue> {
        match &self.inner {
            Some(server) => {
                let stats = server.get_statistics().await;
                serde_json::to_string(&stats)
                    .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
            }
            None => Err(JsValue::from_str("Server not initialized"))
        }
    }
}

/// WASM-specific configuration helpers
impl ServerConfig {
    /// Load configuration from WASM environment
    pub async fn from_wasm_env() -> Result<Self> {
        let mut config = Self::default();
        
        // Override credential store for WASM
        config.credentials = CredentialStore::LocalStorage;
        
        // Try to load from browser storage
        if let Some(storage) = get_local_storage() {
            // Load Loxone URL
            if let Ok(Some(url)) = storage.get_item("loxone_url") {
                config.loxone.url = url.parse()
                    .map_err(|e| LoxoneError::config(format!("Invalid stored URL: {}", e)))?;
            }
            
            // Load username
            if let Ok(Some(username)) = storage.get_item("loxone_username") {
                config.loxone.username = username;
            }
            
            // Load MCP configuration
            if let Ok(Some(transport)) = storage.get_item("mcp_transport") {
                config.mcp.transport.transport_type = transport;
            }
            
            if let Ok(Some(port_str)) = storage.get_item("mcp_port") {
                if let Ok(port) = port_str.parse::<u16>() {
                    config.mcp.transport.port = Some(port);
                }
            }
        }
        
        Ok(config)
    }
    
    /// Save configuration to browser storage
    pub async fn save_to_wasm_storage(&self) -> Result<()> {
        let storage = get_local_storage()
            .ok_or_else(|| LoxoneError::config("Local storage not available"))?;
        
        // Save Loxone configuration
        storage.set_item("loxone_url", &self.loxone.url.to_string())
            .map_err(|_| LoxoneError::config("Failed to save URL"))?;
        
        storage.set_item("loxone_username", &self.loxone.username)
            .map_err(|_| LoxoneError::config("Failed to save username"))?;
        
        // Save MCP configuration
        storage.set_item("mcp_transport", &self.mcp.transport.transport_type)
            .map_err(|_| LoxoneError::config("Failed to save transport type"))?;
        
        if let Some(port) = self.mcp.transport.port {
            storage.set_item("mcp_port", &port.to_string())
                .map_err(|_| LoxoneError::config("Failed to save port"))?;
        }
        
        Ok(())
    }
}

/// Get browser local storage
pub fn get_local_storage() -> Option<Storage> {
    window()?.local_storage().ok()?
}

/// Log to browser console
#[wasm_bindgen]
pub fn log_to_console(message: &str) {
    console::log_1(&format!("Loxone MCP: {}", message).into());
}

/// Log error to browser console
#[wasm_bindgen]
pub fn log_error_to_console(message: &str) {
    console::error_1(&format!("Loxone MCP Error: {}", message).into());
}

/// WASM-specific credential manager implementation
pub struct WasmCredentialManager;

impl WasmCredentialManager {
    /// Store credentials in browser local storage
    pub async fn store_credentials(credentials: &LoxoneCredentials) -> Result<()> {
        let storage = get_local_storage()
            .ok_or_else(|| LoxoneError::credentials("Local storage not available"))?;
        
        // Store credentials as JSON (Note: This is not secure for production)
        let creds_json = serde_json::to_string(credentials)
            .map_err(|e| LoxoneError::credentials(format!("Failed to serialize credentials: {}", e)))?;
        
        // In a real implementation, you would encrypt this data
        storage.set_item("loxone_credentials", &creds_json)
            .map_err(|_| LoxoneError::credentials("Failed to store credentials"))?;
        
        log_to_console("Credentials stored in browser storage");
        Ok(())
    }
    
    /// Get credentials from browser local storage
    pub async fn get_credentials() -> Result<LoxoneCredentials> {
        let storage = get_local_storage()
            .ok_or_else(|| LoxoneError::credentials("Local storage not available"))?;
        
        let creds_json = storage.get_item("loxone_credentials")
            .map_err(|_| LoxoneError::credentials("Failed to access local storage"))?
            .ok_or_else(|| LoxoneError::credentials("No credentials found in storage"))?;
        
        let credentials: LoxoneCredentials = serde_json::from_str(&creds_json)
            .map_err(|e| LoxoneError::credentials(format!("Failed to parse stored credentials: {}", e)))?;
        
        Ok(credentials)
    }
    
    /// Clear credentials from browser local storage
    pub async fn clear_credentials() -> Result<()> {
        let storage = get_local_storage()
            .ok_or_else(|| LoxoneError::credentials("Local storage not available"))?;
        
        storage.remove_item("loxone_credentials")
            .map_err(|_| LoxoneError::credentials("Failed to clear credentials"))?;
        
        log_to_console("Credentials cleared from browser storage");
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

/// WASM-specific utilities
#[wasm_bindgen]
pub struct WasmUtils;

#[wasm_bindgen]
impl WasmUtils {
    /// Get current timestamp as ISO string
    #[wasm_bindgen]
    pub fn current_timestamp() -> String {
        chrono::Utc::now().to_rfc3339()
    }
    
    /// Format bytes to human-readable size
    #[wasm_bindgen]
    pub fn format_bytes(bytes: u32) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;
        
        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }
        
        format!("{:.1} {}", size, UNITS[unit_index])
    }
    
    /// Validate URL format
    #[wasm_bindgen]
    pub fn validate_url(url: &str) -> bool {
        url::Url::parse(url).is_ok()
    }
    
    /// Generate random ID
    #[wasm_bindgen]
    pub fn generate_id() -> String {
        format!("wasm-{}", js_sys::Math::random().to_string().replace("0.", ""))
    }
}

/// Performance monitoring for WASM
#[wasm_bindgen]
pub struct WasmPerformanceMonitor {
    start_time: f64,
}

#[wasm_bindgen]
impl WasmPerformanceMonitor {
    /// Create new performance monitor
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            start_time: js_sys::Date::now(),
        }
    }
    
    /// Get elapsed time in milliseconds
    #[wasm_bindgen]
    pub fn elapsed_ms(&self) -> f64 {
        js_sys::Date::now() - self.start_time
    }
    
    /// Log performance measurement
    #[wasm_bindgen]
    pub fn log_performance(&self, operation: &str) {
        let elapsed = self.elapsed_ms();
        log_to_console(&format!("{} completed in {:.2}ms", operation, elapsed));
    }
}

/// Memory usage monitoring (WASM-specific)
#[wasm_bindgen]
pub fn get_memory_usage() -> Option<js_sys::Object> {
    // Try to get WebAssembly memory information
    if let Ok(memory) = js_sys::Reflect::get(&js_sys::global(), &"WebAssembly".into()) {
        if let Ok(memory_obj) = js_sys::Reflect::get(&memory, &"Memory".into()) {
            return Some(memory_obj.into());
        }
    }
    None
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
    
    /// Check if WebWorkers are available
    #[wasm_bindgen]
    pub fn has_web_workers() -> bool {
        js_sys::Reflect::has(&js_sys::global(), &"Worker".into())
    }
    
    /// Check if WebAssembly is available
    #[wasm_bindgen]
    pub fn has_webassembly() -> bool {
        js_sys::Reflect::has(&js_sys::global(), &"WebAssembly".into())
    }
    
    /// Get browser information
    #[wasm_bindgen]
    pub fn get_browser_info() -> String {
        if let Some(window) = window() {
            if let Some(navigator) = window.navigator() {
                return navigator.user_agent().unwrap_or_else(|_| "Unknown".to_string());
            }
        }
        "Unknown".to_string()
    }
}

/// WASM module initialization
#[wasm_bindgen(start)]
pub fn main() {
    // Set up panic hook
    console_error_panic_hook::set_once();
    
    // Initialize tracing for WASM
    #[cfg(feature = "wasm-logging")]
    tracing_wasm::set_as_global_default();
    
    log_to_console("Loxone MCP WASM module initialized");
    
    // Log browser capabilities
    if BrowserFeatures::has_local_storage() {
        log_to_console("✅ Local Storage available");
    } else {
        log_error_to_console("❌ Local Storage not available");
    }
    
    if BrowserFeatures::has_webassembly() {
        log_to_console("✅ WebAssembly supported");
    } else {
        log_error_to_console("❌ WebAssembly not supported");
    }
}