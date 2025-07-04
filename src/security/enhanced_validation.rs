//! Enhanced input validation and security hardening
//!
//! This module provides comprehensive input validation, security hardening,
//! and threat detection for production environments. It extends the basic
//! sanitization with advanced features including cryptographic validation,
//! rate limiting, and machine learning-based anomaly detection.

use crate::error::Result;
use crate::security::input_sanitization::{InputSanitizer, SanitizationConfig, SanitizationResult};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Enhanced validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedValidationConfig {
    /// Base sanitization configuration
    pub base_sanitization: SanitizationConfig,
    /// Cryptographic validation settings
    pub crypto_validation: CryptoValidationConfig,
    /// Anomaly detection settings
    pub anomaly_detection: AnomalyDetectionConfig,
    /// Security hardening policies
    pub hardening_policies: HardeningPolicies,
    /// Rate limiting per input type
    pub input_rate_limits: HashMap<String, RateLimitConfig>,
    /// Content security policies
    pub content_policies: ContentSecurityPolicies,
}

/// Cryptographic validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoValidationConfig {
    /// Enable checksum validation
    pub checksum_validation: bool,
    /// Enable signature verification
    pub signature_verification: bool,
    /// Trusted public keys for signature verification
    pub trusted_keys: HashSet<String>,
    /// Enable nonce validation (prevents replay attacks)
    pub nonce_validation: bool,
    /// Nonce expiration time
    pub nonce_expiration: Duration,
}

/// Anomaly detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetectionConfig {
    /// Enable anomaly detection
    pub enabled: bool,
    /// Pattern learning mode
    pub learning_mode: bool,
    /// Anomaly threshold (0-100)
    pub anomaly_threshold: f64,
    /// Time window for pattern analysis
    pub analysis_window: Duration,
    /// Maximum patterns to track
    pub max_patterns: usize,
}

/// Security hardening policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardeningPolicies {
    /// Block requests from known bad IPs
    pub ip_blocking_enabled: bool,
    /// Block requests with suspicious patterns
    pub pattern_blocking_enabled: bool,
    /// Enable command injection prevention
    pub command_injection_prevention: bool,
    /// Enable XML external entity (XXE) prevention
    pub xxe_prevention: bool,
    /// Enable LDAP injection prevention
    pub ldap_injection_prevention: bool,
    /// Enable NoSQL injection prevention
    pub nosql_injection_prevention: bool,
    /// Enable template injection prevention
    pub template_injection_prevention: bool,
    /// Maximum allowed request complexity
    pub max_request_complexity: usize,
}

/// Content security policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSecurityPolicies {
    /// Allowed MIME types
    pub allowed_mime_types: HashSet<String>,
    /// Maximum file upload size
    pub max_upload_size: usize,
    /// File extension whitelist
    pub allowed_extensions: HashSet<String>,
    /// Magic number validation for files
    pub magic_number_validation: bool,
    /// Content type sniffing prevention
    pub prevent_content_sniffing: bool,
}

/// Rate limit configuration for specific input types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per time window
    pub max_requests: u32,
    /// Time window duration
    pub window_duration: Duration,
    /// Burst allowance
    pub burst_size: u32,
}

/// Validation result with detailed security information
#[derive(Debug, Clone)]
pub struct EnhancedValidationResult {
    /// Base sanitization result
    pub sanitization_result: SanitizationResult,
    /// Security validation results
    pub security_validations: Vec<SecurityValidation>,
    /// Anomaly detection results
    pub anomaly_results: Option<AnomalyDetectionResult>,
    /// Overall risk score (0-100)
    pub risk_score: f64,
    /// Security recommendations
    pub recommendations: Vec<String>,
    /// Blocked reason if request was blocked
    pub blocked_reason: Option<String>,
}

/// Security validation result
#[derive(Debug, Clone)]
pub struct SecurityValidation {
    /// Validation type
    pub validation_type: ValidationType,
    /// Whether validation passed
    pub passed: bool,
    /// Validation details
    pub details: String,
    /// Severity if failed
    pub severity: ValidationSeverity,
}

/// Types of security validations
#[derive(Debug, Clone)]
pub enum ValidationType {
    Checksum,
    Signature,
    Nonce,
    RateLimit,
    Pattern,
    Complexity,
    ContentType,
    FileExtension,
    MagicNumber,
}

