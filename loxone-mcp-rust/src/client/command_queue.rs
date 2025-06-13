//! Command queuing system for handling commands during disconnection
//!
//! This module provides a robust command queuing mechanism that stores commands
//! when the client is disconnected and automatically executes them upon reconnection.
//!
//! Features:
//! - Persistent command queue with configurable limits
//! - Priority-based command ordering
//! - Automatic retry with exponential backoff
//! - Command expiration and cleanup
//! - Batch execution for performance
//! - Statistics and monitoring

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, RwLock, Semaphore};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Priority levels for queued commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CommandPriority {
    /// Low priority - routine operations
    Low = 1,
    /// Normal priority - standard operations
    Normal = 2,
    /// High priority - important operations
    High = 3,
    /// Critical priority - safety/security operations
    Critical = 4,
}

impl Default for CommandPriority {
    fn default() -> Self {
        CommandPriority::Normal
    }
}

/// Command execution strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStrategy {
    /// Execute immediately when connection is available
    Immediate,
    /// Execute with delay after reconnection
    Delayed { delay: Duration },
    /// Execute in batch with other commands
    Batch { max_batch_size: usize },
    /// Execute with retry logic
    Retry { max_retries: u32, backoff: Duration },
}

impl Default for ExecutionStrategy {
    fn default() -> Self {
        ExecutionStrategy::Immediate
    }
}

/// Queued command information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedCommand {
    /// Unique command identifier
    pub id: Uuid,
    
    /// Device UUID to send command to
    pub device_uuid: String,
    
    /// Command to execute
    pub command: String,
    
    /// Command priority
    pub priority: CommandPriority,
    
    /// Execution strategy
    pub strategy: ExecutionStrategy,
    
    /// When the command was queued
    pub queued_at: SystemTime,
    
    /// Command expiration time
    pub expires_at: Option<SystemTime>,
    
    /// Number of execution attempts
    pub attempt_count: u32,
    
    /// Maximum retry attempts
    pub max_retries: u32,
    
    /// Source of the command (for tracking)
    pub source: String,
    
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    
    /// Whether this command requires consent
    pub requires_consent: bool,
}

impl QueuedCommand {
    /// Create a new queued command
    pub fn new(device_uuid: String, command: String, source: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            device_uuid,
            command,
            priority: CommandPriority::default(),
            strategy: ExecutionStrategy::default(),
            queued_at: SystemTime::now(),
            expires_at: None,
            attempt_count: 0,
            max_retries: 3,
            source,
            metadata: HashMap::new(),
            requires_consent: false,
        }
    }
    
    /// Create a new high-priority command
    pub fn new_high_priority(device_uuid: String, command: String, source: String) -> Self {
        Self {
            priority: CommandPriority::High,
            ..Self::new(device_uuid, command, source)
        }
    }
    
    /// Create a new critical command
    pub fn new_critical(device_uuid: String, command: String, source: String) -> Self {
        Self {
            priority: CommandPriority::Critical,
            max_retries: 5, // More retries for critical commands
            ..Self::new(device_uuid, command, source)
        }
    }
    
    /// Set command expiration
    pub fn with_expiration(mut self, duration: Duration) -> Self {
        self.expires_at = Some(SystemTime::now() + duration);
        self
    }
    
    /// Set execution strategy
    pub fn with_strategy(mut self, strategy: ExecutionStrategy) -> Self {
        self.strategy = strategy;
        self
    }
    
    /// Set max retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
    
    /// Mark as requiring consent
    pub fn with_consent_required(mut self) -> Self {
        self.requires_consent = true;
        self
    }
    
    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
    
    /// Check if command has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            SystemTime::now() > expires_at
        } else {
            false
        }
    }
    
    /// Check if command should be retried
    pub fn should_retry(&self) -> bool {
        self.attempt_count < self.max_retries && !self.is_expired()
    }
    
    /// Increment attempt count
    pub fn increment_attempts(&mut self) {
        self.attempt_count += 1;
    }
    
    /// Get delay for retry based on attempt count
    pub fn get_retry_delay(&self) -> Duration {
        let base_delay_ms = 100_u64;
        let backoff_factor = 2_u64.pow(self.attempt_count.min(10)); // Cap at 2^10
        Duration::from_millis(base_delay_ms * backoff_factor)
    }
}

/// Command execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// Command that was executed
    pub command_id: Uuid,
    
    /// Whether execution was successful
    pub success: bool,
    
    /// Error message if execution failed
    pub error: Option<String>,
    
    /// Execution timestamp
    pub executed_at: SystemTime,
    
    /// Execution duration
    pub duration: Duration,
    
    /// Response data (if available)
    pub response: Option<serde_json::Value>,
}

