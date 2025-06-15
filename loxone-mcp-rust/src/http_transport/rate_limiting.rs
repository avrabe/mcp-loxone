//! Enhanced API rate limiting for HTTP transport
//!
//! This module provides sophisticated rate limiting with different tiers for different
//! endpoint types, burst handling, and adaptive rate limiting based on system load.

use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Rate limiting configuration for different endpoint tiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitTiers {
    /// High-frequency endpoints (tools/call, etc.)
    pub high_frequency: TierConfig,
    /// Medium-frequency endpoints (resources/read, prompts/get)
    pub medium_frequency: TierConfig,
    /// Low-frequency endpoints (tools/list, resources/list, prompts/list)
    pub low_frequency: TierConfig,
    /// Admin endpoints (health, status)
    pub admin: TierConfig,
    /// Global limits per client
    pub global: TierConfig,
}

/// Configuration for a specific rate limiting tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    /// Requests per minute
    pub requests_per_minute: u32,
    /// Burst capacity (requests allowed in quick succession)
    pub burst_capacity: u32,
    /// Window size for rate limiting (in seconds)
    pub window_seconds: u64,
    /// Penalty duration for rate limit violations (in seconds)
    pub penalty_duration_seconds: u64,
}

impl Default for RateLimitTiers {
    fn default() -> Self {
        Self {
            high_frequency: TierConfig {
                requests_per_minute: 60, // 1 per second sustained
                burst_capacity: 10,      // Allow 10 rapid requests
                window_seconds: 60,
                penalty_duration_seconds: 300, // 5 minute penalty
            },
            medium_frequency: TierConfig {
                requests_per_minute: 30, // 1 per 2 seconds sustained
                burst_capacity: 5,       // Allow 5 rapid requests
                window_seconds: 60,
                penalty_duration_seconds: 180, // 3 minute penalty
            },
            low_frequency: TierConfig {
                requests_per_minute: 10, // 1 per 6 seconds sustained
                burst_capacity: 3,       // Allow 3 rapid requests
                window_seconds: 60,
                penalty_duration_seconds: 60, // 1 minute penalty
            },
            admin: TierConfig {
                requests_per_minute: 20, // Admin endpoints
                burst_capacity: 5,
                window_seconds: 60,
                penalty_duration_seconds: 120, // 2 minute penalty
            },
            global: TierConfig {
                requests_per_minute: 100, // Total per client
                burst_capacity: 20,
                window_seconds: 60,
                penalty_duration_seconds: 600, // 10 minute penalty
            },
        }
    }
}

/// Rate limiting result
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// Request allowed
    Allowed { remaining: u32, reset_time: Instant },
    /// Request allowed but using burst capacity
    AllowedBurst { remaining: u32, reset_time: Instant },
    /// Request rate limited
    Limited {
        retry_after: Duration,
        limit_type: String,
    },
    /// Client is in penalty period
    Penalized {
        penalty_remaining: Duration,
        reason: String,
    },
}

/// Client tracking information
#[derive(Debug, Clone)]
struct ClientInfo {
    /// Request counts per tier
    tier_counts: HashMap<EndpointTier, RequestWindow>,
    /// Global request count
    global_count: RequestWindow,
    /// Penalty information
    penalty: Option<PenaltyInfo>,
    /// Client metadata
    metadata: ClientMetadata,
}

/// Request tracking window
#[derive(Debug, Clone)]
struct RequestWindow {
    /// Request timestamps within the window
    requests: Vec<Instant>,
    /// Burst tokens available
    burst_tokens: u32,
    /// Last request time
    last_request: Instant,
    /// Last token refresh time
    last_refresh: Instant,
}

/// Penalty information
#[derive(Debug, Clone)]
struct PenaltyInfo {
    /// When penalty started
    start_time: Instant,
    /// Duration of penalty
    duration: Duration,
    /// Reason for penalty
    reason: String,
    /// Violation count
    #[allow(dead_code)]
    violation_count: u32,
}

/// Client metadata for tracking and analytics
#[derive(Debug, Clone)]
struct ClientMetadata {
    /// First seen time
    first_seen: Instant,
    /// Total requests made
    total_requests: u64,
    /// Total violations
    total_violations: u32,
    /// Client identifier (IP, API key, etc.)
    identifier: String,
}

