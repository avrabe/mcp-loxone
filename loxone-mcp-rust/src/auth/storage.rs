//! Storage backends for API keys
//!
//! This module provides different storage backends for API keys,
//! allowing flexibility in deployment scenarios while maintaining
//! a consistent interface.

use crate::auth::models::{ApiKey, AuditEvent};
use crate::auth::security::{self, SecurityCheck};
use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, info, warn};

/// Storage backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StorageBackendConfig {
    /// File-based storage (JSON)
    File {
        /// Path to the key storage file
        path: PathBuf,
    },
    /// Environment variable storage (for development/simple deployments)
    Environment {
        /// Environment variable name containing JSON key data
        var_name: String,
    },
    /// In-memory storage (testing only - not persistent)
    Memory,
}

/// Storage backend trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Load all API keys from storage
    async fn load_keys(&self) -> Result<HashMap<String, ApiKey>>;
    
    /// Save a new or updated API key
    async fn save_key(&self, key: &ApiKey) -> Result<()>;
    
    /// Remove an API key by ID
    async fn remove_key(&self, key_id: &str) -> Result<()>;
    
    /// Save all keys (bulk operation)
    async fn save_all_keys(&self, keys: &HashMap<String, ApiKey>) -> Result<()>;
    
    /// Log an audit event
    async fn log_audit_event(&self, event: &AuditEvent) -> Result<()>;
    
    /// Get recent audit events
    async fn get_audit_events(&self, limit: usize) -> Result<Vec<AuditEvent>>;
}

/// File-based storage implementation
pub struct FileStorage {
    /// Path to the keys file
    keys_file: PathBuf,
    /// Path to the audit log file
    audit_file: PathBuf,
}

impl FileStorage {
    /// Create a new file storage backend with SSH-style security
    pub async fn new(keys_file: PathBuf) -> Result<Self> {
        // Ensure parent directory exists with secure permissions
        if let Some(parent) = keys_file.parent() {
            security::create_secure_directory(parent)?;
        }
        
        // Create audit file path
        let audit_file = keys_file.with_extension("audit.jsonl");
        
        let storage = Self {
            keys_file,
            audit_file,
        };
        
        // Validate security before proceeding
        storage.validate_security().await?;
        
        // Initialize empty files if they don't exist
        storage.initialize_files().await?;
        
        Ok(storage)
    }
    
    /// Validate SSH-style security for credential files
    async fn validate_security(&self) -> Result<()> {
        let parent_dir = self.keys_file.parent()
            .ok_or_else(|| crate::error::LoxoneError::config("Invalid keys file path"))?;
        
        // Check directory and file permissions
        let files_to_check = [&self.keys_file, &self.audit_file];
        let security_checks = security::validate_credential_security(
            parent_dir,
            &files_to_check.iter().map(|p| p.as_path()).collect::<Vec<_>>()
        )?;
        
        // Check for any insecure permissions
        let mut has_insecure = false;
        for check in &security_checks {
            match check {
                SecurityCheck::Insecure { current, required: _, path, fix_command } => {
                    warn!("⚠️  SECURITY WARNING:");
                    warn!("Permissions {:o} for '{}' are too open.", current, path);
                    warn!("It is recommended that your credential files are NOT accessible by others.");
                    warn!("Run: {}", fix_command);
                    has_insecure = true;
                }
                SecurityCheck::Unchecked { reason } => {
                    debug!("Security check skipped: {}", reason);
                }
                SecurityCheck::Secure => {
                    debug!("Security check passed for credential files");
                }
            }
        }
        
        // Auto-fix permissions on Unix systems
        #[cfg(unix)]
        if has_insecure {
            info!("Auto-fixing insecure permissions...");
            security::auto_fix_permissions(&security_checks, true)?;
        }
        
        // On Windows, just warn but continue
        #[cfg(windows)]
        if has_insecure {
            warn!("Windows detected: Please manually ensure only your user account has access to credential files");
        }
        
        Ok(())
    }
    
