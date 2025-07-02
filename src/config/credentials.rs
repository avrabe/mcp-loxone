//! Credential management for Loxone authentication
//!
//! This module provides secure credential storage and retrieval across
//! different platforms including native systems and WASM environments.

use crate::config::CredentialStore;
use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[cfg(feature = "infisical")]
use crate::config::infisical_client::{create_authenticated_client, InfisicalClient};

/// Loxone credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneCredentials {
    /// Username for Loxone authentication
    pub username: String,

    /// Password for Loxone authentication
    pub password: String,

    /// Optional API key for enhanced security
    pub api_key: Option<String>,

    /// Optional RSA public key for encryption
    #[cfg(feature = "crypto-openssl")]
    pub public_key: Option<String>,
}

/// Credential manager for secure storage and retrieval
pub struct CredentialManager {
    store: CredentialStore,

    #[cfg(feature = "infisical")]
    infisical_client: Option<InfisicalClient>,
}

// Credential key constants (shared across all backends)
impl CredentialManager {
    #[allow(dead_code)] // Used in keyring feature
    const SERVICE_NAME: &'static str = "LoxoneMCP";
    #[allow(dead_code)] // Used in environment variable access
    const USERNAME_KEY: &'static str = "LOXONE_USERNAME";
    #[allow(dead_code)] // Used in environment variable access
    const PASSWORD_KEY: &'static str = "LOXONE_PASSWORD";
    #[allow(dead_code)] // Used in keyring feature
    const HOST_KEY: &'static str = "LOXONE_HOST";
    #[allow(dead_code)] // Used in environment variable access
    const API_KEY_KEY: &'static str = "LOXONE_API_KEY";
}

impl CredentialManager {
    /// Create a new credential manager
    pub fn new(store: CredentialStore) -> Self {
        Self {
            store,
            #[cfg(feature = "infisical")]
            infisical_client: None,
            // wasi_manager removed - feature not available
        }
    }

    /// Create a new credential manager with async initialization
    pub async fn new_async(store: CredentialStore) -> Result<Self> {
        #[allow(unused_mut)]
        let mut manager = Self::new(store.clone());

        match &store {
            #[cfg(feature = "infisical")]
            CredentialStore::Infisical {
                project_id,
                environment,
                client_id,
                client_secret,
                host,
            } => {
                let client = create_authenticated_client(
                    project_id.clone(),
                    environment.clone(),
                    client_id.clone(),
                    client_secret.clone(),
                    host.clone(),
                )
                .await?;
                manager.infisical_client = Some(client);
            }

            // WasiKeyValue support removed - feature not available
            _ => {}
        }

        Ok(manager)
    }

    /// Store credentials securely
    pub async fn store_credentials(&self, credentials: &LoxoneCredentials) -> Result<()> {
        match &self.store {
            #[cfg(feature = "keyring-storage")]
            CredentialStore::Keyring => self.store_keyring(credentials).await,

            CredentialStore::Environment => self.store_environment(credentials).await,

            #[cfg(target_arch = "wasm32")]
            CredentialStore::LocalStorage => self.store_local_storage(credentials).await,

            CredentialStore::FileSystem { path } => self.store_file_system(credentials, path).await,

            #[cfg(feature = "infisical")]
            CredentialStore::Infisical { .. } => self.store_infisical(credentials).await,
            // WasiKeyValue support removed
        }
    }

    /// Retrieve credentials
    pub async fn get_credentials(&self) -> Result<LoxoneCredentials> {
        match &self.store {
            #[cfg(feature = "keyring-storage")]
            CredentialStore::Keyring => self.get_keyring().await,

            CredentialStore::Environment => self.get_environment().await,

            #[cfg(target_arch = "wasm32")]
            CredentialStore::LocalStorage => self.get_local_storage().await,

            CredentialStore::FileSystem { path } => self.get_file_system(path).await,

            #[cfg(feature = "infisical")]
            CredentialStore::Infisical { .. } => self.get_infisical().await,
            // WasiKeyValue support removed
        }
    }

    /// Clear stored credentials
    pub async fn clear_credentials(&self) -> Result<()> {
        match &self.store {
            #[cfg(feature = "keyring-storage")]
            CredentialStore::Keyring => self.clear_keyring().await,

            CredentialStore::Environment => {
                // Cannot clear environment variables
                Err(LoxoneError::credentials(
                    "Cannot clear environment variables",
                ))
            }

            #[cfg(target_arch = "wasm32")]
            CredentialStore::LocalStorage => self.clear_local_storage().await,

            CredentialStore::FileSystem { path } => self.clear_file_system(path).await,

            #[cfg(feature = "infisical")]
            CredentialStore::Infisical { .. } => self.clear_infisical().await,
            // WasiKeyValue support removed
        }
    }

