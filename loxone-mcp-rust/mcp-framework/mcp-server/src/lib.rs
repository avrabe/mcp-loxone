//! Generic MCP server infrastructure with pluggable backends
//!
//! This crate provides a complete MCP server implementation that can be extended
//! with custom backends for different domains (home automation, databases, APIs, etc.).

pub mod backend;
pub mod server;
pub mod handler;
pub mod context;
pub mod middleware;

// Re-export core types
pub use backend::{McpBackend, BackendError};
pub use server::{McpServer, ServerConfig, ServerError};
pub use handler::{GenericServerHandler, HandlerError};
pub use context::RequestContext;
pub use middleware::{MiddlewareStack, Middleware};

// Re-export from dependencies for convenience
pub use mcp_protocol::{self as protocol, *};
pub use mcp_auth::{self as auth, AuthConfig, AuthenticationManager};
pub use mcp_transport::{self as transport, Transport, TransportConfig};
pub use mcp_security::{self as security, SecurityConfig, SecurityMiddleware};
pub use mcp_monitoring::{self as monitoring, MonitoringConfig, MetricsCollector};