/// Configuration for command queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandQueueConfig {
    /// Maximum number of commands in queue
    pub max_queue_size: usize,
    
    /// Default command expiration time
    pub default_expiration: Duration,
    
    /// Maximum number of concurrent executions
    pub max_concurrent_executions: usize,
    
    /// Batch execution settings
    pub batch_size: usize,
    pub batch_timeout: Duration,
    
    /// Enable persistent storage
    pub enable_persistence: bool,
    
    /// Cleanup interval for expired commands
    pub cleanup_interval: Duration,
    
    /// Maximum retry attempts for failed commands
    pub max_retries: u32,
    
    /// Whether to preserve command order within priorities
    pub preserve_order: bool,
}

impl Default for CommandQueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 1000,
            default_expiration: Duration::from_secs(3600), // 1 hour
            max_concurrent_executions: 10,
            batch_size: 5,
            batch_timeout: Duration::from_millis(100),
            enable_persistence: false,
            cleanup_interval: Duration::from_secs(300), // 5 minutes
            max_retries: 3,
            preserve_order: true,
        }
    }
}

/// Statistics for command queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandQueueStats {
    /// Total commands queued
    pub total_queued: u64,
    
    /// Commands currently in queue
    pub current_queue_size: usize,
    
    /// Successfully executed commands
    pub successful_executions: u64,
    
    /// Failed command executions
    pub failed_executions: u64,
    
    /// Expired commands
    pub expired_commands: u64,
    
    /// Commands by priority
    pub commands_by_priority: HashMap<String, usize>,
    
    /// Average execution time
    pub avg_execution_time: Duration,
    
    /// Queue utilization percentage
    pub queue_utilization: f32,
}

/// Command queue for handling commands during disconnection
pub struct CommandQueue {
    /// Configuration
    config: CommandQueueConfig,
    
    /// Priority-based command queues
    queues: Arc<RwLock<HashMap<CommandPriority, VecDeque<QueuedCommand>>>>,
    
    /// Commands currently being executed
    executing: Arc<RwLock<HashMap<Uuid, QueuedCommand>>>,
    
    /// Command execution results history
    results: Arc<RwLock<VecDeque<CommandResult>>>,
    
    /// Statistics
    stats: Arc<RwLock<CommandQueueStats>>,
    
    /// Execution semaphore for concurrency control
    execution_semaphore: Arc<Semaphore>,
    
    /// Shutdown signal
    shutdown_sender: Option<mpsc::UnboundedSender<()>>,
    shutdown_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<()>>>>,
    
    /// Command completion notifications
    completion_sender: mpsc::UnboundedSender<CommandResult>,
    completion_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<CommandResult>>>>,
}

impl CommandQueue {
    /// Create a new command queue with default configuration
    pub fn new() -> Self {
        Self::with_config(CommandQueueConfig::default())
    }
    
