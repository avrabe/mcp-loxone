//! Storage backend for authentication data

use crate::models::ApiKey;
use async_trait::async_trait;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Storage error: {0}")]
    General(String),
}

/// Storage backend trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn load_keys(&self) -> Result<HashMap<String, ApiKey>, StorageError>;
    async fn save_key(&self, key: &ApiKey) -> Result<(), StorageError>;
    async fn delete_key(&self, key_id: &str) -> Result<(), StorageError>;
}

/// File-based storage backend
pub struct FileStorage {
    #[allow(dead_code)]
    path: std::path::PathBuf,
}

impl FileStorage {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl StorageBackend for FileStorage {
    async fn load_keys(&self) -> Result<HashMap<String, ApiKey>, StorageError> {
        Ok(HashMap::new())
    }
    
    async fn save_key(&self, _key: &ApiKey) -> Result<(), StorageError> {
        Ok(())
    }
    
    async fn delete_key(&self, _key_id: &str) -> Result<(), StorageError> {
        Ok(())
    }
}

/// Environment variable storage backend
pub struct EnvironmentStorage {
    #[allow(dead_code)]
    prefix: String,
}

impl EnvironmentStorage {
    pub fn new(prefix: String) -> Self {
        Self { prefix }
    }
}

#[async_trait]
impl StorageBackend for EnvironmentStorage {
    async fn load_keys(&self) -> Result<HashMap<String, ApiKey>, StorageError> {
        Ok(HashMap::new())
    }
    
    async fn save_key(&self, _key: &ApiKey) -> Result<(), StorageError> {
        Ok(())
    }
    
    async fn delete_key(&self, _key_id: &str) -> Result<(), StorageError> {
        Ok(())
    }
}