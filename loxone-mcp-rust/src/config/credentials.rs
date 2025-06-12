//! Credential management for Loxone authentication
//!
//! This module provides secure credential storage and retrieval across
//! different platforms including native systems and WASM environments.

use crate::error::{LoxoneError, Result};
use crate::config::CredentialStore;
use serde::{Deserialize, Serialize};
use std::env;

#[cfg(feature = "infisical")]
use crate::config::infisical_client::{create_authenticated_client, InfisicalClient};

#[cfg(feature = "wasi-keyvalue")]
use crate::config::wasi_keyvalue::WasiKeyValueManager;

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
    #[cfg(feature = "crypto")]
    pub public_key: Option<String>,
}

/// Credential manager for secure storage and retrieval
pub struct CredentialManager {
    store: CredentialStore,
    
    #[cfg(feature = "infisical")]
    infisical_client: Option<InfisicalClient>,
    
    #[cfg(feature = "wasi-keyvalue")]
    wasi_manager: Option<WasiKeyValueManager>,
}

// Credential key constants (shared across all backends)
impl CredentialManager {
    #[allow(dead_code)] // Used in keyring feature
    const SERVICE_NAME: &'static str = "LoxoneMCP";
    const USERNAME_KEY: &'static str = "LOXONE_USER";
    const PASSWORD_KEY: &'static str = "LOXONE_PASS";
    #[allow(dead_code)] // Used in keyring feature
    const HOST_KEY: &'static str = "LOXONE_HOST";
    const API_KEY_KEY: &'static str = "LOXONE_API_KEY";
}

impl CredentialManager {
    /// Create a new credential manager
    pub fn new(store: CredentialStore) -> Self {
        Self { 
            store,
            #[cfg(feature = "infisical")]
            infisical_client: None,
            #[cfg(feature = "wasi-keyvalue")]
            wasi_manager: None,
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
                host 
            } => {
                let client = create_authenticated_client(
                    project_id.clone(),
                    environment.clone(),
                    client_id.clone(),
                    client_secret.clone(),
                    host.clone(),
                ).await?;
                manager.infisical_client = Some(client);
            }
            
            #[cfg(feature = "wasi-keyvalue")]
            CredentialStore::WasiKeyValue { store_name } => {
                let wasi_manager = WasiKeyValueManager::new(store_name.clone())?;
                manager.wasi_manager = Some(wasi_manager);
            }
            
            _ => {}
        }
        