    /// Create a new command queue with custom configuration
    pub fn with_config(config: CommandQueueConfig) -> Self {
        let (shutdown_tx, shutdown_rx) = mpsc::unbounded_channel();
        let (completion_tx, completion_rx) = mpsc::unbounded_channel();
        
        // Initialize priority queues
        let mut queues = HashMap::new();
        queues.insert(CommandPriority::Low, VecDeque::new());
        queues.insert(CommandPriority::Normal, VecDeque::new());
        queues.insert(CommandPriority::High, VecDeque::new());
        queues.insert(CommandPriority::Critical, VecDeque::new());
        
        Self {
            execution_semaphore: Arc::new(Semaphore::new(config.max_concurrent_executions)),
            config,
            queues: Arc::new(RwLock::new(queues)),
            executing: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(CommandQueueStats {
                total_queued: 0,
                current_queue_size: 0,
                successful_executions: 0,
                failed_executions: 0,
                expired_commands: 0,
                commands_by_priority: HashMap::new(),
                avg_execution_time: Duration::from_millis(0),
                queue_utilization: 0.0,
            })),
            shutdown_sender: Some(shutdown_tx),
            shutdown_receiver: Arc::new(RwLock::new(Some(shutdown_rx))),
            completion_sender: completion_tx,
            completion_receiver: Arc::new(RwLock::new(Some(completion_rx))),
        }
    }
    
    /// Start the command queue background tasks
    pub async fn start(&self) -> Result<()> {
        // Start cleanup task
        self.start_cleanup_task().await;
        
        // Start batch execution task
        self.start_batch_execution_task().await;
        
        info!("Command queue started with config: max_size={}, max_concurrent={}", 
              self.config.max_queue_size, self.config.max_concurrent_executions);
        
        Ok(())
    }
    
    /// Stop the command queue
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(sender) = self.shutdown_sender.take() {
            let _ = sender.send(());
        }
        
        info!("Command queue stopped");
        Ok(())
    }
    
    /// Add a command to the queue
    pub async fn enqueue(&self, mut command: QueuedCommand) -> Result<Uuid> {
        // Check queue size limit
        let current_size = self.get_current_queue_size().await;
        if current_size >= self.config.max_queue_size {
            return Err(LoxoneError::resource_exhausted("Command queue is full"));
        }
        
        // Set default expiration if not set
        if command.expires_at.is_none() {
            command.expires_at = Some(SystemTime::now() + self.config.default_expiration);
        }
        
        let command_id = command.id;
        let priority = command.priority;
        
        // Add to appropriate priority queue
        {
            let mut queues = self.queues.write().await;
            if let Some(queue) = queues.get_mut(&priority) {
                queue.push_back(command);
            }
        }
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_queued += 1;
            stats.current_queue_size = current_size + 1;
            *stats.commands_by_priority.entry(format!("{:?}", priority)).or_insert(0) += 1;
            stats.queue_utilization = (stats.current_queue_size as f32 / self.config.max_queue_size as f32) * 100.0;
        }
        
        debug!("Command {} queued with priority {:?}", command_id, priority);
        Ok(command_id)
    }
    
    /// Get the next command to execute (highest priority first)
    pub async fn dequeue(&self) -> Option<QueuedCommand> {
        let mut queues = self.queues.write().await;
        
        // Check queues in priority order (highest first)
        let priorities = [
            CommandPriority::Critical,
            CommandPriority::High,
            CommandPriority::Normal,
            CommandPriority::Low,
        ];
        
        for priority in &priorities {
            if let Some(queue) = queues.get_mut(priority) {
                while let Some(command) = queue.pop_front() {
                    if command.is_expired() {
                        self.handle_expired_command(command).await;
                        continue;
                    }
                    
                    debug!("Dequeued command {} with priority {:?}", command.id, command.priority);
                    return Some(command);
                }
            }
        }
        
        None
    }
    
    /// Execute a batch of commands
    pub async fn execute_batch<F, Fut>(&self, executor: F) -> Result<Vec<CommandResult>>
    where
        F: Fn(QueuedCommand) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<serde_json::Value>> + Send,
    {
        let mut commands = Vec::new();
        let results = Vec::new();
        
        // Collect commands for batch execution
        for _ in 0..self.config.batch_size {
            if let Some(command) = self.dequeue().await {
                commands.push(command);
            } else {
                break;
            }
        }
        
        if commands.is_empty() {
            return Ok(results);
        }
        
        info!("Executing batch of {} commands", commands.len());
        
        // Execute commands concurrently
        use futures::future::join_all;
        
        let futures = commands.into_iter().map(|command| {
            let executor = &executor;
            async move {
                let start_time = SystemTime::now();
                let command_id = command.id;
                
                // Mark as executing
                {
                    let mut executing = self.executing.write().await;
                    executing.insert(command_id, command.clone());
                }
                
                // Execute the command
                let result = match executor(command.clone()).await {
                    Ok(response) => CommandResult {
                        command_id,
                        success: true,
                        error: None,
                        executed_at: start_time,
                        duration: SystemTime::now().duration_since(start_time).unwrap_or_default(),
                        response: Some(response),
                    },
                    Err(e) => CommandResult {
                        command_id,
                        success: false,
                        error: Some(e.to_string()),
                        executed_at: start_time,
                        duration: SystemTime::now().duration_since(start_time).unwrap_or_default(),
                        response: None,
                    },
                };
                
                // Remove from executing
                {
                    let mut executing = self.executing.write().await;
                    executing.remove(&command_id);
                }
                
                // Handle result
                if result.success {
                    debug!("Command {} executed successfully", command_id);
                } else {
                    warn!("Command {} failed: {:?}", command_id, result.error);
                    
                    // Re-queue for retry if appropriate
                    let mut retry_command = command;
                    retry_command.increment_attempts();
                    
                    if retry_command.should_retry() {
                        debug!("Re-queuing command {} for retry (attempt {})", command_id, retry_command.attempt_count);
                        // Add delay for retry
                        tokio::time::sleep(retry_command.get_retry_delay()).await;
                        let _ = self.enqueue(retry_command).await;
                    }
                }
                
                result
            }
        });
        
        let execution_results = join_all(futures).await;
        
        // Update statistics and store results
        {
            let mut stats = self.stats.write().await;
            let mut results_storage = self.results.write().await;
            
            for result in &execution_results {
                if result.success {
                    stats.successful_executions += 1;
                } else {
                    stats.failed_executions += 1;
                }
                
                // Update average execution time
                let total_executions = stats.successful_executions + stats.failed_executions;
                if total_executions > 1 {
                    let current_avg = stats.avg_execution_time.as_millis() as f64;
                    let new_duration = result.duration.as_millis() as f64;
                    let new_avg = ((current_avg * (total_executions - 1) as f64) + new_duration) / total_executions as f64;
                    stats.avg_execution_time = Duration::from_millis(new_avg as u64);
                } else {
                    stats.avg_execution_time = result.duration;
                }
                
                // Store result (keep only last 1000 results)
                results_storage.push_back(result.clone());
                if results_storage.len() > 1000 {
                    results_storage.pop_front();
                }
                
                // Send completion notification
                let _ = self.completion_sender.send(result.clone());
            }
            
            stats.current_queue_size = self.get_current_queue_size().await;
            stats.queue_utilization = (stats.current_queue_size as f32 / self.config.max_queue_size as f32) * 100.0;
        }
        
        Ok(execution_results)
    }
    
    /// Get current queue size across all priorities
    pub async fn get_current_queue_size(&self) -> usize {
        let queues = self.queues.read().await;
        queues.values().map(|q| q.len()).sum()
    }
    
    /// Get queue statistics
    pub async fn get_statistics(&self) -> CommandQueueStats {
        self.stats.read().await.clone()
    }
    
    /// Clear all queued commands
    pub async fn clear(&self) -> Result<usize> {
        let mut queues = self.queues.write().await;
        let mut total_cleared = 0;
        
        for queue in queues.values_mut() {
            total_cleared += queue.len();
            queue.clear();
        }
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.current_queue_size = 0;
            stats.queue_utilization = 0.0;
            stats.commands_by_priority.clear();
        }
        
        info!("Cleared {} commands from queue", total_cleared);
        Ok(total_cleared)
    }
    
    /// Get commands by priority
    pub async fn get_commands_by_priority(&self, priority: CommandPriority) -> Vec<QueuedCommand> {
        let queues = self.queues.read().await;
        if let Some(queue) = queues.get(&priority) {
            queue.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }
    
    /// Remove expired commands from queues
    pub async fn cleanup_expired(&self) -> Result<usize> {
        let mut queues = self.queues.write().await;
        let mut total_expired = 0;
        
        for queue in queues.values_mut() {
            let initial_len = queue.len();
            queue.retain(|cmd| !cmd.is_expired());
            let expired_count = initial_len - queue.len();
            total_expired += expired_count;
        }
        
        if total_expired > 0 {
            let mut stats = self.stats.write().await;
            stats.expired_commands += total_expired as u64;
            stats.current_queue_size = queues.values().map(|q| q.len()).sum();
            stats.queue_utilization = (stats.current_queue_size as f32 / self.config.max_queue_size as f32) * 100.0;
            
            info!("Cleaned up {} expired commands", total_expired);
        }
        
        Ok(total_expired)
    }
    
    /// Handle expired command
    async fn handle_expired_command(&self, command: QueuedCommand) {
        warn!("Command {} expired (queued at {:?})", command.id, command.queued_at);
        
        let result = CommandResult {
            command_id: command.id,
            success: false,
            error: Some("Command expired".to_string()),
            executed_at: SystemTime::now(),
            duration: Duration::from_millis(0),
            response: None,
        };
        
        let _ = self.completion_sender.send(result);
    }
    
    /// Start cleanup task for expired commands
    async fn start_cleanup_task(&self) {
        let queues = self.queues.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();
        let shutdown_receiver = self.shutdown_receiver.clone();
        
        tokio::spawn(async move {
            let mut shutdown_rx = shutdown_receiver.write().await.take();
            let mut interval = tokio::time::interval(config.cleanup_interval);
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Cleanup expired commands
                        let mut total_expired = 0;
                        {
                            let mut queues_guard = queues.write().await;
                            for queue in queues_guard.values_mut() {
                                let initial_len = queue.len();
                                queue.retain(|cmd| !cmd.is_expired());
                                total_expired += initial_len - queue.len();
                            }
                        }
                        
                        if total_expired > 0 {
                            let mut stats_guard = stats.write().await;
                            stats_guard.expired_commands += total_expired as u64;
                            stats_guard.current_queue_size = {
                                let queues_guard = queues.read().await;
                                queues_guard.values().map(|q| q.len()).sum()
                            };
                            stats_guard.queue_utilization = (stats_guard.current_queue_size as f32 / config.max_queue_size as f32) * 100.0;
                            
                            debug!("Cleanup task removed {} expired commands", total_expired);
                        }
                    }
                    _ = async {
                        if let Some(ref mut rx) = shutdown_rx {
                            rx.recv().await
                        } else {
                            std::future::pending().await
                        }
                    } => {
                        debug!("Cleanup task shutting down");
                        break;
                    }
                }
            }
        });
    }
    
    /// Start batch execution task
    async fn start_batch_execution_task(&self) {
        // This would be implemented when integrating with actual clients
        // For now, it's a placeholder that could trigger batch execution
        debug!("Batch execution task would be started here");
    }
}

