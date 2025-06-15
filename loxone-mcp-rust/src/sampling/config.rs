//! Configuration for LLM API integration and MCP sampling
//!
//! This module provides configuration for both MCP sampling protocol and
//! fallback direct LLM API integration for cases where MCP client doesn't
//! support sampling.

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

/// Complete LLM integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// MCP sampling configuration (preferred)
    pub mcp_sampling: McpSamplingConfig,
    /// Fallback direct API configuration
    pub fallback_apis: HashMap<String, LlmApiConfig>,
    /// Model selection and preferences
    pub model_preferences: ModelPreferences,
    /// Request configuration
    pub request_config: RequestConfig,
    /// Security and rate limiting
    pub security: SecurityConfig,
    /// Caching configuration
    pub caching: CachingConfig,
}

/// MCP Sampling protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSamplingConfig {
    /// Enable MCP sampling (preferred method)
    pub enabled: bool,
    /// Timeout for sampling requests
    pub timeout_seconds: u32,
    /// Maximum retries for failed requests
    pub max_retries: u32,
    /// Retry delay in seconds
    pub retry_delay_seconds: u32,
    /// Whether to require client sampling support
    pub require_client_support: bool,
    /// Whether to validate client capabilities before requests
    pub validate_capabilities: bool,
}

/// Direct LLM API configuration (fallback)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmApiConfig {
    /// Provider name (claude, openai, etc.)
    pub provider: String,
    /// API endpoint URL
    pub endpoint: String,
    /// API key (will be loaded from secure storage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Organization ID if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    /// Custom headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Request timeout in seconds
    pub timeout_seconds: u32,
    /// Rate limit: requests per minute
    pub rate_limit_rpm: u32,
    /// Rate limit: tokens per minute
    pub rate_limit_tpm: u32,
    /// Whether this API is enabled
    pub enabled: bool,
    /// Priority (lower number = higher priority)
    pub priority: u32,
}

/// Model selection and preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPreferences {
    /// Primary model choices (in order of preference)
    pub primary_models: Vec<ModelConfig>,
    /// Fallback models if primary models fail
    pub fallback_models: Vec<ModelConfig>,
    /// Model selection strategy
    pub selection_strategy: ModelSelectionStrategy,
    /// Performance vs cost optimization
    pub optimization: OptimizationPreference,
}

/// Individual model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model identifier (e.g., "claude-3-sonnet", "gpt-4")
    pub name: String,
    /// Provider for this model
    pub provider: String,
    /// Cost per 1k tokens (for optimization)
    pub cost_per_1k_tokens: f64,
    /// Average response latency in ms
    pub avg_latency_ms: u32,
    /// Quality score (1-10)
    pub quality_score: u8,
    /// Maximum context length
    pub max_context_length: u32,
    /// Whether this model supports images
    pub supports_images: bool,
    /// Whether this model supports function calling
    pub supports_functions: bool,
    /// Whether this model is currently available
    pub available: bool,
}

/// Model selection strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelSelectionStrategy {
    /// Always use the first available model
    Sequential,
    /// Choose based on request context and optimization
    Adaptive,
    /// Load balance across available models
    RoundRobin,
    /// Choose best model for each request type
    BestFit,
}

/// Optimization preference for model selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationPreference {
    /// Weight for cost optimization (0.0 - 1.0)
    pub cost_weight: f32,
    /// Weight for speed optimization (0.0 - 1.0)
    pub speed_weight: f32,
    /// Weight for quality optimization (0.0 - 1.0)
    pub quality_weight: f32,
}

