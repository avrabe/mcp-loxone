//! MCP Sampling client interface
//!
//! This module provides the interface for servers to send sampling requests
//! to MCP clients following the proper MCP sampling protocol.

use crate::error::{LoxoneError, Result};
use crate::sampling::{SamplingRequest, SamplingResponse};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn, info, error};

/// Trait for MCP sampling clients
#[async_trait]
pub trait SamplingClient: Send + Sync {
    /// Send a sampling request to the MCP client
    async fn create_message(&self, request: SamplingRequest) -> Result<SamplingResponse>;

    /// Check if sampling is supported by the connected client
    fn is_sampling_supported(&self) -> bool;

    /// Get sampling capability information
    fn get_sampling_capabilities(&self) -> SamplingCapabilities;

    /// Enable downcasting to concrete types for health checking
    fn as_any(&self) -> &dyn std::any::Any;
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
    provider_type: String,
    last_health_check: Arc<RwLock<Option<Instant>>>,
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
            provider_type: "mock".to_string(),
            last_health_check: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new mock sampling client with specific provider type
    pub fn new_with_provider(provider_type: &str) -> Self {
        let (max_tokens, models) = match provider_type {
            "ollama" => (8192, vec!["llama3.2".to_string(), "llama3.1".to_string()]),
            "openai" => (4096, vec!["gpt-4o".to_string(), "gpt-4".to_string()]),
            "anthropic" => (200000, vec!["claude-3-5-sonnet-20241022".to_string(), "claude-3-sonnet".to_string()]),
            _ => (4000, vec!["mock-model".to_string()]),
        };

        Self {
            capabilities: SamplingCapabilities {
                supported: true,
                max_tokens: Some(max_tokens),
                supported_models: models,
                supports_images: provider_type == "openai" || provider_type == "anthropic",
                supports_audio: false,
            },
            fallback_enabled: true,
            provider_type: provider_type.to_string(),
            last_health_check: Arc::new(RwLock::new(Some(Instant::now()))),
        }
    }

    /// Simulate a health check for this provider
    pub async fn health_check(&self) -> bool {
        // Update last health check time
        *self.last_health_check.write().await = Some(Instant::now());
        
        // Simulate different health scenarios based on provider type
        match self.provider_type.as_str() {
            "ollama" => {
                // Simulate Ollama being occasionally unavailable (local service)
                let available = std::env::var("OLLAMA_HEALTH_OVERRIDE")
                    .map(|v| v == "true")
                    .unwrap_or(true); // Default to healthy
                debug!("🦙 Ollama health check: {}", if available { "healthy" } else { "unhealthy" });
                available
            }
            "openai" => {
                // Simulate OpenAI being more reliable (cloud service)
                let available = std::env::var("OPENAI_HEALTH_OVERRIDE")
                    .map(|v| v == "true")
                    .unwrap_or(true);
                debug!("🤖 OpenAI health check: {}", if available { "healthy" } else { "unhealthy" });
                available
            }
            "anthropic" => {
                // Simulate Anthropic being reliable (cloud service)
                let available = std::env::var("ANTHROPIC_HEALTH_OVERRIDE")
                    .map(|v| v == "true")
                    .unwrap_or(true);
                debug!("🏛️ Anthropic health check: {}", if available { "healthy" } else { "unhealthy" });
                available
            }
            _ => self.fallback_enabled,
        }
    }

    /// Get provider type
    pub fn provider_type(&self) -> &str {
        &self.provider_type
    }
}

