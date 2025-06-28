//! Generic MCP server infrastructure with pluggable backends
//!
//! This crate provides a complete MCP server implementation that can be extended
//! with custom backends for different domains (home automation, databases, APIs, etc.).
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use mcp_server::{McpServer, McpBackend, ServerConfig};
//! use pulseengine_mcp_protocol::*;
//! use async_trait::async_trait;
//!
//! #[derive(Clone)]
//! struct MyBackend;
//!
//! #[async_trait]
//! impl McpBackend for MyBackend {
//!     type Error = Box<dyn std::error::Error + Send + Sync>;
//!     type Config = ();
//!
//!     async fn initialize(_: ()) -> Result<Self, Self::Error> {
//!         Ok(MyBackend)
//!     }
//!
//!     fn get_server_info(&self) -> ServerInfo {
//!         ServerInfo {
//!             protocol_version: ProtocolVersion::default(),
//!             capabilities: ServerCapabilities::default(),
//!             server_info: Implementation {
//!                 name: "My Server".to_string(),
//!                 version: "1.0.0".to_string(),
//!             },
//!             instructions: Some("Example server".to_string()),
//!         }
//!     }
//!
//!     async fn list_tools(&self, _: PaginatedRequestParam) -> Result<ListToolsResult, Self::Error> {
//!         Ok(ListToolsResult { tools: vec![], next_cursor: String::new() })
//!     }
//!
//!     async fn call_tool(&self, _: CallToolRequestParam) -> Result<CallToolResult, Self::Error> {
//!         Ok(CallToolResult { content: vec![], is_error: Some(false) })
//!     }
//!
//!     // Implement other required methods (simplified for example)
//! #   async fn list_resources(&self, _: PaginatedRequestParam) -> Result<ListResourcesResult, Self::Error> {
//! #       Ok(ListResourcesResult { resources: vec![], next_cursor: String::new() })
//! #   }
//! #   async fn read_resource(&self, _: ReadResourceRequestParam) -> Result<ReadResourceResult, Self::Error> {
//! #       Err("No resources".into())
//! #   }
//! #   async fn list_prompts(&self, _: PaginatedRequestParam) -> Result<ListPromptsResult, Self::Error> {
//! #       Ok(ListPromptsResult { prompts: vec![], next_cursor: String::new() })
//! #   }
//! #   async fn get_prompt(&self, _: GetPromptRequestParam) -> Result<GetPromptResult, Self::Error> {
//! #       Err("No prompts".into())
//! #   }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let backend = MyBackend::initialize(()).await?;
//!     let config = ServerConfig::default();
//!     let mut server = McpServer::new(backend, config).await?;
//!     server.run().await?;
//!     Ok(())
//! }
//! ```

pub mod backend;
pub mod context;
pub mod handler;
pub mod middleware;
pub mod server;

// Re-export core types
pub use backend::{BackendError, McpBackend};
pub use context::RequestContext;
pub use handler::{GenericServerHandler, HandlerError};
pub use middleware::{Middleware, MiddlewareStack};
pub use server::{McpServer, ServerConfig, ServerError};

// Re-export from dependencies for convenience
pub use pulseengine_mcp_auth::{self as auth, AuthConfig, AuthenticationManager};
pub use pulseengine_mcp_monitoring::{self as monitoring, MetricsCollector, MonitoringConfig};
pub use pulseengine_mcp_protocol::{self as protocol, *};
pub use pulseengine_mcp_security::{self as security, SecurityConfig, SecurityMiddleware};
pub use pulseengine_mcp_transport::{self as transport, Transport, TransportConfig};
