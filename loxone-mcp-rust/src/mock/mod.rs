//! Mock implementations for testing
//!
//! This module provides mock clients and components for testing purposes.

use crate::client::{LoxoneClient, LoxoneResponse, LoxoneStructure};
use crate::error::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

/// Mock Loxone client for testing
pub struct MockLoxoneClient {
    connected: bool,
    structure: Option<LoxoneStructure>,
}

impl MockLoxoneClient {
    /// Create new mock client
    pub fn new() -> Self {
        Self {
            connected: false,
            structure: None,
        }
    }

    /// Set mock structure data
    pub fn with_structure(mut self, structure: LoxoneStructure) -> Self {
        self.structure = Some(structure);
        self
    }
}

#[async_trait]
impl LoxoneClient for MockLoxoneClient {
    async fn connect(&mut self) -> Result<()> {
        self.connected = true;
        Ok(())
    }

    async fn is_connected(&self) -> Result<bool> {
        Ok(self.connected)
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    async fn send_command(&self, _uuid: &str, _command: &str) -> Result<LoxoneResponse> {
        Ok(LoxoneResponse {
            code: 200,
            value: Value::String("OK".to_string()),
        })
    }

    async fn get_structure(&self) -> Result<LoxoneStructure> {
        self.structure
            .clone()
            .ok_or_else(|| crate::error::LoxoneError::connection("No structure available in mock"))
    }

    async fn get_device_states(&self, _uuids: &[String]) -> Result<HashMap<String, Value>> {
        Ok(HashMap::new())
    }

    async fn get_system_info(&self) -> Result<Value> {
        Ok(serde_json::json!({
            "version": "mock",
            "name": "Mock Miniserver"
        }))
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.connected)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Default for MockLoxoneClient {
    fn default() -> Self {
        Self::new()
    }
}