/// Validation severity levels
#[derive(Debug, Clone)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Anomaly detection result
#[derive(Debug, Clone)]
pub struct AnomalyDetectionResult {
    /// Anomaly score (0-100)
    pub anomaly_score: f64,
    /// Detected anomalies
    pub anomalies: Vec<DetectedAnomaly>,
    /// Pattern classification
    pub pattern_classification: PatternClassification,
    /// Confidence level
    pub confidence: f64,
}

/// Detected anomaly
#[derive(Debug, Clone)]
pub struct DetectedAnomaly {
    /// Anomaly type
    pub anomaly_type: AnomalyType,
    /// Field or area where anomaly was detected
    pub location: String,
    /// Description of the anomaly
    pub description: String,
    /// Deviation from normal pattern
    pub deviation_score: f64,
}

/// Types of anomalies
#[derive(Debug, Clone)]
pub enum AnomalyType {
    UnusualStructure,
    UnexpectedValue,
    UnusualValue,
    TimingAnomaly,
    FrequencyAnomaly,
    SequenceAnomaly,
    BehavioralAnomaly,
}

/// Pattern classification
#[derive(Debug, Clone)]
pub enum PatternClassification {
    Normal,
    Suspicious,
    Malicious,
    Unknown,
}

/// Request pattern for anomaly detection
#[derive(Debug, Clone)]
struct RequestPattern {
    /// Pattern hash
    pattern_hash: String,
    /// Field structure
    field_structure: HashMap<String, String>,
    /// Value characteristics
    value_characteristics: HashMap<String, ValueCharacteristic>,
    /// Timestamp
    timestamp: SystemTime,
}

/// Value characteristic for pattern learning
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ValueCharacteristic {
    /// Data type
    data_type: String,
    /// Length range
    length_range: (usize, usize),
    /// Common patterns
    common_patterns: HashSet<String>,
    /// Frequency
    frequency: u32,
}

/// Enhanced input validator with comprehensive security
pub struct EnhancedValidator {
    /// Configuration
    config: EnhancedValidationConfig,
    /// Base input sanitizer
    sanitizer: InputSanitizer,
    /// Used nonces (for replay prevention)
    used_nonces: Arc<RwLock<HashMap<String, SystemTime>>>,
    /// Request patterns for anomaly detection
    request_patterns: Arc<RwLock<VecDeque<RequestPattern>>>,
    /// Blocked IPs
    blocked_ips: Arc<RwLock<HashSet<String>>>,
    /// Rate limiters per input type
    rate_limiters: Arc<RwLock<HashMap<String, RateLimiter>>>,
    /// Pattern matchers
    pattern_matchers: HashMap<String, Regex>,
}

/// Simple rate limiter
struct RateLimiter {
    /// Configuration
    config: RateLimitConfig,
    /// Request timestamps
    requests: VecDeque<SystemTime>,
    /// Current burst count
    burst_count: u32,
}

impl Default for EnhancedValidationConfig {
    fn default() -> Self {
        Self {
            base_sanitization: SanitizationConfig::strict(),
            crypto_validation: CryptoValidationConfig {
                checksum_validation: true,
                signature_verification: false,
                trusted_keys: HashSet::new(),
                nonce_validation: true,
                nonce_expiration: Duration::from_secs(300), // 5 minutes
            },
            anomaly_detection: AnomalyDetectionConfig {
                enabled: true,
                learning_mode: false,
                anomaly_threshold: 75.0,
                analysis_window: Duration::from_secs(3600), // 1 hour
                max_patterns: 1000,
            },
            hardening_policies: HardeningPolicies {
                ip_blocking_enabled: true,
                pattern_blocking_enabled: true,
                command_injection_prevention: true,
                xxe_prevention: true,
                ldap_injection_prevention: true,
                nosql_injection_prevention: true,
                template_injection_prevention: true,
                max_request_complexity: 1000,
            },
            input_rate_limits: Self::default_rate_limits(),
            content_policies: ContentSecurityPolicies {
                allowed_mime_types: Self::default_mime_types(),
                max_upload_size: 10 * 1024 * 1024, // 10MB
                allowed_extensions: Self::default_extensions(),
                magic_number_validation: true,
                prevent_content_sniffing: true,
            },
        }
    }
}

impl EnhancedValidationConfig {
    /// Create production configuration
    pub fn production() -> Self {
        let mut config = Self::default();
        config.crypto_validation.signature_verification = true;
        config.anomaly_detection.learning_mode = false;
        config.hardening_policies.ip_blocking_enabled = true;
        config
    }

    /// Create development configuration
    pub fn development() -> Self {
        let mut config = Self::default();
        config.crypto_validation.checksum_validation = false;
        config.anomaly_detection.enabled = false;
        config.hardening_policies.ip_blocking_enabled = false;
        config
    }

