//! Framework-compatible backend implementation
//!
//! This module provides a backend implementation that works with the
//! PulseEngine MCP Framework while integrating with Loxone systems.

use crate::config::ServerConfig;
use crate::error::{LoxoneError, Result};
use pulseengine_mcp_protocol::{
    CallToolRequestParam, CallToolResult, Error as McpError, GetPromptRequestParam,
    GetPromptResult, Implementation, ListPromptsResult, ListResourcesResult, ListToolsResult,
    PaginatedRequestParam, ProtocolVersion, ReadResourceRequestParam, ReadResourceResult,
    ServerCapabilities, ServerInfo, ToolsCapability,
};
use pulseengine_mcp_server::McpBackend;
use std::sync::Arc;
use tracing::info;

/// Simple backend implementation for framework compatibility
#[derive(Debug, Clone)]
pub struct LoxoneFrameworkBackend {
    /// Loxone server configuration
    pub config: ServerConfig,
    /// Initialization timestamp
    pub initialized_at: std::time::Instant,
}

impl LoxoneFrameworkBackend {
    /// Initialize the backend with Loxone configuration
    pub async fn initialize(config: ServerConfig) -> Result<Self> {
        info!("Initializing Loxone framework backend");

        // Validate configuration
        if config.loxone.url.host().is_none() {
            return Err(LoxoneError::config("Invalid Loxone URL - missing host"));
        }

        if config.loxone.username.is_empty() {
            return Err(LoxoneError::config("Loxone username is required"));
        }

        let backend = Self {
            config,
            initialized_at: std::time::Instant::now(),
        };

        info!("✅ Loxone framework backend initialized successfully");
        Ok(backend)
    }

    /// Get the Loxone configuration
    pub fn loxone_config(&self) -> &ServerConfig {
        &self.config
    }

    /// Check if backend is healthy
    pub fn is_healthy(&self) -> bool {
        // Basic health check - backend is healthy if initialized
        true
    }

    /// Get uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.initialized_at.elapsed().as_secs()
    }
}

#[async_trait::async_trait]
impl McpBackend for LoxoneFrameworkBackend {
    type Error = McpError;
    type Config = ServerConfig;

    async fn initialize(config: Self::Config) -> std::result::Result<Self, Self::Error> {
        Self::initialize(config)
            .await
            .map_err(|e| McpError::internal_error(e.to_string()))
    }

    fn get_server_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                logging: None,
                prompts: None,
                resources: None,
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                sampling: None,
                elicitation: None,
            },
            server_info: Implementation {
                name: "loxone-mcp-rust".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: None,
        }
    }

    async fn health_check(&self) -> std::result::Result<(), Self::Error> {
        if self.is_healthy() {
            Ok(())
        } else {
            Err(McpError::internal_error("Backend health check failed"))
        }
    }

    async fn list_tools(
        &self,
        _params: PaginatedRequestParam,
    ) -> std::result::Result<ListToolsResult, Self::Error> {
        // Return empty list for now - tools will be handled by the actual MCP implementation
        Ok(ListToolsResult {
            tools: vec![],
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        _request: CallToolRequestParam,
    ) -> std::result::Result<CallToolResult, Self::Error> {
        // This should not be called as tools are handled elsewhere
        Err(McpError::internal_error(
            "Tool calls not supported through backend",
        ))
    }

    async fn list_resources(
        &self,
        _request: PaginatedRequestParam,
    ) -> std::result::Result<ListResourcesResult, Self::Error> {
        // Return empty list for resources
        Ok(ListResourcesResult {
            resources: vec![],
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
    ) -> std::result::Result<ReadResourceResult, Self::Error> {
        Err(McpError::invalid_params(format!(
            "Resource not found: {}",
            request.uri
        )))
    }

    async fn list_prompts(
        &self,
        _request: PaginatedRequestParam,
    ) -> std::result::Result<ListPromptsResult, Self::Error> {
        // Return empty list for prompts
        Ok(ListPromptsResult {
            prompts: vec![],
            next_cursor: None,
        })
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParam,
    ) -> std::result::Result<GetPromptResult, Self::Error> {
        Err(McpError::invalid_params(format!(
            "Prompt not found: {}",
            request.name
        )))
    }
}

/// Create a backend instance for use throughout the application
pub async fn create_loxone_backend(config: ServerConfig) -> Result<Arc<LoxoneFrameworkBackend>> {
    let backend = LoxoneFrameworkBackend::initialize(config).await?;
    Ok(Arc::new(backend))
}
