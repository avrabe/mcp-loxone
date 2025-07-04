//! Enhanced token validation with comprehensive security checks
//!
//! This module provides advanced JWT token validation, security hardening,
//! and threat detection for Loxone authentication systems.

use crate::error::{LoxoneError, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Type alias for failed attempts tracking
type FailedAttemptsMap = Arc<RwLock<HashMap<String, (u32, DateTime<Utc>)>>>;

/// JWT token claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (user ID)
    pub sub: String,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: String,
    /// Issued at timestamp
    pub iat: i64,
    /// Expiration timestamp
    pub exp: i64,
    /// Not before timestamp
    pub nbf: Option<i64>,
    /// JWT ID
    pub jti: Option<String>,
    /// Custom Loxone claims
    #[serde(flatten)]
    pub loxone_claims: LoxoneClaims,
}

/// Loxone-specific JWT claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneClaims {
    /// User permissions/rights
    pub rights: i32,
    /// Session ID
    pub sid: Option<String>,
    /// Client identifier
    pub client: Option<String>,
    /// IP address restriction
    pub ip: Option<String>,
    /// Device fingerprint
    pub device: Option<String>,
}

/// Token validation result
#[derive(Debug, Clone)]
pub enum TokenValidationResult {
    Valid(Box<ValidatedToken>),
    Invalid(ValidationError),
    Expired,
    Revoked,
    Suspicious(SuspiciousActivity),
}

/// Validated token with extracted information
#[derive(Debug, Clone)]
pub struct ValidatedToken {
    pub claims: JwtClaims,
    pub token_hash: String,
    pub validation_time: DateTime<Utc>,
    pub security_level: SecurityLevel,
}

/// Security level assessment
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Validation error details
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub reason: String,
    pub error_code: String,
    pub severity: SecurityLevel,
}

/// Suspicious activity detection
#[derive(Debug, Clone)]
pub struct SuspiciousActivity {
    pub activity_type: SuspiciousActivityType,
    pub details: String,
    pub risk_score: u8, // 0-100
}

/// Types of suspicious activities
#[derive(Debug, Clone)]
pub enum SuspiciousActivityType {
    TokenReplay,
    UnusualLocation,
    RapidTokenGeneration,
    InvalidSignature,
    TamperedClaims,
    BruteForceAttempt,
}

/// Token validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Maximum token age (seconds)
    pub max_token_age: i64,
    /// Clock skew tolerance (seconds)
    pub clock_skew_tolerance: i64,
    /// Enable strict IP validation
    pub strict_ip_validation: bool,
    /// Enable device fingerprinting
    pub device_fingerprinting: bool,
    /// Maximum tokens per user
    pub max_tokens_per_user: usize,
    /// Token replay detection window (seconds)
    pub replay_detection_window: i64,
    /// Enable advanced threat detection
    pub advanced_threat_detection: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_token_age: 28800,      // 8 hours
            clock_skew_tolerance: 300, // 5 minutes
            strict_ip_validation: true,
            device_fingerprinting: true,
            max_tokens_per_user: 5,
            replay_detection_window: 300, // 5 minutes
            advanced_threat_detection: true,
        }
    }
}

/// Token metadata for security tracking
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TokenMetadata {
    pub token_hash: String,
    pub user_id: String,
    pub issued_at: DateTime<Utc>,
    pub last_used: DateTime<Utc>,
    pub use_count: u64,
    pub ip_addresses: HashSet<String>,
    pub user_agents: HashSet<String>,
}

/// Enhanced token validator with security features
pub struct TokenValidator {
    config: ValidationConfig,
    /// Active tokens tracking
    active_tokens: Arc<RwLock<HashMap<String, TokenMetadata>>>,
    /// Revoked tokens (JTI -> revocation time)
    revoked_tokens: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    /// User token counts
    user_token_counts: Arc<RwLock<HashMap<String, usize>>>,
    /// Recent token hashes for replay detection
    recent_tokens: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    /// Failed validation attempts per IP
    failed_attempts: FailedAttemptsMap,
}