        Ok(manager)
    }
    
    /// Store credentials securely
    pub async fn store_credentials(&self, credentials: &LoxoneCredentials) -> Result<()> {
        match &self.store {
            #[cfg(feature = "keyring-storage")]
            CredentialStore::Keyring => {
                self.store_keyring(credentials).await
            }
            
            CredentialStore::Environment => {
                self.store_environment(credentials).await
            }
            
            #[cfg(target_arch = "wasm32")]
            CredentialStore::LocalStorage => {
                self.store_local_storage(credentials).await
            }
            
            CredentialStore::FileSystem { path } => {
                self.store_file_system(credentials, path).await
            }
            
            #[cfg(feature = "infisical")]
            CredentialStore::Infisical { .. } => {
                self.store_infisical(credentials).await
            }
            
            #[cfg(feature = "wasi-keyvalue")]
            CredentialStore::WasiKeyValue { .. } => {
                self.store_wasi_keyvalue(credentials).await
            }
        }
    }
    
    /// Retrieve credentials
    pub async fn get_credentials(&self) -> Result<LoxoneCredentials> {
        match &self.store {
            #[cfg(feature = "keyring-storage")]
            CredentialStore::Keyring => {
                self.get_keyring().await
            }
            
            CredentialStore::Environment => {
                self.get_environment().await
            }
            
            #[cfg(target_arch = "wasm32")]
            CredentialStore::LocalStorage => {
                self.get_local_storage().await
            }
            
            CredentialStore::FileSystem { path } => {
                self.get_file_system(path).await
            }
            
            #[cfg(feature = "infisical")]
            CredentialStore::Infisical { .. } => {
                self.get_infisical().await
            }
            
            #[cfg(feature = "wasi-keyvalue")]
            CredentialStore::WasiKeyValue { .. } => {
                self.get_wasi_keyvalue().await
            }
        }
    }
    
    /// Clear stored credentials
    pub async fn clear_credentials(&self) -> Result<()> {
        match &self.store {
            #[cfg(feature = "keyring-storage")]
            CredentialStore::Keyring => {
                self.clear_keyring().await
            }
            
            CredentialStore::Environment => {
                // Cannot clear environment variables
                Err(LoxoneError::credentials("Cannot clear environment variables"))
            }
            
            #[cfg(target_arch = "wasm32")]
            CredentialStore::LocalStorage => {
                self.clear_local_storage().await
            }
            
            CredentialStore::FileSystem { path } => {
                self.clear_file_system(path).await
            }
            
            #[cfg(feature = "infisical")]
            CredentialStore::Infisical { .. } => {
                self.clear_infisical().await
            }
            
            #[cfg(feature = "wasi-keyvalue")]
            CredentialStore::WasiKeyValue { .. } => {
                self.clear_wasi_keyvalue().await
            }
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
            CredentialStore::Keyring => {
                self.get_keyring_with_host().await
            }
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
    
    async fn store_keyring(&self, credentials: &LoxoneCredentials) -> Result<()> {
        use keyring::Entry;
        
        // Store username
        let username_entry = Entry::new(Self::SERVICE_NAME, Self::USERNAME_KEY)?;
        username_entry.set_password(&credentials.username)?;
        
        // Store password
        let password_entry = Entry::new(Self::SERVICE_NAME, Self::PASSWORD_KEY)?;
        password_entry.set_password(&credentials.password)?;
        
        // Store API key if present
        if let Some(api_key) = &credentials.api_key {
            let api_key_entry = Entry::new(Self::SERVICE_NAME, Self::API_KEY_KEY)?;
            api_key_entry.set_password(api_key)?;
        }
        
        Ok(())
    }
    
    async fn get_keyring(&self) -> Result<LoxoneCredentials> {
        use keyring::Entry;
        
        // Get username
        let username_entry = Entry::new(Self::SERVICE_NAME, Self::USERNAME_KEY)?;
        let username = username_entry.get_password()
            .map_err(|e| LoxoneError::credentials(format!("Failed to get username: {}", e)))?;
        
        // Get password
        let password_entry = Entry::new(Self::SERVICE_NAME, Self::PASSWORD_KEY)?;
        let password = password_entry.get_password()
            .map_err(|e| LoxoneError::credentials(format!("Failed to get password: {}", e)))?;
        
        // Get API key (optional)
        let api_key = {
            let api_key_entry = Entry::new(Self::SERVICE_NAME, Self::API_KEY_KEY)?;
            api_key_entry.get_password().ok()
        };
        
        // Note: Host URL is stored in keychain but needs to be retrieved separately
        // and applied to the config. This is handled in get_credentials_with_host()
        
        Ok(LoxoneCredentials {
            username,
            password,
            api_key,
            #[cfg(feature = "crypto")]
            public_key: None,
        })
    }
    
    #[cfg(feature = "keyring-storage")]
    async fn get_keyring_with_host(&self) -> Result<(LoxoneCredentials, Option<String>)> {
        // Try security command-line tool first (often avoids prompts on macOS)
        #[cfg(target_os = "macos")]
        {
            use crate::config::security_keychain::SecurityKeychain;
            
            if SecurityKeychain::is_available() {
                tracing::debug!("Trying macOS security command for keychain access...");
                match SecurityKeychain::get_all_credentials() {
                    Ok((username, password, host_url, api_key)) => {
                        tracing::info!("✅ Credentials loaded via security command (no prompts)");
                        let credentials = LoxoneCredentials {
                            username,
                            password,
                            api_key,
                            #[cfg(feature = "crypto")]
                            public_key: None,
                        };
                        return Ok((credentials, host_url));
                    }
                    Err(e) => {
                        tracing::debug!("Security command failed, falling back to keyring crate: {}", e);
                        // Fall through to keyring crate
                    }
                }
            }
        }
        
        // Fallback to keyring crate (may prompt)
        use keyring::Entry;
        
        tracing::debug!("Using keyring crate for keychain access (may prompt)...");
        
        // Try to unlock keychain once to reduce prompts
        let service_name = Self::SERVICE_NAME;
        
        // First, try to get one entry to trigger any required authentication
        let test_entry = Entry::new(service_name, Self::USERNAME_KEY)?;
        
        // If this succeeds, subsequent calls might not prompt
        let username = test_entry.get_password()
            .map_err(|e| LoxoneError::credentials(format!("Failed to get username: {}", e)))?;
        
        // Now get the rest - these should hopefully not prompt since we're authenticated
        let password_entry = Entry::new(service_name, Self::PASSWORD_KEY)?;
        let password = password_entry.get_password()
            .map_err(|e| LoxoneError::credentials(format!("Failed to get password: {}", e)))?;
        
        let host_entry = Entry::new(service_name, Self::HOST_KEY)?;
        let host_url = host_entry.get_password().ok();
        
        let api_key_entry = Entry::new(service_name, Self::API_KEY_KEY)?;
        let api_key = api_key_entry.get_password().ok();
        
        let credentials = LoxoneCredentials {
            username,
            password,
            api_key,
            #[cfg(feature = "crypto")]
            public_key: None,
        };
        
        Ok((credentials, host_url))
    }
    
    /// Get host URL from keychain (for Python compatibility)
    pub async fn get_host_url(&self) -> Result<String> {
        match &self.store {
            #[cfg(feature = "keyring-storage")]
            CredentialStore::Keyring => {
                use keyring::Entry;
                let host_entry = Entry::new(Self::SERVICE_NAME, Self::HOST_KEY)?;
                host_entry.get_password()
                    .map_err(|e| LoxoneError::credentials(format!("Failed to get host URL: {}", e)))
            }
            _ => Err(LoxoneError::credentials("Host URL only available from keyring"))
        }
    }
    
    /// Store host URL in keychain (for Python compatibility)
    #[cfg(feature = "keyring-storage")]
    pub async fn store_host_url(&self, host_url: &str) -> Result<()> {
        match &self.store {
            CredentialStore::Keyring => {
                use keyring::Entry;
                let host_entry = Entry::new(Self::SERVICE_NAME, Self::HOST_KEY)?;
                host_entry.set_password(host_url)
                    .map_err(|e| LoxoneError::credentials(format!("Failed to store host URL: {}", e)))?;
                Ok(())
            }
            _ => Err(LoxoneError::credentials("Host URL storage only available for keyring"))
        }
    }
    
    async fn clear_keyring(&self) -> Result<()> {
        use keyring::Entry;
        
        // Clear username
        let username_entry = Entry::new(Self::SERVICE_NAME, Self::USERNAME_KEY)?;
        username_entry.delete_password().ok();
        
        // Clear password
        let password_entry = Entry::new(Self::SERVICE_NAME, Self::PASSWORD_KEY)?;
        password_entry.delete_password().ok();
        
        // Clear API key
        let api_key_entry = Entry::new(Self::SERVICE_NAME, Self::API_KEY_KEY)?;
        api_key_entry.delete_password().ok();
        
        Ok(())
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
        let username = env::var("LOXONE_USERNAME")
            .map_err(|_| LoxoneError::credentials("LOXONE_USERNAME environment variable not set"))?;
        
        let password = env::var("LOXONE_PASSWORD")
            .map_err(|_| LoxoneError::credentials("LOXONE_PASSWORD environment variable not set"))?;
        
        // Try new name first, then fall back to old name for compatibility
        let api_key = env::var("LOXONE_API_KEY")
            .or_else(|_| env::var("LOXONE_SSE_API_KEY"))
            .ok();
        
        Ok(LoxoneCredentials {
            username,
            password,
            api_key,
            #[cfg(feature = "crypto")]
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
        
        let storage = window.local_storage()
            .map_err(|_| LoxoneError::credentials("Local storage not available"))?
            .ok_or_else(|| LoxoneError::credentials("Local storage is None"))?;
        
        // Store credentials as JSON
        let creds_json = serde_json::to_string(credentials)
            .map_err(|e| LoxoneError::credentials(format!("Failed to serialize credentials: {}", e)))?;
        
        storage.set_item("loxone_credentials", &creds_json)
            .map_err(|_| LoxoneError::credentials("Failed to store credentials in local storage"))?;
        
        Ok(())
    }
    
    async fn get_local_storage(&self) -> Result<LoxoneCredentials> {
        let window = web_sys::window()
            .ok_or_else(|| LoxoneError::credentials("No window object available"))?;
        
        let storage = window.local_storage()
            .map_err(|_| LoxoneError::credentials("Local storage not available"))?
            .ok_or_else(|| LoxoneError::credentials("Local storage is None"))?;
        
        let creds_json = storage.get_item("loxone_credentials")
            .map_err(|_| LoxoneError::credentials("Failed to access local storage"))?
            .ok_or_else(|| LoxoneError::credentials("No credentials found in local storage"))?;
        
        let credentials: LoxoneCredentials = serde_json::from_str(&creds_json)
            .map_err(|e| LoxoneError::credentials(format!("Failed to parse stored credentials: {}", e)))?;
        
        Ok(credentials)
    }
    
    async fn clear_local_storage(&self) -> Result<()> {
        let window = web_sys::window()
            .ok_or_else(|| LoxoneError::credentials("No window object available"))?;
        
        let storage = window.local_storage()
            .map_err(|_| LoxoneError::credentials("Local storage not available"))?
            .ok_or_else(|| LoxoneError::credentials("Local storage is None"))?;
        
        storage.remove_item("loxone_credentials")
            .map_err(|_| LoxoneError::credentials("Failed to clear credentials from local storage"))?;
        
        Ok(())
    }
}

// File system implementation (for WASI)
impl CredentialManager {
    async fn store_file_system(&self, credentials: &LoxoneCredentials, path: &str) -> Result<()> {
        use std::fs;
        
        let creds_json = serde_json::to_string_pretty(credentials)
            .map_err(|e| LoxoneError::credentials(format!("Failed to serialize credentials: {}", e)))?;
        
        fs::write(path, creds_json)
            .map_err(|e| LoxoneError::credentials(format!("Failed to write credentials file: {}", e)))?;
        
        Ok(())
    }
    
    async fn get_file_system(&self, path: &str) -> Result<LoxoneCredentials> {
        use std::fs;
        
        let creds_json = fs::read_to_string(path)
            .map_err(|e| LoxoneError::credentials(format!("Failed to read credentials file: {}", e)))?;
        
        let credentials: LoxoneCredentials = serde_json::from_str(&creds_json)
            .map_err(|e| LoxoneError::credentials(format!("Failed to parse credentials file: {}", e)))?;
        
        Ok(credentials)
    }
    
    async fn clear_file_system(&self, path: &str) -> Result<()> {
        use std::fs;
        
        fs::remove_file(path)
            .map_err(|e| LoxoneError::credentials(format!("Failed to remove credentials file: {}", e)))?;
        
        Ok(())
    }
}

// Infisical implementation
#[cfg(feature = "infisical")]
impl CredentialManager {
    async fn store_infisical(&self, credentials: &LoxoneCredentials) -> Result<()> {
        let client = self.infisical_client.as_ref()
            .ok_or_else(|| LoxoneError::credentials("Infisical client not initialized"))?;
        
        // Store username
        client.set_secret(Self::USERNAME_KEY, &credentials.username).await?;
        
        // Store password
        client.set_secret(Self::PASSWORD_KEY, &credentials.password).await?;
        
        // Store API key if present
        if let Some(api_key) = &credentials.api_key {
            client.set_secret(Self::API_KEY_KEY, api_key).await?;
        }
        
        #[cfg(feature = "crypto")]
        if let Some(public_key) = &credentials.public_key {
            client.set_secret("LOXONE_PUBLIC_KEY", public_key).await?;
        }
        
        tracing::info!("Credentials stored successfully in Infisical");
        Ok(())
    }
    
    async fn get_infisical(&self) -> Result<LoxoneCredentials> {
        let client = self.infisical_client.as_ref()
            .ok_or_else(|| LoxoneError::credentials("Infisical client not initialized"))?;
        
        // Get username
        let username = client.get_secret(Self::USERNAME_KEY).await
            .map_err(|e| LoxoneError::credentials(format!("Failed to get username from Infisical: {}", e)))?;
        
        // Get password
        let password = client.get_secret(Self::PASSWORD_KEY).await
            .map_err(|e| LoxoneError::credentials(format!("Failed to get password from Infisical: {}", e)))?;
        
        // Get API key (optional)
        let api_key = client.get_secret(Self::API_KEY_KEY).await.ok();
        
        #[cfg(feature = "crypto")]
        let public_key = client.get_secret("LOXONE_PUBLIC_KEY").await.ok();
        
        Ok(LoxoneCredentials {
            username,
            password,
            api_key,
            #[cfg(feature = "crypto")]
            public_key,
        })
    }
    
    async fn clear_infisical(&self) -> Result<()> {
        let client = self.infisical_client.as_ref()
            .ok_or_else(|| LoxoneError::credentials("Infisical client not initialized"))?;
        
        // Delete credentials (ignore errors for non-existent secrets)
        let _ = client.delete_secret(Self::USERNAME_KEY).await;
        let _ = client.delete_secret(Self::PASSWORD_KEY).await;
        let _ = client.delete_secret(Self::API_KEY_KEY).await;
        
        #[cfg(feature = "crypto")]
        {
            let _ = client.delete_secret("LOXONE_PUBLIC_KEY").await;
        }
        
        tracing::info!("Credentials cleared from Infisical");
        Ok(())
    }
}

// WASI keyvalue implementation
#[cfg(feature = "wasi-keyvalue")]
impl CredentialManager {
    async fn store_wasi_keyvalue(&self, credentials: &LoxoneCredentials) -> Result<()> {
        let manager = self.wasi_manager.as_ref()
            .ok_or_else(|| LoxoneError::credentials("WASI keyvalue manager not initialized"))?;
        
        // Store username
        manager.set_credential(Self::USERNAME_KEY, &credentials.username).await?;
        
        // Store password
        manager.set_credential(Self::PASSWORD_KEY, &credentials.password).await?;
        
        // Store API key if present
        if let Some(api_key) = &credentials.api_key {
            manager.set_credential(Self::API_KEY_KEY, api_key).await?;
        }
        
        #[cfg(feature = "crypto")]
        if let Some(public_key) = &credentials.public_key {
            manager.set_credential("LOXONE_PUBLIC_KEY", public_key).await?;
        }
        
        tracing::info!("Credentials stored successfully in WASI keyvalue store");
        Ok(())
    }
    
    async fn get_wasi_keyvalue(&self) -> Result<LoxoneCredentials> {
        let manager = self.wasi_manager.as_ref()
            .ok_or_else(|| LoxoneError::credentials("WASI keyvalue manager not initialized"))?;
        
        // Get username
        let username = manager.get_credential(Self::USERNAME_KEY).await?
            .ok_or_else(|| LoxoneError::credentials("Username not found in WASI keyvalue store"))?;
        
        // Get password
        let password = manager.get_credential(Self::PASSWORD_KEY).await?
            .ok_or_else(|| LoxoneError::credentials("Password not found in WASI keyvalue store"))?;
        
        // Get API key (optional)
        let api_key = manager.get_credential(Self::API_KEY_KEY).await?;
        
        #[cfg(feature = "crypto")]
        let public_key = manager.get_credential("LOXONE_PUBLIC_KEY").await?;
        
        Ok(LoxoneCredentials {
            username,
            password,
            api_key,
            #[cfg(feature = "crypto")]
            public_key,
        })
    }
    
    async fn clear_wasi_keyvalue(&self) -> Result<()> {
        let manager = self.wasi_manager.as_ref()
            .ok_or_else(|| LoxoneError::credentials("WASI keyvalue manager not initialized"))?;
        
        // Clear all credentials
        manager.clear_all_credentials().await?;
        
        tracing::info!("Credentials cleared from WASI keyvalue store");
        Ok(())
    }
}

/// Convenience function to create credentials from username/password
pub fn create_credentials(username: String, password: String) -> LoxoneCredentials {
    LoxoneCredentials {
        username,
        password,
        api_key: None,
        #[cfg(feature = "crypto")]
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
            let error_msg = errors.into_iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(LoxoneError::credentials(format!("Failed to store credentials in any backend: {}", error_msg)));
        }
        
        tracing::info!("Credentials stored successfully in {}/{} backends", success_count, self.backends.len());
        Ok(())
    }
}

/// Factory function to create the best available credential manager
/// Priority order: Infisical -> Environment -> WASI/LocalStorage -> Keychain (fallback)
pub async fn create_best_credential_manager() -> Result<MultiBackendCredentialManager> {
    let mut stores = Vec::new();
    
    // Try Infisical first if configured (preferred for team environments)
    #[cfg(feature = "infisical")]
    {
        if let (Ok(project_id), Ok(client_id), Ok(client_secret)) = (
            std::env::var("INFISICAL_PROJECT_ID"),
            std::env::var("INFISICAL_CLIENT_ID"),
            std::env::var("INFISICAL_CLIENT_SECRET"),
        ) {
            let environment = std::env::var("INFISICAL_ENVIRONMENT").unwrap_or_else(|_| "dev".to_string());
            let host = std::env::var("INFISICAL_HOST").ok();
            
            stores.push(CredentialStore::Infisical {
                project_id,
                environment,
                client_id,
                client_secret,
                host,
            });
            tracing::info!("✅ Infisical credential backend enabled");
        } else {
            tracing::info!("ℹ️  Infisical not configured. To enable team credential sharing:");
            tracing::info!("");
            tracing::info!("    # Example for Loxone MCP project:");
            tracing::info!("    export INFISICAL_PROJECT_ID=\"65f8e2c8a8b7d9001c4f2a3b\"");
            tracing::info!("    export INFISICAL_CLIENT_ID=\"6f4d8e91-3a2b-4c5d-9e7f-1a2b3c4d5e6f\"");
            tracing::info!("    export INFISICAL_CLIENT_SECRET=\"st.abc123def456ghi789jkl012mno345pqr678stu901vwx234yz\"");
            tracing::info!("    export INFISICAL_ENVIRONMENT=\"dev\"  # oder: staging, prod");
            tracing::info!("");
            tracing::info!("    1. Erstelle ein Infisical Konto: https://app.infisical.com/signup");
            tracing::info!("    2. Erstelle ein Projekt für dein Team");
            tracing::info!("    3. Gehe zu: Settings → Service Tokens → Create Service Token");
            tracing::info!("    4. Wähle Scopes: secrets:read, secrets:write");
            tracing::info!("    5. Kopiere die generierten IDs und ersetze die Beispielwerte oben");
        }
    }
    
    // Try environment variables second (CI/CD friendly)
    stores.push(CredentialStore::Environment);
    
    // Try WASI keyvalue in WASM environments
    #[cfg(all(feature = "wasi-keyvalue", target_arch = "wasm32"))]
    {
        stores.push(CredentialStore::WasiKeyValue { store_name: None });
    }
    
    // Try local storage in WASM environments
    #[cfg(target_arch = "wasm32")]
    {
        stores.push(CredentialStore::LocalStorage);
    }
    
    // Try keyring on native platforms as final fallback
    #[cfg(all(feature = "keyring-storage", not(target_arch = "wasm32")))]
    {
        stores.push(CredentialStore::Keyring);
        tracing::debug!("Keychain backend enabled as fallback");
    }
    
    MultiBackendCredentialManager::new(stores).await
}