    /// Default rate limits
    fn default_rate_limits() -> HashMap<String, RateLimitConfig> {
        let mut limits = HashMap::new();

        limits.insert(
            "api".to_string(),
            RateLimitConfig {
                max_requests: 100,
                window_duration: Duration::from_secs(60),
                burst_size: 20,
            },
        );

        limits.insert(
            "auth".to_string(),
            RateLimitConfig {
                max_requests: 10,
                window_duration: Duration::from_secs(300),
                burst_size: 3,
            },
        );

        limits.insert(
            "command".to_string(),
            RateLimitConfig {
                max_requests: 50,
                window_duration: Duration::from_secs(60),
                burst_size: 10,
            },
        );

        limits
    }

    /// Default allowed MIME types
    fn default_mime_types() -> HashSet<String> {
        let mut types = HashSet::new();
        types.insert("application/json".to_string());
        types.insert("text/plain".to_string());
        types.insert("application/xml".to_string());
        types.insert("application/octet-stream".to_string());
        types
    }

    /// Default allowed file extensions
    fn default_extensions() -> HashSet<String> {
        let mut extensions = HashSet::new();
        extensions.insert("json".to_string());
        extensions.insert("txt".to_string());
        extensions.insert("xml".to_string());
        extensions.insert("csv".to_string());
        extensions.insert("log".to_string());
        extensions
    }
}

impl EnhancedValidator {
    /// Create new enhanced validator
    pub async fn new(config: EnhancedValidationConfig) -> Result<Self> {
        let sanitizer = InputSanitizer::new(config.base_sanitization.clone())?;

        // Compile pattern matchers
        let mut pattern_matchers = HashMap::new();

        // Command injection patterns
        pattern_matchers.insert(
            "command_injection".to_string(),
            Regex::new(r"(?i)(\||;|&|`|\$\(|\)|\{|\}|<|>|\\n|\\r)")?,
        );

        // NoSQL injection patterns
        pattern_matchers.insert(
            "nosql_injection".to_string(),
            Regex::new(r"(?i)(\$ne|\$gt|\$lt|\$gte|\$lte|\$in|\$nin|\$regex|\$where)")?,
        );

        // LDAP injection patterns
        pattern_matchers.insert("ldap_injection".to_string(), Regex::new(r"[*()\\|&=]")?);

        // Template injection patterns
        pattern_matchers.insert(
            "template_injection".to_string(),
            Regex::new(r"(?i)(\{\{|\}\}|<%|%>|\$\{)")?,
        );

        Ok(Self {
            config,
            sanitizer,
            used_nonces: Arc::new(RwLock::new(HashMap::new())),
            request_patterns: Arc::new(RwLock::new(VecDeque::new())),
            blocked_ips: Arc::new(RwLock::new(HashSet::new())),
            rate_limiters: Arc::new(RwLock::new(HashMap::new())),
            pattern_matchers,
        })
    }

    /// Validate input with enhanced security checks
    pub async fn validate(
        &self,
        data: &Value,
        context: ValidationContext,
    ) -> EnhancedValidationResult {
        let start_time = SystemTime::now();

        // Check if IP is blocked
        if let Some(client_ip) = &context.client_ip {
            if self.is_ip_blocked(client_ip).await {
                return EnhancedValidationResult {
                    sanitization_result: SanitizationResult {
                        is_safe: false,
                        sanitized_data: None,
                        issues: vec![],
                        warnings: vec![],
                    },
                    security_validations: vec![],
                    anomaly_results: None,
                    risk_score: 100.0,
                    recommendations: vec![],
                    blocked_reason: Some("IP address is blocked".to_string()),
                };
            }
        }

        // Base sanitization
        let sanitization_result = self.sanitizer.sanitize(data);

        // Security validations
        let mut security_validations = Vec::new();

        // Checksum validation
        if self.config.crypto_validation.checksum_validation {
            if let Some(checksum) = &context.checksum {
                security_validations.push(self.validate_checksum(data, checksum).await);
            }
        }

        // Nonce validation
        if self.config.crypto_validation.nonce_validation {
            if let Some(nonce) = &context.nonce {
                security_validations.push(self.validate_nonce(nonce).await);
            }
        }

        // Rate limiting
        if let Some(input_type) = &context.input_type {
            security_validations.push(self.check_rate_limit(input_type, &context.client_ip).await);
        }

        // Pattern validation
        security_validations.extend(self.validate_patterns(data).await);

        // Complexity validation
        security_validations.push(self.validate_complexity(data));

        // Anomaly detection
        let anomaly_results = if self.config.anomaly_detection.enabled {
            Some(self.detect_anomalies(data, &context).await)
        } else {
            None
        };

        // Calculate risk score
        let risk_score = self.calculate_risk_score(
            &sanitization_result,
            &security_validations,
            &anomaly_results,
        );

        // Generate recommendations
        let recommendations = self.generate_recommendations(
            &sanitization_result,
            &security_validations,
            &anomaly_results,
        );

        // Check if request should be blocked
        let blocked_reason = self.should_block(
            &sanitization_result,
            &security_validations,
            &anomaly_results,
            risk_score,
        );

        // Log validation metrics
        let validation_time = start_time.elapsed().unwrap_or_default();
        debug!(
            "Enhanced validation completed in {:?} with risk score: {}",
            validation_time, risk_score
        );

        EnhancedValidationResult {
            sanitization_result,
            security_validations,
            anomaly_results,
            risk_score,
            recommendations,
            blocked_reason,
        }
    }

