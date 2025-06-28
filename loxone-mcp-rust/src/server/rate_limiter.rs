//! Rate limiting for MCP server to protect against abuse
//!
//! This module provides configurable rate limiting based on client IP,
//! user agent, and request patterns to prevent abuse and ensure fair usage.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,

    /// Time window for rate limiting
    pub window_duration: Duration,

    /// Burst allowance (requests that can exceed the limit temporarily)
    pub burst_size: u32,

    /// Cleanup interval for expired entries
    pub cleanup_interval: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,                          // 100 requests
            window_duration: Duration::from_secs(60),   // per minute
            burst_size: 10,                             // allow 10 burst requests
            cleanup_interval: Duration::from_secs(300), // cleanup every 5 minutes
        }
    }
}

/// Rate limit bucket for tracking requests
#[derive(Debug, Clone)]
struct RateLimitBucket {
    /// Number of requests in current window
    request_count: u32,

    /// Number of burst requests used
    burst_used: u32,

    /// Window start time
    window_start: Instant,

    /// Last request time
    last_request: Instant,
}

impl RateLimitBucket {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            request_count: 0,
            burst_used: 0,
            window_start: now,
            last_request: now,
        }
    }

    fn reset_window(&mut self, now: Instant) {
        self.request_count = 0;
        self.burst_used = 0;
        self.window_start = now;
    }

    fn is_window_expired(&self, now: Instant, window_duration: Duration) -> bool {
        now.duration_since(self.window_start) >= window_duration
    }
}

/// Rate limiter implementation
pub struct RateLimiter {
    config: RateLimitConfig,
    buckets: Arc<RwLock<HashMap<String, RateLimitBucket>>>,
    last_cleanup: Arc<RwLock<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter with default config
    pub fn new() -> Self {
        Self::with_config(RateLimitConfig::default())
    }

