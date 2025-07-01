//! Transport configuration

use serde::{Deserialize, Serialize};

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportConfig {
    /// Standard I/O transport (for MCP clients)
    Stdio,

    /// HTTP transport with Server-Sent Events
    Http { port: u16, host: Option<String> },

    /// Streamable HTTP transport (MCP Inspector compatible)
    StreamableHttp { port: u16, host: Option<String> },

    /// WebSocket transport
    WebSocket { port: u16, host: Option<String> },
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self::Stdio
    }
}

impl TransportConfig {
    /// Create stdio transport configuration
    pub fn stdio() -> Self {
        Self::Stdio
    }

    /// Create HTTP transport configuration
    pub fn http(port: u16) -> Self {
        Self::Http { port, host: None }
    }

    /// Create Streamable HTTP transport configuration (MCP Inspector compatible)
    pub fn streamable_http(port: u16) -> Self {
        Self::StreamableHttp { port, host: None }
    }

    /// Create WebSocket transport configuration
    pub fn websocket(port: u16) -> Self {
        Self::WebSocket { port, host: None }
    }
}
