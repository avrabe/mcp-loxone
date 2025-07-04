//! Multi-user API key management system

use crate::error::{LoxoneError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// API key roles with specific permissions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyRole {
    /// Full system access - all operations
    Admin,
    /// Device control and monitoring - no key management
    Operator,
    /// Read-only access to all resources
    Monitor,
    /// Specific device control only
    Device {
        #[serde(default)]
        allowed_devices: Vec<String>,
    },
    /// Custom role with specific permissions
    Custom { permissions: Vec<String> },
}

/// API key definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// Unique key ID (format: lmcp_{role}_{seq}_{random})
    pub id: String,

    /// Human-readable name/description
    pub name: String,

    /// Role-based permissions
    pub role: ApiKeyRole,

    /// Who created this key
    pub created_by: String,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Optional expiration
    pub expires_at: Option<DateTime<Utc>>,

    /// IP whitelist (empty = all allowed)
    pub ip_whitelist: Vec<String>,

    /// Is key active
    pub active: bool,

    /// Usage tracking
    #[serde(default)]
    pub last_used: Option<DateTime<Utc>>,

    #[serde(default)]
    pub usage_count: u64,

    /// Optional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Key store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyStoreConfig {
    /// Storage backend type
    pub backend: KeyStoreBackend,

    /// File path for file-based storage
    pub file_path: Option<PathBuf>,

    /// Auto-save changes
    #[serde(default = "default_true")]
    pub auto_save: bool,

    /// Encrypt keys at rest
    #[serde(default)]
    pub encrypt_at_rest: bool,
}

fn default_true() -> bool {
    true
}

impl Default for KeyStoreConfig {
    fn default() -> Self {
        Self {
            backend: KeyStoreBackend::File,
            file_path: Some(default_key_store_path()),
            auto_save: true,
            encrypt_at_rest: false,
        }
    }
}

/// Key store backend types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyStoreBackend {
    /// File-based storage (TOML/JSON)
    File,
    /// Environment variable
    Environment,
    /// In-memory only
    Memory,
    /// SQLite database
    Sqlite,
}

/// Multi-user key store
pub struct KeyStore {
    /// Active keys by ID
    keys: Arc<RwLock<HashMap<String, ApiKey>>>,

    /// Configuration
    config: KeyStoreConfig,

    /// File path for persistence
    file_path: Option<PathBuf>,
}

impl KeyStore {
    /// Create new key store from configuration
    pub async fn new(config: KeyStoreConfig) -> Result<Self> {
        let keys = Arc::new(RwLock::new(HashMap::new()));

        let mut store = Self {
            keys,
            config: config.clone(),
            file_path: config.file_path.clone(),
        };

        // Load existing keys
        store.load().await?;

        Ok(store)
    }

    /// Load keys from configured backend
    pub async fn load(&mut self) -> Result<()> {
        match &self.config.backend {
            KeyStoreBackend::File => self.load_from_file().await,
            KeyStoreBackend::Environment => self.load_from_env().await,
            KeyStoreBackend::Memory => Ok(()),
            KeyStoreBackend::Sqlite => self.load_from_sqlite().await,
        }
    }

    /// Load keys from file
    async fn load_from_file(&mut self) -> Result<()> {
        let path = self
            .file_path
            .as_ref()
            .ok_or_else(|| LoxoneError::config("No file path configured for key store"))?;

        if !path.exists() {
            info!("Key store file not found, starting with empty store");
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&path).await?;

        let keys: Vec<ApiKey> = if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            // Try TOML first, fall back to JSON if it fails
            match toml::from_str(&content) {
                Ok(keys) => keys,
                Err(_) => {
                    warn!("TOML parsing failed, trying JSON format");
                    serde_json::from_str(&content)?
                }
            }
        } else {
            serde_json::from_str(&content)?
        };

        let mut store = self.keys.write().await;
        for key in keys {
            store.insert(key.id.clone(), key);
        }