    /// Create a new rate limiter with custom config
    pub fn with_config(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(RwLock::new(HashMap::new())),
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Check if a request should be allowed
    pub async fn check_request(&self, client_id: &str) -> RateLimitResult {
        let now = Instant::now();

        // Check if cleanup is needed
        self.maybe_cleanup(now).await;

        let mut buckets = self.buckets.write().await;
        let bucket = buckets
            .entry(client_id.to_string())
            .or_insert_with(RateLimitBucket::new);

        // Reset window if expired
        if bucket.is_window_expired(now, self.config.window_duration) {
            bucket.reset_window(now);
        }

        bucket.last_request = now;

        // Check if request is allowed
        if bucket.request_count < self.config.max_requests {
            bucket.request_count += 1;
            debug!(
                client_id = client_id,
                request_count = bucket.request_count,
                max_requests = self.config.max_requests,
                "Request allowed"
            );
            RateLimitResult::Allowed
        } else if bucket.burst_used < self.config.burst_size {
            bucket.burst_used += 1;
            warn!(
                client_id = client_id,
                burst_used = bucket.burst_used,
                burst_size = self.config.burst_size,
                "Request allowed (burst)"
            );
            RateLimitResult::AllowedBurst
        } else {
            let reset_time = bucket.window_start + self.config.window_duration;
            warn!(
                client_id = client_id,
                request_count = bucket.request_count,
                max_requests = self.config.max_requests,
                "Request rate limited"
            );
            RateLimitResult::Limited {
                reset_at: reset_time,
            }
        }
    }

    /// Get current rate limit status for a client
    pub async fn get_status(&self, client_id: &str) -> Option<RateLimitStatus> {
        let buckets = self.buckets.read().await;
        let bucket = buckets.get(client_id)?;

        let now = Instant::now();
        if bucket.is_window_expired(now, self.config.window_duration) {
            return None; // Expired bucket
        }

        Some(RateLimitStatus {
            requests_made: bucket.request_count,
            max_requests: self.config.max_requests,
            burst_used: bucket.burst_used,
            max_burst: self.config.burst_size,
            window_start: bucket.window_start,
            window_duration: self.config.window_duration,
            reset_at: bucket.window_start + self.config.window_duration,
        })
    }

    /// Get rate limiter statistics
    pub async fn get_statistics(&self) -> RateLimiterStats {
        let buckets = self.buckets.read().await;
        let now = Instant::now();

        let mut active_clients = 0;
        let mut total_requests = 0;
        let mut burst_requests = 0;

        for bucket in buckets.values() {
            if !bucket.is_window_expired(now, self.config.window_duration) {
                active_clients += 1;
                total_requests += bucket.request_count;
                burst_requests += bucket.burst_used;
            }
        }

        RateLimiterStats {
            active_clients,
            total_requests,
            burst_requests,
            total_buckets: buckets.len(),
        }
    }

    /// Cleanup expired buckets
    async fn maybe_cleanup(&self, now: Instant) {
        let mut last_cleanup = self.last_cleanup.write().await;
        if now.duration_since(*last_cleanup) < self.config.cleanup_interval {
            return;
        }

        let mut buckets = self.buckets.write().await;
        let initial_count = buckets.len();

        buckets.retain(|_client_id, bucket| {
            !bucket.is_window_expired(now, self.config.window_duration * 2) // Keep for 2x window
        });

        let cleaned = initial_count - buckets.len();
        if cleaned > 0 {
            debug!(
                cleaned_buckets = cleaned,
                remaining_buckets = buckets.len(),
                "Cleaned up expired rate limit buckets"
            );
        }

        *last_cleanup = now;
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a rate limit check
#[derive(Debug, PartialEq)]
pub enum RateLimitResult {
    /// Request is allowed
    Allowed,

    /// Request is allowed using burst capacity
    AllowedBurst,

    /// Request is rate limited
    Limited { reset_at: Instant },
}

/// Current rate limit status for a client
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    pub requests_made: u32,
    pub max_requests: u32,
    pub burst_used: u32,
    pub max_burst: u32,
    pub window_start: Instant,
    pub window_duration: Duration,
    pub reset_at: Instant,
}

/// Rate limiter statistics
#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    pub active_clients: usize,
    pub total_requests: u32,
    pub burst_requests: u32,
    pub total_buckets: usize,
}

/// Rate limiting middleware for different client identification strategies
pub struct RateLimitMiddleware {
    limiter: RateLimiter,
}

impl RateLimitMiddleware {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            limiter: RateLimiter::with_config(config),
        }
    }

    /// Check rate limit based on IP address
    pub async fn check_ip(&self, ip: &str) -> RateLimitResult {
        self.limiter.check_request(&format!("ip:{ip}")).await
    }

    /// Check rate limit based on user agent
    pub async fn check_user_agent(&self, user_agent: &str) -> RateLimitResult {
        let normalized_ua = self.normalize_user_agent(user_agent);
        self.limiter
            .check_request(&format!("ua:{normalized_ua}"))
            .await
    }

    /// Check rate limit based on tool name (per-tool limiting)
    pub async fn check_tool(&self, client_id: &str, tool_name: &str) -> RateLimitResult {
        self.limiter
            .check_request(&format!("tool:{client_id}:{tool_name}"))
            .await
    }

    /// Check rate limit with composite key
    pub async fn check_composite(&self, ip: &str, user_agent: Option<&str>) -> RateLimitResult {
        let key = match user_agent {
            Some(ua) => format!("composite:{ip}:{}", self.normalize_user_agent(ua)),
            None => format!("composite:{ip}"),
        };
        self.limiter.check_request(&key).await
    }

    /// Get statistics
    pub async fn get_stats(&self) -> RateLimiterStats {
        self.limiter.get_statistics().await
    }

    fn normalize_user_agent(&self, user_agent: &str) -> String {
        // Extract just the main application name to group similar clients
        if let Some(main_part) = user_agent.split('/').next() {
            main_part.to_lowercase()
        } else {
            user_agent.to_lowercase()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_rate_limit_basic() {
        let config = RateLimitConfig {
            max_requests: 2,
            window_duration: Duration::from_secs(1),
            burst_size: 1,
            cleanup_interval: Duration::from_secs(60),
        };

        let limiter = RateLimiter::with_config(config);

        // First two requests should be allowed
        assert_eq!(
            limiter.check_request("client1").await,
            RateLimitResult::Allowed
        );
        assert_eq!(
            limiter.check_request("client1").await,
            RateLimitResult::Allowed
        );

        // Third request should use burst
        assert_eq!(
            limiter.check_request("client1").await,
            RateLimitResult::AllowedBurst
        );

        // Fourth request should be limited
        if let RateLimitResult::Limited { .. } = limiter.check_request("client1").await {
            // Expected
        } else {
            panic!("Expected rate limit");
        }
    }

    #[tokio::test]
    async fn test_rate_limit_window_reset() {
        let config = RateLimitConfig {
            max_requests: 1,
            window_duration: Duration::from_millis(100),
            burst_size: 0,
            cleanup_interval: Duration::from_secs(60),
        };

        let limiter = RateLimiter::with_config(config);

        // First request allowed
        assert_eq!(
            limiter.check_request("client1").await,
            RateLimitResult::Allowed
        );

        // Second request limited
        if let RateLimitResult::Limited { .. } = limiter.check_request("client1").await {
            // Expected
        } else {
            panic!("Expected rate limit");
        }

        // Wait for window to reset
        sleep(Duration::from_millis(150)).await;

        // Should be allowed again
        assert_eq!(
            limiter.check_request("client1").await,
            RateLimitResult::Allowed
        );
    }

    #[tokio::test]
    async fn test_multiple_clients() {
        let limiter = RateLimiter::new();

        // Different clients should have separate limits
        assert_eq!(
            limiter.check_request("client1").await,
            RateLimitResult::Allowed
        );
        assert_eq!(
            limiter.check_request("client2").await,
            RateLimitResult::Allowed
        );

        let stats = limiter.get_statistics().await;
        assert_eq!(stats.active_clients, 2);
    }
}
