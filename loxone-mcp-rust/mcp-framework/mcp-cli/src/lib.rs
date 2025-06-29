//! CLI integration and configuration framework for MCP servers
//!
//! This crate provides automatic CLI generation, configuration management, and server setup
//! for MCP servers. It eliminates boilerplate code and provides a modern, ergonomic API.
//!
//! # Features
//!
//! - **Automatic CLI Generation**: Generate command-line interfaces from configuration structs
//! - **Configuration Management**: Type-safe configuration with environment variable support  
//! - **Server Integration**: Seamless integration with the MCP server framework
//! - **Logging Setup**: Built-in structured logging configuration
//! - **Builder Patterns**: Fluent APIs for server configuration
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use pulseengine_mcp_cli::{McpConfig, DefaultLoggingConfig};
//! use pulseengine_mcp_protocol::ServerInfo;
//! use clap::Parser;
//!
//! #[derive(McpConfig, Parser)]
//! struct MyServerConfig {
//!     #[clap(short, long, default_value = "8080")]
//!     port: u16,
//!     
//!     #[clap(short, long)]
//!     database_url: String,
//!     
//!     #[mcp(auto_populate)]
//!     #[clap(skip)]
//!     server_info: Option<ServerInfo>,
//!     
//!     #[mcp(logging)]
//!     #[clap(skip)]
//!     logging: Option<DefaultLoggingConfig>,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = MyServerConfig::parse();
//!     config.initialize_logging()?;
//!     // Use server_builder() for advanced configuration
//!     Ok(())
//! }
//! ```

use thiserror::Error;

/// Re-export commonly used types
pub use pulseengine_mcp_protocol::*;

#[cfg(feature = "cli")]
pub use clap;

/// Error types for CLI operations
#[derive(Debug, Error)]
pub enum CliError {
    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("CLI parsing error: {0}")]
    Parsing(String),

    #[error("Server setup error: {0}")]
    ServerSetup(String),

    #[error("Logging setup error: {0}")]
    Logging(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(#[from] pulseengine_mcp_protocol::Error),
}

impl CliError {
    pub fn configuration(msg: impl Into<String>) -> Self {
        Self::Configuration(msg.into())
    }

    pub fn parsing(msg: impl Into<String>) -> Self {
        Self::Parsing(msg.into())
    }

    pub fn server_setup(msg: impl Into<String>) -> Self {
        Self::ServerSetup(msg.into())
    }

    pub fn logging(msg: impl Into<String>) -> Self {
        Self::Logging(msg.into())
    }
}

/// Configuration trait for MCP servers
pub trait McpConfiguration: Sized {
    /// Initialize logging from configuration
    fn initialize_logging(&self) -> std::result::Result<(), CliError>;

    /// Get server information
    fn get_server_info(&self) -> &ServerInfo;

    /// Get logging configuration
    fn get_logging_config(&self) -> &DefaultLoggingConfig;

    /// Validate the configuration
    fn validate(&self) -> std::result::Result<(), CliError> {
        Ok(())
    }
}

// Re-export proc macros when derive feature is enabled
#[cfg(feature = "derive")]
pub use pulseengine_mcp_cli_derive::{McpBackend, McpConfig};

// Modules
pub mod config;
pub mod server;
pub mod utils;

// Re-export main types
pub use config::*;
pub use server::*;