        info!("Loaded {} API keys from {}", store.len(), path.display());
        Ok(())
    }

    /// Load keys from environment variable
    async fn load_from_env(&mut self) -> Result<()> {
        let json = std::env::var("LOXONE_API_KEYS").unwrap_or_else(|_| "[]".to_string());

        let keys: Vec<ApiKey> = serde_json::from_str(&json)?;

        let mut store = self.keys.write().await;
        for key in keys {
            store.insert(key.id.clone(), key);
        }

        info!("Loaded {} API keys from environment", store.len());
        Ok(())
    }

    /// Load from SQLite (placeholder)
    async fn load_from_sqlite(&mut self) -> Result<()> {
        // TODO: Implement SQLite backend
        warn!("SQLite backend not yet implemented");
        Ok(())
    }

    /// Save keys to configured backend
    pub async fn save(&self) -> Result<()> {
        if !self.config.auto_save {
            return Ok(());
        }

        match &self.config.backend {
            KeyStoreBackend::File => self.save_to_file().await,
            KeyStoreBackend::Environment => {
                warn!("Cannot save to environment variables");
                Ok(())
            }
            KeyStoreBackend::Memory => Ok(()),
            KeyStoreBackend::Sqlite => self.save_to_sqlite().await,
        }
    }

    /// Save keys to file
    async fn save_to_file(&self) -> Result<()> {
        let path = self
            .file_path
            .as_ref()
            .ok_or_else(|| LoxoneError::config("No file path configured for key store"))?;

        let keys = self.keys.read().await;
        let keys_vec: Vec<&ApiKey> = keys.values().collect();

        let content = if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            // Try TOML first, fall back to JSON if it fails
            match toml::to_string_pretty(&keys_vec) {
                Ok(toml_content) => toml_content,
                Err(_) => {
                    warn!("TOML serialization failed, falling back to JSON format");
                    serde_json::to_string_pretty(&keys_vec)?
                }
            }
        } else {
            serde_json::to_string_pretty(&keys_vec)?
        };

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Write atomically
        let temp_path = path.with_extension("tmp");
        tokio::fs::write(&temp_path, content).await?;
        tokio::fs::rename(&temp_path, path).await?;

        debug!("Saved {} keys to {}", keys_vec.len(), path.display());
        Ok(())
    }

    /// Save to SQLite (placeholder)
    async fn save_to_sqlite(&self) -> Result<()> {
        // TODO: Implement SQLite backend
        Ok(())
    }

    /// Add a new API key
    pub async fn add_key(&self, key: ApiKey) -> Result<()> {
        let mut keys = self.keys.write().await;

        // Check if key ID already exists
        if keys.contains_key(&key.id) {
            return Err(LoxoneError::config(format!(
                "Key {} already exists",
                key.id
            )));
        }

        // Check for duplicate name+role combinations
        for existing_key in keys.values() {
            if existing_key.name == key.name && existing_key.role == key.role && existing_key.active
            {
                return Err(LoxoneError::config(format!(
                    "An active API key with name '{}' and role '{:?}' already exists (ID: {}). Use a different name or revoke the existing key first.",
                    key.name, key.role, existing_key.id
                )));
            }
        }

        info!("Adding new API key: {} ({})", key.id, key.name);
        keys.insert(key.id.clone(), key);
        drop(keys);

        self.save().await?;
        Ok(())
    }

    /// Remove an API key
    pub async fn remove_key(&self, key_id: &str) -> Result<()> {
        let mut keys = self.keys.write().await;

        if keys.remove(key_id).is_none() {
            return Err(LoxoneError::not_found(format!("Key {key_id} not found")));
        }

        info!("Removed API key: {key_id}");
        drop(keys);

        self.save().await?;
        Ok(())
    }

    /// Update an existing key
    pub async fn update_key(&self, key: ApiKey) -> Result<()> {
        let mut keys = self.keys.write().await;

        if !keys.contains_key(&key.id) {
            return Err(LoxoneError::not_found(format!("Key {} not found", key.id)));
        }

        info!("Updated API key: {} ({})", key.id, key.name);
        keys.insert(key.id.clone(), key);
        drop(keys);

        self.save().await?;
        Ok(())
    }

    /// Get a key by ID
    pub async fn get_key(&self, key_id: &str) -> Option<ApiKey> {
        self.keys.read().await.get(key_id).cloned()
    }

    /// List all keys
    pub async fn list_keys(&self) -> Vec<ApiKey> {
        self.keys.read().await.values().cloned().collect()
    }

    /// Validate a key and check permissions
    pub async fn validate_key(&self, key_id: &str, client_ip: Option<IpAddr>) -> Result<ApiKey> {
        let keys = self.keys.read().await;

        let key = keys
            .get(key_id)
            .ok_or_else(|| LoxoneError::authentication("Invalid API key"))?;

        // Check if active
        if !key.active {
            return Err(LoxoneError::authentication("API key is inactive"));
        }

        // Check expiration
        if let Some(expires_at) = key.expires_at {
            if Utc::now() > expires_at {
                return Err(LoxoneError::authentication("API key has expired"));
            }
        }

        // Check IP whitelist
        if !key.ip_whitelist.is_empty() {
            if let Some(ip) = client_ip {
                let ip_str = ip.to_string();
                let allowed = key.ip_whitelist.iter().any(|allowed| {
                    // Support CIDR notation
                    if allowed.contains('/') {
                        // Implement CIDR matching
                        #[allow(clippy::collapsible_match)]
                        match (parse_cidr(allowed), ip) {
                            (Some((network, prefix_len)), std::net::IpAddr::V4(ip_v4)) => {
                                if let std::net::IpAddr::V4(net_v4) = network {
                                    check_ipv4_in_cidr(ip_v4, net_v4, prefix_len)
                                } else {
                                    false
                                }
                            }
                            (Some((network, prefix_len)), std::net::IpAddr::V6(ip_v6)) => {
                                if let std::net::IpAddr::V6(net_v6) = network {
                                    check_ipv6_in_cidr(ip_v6, net_v6, prefix_len)
                                } else {
                                    false
                                }
                            }
                            _ => false,
                        }
                    } else {
                        allowed == &ip_str
                    }
                });

                if !allowed {
                    return Err(LoxoneError::authentication("IP address not allowed"));
                }
            }
        }

        Ok(key.clone())
    }

    /// Record key usage
    pub async fn record_usage(&self, key_id: &str) -> Result<()> {
        let mut keys = self.keys.write().await;

        if let Some(key) = keys.get_mut(key_id) {
            key.last_used = Some(Utc::now());
            key.usage_count += 1;

            // Save periodically (every 10 uses)
            if key.usage_count % 10 == 0 {
                drop(keys);
                self.save().await?;
            }
        }

        Ok(())
    }

    /// Check if a key has permission for an operation
    pub async fn check_permission(&self, key_id: &str, operation: &str) -> Result<bool> {
        let keys = self.keys.read().await;

        let key = keys
            .get(key_id)
            .ok_or_else(|| LoxoneError::authentication("Invalid API key"))?;

        Ok(match &key.role {
            ApiKeyRole::Admin => true, // Admin can do everything
            ApiKeyRole::Operator => {
                // Operators can control devices and monitor
                matches!(operation, "control" | "monitor" | "read")
            }
            ApiKeyRole::Monitor => {
                // Monitor can only read
                operation == "read" || operation == "monitor"
            }
            ApiKeyRole::Device { allowed_devices } => {
                // Check if operation is on allowed device
                operation.starts_with("device:")
                    && allowed_devices.iter().any(|d| operation.contains(d))
            }
            ApiKeyRole::Custom { permissions } => permissions.contains(&operation.to_string()),
        })
    }
}

