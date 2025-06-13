//! Loxone Generation 1 MCP Server implementation in Rust with WASM support
//!
//! This crate provides a Model Context Protocol (MCP) server for controlling
//! Loxone Generation 1 home automation systems. It supports compilation to
//! WASM32-WASIP2 for portable deployment.
//!
//! # Features
//!
//! - 30+ MCP tools for device control and monitoring
//! - Real-time sensor discovery and monitoring
//! - Room-based device organization
//! - Climate control with 6 room controllers
//! - WebSocket and HTTP client support
//! - Secure credential management
//! - WASM-compatible for browser and server deployment
//!
//! # Example
//!
//! ```rust,no_run
//! use loxone_mcp_rust::{LoxoneMcpServer, ServerConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ServerConfig::from_env()?;
//!     let server = LoxoneMcpServer::new(config).await?;
//!     server.run().await?;
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod config;
pub mod error;
pub mod http_transport;
pub mod logging;
pub mod mcp_consent;
pub mod server;
pub mod simple_server;
pub mod tools;
pub mod validation;

#[cfg(feature = "crypto")]
pub mod crypto;

#[cfg(feature = "websocket")]
pub mod discovery;

#[cfg(feature = "discovery")]
pub mod mdns_discovery;
#[cfg(feature = "discovery")]
pub mod network_discovery;

// Re-export main types
pub use crate::{
    config::{CredentialStore, ServerConfig},
    error::{LoxoneError, Result},
    server::LoxoneMcpServer,
    simple_server::SimpleLoxoneMcpServer,
};

// Re-export MCP types from rmcp
// pub use rmcp::{}; // TODO: Fix when rmcp API is clarified

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
mod wasm_component;

#[cfg(target_arch = "wasm32")]
pub use wasm::*;

#[cfg(target_arch = "wasm32")]
pub use wasm_component::*;