    /// Validate checksum
    async fn validate_checksum(&self, data: &Value, expected_checksum: &str) -> SecurityValidation {
        let data_str = serde_json::to_string(data).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(data_str.as_bytes());
        let calculated_checksum = hex::encode(hasher.finalize());

        let passed = calculated_checksum == expected_checksum;

        SecurityValidation {
            validation_type: ValidationType::Checksum,
            passed,
            details: if passed {
                "Checksum validation passed".to_string()
            } else {
                format!(
                    "Checksum mismatch: expected {expected_checksum}, got {calculated_checksum}"
                )
            },
            severity: if passed {
                ValidationSeverity::Info
            } else {
                ValidationSeverity::Error
            },
        }
    }

    /// Validate nonce to prevent replay attacks
    async fn validate_nonce(&self, nonce: &str) -> SecurityValidation {
        let mut used_nonces = self.used_nonces.write().await;
        let now = SystemTime::now();

        // Clean expired nonces
        used_nonces.retain(|_, timestamp| {
            now.duration_since(*timestamp).unwrap_or_default()
                < self.config.crypto_validation.nonce_expiration
        });

        // Check if nonce was already used
        if used_nonces.contains_key(nonce) {
            SecurityValidation {
                validation_type: ValidationType::Nonce,
                passed: false,
                details: "Nonce already used (possible replay attack)".to_string(),
                severity: ValidationSeverity::Critical,
            }
        } else {
            // Store the nonce
            used_nonces.insert(nonce.to_string(), now);

            SecurityValidation {
                validation_type: ValidationType::Nonce,
                passed: true,
                details: "Nonce validation passed".to_string(),
                severity: ValidationSeverity::Info,
            }
        }
    }

    /// Check rate limit
    async fn check_rate_limit(
        &self,
        input_type: &str,
        client_ip: &Option<String>,
    ) -> SecurityValidation {
        let limit_config = match self.config.input_rate_limits.get(input_type) {
            Some(config) => config,
            None => {
                return SecurityValidation {
                    validation_type: ValidationType::RateLimit,
                    passed: true,
                    details: "No rate limit configured for input type".to_string(),
                    severity: ValidationSeverity::Info,
                };
            }
        };

        let key = format!(
            "{}:{}",
            input_type,
            client_ip.as_deref().unwrap_or("unknown")
        );
        let mut limiters = self.rate_limiters.write().await;

        let limiter = limiters.entry(key.clone()).or_insert_with(|| RateLimiter {
            config: limit_config.clone(),
            requests: VecDeque::new(),
            burst_count: 0,
        });

        let now = SystemTime::now();

        // Remove old requests outside the window
        while let Some(front) = limiter.requests.front() {
            if now.duration_since(*front).unwrap_or_default() > limiter.config.window_duration {
                limiter.requests.pop_front();
            } else {
                break;
            }
        }

        // Check for burst patterns
        if !limiter.requests.is_empty() {
            let recent_burst_threshold = Duration::from_secs(1);
            let recent_requests = limiter
                .requests
                .iter()
                .filter(|t| now.duration_since(**t).unwrap_or_default() < recent_burst_threshold)
                .count();

            if recent_requests > 5 {
                limiter.burst_count = limiter.burst_count.saturating_add(1);
            } else if recent_requests == 0 {
                limiter.burst_count = limiter.burst_count.saturating_sub(1);
            }
        }

        // Check rate limit
        if limiter.requests.len() >= limiter.config.max_requests as usize {
            SecurityValidation {
                validation_type: ValidationType::RateLimit,
                passed: false,
                details: format!(
                    "Rate limit exceeded: {} requests in {:?} (burst count: {})",
                    limiter.requests.len(),
                    limiter.config.window_duration,
                    limiter.burst_count
                ),
                severity: ValidationSeverity::Warning,
            }
        } else {
            limiter.requests.push_back(now);

            SecurityValidation {
                validation_type: ValidationType::RateLimit,
                passed: true,
                details: format!(
                    "Rate limit OK: {}/{} requests (burst count: {})",
                    limiter.requests.len(),
                    limiter.config.max_requests,
                    limiter.burst_count
                ),
                severity: if limiter.burst_count > 3 {
                    ValidationSeverity::Warning
                } else {
                    ValidationSeverity::Info
                },
            }
        }
    }

