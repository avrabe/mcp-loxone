//! MCP server traits and types

use crate::{
    error::{Error, Result},
    model::*,
};
use async_trait::async_trait;
use std::collections::HashMap;
use uuid::Uuid;

/// Server role marker type
#[derive(Debug, Clone)]
pub struct RoleServer;

/// Request context for MCP operations
#[derive(Debug, Clone)]
pub struct RequestContext<Role> {
    /// Unique request ID
    pub request_id: Uuid,
    /// Request metadata
    pub metadata: HashMap<String, String>,
    /// Client information
    pub client_info: Option<Implementation>,
    /// Role marker
    pub _role: std::marker::PhantomData<Role>,
}

impl<Role> RequestContext<Role> {
    /// Create a new request context
    pub fn new() -> Self {
        Self {
            request_id: Uuid::new_v4(),
            metadata: HashMap::new(),
            client_info: None,
            _role: std::marker::PhantomData,
        }
    }

    /// Create a request context with specific ID
    pub fn with_id(request_id: Uuid) -> Self {
        Self {
            request_id,
            metadata: HashMap::new(),
            client_info: None,
            _role: std::marker::PhantomData,
        }
    }

    /// Set client information
    pub fn with_client_info(mut self, client_info: Implementation) -> Self {
        self.client_info = Some(client_info);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

impl<Role> Default for RequestContext<Role> {
    fn default() -> Self {
        Self::new()
    }
}

/// Main MCP server handler trait
#[async_trait]
pub trait ServerHandler: Send + Sync + Clone {
    /// Health check - responds to ping requests
    async fn ping(&self, context: RequestContext<RoleServer>) -> Result<()>;

    /// Get server information and capabilities
    fn get_info(&self) -> ServerInfo;

    /// List available tools
    async fn list_tools(
        &self,
        request: PaginatedRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult>;

    /// Execute a tool
    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult>;

    /// List available resources
    async fn list_resources(
        &self,
        request: PaginatedRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult>;

    /// Read a resource
    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult>;

    /// List available prompts
    async fn list_prompts(
        &self,
        request: PaginatedRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult>;

    /// Get a specific prompt
    async fn get_prompt(
        &self,
        request: GetPromptRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult>;

    /// Initialize the server
    async fn initialize(
        &self,
        request: InitializeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult>;

    /// Complete auto-completion request
    async fn complete(
        &self,
        request: CompleteRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<CompleteResult>;

    /// Set logging level
    async fn set_level(
        &self,
        request: SetLevelRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<()>;

    /// List resource templates
    async fn list_resource_templates(
        &self,
        request: PaginatedRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult>;

    /// Subscribe to resource updates
    async fn subscribe(
        &self,
        request: SubscribeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<()>;

    /// Unsubscribe from resource updates
    async fn unsubscribe(
        &self,
        request: UnsubscribeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<()>;

    // Prompt-specific methods (for custom prompt implementations)

    /// Get cozy prompt messages
    async fn get_cozy_prompt_messages(
        &self,
        args: HashMap<String, String>,
        context: RequestContext<RoleServer>,
    ) -> Result<Vec<PromptMessage>> {
        let _ = (args, context);
        Err(Error::method_not_found("get_cozy_prompt_messages"))
    }

    /// Get event prompt messages
    async fn get_event_prompt_messages(
        &self,
        args: HashMap<String, String>,
        context: RequestContext<RoleServer>,
    ) -> Result<Vec<PromptMessage>> {
        let _ = (args, context);
        Err(Error::method_not_found("get_event_prompt_messages"))
    }

    /// Get energy prompt messages
    async fn get_energy_prompt_messages(
        &self,
        args: HashMap<String, String>,
        context: RequestContext<RoleServer>,
    ) -> Result<Vec<PromptMessage>> {
        let _ = (args, context);
        Err(Error::method_not_found("get_energy_prompt_messages"))
    }

    /// Get morning prompt messages
    async fn get_morning_prompt_messages(
        &self,
        args: HashMap<String, String>,
        context: RequestContext<RoleServer>,
    ) -> Result<Vec<PromptMessage>> {
        let _ = (args, context);
        Err(Error::method_not_found("get_morning_prompt_messages"))
    }

    /// Get night prompt messages
    async fn get_night_prompt_messages(
        &self,
        args: HashMap<String, String>,
        context: RequestContext<RoleServer>,
    ) -> Result<Vec<PromptMessage>> {
        let _ = (args, context);
        Err(Error::method_not_found("get_night_prompt_messages"))
    }

    /// Get comfort optimization messages
    async fn get_comfort_optimization_messages(
        &self,
        args: HashMap<String, String>,
        context: RequestContext<RoleServer>,
    ) -> Result<Vec<PromptMessage>> {
        let _ = (args, context);
        Err(Error::method_not_found("get_comfort_optimization_messages"))
    }

    /// Get seasonal adjustment messages
    async fn get_seasonal_adjustment_messages(
        &self,
        args: HashMap<String, String>,
        context: RequestContext<RoleServer>,
    ) -> Result<Vec<PromptMessage>> {
        let _ = (args, context);
        Err(Error::method_not_found("get_seasonal_adjustment_messages"))
    }

    /// Get security analysis messages
    async fn get_security_analysis_messages(
        &self,
        args: HashMap<String, String>,
        context: RequestContext<RoleServer>,
    ) -> Result<Vec<PromptMessage>> {
        let _ = (args, context);
        Err(Error::method_not_found("get_security_analysis_messages"))
    }

    /// Get troubleshooting messages
    async fn get_troubleshooting_messages(
        &self,
        args: HashMap<String, String>,
        context: RequestContext<RoleServer>,
    ) -> Result<Vec<PromptMessage>> {
        let _ = (args, context);
        Err(Error::method_not_found("get_troubleshooting_messages"))
    }

    /// Get custom scene messages
    async fn get_custom_scene_messages(
        &self,
        args: HashMap<String, String>,
        context: RequestContext<RoleServer>,
    ) -> Result<Vec<PromptMessage>> {
        let _ = (args, context);
        Err(Error::method_not_found("get_custom_scene_messages"))
    }
}
