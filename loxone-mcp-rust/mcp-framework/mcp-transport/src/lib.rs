//! Transport layer implementations for MCP servers
//!
//! This crate provides multiple transport options for MCP servers:
//! stdio (Claude Desktop), HTTP (web clients), and WebSocket (real-time).
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use mcp_transport::{TransportConfig, create_transport};
//! use pulseengine_mcp_protocol::{Request, Response};
//!
//! // Create HTTP transport
//! let config = TransportConfig::Http { port: 3001 };
//! let mut transport = create_transport(config).unwrap();
//!
//! // Define request handler
//! let handler = Box::new(|request: Request| {
//!     Box::pin(async move {
//!         Response::success(serde_json::json!({"result": "handled"}))
//!     })
//! });
//!
//! // Start transport (in real code)
//! // transport.start(handler).await.unwrap();
//! ```

pub mod batch;
pub mod config;
pub mod http;
pub mod stdio;
pub mod streamable_http;
pub mod validation;
pub mod websocket;

#[cfg(test)]
mod http_test;

use async_trait::async_trait;
use pulseengine_mcp_protocol::{Request, Response};
// std::error::Error not needed with thiserror
use thiserror::Error as ThisError;

pub use config::TransportConfig;

#[derive(Debug, ThisError)]
pub enum TransportError {
    #[error("Transport configuration error: {0}")]
    Config(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Protocol error: {0}")]
    Protocol(String),
}

/// Request handler function type
pub type RequestHandler = Box<
    dyn Fn(Request) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
        + Send
        + Sync,
>;

/// Transport layer trait
#[async_trait]
pub trait Transport: Send + Sync {
    async fn start(&mut self, handler: RequestHandler) -> std::result::Result<(), TransportError>;
    async fn stop(&mut self) -> std::result::Result<(), TransportError>;
    async fn health_check(&self) -> std::result::Result<(), TransportError>;
}

/// Create a transport from configuration
pub fn create_transport(
    config: TransportConfig,
) -> std::result::Result<Box<dyn Transport>, TransportError> {
    match config {
        TransportConfig::Stdio => Ok(Box::new(stdio::StdioTransport::new())),
        TransportConfig::Http { port, .. } => Ok(Box::new(http::HttpTransport::new(port))),
        TransportConfig::WebSocket { port, .. } => {
            Ok(Box::new(websocket::WebSocketTransport::new(port)))
        }
    }
}