    /// Validate that credentials exist and are accessible
    pub async fn validate_credentials(&self) -> Result<bool> {
        match self.get_credentials().await {
            Ok(credentials) => {
                // Basic validation
                if credentials.username.is_empty() || credentials.password.is_empty() {
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
            Err(_) => Ok(false),
        }
    }

    /// Get both credentials and host URL in a single operation
    /// This method provides compatibility for existing code
    pub async fn get_credentials_with_host(&self) -> Result<(LoxoneCredentials, Option<String>)> {
        match &self.store {
            #[cfg(feature = "keyring-storage")]
            CredentialStore::Keyring => self.get_keyring_with_host().await,
            _ => {
                let credentials = self.get_credentials().await?;
                Ok((credentials, None))
            }
        }
    }
}

// Native keyring implementation
#[cfg(feature = "keyring-storage")]
impl CredentialManager {
    async fn store_keyring(&self, _credentials: &LoxoneCredentials) -> Result<()> {
        // Keyring storage is disabled due to unmaintained dependencies
        Err(LoxoneError::credentials(
            "Keyring storage is disabled due to unmaintained dependencies",
        ))
    }

    async fn get_keyring(&self) -> Result<LoxoneCredentials> {
        // Keyring storage is disabled due to unmaintained dependencies
        Err(LoxoneError::credentials(
            "Keyring storage is disabled due to unmaintained dependencies",
        ))
    }

    async fn get_keyring_with_host(&self) -> Result<(LoxoneCredentials, Option<String>)> {
        #[cfg(feature = "keyring-storage")]
        {
            // Try security command-line tool first (often avoids prompts on macOS)
            #[cfg(target_os = "macos")]
            {
                use crate::config::security_keychain::SecurityKeychain;

                if SecurityKeychain::is_available() {
                    tracing::debug!("Trying macOS security command for keychain access...");
                    match SecurityKeychain::get_all_credentials() {
                        Ok((username, password, host_url, api_key)) => {
                            tracing::info!(
                                "✅ Credentials loaded via security command (no prompts)"
                            );
                            let credentials = LoxoneCredentials {
                                username,
                                password,
                                api_key,
                                #[cfg(feature = "crypto-openssl")]
                                public_key: None,
                            };
                            return Ok((credentials, host_url));
                        }
                        Err(e) => {
                            tracing::debug!(
                                "Security command failed, falling back to keyring crate: {}",
                                e
                            );
                            // Fall through to keyring crate
                        }
                    }
                }
            }

            // Keyring feature is disabled, return error
            Err(LoxoneError::credentials(
                "Keyring storage is disabled due to unmaintained dependencies",
            ))
        }

        #[cfg(not(feature = "keyring-storage"))]
        {
            Err(LoxoneError::credentials(
                "Keyring storage is disabled due to unmaintained dependencies",
            ))
        }
    }

    /// Get host URL from keychain (for Python compatibility)
    pub async fn get_host_url(&self) -> Result<String> {
        match &self.store {
            #[cfg(feature = "keyring-storage")]
            CredentialStore::Keyring => Err(LoxoneError::credentials(
                "Keyring storage is disabled due to unmaintained dependencies",
            )),
            _ => Err(LoxoneError::credentials(
                "Host URL only available from keyring",
            )),
        }
    }

    /// Store host URL in keychain (for Python compatibility)
    #[cfg(feature = "keyring-storage")]
    pub async fn store_host_url(&self, _host_url: &str) -> Result<()> {
        match &self.store {
            CredentialStore::Keyring => Err(LoxoneError::credentials(
                "Keyring storage is disabled due to unmaintained dependencies",
            )),
            _ => Err(LoxoneError::credentials(
                "Host URL storage only available for keyring",
            )),
        }
    }

    async fn clear_keyring(&self) -> Result<()> {
        #[cfg(feature = "keyring-storage")]
        {
            Err(LoxoneError::credentials(
                "Keyring storage is disabled due to unmaintained dependencies",
            ))
        }

        #[cfg(not(feature = "keyring-storage"))]
        {
            Err(LoxoneError::credentials(
                "Keyring storage is disabled due to unmaintained dependencies",
            ))
        }
    }
}

// Environment variable implementation
impl CredentialManager {
    async fn store_environment(&self, _credentials: &LoxoneCredentials) -> Result<()> {
        Err(LoxoneError::credentials(
            "Cannot store credentials in environment variables. Set LOXONE_USERNAME and LOXONE_PASSWORD manually."
        ))
    }

    async fn get_environment(&self) -> Result<LoxoneCredentials> {
        let username = env::var(Self::USERNAME_KEY).map_err(|_| {
            LoxoneError::credentials(format!(
                "{} environment variable not set",
                Self::USERNAME_KEY
            ))
        })?;

        let password = env::var(Self::PASSWORD_KEY).map_err(|_| {
            LoxoneError::credentials(format!(
                "{} environment variable not set",
                Self::PASSWORD_KEY
            ))
        })?;

        // Try new name first, then fall back to old name for compatibility
        let api_key = env::var("LOXONE_API_KEY")
            .or_else(|_| env::var("LOXONE_SSE_API_KEY"))
            .ok();

        Ok(LoxoneCredentials {
            username,
            password,
            api_key,
            #[cfg(feature = "crypto-openssl")]
            public_key: env::var("LOXONE_PUBLIC_KEY").ok(),
        })
    }
}

// WASM local storage implementation
#[cfg(target_arch = "wasm32")]
impl CredentialManager {
    async fn store_local_storage(&self, credentials: &LoxoneCredentials) -> Result<()> {
        use wasm_bindgen::JsValue;

        let window = web_sys::window()
            .ok_or_else(|| LoxoneError::credentials("No window object available"))?;

        let storage = window
            .local_storage()
            .map_err(|_| LoxoneError::credentials("Local storage not available"))?
            .ok_or_else(|| LoxoneError::credentials("Local storage is None"))?;

        // Store credentials as JSON
        let creds_json = serde_json::to_string(credentials).map_err(|e| {
            LoxoneError::credentials(format!("Failed to serialize credentials: {e}"))
        })?;

        storage
            .set_item("loxone_credentials", &creds_json)
            .map_err(|_| {
                LoxoneError::credentials("Failed to store credentials in local storage")
            })?;

        Ok(())
    }

    async fn get_local_storage(&self) -> Result<LoxoneCredentials> {
        let window = web_sys::window()
            .ok_or_else(|| LoxoneError::credentials("No window object available"))?;

        let storage = window
            .local_storage()
            .map_err(|_| LoxoneError::credentials("Local storage not available"))?
            .ok_or_else(|| LoxoneError::credentials("Local storage is None"))?;

        let creds_json = storage
            .get_item("loxone_credentials")
            .map_err(|_| LoxoneError::credentials("Failed to access local storage"))?
            .ok_or_else(|| LoxoneError::credentials("No credentials found in local storage"))?;

        let credentials: LoxoneCredentials = serde_json::from_str(&creds_json).map_err(|e| {
            LoxoneError::credentials(format!("Failed to parse stored credentials: {}", e))
        })?;

        Ok(credentials)
    }

    async fn clear_local_storage(&self) -> Result<()> {
        let window = web_sys::window()
            .ok_or_else(|| LoxoneError::credentials("No window object available"))?;

        let storage = window
            .local_storage()
            .map_err(|_| LoxoneError::credentials("Local storage not available"))?
            .ok_or_else(|| LoxoneError::credentials("Local storage is None"))?;

        storage.remove_item("loxone_credentials").map_err(|_| {
            LoxoneError::credentials("Failed to clear credentials from local storage")
        })?;

        Ok(())
    }
}

// File system implementation (for WASI)
impl CredentialManager {
    async fn store_file_system(&self, credentials: &LoxoneCredentials, path: &str) -> Result<()> {
        use std::fs;

        let creds_json = serde_json::to_string_pretty(credentials).map_err(|e| {
            LoxoneError::credentials(format!("Failed to serialize credentials: {e}"))
        })?;

        fs::write(path, creds_json).map_err(|e| {
            LoxoneError::credentials(format!("Failed to write credentials file: {e}"))
        })?;

        Ok(())
    }

    async fn get_file_system(&self, path: &str) -> Result<LoxoneCredentials> {
        use std::fs;

        let creds_json = fs::read_to_string(path).map_err(|e| {
            LoxoneError::credentials(format!("Failed to read credentials file: {e}"))
        })?;

        let credentials: LoxoneCredentials = serde_json::from_str(&creds_json).map_err(|e| {
            LoxoneError::credentials(format!("Failed to parse credentials file: {e}"))
        })?;

        Ok(credentials)
    }

    async fn clear_file_system(&self, path: &str) -> Result<()> {
        use std::fs;

        fs::remove_file(path).map_err(|e| {
            LoxoneError::credentials(format!("Failed to remove credentials file: {e}"))
        })?;

        Ok(())
    }
}

// Infisical implementation
#[cfg(feature = "infisical")]
impl CredentialManager {
    async fn store_infisical(&self, credentials: &LoxoneCredentials) -> Result<()> {
        let client = self
            .infisical_client
            .as_ref()
            .ok_or_else(|| LoxoneError::credentials("Infisical client not initialized"))?;

        // Store username
        client
            .set_secret(Self::USERNAME_KEY, &credentials.username)
            .await?;

        // Store password
        client
            .set_secret(Self::PASSWORD_KEY, &credentials.password)
            .await?;

        // Store API key if present
        if let Some(api_key) = &credentials.api_key {
            client.set_secret(Self::API_KEY_KEY, api_key).await?;
        }

        #[cfg(feature = "crypto-openssl")]
        if let Some(public_key) = &credentials.public_key {
            client.set_secret("LOXONE_PUBLIC_KEY", public_key).await?;
        }

        tracing::info!("Credentials stored successfully in Infisical");
        Ok(())
    }

    async fn get_infisical(&self) -> Result<LoxoneCredentials> {
        let client = self
            .infisical_client
            .as_ref()
            .ok_or_else(|| LoxoneError::credentials("Infisical client not initialized"))?;

        // Get username
        let username = client.get_secret(Self::USERNAME_KEY).await.map_err(|e| {
            LoxoneError::credentials(format!("Failed to get username from Infisical: {e}"))
        })?;

        // Get password
        let password = client.get_secret(Self::PASSWORD_KEY).await.map_err(|e| {
            LoxoneError::credentials(format!("Failed to get password from Infisical: {e}"))
        })?;

        // Get API key (optional)
        let api_key = client.get_secret(Self::API_KEY_KEY).await.ok();

        #[cfg(feature = "crypto-openssl")]
        let public_key = client.get_secret("LOXONE_PUBLIC_KEY").await.ok();

        Ok(LoxoneCredentials {
            username,
            password,
            api_key,
            #[cfg(feature = "crypto-openssl")]
            public_key,
        })
    }

    async fn clear_infisical(&self) -> Result<()> {
        let client = self
            .infisical_client
            .as_ref()
            .ok_or_else(|| LoxoneError::credentials("Infisical client not initialized"))?;

        // Delete credentials (ignore errors for non-existent secrets)
        let _ = client.delete_secret(Self::USERNAME_KEY).await;
        let _ = client.delete_secret(Self::PASSWORD_KEY).await;
        let _ = client.delete_secret(Self::API_KEY_KEY).await;

        #[cfg(feature = "crypto-openssl")]
        {
            let _ = client.delete_secret("LOXONE_PUBLIC_KEY").await;
        }

        tracing::info!("Credentials cleared from Infisical");
        Ok(())
    }
}

// WASI keyvalue implementation removed - feature not available

/// Convenience function to create credentials from username/password
pub fn create_credentials(username: String, password: String) -> LoxoneCredentials {
    LoxoneCredentials {
        username,
        password,
        api_key: None,
        #[cfg(feature = "crypto-openssl")]
        public_key: None,
    }
}

/// Multi-backend credential manager with automatic fallback
pub struct MultiBackendCredentialManager {
    backends: Vec<CredentialManager>,
}

impl MultiBackendCredentialManager {
    /// Create a new multi-backend credential manager
    pub async fn new(stores: Vec<CredentialStore>) -> Result<Self> {
        let mut backends = Vec::new();

        for store in stores {
            match CredentialManager::new_async(store).await {
                Ok(manager) => backends.push(manager),
                Err(e) => {
                    tracing::warn!("Failed to initialize credential backend: {}", e);
                    // Continue with other backends
                }
            }
        }

        if backends.is_empty() {
            return Err(LoxoneError::credentials("No credential backends available"));
        }

        Ok(Self { backends })
    }

    /// Get credentials from the first available backend
    pub async fn get_credentials(&self) -> Result<LoxoneCredentials> {
        let mut last_error = None;

        for backend in &self.backends {
            match backend.get_credentials().await {
                Ok(credentials) => return Ok(credentials),
                Err(e) => {
                    tracing::debug!("Backend failed, trying next: {}", e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| LoxoneError::credentials("No backends available")))
    }

    /// Store credentials in all backends
    pub async fn store_credentials(&self, credentials: &LoxoneCredentials) -> Result<()> {
        let mut errors = Vec::new();
        let mut success_count = 0;

        for backend in &self.backends {
            match backend.store_credentials(credentials).await {
                Ok(()) => success_count += 1,
                Err(e) => {
                    tracing::warn!("Failed to store credentials in backend: {}", e);
                    errors.push(e);
                }
            }
        }

        if success_count == 0 {
            let error_msg = errors
                .into_iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(LoxoneError::credentials(format!(
                "Failed to store credentials in any backend: {error_msg}"
            )));
        }

        tracing::info!(
            "Credentials stored successfully in {}/{} backends",
            success_count,
            self.backends.len()
        );
        Ok(())
    }
}

/// Factory function to create the best available credential manager
/// Priority order: Infisical -> Environment -> WASI/LocalStorage -> Keychain (fallback)
pub async fn create_best_credential_manager() -> Result<MultiBackendCredentialManager> {
    let mut stores = Vec::new();
    let mut infisical_configured = false;
    // Check if environment variables for Loxone are configured
    let env_configured =
        std::env::var("LOXONE_USERNAME").is_ok() && std::env::var("LOXONE_PASSWORD").is_ok();

    // Try Infisical first if configured (preferred for team environments)
    #[cfg(feature = "infisical")]
    {
        if let (Ok(project_id), Ok(client_id), Ok(client_secret)) = (
            std::env::var("INFISICAL_PROJECT_ID"),
            std::env::var("INFISICAL_CLIENT_ID"),
            std::env::var("INFISICAL_CLIENT_SECRET"),
        ) {
            let environment =
                std::env::var("INFISICAL_ENVIRONMENT").unwrap_or_else(|_| "dev".to_string());
            let host = std::env::var("INFISICAL_HOST").ok();

            stores.push(CredentialStore::Infisical {
                project_id,
                environment,
                client_id,
                client_secret,
                host,
            });
            infisical_configured = true;
            tracing::info!("🔐 Using Infisical credential backend");
        }
    }

    // Try environment variables second (CI/CD friendly)
    stores.push(CredentialStore::Environment);

    // WASI keyvalue support removed - feature not available

    // Try local storage in WASM environments
    #[cfg(target_arch = "wasm32")]
    {
        stores.push(CredentialStore::LocalStorage);
    }

    // Try keyring on native platforms as final fallback
    #[cfg(all(feature = "keyring-storage", not(target_arch = "wasm32")))]
    {
        stores.push(CredentialStore::Keyring);
    }

    let manager = MultiBackendCredentialManager::new(stores).await?;

    // Log which backend will actually be used based on what's configured
    if infisical_configured {
        tracing::info!("📋 Credential source: Infisical (team configuration)");
    } else if env_configured {
        tracing::info!("📋 Credential source: Environment variables");
        tracing::debug!("Using LOXONE_USERNAME and LOXONE_PASSWORD");
    } else {
        #[cfg(all(feature = "keyring-storage", not(target_arch = "wasm32")))]
        {
            tracing::info!("📋 Credential source: System keychain (fallback)");
            tracing::info!(
                "💡 Tip: Set LOXONE_USERNAME and LOXONE_PASSWORD for direct configuration"
            );
        }
        #[cfg(not(all(feature = "keyring-storage", not(target_arch = "wasm32"))))]
        {
            tracing::warn!(
                "⚠️  No credentials configured. Set LOXONE_USERNAME and LOXONE_PASSWORD"
            );
        }
    }

    // Only show setup instructions if no backend is configured
    if !infisical_configured && !env_configured {
        #[cfg(all(feature = "keyring-storage", not(target_arch = "wasm32")))]
        {
            tracing::info!("🔧 Run setup to configure credentials: ./loxone-mcp-rust setup");
        }
        #[cfg(not(all(feature = "keyring-storage", not(target_arch = "wasm32"))))]
        {
            tracing::info!("🔧 Configure credentials with environment variables:");
            tracing::info!("   export LOXONE_USERNAME=\"your-username\"");
            tracing::info!("   export LOXONE_PASSWORD=\"your-password\"");
            tracing::info!("   export LOXONE_HOST=\"192.168.1.100\"");
        }
    }

    Ok(manager)
}