#[async_trait]
impl SamplingClient for MockSamplingClient {
    async fn create_message(&self, request: SamplingRequest) -> Result<SamplingResponse> {
        // Check if this provider is healthy
        if !self.health_check().await {
            let error_msg = format!("{} provider is currently unavailable", self.provider_type);
            return Err(LoxoneError::ServiceUnavailable(error_msg));
        }

        if !self.fallback_enabled {
            return Err(LoxoneError::ServiceUnavailable(
                "Sampling not supported by client".to_string(),
            ));
        }

        debug!(
            "{} mock sampling request with {} messages",
            self.provider_type,
            request.messages.len()
        );

        // Generate a mock response based on the request and provider type
        let user_message = request
            .messages
            .iter()
            .find(|m| m.role == "user")
            .ok_or_else(|| LoxoneError::InvalidInput("No user message found".to_string()))?;

        let response_text = if let Some(text) = &user_message.content.text {
            generate_mock_response_for_provider(text, request.system_prompt.as_deref(), &self.provider_type)
        } else {
            format!("I understand your automation request. This is a {} mock response. For real AI-powered automation suggestions, please configure actual {} API credentials.", self.provider_type, self.provider_type)
        };

        let model_name = match self.provider_type.as_str() {
            "ollama" => "llama3.2:latest",
            "openai" => "gpt-4o",
            "anthropic" => "claude-3-5-sonnet-20241022",
            _ => "mock-model",
        };

        Ok(SamplingResponse {
            model: model_name.to_string(),
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
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

/// Generate a provider-specific mock response for testing
fn generate_mock_response_for_provider(user_text: &str, _system_prompt: Option<&str>, provider_type: &str) -> String {
    let user_lower = user_text.to_lowercase();
    let provider_icon = match provider_type {
        "ollama" => "🦙",
        "openai" => "🤖", 
        "anthropic" => "🏛️",
        _ => "🧪",
    };

    let provider_name = match provider_type {
        "ollama" => "Ollama (Local LLM)",
        "openai" => "OpenAI GPT",
        "anthropic" => "Anthropic Claude",
        _ => "Mock Provider",
    };

    // Determine the type of request
    if user_lower.contains("cozy") {
        format!("{} Creating a Cozy Atmosphere ({} Mock Response)\n\nBased on your request, I recommend dimming lights to 30%, adjusting temperature to 22°C, and partially closing blinds for intimacy.\n\n*Note: This is a {} mock response. Configure actual {} credentials for real AI-powered suggestions.*", 
            provider_icon, provider_name, provider_type, provider_type)
    } else if user_lower.contains("event") || user_lower.contains("party") {
        format!("{} Event Preparation ({} Mock Response)\n\nFor your event, I suggest bright entrance lighting, appropriate room temperature, and testing all critical systems.\n\n*Note: This is a {} mock response. Configure actual {} credentials for real AI-powered suggestions.*",
            provider_icon, provider_name, provider_type, provider_type)
    } else {
        format!("{} Home Automation Suggestion ({} Mock Response)\n\nI understand you'd like help with your home automation. For intelligent, context-aware suggestions, configure actual {} credentials.\n\n*Current Status: Using {} simulation for development/testing.*",
            provider_icon, provider_name, provider_type, provider_type)
    }
}

/// Sampling client manager with intelligent provider fallback
pub struct SamplingClientManager {
    /// Primary client (usually mock or Ollama)
    primary_client: Arc<dyn SamplingClient>,
    /// Fallback clients (OpenAI, Anthropic, etc.)
    fallback_clients: Vec<Arc<dyn SamplingClient>>,
    /// Combined capabilities
    capabilities: Arc<RwLock<SamplingCapabilities>>,
    /// Provider configuration for fallback logic
    config: Arc<crate::sampling::config::ProviderFactoryConfig>,
    /// Health status of providers
    provider_health: Arc<RwLock<std::collections::HashMap<String, bool>>>,
}

impl SamplingClientManager {
    /// Create a new sampling client manager with mock client
    pub fn new_with_mock(_fallback_enabled: bool) -> Self {
        let config = crate::sampling::config::ProviderFactoryConfig::from_env();
        Self::new_with_config(config)
    }

    /// Create a new sampling client manager with enhanced configuration
    pub fn new_with_config(config: crate::sampling::config::ProviderFactoryConfig) -> Self {
        // Create clients based on configuration
        let primary_client: Arc<dyn SamplingClient> = if config.ollama.enabled {
            Arc::new(MockSamplingClient::new_with_provider("ollama"))
        } else {
            Arc::new(MockSamplingClient::new(true))
        };
        
        let mut fallback_clients = Vec::new();
        let mut health_map = HashMap::new();
        
        // Add fallback clients based on configuration priority
        let mut providers: Vec<_> = vec![
            ("openai", config.openai.enabled, config.openai.priority),
            ("anthropic", config.anthropic.enabled, config.anthropic.priority),
        ];
        
        // Sort by priority (lower number = higher priority)
        providers.sort_by_key(|(_, _, priority)| *priority);
        
        for (provider_name, enabled, _) in providers {
            if enabled {
                let client: Arc<dyn SamplingClient> = Arc::new(MockSamplingClient::new_with_provider(provider_name));
                fallback_clients.push(client);
                health_map.insert(provider_name.to_string(), true);
                debug!("📦 Added {} fallback client", provider_name);
            }
        }
        
        // Set primary provider health
        let primary_provider = if config.ollama.enabled { "ollama" } else { "mock" };
        health_map.insert(primary_provider.to_string(), true);
        
        let capabilities = Arc::new(RwLock::new(primary_client.get_sampling_capabilities()));
        let provider_health = Arc::new(RwLock::new(health_map));
        let config_arc = Arc::new(config);

        info!("🧠 Enhanced sampling client manager initialized");
        info!("🎯 Primary provider: {}", primary_provider);
        info!("🔄 Fallback providers: {}", fallback_clients.len());
        if !fallback_clients.is_empty() {
            info!("✅ Intelligent fallback enabled");
        }
        
        Self {
            primary_client,
            fallback_clients,
            capabilities,
            config: config_arc,
            provider_health,
        }
    }

    /// Check if sampling is available
    pub async fn is_available(&self) -> bool {
        self.primary_client.is_sampling_supported()
    }

    /// Send a sampling request with intelligent fallback
    pub async fn request_sampling(&self, request: SamplingRequest) -> Result<SamplingResponse> {
        if !self.primary_client.is_sampling_supported() {
            warn!("Sampling request attempted but not supported by client");
            return Err(LoxoneError::ServiceUnavailable(
                "Sampling not supported by connected MCP client. Please use a client like Claude Desktop that supports the sampling protocol.".to_string(),
            ));
        }

        debug!(
            "Sending sampling request with {} messages",
            request.messages.len()
        );

        // Try primary provider first
        match self.try_primary_client(&request).await {
            Ok(response) => {
                debug!("✅ Primary provider response received from model: {}", response.model);
                Ok(response)
            }
            Err(primary_error) => {
                warn!("⚠️ Primary provider failed: {}", primary_error);
                
                // Only attempt fallback if configured
                if !self.config.selection.enable_fallback || self.fallback_clients.is_empty() {
                    warn!("🚫 No fallback providers configured or available");
                    return Err(primary_error);
                }

                info!("🔄 Attempting fallback providers...");
                
                // Try each fallback provider in priority order
                for (index, fallback_client) in self.fallback_clients.iter().enumerate() {
                    info!("🔄 Trying fallback provider {} of {}", index + 1, self.fallback_clients.len());
                    
                    match self.try_fallback_client(fallback_client, &request).await {
                        Ok(response) => {
                            info!("✅ Fallback provider {} succeeded: {}", index + 1, response.model);
                            return Ok(response);
                        }
                        Err(e) => {
                            warn!("⚠️ Fallback provider {} failed: {}", index + 1, e);
                            continue;
                        }
                    }
                }
                
                error!("❌ All providers failed - returning original error");
                Err(primary_error)
            }
        }
    }

    /// Try the primary client with health check
    async fn try_primary_client(&self, request: &SamplingRequest) -> Result<SamplingResponse> {
        // Check health of primary provider if it's a mock client with health check capability
        if let Some(mock_client) = self.primary_client.as_any().downcast_ref::<MockSamplingClient>() {
            if !mock_client.health_check().await {
                return Err(LoxoneError::ServiceUnavailable(
                    format!("{} provider is currently unhealthy", mock_client.provider_type())
                ));
            }
        }

        self.primary_client.create_message(request.clone()).await
    }

    /// Try a fallback client with health check
    async fn try_fallback_client(&self, client: &Arc<dyn SamplingClient>, request: &SamplingRequest) -> Result<SamplingResponse> {
        // Check health of fallback provider if it's a mock client with health check capability
        if let Some(mock_client) = client.as_any().downcast_ref::<MockSamplingClient>() {
            if !mock_client.health_check().await {
                return Err(LoxoneError::ServiceUnavailable(
                    format!("{} fallback provider is currently unhealthy", mock_client.provider_type())
                ));
            }
        }

        client.create_message(request.clone()).await
    }

    /// Get current capabilities
    pub async fn get_capabilities(&self) -> SamplingCapabilities {
        self.capabilities.read().await.clone()
    }

    /// Check health of all providers and update health status
    pub async fn check_provider_health(&self) -> HashMap<String, bool> {
        let mut health_results = HashMap::new();
        
        // Check primary provider health
        let primary_provider = if self.config.ollama.enabled { "ollama" } else { "mock" };
        let primary_healthy = if let Some(mock_client) = self.primary_client.as_any().downcast_ref::<MockSamplingClient>() {
            mock_client.health_check().await
        } else {
            true // Assume healthy for non-mock clients
        };
        health_results.insert(primary_provider.to_string(), primary_healthy);
        
        // Check fallback providers health
        for (i, client) in self.fallback_clients.iter().enumerate() {
            let provider_name = if i == 0 && self.config.openai.enabled {
                "openai"
            } else if self.config.anthropic.enabled {
                "anthropic"
            } else {
                &format!("fallback_{}", i)
            };
            
            let healthy = if let Some(mock_client) = client.as_any().downcast_ref::<MockSamplingClient>() {
                mock_client.health_check().await
            } else {
                true // Assume healthy for non-mock clients
            };
            health_results.insert(provider_name.to_string(), healthy);
        }
        
        // Update stored health status
        {
            let mut health_map = self.provider_health.write().await;
            for (provider, healthy) in &health_results {
                health_map.insert(provider.clone(), *healthy);
            }
        }
        
        health_results
    }

    /// Get current provider health status
    pub async fn get_provider_health(&self) -> HashMap<String, bool> {
        self.provider_health.read().await.clone()
    }

    /// Get a summary of current provider status
    pub async fn get_provider_summary(&self) -> String {
        let health = self.get_provider_health().await;
        let total_providers = 1 + self.fallback_clients.len(); // Primary + fallbacks
        let healthy_providers = health.values().filter(|&&v| v).count();
        
        format!(
            "Providers: {}/{} healthy (Primary: {}, Fallback: {} available)",
            healthy_providers,
            total_providers,
            if self.config.ollama.enabled { "Ollama" } else { "Mock" },
            self.fallback_clients.len()
        )
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