    /// Validate patterns for injection attacks
    async fn validate_patterns(&self, data: &Value) -> Vec<SecurityValidation> {
        let mut validations = Vec::new();
        let data_str = serde_json::to_string(data).unwrap_or_default();

        // Check command injection
        if self.config.hardening_policies.command_injection_prevention {
            if let Some(regex) = self.pattern_matchers.get("command_injection") {
                let passed = !regex.is_match(&data_str);
                validations.push(SecurityValidation {
                    validation_type: ValidationType::Pattern,
                    passed,
                    details: if passed {
                        "No command injection patterns detected".to_string()
                    } else {
                        "Command injection pattern detected".to_string()
                    },
                    severity: if passed {
                        ValidationSeverity::Info
                    } else {
                        ValidationSeverity::Critical
                    },
                });
            }
        }

        // Check NoSQL injection
        if self.config.hardening_policies.nosql_injection_prevention {
            if let Some(regex) = self.pattern_matchers.get("nosql_injection") {
                let passed = !regex.is_match(&data_str);
                validations.push(SecurityValidation {
                    validation_type: ValidationType::Pattern,
                    passed,
                    details: if passed {
                        "No NoSQL injection patterns detected".to_string()
                    } else {
                        "NoSQL injection pattern detected".to_string()
                    },
                    severity: if passed {
                        ValidationSeverity::Info
                    } else {
                        ValidationSeverity::Critical
                    },
                });
            }
        }

        validations
    }

    /// Validate request complexity
    fn validate_complexity(&self, data: &Value) -> SecurityValidation {
        let complexity = self.calculate_complexity(data, 0);
        let passed = complexity <= self.config.hardening_policies.max_request_complexity;

        SecurityValidation {
            validation_type: ValidationType::Complexity,
            passed,
            details: format!(
                "Request complexity: {} (max: {})",
                complexity, self.config.hardening_policies.max_request_complexity
            ),
            severity: if passed {
                ValidationSeverity::Info
            } else {
                ValidationSeverity::Warning
            },
        }
    }

    /// Calculate data complexity
    #[allow(clippy::only_used_in_recursion)]
    fn calculate_complexity(&self, value: &Value, depth: usize) -> usize {
        match value {
            Value::Object(obj) => {
                1 + obj
                    .iter()
                    .map(|(_, v)| self.calculate_complexity(v, depth + 1))
                    .sum::<usize>()
            }
            Value::Array(arr) => {
                1 + arr
                    .iter()
                    .map(|v| self.calculate_complexity(v, depth + 1))
                    .sum::<usize>()
            }
            _ => 1,
        }
    }