    /// Initialize storage files if they don't exist with secure permissions
    async fn initialize_files(&self) -> Result<()> {
        // Initialize keys file with secure permissions
        if !self.keys_file.exists() {
            // Create the file securely first
            security::create_secure_file(&self.keys_file)?;
            
            // Write initial empty content
            let empty_keys = HashMap::<String, ApiKey>::new();
            let json = serde_json::to_string_pretty(&empty_keys)
                .map_err(|e| crate::error::LoxoneError::config(format!("Failed to serialize empty keys: {}", e)))?;
            
            fs::write(&self.keys_file, json).await
                .map_err(|e| crate::error::LoxoneError::config(format!("Failed to initialize keys file: {}", e)))?;
            
            info!("Initialized new secure credentials file: {}", self.keys_file.display());
        }
        
        // Initialize audit file with secure permissions if needed
        if !self.audit_file.exists() {
            security::create_secure_file(&self.audit_file)?;
            info!("Initialized new secure audit file: {}", self.audit_file.display());
        }
        
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for FileStorage {
    async fn load_keys(&self) -> Result<HashMap<String, ApiKey>> {
        let content = fs::read_to_string(&self.keys_file).await
            .map_err(|e| crate::error::LoxoneError::config(format!("Failed to read keys file: {}", e)))?;
        
        if content.trim().is_empty() {
            return Ok(HashMap::new());
        }
        
        let keys: HashMap<String, ApiKey> = serde_json::from_str(&content)
            .map_err(|e| crate::error::LoxoneError::config(format!("Failed to parse keys file: {}", e)))?;
        
        debug!("Loaded {} API keys from file", keys.len());
        Ok(keys)
    }
    
    async fn save_key(&self, key: &ApiKey) -> Result<()> {
        let mut keys = self.load_keys().await?;
        keys.insert(key.id.clone(), key.clone());
        self.save_all_keys(&keys).await
    }
    
    async fn remove_key(&self, key_id: &str) -> Result<()> {
        let mut keys = self.load_keys().await?;
        if keys.remove(key_id).is_some() {
            self.save_all_keys(&keys).await?;
            debug!("Removed API key: {}", key_id);
        } else {
            warn!("Attempted to remove non-existent key: {}", key_id);
        }
        Ok(())
    }
    
    async fn save_all_keys(&self, keys: &HashMap<String, ApiKey>) -> Result<()> {
        let json = serde_json::to_string_pretty(keys)
            .map_err(|e| crate::error::LoxoneError::config(format!("Failed to serialize keys: {}", e)))?;
        
        // Write to temporary file first with secure permissions, then move (atomic operation)
        let temp_file = self.keys_file.with_extension("tmp");
        
        // Create temp file with secure permissions first
        security::create_secure_file(&temp_file)?;
        
        // Now write the content
        fs::write(&temp_file, json).await
            .map_err(|e| crate::error::LoxoneError::config(format!("Failed to write temp keys file: {}", e)))?;
        
        // Rename preserves permissions
        fs::rename(&temp_file, &self.keys_file).await
            .map_err(|e| crate::error::LoxoneError::config(format!("Failed to move temp keys file: {}", e)))?;
        
        debug!("Saved {} API keys to file", keys.len());
        Ok(())
    }
    
    async fn log_audit_event(&self, event: &AuditEvent) -> Result<()> {
        let json = serde_json::to_string(event)
            .map_err(|e| crate::error::LoxoneError::config(format!("Failed to serialize audit event: {}", e)))?;
        
        let line = format!("{}\n", json);
        
        // Append to audit log file
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.audit_file)
            .await
            .map_err(|e| crate::error::LoxoneError::config(format!("Failed to open audit file: {}", e)))?;
        
        file.write_all(line.as_bytes()).await
            .map_err(|e| crate::error::LoxoneError::config(format!("Failed to write audit event: {}", e)))?;
        
        file.flush().await
            .map_err(|e| crate::error::LoxoneError::config(format!("Failed to flush audit file: {}", e)))?;
        
        debug!("Logged audit event: {}", event.event_type);
        Ok(())
    }
    
    async fn get_audit_events(&self, limit: usize) -> Result<Vec<AuditEvent>> {
        if !self.audit_file.exists() {
            return Ok(Vec::new());
        }
        
        let content = fs::read_to_string(&self.audit_file).await
            .map_err(|e| crate::error::LoxoneError::config(format!("Failed to read audit file: {}", e)))?;
        
        let mut events = Vec::new();
        for line in content.lines().rev().take(limit) {
            if line.trim().is_empty() {
                continue;
            }
            
            match serde_json::from_str::<AuditEvent>(line) {
                Ok(event) => events.push(event),
                Err(e) => warn!("Failed to parse audit line: {} - {}", e, line),
            }
        }
        
        // Reverse to get chronological order
        events.reverse();
        
        debug!("Retrieved {} audit events", events.len());
        Ok(events)
    }
}

/// Environment variable storage implementation
pub struct EnvironmentStorage {
    /// Environment variable name for keys
    keys_var: String,
}

impl EnvironmentStorage {
    /// Create a new environment storage backend
    pub fn new(keys_var: String) -> Self {
        Self { keys_var }
    }
}

#[async_trait]
impl StorageBackend for EnvironmentStorage {
    async fn load_keys(&self) -> Result<HashMap<String, ApiKey>> {
        match std::env::var(&self.keys_var) {
            Ok(json) => {
                if json.trim().is_empty() {
                    return Ok(HashMap::new());
                }
                
                let keys: HashMap<String, ApiKey> = serde_json::from_str(&json)
                    .map_err(|e| crate::error::LoxoneError::config(format!("Failed to parse keys from env var {}: {}", self.keys_var, e)))?;
                
                debug!("Loaded {} API keys from environment", keys.len());
                Ok(keys)
            }
            Err(_) => {
                debug!("No keys found in environment variable {}", self.keys_var);
                Ok(HashMap::new())
            }
        }
    }
    
