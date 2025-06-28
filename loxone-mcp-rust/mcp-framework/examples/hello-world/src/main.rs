//! Hello World MCP Server Example
//!
//! This demonstrates a minimal MCP server using the framework.
//! It shows the basic structure without overwhelming complexity.

use pulseengine_mcp_protocol::*;
use pulseengine_mcp_server::{BackendError, McpBackend, McpServer, ServerConfig};
use pulseengine_mcp_transport::TransportConfig;

use async_trait::async_trait;
use serde_json::json;
use thiserror::Error;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

/// Simple backend error type
#[derive(Debug, Error)]
pub enum HelloWorldError {
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Backend error: {0}")]
    Backend(#[from] BackendError),
}

/// Convert backend errors to MCP protocol errors
impl From<HelloWorldError> for pulseengine_mcp_protocol::Error {
    fn from(err: HelloWorldError) -> Self {
        match err {
            HelloWorldError::InvalidParameter(msg) => Error::invalid_params(msg),
            HelloWorldError::Internal(msg) => Error::internal_error(msg),
            HelloWorldError::Backend(backend_err) => backend_err.into(),
        }
    }
}

/// Hello World backend implementation
#[derive(Clone)]
pub struct HelloWorldBackend {
    greeting_count: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

/// Configuration for the Hello World backend
#[derive(Debug, Clone)]
pub struct HelloWorldConfig {
    pub default_greeting: String,
}

impl Default for HelloWorldConfig {
    fn default() -> Self {
        Self {
            default_greeting: "Hello".to_string(),
        }
    }
}

#[async_trait]
impl McpBackend for HelloWorldBackend {
    type Error = HelloWorldError;
    type Config = HelloWorldConfig;

    async fn initialize(_config: Self::Config) -> std::result::Result<Self, Self::Error> {
        info!("Initializing Hello World backend");
        Ok(Self {
            greeting_count: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
    }

    fn get_server_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                resources: None,
                prompts: None,
                logging: None,
                sampling: None,
            },
            server_info: Implementation {
                name: "Hello World MCP Server".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some(
                "A simple demonstration server with basic greeting functionality".to_string(),
            ),
        }
    }

    async fn health_check(&self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn list_tools(
        &self,
        _request: PaginatedRequestParam,
    ) -> std::result::Result<ListToolsResult, Self::Error> {
        let tools = vec![
            Tool {
                name: "say_hello".to_string(),
                description: "Say hello to someone or something".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The name to greet"
                        },
                        "greeting": {
                            "type": "string",
                            "description": "Custom greeting (optional)",
                            "default": "Hello"
                        }
                    },
                    "required": ["name"]
                }),
            },
            Tool {
                name: "count_greetings".to_string(),
                description: "Get the total number of greetings sent".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        ];

        Ok(ListToolsResult {
            tools,
            next_cursor: String::new(),
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
    ) -> std::result::Result<CallToolResult, Self::Error> {
        match request.name.as_str() {
            "say_hello" => {
                let args = request
                    .arguments
                    .unwrap_or(serde_json::Value::Object(Default::default()));

                let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                    HelloWorldError::InvalidParameter("name is required".to_string())
                })?;

                let greeting = args
                    .get("greeting")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Hello");

                // Increment greeting counter
                self.greeting_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                let message = format!("{greeting}, {name}! 👋");

                info!(
                    tool = "say_hello",
                    name = name,
                    greeting = greeting,
                    "Generated greeting"
                );

                Ok(CallToolResult {
                    content: vec![Content::text(message)],
                    is_error: Some(false),
                })
            }

            "count_greetings" => {
                let count = self
                    .greeting_count
                    .load(std::sync::atomic::Ordering::Relaxed);

                info!(
                    tool = "count_greetings",
                    count = count,
                    "Retrieved greeting count"
                );

                Ok(CallToolResult {
                    content: vec![Content::text(format!("Total greetings sent: {count}"))],
                    is_error: Some(false),
                })
            }

            _ => {
                warn!(tool = request.name, "Unknown tool requested");
                Err(HelloWorldError::InvalidParameter(format!(
                    "Unknown tool: {}",
                    request.name
                )))
            }
        }
    }

    // Simple implementations for unused features
    async fn list_resources(
        &self,
        _request: PaginatedRequestParam,
    ) -> std::result::Result<ListResourcesResult, Self::Error> {
        Ok(ListResourcesResult {
            resources: vec![],
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
    ) -> std::result::Result<ReadResourceResult, Self::Error> {
        Err(HelloWorldError::InvalidParameter(format!(
            "Resource not found: {}",
            request.uri
        )))
    }

    async fn list_prompts(
        &self,
        _request: PaginatedRequestParam,
    ) -> std::result::Result<ListPromptsResult, Self::Error> {
        Ok(ListPromptsResult {
            prompts: vec![],
            next_cursor: None,
        })
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParam,
    ) -> std::result::Result<GetPromptResult, Self::Error> {
        Err(HelloWorldError::InvalidParameter(format!(
            "Prompt not found: {}",
            request.name
        )))
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("hello_world_mcp=debug,mcp_server=debug")),
        )
        .init();

    info!("🚀 Starting Hello World MCP Server");

    // Create backend
    let backend_config = HelloWorldConfig::default();
    let backend = HelloWorldBackend::initialize(backend_config).await?;

    // Create server configuration
    let server_config = ServerConfig {
        server_info: backend.get_server_info(),
        transport_config: TransportConfig::Stdio, // Use stdio for MCP clients like Claude Desktop
        ..Default::default()
    };

    // Create and start server
    let mut server = McpServer::new(backend, server_config).await?;

    info!("✅ Hello World MCP Server started successfully");
    info!("💡 Available tools: say_hello, count_greetings");
    info!("🔗 Connect using any MCP client via stdio transport");

    // Run server until shutdown
    server.run().await?;

    info!("👋 Hello World MCP Server stopped");
    Ok(())
}