/// Request configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestConfig {
    /// Default maximum tokens for responses
    pub default_max_tokens: u32,
    /// Default temperature
    pub default_temperature: f32,
    /// Default top_p value
    pub default_top_p: f32,
    /// Whether to include system prompts
    pub include_system_prompts: bool,
    /// Maximum request retries
    pub max_retries: u32,
    /// Request timeout in seconds
    pub timeout_seconds: u32,
    /// Whether to stream responses
    pub enable_streaming: bool,
    /// Custom stop sequences
    pub stop_sequences: Vec<String>,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Whether to encrypt API keys at rest
    pub encrypt_api_keys: bool,
    /// Key derivation iterations for encryption
    pub key_derivation_iterations: u32,
    /// Whether to validate SSL certificates
    pub validate_ssl: bool,
    /// Whether to log API requests (excluding sensitive data)
    pub log_requests: bool,
    /// Whether to log API responses (excluding sensitive data)
    pub log_responses: bool,
    /// Maximum request size in bytes
    pub max_request_size: usize,
    /// Whether to require API key rotation
    pub require_key_rotation: bool,
    /// API key rotation interval in days
    pub key_rotation_days: u32,
}

/// Caching configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachingConfig {
    /// Whether to enable response caching
    pub enabled: bool,
    /// Cache TTL in seconds
    pub ttl_seconds: u32,
    /// Maximum cache size in MB
    pub max_size_mb: u32,
    /// Whether to cache based on request content hash
    pub content_based_caching: bool,
    /// Whether to cache based on semantic similarity
    pub semantic_caching: bool,
    /// Similarity threshold for semantic caching (0.0 - 1.0)
    pub similarity_threshold: f32,
    /// Cache storage location
    pub storage_path: Option<PathBuf>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            mcp_sampling: McpSamplingConfig::default(),
            fallback_apis: Self::default_apis(),
            model_preferences: ModelPreferences::default(),
            request_config: RequestConfig::default(),
            security: SecurityConfig::default(),
            caching: CachingConfig::default(),
        }
    }
}

impl LlmConfig {
    /// Load configuration from file
    pub fn load_from_file(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| LoxoneError::config(format!("Failed to read config file: {}", e)))?;

        let config: Self = toml::from_str(&content)
            .map_err(|e| LoxoneError::config(format!("Failed to parse config: {}", e)))?;

        config.validate()?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        self.validate()?;

        let content = toml::to_string_pretty(self)
            .map_err(|e| LoxoneError::config(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(path, content)
            .map_err(|e| LoxoneError::config(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate optimization weights sum to 1.0
        let opt = &self.model_preferences.optimization;
        let total_weight = opt.cost_weight + opt.speed_weight + opt.quality_weight;
        if (total_weight - 1.0).abs() > 0.01 {
            return Err(LoxoneError::config(
                "Optimization weights must sum to 1.0".to_string(),
            ));
        }

        // Validate timeout values
        if self.mcp_sampling.timeout_seconds == 0 {
            return Err(LoxoneError::config(
                "MCP sampling timeout must be greater than 0".to_string(),
            ));
        }

        // Validate at least one API is configured if MCP sampling is disabled
        if !self.mcp_sampling.enabled && self.fallback_apis.is_empty() {
            return Err(LoxoneError::config(
                "At least one fallback API must be configured if MCP sampling is disabled"
                    .to_string(),
            ));
        }

        // Validate model configurations
        for model in &self.model_preferences.primary_models {
            if model.quality_score == 0 || model.quality_score > 10 {
                return Err(LoxoneError::config(format!(
                    "Model {} quality score must be between 1-10",
                    model.name
                )));
            }
        }

        Ok(())
    }

    /// Get enabled APIs in priority order
    pub fn get_enabled_apis(&self) -> Vec<(&String, &LlmApiConfig)> {
        let mut apis: Vec<_> = self
            .fallback_apis
            .iter()
            .filter(|(_, config)| config.enabled)
            .collect();

        apis.sort_by_key(|(_, config)| config.priority);
        apis
    }

    /// Get available models for a provider
    pub fn get_models_for_provider(&self, provider: &str) -> Vec<&ModelConfig> {
        self.model_preferences
            .primary_models
            .iter()
            .chain(self.model_preferences.fallback_models.iter())
            .filter(|model| model.provider == provider && model.available)
            .collect()
    }

    /// Default API configurations
    fn default_apis() -> HashMap<String, LlmApiConfig> {
        let mut apis = HashMap::new();

        // Claude (Anthropic)
        apis.insert(
            "claude".to_string(),
            LlmApiConfig {
                provider: "anthropic".to_string(),
                endpoint: "https://api.anthropic.com/v1/messages".to_string(),
                api_key: None, // Will be loaded from secure storage
                organization: None,
                headers: HashMap::from([(
                    "anthropic-version".to_string(),
                    "2023-06-01".to_string(),
                )]),
                timeout_seconds: 30,
                rate_limit_rpm: 50,
                rate_limit_tpm: 40000,
                enabled: false, // Disabled by default, requires API key
                priority: 1,
            },
        );

        // OpenAI GPT
        apis.insert(
            "openai".to_string(),
            LlmApiConfig {
                provider: "openai".to_string(),
                endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
                api_key: None,
                organization: None,
                headers: HashMap::new(),
                timeout_seconds: 30,
                rate_limit_rpm: 60,
                rate_limit_tpm: 60000,
                enabled: false,
                priority: 2,
            },
        );

        // Ollama (local)
        apis.insert(
            "ollama".to_string(),
            LlmApiConfig {
                provider: "ollama".to_string(),
                endpoint: "http://localhost:11434/api/chat".to_string(),
                api_key: None, // Local doesn't need API key
                organization: None,
                headers: HashMap::new(),
                timeout_seconds: 60,  // Local models can be slower
                rate_limit_rpm: 1000, // No rate limit for local
                rate_limit_tpm: 1000000,
                enabled: false, // Requires local Ollama installation
                priority: 3,
            },
        );

        apis
    }
}

impl Default for McpSamplingConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Prefer MCP sampling
            timeout_seconds: 30,
            max_retries: 3,
            retry_delay_seconds: 1,
            require_client_support: false,
            validate_capabilities: true,
        }
    }
}

