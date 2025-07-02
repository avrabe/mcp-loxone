//! MCP Sampling Protocol Implementation
//!
//! This module defines the core MCP sampling protocol messages and handlers
//! that will integrate with the future MCP framework.

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

/// MCP Sampling method names
pub mod methods {
    pub const CREATE_MESSAGE: &str = "sampling/createMessage";
    pub const LIST_MODELS: &str = "sampling/listModels";
}

/// Capability flags for sampling support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingCapability {
    pub supports_sampling: bool,
    pub supports_streaming: bool,
    pub supports_images: bool,
    pub supports_tools: bool,
}

/// Server capabilities update to include sampling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedServerCapabilities {
    pub tools: bool,
    pub resources: bool,
    pub prompts: bool,
    pub sampling: Option<SamplingCapability>,
}

/// MCP sampling/createMessage request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageRequest {
    pub method: String,
    pub params: crate::sampling::SamplingRequest,
}

/// MCP sampling/createMessage response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageResponse {
    pub result: crate::sampling::SamplingResponse,
}

/// Sampling protocol handler trait
/// This will be implemented by the server when proper MCP framework support is available
#[async_trait::async_trait]
pub trait SamplingProtocolHandler: Send + Sync {
    /// Handle incoming sampling/createMessage request from server
    async fn handle_create_message(
        &self,
        request: CreateMessageRequest,
    ) -> Result<CreateMessageResponse>;

    /// Check if the connected client supports sampling
    async fn check_sampling_capability(&self) -> Result<SamplingCapability>;

    /// Send a sampling request to the client
    async fn send_sampling_request(
        &self,
        request: crate::sampling::SamplingRequest,
    ) -> Result<crate::sampling::SamplingResponse>;
}

/* Commented out until provider module is fixed
/// Real LLM provider-based protocol handler
pub struct LLMSamplingProtocolHandler {
    capability: SamplingCapability,
    client: Arc<crate::sampling::client::SamplingClientManager>,
}

impl LLMSamplingProtocolHandler {
    /// Create a new LLM sampling protocol handler with provider factory
    pub async fn new(_provider_factory: Arc<()>) -> Result<Self> {
        let client_manager = crate::sampling::client::SamplingClientManager::new_with_providers(provider_factory).await?;

        // Check if sampling is supported
        let is_available = client_manager.is_available().await;

        Ok(Self {
            capability: SamplingCapability {
                supports_sampling: is_available,
                supports_streaming: true, // All our providers support streaming
                supports_images: false,   // Not implemented yet
                supports_tools: false,    // Not implemented yet
            },
            client: Arc::new(client_manager),
        })
    }
}

#[async_trait::async_trait]
impl SamplingProtocolHandler for LLMSamplingProtocolHandler {
    async fn handle_create_message(
        &self,
        request: CreateMessageRequest,
    ) -> Result<CreateMessageResponse> {
        if !self.capability.supports_sampling {
            return Err(LoxoneError::ServiceUnavailable(
                "LLM sampling not available".to_string(),
            ));
        }

        info!("🤖 Handling real LLM create message request");
        let response = self.client.request_sampling(request.params).await?;

        Ok(CreateMessageResponse { result: response })
    }

    async fn check_sampling_capability(&self) -> Result<SamplingCapability> {
        Ok(self.capability.clone())
    }

    async fn send_sampling_request(
        &self,
        request: crate::sampling::SamplingRequest,
    ) -> Result<crate::sampling::SamplingResponse> {
        if !self.capability.supports_sampling {
            return Err(LoxoneError::ServiceUnavailable(
                "LLM sampling not supported".to_string(),
            ));
        }

        info!("🧠 Sending real LLM sampling request");
        self.client.request_sampling(request).await
    }
}
*/

/// Mock implementation for testing and development
pub struct MockSamplingProtocolHandler {
    capability: SamplingCapability,
    client: Arc<crate::sampling::client::SamplingClientManager>,
}

