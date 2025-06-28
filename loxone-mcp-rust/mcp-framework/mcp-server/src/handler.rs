//! Generic request handler for MCP protocol

use crate::{backend::McpBackend, context::RequestContext, middleware::MiddlewareStack};
use pulseengine_mcp_auth::AuthenticationManager;
use pulseengine_mcp_protocol::*;

use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, instrument};

/// Error type for handler operations
#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Authorization failed: {0}")]
    Authorization(String),

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("Protocol error: {0}")]
    Protocol(#[from] Error),
}

/// Generic server handler that implements the MCP protocol
#[derive(Clone)]
pub struct GenericServerHandler<B: McpBackend> {
    backend: Arc<B>,
    #[allow(dead_code)]
    auth_manager: Arc<AuthenticationManager>,
    middleware: MiddlewareStack,
}

impl<B: McpBackend> GenericServerHandler<B> {
    /// Create a new handler
    pub fn new(
        backend: Arc<B>,
        auth_manager: Arc<AuthenticationManager>,
        middleware: MiddlewareStack,
    ) -> Self {
        Self {
            backend,
            auth_manager,
            middleware,
        }
    }

    /// Handle an MCP request
    #[instrument(skip(self, request))]
    pub async fn handle_request(
        &self,
        request: Request,
    ) -> std::result::Result<Response, HandlerError> {
        debug!("Handling request: {}", request.method);

        // Store request ID before moving request
        let request_id = request.id.clone();

        // Create request context
        let context = RequestContext::new();

        // Apply middleware
        let request = self.middleware.process_request(request, &context).await?;

        // Route to appropriate handler
        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "tools/list" => self.handle_list_tools(request).await,
            "tools/call" => self.handle_call_tool(request).await,
            "resources/list" => self.handle_list_resources(request).await,
            "resources/read" => self.handle_read_resource(request).await,
            "resources/templates/list" => self.handle_list_resource_templates(request).await,
            "prompts/list" => self.handle_list_prompts(request).await,
            "prompts/get" => self.handle_get_prompt(request).await,
            "resources/subscribe" => self.handle_subscribe(request).await,
            "resources/unsubscribe" => self.handle_unsubscribe(request).await,
            "completion/complete" => self.handle_complete(request).await,
            "logging/setLevel" => self.handle_set_level(request).await,
            "ping" => self.handle_ping(request).await,
            _ => self.handle_custom_method(request).await,
        };

        match result {
            Ok(response) => {
                // Apply response middleware
                let response = self.middleware.process_response(response, &context).await?;
                Ok(response)
            }
            Err(error) => {
                error!("Request failed: {}", error);
                Ok(Response {
                    jsonrpc: "2.0".to_string(),
                    id: request_id,
                    result: None,
                    error: Some(error),
                })
            }
        }
    }

    async fn handle_initialize(&self, request: Request) -> std::result::Result<Response, Error> {
        let _params: InitializeRequestParam = serde_json::from_value(request.params)?;

        let result = InitializeResult {
            protocol_version: pulseengine_mcp_protocol::MCP_VERSION.to_string(),
            capabilities: self.backend.get_server_info().capabilities,
            server_info: self.backend.get_server_info().server_info.clone(),
            instructions: Some(String::new()), // MCP Inspector expects a string, not null
        };

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }

    async fn handle_list_tools(&self, request: Request) -> std::result::Result<Response, Error> {
        let params: PaginatedRequestParam = serde_json::from_value(request.params)?;

        let result = self
            .backend
            .list_tools(params)
            .await
            .map_err(|e| e.into())?;

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }

    async fn handle_call_tool(&self, request: Request) -> std::result::Result<Response, Error> {
        let params: CallToolRequestParam = serde_json::from_value(request.params)?;

        let result = self.backend.call_tool(params).await.map_err(|e| e.into())?;

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }

    async fn handle_list_resources(
        &self,
        request: Request,
    ) -> std::result::Result<Response, Error> {
        let params: PaginatedRequestParam = serde_json::from_value(request.params)?;

        let result = self
            .backend
            .list_resources(params)
            .await
            .map_err(|e| e.into())?;

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }

    async fn handle_read_resource(&self, request: Request) -> std::result::Result<Response, Error> {
        let params: ReadResourceRequestParam = serde_json::from_value(request.params)?;

        let result = self
            .backend
            .read_resource(params)
            .await
            .map_err(|e| e.into())?;

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }

    async fn handle_list_resource_templates(
        &self,
        request: Request,
    ) -> std::result::Result<Response, Error> {
        let params: PaginatedRequestParam = serde_json::from_value(request.params)?;

        let result = self
            .backend
            .list_resource_templates(params)
            .await
            .map_err(|e| e.into())?;

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }

    async fn handle_list_prompts(&self, request: Request) -> std::result::Result<Response, Error> {
        let params: PaginatedRequestParam = serde_json::from_value(request.params)?;

        let result = self
            .backend
            .list_prompts(params)
            .await
            .map_err(|e| e.into())?;

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }

    async fn handle_get_prompt(&self, request: Request) -> std::result::Result<Response, Error> {
        let params: GetPromptRequestParam = serde_json::from_value(request.params)?;

        let result = self
            .backend
            .get_prompt(params)
            .await
            .map_err(|e| e.into())?;

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }

    async fn handle_subscribe(&self, request: Request) -> std::result::Result<Response, Error> {
        let params: SubscribeRequestParam = serde_json::from_value(request.params)?;

        self.backend.subscribe(params).await.map_err(|e| e.into())?;

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::Value::Object(Default::default())),
            error: None,
        })
    }

    async fn handle_unsubscribe(&self, request: Request) -> std::result::Result<Response, Error> {
        let params: UnsubscribeRequestParam = serde_json::from_value(request.params)?;

        self.backend
            .unsubscribe(params)
            .await
            .map_err(|e| e.into())?;

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::Value::Object(Default::default())),
            error: None,
        })
    }

    async fn handle_complete(&self, request: Request) -> std::result::Result<Response, Error> {
        let params: CompleteRequestParam = serde_json::from_value(request.params)?;

        let result = self.backend.complete(params).await.map_err(|e| e.into())?;

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }

    async fn handle_set_level(&self, request: Request) -> std::result::Result<Response, Error> {
        let params: SetLevelRequestParam = serde_json::from_value(request.params)?;

        self.backend.set_level(params).await.map_err(|e| e.into())?;

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::Value::Object(Default::default())),
            error: None,
        })
    }

    async fn handle_ping(&self, _request: Request) -> std::result::Result<Response, Error> {
        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: _request.id,
            result: Some(serde_json::Value::Object(Default::default())),
            error: None,
        })
    }

    async fn handle_custom_method(&self, request: Request) -> std::result::Result<Response, Error> {
        let result = self
            .backend
            .handle_custom_method(&request.method, request.params)
            .await
            .map_err(|e| e.into())?;

        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(result),
            error: None,
        })
    }
}

// Convert HandlerError to protocol Error
impl From<HandlerError> for Error {
    fn from(err: HandlerError) -> Self {
        match err {
            HandlerError::Authentication(msg) => Error::unauthorized(msg),
            HandlerError::Authorization(msg) => Error::forbidden(msg),
            HandlerError::Backend(msg) => Error::internal_error(msg),
            HandlerError::Protocol(e) => e,
        }
    }
}
