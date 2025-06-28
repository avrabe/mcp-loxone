//! Backend trait for pluggable MCP implementations

use async_trait::async_trait;
use pulseengine_mcp_protocol::*;
use std::error::Error as StdError;
use thiserror::Error;

/// Error type for backend operations
#[derive(Debug, Error)]
pub enum BackendError {
    #[error("Backend not initialized")]
    NotInitialized,

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Operation not supported: {0}")]
    NotSupported(String),

    #[error("Internal backend error: {0}")]
    Internal(String),

    #[error("Custom error: {0}")]
    Custom(Box<dyn StdError + Send + Sync>),
}

impl BackendError {
    pub fn configuration(msg: impl Into<String>) -> Self {
        Self::Configuration(msg.into())
    }

    pub fn connection(msg: impl Into<String>) -> Self {
        Self::Connection(msg.into())
    }

    pub fn not_supported(msg: impl Into<String>) -> Self {
        Self::NotSupported(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    pub fn custom(error: impl StdError + Send + Sync + 'static) -> Self {
        Self::Custom(Box::new(error))
    }
}

/// Convert BackendError to MCP protocol Error
impl From<BackendError> for Error {
    fn from(err: BackendError) -> Self {
        match err {
            BackendError::NotInitialized => Error::internal_error("Backend not initialized"),
            BackendError::Configuration(msg) => Error::invalid_params(msg),
            BackendError::Connection(msg) => {
                Error::internal_error(format!("Connection failed: {}", msg))
            }
            BackendError::NotSupported(msg) => Error::method_not_found(msg),
            BackendError::Internal(msg) => Error::internal_error(msg),
            BackendError::Custom(err) => Error::internal_error(err.to_string()),
        }
    }
}

/// Main trait for MCP backend implementations
///
/// This trait defines the interface that backends must implement to provide
/// domain-specific functionality (tools, resources, prompts) while the framework
/// handles the MCP protocol, authentication, transport, and middleware.
#[async_trait]
pub trait McpBackend: Send + Sync + Clone {
    /// Backend-specific error type
    type Error: StdError + Send + Sync + Into<Error> + From<BackendError> + 'static;

    /// Backend configuration type
    type Config: Clone + Send + Sync;

    /// Initialize the backend with configuration
    ///
    /// This is called once during server startup and should establish any
    /// necessary connections, load configuration, and prepare the backend
    /// for handling requests.
    async fn initialize(config: Self::Config) -> std::result::Result<Self, Self::Error>;

    /// Get server information and capabilities
    ///
    /// This defines what the backend supports (tools, resources, prompts)
    /// and provides metadata about the implementation.
    fn get_server_info(&self) -> ServerInfo;

    /// Health check for the backend
    ///
    /// Should verify that all backend services are operational.
    /// Called regularly for monitoring and health endpoints.
    async fn health_check(&self) -> std::result::Result<(), Self::Error>;

    // Tool Management

    /// List available tools with pagination
    async fn list_tools(
        &self,
        request: PaginatedRequestParam,
    ) -> std::result::Result<ListToolsResult, Self::Error>;

    /// Execute a tool with the given parameters
    async fn call_tool(
        &self,
        request: CallToolRequestParam,
    ) -> std::result::Result<CallToolResult, Self::Error>;

    // Resource Management

    /// List available resources with pagination
    async fn list_resources(
        &self,
        request: PaginatedRequestParam,
    ) -> std::result::Result<ListResourcesResult, Self::Error>;

    /// Read a resource by URI
    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
    ) -> std::result::Result<ReadResourceResult, Self::Error>;

    /// List resource templates (optional)
    async fn list_resource_templates(
        &self,
        request: PaginatedRequestParam,
    ) -> std::result::Result<ListResourceTemplatesResult, Self::Error> {
        let _ = request;
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: String::new(),
        })
    }

    // Prompt Management

    /// List available prompts with pagination
    async fn list_prompts(
        &self,
        request: PaginatedRequestParam,
    ) -> std::result::Result<ListPromptsResult, Self::Error>;

    /// Get a specific prompt
    async fn get_prompt(
        &self,
        request: GetPromptRequestParam,
    ) -> std::result::Result<GetPromptResult, Self::Error>;

    // Subscription Management (optional)

    /// Subscribe to resource updates
    async fn subscribe(
        &self,
        request: SubscribeRequestParam,
    ) -> std::result::Result<(), Self::Error> {
        let _ = request;
        Err(BackendError::not_supported("Subscriptions not supported").into())
    }

    /// Unsubscribe from resource updates
    async fn unsubscribe(
        &self,
        request: UnsubscribeRequestParam,
    ) -> std::result::Result<(), Self::Error> {
        let _ = request;
        Err(BackendError::not_supported("Subscriptions not supported").into())
    }

    // Auto-completion (optional)

    /// Complete tool or resource names
    async fn complete(
        &self,
        request: CompleteRequestParam,
    ) -> std::result::Result<CompleteResult, Self::Error> {
        let _ = request;
        Ok(CompleteResult { completion: vec![] })
    }

    // Logging control (optional)

    /// Set logging level
    async fn set_level(
        &self,
        request: SetLevelRequestParam,
    ) -> std::result::Result<(), Self::Error> {
        let _ = request;
        Err(BackendError::not_supported("Logging level control not supported").into())
    }

    // Lifecycle hooks

    /// Called when the server is starting up
    async fn on_startup(&self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    /// Called when the server is shutting down
    async fn on_shutdown(&self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    /// Called when a client connects
    async fn on_client_connect(
        &self,
        client_info: &Implementation,
    ) -> std::result::Result<(), Self::Error> {
        let _ = client_info;
        Ok(())
    }

    /// Called when a client disconnects
    async fn on_client_disconnect(
        &self,
        client_info: &Implementation,
    ) -> std::result::Result<(), Self::Error> {
        let _ = client_info;
        Ok(())
    }

    // Custom method handlers (for domain-specific extensions)

    /// Handle custom methods not part of the standard MCP protocol
    async fn handle_custom_method(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, Self::Error> {
        let _ = (method, params);
        Err(BackendError::not_supported(format!("Custom method not supported: {}", method)).into())
    }
}

/// Convenience trait for backends that don't need all capabilities
#[async_trait]
pub trait SimpleBackend: Send + Sync + Clone {
    type Error: StdError + Send + Sync + Into<Error> + From<BackendError> + 'static;
    type Config: Clone + Send + Sync;

    async fn initialize(config: Self::Config) -> std::result::Result<Self, Self::Error>;
    fn get_server_info(&self) -> ServerInfo;
    async fn health_check(&self) -> std::result::Result<(), Self::Error>;

    // Only require tools - other methods have default implementations
    async fn list_tools(
        &self,
        request: PaginatedRequestParam,
    ) -> std::result::Result<ListToolsResult, Self::Error>;
    async fn call_tool(
        &self,
        request: CallToolRequestParam,
    ) -> std::result::Result<CallToolResult, Self::Error>;
}

/// Blanket implementation to convert SimpleBackend to McpBackend
#[async_trait]
impl<T> McpBackend for T
where
    T: SimpleBackend,
{
    type Error = T::Error;
    type Config = T::Config;

    async fn initialize(config: Self::Config) -> std::result::Result<Self, Self::Error> {
        T::initialize(config).await
    }

    fn get_server_info(&self) -> ServerInfo {
        T::get_server_info(self)
    }

    async fn health_check(&self) -> std::result::Result<(), Self::Error> {
        T::health_check(self).await
    }

    async fn list_tools(
        &self,
        request: PaginatedRequestParam,
    ) -> std::result::Result<ListToolsResult, Self::Error> {
        T::list_tools(self, request).await
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
    ) -> std::result::Result<CallToolResult, Self::Error> {
        T::call_tool(self, request).await
    }

    // Default implementations for optional capabilities
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
        Err(BackendError::not_supported(format!("Resource not found: {}", request.uri)).into())
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
        Err(BackendError::not_supported(format!("Prompt not found: {}", request.name)).into())
    }
}
