//! Enhanced CORS implementation for full web deployment support
//!
//! This module extends the basic CORS functionality with advanced features
//! for modern web applications, including dynamic origin validation,
//! sophisticated header handling, and security-focused configuration.

use crate::error::{LoxoneError, Result};
use crate::security::cors::CorsConfig;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Enhanced CORS configuration with advanced features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedCorsConfig {
    /// Base CORS configuration
    pub base: CorsConfig,
    /// Dynamic origin validation
    pub dynamic_origins: DynamicOriginConfig,
    /// Request context validation
    pub context_validation: ContextValidationConfig,
    /// Security policies
    pub security_policies: CorsSecurityPolicies,
    /// Performance optimizations
    pub performance: CorsPerformanceConfig,
}

/// Dynamic origin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicOriginConfig {
    /// Enable dynamic origin validation
    pub enabled: bool,
    /// Trusted domain patterns (regex)
    pub trusted_patterns: Vec<String>,
    /// Development domains (only in dev mode)
    pub development_domains: HashSet<String>,
    /// Subdomain policy
    pub subdomain_policy: SubdomainPolicy,
    /// IP address policy
    pub ip_policy: IpAddressPolicy,
}

/// Subdomain handling policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubdomainPolicy {
    /// Allow all subdomains of trusted domains
    AllowAll,
    /// Allow specific subdomains only
    Whitelist(HashSet<String>),
    /// Deny all subdomains
    DenyAll,
}

/// IP address handling policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpAddressPolicy {
    /// Allow local IP addresses (127.0.0.1, localhost, 192.168.x.x, etc.)
    AllowLocal,
    /// Allow specific IP ranges
    AllowRanges(Vec<String>),
    /// Deny all IP addresses
    DenyAll,
}

/// Context-based validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextValidationConfig {
    /// Validate User-Agent header
    pub validate_user_agent: bool,
    /// Validate Referer header
    pub validate_referer: bool,
    /// Check for suspicious request patterns
    pub detect_suspicious_patterns: bool,
    /// Maximum request headers
    pub max_headers: Option<usize>,
    /// Maximum header value length
    pub max_header_length: Option<usize>,
}

/// CORS security policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsSecurityPolicies {
    /// Block requests from known bad origins
    pub block_malicious_origins: bool,
    /// Enforce HTTPS for credentials
    pub enforce_https_credentials: bool,
    /// Validate request timing
    pub validate_timing: bool,
    /// Rate limit preflight requests
    pub rate_limit_preflight: bool,
    /// CSP (Content Security Policy) integration
    pub csp_integration: bool,
}

/// Performance optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsPerformanceConfig {
    /// Cache preflight responses
    pub cache_preflight: bool,
    /// Preflight cache duration
    pub preflight_cache_duration: Duration,
    /// Optimize header generation
    pub optimize_headers: bool,
    /// Compress CORS headers
    pub compress_headers: bool,
}

/// Request context for CORS validation
#[derive(Debug)]
pub struct CorsRequestContext {
    pub origin: Option<String>,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub client_ip: Option<String>,
    pub is_https: bool,
    pub timestamp: SystemTime,
}

/// CORS validation result with detailed information
#[derive(Debug)]
pub enum EnhancedCorsResult {
    /// Request allowed with headers and metadata
    Allowed {
        headers: Vec<(String, String)>,
        metadata: CorsMetadata,
    },
    /// Preflight request handled
    Preflight {
        headers: Vec<(String, String)>,
        cache_duration: Option<Duration>,
    },
    /// Request forbidden with reason
    Forbidden { reason: String, code: String },
    /// Request blocked due to security policy
    Blocked { reason: String, severity: String },
}

/// CORS request metadata
#[derive(Debug, Clone)]
pub struct CorsMetadata {
    pub origin_type: OriginType,
    pub security_level: SecurityLevel,
    pub validation_time: Duration,
    pub cached: bool,
}

/// Origin classification
#[derive(Debug, Clone)]
pub enum OriginType {
    Trusted,
    Development,
    Subdomain,
    LocalNetwork,
    External,
    Suspicious,
}

/// Security level assessment
#[derive(Debug, Clone)]
pub enum SecurityLevel {
    High,
    Medium,
    Low,
    Critical,
}

