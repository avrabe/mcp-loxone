//! Example demonstrating the rate limiting implementation
//!
//! This example shows how the rate limiter protects against abuse
//! and manages burst capacity for different client identification strategies.

use loxone_mcp_rust::server::rate_limiter::{
    RateLimitConfig, RateLimitMiddleware, RateLimitResult,
};
use std::time::Duration;
use tracing::{info, warn, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🛡️ Rate limiting demonstration");

    // Create a restrictive rate limiter for demonstration
    let config = RateLimitConfig {
        max_requests: 3,                          // Only 3 requests
        window_duration: Duration::from_secs(10), // per 10 seconds
        burst_size: 2,                            // allow 2 burst requests
        cleanup_interval: Duration::from_secs(60),
    };

    let rate_limiter = RateLimitMiddleware::new(config);
    info!("✅ Rate limiter created with 3 requests per 10 seconds + 2 burst");

    // Example 1: Basic rate limiting
    info!("\n🔄 Example 1: Basic rate limiting");
    let client_ip = "192.168.1.100";

    for i in 1..=7 {
        let result = rate_limiter.check_ip(client_ip).await;
        match result {
            RateLimitResult::Allowed => {
                info!("✅ Request {}: Allowed", i);
            }
            RateLimitResult::AllowedBurst => {
                warn!("⚡ Request {}: Allowed using burst capacity", i);
            }
            RateLimitResult::Limited { reset_at } => {
                warn!("🚫 Request {}: Rate limited (resets at {:?})", i, reset_at);
            }
        }

        // Small delay between requests
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // Example 2: Multiple client identification strategies
    info!("\n🔄 Example 2: Different client identification");

    // User agent based limiting
    let result1 = rate_limiter.check_user_agent("n8n/1.0").await;
    info!("User agent 'n8n/1.0': {:?}", result1);

    let result2 = rate_limiter.check_user_agent("claude-desktop/1.0").await;
    info!("User agent 'claude-desktop/1.0': {:?}", result2);

    // Tool-based limiting
    let result3 = rate_limiter.check_tool("client-123", "list_rooms").await;
    info!("Tool 'list_rooms' for client-123: {:?}", result3);

    let result4 = rate_limiter
        .check_tool("client-123", "control_device")
        .await;
    info!("Tool 'control_device' for client-123: {:?}", result4);

    // Composite limiting (IP + User Agent)
    let result5 = rate_limiter
        .check_composite("10.0.0.1", Some("postman/1.0"))
        .await;
    info!("Composite check (IP + UA): {:?}", result5);

    // Example 3: Statistics
    info!("\n📊 Example 3: Rate limiter statistics");
    let stats = rate_limiter.get_stats().await;
    info!("Statistics: {:?}", stats);
    info!(
        "Active clients: {}, Total requests: {}, Burst used: {}, Total buckets: {}",
        stats.active_clients, stats.total_requests, stats.burst_requests, stats.total_buckets
    );

    info!("\n✅ Rate limiting demonstration complete");
    Ok(())
}