impl Default for CommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_command_queue_creation() {
        let queue = CommandQueue::new();
        let stats = queue.get_statistics().await;
        assert_eq!(stats.current_queue_size, 0);
        assert_eq!(stats.total_queued, 0);
    }
    
    #[tokio::test]
    async fn test_command_enqueue_dequeue() {
        let queue = CommandQueue::new();
        
        let command = QueuedCommand::new("device1".to_string(), "on".to_string(), "test".to_string());
        let command_id = command.id;
        
        // Enqueue command
        let result = queue.enqueue(command).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), command_id);
        
        // Check queue size
        assert_eq!(queue.get_current_queue_size().await, 1);
        
        // Dequeue command
        let dequeued = queue.dequeue().await;
        assert!(dequeued.is_some());
        assert_eq!(dequeued.unwrap().id, command_id);
        
        // Queue should be empty now
        assert_eq!(queue.get_current_queue_size().await, 0);
    }
    
    #[tokio::test]
    async fn test_command_priority_ordering() {
        let queue = CommandQueue::new();
        
        // Add commands with different priorities
        let low_cmd = QueuedCommand::new("device1".to_string(), "low".to_string(), "test".to_string());
        let high_cmd = QueuedCommand::new_high_priority("device2".to_string(), "high".to_string(), "test".to_string());
        let critical_cmd = QueuedCommand::new_critical("device3".to_string(), "critical".to_string(), "test".to_string());
        
        // Enqueue in random order
        queue.enqueue(low_cmd).await.unwrap();
        queue.enqueue(high_cmd.clone()).await.unwrap();
        queue.enqueue(critical_cmd.clone()).await.unwrap();
        
        // Should dequeue in priority order
        let first = queue.dequeue().await.unwrap();
        assert_eq!(first.id, critical_cmd.id);
        
        let second = queue.dequeue().await.unwrap();
        assert_eq!(second.id, high_cmd.id);
    }
    
    #[tokio::test]
    async fn test_command_expiration() {
        let queue = CommandQueue::new();
        
        let expired_command = QueuedCommand::new("device1".to_string(), "test".to_string(), "test".to_string())
            .with_expiration(Duration::from_millis(1)); // Very short expiration
        
        queue.enqueue(expired_command).await.unwrap();
        
        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Cleanup should remove expired command
        let cleaned = queue.cleanup_expired().await.unwrap();
        assert_eq!(cleaned, 1);
        assert_eq!(queue.get_current_queue_size().await, 0);
    }
    
    #[tokio::test]
    async fn test_queue_size_limit() {
        let config = CommandQueueConfig {
            max_queue_size: 2,
            ..Default::default()
        };
        let queue = CommandQueue::with_config(config);
        
        // Add commands up to limit
        let cmd1 = QueuedCommand::new("device1".to_string(), "cmd1".to_string(), "test".to_string());
        let cmd2 = QueuedCommand::new("device2".to_string(), "cmd2".to_string(), "test".to_string());
        let cmd3 = QueuedCommand::new("device3".to_string(), "cmd3".to_string(), "test".to_string());
        
        assert!(queue.enqueue(cmd1).await.is_ok());
        assert!(queue.enqueue(cmd2).await.is_ok());
        
        // Third command should fail due to size limit
        let result = queue.enqueue(cmd3).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_command_with_metadata() {
        let queue = CommandQueue::new();
        
        let command = QueuedCommand::new("device1".to_string(), "test".to_string(), "test".to_string())
            .with_metadata("room".to_string(), "living_room".to_string())
            .with_metadata("type".to_string(), "light".to_string());
        
        assert_eq!(command.metadata.get("room"), Some(&"living_room".to_string()));
        assert_eq!(command.metadata.get("type"), Some(&"light".to_string()));
        
        queue.enqueue(command).await.unwrap();
        let dequeued = queue.dequeue().await.unwrap();
        assert_eq!(dequeued.metadata.len(), 2);
    }
}