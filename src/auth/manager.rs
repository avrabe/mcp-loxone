//! Unified Authentication Manager
//!
//! This is the main entry point for the authentication system. It coordinates
//! between storage, validation, and caching to provide a unified API for
//! authentication across all server components.

use crate::auth::models::{ApiKey, AuditEvent, AuthContext, AuthResult, Role};
use crate::auth::storage::{create_storage_backend, StorageBackend, StorageBackendConfig};
use crate::auth::validation::{extract_api_key, extract_client_ip, ValidationConfig, Validator};
use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// The main authentication manager
pub struct AuthenticationManager {
    /// Storage backend for persistent key storage
    storage: Arc<dyn StorageBackend>,
    /// Validator for authentication logic and rate limiting
    validator: Validator,
    /// In-memory cache for fast key lookups
    key_cache: Arc<RwLock<HashMap<String, ApiKey>>>,
    /// Configuration
    config: AuthManagerConfig,
}

/// Configuration for the authentication manager
#[derive(Debug, Clone)]
pub struct AuthManagerConfig {
    /// Storage backend configuration
    pub storage_config: StorageBackendConfig,
    /// Validation configuration
    pub validation_config: ValidationConfig,
    /// Cache refresh interval in minutes
    pub cache_refresh_interval_minutes: u64,
    /// Enable automatic cache warming
    pub enable_cache_warming: bool,
}

impl Default for AuthManagerConfig {
    fn default() -> Self {
        Self {
            storage_config: StorageBackendConfig::File {
                path: dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".loxone")
                    .join("api_keys.json"),
            },
            validation_config: ValidationConfig::default(),
            cache_refresh_interval_minutes: 5,
            enable_cache_warming: true,
        }
    }
}

impl AuthenticationManager {
    /// Create a new authentication manager with default configuration
    pub async fn new() -> Result<Self> {
        Self::with_config(AuthManagerConfig::default()).await
    }

    /// Create a new authentication manager with custom configuration
    pub async fn with_config(config: AuthManagerConfig) -> Result<Self> {
        let storage = create_storage_backend(&config.storage_config).await?;
        let validator = Validator::new(config.validation_config.clone());

        let manager = Self {
            storage,
            validator,
            key_cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        };

        // Load initial keys into cache
        manager.refresh_cache().await?;

        // Start background tasks
        if manager.config.enable_cache_warming {
            manager.start_background_tasks().await;
        }

        info!("Authentication manager initialized successfully");
        Ok(manager)
    }

    /// Authenticate a request using headers and query parameters
    pub async fn authenticate_request(
        &self,
        headers: &axum::http::HeaderMap,
        query: Option<&str>,
    ) -> AuthResult {
        let client_ip = extract_client_ip(headers);

        let api_key_secret = match extract_api_key(headers, query) {
            Some(key) => key,
            None => {
                self.log_audit_event(AuditEvent::auth_failure(
                    client_ip,
                    "No API key provided".to_string(),
                ))
                .await;

                return AuthResult::Unauthorized {
                    reason: "No API key provided in headers or query parameters".to_string(),
                };
            }
        };

        self.authenticate(&api_key_secret, &client_ip).await
    }

    /// Authenticate with API key and client IP
    pub async fn authenticate(&self, api_key_secret: &str, client_ip: &str) -> AuthResult {
        let keys = self.get_cached_keys().await;
        let result = self
            .validator
            .validate_authentication(api_key_secret, client_ip, &keys)
            .await;

        match &result {
            AuthResult::Success(auth_success) => {
                debug!(
                    "Authentication successful for key: {} from IP: {}",
                    auth_success.key.id, client_ip
                );

                // Update usage statistics
                self.update_key_usage(&auth_success.key.id).await;

                // Log successful authentication
                self.log_audit_event(AuditEvent::auth_success(
                    auth_success.key.id.clone(),
                    client_ip.to_string(),
                ))
                .await;
            }
            AuthResult::Unauthorized { reason } => {
                warn!("Authentication failed from IP {}: {}", client_ip, reason);

                self.log_audit_event(AuditEvent::auth_failure(
                    client_ip.to_string(),
                    reason.clone(),
                ))
                .await;
            }
            AuthResult::Forbidden { reason } => {
                warn!("Authentication forbidden from IP {}: {}", client_ip, reason);

                self.log_audit_event(AuditEvent::auth_failure(
                    client_ip.to_string(),
                    reason.clone(),
                ))
                .await;
            }
            AuthResult::RateLimited {
                retry_after_seconds,
            } => {
                warn!(
                    "Rate limited IP {} for {} seconds",
                    client_ip, retry_after_seconds
                );
            }
        }

        result
    }

