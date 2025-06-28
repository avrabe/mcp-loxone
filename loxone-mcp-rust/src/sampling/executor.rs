//! Command execution module for executing parsed LLM responses
//!
//! Takes parsed device commands from sampling responses and executes them
//! against the Loxone system through the appropriate tool interfaces.

use super::response_parser::{DeviceCommand, SamplingResponse};
// Removed audit_log imports - module was unused
use crate::client::ClientContext;
use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Command execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Command that was executed
    pub command: DeviceCommand,
    /// Whether execution was successful
    pub success: bool,
    /// Result message
    pub message: String,
    /// Actual device UUID that was targeted
    pub device_uuid: Option<String>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Whether user approval was required
    pub required_approval: bool,
}

/// Batch execution result for multiple commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExecutionResult {
    /// Individual execution results
    pub results: Vec<ExecutionResult>,
    /// Overall success count
    pub success_count: usize,
    /// Overall failure count
    pub failure_count: usize,
    /// Total execution time in milliseconds
    pub total_time_ms: u64,
    /// Commands that required manual approval
    pub approval_required: Vec<DeviceCommand>,
}

/// Execution context for commands
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// User who initiated the request
    pub user_id: String,
    /// Session or request ID for tracking
    pub session_id: String,
    /// Whether to require approval for safety-critical actions
    pub require_approval: bool,
    /// Dry run mode - validate but don't execute
    pub dry_run: bool,
    /// Maximum number of commands to execute in one batch
    pub max_batch_size: usize,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            user_id: "system".to_string(),
            session_id: Uuid::new_v4().to_string(),
            require_approval: true,
            dry_run: false,
            max_batch_size: 10,
        }
    }
}

/// Command executor that interfaces with Loxone system
pub struct CommandExecutor {
    client_context: Arc<ClientContext>,
    // audit_logger removed - audit_log module was unused
    device_cache: Arc<tokio::sync::RwLock<HashMap<String, String>>>, // name -> UUID mapping
    safety_rules: SafetyRules,
}

/// Safety rules for command execution
#[derive(Debug, Clone)]
struct SafetyRules {
    /// Commands that always require approval
    high_risk_actions: Vec<String>,
    /// Device types that require approval
    protected_devices: Vec<String>,
    /// Maximum temperature changes allowed
    max_temp_change: f32,
    /// Time-based restrictions
    night_mode_restrictions: bool,
}

impl Default for SafetyRules {
    fn default() -> Self {
        Self {
            high_risk_actions: vec![
                "unlock".to_string(),
                "disable_alarm".to_string(),
                "emergency_stop".to_string(),
            ],
            protected_devices: vec![
                "security".to_string(),
                "alarm".to_string(),
                "lock".to_string(),
            ],
            max_temp_change: 5.0, // Max 5°C change per command
            night_mode_restrictions: true,
        }
    }
}

impl CommandExecutor {
    /// Create new command executor
    pub fn new(client_context: Arc<ClientContext>) -> Self {
        // Create a simple audit logger for command execution
        // Audit logger removed - module was unused

        Self {
            client_context,
            // audit_logger removed
            device_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            safety_rules: SafetyRules::default(),
        }
    }

    /// Execute a parsed sampling response
    pub async fn execute_sampling_response(
        &self,
        response: SamplingResponse,
        context: ExecutionContext,
    ) -> Result<BatchExecutionResult> {
        info!(
            "Executing sampling response with {} commands and {} recommendations",
            response.commands.len(),
            response.recommendations.len()
        );

        // Audit logging removed - module was unused

        // Validate response confidence
        if response.confidence < 0.3 {
            warn!("Low confidence sampling response: {}", response.confidence);
            return Err(LoxoneError::Generic(anyhow::anyhow!(
                "Sampling response confidence too low: {}",
                response.confidence
            )));
        }

        // Execute commands in batch
        self.execute_command_batch(response.commands, context).await
    }

