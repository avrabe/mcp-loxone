//! Enhanced rate limiting for production security

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    pub enabled: bool,
    /// Global rate limits
    pub global_limits: RateLimits,
    /// Per-endpoint rate limits
    pub endpoint_limits: HashMap<String, RateLimits>,
    /// Per-method rate limits (for MCP methods)
    pub method_limits: HashMap<String, RateLimits>,
    /// IP-based rate limiting
    pub ip_based: bool,
    /// Client ID-based rate limiting
    pub client_based: bool,
    /// Burst capacity multiplier
    pub burst_multiplier: f64,
    /// Penalty configuration
    pub penalty_config: PenaltyConfig,
    /// Whitelist of IPs/clients that bypass rate limiting
    pub whitelist: Whitelist,
}

/// Rate limit configuration for a specific scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    /// Requests per minute
    pub requests_per_minute: u32,
    /// Requests per hour
    pub requests_per_hour: Option<u32>,
    /// Requests per day
    pub requests_per_day: Option<u32>,
    /// Burst capacity (short-term allowance)
    pub burst_capacity: u32,
    /// Recovery rate (tokens per second)
    pub recovery_rate: f64,
}

/// Penalty configuration for rate limit violations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PenaltyConfig {
    /// Enable penalties for violations
    pub enabled: bool,
    /// Penalty duration for first violation
    pub first_violation_penalty: Duration,
    /// Penalty duration for repeated violations
    pub repeated_violation_penalty: Duration,
    /// Number of violations before escalating penalty
    pub escalation_threshold: u32,
    /// Maximum penalty duration
    pub max_penalty_duration: Duration,
    /// Cooldown period before resetting violation count
    pub cooldown_period: Duration,
}

/// Whitelist configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Whitelist {
    /// Whitelisted IP addresses
    pub ip_addresses: Vec<String>,
    /// Whitelisted client IDs
    pub client_ids: Vec<String>,
    /// Whitelisted API keys
    pub api_keys: Vec<String>,
    /// Whitelist patterns (regex)
    pub patterns: Vec<String>,
}

impl RateLimitConfig {
    /// Create production rate limiting configuration
    pub fn production() -> Self {
        let mut endpoint_limits = HashMap::new();

        // High-frequency endpoints
        endpoint_limits.insert(
            "/health".to_string(),
            RateLimits {
                requests_per_minute: 60,
                requests_per_hour: Some(1000),
                requests_per_day: None,
                burst_capacity: 10,
                recovery_rate: 1.0,
            },
        );

        // MCP message endpoint
        endpoint_limits.insert(
            "/message".to_string(),
            RateLimits {
                requests_per_minute: 30,
                requests_per_hour: Some(500),
                requests_per_day: Some(5000),
                burst_capacity: 5,
                recovery_rate: 0.5,
            },
        );

        // SSE endpoints
        endpoint_limits.insert(
            "/sse".to_string(),
            RateLimits {
                requests_per_minute: 10,
                requests_per_hour: Some(60),
                requests_per_day: Some(500),
                burst_capacity: 2,
                recovery_rate: 0.2,
            },
        );

        let mut method_limits = HashMap::new();

        // Resource-intensive methods
        method_limits.insert(
            "tools/call".to_string(),
            RateLimits {
                requests_per_minute: 20,
                requests_per_hour: Some(300),
                requests_per_day: Some(3000),
                burst_capacity: 3,
                recovery_rate: 0.3,
            },
        );

        // Sampling/LLM methods (expensive)
        method_limits.insert(
            "sampling/createMessage".to_string(),
            RateLimits {
                requests_per_minute: 5,
                requests_per_hour: Some(50),
                requests_per_day: Some(500),
                burst_capacity: 1,
                recovery_rate: 0.1,
            },
        );

        Self {
            enabled: true,
            global_limits: RateLimits {
                requests_per_minute: 100,
                requests_per_hour: Some(2000),
                requests_per_day: Some(20000),
                burst_capacity: 20,
                recovery_rate: 2.0,
            },
            endpoint_limits,
            method_limits,
            ip_based: true,
            client_based: true,
            burst_multiplier: 1.5,
            penalty_config: PenaltyConfig {
                enabled: true,
                first_violation_penalty: Duration::from_secs(60), // 1 minute
                repeated_violation_penalty: Duration::from_secs(600), // 10 minutes
                escalation_threshold: 3,
                max_penalty_duration: Duration::from_secs(3600), // 1 hour
                cooldown_period: Duration::from_secs(1800),      // 30 minutes
            },
            whitelist: Whitelist {
                ip_addresses: vec!["127.0.0.1".to_string()], // Localhost always whitelisted
                client_ids: Vec::new(),
                api_keys: Vec::new(),
                patterns: Vec::new(),
            },
        }
    }

