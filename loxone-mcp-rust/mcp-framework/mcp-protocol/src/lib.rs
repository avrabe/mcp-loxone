//! Core Model Context Protocol types and validation
//!
//! This crate provides the fundamental types, traits, and validation logic
//! for the Model Context Protocol. It serves as the foundation for building
//! MCP servers and clients with strong type safety and validation.

pub mod error;
pub mod model;
pub mod validation;

// Re-export core types for easy access
pub use error::{Error, Result};
pub use model::*;
pub use validation::Validator;

/// Protocol version constants
pub const MCP_VERSION: &str = "2025-03-26";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[MCP_VERSION];

/// Check if a protocol version is supported
pub fn is_protocol_version_supported(version: &str) -> bool {
    SUPPORTED_PROTOCOL_VERSIONS.contains(&version)
}

/// Validate MCP protocol version compatibility
pub fn validate_protocol_version(client_version: &str) -> Result<()> {
    if is_protocol_version_supported(client_version) {
        Ok(())
    } else {
        Err(Error::protocol_version_mismatch(client_version, MCP_VERSION))
    }
}