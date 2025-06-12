//! Configuration management for the Loxone MCP server

pub mod credentials;

#[cfg(target_os = "macos")]
pub mod security_keychain;

#[cfg(feature = "infisical")]
pub mod infisical_client;

#[cfg(feature = "wasi-keyvalue")]
pub mod wasi_keyvalue;

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::{env, time::Duration};
use url::Url;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    /// Loxone Miniserver configuration
    pub loxone: LoxoneConfig,
    
    /// MCP server configuration
    pub mcp: McpConfig,
    
    /// Credential storage configuration
    pub credentials: CredentialStore,
    
    /// Logging configuration
    pub logging: LoggingConfig,
    
    /// Feature flags
    pub features: FeatureConfig,
}

/// Loxone Miniserver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneConfig {
    /// Miniserver URL (e.g., "http://192.168.1.100")
    pub url: Url,
    
    /// Username for authentication
    pub username: String,
    
    /// Connection timeout
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,
    
    /// Maximum number of connection retries
    pub max_retries: u32,
    
    /// Enable SSL/TLS verification
    pub verify_ssl: bool,
    
    /// WebSocket configuration
    #[cfg(feature = "websocket")]
    pub websocket: WebSocketConfig,
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Server name for MCP identification
    pub name: String,
    
    /// Server version
    pub version: String,
    
    /// Transport configuration
    pub transport: TransportConfig,
    
    /// Tool configuration
    pub tools: ToolConfig,
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Transport type (stdio, http, etc.)
    pub transport_type: String,
    
    /// Port for HTTP transport (if applicable)
    pub port: Option<u16>,
    
    /// Host for HTTP transport (if applicable)  
    pub host: Option<String>,
}

/// Tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Enable room management tools
    pub enable_rooms: bool,
    
    /// Enable device control tools
    pub enable_devices: bool,
    
    /// Enable sensor discovery tools
    pub enable_sensors: bool,
    
    /// Enable climate control tools
    pub enable_climate: bool,
    
    /// Enable weather tools
    pub enable_weather: bool,
    
    /// Maximum devices to return in listings
    pub max_devices_per_query: usize,
}

/// WebSocket configuration
#[cfg(feature = "websocket")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    /// Enable real-time monitoring
    pub enable_monitoring: bool,
    
    /// Sensor discovery duration
    #[serde(with = "humantime_serde")]
    pub discovery_duration: Duration,
    
    /// Connection keep-alive interval
    #[serde(with = "humantime_serde")]
    pub keepalive_interval: Duration,
}

/// Credential storage options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CredentialStore {
    /// Use system keyring (not available in WASM)
    #[cfg(feature = "keyring-storage")]
    Keyring,
    
    /// Use environment variables
    Environment,
    
    /// Use browser local storage (WASM only)
    #[cfg(target_arch = "wasm32")]
    LocalStorage,
    
    /// Use file system storage (WASI)
    FileSystem { path: String },
    
    /// Use Infisical for centralized secret management
    #[cfg(feature = "infisical")]
    Infisical {
        project_id: String,
        environment: String,
        client_id: String,
        client_secret: String,
        host: Option<String>, // For self-hosted instances
    },
    
    /// Use WASI keyvalue interface (WASM component model)
    #[cfg(feature = "wasi-keyvalue")]
    WasiKeyValue { store_name: Option<String> },
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    
    /// Enable structured JSON logging
    pub json_format: bool,
    
    /// Log to file (path)
    pub file: Option<String>,
}

/// Feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Enable encryption features
    pub enable_crypto: bool,
    
    /// Enable WebSocket features
    pub enable_websocket: bool,
    
    /// Enable caching
    pub enable_caching: bool,
    
    /// Cache TTL
    #[serde(with = "humantime_serde")]
    pub cache_ttl: Duration,
}


impl Default for LoxoneConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:80".parse().unwrap(),
            username: "admin".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
            verify_ssl: true,
            #[cfg(feature = "websocket")]
            websocket: WebSocketConfig::default(),
        }
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            name: "Loxone Controller (Rust)".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            transport: TransportConfig::default(),
            tools: ToolConfig::default(),
        }
    }
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            transport_type: "stdio".to_string(),
            port: None,
            host: None,
        }
    }
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            enable_rooms: true,
            enable_devices: true,
            enable_sensors: true,
            enable_climate: true,
            enable_weather: true,
            max_devices_per_query: 100,
        }
    }
}