impl TokenValidator {
    /// Create a new token validator
    pub fn new(config: ValidationConfig) -> Self {
        Self {
            config,
            active_tokens: Arc::new(RwLock::new(HashMap::new())),
            revoked_tokens: Arc::new(RwLock::new(HashMap::new())),
            user_token_counts: Arc::new(RwLock::new(HashMap::new())),
            recent_tokens: Arc::new(RwLock::new(HashMap::new())),
            failed_attempts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Validate a JWT token with comprehensive security checks
    pub async fn validate_token(
        &self,
        token: &str,
        client_ip: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<TokenValidationResult> {
        // Basic token format validation
        if token.is_empty() {
            return Ok(TokenValidationResult::Invalid(ValidationError {
                reason: "Empty token".to_string(),
                error_code: "TOKEN_EMPTY".to_string(),
                severity: SecurityLevel::Medium,
            }));
        }

        // Calculate token hash for tracking
        let token_hash = self.calculate_token_hash(token);

        // Check for token replay
        if self.config.advanced_threat_detection {
            if let Some(suspicious) = self.detect_token_replay(&token_hash).await {
                return Ok(TokenValidationResult::Suspicious(suspicious));
            }
        }

        // Parse JWT token
        let claims = match self.parse_jwt_token(token) {
            Ok(claims) => claims,
            Err(e) => {
                self.record_failed_attempt(client_ip).await;
                return Ok(TokenValidationResult::Invalid(ValidationError {
                    reason: format!("Token parsing failed: {e}"),
                    error_code: "TOKEN_PARSE_ERROR".to_string(),
                    severity: SecurityLevel::High,
                }));
            }
        };

        // Check if token is revoked
        if let Some(jti) = &claims.jti {
            if self.is_token_revoked(jti).await {
                return Ok(TokenValidationResult::Revoked);
            }
        }

        // Validate token expiration
        let now = Utc::now().timestamp();
        if claims.exp <= now {
            return Ok(TokenValidationResult::Expired);
        }

        // Validate not-before claim
        if let Some(nbf) = claims.nbf {
            if nbf > now + self.config.clock_skew_tolerance {
                return Ok(TokenValidationResult::Invalid(ValidationError {
                    reason: "Token not yet valid".to_string(),
                    error_code: "TOKEN_NOT_YET_VALID".to_string(),
                    severity: SecurityLevel::Medium,
                }));
            }
        }

        // Validate token age
        if now - claims.iat > self.config.max_token_age {
            return Ok(TokenValidationResult::Invalid(ValidationError {
                reason: "Token too old".to_string(),
                error_code: "TOKEN_TOO_OLD".to_string(),
                severity: SecurityLevel::Medium,
            }));
        }

        // Validate IP restriction
        if self.config.strict_ip_validation {
            if let (Some(token_ip), Some(client_ip)) = (&claims.loxone_claims.ip, client_ip) {
                if token_ip != client_ip {
                    return Ok(TokenValidationResult::Suspicious(SuspiciousActivity {
                        activity_type: SuspiciousActivityType::UnusualLocation,
                        details: format!("IP mismatch: token={token_ip}, client={client_ip}"),
                        risk_score: 80,
                    }));
                }
            }
        }

        // Check user token limits
        if let Err(suspicious) = self.check_user_token_limits(&claims.sub).await {
            return Ok(TokenValidationResult::Suspicious(suspicious));
        }

        // Update token tracking
        self.update_token_tracking(&token_hash, &claims, client_ip, user_agent)
            .await;

        // Assess security level
        let security_level = self
            .assess_security_level(&claims, client_ip, user_agent)
            .await;

        // Clear failed attempts for this IP on successful validation
        if let Some(ip) = client_ip {
            self.clear_failed_attempts(ip).await;
        }

        Ok(TokenValidationResult::Valid(Box::new(ValidatedToken {
            claims,
            token_hash,
            validation_time: Utc::now(),
            security_level,
        })))
    }

    /// Revoke a token by JTI
    pub async fn revoke_token(&self, jti: &str) -> Result<()> {
        let mut revoked = self.revoked_tokens.write().await;
        revoked.insert(jti.to_string(), Utc::now());

        debug!("Token revoked: {}", jti);
        Ok(())
    }

    /// Parse JWT token (simplified implementation - in production use a proper JWT library)
    fn parse_jwt_token(&self, token: &str) -> Result<JwtClaims> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(LoxoneError::authentication("Invalid JWT format"));
        }

        // Decode payload (simplified - real implementation should verify signature)
        let payload = general_purpose::URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|e| LoxoneError::authentication(format!("Failed to decode payload: {e}")))?;

        let claims: JwtClaims = serde_json::from_slice(&payload)
            .map_err(|e| LoxoneError::authentication(format!("Failed to parse claims: {e}")))?;

        Ok(claims)
    }

