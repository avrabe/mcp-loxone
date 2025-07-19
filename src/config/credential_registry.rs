//! Shared credential registry for managing multiple Loxone credentials

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use crate::error::{LoxoneError, Result};

/// Stored credential metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredential {
    /// Unique credential ID
    pub id: String,
    /// Friendly name
    pub name: String,
    /// Host information
    pub host: String,
    /// Port
    pub port: u16,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last used timestamp
    pub last_used: Option<DateTime<Utc>>,
}

/// Credential registry for managing multiple credentials
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CredentialRegistry {
    /// Map of credential ID to metadata
    pub credentials: HashMap<String, StoredCredential>,
}

impl CredentialRegistry {
    /// Registry file path
    pub fn registry_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".loxone-mcp")
            .join("registry.json")
    }

    /// Load registry from file
    pub fn load() -> Result<Self> {
        let path = Self::registry_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path).map_err(|e| {
            LoxoneError::credentials(format!("Failed to read credential registry: {e}"))
        })?;

        serde_json::from_str(&content).map_err(|e| {
            LoxoneError::credentials(format!("Failed to parse credential registry: {e}"))
        })
    }

    /// Save registry to file
    pub fn save(&self) -> Result<()> {
        let path = Self::registry_path();

        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                LoxoneError::credentials(format!("Failed to create registry directory: {e}"))
            })?;
        }

        let content = serde_json::to_string_pretty(self).map_err(|e| {
            LoxoneError::credentials(format!("Failed to serialize credential registry: {e}"))
        })?;

        fs::write(&path, content).map_err(|e| {
            LoxoneError::credentials(format!("Failed to write credential registry: {e}"))
        })?;

        Ok(())
    }

    /// Add or update a credential entry
    pub fn add_credential(&mut self, credential: StoredCredential) {
        self.credentials.insert(credential.id.clone(), credential);
    }

    /// Add a credential with auto-generated ID
    pub fn add_credential_with_id(&mut self, name: String, host: String, port: u16) -> String {
        let id = Uuid::new_v4().to_string();
        let credential = StoredCredential::new(id.clone(), name, host, port);
        self.credentials.insert(id.clone(), credential);
        id
    }

    /// Remove a credential entry
    pub fn remove_credential(&mut self, id: &str) -> Option<StoredCredential> {
        self.credentials.remove(id)
    }

    /// Get a credential entry
    pub fn get_credential(&self, id: &str) -> Option<&StoredCredential> {
        self.credentials.get(id)
    }

    /// Update last used timestamp for a credential
    pub fn update_last_used(&mut self, id: &str) -> Result<()> {
        if let Some(credential) = self.credentials.get_mut(id) {
            credential.last_used = Some(Utc::now());
            Ok(())
        } else {
            Err(LoxoneError::not_found(format!(
                "Credential not found: {id}"
            )))
        }
    }

    /// List all credential IDs
    pub fn list_credential_ids(&self) -> Vec<String> {
        self.credentials.keys().cloned().collect()
    }

    /// Check if a credential exists
    pub fn contains_credential(&self, id: &str) -> bool {
        self.credentials.contains_key(id)
    }

    /// Get credential by ID (mutable reference)
    pub fn get_credential_mut(&mut self, id: &str) -> Option<&mut StoredCredential> {
        self.credentials.get_mut(id)
    }

    /// List all credentials as a vector
    pub fn list_credentials(&self) -> Vec<&StoredCredential> {
        self.credentials.values().collect()
    }

    /// Update credential properties
    pub fn update_credential(
        &mut self,
        id: &str,
        name: Option<String>,
        host: Option<String>,
    ) -> bool {
        if let Some(credential) = self.credentials.get_mut(id) {
            if let Some(new_name) = name {
                credential.name = new_name;
            }
            if let Some(new_host) = host {
                credential.host = new_host;
            }
            true
        } else {
            false
        }
    }

    /// Mark a credential as used (alias for update_last_used)
    pub fn mark_used(&mut self, id: &str) {
        if let Some(credential) = self.credentials.get_mut(id) {
            credential.last_used = Some(Utc::now());
        }
    }
}

impl StoredCredential {
    /// Create a new stored credential
    pub fn new(id: String, name: String, host: String, port: u16) -> Self {
        Self {
            id,
            name,
            host,
            port,
            created_at: Utc::now(),
            last_used: None,
        }
    }

    /// Update the credential's last used timestamp
    pub fn mark_used(&mut self) {
        self.last_used = Some(Utc::now());
    }
}