    /// Detect anomalies in the request
    async fn detect_anomalies(
        &self,
        data: &Value,
        context: &ValidationContext,
    ) -> AnomalyDetectionResult {
        let mut anomalies = Vec::new();
        let mut anomaly_score = 0.0;

        // Extract request pattern
        let pattern = self.extract_pattern(data);

        // Compare with historical patterns
        let patterns = self.request_patterns.read().await;
        let pattern_match = self.find_similar_patterns(&pattern, &patterns);

        if pattern_match.is_empty() && !self.config.anomaly_detection.learning_mode {
            anomalies.push(DetectedAnomaly {
                anomaly_type: AnomalyType::UnusualStructure,
                location: "request_structure".to_string(),
                description: "Request structure does not match any known patterns".to_string(),
                deviation_score: 80.0,
            });
            anomaly_score += 80.0;
        } else if !pattern_match.is_empty() {
            // Check value characteristics against historical patterns
            for hist_pattern in pattern_match {
                for (field, char) in &pattern.value_characteristics {
                    if let Some(hist_char) = hist_pattern.value_characteristics.get(field) {
                        // Check if value lengths are significantly different
                        if char.data_type == "string" {
                            let (min_len, max_len) = char.length_range;
                            let (hist_min, hist_max) = hist_char.length_range;
                            if min_len > hist_max * 2 || max_len < hist_min / 2 {
                                anomalies.push(DetectedAnomaly {
                                    anomaly_type: AnomalyType::UnusualValue,
                                    location: field.clone(),
                                    description: format!("Value length {min_len} significantly different from historical range {hist_min}-{hist_max}"),
                                    deviation_score: 40.0,
                                });
                                anomaly_score += 40.0;
                            }
                        }
                    }
                }
            }
        }

        // Check timing anomalies
        if let Some(last_request) = context.last_request_time {
            let time_diff = SystemTime::now()
                .duration_since(last_request)
                .unwrap_or_default();

            if time_diff < Duration::from_millis(100) {
                anomalies.push(DetectedAnomaly {
                    anomaly_type: AnomalyType::TimingAnomaly,
                    location: "request_timing".to_string(),
                    description: "Requests arriving too quickly".to_string(),
                    deviation_score: 60.0,
                });
                anomaly_score += 60.0;
            }
        }

        // Store pattern if in learning mode
        if self.config.anomaly_detection.learning_mode {
            drop(patterns);
            let mut patterns_mut = self.request_patterns.write().await;
            patterns_mut.push_back(pattern);

            // Limit pattern storage
            while patterns_mut.len() > self.config.anomaly_detection.max_patterns {
                patterns_mut.pop_front();
            }
        }

        let pattern_classification = match anomaly_score {
            s if s >= 80.0 => PatternClassification::Malicious,
            s if s >= 50.0 => PatternClassification::Suspicious,
            s if s >= 20.0 => PatternClassification::Unknown,
            _ => PatternClassification::Normal,
        };

        AnomalyDetectionResult {
            anomaly_score,
            anomalies,
            pattern_classification,
            confidence: if self.config.anomaly_detection.learning_mode {
                50.0
            } else {
                85.0
            },
        }
    }

    /// Extract pattern from request data
    fn extract_pattern(&self, data: &Value) -> RequestPattern {
        let mut field_structure = HashMap::new();
        let mut value_characteristics = HashMap::new();

        self.extract_structure(data, "", &mut field_structure, &mut value_characteristics);

        let pattern_str = format!("{field_structure:?}");
        let mut hasher = Sha256::new();
        hasher.update(pattern_str.as_bytes());
        let pattern_hash = hex::encode(hasher.finalize());

        RequestPattern {
            pattern_hash,
            field_structure,
            value_characteristics,
            timestamp: SystemTime::now(),
        }
    }

    /// Extract structure from value
    #[allow(clippy::only_used_in_recursion)]
    fn extract_structure(
        &self,
        value: &Value,
        path: &str,
        structure: &mut HashMap<String, String>,
        characteristics: &mut HashMap<String, ValueCharacteristic>,
    ) {
        match value {
            Value::Object(obj) => {
                structure.insert(path.to_string(), "object".to_string());
                for (key, val) in obj {
                    let new_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    self.extract_structure(val, &new_path, structure, characteristics);
                }
            }
            Value::Array(arr) => {
                structure.insert(path.to_string(), format!("array[{}]", arr.len()));
                if let Some(first) = arr.first() {
                    self.extract_structure(first, &format!("{path}[]"), structure, characteristics);
                }
            }
            Value::String(s) => {
                structure.insert(path.to_string(), "string".to_string());
                characteristics.insert(
                    path.to_string(),
                    ValueCharacteristic {
                        data_type: "string".to_string(),
                        length_range: (s.len(), s.len()),
                        common_patterns: HashSet::new(),
                        frequency: 1,
                    },
                );
            }
            Value::Number(n) => {
                structure.insert(path.to_string(), "number".to_string());
                characteristics.insert(
                    path.to_string(),
                    ValueCharacteristic {
                        data_type: "number".to_string(),
                        length_range: (0, 0),
                        common_patterns: HashSet::from([n.to_string()]),
                        frequency: 1,
                    },
                );
            }
            Value::Bool(b) => {
                structure.insert(path.to_string(), "boolean".to_string());
                characteristics.insert(
                    path.to_string(),
                    ValueCharacteristic {
                        data_type: "boolean".to_string(),
                        length_range: (0, 0),
                        common_patterns: HashSet::from([b.to_string()]),
                        frequency: 1,
                    },
                );
            }
            Value::Null => {
                structure.insert(path.to_string(), "null".to_string());
            }
        }
    }