    /// Check if a session has the required permission
    pub async fn check_permission(&self, context: &AuthContext, permission: &str) -> bool {
        self.validator.check_permission(context, permission)
    }

    /// Create a new API key
    pub async fn create_key(
        &self,
        name: String,
        role: Role,
        created_by: String,
        expires_in_days: Option<u32>,
    ) -> Result<ApiKey> {
        let key = ApiKey::new(name, role.clone(), created_by.clone(), expires_in_days);

        // Save to storage
        self.storage.save_key(&key).await?;

        // Update cache
        {
            let mut cache = self.key_cache.write().await;
            cache.insert(key.id.clone(), key.clone());
        }

        // Log key creation
        self.log_audit_event(AuditEvent::key_created(key.id.clone(), created_by, role))
            .await;

        info!("Created new API key: {} ({})", key.id, key.name);
        Ok(key)
    }

    /// Update an existing API key
    pub async fn update_key(&self, key: ApiKey) -> Result<()> {
        // Save to storage
        self.storage.save_key(&key).await?;

        // Update cache
        {
            let mut cache = self.key_cache.write().await;
            cache.insert(key.id.clone(), key.clone());
        }

        debug!("Updated API key: {}", key.id);
        Ok(())
    }

    /// Delete an API key
    pub async fn delete_key(&self, key_id: &str) -> Result<bool> {
        // Remove from storage
        self.storage.remove_key(key_id).await?;

        // Remove from cache
        let removed = {
            let mut cache = self.key_cache.write().await;
            cache.remove(key_id).is_some()
        };

        if removed {
            info!("Deleted API key: {}", key_id);
        } else {
            warn!("Attempted to delete non-existent key: {}", key_id);
        }

        Ok(removed)
    }

    /// List all API keys
    pub async fn list_keys(&self) -> Vec<ApiKey> {
        let cache = self.key_cache.read().await;
        cache.values().cloned().collect()
    }

    /// Get a specific API key by ID
    pub async fn get_key(&self, key_id: &str) -> Option<ApiKey> {
        let cache = self.key_cache.read().await;
        cache.get(key_id).cloned()
    }

    /// Get recent audit events
    pub async fn get_audit_events(&self, limit: usize) -> Result<Vec<AuditEvent>> {
        self.storage.get_audit_events(limit).await
    }

    /// Get authentication statistics
    pub async fn get_auth_stats(&self) -> AuthStats {
        let cache = self.key_cache.read().await;
        let total_keys = cache.len();
        let active_keys = cache.values().filter(|k| k.is_valid()).count();
        let expired_keys = cache.values().filter(|k| !k.is_valid()).count();

        let rate_limit_stats = self.validator.get_rate_limit_stats().await;

        AuthStats {
            total_keys,
            active_keys,
            expired_keys,
            currently_blocked_ips: rate_limit_stats.currently_blocked_ips,
            total_failed_attempts: rate_limit_stats.total_failed_attempts,
        }
    }

    /// Refresh the key cache from storage
    async fn refresh_cache(&self) -> Result<()> {
        let keys = self.storage.load_keys().await?;
        let key_count = keys.len();

        {
            let mut cache = self.key_cache.write().await;
            *cache = keys;
        }

        debug!("Refreshed key cache with {} keys", key_count);
        Ok(())
    }

    /// Get cached keys for validation
    async fn get_cached_keys(&self) -> HashMap<String, ApiKey> {
        let cache = self.key_cache.read().await;
        cache.clone()
    }

    /// Update key usage statistics (in-memory only, periodic persistence)
    async fn update_key_usage(&self, key_id: &str) {
        if let Some(mut key) = self.get_key(key_id).await {
            key.record_usage();

            // Update in cache only - let background task persist to storage
            {
                let mut cache = self.key_cache.write().await;
                cache.insert(key_id.to_string(), key);
            }
        }
    }