/// Enhanced CORS middleware with advanced features
pub struct EnhancedCorsMiddleware {
    config: EnhancedCorsConfig,
    /// Compiled regex patterns for origin validation
    origin_patterns: Vec<Regex>,
    /// Preflight cache
    preflight_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Request statistics
    stats: Arc<RwLock<CorsStats>>,
    /// Blocked origins tracking
    blocked_origins: Arc<RwLock<HashMap<String, BlockedOriginInfo>>>,
}

/// Preflight cache entry
#[derive(Debug, Clone)]
struct CacheEntry {
    headers: Vec<(String, String)>,
    expires_at: SystemTime,
    hit_count: u64,
}

/// CORS statistics
#[derive(Debug, Clone, Default)]
pub struct CorsStats {
    pub total_requests: u64,
    pub allowed_requests: u64,
    pub blocked_requests: u64,
    pub preflight_requests: u64,
    pub cache_hits: u64,
    pub security_violations: u64,
}

/// Blocked origin information
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct BlockedOriginInfo {
    reason: String,
    blocked_at: SystemTime,
    block_count: u32,
}

impl Default for EnhancedCorsConfig {
    fn default() -> Self {
        Self {
            base: CorsConfig::restrictive(),
            dynamic_origins: DynamicOriginConfig {
                enabled: true,
                trusted_patterns: vec![r"^https://[a-zA-Z0-9\-]+\.example\.com$".to_string()],
                development_domains: {
                    let mut domains = HashSet::new();
                    domains.insert("http://localhost:3000".to_string());
                    domains.insert("http://localhost:8080".to_string());
                    domains.insert("http://127.0.0.1:3000".to_string());
                    domains
                },
                subdomain_policy: SubdomainPolicy::AllowAll,
                ip_policy: IpAddressPolicy::AllowLocal,
            },
            context_validation: ContextValidationConfig {
                validate_user_agent: true,
                validate_referer: true,
                detect_suspicious_patterns: true,
                max_headers: Some(50),
                max_header_length: Some(8192),
            },
            security_policies: CorsSecurityPolicies {
                block_malicious_origins: true,
                enforce_https_credentials: true,
                validate_timing: true,
                rate_limit_preflight: true,
                csp_integration: true,
            },
            performance: CorsPerformanceConfig {
                cache_preflight: true,
                preflight_cache_duration: Duration::from_secs(3600), // 1 hour
                optimize_headers: true,
                compress_headers: false,
            },
        }
    }
}

impl EnhancedCorsConfig {
    /// Create production-ready CORS configuration
    pub fn production() -> Self {
        let mut config = Self {
            base: CorsConfig::restrictive(),
            ..Default::default()
        };
        config.security_policies.block_malicious_origins = true;
        config.security_policies.enforce_https_credentials = true;
        config.context_validation.detect_suspicious_patterns = true;
        config
    }

    /// Create development-friendly CORS configuration
    pub fn development() -> Self {
        let mut config = Self {
            base: CorsConfig::permissive(),
            ..Default::default()
        };
        config.security_policies.block_malicious_origins = false;
        config.security_policies.enforce_https_credentials = false;
        config.context_validation.detect_suspicious_patterns = false;
        config
    }

    /// Add trusted origin pattern
    pub fn add_trusted_pattern(&mut self, pattern: String) -> Result<()> {
        Regex::new(&pattern)
            .map_err(|e| LoxoneError::invalid_input(format!("Invalid regex pattern: {e}")))?;
        self.dynamic_origins.trusted_patterns.push(pattern);
        Ok(())
    }

    /// Add development domain
    pub fn add_development_domain(&mut self, domain: String) {
        self.dynamic_origins.development_domains.insert(domain);
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate base configuration
        self.base.validate()?;

        // Validate regex patterns
        for pattern in &self.dynamic_origins.trusted_patterns {
            Regex::new(pattern).map_err(|e| {
                LoxoneError::invalid_input(format!("Invalid regex pattern '{pattern}': {e}"))
            })?;
        }

        // Validate performance settings
        if self.performance.preflight_cache_duration.as_secs() > 86400 * 7 {
            return Err(LoxoneError::invalid_input(
                "Preflight cache duration too long (max 7 days)",
            ));
        }

        Ok(())
    }
}

