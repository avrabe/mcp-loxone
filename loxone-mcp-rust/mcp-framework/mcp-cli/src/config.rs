//! Configuration management and utilities

use crate::CliError;
use pulseengine_mcp_protocol::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use serde::{Deserialize, Serialize};
use std::env;

/// Default logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultLoggingConfig {
    pub level: String,
    pub format: LogFormat,
    pub output: LogOutput,
    pub structured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "pretty")]
    Pretty,
    #[serde(rename = "compact")]
    Compact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogOutput {
    #[serde(rename = "stdout")]
    Stdout,
    #[serde(rename = "stderr")]
    Stderr,
    #[serde(rename = "file")]
    File(String),
}

impl Default for DefaultLoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: LogFormat::Pretty,
            output: LogOutput::Stdout,
            structured: true,
        }
    }
}

impl DefaultLoggingConfig {
    pub fn initialize(&self) -> Result<(), CliError> {
        // Initialize tracing subscriber based on configuration
        use tracing_subscriber::{fmt, prelude::*, EnvFilter};

        let level = env::var("RUST_LOG").unwrap_or_else(|_| self.level.clone());
        let filter = EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new(&level))
            .map_err(|e| CliError::logging(format!("Invalid log level: {e}")))?;

        match self.format {
            LogFormat::Json => {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt::layer().json())
                    .init();
            }
            LogFormat::Pretty => {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt::layer().pretty())
                    .init();
            }
            LogFormat::Compact => {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt::layer().compact())
                    .init();
            }
        }

        Ok(())
    }
}

/// Utility to create default server info from Cargo.toml
pub fn create_server_info(name: Option<String>, version: Option<String>) -> ServerInfo {
    ServerInfo {
        protocol_version: ProtocolVersion::default(),
        capabilities: ServerCapabilities::default(),
        server_info: Implementation {
            name: name.unwrap_or_else(|| env!("CARGO_PKG_NAME").to_string()),
            version: version.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
        },
        instructions: None,
    }
}

/// Environment variable utilities
pub mod env_utils {
    use std::env;
    use std::str::FromStr;

    /// Get environment variable with default value
    pub fn get_env_or_default<T>(key: &str, default: T) -> T
    where
        T: FromStr + Clone,
    {
        env::var(key)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }

    /// Get required environment variable
    pub fn get_required_env<T>(key: &str) -> Result<T, crate::CliError>
    where
        T: FromStr,
        T::Err: std::fmt::Display,
    {
        env::var(key)
            .map_err(|_| {
                crate::CliError::configuration(format!(
                    "Missing required environment variable: {key}"
                ))
            })?
            .parse()
            .map_err(|e| crate::CliError::configuration(format!("Invalid value for {key}: {e}")))
    }
}