    /// Create development rate limiting configuration
    pub fn development() -> Self {
        Self {
            enabled: true,
            global_limits: RateLimits {
                requests_per_minute: 1000,
                requests_per_hour: None,
                requests_per_day: None,
                burst_capacity: 100,
                recovery_rate: 10.0,
            },
            endpoint_limits: HashMap::new(),
            method_limits: HashMap::new(),
            ip_based: true,
            client_based: false,
            burst_multiplier: 2.0,
            penalty_config: PenaltyConfig {
                enabled: false,
                first_violation_penalty: Duration::from_secs(10),
                repeated_violation_penalty: Duration::from_secs(30),
                escalation_threshold: 5,
                max_penalty_duration: Duration::from_secs(300),
                cooldown_period: Duration::from_secs(300),
            },
            whitelist: Whitelist {
                ip_addresses: vec!["127.0.0.1".to_string(), "::1".to_string()],
                client_ids: Vec::new(),
                api_keys: Vec::new(),
                patterns: vec!["^localhost".to_string()],
            },
        }
    }

    /// Create testing rate limiting configuration (minimal restrictions)
    pub fn testing() -> Self {
        Self {
            enabled: false,
            global_limits: RateLimits {
                requests_per_minute: 10000,
                requests_per_hour: None,
                requests_per_day: None,
                burst_capacity: 1000,
                recovery_rate: 100.0,
            },
            endpoint_limits: HashMap::new(),
            method_limits: HashMap::new(),
            ip_based: false,
            client_based: false,
            burst_multiplier: 10.0,
            penalty_config: PenaltyConfig {
                enabled: false,
                first_violation_penalty: Duration::from_secs(1),
                repeated_violation_penalty: Duration::from_secs(1),
                escalation_threshold: 100,
                max_penalty_duration: Duration::from_secs(1),
                cooldown_period: Duration::from_secs(1),
            },
            whitelist: Whitelist {
                ip_addresses: Vec::new(),
                client_ids: Vec::new(),
                api_keys: Vec::new(),
                patterns: vec![".*".to_string()], // Match all
            },
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate global limits
        self.validate_limits(&self.global_limits, "global")?;

        // Validate endpoint limits
        for (endpoint, limits) in &self.endpoint_limits {
            self.validate_limits(limits, endpoint)?;
        }

        // Validate method limits
        for (method, limits) in &self.method_limits {
            self.validate_limits(limits, method)?;
        }

        // Validate penalty configuration
        if self.penalty_config.enabled {
            if self.penalty_config.first_violation_penalty
                > self.penalty_config.max_penalty_duration
            {
                return Err(LoxoneError::invalid_input(
                    "First violation penalty cannot exceed max penalty duration",
                ));
            }
            if self.penalty_config.repeated_violation_penalty
                > self.penalty_config.max_penalty_duration
            {
                return Err(LoxoneError::invalid_input(
                    "Repeated violation penalty cannot exceed max penalty duration",
                ));
            }
        }

        // Validate burst multiplier
        if self.burst_multiplier < 1.0 {
            return Err(LoxoneError::invalid_input(
                "Burst multiplier must be at least 1.0",
            ));
        }

        Ok(())
    }

    /// Validate rate limits
    fn validate_limits(&self, limits: &RateLimits, context: &str) -> Result<()> {
        if limits.requests_per_minute == 0 {
            return Err(LoxoneError::invalid_input(format!(
                "Requests per minute cannot be 0 for {context}"
            )));
        }

        if limits.burst_capacity == 0 {
            return Err(LoxoneError::invalid_input(format!(
                "Burst capacity cannot be 0 for {context}"
            )));
        }

        if limits.recovery_rate <= 0.0 {
            return Err(LoxoneError::invalid_input(format!(
                "Recovery rate must be positive for {context}"
            )));
        }

        // Check consistency between time windows
        if let Some(per_hour) = limits.requests_per_hour {
            if per_hour < limits.requests_per_minute {
                return Err(LoxoneError::invalid_input(format!(
                    "Hourly limit cannot be less than per-minute limit for {context}"
                )));
            }
        }

        if let Some(per_day) = limits.requests_per_day {
            if let Some(per_hour) = limits.requests_per_hour {
                if per_day < per_hour {
                    return Err(LoxoneError::invalid_input(format!(
                        "Daily limit cannot be less than hourly limit for {context}"
                    )));
                }
            }
        }

        Ok(())
    }

    /// Check if rate limiting is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get effective limits for an endpoint
    pub fn get_endpoint_limits(&self, endpoint: &str) -> &RateLimits {
        self.endpoint_limits
            .get(endpoint)
            .unwrap_or(&self.global_limits)
    }

    /// Get effective limits for a method
    pub fn get_method_limits(&self, method: &str) -> &RateLimits {
        self.method_limits
            .get(method)
            .unwrap_or(&self.global_limits)
    }

    /// Check if identifier is whitelisted
    pub fn is_whitelisted(&self, identifier: &str, identifier_type: WhitelistType) -> bool {
        match identifier_type {
            WhitelistType::IpAddress => self
                .whitelist
                .ip_addresses
                .contains(&identifier.to_string()),
            WhitelistType::ClientId => self.whitelist.client_ids.contains(&identifier.to_string()),
            WhitelistType::ApiKey => self.whitelist.api_keys.contains(&identifier.to_string()),
        }
    }
}

/// Whitelist identifier type
#[derive(Debug, Clone)]
pub enum WhitelistType {
    /// IP address
    IpAddress,
    /// Client ID
    ClientId,
    /// API key
    ApiKey,
}

/// Rate limit bucket for tracking requests
#[derive(Debug, Clone)]
pub struct RateLimitBucket {
    /// Available tokens
    pub tokens: f64,
    /// Last update time
    pub last_update: std::time::Instant,
    /// Window counters for different time periods
    pub window_counters: WindowCounters,
    /// Violation count
    pub violations: u32,
    /// Last violation time
    pub last_violation: Option<std::time::Instant>,
    /// Penalty expiry time
    pub penalty_until: Option<std::time::Instant>,
}

/// Window counters for different time periods
#[derive(Debug, Clone)]
pub struct WindowCounters {
    /// Minute window
    pub minute: SlidingWindow,
    /// Hour window
    pub hour: SlidingWindow,
    /// Day window
    pub day: SlidingWindow,
}

/// Sliding window counter
#[derive(Debug, Clone)]
pub struct SlidingWindow {
    /// Window duration
    pub duration: Duration,
    /// Request timestamps
    pub requests: Vec<std::time::Instant>,
}

impl SlidingWindow {
    /// Create new sliding window
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            requests: Vec::new(),
        }
    }

    /// Add a request and clean old entries
    pub fn add_request(&mut self, now: std::time::Instant) {
        self.requests.push(now);
        self.cleanup(now);
    }

    /// Get current count
    pub fn count(&mut self, now: std::time::Instant) -> usize {
        self.cleanup(now);
        self.requests.len()
    }

    /// Clean up old entries
    fn cleanup(&mut self, now: std::time::Instant) {
        let cutoff = now - self.duration;
        self.requests.retain(|&timestamp| timestamp > cutoff);
    }
}