    async fn save_key(&self, key: &ApiKey) -> Result<()> {
        let mut keys = self.load_keys().await?;
        keys.insert(key.id.clone(), key.clone());
        self.save_all_keys(&keys).await
    }
    
    async fn remove_key(&self, key_id: &str) -> Result<()> {
        let mut keys = self.load_keys().await?;
        if keys.remove(key_id).is_some() {
            self.save_all_keys(&keys).await?;
            debug!("Removed API key: {}", key_id);
        }
        Ok(())
    }
    
    async fn save_all_keys(&self, keys: &HashMap<String, ApiKey>) -> Result<()> {
        let json = serde_json::to_string(keys)
            .map_err(|e| crate::error::LoxoneError::config(format!("Failed to serialize keys for env var: {}", e)))?;
        
        std::env::set_var(&self.keys_var, json);
        debug!("Saved {} API keys to environment", keys.len());
        
        warn!("Environment storage does not persist across restarts - use file storage for production");
        Ok(())
    }
    
    async fn log_audit_event(&self, event: &AuditEvent) -> Result<()> {
        // For environment storage, we just log to tracing
        info!(
            event_type = %event.event_type,
            key_id = ?event.key_id,
            client_ip = %event.client_ip,
            success = event.success,
            "Audit event"
        );
        Ok(())
    }
    
    async fn get_audit_events(&self, _limit: usize) -> Result<Vec<AuditEvent>> {
        // Environment storage doesn't support audit history
        Ok(Vec::new())
    }
}

/// In-memory storage (for testing)
pub struct MemoryStorage {
    keys: std::sync::Arc<tokio::sync::RwLock<HashMap<String, ApiKey>>>,
    audit_events: std::sync::Arc<tokio::sync::RwLock<Vec<AuditEvent>>>,
}

impl MemoryStorage {
    /// Create a new memory storage backend
    pub fn new() -> Self {
        Self {
            keys: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            audit_events: std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageBackend for MemoryStorage {
    async fn load_keys(&self) -> Result<HashMap<String, ApiKey>> {
        let keys = self.keys.read().await;
        Ok(keys.clone())
    }
    
    async fn save_key(&self, key: &ApiKey) -> Result<()> {
        let mut keys = self.keys.write().await;
        keys.insert(key.id.clone(), key.clone());
        debug!("Saved API key to memory: {}", key.id);
        Ok(())
    }
    
    async fn remove_key(&self, key_id: &str) -> Result<()> {
        let mut keys = self.keys.write().await;
        if keys.remove(key_id).is_some() {
            debug!("Removed API key from memory: {}", key_id);
        }
        Ok(())
    }
    
    async fn save_all_keys(&self, keys: &HashMap<String, ApiKey>) -> Result<()> {
        let mut storage_keys = self.keys.write().await;
        *storage_keys = keys.clone();
        debug!("Saved {} API keys to memory", keys.len());
        Ok(())
    }
    
    async fn log_audit_event(&self, event: &AuditEvent) -> Result<()> {
        let mut events = self.audit_events.write().await;
        events.push(event.clone());
        
        // Keep only last 1000 events to prevent memory growth
        if events.len() > 1000 {
            events.remove(0);
        }
        
        debug!("Logged audit event to memory: {}", event.event_type);
        Ok(())
    }
    
    async fn get_audit_events(&self, limit: usize) -> Result<Vec<AuditEvent>> {
        let events = self.audit_events.read().await;
        let start_idx = events.len().saturating_sub(limit);
        Ok(events[start_idx..].to_vec())
    }
}

/// Create a storage backend from configuration
pub async fn create_storage_backend(config: &StorageBackendConfig) -> Result<Box<dyn StorageBackend>> {
    match config {
        StorageBackendConfig::File { path } => {
            let storage = FileStorage::new(path.clone()).await?;
            Ok(Box::new(storage))
        }
        StorageBackendConfig::Environment { var_name } => {
            let storage = EnvironmentStorage::new(var_name.clone());
            Ok(Box::new(storage))
        }
        StorageBackendConfig::Memory => {
            let storage = MemoryStorage::new();
            Ok(Box::new(storage))
        }
    }
}