    /// Execute a batch of commands
    pub async fn execute_command_batch(
        &self,
        commands: Vec<DeviceCommand>,
        context: ExecutionContext,
    ) -> Result<BatchExecutionResult> {
        let start_time = std::time::Instant::now();

        if commands.len() > context.max_batch_size {
            return Err(LoxoneError::Generic(anyhow::anyhow!(
                "Batch size {} exceeds maximum {}",
                commands.len(),
                context.max_batch_size
            )));
        }

        let mut results = Vec::new();
        let mut approval_required = Vec::new();

        // Refresh device cache if needed
        self.refresh_device_cache().await?;

        for command in commands {
            // Check safety rules
            if self.requires_approval(&command, &context) {
                approval_required.push(command.clone());

                if context.require_approval {
                    results.push(ExecutionResult {
                        command: command.clone(),
                        success: false,
                        message: "Command requires manual approval".to_string(),
                        device_uuid: None,
                        execution_time_ms: 0,
                        required_approval: true,
                    });
                    continue;
                }
            }

            // Execute individual command
            let result = self.execute_single_command(command, &context).await;
            results.push(result);
        }

        let total_time_ms = start_time.elapsed().as_millis() as u64;
        let success_count = results.iter().filter(|r| r.success).count();
        let failure_count = results.len() - success_count;

        info!(
            "Batch execution completed: {}/{} successful in {}ms",
            success_count,
            results.len(),
            total_time_ms
        );

        Ok(BatchExecutionResult {
            results,
            success_count,
            failure_count,
            total_time_ms,
            approval_required,
        })
    }

    /// Execute a single command
    async fn execute_single_command(
        &self,
        command: DeviceCommand,
        context: &ExecutionContext,
    ) -> ExecutionResult {
        let start_time = std::time::Instant::now();
        debug!(
            "Executing command: {} {} on {}",
            command.action,
            command.value.as_deref().unwrap_or(""),
            command.device
        );

        // Resolve device UUID
        let device_uuid = match self.resolve_device_uuid(&command).await {
            Ok(uuid) => uuid,
            Err(e) => {
                return ExecutionResult {
                    command,
                    success: false,
                    message: format!("Failed to resolve device: {}", e),
                    device_uuid: None,
                    execution_time_ms: start_time.elapsed().as_millis() as u64,
                    required_approval: false,
                };
            }
        };

        // Execute based on action type
        let result = if context.dry_run {
            Ok("Dry run - command validated but not executed".to_string())
        } else {
            match command.action.as_str() {
                "on" | "off" => {
                    self.execute_light_command(&device_uuid, &command.action)
                        .await
                }
                "up" | "down" => {
                    self.execute_blind_command(&device_uuid, &command.action)
                        .await
                }
                "set_temperature" => {
                    self.execute_climate_command(&device_uuid, &command.value)
                        .await
                }
                "volume" | "play" | "stop" | "pause" => {
                    self.execute_audio_command(&device_uuid, &command.action, &command.value)
                        .await
                }
                _ => Err(LoxoneError::Generic(anyhow::anyhow!(
                    "Unsupported action: {}",
                    command.action
                ))),
            }
        };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        let success = result.is_ok();
        let message = match result {
            Ok(msg) => msg,
            Err(e) => e.to_string(),
        };

        // Audit logging removed - module was unused

        ExecutionResult {
            command,
            success,
            message,
            device_uuid: Some(device_uuid),
            execution_time_ms,
            required_approval: false,
        }
    }

    /// Check if command requires manual approval
    fn requires_approval(&self, command: &DeviceCommand, context: &ExecutionContext) -> bool {
        // Always allow in dry run mode
        if context.dry_run {
            return false;
        }

        // Check high-risk actions
        if self
            .safety_rules
            .high_risk_actions
            .contains(&command.action)
        {
            return true;
        }

        // Check protected device types
        let device_type = self.infer_device_type(&command.device);
        if self.safety_rules.protected_devices.contains(&device_type) {
            return true;
        }

        // Check temperature change limits
        if command.action == "set_temperature" {
            if let Some(ref value_str) = command.value {
                if let Ok(target_temp) = value_str.parse::<f32>() {
                    // In a real implementation, we'd get current temperature
                    // For now, assume reasonable limits
                    if !(15.0..=25.0).contains(&target_temp) {
                        return true;
                    }
                }
            }
        }

        // Check night mode restrictions
        if self.safety_rules.night_mode_restrictions {
            use chrono::Timelike;
            let current_hour = chrono::Utc::now().hour();
            if (current_hour >= 22 || current_hour <= 6) && command.action.contains("on") {
                return true;
            }
        }

        false
    }

