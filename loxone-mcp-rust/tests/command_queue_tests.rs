//! Tests for command queue functionality

use loxone_mcp_rust::client::command_queue::{
    CommandQueue, CommandQueueConfig, QueuedCommand, CommandPriority, ExecutionStrategy,
    CommandResult
};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tokio::time::timeout;

fn create_test_config() -> CommandQueueConfig {
    CommandQueueConfig {
        max_queue_size: 100,
        default_expiration: Duration::from_secs(60),
        max_concurrent_executions: 5,
        batch_size: 3,
        batch_timeout: Duration::from_millis(100),
        enable_persistence: false,
        cleanup_interval: Duration::from_secs(10),
        max_retries: 2,
        preserve_order: true,
    }
}

#[tokio::test]
async fn test_command_queue_creation() {
    let queue = CommandQueue::new();
    let stats = queue.get_statistics().await;
    assert_eq!(stats.current_queue_size, 0);
    assert_eq!(stats.total_queued, 0);
}

#[tokio::test]
async fn test_command_queue_with_custom_config() {
    let config = create_test_config();
    let queue = CommandQueue::with_config(config.clone());
    
    // Verify the queue uses the custom configuration
    let stats = queue.get_statistics().await;
    assert_eq!(stats.current_queue_size, 0);
}

#[tokio::test]
async fn test_command_creation() {
    let command = QueuedCommand::new("device1".to_string(), "on".to_string(), "test".to_string());
    assert_eq!(command.device_uuid, "device1");
    assert_eq!(command.command, "on");
    assert_eq!(command.source, "test");
    assert_eq!(command.priority, CommandPriority::Normal);
    assert_eq!(command.attempt_count, 0);
    assert!(!command.requires_consent);
}

#[tokio::test]
async fn test_high_priority_command() {
    let command = QueuedCommand::new_high_priority("device1".to_string(), "emergency".to_string(), "test".to_string());
    assert_eq!(command.priority, CommandPriority::High);
}

#[tokio::test]
async fn test_critical_command() {
    let command = QueuedCommand::new_critical("device1".to_string(), "alarm".to_string(), "test".to_string());
    assert_eq!(command.priority, CommandPriority::Critical);
    assert_eq!(command.max_retries, 5); // Critical commands get more retries
}

#[tokio::test]
async fn test_command_with_expiration() {
    let command = QueuedCommand::new("device1".to_string(), "test".to_string(), "test".to_string())
        .with_expiration(Duration::from_secs(10));
    
    assert!(command.expires_at.is_some());
    assert!(!command.is_expired()); // Should not be expired immediately
}

#[tokio::test]
async fn test_command_with_metadata() {
    let command = QueuedCommand::new("device1".to_string(), "test".to_string(), "test".to_string())
        .with_metadata("room".to_string(), "living_room".to_string())
        .with_metadata("type".to_string(), "light".to_string());
    
    assert_eq!(command.metadata.get("room"), Some(&"living_room".to_string()));
    assert_eq!(command.metadata.get("type"), Some(&"light".to_string()));
}

#[tokio::test]
async fn test_command_with_strategy() {
    let command = QueuedCommand::new("device1".to_string(), "test".to_string(), "test".to_string())
        .with_strategy(ExecutionStrategy::Batch { max_batch_size: 5 });
    
    match command.strategy {
        ExecutionStrategy::Batch { max_batch_size } => assert_eq!(max_batch_size, 5),
        _ => panic!("Expected batch strategy"),
    }
}

#[tokio::test]
async fn test_command_retry_logic() {
    let mut command = QueuedCommand::new("device1".to_string(), "test".to_string(), "test".to_string())
        .with_max_retries(3);
    
    assert!(command.should_retry());
    
    command.increment_attempts();
    assert_eq!(command.attempt_count, 1);
    assert!(command.should_retry());
    
    // Exhaust retries
    command.increment_attempts();
    command.increment_attempts();
    command.increment_attempts();
    assert!(!command.should_retry());
}

#[tokio::test]
async fn test_command_retry_delay() {
    let mut command = QueuedCommand::new("device1".to_string(), "test".to_string(), "test".to_string());
    
    let delay1 = command.get_retry_delay();
    command.increment_attempts();
    let delay2 = command.get_retry_delay();
    command.increment_attempts();
    let delay3 = command.get_retry_delay();
    
    // Delays should increase (exponential backoff)
    assert!(delay2 > delay1);
    assert!(delay3 > delay2);
}

