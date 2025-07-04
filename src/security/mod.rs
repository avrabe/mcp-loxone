//! Security hardening and production security measures

pub mod cors;
pub mod encryption;
pub mod enhanced_cors;
pub mod enhanced_validation;
pub mod headers;
pub mod input_sanitization;
pub mod key_store;
pub mod middleware;
pub mod policy;
pub mod rate_limiting;

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Security configuration for production deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Security headers configuration
    pub headers: headers::SecurityHeadersConfig,
    /// CORS policy configuration
    pub cors: cors::CorsConfig,
    /// Input sanitization configuration
    pub input_sanitization: input_sanitization::SanitizationConfig,
    /// Rate limiting configuration
    pub rate_limiting: rate_limiting::RateLimitConfig,
    /// Security policy configuration
    pub policy: policy::SecurityPolicyConfig,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            headers: headers::SecurityHeadersConfig::production(),
            cors: cors::CorsConfig::restrictive(),
            input_sanitization: input_sanitization::SanitizationConfig::strict(),
            rate_limiting: rate_limiting::RateLimitConfig::production(),
            policy: policy::SecurityPolicyConfig::secure(),
        }
    }
}

impl SecurityConfig {
    /// Create a development configuration with relaxed security
    pub fn development() -> Self {
        Self {
            headers: headers::SecurityHeadersConfig::development(),
            cors: cors::CorsConfig::permissive(),
            input_sanitization: input_sanitization::SanitizationConfig::lenient(),
            rate_limiting: rate_limiting::RateLimitConfig::development(),
            policy: policy::SecurityPolicyConfig::development(),
        }
    }

    /// Create a production configuration with maximum security
    pub fn production() -> Self {
        Self::default()
    }

    /// Create a testing configuration with minimal security for automated tests
    pub fn testing() -> Self {
        Self {
            headers: headers::SecurityHeadersConfig::minimal(),
            cors: cors::CorsConfig::testing(),
            input_sanitization: input_sanitization::SanitizationConfig::disabled(),
            rate_limiting: rate_limiting::RateLimitConfig::testing(),
            policy: policy::SecurityPolicyConfig::testing(),
        }
    }

    /// Validate the security configuration
    pub fn validate(&self) -> Result<()> {
        self.headers.validate()?;
        self.cors.validate()?;
        self.input_sanitization.validate()?;
        self.rate_limiting.validate()?;
        self.policy.validate()?;
        Ok(())
    }

    /// Check if the configuration is suitable for production
    pub fn is_production_ready(&self) -> bool {
        self.headers.is_secure()
            && self.cors.is_restrictive()
            && self.input_sanitization.is_enabled()
            && self.rate_limiting.is_enabled()
            && self.policy.is_secure()
    }

    /// Get security warnings for the current configuration
    pub fn get_security_warnings(&self) -> Vec<SecurityWarning> {
        let mut warnings = Vec::new();

        if !self.headers.is_secure() {
            warnings.push(SecurityWarning {
                category: SecurityCategory::Headers,
                severity: SecuritySeverity::High,
                message: "Security headers are not properly configured for production".to_string(),
                recommendation: "Enable HSTS, CSP, and other security headers".to_string(),
            });
        }

        if !self.cors.is_restrictive() {
            warnings.push(SecurityWarning {
                category: SecurityCategory::Cors,
                severity: SecuritySeverity::Medium,
                message: "CORS policy is too permissive".to_string(),
                recommendation: "Restrict CORS to specific origins and methods".to_string(),
            });
        }

        if !self.input_sanitization.is_enabled() {
            warnings.push(SecurityWarning {
                category: SecurityCategory::InputSanitization,
                severity: SecuritySeverity::High,
                message: "Input sanitization is disabled".to_string(),
                recommendation: "Enable input sanitization to prevent injection attacks"
                    .to_string(),
            });
        }

        if !self.rate_limiting.is_enabled() {
            warnings.push(SecurityWarning {
                category: SecurityCategory::RateLimiting,
                severity: SecuritySeverity::Medium,
                message: "Rate limiting is disabled".to_string(),
                recommendation: "Enable rate limiting to prevent abuse".to_string(),
            });
        }

        if !self.policy.is_secure() {
            warnings.push(SecurityWarning {
                category: SecurityCategory::Policy,
                severity: SecuritySeverity::High,
                message: "Security policy is not configured for production".to_string(),
                recommendation: "Configure secure session management and authentication"
                    .to_string(),
            });
        }

        warnings
    }
}

/// Security warning information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityWarning {
    /// Security category
    pub category: SecurityCategory,
    /// Severity level
    pub severity: SecuritySeverity,
    /// Warning message
    pub message: String,
    /// Recommendation to fix the issue
    pub recommendation: String,
}

/// Security category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityCategory {
    /// Security headers
    Headers,
    /// CORS policy
    Cors,
    /// Input sanitization
    InputSanitization,
    /// Rate limiting
    RateLimiting,
    /// Security policy
    Policy,
    /// Authentication
    Authentication,
    /// Session management
    SessionManagement,
}

/// Security severity level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecuritySeverity {
    /// Low severity - informational
    Low,
    /// Medium severity - should be addressed
    Medium,
    /// High severity - must be addressed before production
    High,
    /// Critical severity - immediate attention required
    Critical,
}

