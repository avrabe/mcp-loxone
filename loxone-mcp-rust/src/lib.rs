//! Loxone MCP Server implementation in Rust with WASM support
//!
//! This crate provides a Model Context Protocol (MCP) server for controlling
//! Loxone home automation systems. It supports compilation to
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
//! - WASM-compatible for server deployment via WASIP2
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

// pub mod audit_log; // Removed: unused module
pub mod client;
pub mod config;
pub mod error;
// pub mod history; // Removed: unused module
pub mod http_transport;
pub mod logging;
pub mod mcp_consent;
pub mod monitoring;
pub mod performance;
pub mod sampling;
pub mod security;
pub mod server;
pub mod services;
pub mod shared_styles;
pub mod tools;
pub mod validation;

pub mod mock;

#[cfg(feature = "crypto-openssl")]
pub mod crypto;

pub mod discovery;

// Re-export main types
pub use crate::{
    config::{CredentialStore, ServerConfig},
    error::{LoxoneError, Result},
    server::LoxoneMcpServer,
};

// Re-export MCP types from rmcp
// pub use rmcp::{}; // TODO: Fix when rmcp API is clarified


// Test module for subscription integration
#[cfg(test)]
mod test_subscription_integration;
