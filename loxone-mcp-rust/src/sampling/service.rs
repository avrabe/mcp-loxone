//! Integrated sampling service that handles the complete flow from request to execution
//!
//! This service coordinates the MCP sampling protocol, response parsing, and command execution
//! to provide a complete LLM-powered home automation solution.

use super::client::{SamplingCapabilities, SamplingClient};
use super::executor::{BatchExecutionResult, CommandExecutor, ExecutionContext};
use super::response_parser::{CommandExtractor, SamplingResponse as ParsedResponse};
use super::{AutomationSamplingBuilder, SamplingMessage, SamplingRequest};
use crate::audit_log::{
    AuditConfig, AuditEntry, AuditEventType, AuditLogger, AuditOutput, AuditSeverity,
};
use crate::client::ClientContext;
use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Complete sampling service result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingServiceResult {
    /// Original request metadata
    pub request_id: String,
    /// LLM response from sampling
    pub llm_response: String,
    /// Parsed commands and recommendations
    pub parsed_response: ParsedResponse,
    /// Execution results
    pub execution_result: BatchExecutionResult,
    /// Total processing time
    pub total_time_ms: u64,
    /// Service metrics
    pub metrics: SamplingMetrics,
}

/// Service performance and accuracy metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingMetrics {
    /// Time spent on LLM sampling
    pub sampling_time_ms: u64,
    /// Time spent on response parsing
    pub parsing_time_ms: u64,
    /// Time spent on command execution
    pub execution_time_ms: u64,
    /// LLM response confidence
    pub llm_confidence: f32,
    /// Number of commands extracted
    pub commands_extracted: usize,
    /// Number of recommendations extracted
    pub recommendations_extracted: usize,
    /// Command success rate
    pub execution_success_rate: f32,
}

/// Sampling service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingServiceConfig {
    /// Enable automatic execution of high-confidence commands
    pub auto_execute_threshold: f32,
    /// Maximum commands to execute in one session
    pub max_commands_per_session: usize,
    /// Enable safety checks
    pub enable_safety_checks: bool,
    /// Require human approval for certain actions
    pub require_human_approval: bool,
    /// Cache parsed responses to avoid re-parsing
    pub enable_response_caching: bool,
    /// Timeout for LLM sampling requests (seconds)
    pub sampling_timeout_seconds: u32,
}

impl Default for SamplingServiceConfig {
    fn default() -> Self {
        Self {
            auto_execute_threshold: 0.7,
            max_commands_per_session: 5,
            enable_safety_checks: true,
            require_human_approval: true,
            enable_response_caching: true,
            sampling_timeout_seconds: 30,
        }
    }
}

/// Comprehensive sampling service
pub struct SamplingService {
    client: Arc<dyn SamplingClient>,
    client_context: Arc<ClientContext>,
    command_extractor: CommandExtractor,
    command_executor: CommandExecutor,
    audit_logger: AuditLogger,
    config: SamplingServiceConfig,
    response_cache: Arc<tokio::sync::RwLock<std::collections::HashMap<String, ParsedResponse>>>,
}

