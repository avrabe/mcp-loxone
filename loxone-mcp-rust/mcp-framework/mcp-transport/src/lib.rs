//! Transport layer implementations for MCP servers

pub mod config;
pub mod stdio;
pub mod http;
pub mod websocket;
pub mod validation;
pub mod batch;
pub mod streamable_http;

#[cfg(test)]
mod http_test;

use async_trait::async_trait;
use mcp_protocol::{Request, Response};
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
pub type RequestHandler = Box<dyn Fn(Request) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> + Send + Sync>;

/// Transport layer trait
#[async_trait]
pub trait Transport: Send + Sync {
    async fn start(&mut self, handler: RequestHandler) -> std::result::Result<(), TransportError>;
    async fn stop(&mut self) -> std::result::Result<(), TransportError>;
    async fn health_check(&self) -> std::result::Result<(), TransportError>;
}

/// Create a transport from configuration
pub fn create_transport(config: TransportConfig) -> std::result::Result<Box<dyn Transport>, TransportError> {
    match config {
        TransportConfig::Stdio => Ok(Box::new(stdio::StdioTransport::new())),
        TransportConfig::Http { port, .. } => Ok(Box::new(http::HttpTransport::new(port))),
        TransportConfig::WebSocket { port, .. } => Ok(Box::new(websocket::WebSocketTransport::new(port))),
    }
}