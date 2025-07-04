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

// Core modules
pub mod auth;
pub mod client;
pub mod config;
pub mod crypto;
pub mod discovery;
pub mod error;
pub mod error_recovery;
pub mod framework_integration;
pub mod health;
// pub mod http_transport; // Disabled during framework migration - use framework's HTTP transport instead
pub mod logging;
pub mod mcp_consent;
pub mod monitoring;
pub mod performance;
pub mod sampling;
pub mod security;
pub mod server;
pub mod services;
pub mod shared_styles;
pub mod storage;
pub mod tools;
pub mod validation;

// Test support modules - available for both unit tests and integration tests
#[cfg(any(test, feature = "test-utils"))]
pub mod mock;

// Re-export main types for convenience
pub use config::ServerConfig;
pub use error::{LoxoneError, Result};
pub use framework_integration::LoxoneBackend;