impl SamplingService {
    /// Create new sampling service
    pub fn new(
        client: Arc<dyn SamplingClient>,
        client_context: Arc<ClientContext>,
        config: SamplingServiceConfig,
    ) -> Self {
        // Create audit logger
        let audit_config = AuditConfig::default();
        let audit_output = AuditOutput::Stdout;
        let audit_logger = AuditLogger::new(audit_config, audit_output);

        Self {
            client,
            client_context: client_context.clone(),
            command_extractor: CommandExtractor::default(),
            command_executor: CommandExecutor::new(client_context),
            audit_logger,
            config,
            response_cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Process a complete sampling request from user input to execution
    pub async fn process_automation_request(
        &self,
        user_input: String,
        context: ExecutionContext,
    ) -> Result<SamplingServiceResult> {
        let request_id = Uuid::new_v4().to_string();
        let start_time = Instant::now();

        info!(
            "Processing automation request: {} (ID: {})",
            user_input, request_id
        );

        // Build context-aware sampling request
        let sampling_request = self.build_contextual_request(&user_input).await?;

        // Execute the full pipeline
        let result = self
            .execute_pipeline(sampling_request, context, request_id.clone())
            .await?;

        let total_time_ms = start_time.elapsed().as_millis() as u64;
        info!(
            "Completed automation request {} in {}ms",
            request_id, total_time_ms
        );

        Ok(SamplingServiceResult {
            request_id,
            llm_response: result.0,
            parsed_response: result.1,
            execution_result: result.2,
            total_time_ms,
            metrics: result.3,
        })
    }

    /// Process a cozy home scenario
    pub async fn create_cozy_atmosphere(
        &self,
        time_of_day: String,
        weather: String,
        mood: String,
        context: ExecutionContext,
    ) -> Result<SamplingServiceResult> {
        let request_id = Uuid::new_v4().to_string();
        info!(
            "Creating cozy atmosphere: {} {} {}",
            time_of_day, weather, mood
        );

        // Build specialized cozy request
        let builder = self.create_automation_builder().await?;
        let sampling_request = builder.build_cozy_request(&time_of_day, &weather, &mood)?;

        // Execute pipeline
        let result = self
            .execute_pipeline(sampling_request, context, request_id.clone())
            .await?;

        Ok(SamplingServiceResult {
            request_id,
            llm_response: result.0,
            parsed_response: result.1,
            execution_result: result.2,
            total_time_ms: result.3.sampling_time_ms
                + result.3.parsing_time_ms
                + result.3.execution_time_ms,
            metrics: result.3,
        })
    }

    /// Process event preparation scenario
    pub async fn prepare_for_event(
        &self,
        event_type: String,
        room: Option<String>,
        duration: Option<String>,
        guest_count: Option<String>,
        context: ExecutionContext,
    ) -> Result<SamplingServiceResult> {
        let request_id = Uuid::new_v4().to_string();
        info!("Preparing for event: {} in {:?}", event_type, room);

        // Build specialized event request
        let builder = self.create_automation_builder().await?;
        let sampling_request = builder.build_event_request(
            &event_type,
            room.as_deref(),
            duration.as_deref(),
            guest_count.as_deref(),
        )?;

        // Execute pipeline
        let result = self
            .execute_pipeline(sampling_request, context, request_id.clone())
            .await?;

        Ok(SamplingServiceResult {
            request_id,
            llm_response: result.0,
            parsed_response: result.1,
            execution_result: result.2,
            total_time_ms: result.3.sampling_time_ms
                + result.3.parsing_time_ms
                + result.3.execution_time_ms,
            metrics: result.3,
        })
    }

    /// Execute the complete pipeline: sample -> parse -> execute
    async fn execute_pipeline(
        &self,
        sampling_request: SamplingRequest,
        context: ExecutionContext,
        request_id: String,
    ) -> Result<(
        String,
        ParsedResponse,
        BatchExecutionResult,
        SamplingMetrics,
    )> {
        // Step 1: LLM Sampling
        let sampling_start = Instant::now();
        let llm_response = self.execute_sampling(sampling_request.clone()).await?;
        let sampling_time_ms = sampling_start.elapsed().as_millis() as u64;

        // Step 2: Response Parsing
        let parsing_start = Instant::now();
        let parsed_response = self.parse_response(llm_response.clone()).await?;
        let parsing_time_ms = parsing_start.elapsed().as_millis() as u64;

        // Step 3: Command Execution
        let execution_start = Instant::now();
        let execution_result = self
            .execute_commands(parsed_response.clone(), context)
            .await?;
        let execution_time_ms = execution_start.elapsed().as_millis() as u64;

        // Calculate metrics
        let metrics = SamplingMetrics {
            sampling_time_ms,
            parsing_time_ms,
            execution_time_ms,
            llm_confidence: parsed_response.confidence,
            commands_extracted: parsed_response.commands.len(),
            recommendations_extracted: parsed_response.recommendations.len(),
            execution_success_rate: if execution_result.results.is_empty() {
                0.0
            } else {
                execution_result.success_count as f32 / execution_result.results.len() as f32
            },
        };

        // Audit the complete pipeline
        let audit_entry = AuditEntry::new(
            AuditSeverity::Info,
            AuditEventType::SystemLifecycle {
                event: "automation_pipeline".to_string(),
                details: std::collections::HashMap::from([
                    (
                        "success_count".to_string(),
                        execution_result.success_count.to_string(),
                    ),
                    (
                        "total_commands".to_string(),
                        execution_result.results.len().to_string(),
                    ),
                    ("request_id".to_string(), request_id.clone()),
                ]),
            },
        );
        if let Err(e) = self.audit_logger.log(audit_entry).await {
            warn!("Failed to audit pipeline execution: {}", e);
        }

        Ok((llm_response, parsed_response, execution_result, metrics))
    }

    /// Execute LLM sampling
    async fn execute_sampling(&self, request: SamplingRequest) -> Result<String> {
        debug!(
            "Executing LLM sampling with {} messages",
            request.messages.len()
        );

        // Check if client supports sampling
        if !self.client.is_sampling_supported() {
            return Err(LoxoneError::Generic(anyhow::anyhow!(
                "MCP client does not support sampling protocol"
            )));
        }

        // Execute sampling request
        let response = self.client.create_message(request).await?;

        // Extract text content from response
        match response.content.text {
            Some(text) => {
                debug!("Received LLM response: {} characters", text.len());
                Ok(text)
            }
            None => Err(LoxoneError::Generic(anyhow::anyhow!(
                "LLM response contained no text content"
            ))),
        }
    }

    /// Parse LLM response into commands and recommendations
    async fn parse_response(&self, content: String) -> Result<ParsedResponse> {
        debug!("Parsing LLM response");

        // Check cache first
        if self.config.enable_response_caching {
            let cache_key = format!("{:x}", md5::compute(&content));
            let cache = self.response_cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                debug!("Using cached parsed response");
                return Ok(cached.clone());
            }
        }

        // Parse the response
        let parsed = self.command_extractor.parse_response(content.clone())?;

        // Cache the result
        if self.config.enable_response_caching {
            let cache_key = format!("{:x}", md5::compute(&content));
            self.response_cache
                .write()
                .await
                .insert(cache_key, parsed.clone());
        }

        debug!(
            "Parsed response: {} commands, {} recommendations, confidence: {}",
            parsed.commands.len(),
            parsed.recommendations.len(),
            parsed.confidence
        );

        Ok(parsed)
    }

    /// Execute parsed commands
    async fn execute_commands(
        &self,
        parsed_response: ParsedResponse,
        context: ExecutionContext,
    ) -> Result<BatchExecutionResult> {
        debug!("Executing {} commands", parsed_response.commands.len());

        // Apply configuration limits
        let commands = if parsed_response.commands.len() > self.config.max_commands_per_session {
            warn!(
                "Limiting commands from {} to {}",
                parsed_response.commands.len(),
                self.config.max_commands_per_session
            );
            parsed_response
                .commands
                .into_iter()
                .take(self.config.max_commands_per_session)
                .collect()
        } else {
            parsed_response.commands
        };

        // Check confidence threshold for auto-execution
        if parsed_response.confidence < self.config.auto_execute_threshold {
            warn!(
                "Response confidence {} below auto-execute threshold {}",
                parsed_response.confidence, self.config.auto_execute_threshold
            );

            if self.config.require_human_approval {
                return Ok(BatchExecutionResult {
                    results: Vec::new(),
                    success_count: 0,
                    failure_count: 0,
                    total_time_ms: 0,
                    approval_required: commands,
                });
            }
        }

        // Execute commands
        self.command_executor
            .execute_command_batch(commands, context)
            .await
    }

    /// Build context-aware sampling request
    async fn build_contextual_request(&self, user_input: &str) -> Result<SamplingRequest> {
        let builder = self.create_automation_builder().await?;

        let user_message = SamplingMessage::user(format!(
            "{}\n\nCurrent Home State:\n{}",
            user_input,
            builder.build_context_text().unwrap_or_default()
        ));

        Ok(SamplingRequest::new(vec![user_message])
            .with_system_prompt(builder.system_prompt.clone())
            .with_max_tokens(800)
            .with_temperature(0.7)
            .with_metadata(
                "request_type".to_string(),
                serde_json::Value::String("general".to_string()),
            ))
    }

    /// Create automation builder with current system state
    async fn create_automation_builder(&self) -> Result<AutomationSamplingBuilder> {
        let mut builder = AutomationSamplingBuilder::new();

        // Add rooms data
        let rooms = self.client_context.rooms.read().await;
        if !rooms.is_empty() {
            builder = builder.with_rooms(serde_json::to_value(&*rooms)?);
        }

        // Add devices data
        let devices = self.client_context.devices.read().await;
        if !devices.is_empty() {
            // Convert to more useful format for LLM
            let device_summary: Vec<_> = devices
                .values()
                .map(|device| {
                    serde_json::json!({
                        "name": device.name,
                        "type": device.device_type,
                        "room": device.room,
                        "uuid": device.uuid
                    })
                })
                .collect();
            builder = builder.with_devices(serde_json::to_value(device_summary)?);
        }

        // Add sensor data (if available from client context)
        // This would be expanded based on available sensor integration

        Ok(builder)
    }

    /// Get service capabilities
    pub fn get_capabilities(&self) -> SamplingServiceCapabilities {
        SamplingServiceCapabilities {
            supports_sampling: self.client.is_sampling_supported(),
            sampling_capabilities: self.client.get_sampling_capabilities(),
            max_commands_per_session: self.config.max_commands_per_session,
            auto_execute_threshold: self.config.auto_execute_threshold,
            safety_checks_enabled: self.config.enable_safety_checks,
            supported_scenarios: vec![
                "general_automation".to_string(),
                "cozy_atmosphere".to_string(),
                "event_preparation".to_string(),
                "energy_optimization".to_string(),
                "security_setup".to_string(),
            ],
        }
    }

    /// Get service statistics
    pub async fn get_service_stats(&self) -> Result<serde_json::Value> {
        let cache_size = self.response_cache.read().await.len();
        let executor_stats = self.command_executor.get_execution_stats().await?;

        Ok(serde_json::json!({
            "config": self.config,
            "response_cache_size": cache_size,
            "executor_stats": executor_stats,
            "client_capabilities": self.client.get_sampling_capabilities()
        }))
    }

    /// Clear response cache
    pub async fn clear_cache(&self) {
        self.response_cache.write().await.clear();
        info!("Cleared response cache");
    }
}

/// Service capabilities description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingServiceCapabilities {
    pub supports_sampling: bool,
    pub sampling_capabilities: SamplingCapabilities,
    pub max_commands_per_session: usize,
    pub auto_execute_threshold: f32,
    pub safety_checks_enabled: bool,
    pub supported_scenarios: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sampling::client::MockSamplingClient;

    #[tokio::test]
    async fn test_sampling_service_creation() {
        let client = Arc::new(MockSamplingClient::new(false));
        let client_context = Arc::new(ClientContext::new());
        let config = SamplingServiceConfig::default();

        let service = SamplingService::new(client, client_context, config);
        let capabilities = service.get_capabilities();

        assert!(capabilities
            .supported_scenarios
            .contains(&"general_automation".to_string()));
    }

    #[tokio::test]
    async fn test_contextual_request_building() {
        let client = Arc::new(MockSamplingClient::new(false));
        let client_context = Arc::new(ClientContext::new());
        let config = SamplingServiceConfig::default();

        let service = SamplingService::new(client, client_context, config);

        let request = service
            .build_contextual_request("Turn on the lights")
            .await
            .unwrap();
        assert!(!request.messages.is_empty());
        assert!(request.system_prompt.is_some());
    }
}
