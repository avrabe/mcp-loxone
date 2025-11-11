//! Hello World MCP Server Example
//!
//! This demonstrates the minimal code needed to create an MCP server
//! using the mcp-framework. It implements a simple "say_hello" tool.

use pulseengine_mcp_protocol::*;
use pulseengine_mcp_protocol::{ElicitationCapability, SamplingCapability};
use pulseengine_mcp_server::{McpBackend, McpServer, ServerConfig};
use pulseengine_mcp_transport::TransportConfig;

use async_trait::async_trait;
use serde_json::json;
use thiserror::Error;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Simple backend error type
#[derive(Debug, Error)]
pub enum HelloWorldError {
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<HelloWorldError> for Error {
    fn from(err: HelloWorldError) -> Self {
        match err {
            HelloWorldError::InvalidParameter(msg) => Error::invalid_params(msg),
            HelloWorldError::Internal(msg) => Error::internal_error(msg),
        }
    }
}

impl From<pulseengine_mcp_server::BackendError> for HelloWorldError {
    fn from(err: pulseengine_mcp_server::BackendError) -> Self {
        HelloWorldError::Internal(err.to_string())
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
                // Enable sampling capability - allows server-initiated LLM calls
                sampling: Some(SamplingCapability {}),
                // Enable elicitation capability - allows server-initiated user input requests
                elicitation: Some(ElicitationCapability {}),
            },
            server_info: Implementation {
                name: "Hello World MCP Server".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: None,
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
                output_schema: None,
                // v0.13.0 new fields
                title: None,
                annotations: None,
                icons: None,
            },
            Tool {
                name: "count_greetings".to_string(),
                description: "Get the total number of greetings sent".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
                output_schema: None,
                // v0.13.0 new fields
                title: None,
                annotations: None,
                icons: None,
            },
        ];

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
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

                Ok(CallToolResult {
                    content: vec![Content::text(message)],
                    is_error: None,
                    structured_content: None,
                    _meta: None, // v0.13.0 new field
                })
            }

            "count_greetings" => {
                let count = self
                    .greeting_count
                    .load(std::sync::atomic::Ordering::Relaxed);

                Ok(CallToolResult {
                    content: vec![Content::text(format!("Total greetings sent: {count}"))],
                    is_error: None,
                    structured_content: None,
                    _meta: None, // v0.13.0 new field
                })
            }

            _ => Err(HelloWorldError::InvalidParameter(format!(
                "Unknown tool: {}",
                request.name
            ))),
        }
    }

    // Use default implementations for resources and prompts
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
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("hello_world_mcp=debug,pulseengine_mcp_server=debug")
        }))
        .init();

    info!("🚀 Starting Hello World MCP Server");

    // Create backend
    let backend_config = HelloWorldConfig::default();
    let backend = HelloWorldBackend::initialize(backend_config).await?;

    // Create server configuration
    let mut auth_config = pulseengine_mcp_auth::default_config();
    auth_config.enabled = false; // Disable authentication for this example

    let server_config = ServerConfig {
        server_info: backend.get_server_info(),
        auth_config,
        transport_config: TransportConfig::Stdio, // Use stdio transport for MCP clients
        ..Default::default()
    };

    // Create and start server
    let mut server = McpServer::new(backend, server_config).await?;

    info!("✅ Hello World MCP Server started successfully");
    info!("💡 Available tools: say_hello, count_greetings");
    info!("🔗 Connect using any MCP client via stdio");

    // Run server until shutdown
    server.run().await?;

    info!("👋 Hello World MCP Server stopped");
    Ok(())
}