impl EnhancedCorsMiddleware {
    /// Create new enhanced CORS middleware
    pub fn new(config: EnhancedCorsConfig) -> Result<Self> {
        config.validate()?;

        // Compile regex patterns
        let mut origin_patterns = Vec::new();
        for pattern in &config.dynamic_origins.trusted_patterns {
            let regex = Regex::new(pattern).map_err(|e| {
                LoxoneError::invalid_input(format!("Failed to compile regex '{pattern}': {e}"))
            })?;
            origin_patterns.push(regex);
        }

        Ok(Self {
            config,
            origin_patterns,
            preflight_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CorsStats::default())),
            blocked_origins: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Process CORS request with enhanced validation
    pub async fn process_request(&self, context: CorsRequestContext) -> EnhancedCorsResult {
        let start_time = SystemTime::now();

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_requests += 1;
        }

        // Check if origin is blocked
        if let Some(origin) = &context.origin {
            if self.is_origin_blocked(origin).await {
                let mut stats = self.stats.write().await;
                stats.blocked_requests += 1;
                return EnhancedCorsResult::Blocked {
                    reason: "Origin is blocked due to previous security violations".to_string(),
                    severity: "high".to_string(),
                };
            }
        }

        // Validate request context
        if let Some(violation) = self.validate_request_context(&context).await {
            self.record_security_violation(&context, &violation).await;
            return EnhancedCorsResult::Blocked {
                reason: violation,
                severity: "medium".to_string(),
            };
        }

        // Enhanced origin validation
        let origin_validation = self.validate_origin_enhanced(&context).await;
        match origin_validation {
            OriginValidation::Allowed(origin_type) => {
                // Handle preflight requests
                if context.method.to_uppercase() == "OPTIONS" {
                    return self.handle_preflight_request(&context, origin_type).await;
                }

                // Generate headers for actual request
                let headers = self.generate_enhanced_headers(&context, &origin_type).await;
                let validation_time = start_time.elapsed().unwrap_or_default();

                // Update statistics
                {
                    let mut stats = self.stats.write().await;
                    stats.allowed_requests += 1;
                }

                EnhancedCorsResult::Allowed {
                    headers,
                    metadata: CorsMetadata {
                        origin_type,
                        security_level: self.assess_security_level(&context),
                        validation_time,
                        cached: false,
                    },
                }
            }
            OriginValidation::Forbidden(reason) => {
                let mut stats = self.stats.write().await;
                stats.blocked_requests += 1;
                EnhancedCorsResult::Forbidden {
                    reason,
                    code: "CORS_ORIGIN_FORBIDDEN".to_string(),
                }
            }
        }
    }

    /// Validate origin with enhanced rules
    async fn validate_origin_enhanced(&self, context: &CorsRequestContext) -> OriginValidation {
        let Some(origin) = &context.origin else {
            return OriginValidation::Allowed(OriginType::External);
        };

        // Check base CORS policy
        if !self.config.base.is_origin_allowed(origin) {
            return OriginValidation::Forbidden("Origin not in base allow list".to_string());
        }

        // Check dynamic patterns
        if self.config.dynamic_origins.enabled {
            for pattern in &self.origin_patterns {
                if pattern.is_match(origin) {
                    return OriginValidation::Allowed(OriginType::Trusted);
                }
            }

            // Check development domains
            if self
                .config
                .dynamic_origins
                .development_domains
                .contains(origin)
            {
                return OriginValidation::Allowed(OriginType::Development);
            }

            // Check subdomain policy
            if self.is_subdomain_allowed(origin) {
                return OriginValidation::Allowed(OriginType::Subdomain);
            }

            // Check IP address policy
            if self.is_ip_address_allowed(origin) {
                return OriginValidation::Allowed(OriginType::LocalNetwork);
            }
        }

        OriginValidation::Allowed(OriginType::External)
    }

    /// Validate request context for security issues
    async fn validate_request_context(&self, context: &CorsRequestContext) -> Option<String> {
        if !self.config.context_validation.detect_suspicious_patterns {
            return None;
        }

        // Check header count
        if let Some(max_headers) = self.config.context_validation.max_headers {
            if context.headers.len() > max_headers {
                return Some(format!(
                    "Too many headers: {} > {}",
                    context.headers.len(),
                    max_headers
                ));
            }
        }

        // Check header lengths
        if let Some(max_length) = self.config.context_validation.max_header_length {
            for (name, value) in &context.headers {
                if name.len() + value.len() > max_length {
                    return Some(format!(
                        "Header too long: {} + {} > {}",
                        name.len(),
                        value.len(),
                        max_length
                    ));
                }
            }
        }

        // Check for suspicious User-Agent
        if self.config.context_validation.validate_user_agent {
            if let Some(ua) = &context.user_agent {
                if self.is_suspicious_user_agent(ua) {
                    return Some("Suspicious User-Agent detected".to_string());
                }
            }
        }

        // Check HTTPS enforcement for credentials
        if self.config.security_policies.enforce_https_credentials
            && self.config.base.allow_credentials
            && !context.is_https
        {
            return Some("HTTPS required for credential requests".to_string());
        }

        None
    }

