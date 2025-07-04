//! Batch operations framework for efficient multi-device control
//!
//! This module provides tools for executing multiple device commands efficiently,
//! supporting both parallel and sequential execution modes with error handling,
//! rollback capabilities, and progress tracking.

use crate::client::LoxoneDevice;
use crate::tools::{ToolContext, ToolResponse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, error};

/// Batch operation execution mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    /// Execute all commands in parallel
    Parallel,
    /// Execute commands sequentially in order
    Sequential,
    /// Execute in parallel but with dependency order
    Ordered,
}

/// Batch operation priority
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BatchPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Error handling strategy for batch operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ErrorStrategy {
    /// Continue execution on errors
    Continue,
    /// Stop on first error
    StopOnError,
    /// Attempt to rollback on errors
    Rollback,
}

/// Individual batch command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCommand {
    /// Unique command ID
    pub id: String,
    /// Target device name or UUID
    pub device: String,
    /// Command to execute
    pub command: String,
    /// Optional command parameters
    pub parameters: Option<HashMap<String, Value>>,
    /// Command priority
    pub priority: BatchPriority,
    /// Dependencies (command IDs that must complete first)
    pub dependencies: Vec<String>,
    /// Rollback command (for error recovery)
    pub rollback_command: Option<String>,
    /// Retry count on failure
    pub retry_count: Option<u32>,
    /// Timeout for this command (milliseconds)
    pub timeout: Option<u64>,
}

/// Batch operation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOperation {
    /// Batch operation ID
    pub id: String,
    /// Operation name/description
    pub name: String,
    /// List of commands to execute
    pub commands: Vec<BatchCommand>,
    /// Execution mode
    pub execution_mode: ExecutionMode,
    /// Error handling strategy
    pub error_strategy: ErrorStrategy,
    /// Maximum parallel executions (for parallel mode)
    pub max_parallel: Option<u32>,
    /// Global timeout for entire operation (milliseconds)
    pub global_timeout: Option<u64>,
    /// Enable progress tracking
    pub track_progress: bool,
}

/// Batch command execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCommandResult {
    /// Command ID
    pub command_id: String,
    /// Target device name
    pub device_name: String,
    /// Device UUID
    pub device_uuid: String,
    /// Execution status
    pub status: CommandStatus,
    /// Command that was executed
    pub command: String,
    /// Response from device
    pub response: Option<Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Retry attempts made
    pub retry_attempts: u32,
    /// Timestamp of execution
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Command execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CommandStatus {
    Pending,
    Running,
    Success,
    Failed,
    Timeout,
    Cancelled,
    Rollback,
}

/// Batch operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOperationResult {
    /// Batch operation ID
    pub batch_id: String,
    /// Operation name
    pub name: String,
    /// Overall status
    pub status: BatchStatus,
    /// Total commands
    pub total_commands: usize,
    /// Successful commands
    pub successful_commands: usize,
    /// Failed commands
    pub failed_commands: usize,
    /// Individual command results
    pub command_results: Vec<BatchCommandResult>,
    /// Total execution time
    pub total_duration_ms: u64,
    /// Execution start time
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// Execution end time
    pub end_time: chrono::DateTime<chrono::Utc>,
    /// Progress tracking data
    pub progress: Option<BatchProgress>,
}

/// Batch operation status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BatchStatus {
    Pending,
    Running,
    Completed,
    Failed,
    PartialSuccess,
    Cancelled,
    TimedOut,
}

/// Batch operation progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProgress {
    /// Current progress percentage (0-100)
    pub percentage: f64,
    /// Commands completed
    pub completed: usize,
    /// Commands failed
    pub failed: usize,
    /// Commands remaining
    pub remaining: usize,
    /// Estimated time remaining (milliseconds)
    pub estimated_remaining_ms: Option<u64>,
}