#[cfg(feature = "websocket")]
impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            enable_monitoring: true,
            discovery_duration: Duration::from_secs(60),
            keepalive_interval: Duration::from_secs(30),
        }
    }
}

impl Default for CredentialStore {
    fn default() -> Self {
        // Check for Infisical configuration first
        #[cfg(feature = "infisical")]
        {
            if let (Ok(project_id), Ok(client_id), Ok(client_secret)) = (
                env::var("INFISICAL_PROJECT_ID"),
                env::var("INFISICAL_CLIENT_ID"),
                env::var("INFISICAL_CLIENT_SECRET"),
            ) {
                let environment = env::var("INFISICAL_ENVIRONMENT").unwrap_or_else(|_| "dev".to_string());
                let host = env::var("INFISICAL_HOST").ok();
                
                return CredentialStore::Infisical {
                    project_id,
                    environment,
                    client_id,
                    client_secret,
                    host,
                };
            }
        }
        
        // WASM environment preferences
        #[cfg(target_arch = "wasm32")]
        {
            #[cfg(feature = "wasi-keyvalue")]
            return CredentialStore::WasiKeyValue { store_name: None };
            
            #[cfg(not(feature = "wasi-keyvalue"))]
            return CredentialStore::LocalStorage;
        }
        
        // Native environment preferences
        #[cfg(not(target_arch = "wasm32"))]
        {
            #[cfg(feature = "keyring-storage")]
            return CredentialStore::Keyring;
            
            #[cfg(not(feature = "keyring-storage"))]
            return CredentialStore::Environment;
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json_format: false,
            file: None,
        }
    }
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            enable_crypto: cfg!(feature = "crypto"),
            enable_websocket: cfg!(feature = "websocket"),
            enable_caching: true,
            cache_ttl: Duration::from_secs(30),
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let mut config = Self::default();
        
        // Load Loxone configuration
        if let Ok(url) = env::var("LOXONE_URL") {
            config.loxone.url = url.parse()
                .map_err(|e| LoxoneError::config(format!("Invalid LOXONE_URL: {}", e)))?;
        }
        
        if let Ok(username) = env::var("LOXONE_USERNAME") {
            config.loxone.username = username;
        }
        
        if let Ok(timeout) = env::var("LOXONE_TIMEOUT") {
            config.loxone.timeout = Duration::from_secs(
                timeout.parse()
                    .map_err(|e| LoxoneError::config(format!("Invalid LOXONE_TIMEOUT: {}", e)))?
            );
        }
        
        // Load logging configuration
        if let Ok(level) = env::var("RUST_LOG") {
            config.logging.level = level;
        }
        
        // Load transport configuration
        if let Ok(transport) = env::var("MCP_TRANSPORT") {
            config.mcp.transport.transport_type = transport;
        }
        
        if let Ok(port) = env::var("MCP_PORT") {
            config.mcp.transport.port = Some(
                port.parse()
                    .map_err(|e| LoxoneError::config(format!("Invalid MCP_PORT: {}", e)))?
            );
        }
        
        Ok(config)
    }
    
    /// Load configuration for WASM environment
    #[cfg(target_arch = "wasm32")]
    pub async fn from_wasm_env() -> Result<Self> {
        let mut config = Self::default();
        
        // Override credential store for WASM
        config.credentials = CredentialStore::LocalStorage;
        
        // Try to load from browser storage
        if let Ok(storage) = web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten()
        {
            if let Ok(Some(url)) = storage.get_item("loxone_url") {
                config.loxone.url = url.parse()
                    .map_err(|e| LoxoneError::config(format!("Invalid stored URL: {}", e)))?;
            }
            
            if let Ok(Some(username)) = storage.get_item("loxone_username") {
                config.loxone.username = username;
            }
        }
        
        Ok(config)
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate URL
        if self.loxone.url.scheme() != "http" && self.loxone.url.scheme() != "https" {
            return Err(LoxoneError::config("URL must use http or https scheme"));
        }
        
        // Validate username
        if self.loxone.username.is_empty() {
            return Err(LoxoneError::config("Username cannot be empty"));
        }
        
        // Validate timeout
        if self.loxone.timeout.is_zero() {
            return Err(LoxoneError::config("Timeout must be greater than zero"));
        }
        
        Ok(())
    }
}