impl Default for ModelPreferences {
    fn default() -> Self {
        Self {
            primary_models: vec![
                ModelConfig {
                    name: "claude-3-sonnet-20240229".to_string(),
                    provider: "anthropic".to_string(),
                    cost_per_1k_tokens: 0.003,
                    avg_latency_ms: 2000,
                    quality_score: 9,
                    max_context_length: 200000,
                    supports_images: true,
                    supports_functions: true,
                    available: true,
                },
                ModelConfig {
                    name: "gpt-4-turbo".to_string(),
                    provider: "openai".to_string(),
                    cost_per_1k_tokens: 0.01,
                    avg_latency_ms: 3000,
                    quality_score: 9,
                    max_context_length: 128000,
                    supports_images: true,
                    supports_functions: true,
                    available: true,
                },
            ],
            fallback_models: vec![
                ModelConfig {
                    name: "claude-3-haiku-20240307".to_string(),
                    provider: "anthropic".to_string(),
                    cost_per_1k_tokens: 0.00025,
                    avg_latency_ms: 1000,
                    quality_score: 7,
                    max_context_length: 200000,
                    supports_images: true,
                    supports_functions: true,
                    available: true,
                },
                ModelConfig {
                    name: "gpt-3.5-turbo".to_string(),
                    provider: "openai".to_string(),
                    cost_per_1k_tokens: 0.0005,
                    avg_latency_ms: 1500,
                    quality_score: 6,
                    max_context_length: 16384,
                    supports_images: false,
                    supports_functions: true,
                    available: true,
                },
            ],
            selection_strategy: ModelSelectionStrategy::Adaptive,
            optimization: OptimizationPreference {
                cost_weight: 0.2,
                speed_weight: 0.3,
                quality_weight: 0.5,
            },
        }
    }
}

impl Default for RequestConfig {
    fn default() -> Self {
        Self {
            default_max_tokens: 1000,
            default_temperature: 0.7,
            default_top_p: 0.9,
            include_system_prompts: true,
            max_retries: 3,
            timeout_seconds: 30,
            enable_streaming: false, // Disabled for MCP compatibility
            stop_sequences: vec![],
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            encrypt_api_keys: true,
            key_derivation_iterations: 100000,
            validate_ssl: true,
            log_requests: true,
            log_responses: false, // Don't log responses by default for privacy
            max_request_size: 1024 * 1024, // 1MB
            require_key_rotation: false,
            key_rotation_days: 90,
        }
    }
}