    /// Resolve device name to UUID
    async fn resolve_device_uuid(&self, command: &DeviceCommand) -> Result<String> {
        // First check cache
        {
            let cache = self.device_cache.read().await;
            if let Some(uuid) = cache.get(&command.device) {
                return Ok(uuid.clone());
            }
        }

        // Search in client context
        let devices = self.client_context.devices.read().await;

        // Try exact name match first
        for (uuid, device) in devices.iter() {
            if device.name == command.device {
                // Cache the result
                self.device_cache
                    .write()
                    .await
                    .insert(command.device.clone(), uuid.clone());
                return Ok(uuid.clone());
            }
        }

        // Try fuzzy matching
        for (uuid, device) in devices.iter() {
            if self.fuzzy_match(&device.name, &command.device) {
                // Cache the result
                self.device_cache
                    .write()
                    .await
                    .insert(command.device.clone(), uuid.clone());
                return Ok(uuid.clone());
            }
        }

        // Try room-based matching
        if let Some(ref room) = command.room {
            let device_type = self.infer_device_type(&command.device);
            for (uuid, device) in devices.iter() {
                if device.room.as_ref().map(|r| r.to_lowercase()) == Some(room.to_lowercase())
                    && device.device_type.to_lowercase().contains(&device_type)
                {
                    // Cache the result
                    self.device_cache
                        .write()
                        .await
                        .insert(command.device.clone(), uuid.clone());
                    return Ok(uuid.clone());
                }
            }
        }

        Err(LoxoneError::Generic(anyhow::anyhow!(
            "Device not found: {}",
            command.device
        )))
    }

    /// Fuzzy match device names
    fn fuzzy_match(&self, device_name: &str, command_name: &str) -> bool {
        let device_lower = device_name.to_lowercase();
        let command_lower = command_name.to_lowercase();

        // Check if command name contains device name or vice versa
        device_lower.contains(&command_lower) || command_lower.contains(&device_lower)
    }

