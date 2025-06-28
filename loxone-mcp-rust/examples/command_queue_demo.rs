//! Command Queue Demo
//!
//! This example demonstrates the command queuing system for handling commands
//! during disconnection periods. Commands are queued when the client is offline
//! and automatically executed when reconnection is established.

use loxone_mcp_rust::client::command_queue::{
    CommandQueue, CommandQueueConfig, ExecutionStrategy, QueuedCommand,
};
use loxone_mcp_rust::client::token_http_client::TokenHttpClient;
use loxone_mcp_rust::config::credentials::LoxoneCredentials;
use loxone_mcp_rust::config::{AuthMethod, LoxoneConfig};
use std::sync::Arc;
use std::time::Duration;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚦 Command Queue Demo");
    println!("===================\n");

    // Demo 1: Command Queue Configuration
    println!("1️⃣  Command Queue Configuration Options");

    // Default configuration
    let default_config = CommandQueueConfig::default();
    println!("   🎯 Default Configuration:");
    println!("      Max queue size: {}", default_config.max_queue_size);
    println!(
        "      Default expiration: {:?}",
        default_config.default_expiration
    );
    println!(
        "      Max concurrent executions: {}",
        default_config.max_concurrent_executions
    );
    println!("      Batch size: {}", default_config.batch_size);
    println!("      Batch timeout: {:?}", default_config.batch_timeout);
    println!(
        "      Cleanup interval: {:?}",
        default_config.cleanup_interval
    );

    // High-throughput configuration
    let high_throughput_config = CommandQueueConfig {
        max_queue_size: 5000,
        max_concurrent_executions: 20,
        batch_size: 10,
        batch_timeout: Duration::from_millis(50),
        cleanup_interval: Duration::from_secs(60),
        preserve_order: false, // Allow reordering for better performance
        ..Default::default()
    };

    println!("\n   🚀 High-Throughput Configuration:");
    println!(
        "      Max queue size: {}",
        high_throughput_config.max_queue_size
    );
    println!(
        "      Max concurrent: {}",
        high_throughput_config.max_concurrent_executions
    );
    println!("      Batch size: {}", high_throughput_config.batch_size);
    println!(
        "      Preserve order: {}",
        high_throughput_config.preserve_order
    );

    // Conservative configuration for critical systems
    let conservative_config = CommandQueueConfig {
        max_queue_size: 500,
        default_expiration: Duration::from_secs(7200), // 2 hours
        max_concurrent_executions: 3,
        batch_size: 2,
        batch_timeout: Duration::from_millis(500),
        max_retries: 5,
        preserve_order: true,
        ..Default::default()
    };

    println!("\n   🛡️  Conservative Configuration:");
    println!(
        "      Max queue size: {}",
        conservative_config.max_queue_size
    );
    println!(
        "      Expiration: {:?}",
        conservative_config.default_expiration
    );
    println!("      Max retries: {}", conservative_config.max_retries);
    println!(
        "      Preserve order: {}",
        conservative_config.preserve_order
    );

    // Demo 2: Command Priorities and Types
    println!("\n2️⃣  Command Priorities and Types");

    let commands = vec![
        (
            QueuedCommand::new(
                "device1".to_string(),
                "status".to_string(),
                "demo".to_string(),
            ),
            "Normal priority - routine operation",
        ),
        (
            QueuedCommand::new_high_priority(
                "device2".to_string(),
                "emergency_off".to_string(),
                "demo".to_string(),
            ),
            "High priority - important operation",
        ),
        (
            QueuedCommand::new_critical(
                "security1".to_string(),
                "alarm_arm".to_string(),
                "demo".to_string(),
            ),
            "Critical priority - security operation",
        ),
        (
            QueuedCommand::new(
                "device3".to_string(),
                "lights_on".to_string(),
                "demo".to_string(),
            )
            .with_expiration(Duration::from_secs(300)),
            "Normal priority with 5-minute expiration",
        ),
        (
            QueuedCommand::new(
                "device4".to_string(),
                "batch_operation".to_string(),
                "demo".to_string(),
            )
            .with_strategy(ExecutionStrategy::Batch { max_batch_size: 5 }),
            "Batch execution strategy",
        ),
        (
            QueuedCommand::new(
                "device5".to_string(),
                "retry_operation".to_string(),
                "demo".to_string(),
            )
            .with_strategy(ExecutionStrategy::Retry {
                max_retries: 3,
                backoff: Duration::from_millis(500),
            }),
            "Retry execution strategy",
        ),
    ];

    for (command, description) in commands {
        println!("   📋 {}: Priority={:?}", description, command.priority);
        if let Some(expires_at) = command.expires_at {
            println!("      Expires at: {expires_at:?}");
        }
        match command.strategy {
            ExecutionStrategy::Immediate => println!("      Strategy: Immediate execution"),
            ExecutionStrategy::Delayed { delay } => {
                println!("      Strategy: Delayed by {delay:?}")
            }
            ExecutionStrategy::Batch { max_batch_size } => {
                println!("      Strategy: Batch (max {max_batch_size})")
            }
            ExecutionStrategy::Retry {
                max_retries,
                backoff,
            } => println!(
                "      Strategy: Retry ({max_retries} attempts, {backoff:?} backoff)"
            ),
        }
    }

    // Demo 3: Command Queue Creation and Operations
    println!("\n3️⃣  Command Queue Operations");

    let queue = CommandQueue::with_config(default_config);
    queue.start().await?;

    println!("   ✅ Command queue created and started");

    // Add various commands to demonstrate priority ordering
    let low_cmd = QueuedCommand::new(
        "device1".to_string(),
        "low_priority".to_string(),
        "demo".to_string(),
    );
    let normal_cmd = QueuedCommand::new(
        "device2".to_string(),
        "normal_priority".to_string(),
        "demo".to_string(),
    );
    let high_cmd = QueuedCommand::new_high_priority(
        "device3".to_string(),
        "high_priority".to_string(),
        "demo".to_string(),
    );
    let critical_cmd = QueuedCommand::new_critical(
        "device4".to_string(),
        "critical_priority".to_string(),
        "demo".to_string(),
    );

    // Enqueue in random order
    queue.enqueue(normal_cmd).await?;
    queue.enqueue(low_cmd).await?;
    queue.enqueue(critical_cmd).await?;
    queue.enqueue(high_cmd).await?;

    println!("   📥 Enqueued 4 commands with different priorities");

    // Show queue statistics
    let stats = queue.get_statistics().await;
    println!("   📊 Queue Statistics:");
    println!("      Total queued: {}", stats.total_queued);
    println!("      Current size: {}", stats.current_queue_size);
    println!("      Queue utilization: {:.1}%", stats.queue_utilization);

    // Demonstrate priority-based dequeuing
    println!("\n   📤 Dequeuing commands (should be in priority order):");
    for i in 1..=4 {
        if let Some(command) = queue.dequeue().await {
            println!(
                "      {}. {} (Priority: {:?})",
                i, command.command, command.priority
            );
        }
    }

    // Demo 4: HTTP Client Integration
    println!("\n4️⃣  HTTP Client Integration");

    let config = LoxoneConfig {
        url: Url::parse("http://192.168.1.100")?,
        username: "demo_user".to_string(),
        verify_ssl: false,
        timeout: Duration::from_secs(30),
        max_retries: 3,
        max_connections: Some(10),
        #[cfg(feature = "websocket")]
        websocket: Default::default(),
        auth_method: AuthMethod::Token,
    };

    let credentials = LoxoneCredentials {
        username: "demo_user".to_string(),
        password: "demo_password".to_string(),
        api_key: None,
        #[cfg(feature = "crypto-openssl")]
        public_key: None,
    };

    match TokenHttpClient::new(config, credentials).await {
        Ok(mut client) => {
            println!("   ✅ HTTP client created successfully");

            // Enable command queuing
            let command_queue = Arc::new(CommandQueue::new());
            command_queue.start().await?;
            client.enable_command_queue(command_queue);

            println!("   ✅ Command queuing enabled for HTTP client");

            if client.has_command_queue() {
                println!("   ✅ Command queue is active");
            }

            // Simulate commands while disconnected
            println!("\n   🔌 Simulating commands while disconnected:");
            println!("      Commands will be queued instead of failing");

            // These would normally be queued since client is not connected
            println!("      • send_command('light1', 'on') -> Would be queued");
            println!(
                "      • send_command('security', 'arm') -> Would be queued with high priority"
            );
            println!("      • send_command('alarm', 'emergency') -> Would be queued with critical priority");

            // Show queue statistics
            if let Some(stats) = client.get_queue_stats().await {
                println!("   📊 Client Queue Statistics:");
                println!("      Total queued: {}", stats.total_queued);
                println!("      Current size: {}", stats.current_queue_size);
            }
        }
        Err(e) => println!("   ❌ Error creating client: {e}"),
    }

    // Demo 5: Command Expiration and Cleanup
    println!("\n5️⃣  Command Expiration and Cleanup");

    let cleanup_queue = CommandQueue::new();
    cleanup_queue.start().await?;

    // Add a command with very short expiration
    let short_lived_command = QueuedCommand::new(
        "device1".to_string(),
        "test".to_string(),
        "demo".to_string(),
    )
    .with_expiration(Duration::from_millis(100));

    cleanup_queue.enqueue(short_lived_command).await?;
    println!("   ⏰ Added command with 100ms expiration");

    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Cleanup expired commands
    let expired_count = cleanup_queue.cleanup_expired().await?;
    println!("   🧹 Cleaned up {expired_count} expired commands");

    let final_stats = cleanup_queue.get_statistics().await;
    println!(
        "   📊 After cleanup - Queue size: {}, Expired total: {}",
        final_stats.current_queue_size, final_stats.expired_commands
    );

    // Demo 6: Batch Execution Simulation
    println!("\n6️⃣  Batch Execution Simulation");

    let batch_queue = CommandQueue::with_config(CommandQueueConfig {
        batch_size: 3,
        ..Default::default()
    });
    batch_queue.start().await?;

    // Add multiple commands for batch processing
    for i in 1..=5 {
        let command = QueuedCommand::new(
            format!("device{i}"),
            format!("batch_cmd_{i}"),
            "demo".to_string(),
        );
        batch_queue.enqueue(command).await?;
    }

    println!("   📦 Added 5 commands for batch processing");

    // Simulate batch execution
    let executor = |command: QueuedCommand| {
        Box::pin(async move {
            // Simulate command execution
            println!(
                "      Executing: {} -> {}",
                command.device_uuid, command.command
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(serde_json::json!({"result": "success", "device": command.device_uuid}))
        })
    };

    let results = batch_queue.execute_batch(executor).await?;
    println!(
        "   ✅ Batch execution completed: {} commands processed",
        results.len()
    );

    for result in &results {
        println!(
            "      Command {}: {} in {:?}",
            result.command_id,
            if result.success {
                "✅ Success"
            } else {
                "❌ Failed"
            },
            result.duration
        );
    }

    // Demo 7: Recovery and Reconnection Scenario
    println!("\n7️⃣  Recovery and Reconnection Scenario");

    println!("   📖 Typical Usage Scenario:");
    println!("   1. Client starts and connects to Loxone Miniserver");
    println!("   2. Network connection is lost");
    println!("   3. Commands are queued instead of failing");
    println!("   4. Connection is restored");
    println!("   5. Queued commands are automatically executed");

    println!("\n   🔄 Implementation Steps:");
    println!("   • Enable command queue: client.enable_command_queue(queue)");
    println!("   • Commands during disconnection are queued with appropriate priority");
    println!("   • Reconnection triggers: client.process_command_queue()");
    println!("   • Commands are executed in priority order with retry logic");
    println!("   • Statistics and audit trail maintained throughout");

    println!("\n✨ Command Queue Benefits Summary:");
    println!("   • Reliable command delivery during network issues");
    println!("   • Priority-based execution for critical operations");
    println!("   • Configurable retry logic with exponential backoff");
    println!("   • Batch processing for performance optimization");
    println!("   • Automatic cleanup of expired commands");
    println!("   • Comprehensive statistics and monitoring");
    println!("   • Seamless integration with existing client code");

    println!("\n🔧 Integration Examples:");
    println!("   // Enable command queuing");
    println!("   let queue = Arc::new(CommandQueue::new());");
    println!("   queue.start().await?;");
    println!("   client.enable_command_queue(queue);");
    println!("   ");
    println!("   // Commands are automatically queued when offline");
    println!("   client.send_command(\"device-uuid\", \"command\").await?;");
    println!("   ");
    println!("   // Queue processing happens automatically on reconnection");
    println!("   client.connect().await?; // Triggers queue processing");

    Ok(())
}