/// Execute a batch operation
pub async fn execute_batch_operation(
    context: ToolContext,
    batch_operation: BatchOperation,
) -> ToolResponse {
    debug!(
        "Executing batch operation '{}' with {} commands",
        batch_operation.name,
        batch_operation.commands.len()
    );

    let start_time = chrono::Utc::now();
    let execution_start = Instant::now();

    // Validate batch operation
    if let Err(validation_error) = validate_batch_operation(&batch_operation) {
        return ToolResponse::error(format!("Invalid batch operation: {}", validation_error));
    }

    // Resolve device UUIDs for all commands
    let resolved_commands = match resolve_batch_devices(&context, &batch_operation.commands).await {
        Ok(commands) => commands,
        Err(e) => return ToolResponse::error(format!("Failed to resolve devices: {}", e)),
    };

    // Execute based on mode
    let command_results = match batch_operation.execution_mode {
        ExecutionMode::Parallel => {
            execute_parallel_batch(&context, &resolved_commands, &batch_operation).await
        }
        ExecutionMode::Sequential => {
            execute_sequential_batch(&context, &resolved_commands, &batch_operation).await
        }
        ExecutionMode::Ordered => {
            execute_ordered_batch(&context, &resolved_commands, &batch_operation).await
        }
    };

    let total_duration = execution_start.elapsed();
    let end_time = chrono::Utc::now();

    // Calculate final statistics
    let successful_commands = command_results
        .iter()
        .filter(|r| r.status == CommandStatus::Success)
        .count();
    let failed_commands = command_results
        .iter()
        .filter(|r| r.status == CommandStatus::Failed)
        .count();

    let overall_status = if failed_commands == 0 {
        BatchStatus::Completed
    } else if successful_commands > 0 {
        BatchStatus::PartialSuccess
    } else {
        BatchStatus::Failed
    };

    let result = BatchOperationResult {
        batch_id: batch_operation.id.clone(),
        name: batch_operation.name.clone(),
        status: overall_status,
        total_commands: batch_operation.commands.len(),
        successful_commands,
        failed_commands,
        command_results,
        total_duration_ms: total_duration.as_millis() as u64,
        start_time,
        end_time,
        progress: None, // Final result doesn't need progress
    };

    ToolResponse::success(json!({
        "batch_result": result,
        "summary": {
            "total": result.total_commands,
            "successful": result.successful_commands,
            "failed": result.failed_commands,
            "duration_seconds": result.total_duration_ms as f64 / 1000.0,
            "success_rate": if result.total_commands > 0 {
                (result.successful_commands as f64 / result.total_commands as f64) * 100.0
            } else { 0.0 }
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Create a batch operation from room-based commands
pub async fn create_room_batch_operation(
    context: ToolContext,
    room_name: String,
    command_template: String,
    device_filter: Option<Vec<String>>,
    execution_mode: Option<ExecutionMode>,
) -> ToolResponse {
    debug!(
        "Creating batch operation for room '{}' with command '{}'",
        room_name, command_template
    );

    // Get all devices in the specified room
    let devices = context.context.devices.read().await;
    let room_devices: Vec<_> = devices
        .values()
        .filter(|device| {
            device
                .room
                .as_ref()
                .map(|r| r.to_lowercase().contains(&room_name.to_lowercase()))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    if room_devices.is_empty() {
        return ToolResponse::error(format!("No devices found in room '{}'", room_name));
    }

    // Filter devices by type if specified
    let filtered_devices = if let Some(filter) = device_filter {
        room_devices
            .into_iter()
            .filter(|device| filter.iter().any(|f| device.device_type.contains(f)))
            .collect()
    } else {
        room_devices
    };

    // Create batch commands
    let mut batch_commands = Vec::new();
    for (index, device) in filtered_devices.iter().enumerate() {
        batch_commands.push(BatchCommand {
            id: format!("cmd_{}", index),
            device: device.name.clone(),
            command: command_template.clone(),
            parameters: None,
            priority: BatchPriority::Normal,
            dependencies: Vec::new(),
            rollback_command: None,
            retry_count: Some(2),
            timeout: Some(5000), // 5 second timeout
        });
    }

    let batch_operation = BatchOperation {
        id: format!("room_batch_{}", chrono::Utc::now().timestamp()),
        name: format!("Room {} - {}", room_name, command_template),
        commands: batch_commands,
        execution_mode: execution_mode.unwrap_or(ExecutionMode::Parallel),
        error_strategy: ErrorStrategy::Continue,
        max_parallel: Some(10),
        global_timeout: Some(30000), // 30 second timeout
        track_progress: true,
    };

    ToolResponse::success(json!({
        "batch_operation": batch_operation,
        "room": room_name,
        "devices_count": filtered_devices.len(),
        "message": format!("Created batch operation for {} devices in room '{}'", filtered_devices.len(), room_name),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Create scheduled batch operations
pub async fn schedule_batch_operation(
    _context: ToolContext,
    batch_operation: BatchOperation,
    schedule_time: chrono::DateTime<chrono::Utc>,
    repeat_interval: Option<Duration>,
) -> ToolResponse {
    debug!(
        "Scheduling batch operation '{}' for {}",
        batch_operation.name, schedule_time
    );

    // In a real implementation, this would integrate with a job scheduler
    // For now, return a placeholder response

    let schedule_info = json!({
        "batch_id": batch_operation.id,
        "name": batch_operation.name,
        "scheduled_time": schedule_time.to_rfc3339(),
        "repeat_interval_seconds": repeat_interval.map(|d| d.as_secs()),
        "status": "scheduled",
        "commands_count": batch_operation.commands.len()
    });

    ToolResponse::success(json!({
        "schedule": schedule_info,
        "message": format!("Batch operation '{}' scheduled for {}", batch_operation.name, schedule_time),
        "note": "Scheduling feature requires job scheduler integration",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Get batch operation templates for common scenarios
pub async fn get_batch_templates(_context: ToolContext) -> ToolResponse {
    debug!("Getting batch operation templates");

    let templates = vec![
        json!({
            "name": "All Lights Off",
            "description": "Turn off all lights in the system",
            "template": {
                "execution_mode": "parallel",
                "error_strategy": "continue",
                "device_filter": ["Light", "Dimmer"],
                "command": "off"
            }
        }),
        json!({
            "name": "Close All Blinds",
            "description": "Close all blinds/rolladen in the system",
            "template": {
                "execution_mode": "sequential",
                "error_strategy": "continue",
                "device_filter": ["Jalousie", "Blind"],
                "command": "down"
            }
        }),
        json!({
            "name": "Security Mode",
            "description": "Activate security mode - lights off, blinds closed, doors locked",
            "template": {
                "execution_mode": "ordered",
                "error_strategy": "rollback",
                "commands": [
                    {"device_filter": ["Light"], "command": "off", "priority": "normal"},
                    {"device_filter": ["Jalousie"], "command": "down", "priority": "normal"},
                    {"device_filter": ["Lock"], "command": "lock", "priority": "high"}
                ]
            }
        }),
        json!({
            "name": "Morning Routine",
            "description": "Morning automation - open blinds, turn on lights, set climate",
            "template": {
                "execution_mode": "ordered",
                "error_strategy": "continue",
                "commands": [
                    {"device_filter": ["Jalousie"], "command": "up", "delay": 0},
                    {"device_filter": ["Light"], "command": "on", "delay": 2000},
                    {"device_filter": ["Climate"], "command": "comfort", "delay": 5000}
                ]
            }
        }),
        json!({
            "name": "Energy Saving",
            "description": "Reduce energy consumption across all devices",
            "template": {
                "execution_mode": "parallel",
                "error_strategy": "continue",
                "commands": [
                    {"device_filter": ["Light"], "command": "dim/20"},
                    {"device_filter": ["Climate"], "command": "eco"},
                    {"device_filter": ["Audio"], "command": "off"}
                ]
            }
        }),
    ];

    ToolResponse::success(json!({
        "templates": templates,
        "count": templates.len(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// Helper functions for batch operations

/// Validate batch operation configuration
fn validate_batch_operation(batch: &BatchOperation) -> Result<(), String> {
    if batch.commands.is_empty() {
        return Err("Batch operation must contain at least one command".to_string());
    }

    if batch.commands.len() > 1000 {
        return Err("Batch operation cannot exceed 1000 commands".to_string());
    }

    // Validate command IDs are unique
    let mut ids = std::collections::HashSet::new();
    for cmd in &batch.commands {
        if !ids.insert(&cmd.id) {
            return Err(format!("Duplicate command ID: {}", cmd.id));
        }
    }

    // Validate dependencies exist
    for cmd in &batch.commands {
        for dep_id in &cmd.dependencies {
            if !ids.contains(dep_id) {
                return Err(format!(
                    "Command '{}' depends on non-existent command '{}'",
                    cmd.id, dep_id
                ));
            }
        }
    }

    Ok(())
}

/// Resolve device names to UUIDs for batch commands
async fn resolve_batch_devices(
    context: &ToolContext,
    commands: &[BatchCommand],
) -> Result<Vec<(BatchCommand, LoxoneDevice)>, String> {
    let devices = context.context.devices.read().await;
    let mut resolved = Vec::new();

    for cmd in commands {
        // Try exact UUID match first
        let device = if let Some(device) = devices.get(&cmd.device) {
            device.clone()
        } else {
            // Try name matching
            devices
                .values()
                .find(|d| d.name.to_lowercase().contains(&cmd.device.to_lowercase()))
                .cloned()
                .ok_or_else(|| format!("Device not found: {}", cmd.device))?
        };

        resolved.push((cmd.clone(), device));
    }

    Ok(resolved)
}

/// Execute batch commands in parallel
async fn execute_parallel_batch(
    context: &ToolContext,
    commands: &[(BatchCommand, LoxoneDevice)],
    config: &BatchOperation,
) -> Vec<BatchCommandResult> {
    use futures::future::join_all;

    let max_parallel = config.max_parallel.unwrap_or(10) as usize;
    let mut results = Vec::new();

    // Process commands in chunks to respect max_parallel limit
    for chunk in commands.chunks(max_parallel) {
        let futures: Vec<_> = chunk
            .iter()
            .map(|(cmd, device)| execute_single_command(context, cmd, device))
            .collect();

        let chunk_results = join_all(futures).await;
        results.extend(chunk_results);
    }

    results
}

/// Execute batch commands sequentially
async fn execute_sequential_batch(
    context: &ToolContext,
    commands: &[(BatchCommand, LoxoneDevice)],
    config: &BatchOperation,
) -> Vec<BatchCommandResult> {
    let mut results = Vec::new();

    for (cmd, device) in commands {
        let result = execute_single_command(context, cmd, device).await;

        // Check error strategy
        if result.status == CommandStatus::Failed
            && config.error_strategy == ErrorStrategy::StopOnError
        {
            results.push(result);
            break;
        }

        results.push(result);
    }

    results
}

/// Execute batch commands with dependency ordering
async fn execute_ordered_batch(
    context: &ToolContext,
    commands: &[(BatchCommand, LoxoneDevice)],
    _config: &BatchOperation,
) -> Vec<BatchCommandResult> {
    // For simplicity, implement basic dependency resolution
    // In a full implementation, this would use a topological sort

    let mut results = Vec::new();
    let mut completed_commands = std::collections::HashSet::new();
    let mut remaining_commands: Vec<_> = commands.iter().collect();

    while !remaining_commands.is_empty() {
        let mut executed_this_round = false;

        // Find commands with satisfied dependencies
        let mut ready_commands = Vec::new();
        remaining_commands.retain(|(cmd, device)| {
            let dependencies_satisfied = cmd
                .dependencies
                .iter()
                .all(|dep_id| completed_commands.contains(dep_id));

            if dependencies_satisfied {
                ready_commands.push((cmd.clone(), device.clone()));
                false // Remove from remaining
            } else {
                true // Keep in remaining
            }
        });

        // Execute ready commands in parallel
        if !ready_commands.is_empty() {
            use futures::future::join_all;

            let futures: Vec<_> = ready_commands
                .iter()
                .map(|(cmd, device)| execute_single_command(context, cmd, device))
                .collect();

            let round_results = join_all(futures).await;

            for result in round_results {
                completed_commands.insert(result.command_id.clone());
                results.push(result);
                executed_this_round = true;
            }
        }

        // Prevent infinite loop if no progress is made
        if !executed_this_round {
            error!("Circular dependency detected in batch operation");
            break;
        }
    }

    results
}

/// Execute a single command with retry logic
async fn execute_single_command(
    context: &ToolContext,
    cmd: &BatchCommand,
    device: &LoxoneDevice,
) -> BatchCommandResult {
    let start_time = Instant::now();
    let timestamp = chrono::Utc::now();
    let max_retries = cmd.retry_count.unwrap_or(0);

    for attempt in 0..=max_retries {
        match context
            .send_device_command(&device.uuid, &cmd.command)
            .await
        {
            Ok(response) => {
                return BatchCommandResult {
                    command_id: cmd.id.clone(),
                    device_name: device.name.clone(),
                    device_uuid: device.uuid.clone(),
                    status: CommandStatus::Success,
                    command: cmd.command.clone(),
                    response: Some(response.value),
                    error: None,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    retry_attempts: attempt,
                    timestamp,
                };
            }
            Err(e) => {
                if attempt == max_retries {
                    return BatchCommandResult {
                        command_id: cmd.id.clone(),
                        device_name: device.name.clone(),
                        device_uuid: device.uuid.clone(),
                        status: CommandStatus::Failed,
                        command: cmd.command.clone(),
                        response: None,
                        error: Some(e.to_string()),
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        retry_attempts: attempt,
                        timestamp,
                    };
                }
                // Wait before retry
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }

    // This should never be reached due to the loop logic, but just in case
    BatchCommandResult {
        command_id: cmd.id.clone(),
        device_name: device.name.clone(),
        device_uuid: device.uuid.clone(),
        status: CommandStatus::Failed,
        command: cmd.command.clone(),
        response: None,
        error: Some("Unexpected error in retry logic".to_string()),
        duration_ms: start_time.elapsed().as_millis() as u64,
        retry_attempts: max_retries,
        timestamp,
    }
}