impl Default for WindowCounters {
    fn default() -> Self {
        Self {
            minute: SlidingWindow::new(Duration::from_secs(60)),
            hour: SlidingWindow::new(Duration::from_secs(3600)),
            day: SlidingWindow::new(Duration::from_secs(86400)),
        }
    }
}

impl RateLimitBucket {
    /// Create new rate limit bucket
    pub fn new(initial_tokens: f64) -> Self {
        Self {
            tokens: initial_tokens,
            last_update: std::time::Instant::now(),
            window_counters: WindowCounters::default(),
            violations: 0,
            last_violation: None,
            penalty_until: None,
        }
    }

    /// Check if request is allowed
    pub fn check_request(
        &mut self,
        limits: &RateLimits,
        penalty_config: &PenaltyConfig,
    ) -> RateLimitResult {
        let now = std::time::Instant::now();

        // Check if under penalty
        if let Some(penalty_until) = self.penalty_until {
            if now < penalty_until {
                return RateLimitResult::Penalized {
                    until: penalty_until,
                    reason: "Rate limit violations".to_string(),
                };
            } else {
                // Penalty expired
                self.penalty_until = None;
            }
        }

        // Update tokens based on recovery rate
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        self.tokens =
            (self.tokens + elapsed * limits.recovery_rate).min(limits.burst_capacity as f64);
        self.last_update = now;

        // Check token bucket
        if self.tokens < 1.0 {
            self.record_violation(now, penalty_config);
            return RateLimitResult::Limited {
                retry_after: Duration::from_secs_f64((1.0 - self.tokens) / limits.recovery_rate),
                limit_type: "burst".to_string(),
            };
        }

        // Check sliding windows
        self.window_counters.minute.add_request(now);
        self.window_counters.hour.add_request(now);
        self.window_counters.day.add_request(now);

        // Check minute limit
        if self.window_counters.minute.count(now) >= limits.requests_per_minute as usize {
            self.record_violation(now, penalty_config);
            return RateLimitResult::Limited {
                retry_after: Duration::from_secs(60),
                limit_type: "minute".to_string(),
            };
        }

        // Check hour limit
        if let Some(hour_limit) = limits.requests_per_hour {
            if self.window_counters.hour.count(now) >= hour_limit as usize {
                self.record_violation(now, penalty_config);
                return RateLimitResult::Limited {
                    retry_after: Duration::from_secs(3600),
                    limit_type: "hour".to_string(),
                };
            }
        }

        // Check day limit
        if let Some(day_limit) = limits.requests_per_day {
            if self.window_counters.day.count(now) >= day_limit as usize {
                self.record_violation(now, penalty_config);
                return RateLimitResult::Limited {
                    retry_after: Duration::from_secs(86400),
                    limit_type: "day".to_string(),
                };
            }
        }

        // Request allowed
        self.tokens -= 1.0;

        RateLimitResult::Allowed {
            remaining_tokens: self.tokens as u32,
            reset_after: Duration::from_secs(60),
        }
    }