impl Default for CachingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_seconds: 3600, // 1 hour
            max_size_mb: 100,
            content_based_caching: true,
            semantic_caching: false, // Requires additional ML models
            similarity_threshold: 0.95,
            storage_path: None, // Use default temp directory
        }
    }
}

/// Configuration manager for LLM settings
pub struct LlmConfigManager {
    config: LlmConfig,
    config_path: PathBuf,
}

impl LlmConfigManager {
    /// Create new configuration manager
    pub fn new(config_path: PathBuf) -> Result<Self> {
        let config = if config_path.exists() {
            let file_size = std::fs::metadata(&config_path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);

            if file_size > 0 {
                // File exists and is not empty, try to load it
                match LlmConfig::load_from_file(&config_path) {
                    Ok(config) => config,
                    Err(_) => {
                        // File exists but is corrupted/empty, create default and save
                        let default_config = LlmConfig::default();
                        default_config.save_to_file(&config_path)?;
                        default_config
                    }
                }
            } else {
                // File exists but is empty - create default config and save it
                let default_config = LlmConfig::default();
                default_config.save_to_file(&config_path)?;
                default_config
            }
        } else {
            // File doesn't exist - create default config and save it
            let default_config = LlmConfig::default();
            default_config.save_to_file(&config_path)?;
            default_config
        };

        Ok(Self {
            config,
            config_path,
        })
    }

    /// Get current configuration
    pub fn get_config(&self) -> &LlmConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, new_config: LlmConfig) -> Result<()> {
        new_config.validate()?;
        self.config = new_config;
        self.save()?;
        Ok(())
    }

    /// Save current configuration
    pub fn save(&self) -> Result<()> {
        self.config.save_to_file(&self.config_path)
    }

    /// Reload configuration from file
    pub fn reload(&mut self) -> Result<()> {
        self.config = LlmConfig::load_from_file(&self.config_path)?;
        Ok(())
    }

    /// Update API key for a provider
    pub fn set_api_key(&mut self, provider: &str, api_key: String) -> Result<()> {
        if let Some(api_config) = self.config.fallback_apis.get_mut(provider) {
            api_config.api_key = Some(api_key);
            api_config.enabled = true; // Enable when API key is set
            self.save()?;
            Ok(())
        } else {
            Err(LoxoneError::config(format!(
                "Unknown provider: {}",
                provider
            )))
        }
    }

    /// Remove API key for a provider
    pub fn remove_api_key(&mut self, provider: &str) -> Result<()> {
        if let Some(api_config) = self.config.fallback_apis.get_mut(provider) {
            api_config.api_key = None;
            api_config.enabled = false; // Disable when API key is removed
            self.save()?;
            Ok(())
        } else {
            Err(LoxoneError::config(format!(
                "Unknown provider: {}",
                provider
            )))
        }
    }

    /// Get best model for a request type
    pub fn get_best_model(&self, request_type: &str, context_size: usize) -> Option<&ModelConfig> {
        let models = &self.config.model_preferences.primary_models;

        // Filter models that can handle the context size
        let suitable_models: Vec<_> = models
            .iter()
            .filter(|model| model.available && model.max_context_length as usize >= context_size)
            .collect();

        if suitable_models.is_empty() {
            return None;
        }

        match self.config.model_preferences.selection_strategy {
            ModelSelectionStrategy::Sequential => suitable_models.first().copied(),
            ModelSelectionStrategy::Adaptive => {
                // Score models based on optimization preferences
                self.score_and_select_model(&suitable_models, request_type)
            }
            ModelSelectionStrategy::RoundRobin => {
                // Simple round-robin (in real implementation, would track state)
                suitable_models.first().copied()
            }
            ModelSelectionStrategy::BestFit => {
                self.select_best_fit_model(&suitable_models, request_type)
            }
        }
    }

    /// Score and select model based on optimization preferences
    fn score_and_select_model<'a>(
        &self,
        models: &[&'a ModelConfig],
        _request_type: &str,
    ) -> Option<&'a ModelConfig> {
        let opt = &self.config.model_preferences.optimization;

        let mut best_model = None;
        let mut best_score = f32::NEG_INFINITY;

        for model in models {
            // Normalize metrics (lower cost and latency are better, higher quality is better)
            let cost_score = 1.0 - (model.cost_per_1k_tokens / 0.01).min(1.0) as f32;
            let speed_score = 1.0 - (model.avg_latency_ms as f32 / 5000.0).min(1.0);
            let quality_score = model.quality_score as f32 / 10.0;

            let total_score = cost_score * opt.cost_weight
                + speed_score * opt.speed_weight
                + quality_score * opt.quality_weight;

            if total_score > best_score {
                best_score = total_score;
                best_model = Some(*model);
            }
        }

        best_model
    }

    /// Select best fit model for specific request type
    fn select_best_fit_model<'a>(
        &self,
        models: &[&'a ModelConfig],
        request_type: &str,
    ) -> Option<&'a ModelConfig> {
        // Prioritize based on request type
        match request_type {
            "image_analysis" => models.iter().find(|m| m.supports_images).copied(),
            "function_calling" => models.iter().find(|m| m.supports_functions).copied(),
            "quick_response" => {
                // Find fastest model
                models.iter().min_by_key(|m| m.avg_latency_ms).copied()
            }
            "high_quality" => {
                // Find highest quality model
                models.iter().max_by_key(|m| m.quality_score).copied()
            }
            _ => {
                // Default to first available
                models.first().copied()
            }
        }
    }

    /// Get configuration summary for logging
    pub fn get_config_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "mcp_sampling_enabled": self.config.mcp_sampling.enabled,
            "enabled_apis": self.config.get_enabled_apis().iter().map(|(name, config)| {
                serde_json::json!({
                    "name": name,
                    "provider": config.provider,
                    "priority": config.priority
                })
            }).collect::<Vec<_>>(),
            "primary_models": self.config.model_preferences.primary_models.len(),
            "fallback_models": self.config.model_preferences.fallback_models.len(),
            "caching_enabled": self.config.caching.enabled,
            "security_encryption": self.config.security.encrypt_api_keys
        })
    }
}

