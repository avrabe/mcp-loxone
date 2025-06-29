//! Minimal backend example

use pulseengine_mcp_cli::McpBackend;

#[derive(Clone, McpBackend)]
#[mcp_backend(simple)]
pub struct SimpleBackend {
    name: String,
}