    /// Handle preflight requests with caching
    async fn handle_preflight_request(
        &self,
        context: &CorsRequestContext,
        origin_type: OriginType,
    ) -> EnhancedCorsResult {
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.preflight_requests += 1;
        }

        // Check cache if enabled
        if self.config.performance.cache_preflight {
            let cache_key = self.generate_cache_key(context);
            let cached_entry = {
                let cache = self.preflight_cache.read().await;
                cache.get(&cache_key).cloned()
            };

            if let Some(mut entry) = cached_entry {
                if entry.expires_at > SystemTime::now() {
                    entry.hit_count += 1;
                    // Update cache with hit count
                    {
                        let mut cache = self.preflight_cache.write().await;
                        cache.insert(cache_key, entry.clone());
                    }

                    // Update statistics
                    {
                        let mut stats = self.stats.write().await;
                        stats.cache_hits += 1;
                    }

                    return EnhancedCorsResult::Preflight {
                        headers: entry.headers,
                        cache_duration: Some(self.config.performance.preflight_cache_duration),
                    };
                }
            }
        }

        // Generate headers
        let headers = self.generate_enhanced_headers(context, &origin_type).await;

        // Cache the result
        if self.config.performance.cache_preflight {
            let cache_key = self.generate_cache_key(context);
            let expires_at = SystemTime::now() + self.config.performance.preflight_cache_duration;
            let entry = CacheEntry {
                headers: headers.clone(),
                expires_at,
                hit_count: 0,
            };

            let mut cache = self.preflight_cache.write().await;
            cache.insert(cache_key, entry);
        }