/// Environment-based provider configuration for the LLM provider factory
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderFactoryConfig {
    /// Ollama configuration (PRIMARY provider)
    pub ollama: OllamaConfig,
    /// OpenAI configuration (FALLBACK provider)
    pub openai: OpenAIConfig,
    /// Anthropic configuration (FALLBACK provider)
    pub anthropic: AnthropicConfig,
    /// Provider selection preferences
    pub selection: ProviderSelectionConfig,
    /// Health monitoring configuration
    pub health: ProviderHealthConfig,
}

/// Ollama provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// Ollama server base URL
    pub base_url: String,
    /// Default model to use
    pub default_model: String,
    /// Available models (auto-detected if empty)
    pub available_models: Vec<String>,
    /// Whether to auto-download missing models
    pub auto_download_models: bool,
    /// Connection timeout in seconds
    pub timeout_seconds: u32,
    /// Whether Ollama is enabled
    pub enabled: bool,
    /// Priority level (lower = higher priority)
    pub priority: u32,
}

/// OpenAI provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    /// OpenAI API key (from environment or secure storage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Organization ID (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    /// Default model to use
    pub default_model: String,
    /// Connection timeout in seconds
    pub timeout_seconds: u32,
    /// Whether OpenAI is enabled
    pub enabled: bool,
    /// Priority level (lower = higher priority)
    pub priority: u32,
}

/// Anthropic provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    /// Anthropic API key (from environment or secure storage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Default model to use
    pub default_model: String,
    /// Connection timeout in seconds
    pub timeout_seconds: u32,
    /// Whether Anthropic is enabled
    pub enabled: bool,
    /// Priority level (lower = higher priority)
    pub priority: u32,
}