/// Endpoint tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndpointTier {
    HighFrequency,
    MediumFrequency,
    LowFrequency,
    Admin,
}

impl EndpointTier {
    /// Classify endpoint by method name
    pub fn from_method(method: &str) -> Self {
        match method {
            // High-frequency endpoints (actions)
            "tools/call" => Self::HighFrequency,

            // Medium-frequency endpoints (data with parameters)
            "resources/read" | "prompts/get" => Self::MediumFrequency,

            // Low-frequency endpoints (lists and discovery)
            "tools/list" | "resources/list" | "prompts/list" | "initialize" => Self::LowFrequency,

            // Admin endpoints
            "health" | "admin/status" => Self::Admin,

            // Default to medium frequency for unknown endpoints
            _ => Self::MediumFrequency,
        }
    }
}

/// Enhanced rate limiter with tier-based limiting
#[derive(Clone)]
pub struct EnhancedRateLimiter {
    /// Rate limiting configuration
    config: RateLimitTiers,
    /// Client tracking data
    clients: Arc<RwLock<HashMap<String, ClientInfo>>>,
    /// System load factor (affects rate limits dynamically)
    load_factor: Arc<RwLock<f64>>,
}

impl EnhancedRateLimiter {
    /// Create new enhanced rate limiter
    pub fn new(config: RateLimitTiers) -> Self {
        Self {
            config,
            clients: Arc::new(RwLock::new(HashMap::new())),
            load_factor: Arc::new(RwLock::new(1.0)),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(RateLimitTiers::default())
    }

    /// Check rate limit for a request
    pub async fn check_rate_limit(
        &self,
        client_id: &str,
        method: &str,
        _headers: &HeaderMap,
    ) -> RateLimitResult {
        let tier = EndpointTier::from_method(method);

        // Get or create client info
        let mut clients = self.clients.write().await;
        let client_info = clients
            .entry(client_id.to_string())
            .or_insert_with(|| ClientInfo {
                tier_counts: HashMap::new(),
                global_count: RequestWindow::new(),
                penalty: None,
                metadata: ClientMetadata {
                    first_seen: Instant::now(),
                    total_requests: 0,
                    total_violations: 0,
                    identifier: client_id.to_string(),
                },
            });

        // Check if client is in penalty period
        if let Some(penalty) = &client_info.penalty {
            let penalty_remaining = penalty
                .duration
                .saturating_sub(penalty.start_time.elapsed());
            if penalty_remaining > Duration::ZERO {
                return RateLimitResult::Penalized {
                    penalty_remaining,
                    reason: penalty.reason.clone(),
                };
            } else {
                // Penalty expired, remove it
                client_info.penalty = None;
            }
        }

        // Update client metadata
        client_info.metadata.total_requests += 1;

        // Get current load factor
        let load_factor = *self.load_factor.read().await;

        // Check tier-specific rate limit
        let tier_config = self.get_tier_config(tier);
        let tier_result = self
            .check_tier_limit(client_info, tier, tier_config, load_factor)
            .await;

        // Check global rate limit
        let global_result = self.check_global_limit(client_info, load_factor).await;

        // Determine final result (most restrictive wins)
        let result = match (&tier_result, &global_result) {
            (RateLimitResult::Limited { .. }, _) => tier_result,
            (_, RateLimitResult::Limited { .. }) => global_result,
            (RateLimitResult::AllowedBurst { .. }, RateLimitResult::Allowed { .. }) => tier_result,
            (RateLimitResult::Allowed { .. }, RateLimitResult::AllowedBurst { .. }) => {
                global_result
            }
            _ => tier_result, // Both allowed
        };

        // Handle rate limit violations
        if matches!(result, RateLimitResult::Limited { .. }) {
            self.handle_violation(client_info, tier, method).await;
        }

        result
    }

    /// Check tier-specific rate limit
    async fn check_tier_limit(
        &self,
        client_info: &mut ClientInfo,
        tier: EndpointTier,
        config: &TierConfig,
        load_factor: f64,
    ) -> RateLimitResult {
        let window = client_info
            .tier_counts
            .entry(tier)
            .or_insert_with(RequestWindow::new);
        self.check_window_limit(window, config, load_factor, format!("{:?}", tier))
            .await
    }

    /// Check global rate limit
    async fn check_global_limit(
        &self,
        client_info: &mut ClientInfo,
        load_factor: f64,
    ) -> RateLimitResult {
        self.check_window_limit(
            &mut client_info.global_count,
            &self.config.global,
            load_factor,
            "global".to_string(),
        )
        .await
    }

    /// Check rate limit for a specific window
    async fn check_window_limit(
        &self,
        window: &mut RequestWindow,
        config: &TierConfig,
        load_factor: f64,
        limit_type: String,
    ) -> RateLimitResult {
        let now = Instant::now();
        let window_duration = Duration::from_secs(config.window_seconds);

        // Adjust limits based on load factor
        let adjusted_limit = ((config.requests_per_minute as f64) / load_factor).max(1.0) as u32;
        let adjusted_burst = ((config.burst_capacity as f64) / load_factor).max(1.0) as u32;

        // Clean old requests outside the window
        window
            .requests
            .retain(|&req_time| now.duration_since(req_time) < window_duration);

        // Refresh burst tokens
        self.refresh_burst_tokens(window, config, now);

        // Check if within rate limit
        if window.requests.len() < adjusted_limit as usize {
            // Check if we can use burst capacity
            if window.burst_tokens > 0 {
                window.burst_tokens -= 1;
                window.requests.push(now);
                window.last_request = now;

                let remaining = adjusted_limit.saturating_sub(window.requests.len() as u32);
                let reset_time = now + window_duration;

                if window.burst_tokens < adjusted_burst / 2 {
                    RateLimitResult::AllowedBurst {
                        remaining,
                        reset_time,
                    }
                } else {
                    RateLimitResult::Allowed {
                        remaining,
                        reset_time,
                    }
                }
            } else {
                // No burst tokens, check regular rate
                window.requests.push(now);
                window.last_request = now;

                let remaining = adjusted_limit.saturating_sub(window.requests.len() as u32);
                let reset_time = now + window_duration;

                RateLimitResult::Allowed {
                    remaining,
                    reset_time,
                }
            }
        } else {
            // Rate limited
            let oldest_request = window.requests.first().copied().unwrap_or(now);
            let retry_after = window_duration.saturating_sub(now.duration_since(oldest_request));

            RateLimitResult::Limited {
                retry_after,
                limit_type,
            }
        }
    }

    /// Refresh burst tokens based on time elapsed
    fn refresh_burst_tokens(&self, window: &mut RequestWindow, config: &TierConfig, now: Instant) {
        let time_since_refresh = now.duration_since(window.last_refresh);

        // Prevent divide by zero - ensure requests_per_minute is at least 1
        let requests_per_minute = config.requests_per_minute.max(1);
        let token_refresh_interval = Duration::from_secs(60 / requests_per_minute as u64);

        if time_since_refresh >= token_refresh_interval && token_refresh_interval.as_secs() > 0 {
            let tokens_to_add =
                (time_since_refresh.as_secs() / token_refresh_interval.as_secs()) as u32;
            window.burst_tokens = (window.burst_tokens + tokens_to_add).min(config.burst_capacity);
            window.last_refresh = now;
        }
    }

    /// Handle rate limit violation
    async fn handle_violation(
        &self,
        client_info: &mut ClientInfo,
        tier: EndpointTier,
        method: &str,
    ) {
        client_info.metadata.total_violations += 1;

        let tier_config = self.get_tier_config(tier);
        let violation_count = client_info.metadata.total_violations;

        // Escalating penalties for repeat violations
        let penalty_duration = Duration::from_secs(
            tier_config.penalty_duration_seconds * (violation_count as u64).min(5),
        );

        let penalty = PenaltyInfo {
            start_time: Instant::now(),
            duration: penalty_duration,
            reason: format!(
                "Rate limit violation on {} (violation #{})",
                method, violation_count
            ),
            violation_count,
        };

        client_info.penalty = Some(penalty);

        warn!(
            client = %client_info.metadata.identifier,
            method = %method,
            violations = %violation_count,
            penalty_duration = ?penalty_duration,
            "Rate limit violation with penalty applied"
        );
    }

    /// Get configuration for a tier
    fn get_tier_config(&self, tier: EndpointTier) -> &TierConfig {
        match tier {
            EndpointTier::HighFrequency => &self.config.high_frequency,
            EndpointTier::MediumFrequency => &self.config.medium_frequency,
            EndpointTier::LowFrequency => &self.config.low_frequency,
            EndpointTier::Admin => &self.config.admin,
        }
    }

    /// Update system load factor (affects rate limits dynamically)
    pub async fn update_load_factor(&self, load: f64) {
        let mut load_factor = self.load_factor.write().await;
        *load_factor = load.clamp(0.1, 10.0); // Clamp between 0.1 and 10.0

        debug!(load_factor = %load, "Updated system load factor");
    }

    /// Get rate limiting statistics
    pub async fn get_statistics(&self) -> RateLimitStatistics {
        let clients = self.clients.read().await;
        let total_clients = clients.len();
        let penalized_clients = clients.values().filter(|c| c.penalty.is_some()).count();
        let total_requests: u64 = clients.values().map(|c| c.metadata.total_requests).sum();
        let total_violations: u32 = clients.values().map(|c| c.metadata.total_violations).sum();

        RateLimitStatistics {
            total_clients,
            penalized_clients,
            total_requests,
            total_violations,
            load_factor: *self.load_factor.read().await,
        }
    }

    /// Clean up old client data
    pub async fn cleanup_expired_clients(&self) {
        let mut clients = self.clients.write().await;
        let now = Instant::now();
        let cleanup_threshold = Duration::from_secs(24 * 3600); // 24 hours

        clients.retain(|_id, client| {
            now.duration_since(client.metadata.first_seen) < cleanup_threshold
        });

        debug!(
            remaining_clients = clients.len(),
            "Cleaned up expired client data"
        );
    }

    /// Extract client identifier from request headers
    pub fn extract_client_id(headers: &HeaderMap) -> String {
        // Try to get client ID from various headers
        if let Some(api_key) = headers.get("authorization") {
            if let Ok(auth_str) = api_key.to_str() {
                if let Some(stripped) = auth_str.strip_prefix("Bearer ") {
                    return format!("api_key_{}", &stripped.chars().take(8).collect::<String>());
                }
            }
        }

        if let Some(forwarded_for) = headers.get("x-forwarded-for") {
            if let Ok(ip) = forwarded_for.to_str() {
                return format!("ip_{}", ip.split(',').next().unwrap_or(ip).trim());
            }
        }

        if let Some(real_ip) = headers.get("x-real-ip") {
            if let Ok(ip) = real_ip.to_str() {
                return format!("ip_{}", ip);
            }
        }

        // Fallback to a default identifier
        "unknown_client".to_string()
    }
}

impl RequestWindow {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            requests: Vec::new(),
            burst_tokens: 0, // Will be set during first refresh
            last_request: now,
            last_refresh: now,
        }
    }
}

/// Rate limiting statistics
#[derive(Debug, Serialize)]
pub struct RateLimitStatistics {
    pub total_clients: usize,
    pub penalized_clients: usize,
    pub total_requests: u64,
    pub total_violations: u32,
    pub load_factor: f64,
}

/// Rate limit information for HTTP headers
#[derive(Debug)]
pub struct RateLimitHeaders {
    pub limit: u32,
    pub remaining: u32,
    pub reset: u64,
    pub retry_after: Option<u64>,
}

impl RateLimitHeaders {
    /// Convert to HTTP headers
    pub fn to_headers(&self) -> Vec<(String, String)> {
        let mut headers = vec![
            ("X-RateLimit-Limit".to_string(), self.limit.to_string()),
            (
                "X-RateLimit-Remaining".to_string(),
                self.remaining.to_string(),
            ),
            ("X-RateLimit-Reset".to_string(), self.reset.to_string()),
        ];

        if let Some(retry_after) = self.retry_after {
            headers.push(("Retry-After".to_string(), retry_after.to_string()));
        }

        headers
    }
}