    /// Log an audit event
    async fn log_audit_event(&self, event: AuditEvent) {
        if let Err(e) = self.storage.log_audit_event(&event).await {
            error!("Failed to log audit event: {}", e);
        }
    }

    /// Start background maintenance tasks
    async fn start_background_tasks(&self) {
        let validator_ref = self.validator.clone();
        let storage_ref = self.storage.clone();
        let cache_ref = self.key_cache.clone();
        let refresh_interval = self.config.cache_refresh_interval_minutes;

        // Rate limit cleanup task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // Every hour

            loop {
                interval.tick().await;
                validator_ref.cleanup_rate_limits().await;
            }
        });

        // Cache refresh task
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(refresh_interval * 60));

            loop {
                interval.tick().await;

                debug!("Refreshing API key cache from storage...");
                match storage_ref.load_keys().await {
                    Ok(keys) => {
                        let key_count = keys.len();
                        {
                            let mut cache = cache_ref.write().await;
                            *cache = keys; // Replace the entire cache with loaded keys
                        }
                        debug!("Cache refreshed successfully with {} keys", key_count);
                    }
                    Err(e) => {
                        warn!("Failed to refresh cache from storage: {}", e);
                    }
                }
            }
        });

        // Usage persistence task - save usage statistics every 5 minutes
        let usage_storage_ref = self.storage.clone();
        let usage_cache_ref = self.key_cache.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // Every 5 minutes

            loop {
                interval.tick().await;

                debug!("Persisting key usage statistics to storage...");
                let keys = {
                    let cache = usage_cache_ref.read().await;
                    cache.clone()
                };

                if let Err(e) = usage_storage_ref.save_all_keys(&keys).await {
                    warn!("Failed to persist key usage statistics: {}", e);
                } else {
                    debug!(
                        "Successfully persisted usage statistics for {} keys",
                        keys.len()
                    );
                }
            }
        });

        debug!("Background tasks started (rate limit cleanup + cache refresh + usage persistence)");
    }

    /// Gracefully shutdown and persist any pending usage statistics
    pub async fn shutdown(&self) -> Result<()> {
        info!("Saving final usage statistics before shutdown...");

        let keys = {
            let cache = self.key_cache.read().await;
            cache.clone()
        };

        self.storage.save_all_keys(&keys).await?;
        info!("Usage statistics saved successfully on shutdown");

        Ok(())
    }
}

// Implementing Clone for Validator to support background tasks
impl Clone for Validator {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            rate_limit_state: self.rate_limit_state.clone(),
        }
    }
}

/// Authentication statistics
#[derive(Debug, Clone)]
pub struct AuthStats {
    /// Total number of API keys
    pub total_keys: usize,
    /// Number of active (valid) keys
    pub active_keys: usize,
    /// Number of expired keys
    pub expired_keys: usize,
    /// Number of currently blocked IPs
    pub currently_blocked_ips: u32,
    /// Total failed authentication attempts
    pub total_failed_attempts: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::storage::StorageBackendConfig;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_authentication_manager() {
        let temp_dir = TempDir::new().unwrap();
        let keys_file = temp_dir.path().join("test_keys.json");

        let config = AuthManagerConfig {
            storage_config: StorageBackendConfig::File { path: keys_file },
            validation_config: ValidationConfig::default(),
            cache_refresh_interval_minutes: 60,
            enable_cache_warming: false,
        };

        let manager = AuthenticationManager::with_config(config).await.unwrap();

        // Create a test key
        let key = manager
            .create_key(
                "Test Key".to_string(),
                Role::Operator,
                "test_user".to_string(),
                Some(365),
            )
            .await
            .unwrap();

        // Test authentication
        let result = manager.authenticate(&key.secret, "127.0.0.1").await;
        match result {
            AuthResult::Success(auth_success) => {
                assert_eq!(auth_success.key.id, key.id);
            }
            _ => panic!("Expected successful authentication"),
        }

        // Test invalid key
        let result = manager.authenticate("invalid_key", "127.0.0.1").await;
        match result {
            AuthResult::Unauthorized { .. } => {
                // Expected
            }
            _ => panic!("Expected authentication failure"),
        }
    }
}