/// Provider selection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSelectionConfig {
    /// Enable automatic fallback to cloud providers when Ollama unavailable
    pub enable_fallback: bool,
    /// Cost priority weight (0.0 - 1.0)
    pub cost_priority: f32,
    /// Speed priority weight (0.0 - 1.0)
    pub speed_priority: f32,
    /// Quality priority weight (0.0 - 1.0)
    pub quality_priority: f32,
    /// Prefer local providers over cloud providers
    pub prefer_local: bool,
}

/// Provider health monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealthConfig {
    /// Health check interval in seconds
    pub check_interval_seconds: u32,
    /// Health check timeout in seconds
    pub check_timeout_seconds: u32,
    /// Number of failed health checks before marking provider as unhealthy
    pub failure_threshold: u32,
    /// Whether to automatically disable unhealthy providers
    pub auto_disable_unhealthy: bool,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            default_model: "qwen3:14b".to_string(),
            available_models: vec![], // Auto-detected
            auto_download_models: true,
            timeout_seconds: 60,
            enabled: true, // Ollama is PRIMARY provider
            priority: 1,   // Highest priority
        }
    }
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: None, // Must be provided via environment
            organization: None,
            default_model: "gpt-4o".to_string(),
            timeout_seconds: 30,
            enabled: false, // Disabled by default, enabled when API key provided
            priority: 2,    // Secondary priority
        }
    }
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: None, // Must be provided via environment
            default_model: "claude-3-5-sonnet-20241022".to_string(),
            timeout_seconds: 30,
            enabled: false, // Disabled by default, enabled when API key provided
            priority: 3,    // Tertiary priority
        }
    }
}

impl Default for ProviderSelectionConfig {
    fn default() -> Self {
        Self {
            enable_fallback: true,
            cost_priority: 0.3,    // Moderate cost concern
            speed_priority: 0.4,   // Important for user experience
            quality_priority: 0.3, // Balance with cost and speed
            prefer_local: true,    // Prefer Ollama for privacy and cost
        }
    }
}

impl Default for ProviderHealthConfig {
    fn default() -> Self {
        Self {
            check_interval_seconds: 60,    // Check every minute
            check_timeout_seconds: 10,     // 10 second timeout for health checks
            failure_threshold: 3,          // 3 failed checks before marking unhealthy
            auto_disable_unhealthy: false, // Keep trying, don't auto-disable
        }
    }
}

