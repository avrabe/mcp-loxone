//! MCP Sampling client interface
//!
//! This module provides the interface for servers to send sampling requests
//! to MCP clients following the proper MCP sampling protocol.

use crate::error::{LoxoneError, Result};
use crate::sampling::{SamplingRequest, SamplingResponse};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Trait for MCP sampling clients
#[async_trait]
pub trait SamplingClient: Send + Sync {
    /// Send a sampling request to the MCP client
    async fn create_message(&self, request: SamplingRequest) -> Result<SamplingResponse>;

    /// Check if sampling is supported by the connected client
    fn is_sampling_supported(&self) -> bool;

    /// Get sampling capability information
    fn get_sampling_capabilities(&self) -> SamplingCapabilities;
}

/// Sampling capabilities supported by the client
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SamplingCapabilities {
    pub supported: bool,
    pub max_tokens: Option<u32>,
    pub supported_models: Vec<String>,
    pub supports_images: bool,
    pub supports_audio: bool,
}

/// Mock sampling client for testing and fallback
pub struct MockSamplingClient {
    capabilities: SamplingCapabilities,
    fallback_enabled: bool,
}

impl MockSamplingClient {
    /// Create a new mock sampling client
    pub fn new(fallback_enabled: bool) -> Self {
        Self {
            capabilities: SamplingCapabilities {
                supported: fallback_enabled,
                max_tokens: Some(4000),
                supported_models: vec!["claude-3-sonnet".to_string(), "gpt-4".to_string()],
                supports_images: false,
                supports_audio: false,
            },
            fallback_enabled,
        }
    }
}

#[async_trait]
impl SamplingClient for MockSamplingClient {
    async fn create_message(&self, request: SamplingRequest) -> Result<SamplingResponse> {
        if !self.fallback_enabled {
            return Err(LoxoneError::ServiceUnavailable(
                "Sampling not supported by client".to_string(),
            ));
        }

        debug!(
            "Mock sampling request with {} messages",
            request.messages.len()
        );

        // Generate a mock response based on the request
        let user_message = request
            .messages
            .iter()
            .find(|m| m.role == "user")
            .ok_or_else(|| LoxoneError::InvalidInput("No user message found".to_string()))?;

        let response_text = if let Some(text) = &user_message.content.text {
            generate_mock_response(text, request.system_prompt.as_deref())
        } else {
            "I understand your automation request. However, this is a mock response since no actual LLM client is connected. Please connect an MCP client that supports sampling for real AI-powered automation suggestions.".to_string()
        };

        Ok(SamplingResponse {
            model: "mock-model".to_string(),
            stop_reason: "endTurn".to_string(),
            role: "assistant".to_string(),
            content: crate::sampling::SamplingMessageContent::text(response_text),
        })
    }

    fn is_sampling_supported(&self) -> bool {
        self.capabilities.supported
    }

    fn get_sampling_capabilities(&self) -> SamplingCapabilities {
        self.capabilities.clone()
    }
}

/// Generate a mock response for testing
fn generate_mock_response(user_text: &str, _system_prompt: Option<&str>) -> String {
    let user_lower = user_text.to_lowercase();

    // Determine the type of request
    if user_lower.contains("cozy") {
        "🏠 Creating a Cozy Atmosphere (Mock Response)\n\nBased on your request, I recommend dimming lights to 30%, adjusting temperature to 22°C, and partially closing blinds for intimacy.\n\n*Note: This is a mock response. Connect a real MCP client with sampling support for AI-powered suggestions.*".to_string()
    } else if user_lower.contains("event") || user_lower.contains("party") {
        "🎉 Event Preparation (Mock Response)\n\nFor your event, I suggest bright entrance lighting, appropriate room temperature, and testing all critical systems.\n\n*Note: This is a mock response. Connect a real MCP client with sampling support for AI-powered suggestions.*".to_string()
    } else {
        "🏠 Home Automation Suggestion (Mock Response)\n\nI understand you'd like help with your home automation. For intelligent, context-aware suggestions, connect an MCP client that supports sampling.".to_string()
    }
}

/// Sampling client manager
pub struct SamplingClientManager {
    client: Arc<dyn SamplingClient>,
    capabilities: Arc<RwLock<SamplingCapabilities>>,
}

impl SamplingClientManager {
    /// Create a new sampling client manager with mock client
    pub fn new_with_mock(fallback_enabled: bool) -> Self {
        let client = Arc::new(MockSamplingClient::new(fallback_enabled));
        let capabilities = Arc::new(RwLock::new(client.get_sampling_capabilities()));

        debug!("🧪 Sampling client manager initialized with mock client");
        Self {
            client,
            capabilities,
        }
    }

    /// Check if sampling is available
    pub async fn is_available(&self) -> bool {
        self.client.is_sampling_supported()
    }

    /// Send a sampling request
    pub async fn request_sampling(&self, request: SamplingRequest) -> Result<SamplingResponse> {
        if !self.client.is_sampling_supported() {
            warn!("Sampling request attempted but not supported by client");
            return Err(LoxoneError::ServiceUnavailable(
                "Sampling not supported by connected MCP client. Please use a client like Claude Desktop that supports the sampling protocol.".to_string(),
            ));
        }

        debug!(
            "Sending sampling request with {} messages",
            request.messages.len()
        );

        match self.client.create_message(request).await {
            Ok(response) => {
                debug!("Sampling response received from model: {}", response.model);
                Ok(response)
            }
            Err(e) => {
                warn!("Sampling request failed: {}", e);
                Err(e)
            }
        }
    }

    /// Get current capabilities
    pub async fn get_capabilities(&self) -> SamplingCapabilities {
        self.capabilities.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sampling::SamplingMessage;

    #[tokio::test]
    async fn test_mock_sampling_client() {
        let client = MockSamplingClient::new(true);
        assert!(client.is_sampling_supported());

        let request = SamplingRequest::new(vec![SamplingMessage::user(
            "Make my home cozy for the evening",
        )]);

        let response = client.create_message(request).await.unwrap();
        assert_eq!(response.role, "assistant");
        assert!(response.content.text.unwrap().contains("Cozy"));
    }
}
