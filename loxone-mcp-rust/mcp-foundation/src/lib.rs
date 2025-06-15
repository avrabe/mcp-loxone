//! MCP Foundation - Custom Model Context Protocol implementation
//!
//! This crate provides a custom, lightweight implementation of the Model Context Protocol (MCP)
//! specifically designed for the Loxone integration. It replaces the external rmcp dependency
//! with a tailored solution that provides exactly what we need without external dependencies.

pub mod error;
pub mod model;
pub mod server;
pub mod service;

// Re-export core types for easy access
pub use error::{Error, Result};
pub use model::*;
pub use server::{RequestContext, RoleServer, ServerHandler};
pub use service::ServiceExt;