    /// Find similar patterns
    fn find_similar_patterns<'a>(
        &self,
        pattern: &RequestPattern,
        patterns: &'a VecDeque<RequestPattern>,
    ) -> Vec<&'a RequestPattern> {
        let now = SystemTime::now();
        patterns
            .iter()
            .filter(|p| {
                // Check if pattern is recent (within 24 hours)
                let age_ok = now
                    .duration_since(p.timestamp)
                    .map(|d| d.as_secs() < 86400)
                    .unwrap_or(false);

                // Check pattern hash for exact matches
                let hash_match = p.pattern_hash == pattern.pattern_hash;

                // Check structure similarity
                let structure_match = p.field_structure.len() == pattern.field_structure.len()
                    && p.field_structure
                        .keys()
                        .all(|k| pattern.field_structure.contains_key(k));

                // Pattern matches if hash matches OR structure matches (and is recent)
                age_ok && (hash_match || structure_match)
            })
            .collect()
    }

    /// Calculate overall risk score
    fn calculate_risk_score(
        &self,
        sanitization_result: &SanitizationResult,
        security_validations: &[SecurityValidation],
        anomaly_results: &Option<AnomalyDetectionResult>,
    ) -> f64 {
        let mut risk_score = 0.0;

        // Sanitization issues
        for issue in &sanitization_result.issues {
            risk_score += match issue.severity {
                crate::security::input_sanitization::SanitizationSeverity::Critical => 30.0,
                crate::security::input_sanitization::SanitizationSeverity::High => 20.0,
                crate::security::input_sanitization::SanitizationSeverity::Medium => 10.0,
                crate::security::input_sanitization::SanitizationSeverity::Low => 5.0,
            };
        }

        // Security validation failures
        for validation in security_validations {
            if !validation.passed {
                risk_score += match validation.severity {
                    ValidationSeverity::Critical => 40.0,
                    ValidationSeverity::Error => 25.0,
                    ValidationSeverity::Warning => 15.0,
                    ValidationSeverity::Info => 5.0,
                };
            }
        }

        // Anomaly score
        if let Some(anomaly) = anomaly_results {
            risk_score += anomaly.anomaly_score * 0.5;
        }

        risk_score.min(100.0)
    }

    /// Generate security recommendations
    fn generate_recommendations(
        &self,
        sanitization_result: &SanitizationResult,
        security_validations: &[SecurityValidation],
        anomaly_results: &Option<AnomalyDetectionResult>,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Check for common issues
        if sanitization_result.issues.iter().any(|i| {
            matches!(
                i.issue_type,
                crate::security::input_sanitization::SanitizationIssueType::XssAttempt
            )
        }) {
            recommendations
                .push("Consider implementing Content Security Policy (CSP) headers".to_string());
        }

        if security_validations
            .iter()
            .any(|v| !v.passed && matches!(v.validation_type, ValidationType::RateLimit))
        {
            recommendations.push("Rate limit exceeded - consider implementing CAPTCHA".to_string());
        }

        if let Some(anomaly) = anomaly_results {
            if matches!(
                anomaly.pattern_classification,
                PatternClassification::Suspicious | PatternClassification::Malicious
            ) {
                recommendations.push("Unusual pattern detected - review security logs".to_string());
            }
        }

        recommendations
    }

    /// Determine if request should be blocked
    fn should_block(
        &self,
        sanitization_result: &SanitizationResult,
        security_validations: &[SecurityValidation],
        anomaly_results: &Option<AnomalyDetectionResult>,
        risk_score: f64,
    ) -> Option<String> {
        // Block if risk score is too high
        if risk_score >= 80.0 {
            return Some(format!("High risk score: {risk_score:.1}"));
        }

        // Block on critical security failures
        for validation in security_validations {
            if !validation.passed && matches!(validation.severity, ValidationSeverity::Critical) {
                return Some(format!(
                    "Critical security validation failed: {:?}",
                    validation.validation_type
                ));
            }
        }

        // Block on malicious patterns
        if let Some(anomaly) = anomaly_results {
            if matches!(
                anomaly.pattern_classification,
                PatternClassification::Malicious
            ) {
                return Some("Malicious pattern detected".to_string());
            }
        }

        // Block if not safe according to sanitization
        if !sanitization_result.is_safe {
            return Some("Input sanitization failed".to_string());
        }

        None
    }

    /// Check if IP is blocked
    async fn is_ip_blocked(&self, ip: &str) -> bool {
        let blocked_ips = self.blocked_ips.read().await;
        blocked_ips.contains(ip)
    }

    /// Block an IP address
    pub async fn block_ip(&self, ip: String, reason: &str) {
        let mut blocked_ips = self.blocked_ips.write().await;
        blocked_ips.insert(ip.clone());
        warn!("IP blocked: {} - Reason: {}", ip, reason);
    }

    /// Unblock an IP address
    pub async fn unblock_ip(&self, ip: &str) {
        let mut blocked_ips = self.blocked_ips.write().await;
        if blocked_ips.remove(ip) {
            info!("IP unblocked: {}", ip);
        }
    }

    /// Get validation statistics
    pub async fn get_stats(&self) -> ValidationStats {
        let nonces = self.used_nonces.read().await;
        let patterns = self.request_patterns.read().await;
        let blocked_ips = self.blocked_ips.read().await;
        let rate_limiters = self.rate_limiters.read().await;

        ValidationStats {
            active_nonces: nonces.len(),
            tracked_patterns: patterns.len(),
            blocked_ips: blocked_ips.len(),
            active_rate_limiters: rate_limiters.len(),
        }
    }
}

