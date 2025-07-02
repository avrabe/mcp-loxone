//! Integration module for using the new MCP framework with Loxone server
//!
//! This module provides the bridge between the Loxone-specific implementation
//! and the generic MCP framework.

pub mod adapters;
pub mod backend;

pub use backend::LoxoneBackend;