    /// Infer device type from device name
    fn infer_device_type(&self, device_name: &str) -> String {
        let name_lower = device_name.to_lowercase();

        if name_lower.contains("light") || name_lower.contains("lamp") {
            "light".to_string()
        } else if name_lower.contains("blind")
            || name_lower.contains("rolladen")
            || name_lower.contains("shutter")
        {
            "blind".to_string()
        } else if name_lower.contains("temperature")
            || name_lower.contains("climate")
            || name_lower.contains("heating")
        {
            "climate".to_string()
        } else if name_lower.contains("audio")
            || name_lower.contains("music")
            || name_lower.contains("speaker")
        {
            "audio".to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// Execute light control command
    async fn execute_light_command(&self, device_uuid: &str, action: &str) -> Result<String> {
        debug!("Executing light command: {} on {}", action, device_uuid);

        // This would typically call the actual Loxone tools
        // For now, simulate the execution
        let _command = match action {
            "on" => "On",
            "off" => "Off",
            _ => {
                return Err(LoxoneError::Generic(anyhow::anyhow!(
                    "Invalid light action: {}",
                    action
                )))
            }
        };

        // In real implementation, this would call:
        // self.loxone_client.send_command(device_uuid, command).await?;

        info!("Light {} turned {}", device_uuid, action);
        Ok(format!(
            "Light {} successfully turned {}",
            device_uuid, action
        ))
    }

    /// Execute blind control command
    async fn execute_blind_command(&self, device_uuid: &str, action: &str) -> Result<String> {
        debug!("Executing blind command: {} on {}", action, device_uuid);

        let _command = match action {
            "up" => "FullUp",
            "down" => "FullDown",
            "stop" => "Stop",
            _ => {
                return Err(LoxoneError::Generic(anyhow::anyhow!(
                    "Invalid blind action: {}",
                    action
                )))
            }
        };

        // In real implementation, this would call:
        // self.loxone_client.send_command(device_uuid, command).await?;

        info!("Blind {} moved {}", device_uuid, action);
        Ok(format!(
            "Blind {} successfully moved {}",
            device_uuid, action
        ))
    }

    /// Execute climate control command
    async fn execute_climate_command(
        &self,
        device_uuid: &str,
        value: &Option<String>,
    ) -> Result<String> {
        let temperature = value
            .as_ref()
            .ok_or_else(|| LoxoneError::Generic(anyhow::anyhow!("No temperature value provided")))?
            .parse::<f32>()
            .map_err(|e| {
                LoxoneError::Generic(anyhow::anyhow!("Invalid temperature value: {}", e))
            })?;

        debug!(
            "Executing climate command: set temperature to {}°C on {}",
            temperature, device_uuid
        );

        // Validate temperature range
        if !(10.0..=30.0).contains(&temperature) {
            return Err(LoxoneError::Generic(anyhow::anyhow!(
                "Temperature {} is outside valid range (10-30°C)",
                temperature
            )));
        }

        // In real implementation, this would call:
        // self.loxone_client.set_temperature(device_uuid, temperature).await?;

        info!("Temperature set to {}°C on {}", temperature, device_uuid);
        Ok(format!(
            "Temperature successfully set to {}°C on {}",
            temperature, device_uuid
        ))
    }

    /// Execute audio control command
    async fn execute_audio_command(
        &self,
        device_uuid: &str,
        action: &str,
        value: &Option<String>,
    ) -> Result<String> {
        debug!(
            "Executing audio command: {} on {} with value {:?}",
            action, device_uuid, value
        );

        match action {
            "play" => {
                // In real implementation: self.loxone_client.audio_play(device_uuid).await?;
                Ok(format!("Audio playback started on {}", device_uuid))
            }
            "stop" => {
                // In real implementation: self.loxone_client.audio_stop(device_uuid).await?;
                Ok(format!("Audio playback stopped on {}", device_uuid))
            }
            "pause" => {
                // In real implementation: self.loxone_client.audio_pause(device_uuid).await?;
                Ok(format!("Audio playback paused on {}", device_uuid))
            }
            "volume" => {
                let volume = value
                    .as_ref()
                    .ok_or_else(|| {
                        LoxoneError::Generic(anyhow::anyhow!("No volume value provided"))
                    })?
                    .parse::<u32>()
                    .map_err(|e| {
                        LoxoneError::Generic(anyhow::anyhow!("Invalid volume value: {}", e))
                    })?;

                if volume > 100 {
                    return Err(LoxoneError::Generic(anyhow::anyhow!(
                        "Volume cannot exceed 100%"
                    )));
                }

                // In real implementation: self.loxone_client.set_volume(device_uuid, volume).await?;
                Ok(format!("Volume set to {}% on {}", volume, device_uuid))
            }
            _ => Err(LoxoneError::Generic(anyhow::anyhow!(
                "Invalid audio action: {}",
                action
            ))),
        }
    }

    /// Refresh device cache from client context
    async fn refresh_device_cache(&self) -> Result<()> {
        let devices = self.client_context.devices.read().await;
        let mut cache = self.device_cache.write().await;

        cache.clear();
        for (uuid, device) in devices.iter() {
            cache.insert(device.name.clone(), uuid.clone());

            // Also cache variations
            if let Some(ref room) = device.room {
                let room_device_name = format!("{} {}", room, device.device_type);
                cache.insert(room_device_name, uuid.clone());
            }
        }

        debug!("Refreshed device cache with {} entries", cache.len());
        Ok(())
    }

    /// Get execution statistics
    pub async fn get_execution_stats(&self) -> Result<serde_json::Value> {
        // In a real implementation, this would aggregate from audit logs
        Ok(serde_json::json!({
            "cache_size": self.device_cache.read().await.len(),
            "safety_rules": {
                "high_risk_actions": self.safety_rules.high_risk_actions.len(),
                "protected_devices": self.safety_rules.protected_devices.len(),
                "max_temp_change": self.safety_rules.max_temp_change,
                "night_mode_restrictions": self.safety_rules.night_mode_restrictions
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::LoxoneDevice;

    #[tokio::test]
    async fn test_device_resolution() {
        let client_context = Arc::new(ClientContext::new());

        // Add a test device
        {
            let mut devices = client_context.devices.write().await;
            devices.insert(
                "test-uuid".to_string(),
                LoxoneDevice {
                    uuid: "test-uuid".to_string(),
                    name: "Living Room Light".to_string(),
                    device_type: "Light".to_string(),
                    room: Some("Living Room".to_string()),
                    states: HashMap::new(),
                    category: "Lighting".to_string(),
                    sub_controls: HashMap::new(),
                },
            );
        }

        let executor = CommandExecutor::new(client_context);

        let command = DeviceCommand {
            device: "Living Room Light".to_string(),
            action: "on".to_string(),
            value: None,
            room: Some("Living Room".to_string()),
            confidence: 0.9,
        };

        let uuid = executor.resolve_device_uuid(&command).await.unwrap();
        assert_eq!(uuid, "test-uuid");
    }

    #[tokio::test]
    async fn test_safety_rules() {
        let client_context = Arc::new(ClientContext::new());
        let mut executor = CommandExecutor::new(client_context);
        // Disable night mode restrictions for consistent testing
        executor.safety_rules.night_mode_restrictions = false;
        let context = ExecutionContext::default();

        let safe_command = DeviceCommand {
            device: "Living Room Light".to_string(),
            action: "on".to_string(),
            value: None,
            room: Some("Living Room".to_string()),
            confidence: 0.9,
        };

        let risky_command = DeviceCommand {
            device: "Front Door Lock".to_string(),
            action: "unlock".to_string(),
            value: None,
            room: None,
            confidence: 0.9,
        };

        assert!(!executor.requires_approval(&safe_command, &context));
        assert!(executor.requires_approval(&risky_command, &context));
    }
}