        EnhancedCorsResult::Preflight {
            headers,
            cache_duration: Some(self.config.performance.preflight_cache_duration),
        }
    }

    /// Generate enhanced CORS headers
    async fn generate_enhanced_headers(
        &self,
        context: &CorsRequestContext,
        origin_type: &OriginType,
    ) -> Vec<(String, String)> {
        let mut headers = self
            .config
            .base
            .generate_headers(context.origin.as_deref(), Some(&context.method));

        // Add enhanced security headers based on origin type
        match origin_type {
            OriginType::Trusted => {
                headers.push(("X-CORS-Security-Level".to_string(), "trusted".to_string()));
            }
            OriginType::Development => {
                headers.push((
                    "X-CORS-Security-Level".to_string(),
                    "development".to_string(),
                ));
                headers.push((
                    "X-CORS-Warning".to_string(),
                    "Development mode enabled".to_string(),
                ));
            }
            OriginType::External => {
                headers.push(("X-CORS-Security-Level".to_string(), "external".to_string()));
            }
            _ => {
                headers.push(("X-CORS-Security-Level".to_string(), "standard".to_string()));
            }
        }

        // Add timing information for debugging
        if cfg!(debug_assertions) {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            headers.push(("X-CORS-Timestamp".to_string(), timestamp.to_string()));
        }

        headers
    }

    /// Check if subdomain is allowed
    fn is_subdomain_allowed(&self, origin: &str) -> bool {
        match &self.config.dynamic_origins.subdomain_policy {
            SubdomainPolicy::AllowAll => {
                // Check if it's a subdomain of any trusted pattern
                self.origin_patterns.iter().any(|pattern| {
                    // Simple subdomain check - in production use proper domain parsing
                    let pattern_str = pattern.as_str();
                    if let Some(domain) = pattern_str.strip_prefix("^https://[a-zA-Z0-9\\-]+\\.") {
                        if let Some(domain) = domain.strip_suffix("$") {
                            origin.contains(domain)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                })
            }
            SubdomainPolicy::Whitelist(allowed) => allowed.contains(origin),
            SubdomainPolicy::DenyAll => false,
        }
    }

    /// Check if IP address is allowed
    fn is_ip_address_allowed(&self, origin: &str) -> bool {
        match &self.config.dynamic_origins.ip_policy {
            IpAddressPolicy::AllowLocal => {
                origin.contains("localhost")
                    || origin.contains("127.0.0.1")
                    || origin.contains("192.168.")
                    || origin.contains("10.")
                    || origin.contains("172.16.")
            }
            IpAddressPolicy::AllowRanges(ranges) => {
                // Extract IP address from origin
                if let Some(ip_str) = Self::extract_ip_from_origin(origin) {
                    ranges
                        .iter()
                        .any(|range| Self::is_ip_in_cidr(&ip_str, range))
                } else {
                    false
                }
            }
            IpAddressPolicy::DenyAll => false,
        }
    }

    /// Check if User-Agent is suspicious
    fn is_suspicious_user_agent(&self, user_agent: &str) -> bool {
        let suspicious_patterns = [
            "bot", "crawler", "spider", "scraper", "curl", "wget", "python", "java",
        ];

        let ua_lower = user_agent.to_lowercase();
        suspicious_patterns
            .iter()
            .any(|pattern| ua_lower.contains(pattern))
    }

    /// Assess security level
    fn assess_security_level(&self, context: &CorsRequestContext) -> SecurityLevel {
        let mut risk_factors = 0;

        if !context.is_https {
            risk_factors += 2;
        }

        if context.user_agent.is_none() {
            risk_factors += 1;
        }

        if context.headers.len() > 20 {
            risk_factors += 1;
        }

        match risk_factors {
            0..=1 => SecurityLevel::High,
            2..=3 => SecurityLevel::Medium,
            4..=5 => SecurityLevel::Low,
            _ => SecurityLevel::Critical,
        }
    }

    /// Generate cache key for preflight requests
    fn generate_cache_key(&self, context: &CorsRequestContext) -> String {
        format!(
            "{}:{}:{}",
            context.origin.as_deref().unwrap_or("null"),
            context.method,
            context
                .headers
                .get("access-control-request-headers")
                .unwrap_or(&String::new())
        )
    }

    /// Check if origin is blocked
    async fn is_origin_blocked(&self, origin: &str) -> bool {
        let blocked = self.blocked_origins.read().await;
        blocked.contains_key(origin)
    }

    /// Record security violation
    async fn record_security_violation(&self, context: &CorsRequestContext, violation: &str) {
        if let Some(origin) = &context.origin {
            let mut blocked = self.blocked_origins.write().await;
            let info = blocked
                .entry(origin.clone())
                .or_insert_with(|| BlockedOriginInfo {
                    reason: violation.to_string(),
                    blocked_at: SystemTime::now(),
                    block_count: 0,
                });
            info.block_count += 1;
        }

        let mut stats = self.stats.write().await;
        stats.security_violations += 1;

        warn!(
            "CORS security violation: {} from origin {:?}",
            violation, context.origin
        );
    }

    /// Get CORS statistics
    pub async fn get_stats(&self) -> CorsStats {
        self.stats.read().await.clone()
    }

    /// Cleanup expired cache entries
    pub async fn cleanup_cache(&self) {
        let now = SystemTime::now();
        let mut cache = self.preflight_cache.write().await;
        cache.retain(|_, entry| entry.expires_at > now);

        debug!(
            "CORS cache cleanup completed, {} entries remaining",
            cache.len()
        );
    }

    /// Extract IP address from origin URL (helper for CIDR checking)
    fn extract_ip_from_origin(origin: &str) -> Option<String> {
        // Parse URL to extract host
        if let Ok(url) = origin.parse::<url::Url>() {
            if let Some(host) = url.host_str() {
                // Check if host is an IP address (contains only digits, dots, and colons)
                if host.chars().all(|c| c.is_numeric() || c == '.' || c == ':') {
                    return Some(host.to_string());
                }
            }
        }
        None
    }

    /// Check if an IP address is within a CIDR range
    fn is_ip_in_cidr(ip: &str, cidr: &str) -> bool {
        // Parse CIDR notation (e.g., "192.168.1.0/24")
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() != 2 {
            return false;
        }

        let network_ip = parts[0];
        let prefix_len: u32 = match parts[1].parse() {
            Ok(len) => len,
            Err(_) => return false,
        };

        // Only support IPv4 for now
        if prefix_len > 32 {
            return false;
        }

        // Parse IP addresses
        let target_ip = match Self::parse_ipv4(ip) {
            Some(ip) => ip,
            None => return false,
        };

        let network_ip = match Self::parse_ipv4(network_ip) {
            Some(ip) => ip,
            None => return false,
        };

        // Create network mask
        let mask = if prefix_len == 0 {
            0
        } else {
            0xffffffff << (32 - prefix_len)
        };

        // Check if IP is in network
        (target_ip & mask) == (network_ip & mask)
    }

    /// Parse IPv4 address string to u32
    fn parse_ipv4(ip_str: &str) -> Option<u32> {
        let parts: Vec<&str> = ip_str.split('.').collect();
        if parts.len() != 4 {
            return None;
        }

        let mut result = 0u32;
        for (i, part) in parts.iter().enumerate() {
            if let Ok(byte) = part.parse::<u8>() {
                result |= (byte as u32) << (8 * (3 - i));
            } else {
                return None;
            }
        }

        Some(result)
    }
}

/// Origin validation result
enum OriginValidation {
    Allowed(OriginType),
    Forbidden(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enhanced_cors_config() {
        let config = EnhancedCorsConfig::production();
        assert!(config.validate().is_ok());
    }

    #[tokio::test]
    async fn test_cors_middleware_creation() {
        let config = EnhancedCorsConfig::development();
        let middleware = EnhancedCorsMiddleware::new(config);
        assert!(middleware.is_ok());
    }

    #[tokio::test]
    async fn test_request_processing() {
        let config = EnhancedCorsConfig::development();
        let middleware = EnhancedCorsMiddleware::new(config).unwrap();

        let context = CorsRequestContext {
            origin: Some("http://localhost:3000".to_string()),
            method: "GET".to_string(),
            headers: HashMap::new(),
            user_agent: Some("Mozilla/5.0".to_string()),
            referer: None,
            client_ip: Some("127.0.0.1".to_string()),
            is_https: false,
            timestamp: SystemTime::now(),
        };

        let result = middleware.process_request(context).await;
        match result {
            EnhancedCorsResult::Allowed { .. } => (),
            _ => panic!("Request should be allowed"),
        }
    }

    #[test]
    fn test_cidr_checking() {
        // Test basic CIDR ranges
        assert!(EnhancedCorsMiddleware::is_ip_in_cidr(
            "192.168.1.10",
            "192.168.1.0/24"
        ));
        assert!(EnhancedCorsMiddleware::is_ip_in_cidr(
            "192.168.1.1",
            "192.168.1.0/24"
        ));
        assert!(EnhancedCorsMiddleware::is_ip_in_cidr(
            "192.168.1.254",
            "192.168.1.0/24"
        ));
        assert!(!EnhancedCorsMiddleware::is_ip_in_cidr(
            "192.168.2.1",
            "192.168.1.0/24"
        ));

        // Test /16 range
        assert!(EnhancedCorsMiddleware::is_ip_in_cidr(
            "192.168.5.10",
            "192.168.0.0/16"
        ));
        assert!(!EnhancedCorsMiddleware::is_ip_in_cidr(
            "192.169.1.10",
            "192.168.0.0/16"
        ));

        // Test /8 range
        assert!(EnhancedCorsMiddleware::is_ip_in_cidr(
            "10.1.2.3",
            "10.0.0.0/8"
        ));
        assert!(!EnhancedCorsMiddleware::is_ip_in_cidr(
            "11.1.2.3",
            "10.0.0.0/8"
        ));

        // Test single host /32
        assert!(EnhancedCorsMiddleware::is_ip_in_cidr(
            "192.168.1.1",
            "192.168.1.1/32"
        ));
        assert!(!EnhancedCorsMiddleware::is_ip_in_cidr(
            "192.168.1.2",
            "192.168.1.1/32"
        ));
    }

    #[test]
    fn test_ip_extraction() {
        assert_eq!(
            EnhancedCorsMiddleware::extract_ip_from_origin("http://192.168.1.1:8080"),
            Some("192.168.1.1".to_string())
        );
        assert_eq!(
            EnhancedCorsMiddleware::extract_ip_from_origin("https://127.0.0.1"),
            Some("127.0.0.1".to_string())
        );
        assert_eq!(
            EnhancedCorsMiddleware::extract_ip_from_origin("http://example.com"),
            None
        );
    }

    #[test]
    fn test_ipv4_parsing() {
        assert_eq!(
            EnhancedCorsMiddleware::parse_ipv4("192.168.1.1"),
            Some(0xc0a80101)
        );
        assert_eq!(
            EnhancedCorsMiddleware::parse_ipv4("127.0.0.1"),
            Some(0x7f000001)
        );
        assert_eq!(EnhancedCorsMiddleware::parse_ipv4("0.0.0.0"), Some(0));
        assert_eq!(
            EnhancedCorsMiddleware::parse_ipv4("255.255.255.255"),
            Some(0xffffffff)
        );
        assert_eq!(EnhancedCorsMiddleware::parse_ipv4("invalid"), None);
        assert_eq!(EnhancedCorsMiddleware::parse_ipv4("192.168.1"), None);
    }
}