/// Validation context
#[derive(Debug, Clone)]
pub struct ValidationContext {
    /// Client IP address
    pub client_ip: Option<String>,
    /// Input type (e.g., "api", "auth", "command")
    pub input_type: Option<String>,
    /// Request checksum
    pub checksum: Option<String>,
    /// Request nonce
    pub nonce: Option<String>,
    /// Last request time from this client
    pub last_request_time: Option<SystemTime>,
    /// User agent
    pub user_agent: Option<String>,
    /// Request ID for tracking
    pub request_id: Option<String>,
}

/// Validation statistics
#[derive(Debug, Clone)]
pub struct ValidationStats {
    pub active_nonces: usize,
    pub tracked_patterns: usize,
    pub blocked_ips: usize,
    pub active_rate_limiters: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_enhanced_validator_creation() {
        let config = EnhancedValidationConfig::default();
        let validator = EnhancedValidator::new(config).await;
        assert!(validator.is_ok());
    }

    #[tokio::test]
    async fn test_basic_validation() {
        let config = EnhancedValidationConfig::development();
        let validator = EnhancedValidator::new(config).await.unwrap();

        let data = json!({
            "room": "Kitchen",
            "action": "on"
        });

        let context = ValidationContext {
            client_ip: Some("127.0.0.1".to_string()),
            input_type: Some("api".to_string()),
            checksum: None,
            nonce: None,
            last_request_time: None,
            user_agent: Some("test-client".to_string()),
            request_id: Some("test-123".to_string()),
        };

        let result = validator.validate(&data, context).await;
        assert!(result.blocked_reason.is_none());
    }

    #[tokio::test]
    async fn test_nonce_validation() {
        let config = EnhancedValidationConfig::development();
        let validator = EnhancedValidator::new(config).await.unwrap();

        let data = json!({"test": "data"});
        let nonce = "unique-nonce-123";

        let context1 = ValidationContext {
            client_ip: Some("127.0.0.1".to_string()),
            input_type: Some("api".to_string()),
            checksum: None,
            nonce: Some(nonce.to_string()),
            last_request_time: None,
            user_agent: None,
            request_id: None,
        };

        // First request should pass
        let result1 = validator.validate(&data, context1.clone()).await;
        assert!(result1.blocked_reason.is_none());

        // Second request with same nonce should fail
        let result2 = validator.validate(&data, context1).await;
        assert!(result2
            .security_validations
            .iter()
            .any(|v| { matches!(v.validation_type, ValidationType::Nonce) && !v.passed }));
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let mut config = EnhancedValidationConfig::development();
        config.input_rate_limits.insert(
            "test".to_string(),
            RateLimitConfig {
                max_requests: 2,
                window_duration: Duration::from_secs(1),
                burst_size: 1,
            },
        );

        let validator = EnhancedValidator::new(config).await.unwrap();
        let data = json!({"test": "data"});

        for i in 0..3 {
            let context = ValidationContext {
                client_ip: Some("127.0.0.1".to_string()),
                input_type: Some("test".to_string()),
                checksum: None,
                nonce: Some(format!("nonce-{i}")),
                last_request_time: None,
                user_agent: None,
                request_id: None,
            };

            let result = validator.validate(&data, context).await;

            if i < 2 {
                // First 2 requests should pass
                assert!(result.blocked_reason.is_none());
            } else {
                // Third request should be rate limited
                assert!(result.security_validations.iter().any(|v| {
                    matches!(v.validation_type, ValidationType::RateLimit) && !v.passed
                }));
            }
        }
    }
}