    /// Record a violation
    fn record_violation(&mut self, now: std::time::Instant, penalty_config: &PenaltyConfig) {
        if !penalty_config.enabled {
            return;
        }

        // Check if we should reset violation count
        if let Some(last_violation) = self.last_violation {
            if now.duration_since(last_violation) > penalty_config.cooldown_period {
                self.violations = 0;
            }
        }

        self.violations += 1;
        self.last_violation = Some(now);

        // Apply penalty
        let penalty_duration = if self.violations >= penalty_config.escalation_threshold {
            penalty_config.repeated_violation_penalty
        } else {
            penalty_config.first_violation_penalty
        };

        let penalty_duration = penalty_duration.min(penalty_config.max_penalty_duration);
        self.penalty_until = Some(now + penalty_duration);
    }
}

/// Rate limit check result
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// Request allowed
    Allowed {
        /// Remaining tokens
        remaining_tokens: u32,
        /// Time until token reset
        reset_after: Duration,
    },
    /// Request rate limited
    Limited {
        /// Time to retry
        retry_after: Duration,
        /// Which limit was exceeded
        limit_type: String,
    },
    /// Client is penalized
    Penalized {
        /// Penalty end time
        until: std::time::Instant,
        /// Reason for penalty
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_config() {
        let config = RateLimitConfig::production();
        assert!(config.validate().is_ok());
        assert!(config.is_enabled());
    }

    #[test]
    fn test_sliding_window() {
        let mut window = SlidingWindow::new(Duration::from_secs(60));
        let now = std::time::Instant::now();

        window.add_request(now);
        window.add_request(now);

        assert_eq!(window.count(now), 2);

        // Test cleanup
        let old_time = now - Duration::from_secs(120);
        window.requests.insert(0, old_time);
        assert_eq!(window.count(now), 2); // Old request should be cleaned up
    }

    #[test]
    fn test_rate_limit_bucket() {
        let limits = RateLimits {
            requests_per_minute: 10,
            requests_per_hour: Some(100),
            requests_per_day: None,
            burst_capacity: 5,
            recovery_rate: 1.0,
        };

        let penalty_config = PenaltyConfig {
            enabled: false,
            first_violation_penalty: Duration::from_secs(60),
            repeated_violation_penalty: Duration::from_secs(600),
            escalation_threshold: 3,
            max_penalty_duration: Duration::from_secs(3600),
            cooldown_period: Duration::from_secs(1800),
        };

        let mut bucket = RateLimitBucket::new(5.0);

        // First few requests should succeed
        for _ in 0..5 {
            match bucket.check_request(&limits, &penalty_config) {
                RateLimitResult::Allowed { .. } => (),
                _ => panic!("Request should be allowed"),
            }
        }

        // Next request should be limited
        match bucket.check_request(&limits, &penalty_config) {
            RateLimitResult::Limited { .. } => (),
            _ => panic!("Request should be limited"),
        }
    }

    #[test]
    fn test_whitelist() {
        let config = RateLimitConfig::production();
        assert!(config.is_whitelisted("127.0.0.1", WhitelistType::IpAddress));
        assert!(!config.is_whitelisted("192.168.1.1", WhitelistType::IpAddress));
    }
}
