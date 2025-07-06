//! Server module for MCP components
//!
//! This module contains active server components that work with the framework.

pub mod framework_backend;
// Legacy modules temporarily disabled during framework migration
// pub mod handlers;
pub mod health_check;
pub mod loxone_batch_executor;
pub mod models;
pub mod rate_limiter;
pub mod request_coalescing;
pub mod request_context;
pub mod resource_monitor;
pub mod response_cache;
// pub mod response_optimization; // Disabled for legacy cleanup
// pub mod rmcp_impl;
pub mod schema_validation;

// Legacy MCP Resources enabled for weather storage integration
pub mod resources;

/// Real-time resource subscription system for MCP
pub mod subscription;

pub use framework_backend::*;
pub use models::*;
pub use request_context::*;

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

/// Legacy MCP transport using stdio
/// Disabled during framework migration - use framework_integration::backend::LoxoneBackend instead
#[deprecated(note = "Use framework_integration::backend::LoxoneBackend instead")]
pub struct LegacyMcpTransport;

/// Legacy HTTP server
/// Disabled during framework migration - use framework_integration::backend::LoxoneBackend instead
#[deprecated(note = "Use framework_integration::backend::LoxoneBackend instead")]
pub struct LegacyHttpServer;

// Framework migration complete - all server functionality moved to:
// - framework_integration::backend::LoxoneBackend (main MCP backend)
// - main.rs (entry points using framework)
//
// This module now only contains supporting components:
// - Component modules (health_check, rate_limiter, etc.)
// - Utility structs (DummyMetricsCollector)
// - Resource subscription system
