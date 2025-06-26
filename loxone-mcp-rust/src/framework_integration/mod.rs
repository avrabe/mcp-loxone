//! Integration module for using the new MCP framework with Loxone server
//!
//! This module provides the bridge between the Loxone-specific implementation
//! and the generic MCP framework.

pub mod backend;
pub mod adapters;

pub use backend::LoxoneBackend;