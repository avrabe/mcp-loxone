//! Simple Ollama HTTP client for demonstration
//!
//! This module provides a basic HTTP client for interacting with Ollama's API
//! to demonstrate real LLM integration.

use crate::error::{LoxoneError, Result};
#[cfg(test)]
use crate::sampling::SamplingMessage;
use crate::sampling::{SamplingRequest, SamplingResponse};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

/// Ollama API generate request
#[derive(Debug, Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

/// Ollama generation options
#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
}

/// Ollama API response
#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    model: String,
    response: String,
    done: bool,
    #[serde(default)]
    #[allow(dead_code)]
    context: Vec<i32>,
    #[serde(default)]
    total_duration: u64,
    #[serde(default)]
    #[allow(dead_code)]
    load_duration: u64,
    #[serde(default)]
    #[allow(dead_code)]
    prompt_eval_count: u32,
    #[serde(default)]
    eval_count: u32,
    #[serde(default)]
    #[allow(dead_code)]
    eval_duration: u64,
}

/// Ollama model info
#[derive(Debug, Deserialize)]
struct OllamaModelInfo {
    name: String,
    #[allow(dead_code)]
    modified_at: String,
    #[allow(dead_code)]
    size: u64,
}

/// Ollama models list response
#[derive(Debug, Deserialize)]
struct OllamaModelsResponse {
    models: Vec<OllamaModelInfo>,
}

/// Simple Ollama HTTP client
pub struct OllamaHttpClient {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaHttpClient {
    /// Create a new Ollama HTTP client
    pub fn new(base_url: String, model: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120)) // 2 minutes timeout for LLM responses
            .build()
            .map_err(|e| LoxoneError::connection(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            base_url,
            model,
            client,
        })
    }

    /// Check if Ollama is available
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    debug!("Ollama health check passed");
                    Ok(true)
                } else {
                    debug!(
                        "Ollama health check failed with status: {}",
                        response.status()
                    );
                    Ok(false)
                }
            }
            Err(e) => {
                debug!("Ollama health check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// List available models
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| LoxoneError::connection(format!("Failed to list models: {}", e)))?;

        if !response.status().is_success() {
            return Err(LoxoneError::connection(format!(
                "Failed to list models: HTTP {}",
                response.status()
            )));
        }

        let models_response: OllamaModelsResponse = response
            .json()
            .await
            .map_err(|e| LoxoneError::config(format!("Failed to parse models response: {}", e)))?;

        Ok(models_response.models.into_iter().map(|m| m.name).collect())
    }

    /// Check if a specific model is available
    pub async fn has_model(&self, model_name: &str) -> Result<bool> {
        let models = self.list_models().await?;
        Ok(models.iter().any(|m| m == model_name))
    }

    /// Generate a response from Ollama
    pub async fn generate(&self, request: &SamplingRequest) -> Result<SamplingResponse> {
        // Convert MCP sampling request to Ollama format
        let prompt = self.build_prompt(request);
        let system_prompt = request.system_prompt.clone();

        let ollama_request = OllamaGenerateRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
            system: system_prompt,
            options: Some(OllamaOptions {
                temperature: request.sampling_params.temperature,
                top_p: request.sampling_params.top_p,
                num_predict: request.sampling_params.max_tokens.map(|t| t as i32),
            }),
        };

        let url = format!("{}/api/generate", self.base_url);

        info!(
            "🦙 Sending request to Ollama at {} with model {}",
            self.base_url, self.model
        );

        let response = self
            .client
            .post(&url)
            .json(&ollama_request)
            .send()
            .await
            .map_err(|e| {
                LoxoneError::connection(format!("Failed to send Ollama request: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LoxoneError::connection(format!(
                "Ollama request failed: HTTP {} - {}",
                status, error_text
            )));
        }

        let ollama_response: OllamaGenerateResponse = response
            .json()
            .await
            .map_err(|e| LoxoneError::config(format!("Failed to parse Ollama response: {}", e)))?;

        info!(
            "✅ Received Ollama response ({}ms total, {} tokens)",
            ollama_response.total_duration / 1_000_000,
            ollama_response.eval_count
        );

        // Convert to MCP sampling response
        Ok(SamplingResponse {
            model: ollama_response.model,
            stop_reason: if ollama_response.done {
                "endTurn".to_string()
            } else {
                "maxTokens".to_string()
            },
            role: "assistant".to_string(),
            content: crate::sampling::SamplingMessageContent::text(ollama_response.response),
        })
    }

    /// Build a prompt from sampling messages
    fn build_prompt(&self, request: &SamplingRequest) -> String {
        let mut parts = Vec::new();

        for message in &request.messages {
            let role_prefix = match message.role.as_str() {
                "system" => "System",
                "user" => "User",
                "assistant" => "Assistant",
                _ => &message.role,
            };

            if let Some(text) = &message.content.text {
                parts.push(format!("{}: {}", role_prefix, text));
            }
        }

        parts.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires local Ollama instance"]
    async fn test_ollama_health_check() {
        let client = OllamaHttpClient::new(
            "http://localhost:11434".to_string(),
            "qwen3:14b".to_string(),
        )
        .unwrap();

        let healthy = client.health_check().await.unwrap();
        assert!(healthy, "Ollama should be healthy");
    }

    #[tokio::test]
    #[ignore = "Requires local Ollama instance"]
    async fn test_ollama_list_models() {
        let client = OllamaHttpClient::new(
            "http://localhost:11434".to_string(),
            "qwen3:14b".to_string(),
        )
        .unwrap();

        let models = client.list_models().await.unwrap();
        assert!(!models.is_empty(), "Should have at least one model");
        println!("Available models: {:?}", models);
    }

    #[tokio::test]
    #[ignore = "Requires local Ollama instance with qwen3:14b model"]
    async fn test_ollama_generate() {
        let client = OllamaHttpClient::new(
            "http://localhost:11434".to_string(),
            "qwen3:14b".to_string(),
        )
        .unwrap();

        let request = SamplingRequest::new(vec![SamplingMessage::user("What is 2 + 2?")]);

        let response = client.generate(&request).await.unwrap();
        assert_eq!(response.role, "assistant");
        assert!(response.content.text.is_some());

        let text = response.content.text.unwrap();
        assert!(text.contains("4"), "Response should contain the answer 4");
    }
}