#[tokio::test]
async fn test_queue_enqueue_dequeue() {
    let queue = CommandQueue::with_config(create_test_config());
    
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
async fn test_priority_ordering() {
    let queue = CommandQueue::with_config(create_test_config());
    
    // Add commands with different priorities in random order
    let low_cmd = QueuedCommand::new("device1".to_string(), "low".to_string(), "test".to_string());
    let normal_cmd = QueuedCommand::new("device2".to_string(), "normal".to_string(), "test".to_string());
    let high_cmd = QueuedCommand::new_high_priority("device3".to_string(), "high".to_string(), "test".to_string());
    let critical_cmd = QueuedCommand::new_critical("device4".to_string(), "critical".to_string(), "test".to_string());
    
    // Store IDs for verification
    let low_id = low_cmd.id;
    let normal_id = normal_cmd.id;
    let high_id = high_cmd.id;
    let critical_id = critical_cmd.id;
    
    // Enqueue in random order
    queue.enqueue(normal_cmd).await.unwrap();
    queue.enqueue(low_cmd).await.unwrap();
    queue.enqueue(critical_cmd).await.unwrap();
    queue.enqueue(high_cmd).await.unwrap();
    
    // Should dequeue in priority order: Critical, High, Normal, Low
    let first = queue.dequeue().await.unwrap();
    assert_eq!(first.id, critical_id);
    
    let second = queue.dequeue().await.unwrap();
    assert_eq!(second.id, high_id);
    
    let third = queue.dequeue().await.unwrap();
    assert_eq!(third.id, normal_id);
    
    let fourth = queue.dequeue().await.unwrap();
    assert_eq!(fourth.id, low_id);
}

#[tokio::test]
async fn test_queue_size_limit() {
    let config = CommandQueueConfig {
        max_queue_size: 2,
        ..create_test_config()
    };
    let queue = CommandQueue::with_config(config);
    
    let cmd1 = QueuedCommand::new("device1".to_string(), "cmd1".to_string(), "test".to_string());
    let cmd2 = QueuedCommand::new("device2".to_string(), "cmd2".to_string(), "test".to_string());
    let cmd3 = QueuedCommand::new("device3".to_string(), "cmd3".to_string(), "test".to_string());
    
    // First two commands should succeed
    assert!(queue.enqueue(cmd1).await.is_ok());
    assert!(queue.enqueue(cmd2).await.is_ok());
    
    // Third command should fail due to size limit
    let result = queue.enqueue(cmd3).await;
    assert!(result.is_err());
    assert_eq!(queue.get_current_queue_size().await, 2);
}

#[tokio::test]
async fn test_command_expiration() {
    let queue = CommandQueue::with_config(create_test_config());
    
    let expired_command = QueuedCommand::new("device1".to_string(), "test".to_string(), "test".to_string())
        .with_expiration(Duration::from_millis(1)); // Very short expiration
    
    queue.enqueue(expired_command).await.unwrap();
    assert_eq!(queue.get_current_queue_size().await, 1);
    
    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    // Cleanup should remove expired command
    let cleaned = queue.cleanup_expired().await.unwrap();
    assert_eq!(cleaned, 1);
    assert_eq!(queue.get_current_queue_size().await, 0);
}

#[tokio::test]
async fn test_commands_by_priority() {
    let queue = CommandQueue::with_config(create_test_config());
    
    // Add multiple commands with same priority
    let cmd1 = QueuedCommand::new_high_priority("device1".to_string(), "cmd1".to_string(), "test".to_string());
    let cmd2 = QueuedCommand::new_high_priority("device2".to_string(), "cmd2".to_string(), "test".to_string());
    let cmd3 = QueuedCommand::new("device3".to_string(), "cmd3".to_string(), "test".to_string());
    
    queue.enqueue(cmd1).await.unwrap();
    queue.enqueue(cmd2).await.unwrap();
    queue.enqueue(cmd3).await.unwrap();
    
    // Get high priority commands
    let high_priority_commands = queue.get_commands_by_priority(CommandPriority::High).await;
    assert_eq!(high_priority_commands.len(), 2);
    
    // Get normal priority commands
    let normal_priority_commands = queue.get_commands_by_priority(CommandPriority::Normal).await;
    assert_eq!(normal_priority_commands.len(), 1);
}

#[tokio::test]
async fn test_queue_clear() {
    let queue = CommandQueue::with_config(create_test_config());
    
    // Add several commands
    for i in 1..=5 {
        let command = QueuedCommand::new(format!("device{}", i), "test".to_string(), "test".to_string());
        queue.enqueue(command).await.unwrap();
    }
    
    assert_eq!(queue.get_current_queue_size().await, 5);
    
    // Clear all commands
    let cleared = queue.clear().await.unwrap();
    assert_eq!(cleared, 5);
    assert_eq!(queue.get_current_queue_size().await, 0);
}

#[tokio::test]
async fn test_batch_execution() {
    let queue = CommandQueue::with_config(create_test_config());
    
    // Add multiple commands
    for i in 1..=4 {
        let command = QueuedCommand::new(format!("device{}", i), format!("cmd{}", i), "test".to_string());
        queue.enqueue(command).await.unwrap();
    }
    
    // Mock executor that always succeeds
    let executor = |command: QueuedCommand| {
        Box::pin(async move {
            Ok(serde_json::json!({
                "device": command.device_uuid,
                "command": command.command,
                "status": "success"
            }))
        })
    };
    
    // Execute a batch
    let results = queue.execute_batch(executor).await.unwrap();
    
    // Should execute up to batch_size commands (3 in our config)
    assert!(results.len() <= 3);
    assert!(results.len() > 0);
    
    // All results should be successful
    for result in &results {
        assert!(result.success);
        assert!(result.error.is_none());
        assert!(result.response.is_some());
    }
}

#[tokio::test]
async fn test_batch_execution_with_failures() {
    let queue = CommandQueue::with_config(create_test_config());
    
    // Add commands
    let success_cmd = QueuedCommand::new("device1".to_string(), "success".to_string(), "test".to_string());
    let fail_cmd = QueuedCommand::new("device2".to_string(), "fail".to_string(), "test".to_string());
    
    queue.enqueue(success_cmd).await.unwrap();
    queue.enqueue(fail_cmd).await.unwrap();
    
    // Mock executor that fails for "fail" commands
    let executor = |command: QueuedCommand| {
        Box::pin(async move {
            if command.command == "fail" {
                Err(loxone_mcp_rust::error::LoxoneError::device_control("Simulated failure"))
            } else {
                Ok(serde_json::json!({"status": "success"}))
            }
        })
    };
    
    let results = queue.execute_batch(executor).await.unwrap();
    
    // Should have both success and failure results
    assert_eq!(results.len(), 2);
    
    let success_result = results.iter().find(|r| r.success).unwrap();
    let failure_result = results.iter().find(|r| !r.success).unwrap();
    
    assert!(success_result.error.is_none());
    assert!(failure_result.error.is_some());
}

#[tokio::test]
async fn test_queue_statistics() {
    let queue = CommandQueue::with_config(create_test_config());
    
    // Initial statistics
    let initial_stats = queue.get_statistics().await;
    assert_eq!(initial_stats.total_queued, 0);
    assert_eq!(initial_stats.current_queue_size, 0);
    assert_eq!(initial_stats.successful_executions, 0);
    assert_eq!(initial_stats.failed_executions, 0);
    
    // Add some commands
    for i in 1..=3 {
        let command = QueuedCommand::new_high_priority(format!("device{}", i), "test".to_string(), "test".to_string());
        queue.enqueue(command).await.unwrap();
    }
    
    // Check updated statistics
    let stats = queue.get_statistics().await;
    assert_eq!(stats.total_queued, 3);
    assert_eq!(stats.current_queue_size, 3);
    assert!(stats.commands_by_priority.contains_key("High"));
}

#[tokio::test]
async fn test_queue_start_stop() {
    let mut queue = CommandQueue::with_config(create_test_config());
    
    // Start the queue
    let start_result = queue.start().await;
    assert!(start_result.is_ok());
    
    // Stop the queue
    let stop_result = queue.stop().await;
    assert!(stop_result.is_ok());
}

#[tokio::test]
async fn test_expired_command_handling() {
    let queue = CommandQueue::with_config(create_test_config());
    
    // Create an already expired command
    let mut expired_command = QueuedCommand::new("device1".to_string(), "test".to_string(), "test".to_string());
    expired_command.expires_at = Some(SystemTime::now() - Duration::from_secs(1));
    
    queue.enqueue(expired_command).await.unwrap();
    
    // Dequeue should skip expired command and return None (if no other commands)
    let dequeued = queue.dequeue().await;
    assert!(dequeued.is_none());
    
    // Queue should be empty after expired command is handled
    assert_eq!(queue.get_current_queue_size().await, 0);
}

#[tokio::test]
async fn test_command_with_consent_required() {
    let command = QueuedCommand::new("device1".to_string(), "sensitive_op".to_string(), "test".to_string())
        .with_consent_required();
    
    assert!(command.requires_consent);
}

#[tokio::test]
async fn test_queue_utilization_calculation() {
    let config = CommandQueueConfig {
        max_queue_size: 10,
        ..create_test_config()
    };
    let queue = CommandQueue::with_config(config);
    
    // Add 5 commands to a queue with max size 10
    for i in 1..=5 {
        let command = QueuedCommand::new(format!("device{}", i), "test".to_string(), "test".to_string());
        queue.enqueue(command).await.unwrap();
    }
    
    let stats = queue.get_statistics().await;
    assert_eq!(stats.queue_utilization, 50.0); // 5/10 * 100 = 50%
}