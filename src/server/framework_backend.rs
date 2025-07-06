//! Framework-compatible backend implementation
//!
//! This module provides a backend implementation that works with the
//! PulseEngine MCP Framework while integrating with Loxone systems.

use crate::config::ServerConfig;
use crate::error::{LoxoneError, Result};
use std::sync::Arc;
use tracing::info;

/// Simple backend implementation for framework compatibility
#[derive(Debug, Clone)]
pub struct LoxoneFrameworkBackend {
    /// Loxone server configuration
    pub config: ServerConfig,
    /// Initialization timestamp
    pub initialized_at: std::time::Instant,
}

impl LoxoneFrameworkBackend {
    /// Initialize the backend with Loxone configuration
    pub async fn initialize(config: ServerConfig) -> Result<Self> {
        info!("Initializing Loxone framework backend");
        
        // Validate configuration
        if config.loxone.url.host().is_none() {
            return Err(LoxoneError::config("Invalid Loxone URL - missing host"));
        }
        
        if config.loxone.username.is_empty() {
            return Err(LoxoneError::config("Loxone username is required"));
        }
        
        let backend = Self {
            config,
            initialized_at: std::time::Instant::now(),
        };
        
        info!("✅ Loxone framework backend initialized successfully");
        Ok(backend)
    }
    
    /// Get the Loxone configuration
    pub fn loxone_config(&self) -> &ServerConfig {
        &self.config
    }
    
    /// Check if backend is healthy
    pub fn is_healthy(&self) -> bool {
        // Basic health check - backend is healthy if initialized
        true
    }
    
    /// Get uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.initialized_at.elapsed().as_secs()
    }
}

/// Create a backend instance for use throughout the application
pub async fn create_loxone_backend(config: ServerConfig) -> Result<Arc<LoxoneFrameworkBackend>> {
    let backend = LoxoneFrameworkBackend::initialize(config).await?;
    Ok(Arc::new(backend))
}