impl MockSamplingProtocolHandler {
    pub fn new(enable_sampling: bool) -> Self {
        Self {
            capability: SamplingCapability {
                supports_sampling: enable_sampling,
                supports_streaming: false,
                supports_images: false,
                supports_tools: false,
            },
            client: Arc::new(
                crate::sampling::client::SamplingClientManager::new_with_mock(enable_sampling),
            ),
        }
    }
}

#[async_trait::async_trait]
impl SamplingProtocolHandler for MockSamplingProtocolHandler {
    async fn handle_create_message(
        &self,
        request: CreateMessageRequest,
    ) -> Result<CreateMessageResponse> {
        if !self.capability.supports_sampling {
            return Err(LoxoneError::ServiceUnavailable(
                "Sampling not supported".to_string(),
            ));
        }

        debug!("Mock handling create message request");
        let response = self.client.request_sampling(request.params).await?;

        Ok(CreateMessageResponse { result: response })
    }

    async fn check_sampling_capability(&self) -> Result<SamplingCapability> {
        Ok(self.capability.clone())
    }

    async fn send_sampling_request(
        &self,
        request: crate::sampling::SamplingRequest,
    ) -> Result<crate::sampling::SamplingResponse> {
        if !self.capability.supports_sampling {
            return Err(LoxoneError::ServiceUnavailable(
                "Sampling not supported by client".to_string(),
            ));
        }

        info!("Sending sampling request to mock client");
        self.client.request_sampling(request).await
    }
}

/// Sampling protocol integration point for the server
/// This will be the main interface for the server to use sampling
pub struct SamplingProtocolIntegration {
    handler: Arc<dyn SamplingProtocolHandler>,
}

impl SamplingProtocolIntegration {
    // Create new integration with real LLM providers
    // Commented out until provider module is fixed
    // pub async fn new_with_providers(provider_factory: Arc<LLMProviderFactory>) -> Result<Self> {
    //     let handler = Arc::new(LLMSamplingProtocolHandler::new(provider_factory).await?);
    //     Ok(Self { handler })
    // }

    /// Create new integration with mock handler
    pub fn new_with_mock(enable_sampling: bool) -> Self {
        Self {
            handler: Arc::new(MockSamplingProtocolHandler::new(enable_sampling)),
        }
    }

    /// Create new integration with custom handler (for future framework)
    pub fn new_with_handler(handler: Arc<dyn SamplingProtocolHandler>) -> Self {
        Self { handler }
    }

    /// Check if sampling is available
    pub async fn is_sampling_available(&self) -> bool {
        match self.handler.check_sampling_capability().await {
            Ok(cap) => cap.supports_sampling,
            Err(_) => false,
        }
    }

    /// Send a sampling request
    pub async fn request_sampling(
        &self,
        request: crate::sampling::SamplingRequest,
    ) -> Result<crate::sampling::SamplingResponse> {
        self.handler.send_sampling_request(request).await
    }

    /// Get sampling capabilities
    pub async fn get_capabilities(&self) -> Result<SamplingCapability> {
        self.handler.check_sampling_capability().await
    }
}

/// Helper to check if a client supports sampling based on capabilities
pub fn client_supports_sampling(capabilities: &serde_json::Value) -> bool {
    capabilities
        .get("sampling")
        .and_then(|s| s.as_object())
        .and_then(|o| o.get("supports_sampling"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_sampling_protocol() {
        let integration = SamplingProtocolIntegration::new_with_mock(true);
        assert!(integration.is_sampling_available().await);

        let request =
            crate::sampling::SamplingRequest::new(vec![crate::sampling::SamplingMessage::user(
                "Test message",
            )]);

        let response = integration.request_sampling(request).await.unwrap();
        assert_eq!(response.role, "assistant");
    }

    #[test]
    fn test_client_capability_check() {
        let caps = serde_json::json!({
            "sampling": {
                "supports_sampling": true,
                "supports_streaming": false
            }
        });
        assert!(client_supports_sampling(&caps));

        let no_sampling = serde_json::json!({
            "tools": true,
            "resources": true
        });
        assert!(!client_supports_sampling(&no_sampling));
    }
}