/// Security audit result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAudit {
    /// Overall security score (0-100)
    pub score: u8,
    /// Security warnings
    pub warnings: Vec<SecurityWarning>,
    /// Security recommendations
    pub recommendations: Vec<String>,
    /// Configuration analysis
    pub configuration_analysis: HashMap<String, String>,
    /// Compliance status
    pub compliance: ComplianceStatus,
}

/// Compliance status for various security standards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStatus {
    /// OWASP Top 10 compliance
    pub owasp_top_10: bool,
    /// GDPR compliance considerations
    pub gdpr_ready: bool,
    /// Security baseline compliance
    pub security_baseline: bool,
}

/// Security hardening service
pub struct SecurityHardeningService {
    config: SecurityConfig,
}

impl SecurityHardeningService {
    /// Create new security hardening service
    pub fn new(config: SecurityConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Perform security audit
    pub async fn audit(&self) -> SecurityAudit {
        let warnings = self.config.get_security_warnings();
        let score = self.calculate_security_score(&warnings);
        let recommendations = self.generate_recommendations(&warnings);
        let configuration_analysis = self.analyze_configuration();
        let compliance = self.check_compliance();

        SecurityAudit {
            score,
            warnings,
            recommendations,
            configuration_analysis,
            compliance,
        }
    }

    /// Calculate security score based on warnings
    fn calculate_security_score(&self, warnings: &[SecurityWarning]) -> u8 {
        let mut score = 100u8;

        for warning in warnings {
            let deduction = match warning.severity {
                SecuritySeverity::Low => 5,
                SecuritySeverity::Medium => 15,
                SecuritySeverity::High => 25,
                SecuritySeverity::Critical => 40,
            };
            score = score.saturating_sub(deduction);
        }

        score
    }

    /// Generate security recommendations
    fn generate_recommendations(&self, warnings: &[SecurityWarning]) -> Vec<String> {
        let mut recommendations = vec![
            "Regularly update dependencies to patch security vulnerabilities".to_string(),
            "Implement comprehensive logging and monitoring".to_string(),
            "Use strong authentication mechanisms".to_string(),
            "Regularly backup and test recovery procedures".to_string(),
        ];

        // Add specific recommendations based on warnings
        for warning in warnings {
            recommendations.push(warning.recommendation.clone());
        }

        // Remove duplicates
        recommendations.sort();
        recommendations.dedup();

        recommendations
    }

    /// Analyze current configuration
    fn analyze_configuration(&self) -> HashMap<String, String> {
        let mut analysis = HashMap::new();

        analysis.insert(
            "security_headers".to_string(),
            if self.config.headers.is_secure() {
                "Secure"
            } else {
                "Needs attention"
            }
            .to_string(),
        );
        analysis.insert(
            "cors_policy".to_string(),
            if self.config.cors.is_restrictive() {
                "Restrictive"
            } else {
                "Permissive"
            }
            .to_string(),
        );
        analysis.insert(
            "input_sanitization".to_string(),
            if self.config.input_sanitization.is_enabled() {
                "Enabled"
            } else {
                "Disabled"
            }
            .to_string(),
        );
        analysis.insert(
            "rate_limiting".to_string(),
            if self.config.rate_limiting.is_enabled() {
                "Enabled"
            } else {
                "Disabled"
            }
            .to_string(),
        );
        analysis.insert(
            "security_policy".to_string(),
            if self.config.policy.is_secure() {
                "Secure"
            } else {
                "Needs hardening"
            }
            .to_string(),
        );

        analysis
    }

    /// Check compliance with security standards
    fn check_compliance(&self) -> ComplianceStatus {
        let owasp_top_10 = self.config.is_production_ready()
            && self.config.input_sanitization.is_enabled()
            && self.config.headers.is_secure();

        let gdpr_ready = self.config.policy.is_secure() && self.config.headers.is_secure();

        let security_baseline = self.config.is_production_ready();

        ComplianceStatus {
            owasp_top_10,
            gdpr_ready,
            security_baseline,
        }
    }

    /// Get security configuration
    pub fn get_config(&self) -> &SecurityConfig {
        &self.config
    }

    /// Update security configuration
    pub fn update_config(&mut self, config: SecurityConfig) -> Result<()> {
        config.validate()?;
        self.config = config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_config_validation() {
        let config = SecurityConfig::production();
        assert!(config.validate().is_ok());
        assert!(config.is_production_ready());
    }

    #[test]
    fn test_development_config() {
        let config = SecurityConfig::development();
        assert!(config.validate().is_ok());
        assert!(!config.is_production_ready()); // Should not be production ready
    }

    #[tokio::test]
    async fn test_security_audit() {
        let config = SecurityConfig::development();
        let service = SecurityHardeningService::new(config).unwrap();

        let audit = service.audit().await;
        assert!(audit.score < 100); // Development config should have warnings
        assert!(!audit.warnings.is_empty());
    }

    #[test]
    fn test_security_warnings() {
        let config = SecurityConfig::development();
        let warnings = config.get_security_warnings();
        assert!(!warnings.is_empty()); // Development config should have warnings
    }
}