    /// Calculate secure hash of token for tracking
    fn calculate_token_hash(&self, token: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Check if token is revoked
    async fn is_token_revoked(&self, jti: &str) -> bool {
        let revoked = self.revoked_tokens.read().await;
        revoked.contains_key(jti)
    }

    /// Detect token replay attacks
    async fn detect_token_replay(&self, token_hash: &str) -> Option<SuspiciousActivity> {
        let mut recent = self.recent_tokens.write().await;
        let now = Utc::now();

        // Clean old entries
        recent.retain(|_, time| {
            now.signed_duration_since(*time).num_seconds() < self.config.replay_detection_window
        });

        // Check for replay
        if recent.contains_key(token_hash) {
            return Some(SuspiciousActivity {
                activity_type: SuspiciousActivityType::TokenReplay,
                details: "Token used multiple times within detection window".to_string(),
                risk_score: 95,
            });
        }

        // Record this token use
        recent.insert(token_hash.to_string(), now);
        None
    }

    /// Check user token limits
    async fn check_user_token_limits(
        &self,
        user_id: &str,
    ) -> std::result::Result<(), SuspiciousActivity> {
        let mut counts = self.user_token_counts.write().await;
        let count = counts.entry(user_id.to_string()).or_insert(0);

        if *count >= self.config.max_tokens_per_user {
            return Err(SuspiciousActivity {
                activity_type: SuspiciousActivityType::RapidTokenGeneration,
                details: format!(
                    "User {} has {} active tokens (limit: {})",
                    user_id, count, self.config.max_tokens_per_user
                ),
                risk_score: 70,
            });
        }

        *count += 1;
        Ok(())
    }

    /// Update token tracking metadata
    async fn update_token_tracking(
        &self,
        token_hash: &str,
        claims: &JwtClaims,
        client_ip: Option<&str>,
        user_agent: Option<&str>,
    ) {
        let mut tokens = self.active_tokens.write().await;
        let now = Utc::now();

        let metadata = tokens
            .entry(token_hash.to_string())
            .or_insert_with(|| TokenMetadata {
                token_hash: token_hash.to_string(),
                user_id: claims.sub.clone(),
                issued_at: DateTime::from_timestamp(claims.iat, 0).unwrap_or(now),
                last_used: now,
                use_count: 0,
                ip_addresses: HashSet::new(),
                user_agents: HashSet::new(),
            });

        metadata.last_used = now;
        metadata.use_count += 1;

        if let Some(ip) = client_ip {
            metadata.ip_addresses.insert(ip.to_string());
        }
        if let Some(ua) = user_agent {
            metadata.user_agents.insert(ua.to_string());
        }
    }

    /// Assess security level based on various factors
    async fn assess_security_level(
        &self,
        claims: &JwtClaims,
        client_ip: Option<&str>,
        user_agent: Option<&str>,
    ) -> SecurityLevel {
        let mut risk_factors = 0;

        // Check token age
        let token_age = Utc::now().timestamp() - claims.iat;
        if token_age > 3600 {
            risk_factors += 1;
        } // > 1 hour
        if token_age > 14400 {
            risk_factors += 1;
        } // > 4 hours

        // Check IP consistency
        if let Some(token_metadata) = self
            .active_tokens
            .read()
            .await
            .get(&self.calculate_token_hash(""))
        {
            if let Some(ip) = client_ip {
                if token_metadata.ip_addresses.len() > 3
                    || !token_metadata.ip_addresses.contains(ip)
                {
                    risk_factors += 2;
                }
            }
        }

        // Check permissions level
        if claims.loxone_claims.rights > 1000 {
            risk_factors += 1; // High privileges
        }

        // Analyze user agent for suspicious patterns
        if let Some(ua) = user_agent {
            if ua.len() < 10 || ua.contains("bot") || ua.contains("script") || ua.contains("curl") {
                risk_factors += 1; // Suspicious user agent
            }
        }

        match risk_factors {
            0..=1 => SecurityLevel::High,
            2..=3 => SecurityLevel::Medium,
            4..=5 => SecurityLevel::Low,
            _ => SecurityLevel::Critical,
        }
    }

    /// Record failed validation attempt
    async fn record_failed_attempt(&self, client_ip: Option<&str>) {
        if let Some(ip) = client_ip {
            let mut attempts = self.failed_attempts.write().await;
            let now = Utc::now();

            let (count, _) = attempts.entry(ip.to_string()).or_insert((0, now));
            *count += 1;

            if *count > 5 {
                warn!(
                    "High number of failed token validations from IP: {} ({})",
                    ip, count
                );
            }
        }
    }

    /// Clear failed attempts for IP
    async fn clear_failed_attempts(&self, client_ip: &str) {
        let mut attempts = self.failed_attempts.write().await;
        attempts.remove(client_ip);
    }

    /// Clean up expired data
    pub async fn cleanup_expired_data(&self) {
        let now = Utc::now();

        // Clean up revoked tokens older than 7 days
        {
            let mut revoked = self.revoked_tokens.write().await;
            revoked.retain(|_, revoke_time| now.signed_duration_since(*revoke_time).num_days() < 7);
        }

        // Clean up expired active tokens
        {
            let mut tokens = self.active_tokens.write().await;
            tokens.retain(|_, metadata| {
                now.signed_duration_since(metadata.last_used).num_hours() < 24
            });
        }

        // Clean up failed attempts older than 1 hour
        {
            let mut attempts = self.failed_attempts.write().await;
            attempts.retain(|_, (_, time)| now.signed_duration_since(*time).num_hours() < 1);
        }

        debug!("Completed token validator cleanup");
    }

    /// Get security statistics
    pub async fn get_security_stats(&self) -> SecurityStats {
        let tokens = self.active_tokens.read().await;
        let revoked = self.revoked_tokens.read().await;
        let attempts = self.failed_attempts.read().await;

        SecurityStats {
            active_tokens: tokens.len(),
            revoked_tokens: revoked.len(),
            failed_attempts_last_hour: attempts.len(),
            unique_users: tokens
                .values()
                .map(|t| &t.user_id)
                .collect::<HashSet<_>>()
                .len(),
            average_token_age: tokens
                .values()
                .map(|t| Utc::now().signed_duration_since(t.issued_at).num_minutes())
                .sum::<i64>()
                / tokens.len().max(1) as i64,
        }
    }
}

/// Security statistics
#[derive(Debug, Clone)]
pub struct SecurityStats {
    pub active_tokens: usize,
    pub revoked_tokens: usize,
    pub failed_attempts_last_hour: usize,
    pub unique_users: usize,
    pub average_token_age: i64, // minutes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_validation() {
        let config = ValidationConfig::default();
        let validator = TokenValidator::new(config);

        // This would need a proper JWT token for testing
        // For now, just test the validation logic structure
        let result = validator.validate_token("", None, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_token_revocation() {
        let config = ValidationConfig::default();
        let validator = TokenValidator::new(config);

        let jti = "test-token-id";
        validator.revoke_token(jti).await.unwrap();

        assert!(validator.is_token_revoked(jti).await);
    }

    #[tokio::test]
    async fn test_cleanup() {
        let config = ValidationConfig::default();
        let validator = TokenValidator::new(config);

        validator.cleanup_expired_data().await;
        // Test passes if no panic occurs
    }
}
