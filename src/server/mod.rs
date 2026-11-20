//! Server module for MCP components
//!
//! This module contains the macro-based MCP server and supporting components.

pub mod framework_backend;
pub mod health_check;
#[cfg(feature = "http-server")]
pub mod http_auth;
pub mod loxone_batch_executor;
pub mod macro_backend;
pub mod models;
pub mod rate_limiter;
pub mod request_coalescing;
pub mod resource_monitor;
pub mod response_cache;
#[cfg(feature = "rmcp")]
pub mod rmcp_tools;
pub mod schema_validation;

// Legacy MCP Resources enabled for weather storage integration
pub mod resources;

/// Real-time resource subscription system for MCP
pub mod subscription;

pub use framework_backend::*;
pub use models::*;

/// Dummy metrics collector for HTTP transport compatibility
pub struct DummyMetricsCollector;

impl DummyMetricsCollector {
    pub fn record_prompt(&self) {}
    pub fn connection_opened(&self) {}
    pub fn connection_closed(&self) {}
    pub async fn get_metrics(&self) -> serde_json::Value {
        serde_json::json!({
            "requests": 0,
            "connections": 0
        })
    }
}

impl Default for DummyMetricsCollector {
    fn default() -> Self {
        Self
    }
}