impl ProviderFactoryConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Ollama configuration
        if let Ok(base_url) = env::var("OLLAMA_BASE_URL") {
            config.ollama.base_url = base_url;
        }
        if let Ok(model) = env::var("OLLAMA_DEFAULT_MODEL") {
            config.ollama.default_model = model;
        }
        if let Ok(auto_download) = env::var("OLLAMA_AUTO_DOWNLOAD") {
            config.ollama.auto_download_models = auto_download.parse().unwrap_or(true);
        }
        if let Ok(timeout) = env::var("OLLAMA_TIMEOUT") {
            config.ollama.timeout_seconds = timeout.parse().unwrap_or(60);
        }
        if let Ok(enabled) = env::var("OLLAMA_ENABLED") {
            config.ollama.enabled = enabled.parse().unwrap_or(true);
        }

        // OpenAI configuration
        if let Ok(api_key) = env::var("OPENAI_API_KEY") {
            config.openai.api_key = Some(api_key);
            config.openai.enabled = true; // Auto-enable when API key is provided
        }
        if let Ok(org) = env::var("OPENAI_ORGANIZATION") {
            config.openai.organization = Some(org);
        }
        if let Ok(model) = env::var("OPENAI_DEFAULT_MODEL") {
            config.openai.default_model = model;
        }
        if let Ok(timeout) = env::var("OPENAI_TIMEOUT") {
            config.openai.timeout_seconds = timeout.parse().unwrap_or(30);
        }

        // Anthropic configuration
        if let Ok(api_key) = env::var("ANTHROPIC_API_KEY") {
            config.anthropic.api_key = Some(api_key);
            config.anthropic.enabled = true; // Auto-enable when API key is provided
        }
        if let Ok(model) = env::var("ANTHROPIC_DEFAULT_MODEL") {
            config.anthropic.default_model = model;
        }
        if let Ok(timeout) = env::var("ANTHROPIC_TIMEOUT") {
            config.anthropic.timeout_seconds = timeout.parse().unwrap_or(30);
        }

        // Provider selection configuration
        if let Ok(fallback) = env::var("LLM_ENABLE_FALLBACK") {
            config.selection.enable_fallback = fallback.parse().unwrap_or(true);
        }
        if let Ok(cost_priority) = env::var("LLM_COST_PRIORITY") {
            config.selection.cost_priority = cost_priority.parse().unwrap_or(0.3);
        }
        if let Ok(speed_priority) = env::var("LLM_SPEED_PRIORITY") {
            config.selection.speed_priority = speed_priority.parse().unwrap_or(0.4);
        }
        if let Ok(quality_priority) = env::var("LLM_QUALITY_PRIORITY") {
            config.selection.quality_priority = quality_priority.parse().unwrap_or(0.3);
        }
        if let Ok(prefer_local) = env::var("LLM_PREFER_LOCAL") {
            config.selection.prefer_local = prefer_local.parse().unwrap_or(true);
        }

        // Health monitoring configuration
        if let Ok(interval) = env::var("LLM_HEALTH_CHECK_INTERVAL") {
            config.health.check_interval_seconds = interval.parse().unwrap_or(60);
        }
        if let Ok(timeout) = env::var("LLM_HEALTH_CHECK_TIMEOUT") {
            config.health.check_timeout_seconds = timeout.parse().unwrap_or(10);
        }
        if let Ok(threshold) = env::var("LLM_HEALTH_FAILURE_THRESHOLD") {
            config.health.failure_threshold = threshold.parse().unwrap_or(3);
        }
        if let Ok(auto_disable) = env::var("LLM_AUTO_DISABLE_UNHEALTHY") {
            config.health.auto_disable_unhealthy = auto_disable.parse().unwrap_or(false);
        }

        config
    }

    /// Get enabled providers in priority order
    pub fn get_enabled_providers(&self) -> Vec<(&str, u32)> {
        let mut providers = Vec::new();

        if self.ollama.enabled {
            providers.push(("ollama", self.ollama.priority));
        }
        if self.openai.enabled {
            providers.push(("openai", self.openai.priority));
        }
        if self.anthropic.enabled {
            providers.push(("anthropic", self.anthropic.priority));
        }

        // Sort by priority (lower number = higher priority)
        providers.sort_by_key(|(_, priority)| *priority);
        providers
    }

    /// Check if Ollama should be the primary provider
    pub fn is_ollama_primary(&self) -> bool {
        self.ollama.enabled && self.ollama.priority == 1
    }

    /// Check if fallback providers are available
    pub fn has_fallback_providers(&self) -> bool {
        self.selection.enable_fallback && (self.openai.enabled || self.anthropic.enabled)
    }

    /// Get provider selection strategy summary
    pub fn get_selection_summary(&self) -> String {
        let enabled_providers = self.get_enabled_providers();
        let primary = enabled_providers
            .first()
            .map(|(name, _)| *name)
            .unwrap_or("none");

        format!(
            "Primary: {}, Fallback: {}, Local preference: {}",
            primary,
            if self.has_fallback_providers() {
                "enabled"
            } else {
                "disabled"
            },
            if self.selection.prefer_local {
                "yes"
            } else {
                "no"
            }
        )
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Check that at least one provider is enabled
        if !self.ollama.enabled && !self.openai.enabled && !self.anthropic.enabled {
            return Err(LoxoneError::config(
                "At least one LLM provider must be enabled".to_string(),
            ));
        }

        // Check that priority weights sum to reasonable values
        let total_priority = self.selection.cost_priority
            + self.selection.speed_priority
            + self.selection.quality_priority;
        if total_priority <= 0.0 || total_priority > 3.0 {
            return Err(LoxoneError::config(
                "Provider selection priority weights must be positive and reasonable (sum should be around 1.0)".to_string(),
            ));
        }

        // Validate URLs
        if self.ollama.enabled && !self.ollama.base_url.starts_with("http") {
            return Err(LoxoneError::config(
                "Ollama base URL must be a valid HTTP/HTTPS URL".to_string(),
            ));
        }

        // Check that cloud providers have API keys if enabled
        if self.openai.enabled && self.openai.api_key.is_none() {
            return Err(LoxoneError::config(
                "OpenAI provider is enabled but no API key provided".to_string(),
            ));
        }

        if self.anthropic.enabled && self.anthropic.api_key.is_none() {
            return Err(LoxoneError::config(
                "Anthropic provider is enabled but no API key provided".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config_validation() {
        let config = LlmConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config = LlmConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: LlmConfig = toml::from_str(&serialized).unwrap();
        assert!(deserialized.validate().is_ok());
    }

    #[test]
    fn test_config_manager() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_path = temp_file.path().to_path_buf();

        let manager = LlmConfigManager::new(config_path).unwrap();
        assert!(manager.get_config().validate().is_ok());
    }

    #[test]
    fn test_model_selection() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_path = temp_file.path().to_path_buf();

        let manager = LlmConfigManager::new(config_path).unwrap();
        let model = manager.get_best_model("general", 1000);
        assert!(model.is_some());
    }

    #[test]
    fn test_provider_factory_config_default() {
        let config = ProviderFactoryConfig::default();
        assert!(config.ollama.enabled);
        assert_eq!(config.ollama.priority, 1); // Ollama should be primary
        assert!(!config.openai.enabled); // Disabled by default
        assert!(!config.anthropic.enabled); // Disabled by default
        assert!(config.selection.enable_fallback);
        assert!(config.selection.prefer_local);
    }

    #[test]
    fn test_provider_factory_config_validation() {
        let config = ProviderFactoryConfig::default();
        assert!(config.validate().is_ok());

        // Test invalid configuration - no providers enabled
        let mut invalid_config = config.clone();
        invalid_config.ollama.enabled = false;
        invalid_config.openai.enabled = false;
        invalid_config.anthropic.enabled = false;
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_provider_priority_ordering() {
        let config = ProviderFactoryConfig::default();
        let providers = config.get_enabled_providers();

        // Only Ollama should be enabled by default
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].0, "ollama");
        assert_eq!(providers[0].1, 1); // Priority 1
    }

    #[test]
    fn test_provider_selection_summary() {
        let config = ProviderFactoryConfig::default();
        let summary = config.get_selection_summary();
        assert!(summary.contains("Primary: ollama"));
        assert!(summary.contains("Fallback: disabled")); // No cloud providers enabled
        assert!(summary.contains("Local preference: yes"));
    }

    #[test]
    fn test_provider_factory_config_from_env() {
        // Set test environment variables
        env::set_var("OLLAMA_BASE_URL", "http://test:11434");
        env::set_var("OLLAMA_DEFAULT_MODEL", "test-model");
        env::set_var("OPENAI_API_KEY", "test-openai-key");
        env::set_var("ANTHROPIC_API_KEY", "test-anthropic-key");
        env::set_var("LLM_ENABLE_FALLBACK", "true");

        let config = ProviderFactoryConfig::from_env();

        assert_eq!(config.ollama.base_url, "http://test:11434");
        assert_eq!(config.ollama.default_model, "test-model");
        assert!(config.openai.enabled); // Should be enabled when API key is provided
        assert!(config.anthropic.enabled); // Should be enabled when API key is provided
        assert_eq!(config.openai.api_key.unwrap(), "test-openai-key");
        assert_eq!(config.anthropic.api_key.unwrap(), "test-anthropic-key");

        // Clean up environment variables
        env::remove_var("OLLAMA_BASE_URL");
        env::remove_var("OLLAMA_DEFAULT_MODEL");
        env::remove_var("OPENAI_API_KEY");
        env::remove_var("ANTHROPIC_API_KEY");
        env::remove_var("LLM_ENABLE_FALLBACK");
    }
}