/// Default key store path
pub fn default_key_store_path() -> PathBuf {
    // Use current directory as fallback instead of dirs crate
    PathBuf::from(".").join("loxone-mcp").join("keys.toml")
}

/// Parse CIDR notation (e.g., "192.168.1.0/24") into network address and prefix length
fn parse_cidr(cidr: &str) -> Option<(std::net::IpAddr, u8)> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        return None;
    }

    let ip_addr = parts[0].parse::<std::net::IpAddr>().ok()?;
    let prefix_len = parts[1].parse::<u8>().ok()?;

    Some((ip_addr, prefix_len))
}

/// Check if an IPv4 address is within a CIDR range
fn check_ipv4_in_cidr(ip: std::net::Ipv4Addr, network: std::net::Ipv4Addr, prefix_len: u8) -> bool {
    if prefix_len > 32 {
        return false;
    }

    let ip_bits = u32::from(ip);
    let network_bits = u32::from(network);
    let mask = if prefix_len == 0 {
        0
    } else {
        !((1u32 << (32 - prefix_len)) - 1)
    };

    (ip_bits & mask) == (network_bits & mask)
}

/// Check if an IPv6 address is within a CIDR range
fn check_ipv6_in_cidr(ip: std::net::Ipv6Addr, network: std::net::Ipv6Addr, prefix_len: u8) -> bool {
    if prefix_len > 128 {
        return false;
    }

    let ip_bits = u128::from(ip);
    let network_bits = u128::from(network);
    let mask = if prefix_len == 0 {
        0
    } else if prefix_len == 128 {
        !0u128
    } else {
        !((1u128 << (128 - prefix_len)) - 1)
    };

    (ip_bits & mask) == (network_bits & mask)
}
