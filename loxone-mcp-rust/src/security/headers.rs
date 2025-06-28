//! Security headers implementation for production security

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Security headers configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityHeadersConfig {
    /// HTTP Strict Transport Security (HSTS)
    pub hsts: HstsConfig,
    /// Content Security Policy (CSP)
    pub csp: CspConfig,
    /// X-Frame-Options
    pub frame_options: FrameOptionsConfig,
    /// X-Content-Type-Options
    pub content_type_options: bool,
    /// X-XSS-Protection
    pub xss_protection: XssProtectionConfig,
    /// Referrer Policy
    pub referrer_policy: ReferrerPolicyConfig,
    /// Permissions Policy (formerly Feature Policy)
    pub permissions_policy: PermissionsPolicyConfig,
    /// Server header control
    pub server_header: ServerHeaderConfig,
    /// Custom security headers
    pub custom_headers: HashMap<String, String>,
}

/// HSTS (HTTP Strict Transport Security) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HstsConfig {
    /// Enable HSTS
    pub enabled: bool,
    /// Max age in seconds
    pub max_age: Duration,
    /// Include subdomains
    pub include_subdomains: bool,
    /// Preload directive
    pub preload: bool,
}

/// Content Security Policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CspConfig {
    /// Enable CSP
    pub enabled: bool,
    /// CSP directives
    pub directives: HashMap<String, Vec<String>>,
    /// Report-only mode
    pub report_only: bool,
    /// Report URI
    pub report_uri: Option<String>,
}

/// X-Frame-Options configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameOptionsConfig {
    /// Frame options policy
    pub policy: FramePolicy,
    /// Allowed origins for ALLOW-FROM
    pub allowed_origins: Vec<String>,
}

/// Frame policy options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FramePolicy {
    /// DENY - prevent framing entirely
    Deny,
    /// SAMEORIGIN - allow framing by same origin
    SameOrigin,
    /// ALLOW-FROM - allow framing from specific origins
    AllowFrom,
}

/// XSS Protection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XssProtectionConfig {
    /// Enable XSS protection
    pub enabled: bool,
    /// Block mode (vs filter mode)
    pub block: bool,
    /// Report URI
    pub report_uri: Option<String>,
}

/// Referrer Policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferrerPolicyConfig {
    /// Referrer policy
    pub policy: ReferrerPolicy,
}

/// Referrer policy options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReferrerPolicy {
    /// no-referrer
    NoReferrer,
    /// no-referrer-when-downgrade
    NoReferrerWhenDowngrade,
    /// origin
    Origin,
    /// origin-when-cross-origin
    OriginWhenCrossOrigin,
    /// same-origin
    SameOrigin,
    /// strict-origin
    StrictOrigin,
    /// strict-origin-when-cross-origin
    StrictOriginWhenCrossOrigin,
    /// unsafe-url
    UnsafeUrl,
}

/// Permissions Policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionsPolicyConfig {
    /// Enable permissions policy
    pub enabled: bool,
    /// Feature permissions
    pub features: HashMap<String, PermissionDirective>,
}

/// Permission directive for features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionDirective {
    /// Allow for all origins
    All,
    /// Allow for same origin only
    Self_,
    /// Allow for specific origins
    Origins(Vec<String>),
    /// Deny for all
    None,
}

/// Server header configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHeaderConfig {
    /// Hide server information
    pub hide: bool,
    /// Custom server string
    pub custom_value: Option<String>,
}

impl SecurityHeadersConfig {
    /// Create production security headers configuration
    pub fn production() -> Self {
        Self {
            hsts: HstsConfig {
                enabled: true,
                max_age: Duration::from_secs(31536000), // 1 year
                include_subdomains: true,
                preload: false, // Should be manually enabled after testing
            },
            csp: CspConfig {
                enabled: true,
                directives: Self::production_csp_directives(),
                report_only: false,
                report_uri: None,
            },
            frame_options: FrameOptionsConfig {
                policy: FramePolicy::Deny,
                allowed_origins: Vec::new(),
            },
            content_type_options: true,
            xss_protection: XssProtectionConfig {
                enabled: true,
                block: true,
                report_uri: None,
            },
            referrer_policy: ReferrerPolicyConfig {
                policy: ReferrerPolicy::StrictOriginWhenCrossOrigin,
            },
            permissions_policy: PermissionsPolicyConfig {
                enabled: true,
                features: Self::production_permissions_policy(),
            },
            server_header: ServerHeaderConfig {
                hide: true,
                custom_value: None,
            },
            custom_headers: HashMap::new(),
        }
    }

    /// Create development security headers configuration
    pub fn development() -> Self {
        Self {
            hsts: HstsConfig {
                enabled: false,                    // Disabled for development
                max_age: Duration::from_secs(300), // 5 minutes for testing
                include_subdomains: false,
                preload: false,
            },
            csp: CspConfig {
                enabled: true,
                directives: Self::development_csp_directives(),
                report_only: true, // Report-only mode for development
                report_uri: None,
            },
            frame_options: FrameOptionsConfig {
                policy: FramePolicy::SameOrigin,
                allowed_origins: Vec::new(),
            },
            content_type_options: true,
            xss_protection: XssProtectionConfig {
                enabled: true,
                block: false, // Filter mode for development
                report_uri: None,
            },
            referrer_policy: ReferrerPolicyConfig {
                policy: ReferrerPolicy::NoReferrerWhenDowngrade,
            },
            permissions_policy: PermissionsPolicyConfig {
                enabled: false, // Disabled for development
                features: HashMap::new(),
            },
            server_header: ServerHeaderConfig {
                hide: false,
                custom_value: Some("Loxone-MCP-Dev".to_string()),
            },
            custom_headers: HashMap::new(),
        }
    }

    /// Create minimal security headers configuration for testing
    pub fn minimal() -> Self {
        Self {
            hsts: HstsConfig {
                enabled: false,
                max_age: Duration::from_secs(0),
                include_subdomains: false,
                preload: false,
            },
            csp: CspConfig {
                enabled: false,
                directives: HashMap::new(),
                report_only: false,
                report_uri: None,
            },
            frame_options: FrameOptionsConfig {
                policy: FramePolicy::SameOrigin,
                allowed_origins: Vec::new(),
            },
            content_type_options: false,
            xss_protection: XssProtectionConfig {
                enabled: false,
                block: false,
                report_uri: None,
            },
            referrer_policy: ReferrerPolicyConfig {
                policy: ReferrerPolicy::UnsafeUrl,
            },
            permissions_policy: PermissionsPolicyConfig {
                enabled: false,
                features: HashMap::new(),
            },
            server_header: ServerHeaderConfig {
                hide: false,
                custom_value: None,
            },
            custom_headers: HashMap::new(),
        }
    }

    /// Production CSP directives
    fn production_csp_directives() -> HashMap<String, Vec<String>> {
        let mut directives = HashMap::new();

        directives.insert("default-src".to_string(), vec!["'self'".to_string()]);
        directives.insert(
            "script-src".to_string(),
            vec!["'self'".to_string(), "'unsafe-inline'".to_string()],
        );
        directives.insert(
            "style-src".to_string(),
            vec!["'self'".to_string(), "'unsafe-inline'".to_string()],
        );
        directives.insert(
            "img-src".to_string(),
            vec!["'self'".to_string(), "data:".to_string()],
        );
        directives.insert("font-src".to_string(), vec!["'self'".to_string()]);
        directives.insert("connect-src".to_string(), vec!["'self'".to_string()]);
        directives.insert("media-src".to_string(), vec!["'self'".to_string()]);
        directives.insert("object-src".to_string(), vec!["'none'".to_string()]);
        directives.insert("child-src".to_string(), vec!["'self'".to_string()]);
        directives.insert("worker-src".to_string(), vec!["'self'".to_string()]);
        directives.insert("frame-ancestors".to_string(), vec!["'none'".to_string()]);
        directives.insert("form-action".to_string(), vec!["'self'".to_string()]);
        directives.insert("base-uri".to_string(), vec!["'self'".to_string()]);
        directives.insert("upgrade-insecure-requests".to_string(), vec![]);

        directives
    }

    /// Development CSP directives (more permissive)
    fn development_csp_directives() -> HashMap<String, Vec<String>> {
        let mut directives = HashMap::new();

        directives.insert(
            "default-src".to_string(),
            vec![
                "'self'".to_string(),
                "'unsafe-inline'".to_string(),
                "'unsafe-eval'".to_string(),
            ],
        );
        directives.insert(
            "script-src".to_string(),
            vec![
                "'self'".to_string(),
                "'unsafe-inline'".to_string(),
                "'unsafe-eval'".to_string(),
            ],
        );
        directives.insert(
            "style-src".to_string(),
            vec!["'self'".to_string(), "'unsafe-inline'".to_string()],
        );
        directives.insert(
            "img-src".to_string(),
            vec!["'self'".to_string(), "data:".to_string(), "*".to_string()],
        );
        directives.insert(
            "font-src".to_string(),
            vec!["'self'".to_string(), "data:".to_string()],
        );
        directives.insert(
            "connect-src".to_string(),
            vec!["'self'".to_string(), "ws:".to_string(), "wss:".to_string()],
        );

        directives
    }

    /// Production permissions policy
    fn production_permissions_policy() -> HashMap<String, PermissionDirective> {
        let mut features = HashMap::new();

        // Disable potentially dangerous features
        features.insert("camera".to_string(), PermissionDirective::None);
        features.insert("microphone".to_string(), PermissionDirective::None);
        features.insert("geolocation".to_string(), PermissionDirective::None);
        features.insert("gyroscope".to_string(), PermissionDirective::None);
        features.insert("accelerometer".to_string(), PermissionDirective::None);
        features.insert("magnetometer".to_string(), PermissionDirective::None);
        features.insert("payment".to_string(), PermissionDirective::None);
        features.insert("usb".to_string(), PermissionDirective::None);
        features.insert("bluetooth".to_string(), PermissionDirective::None);

        // Allow necessary features for self
        features.insert("fullscreen".to_string(), PermissionDirective::Self_);
        features.insert("web-share".to_string(), PermissionDirective::Self_);

        features
    }

    /// Generate HTTP headers map
    pub fn to_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();

        // HSTS
        if self.hsts.enabled {
            let mut hsts_value = format!("max-age={}", self.hsts.max_age.as_secs());
            if self.hsts.include_subdomains {
                hsts_value.push_str("; includeSubDomains");
            }
            if self.hsts.preload {
                hsts_value.push_str("; preload");
            }
            headers.insert("Strict-Transport-Security".to_string(), hsts_value);
        }

        // CSP
        if self.csp.enabled && !self.csp.directives.is_empty() {
            let csp_value = self.build_csp_header();
            let header_name = if self.csp.report_only {
                "Content-Security-Policy-Report-Only"
            } else {
                "Content-Security-Policy"
            };
            headers.insert(header_name.to_string(), csp_value);
        }

        // X-Frame-Options
        let frame_options_value = match &self.frame_options.policy {
            FramePolicy::Deny => "DENY".to_string(),
            FramePolicy::SameOrigin => "SAMEORIGIN".to_string(),
            FramePolicy::AllowFrom => {
                if let Some(origin) = self.frame_options.allowed_origins.first() {
                    format!("ALLOW-FROM {origin}")
                } else {
                    "DENY".to_string()
                }
            }
        };
        headers.insert("X-Frame-Options".to_string(), frame_options_value);

        // X-Content-Type-Options
        if self.content_type_options {
            headers.insert("X-Content-Type-Options".to_string(), "nosniff".to_string());
        }

        // X-XSS-Protection
        if self.xss_protection.enabled {
            let mut xss_value = "1".to_string();
            if self.xss_protection.block {
                xss_value.push_str("; mode=block");
            }
            if let Some(report_uri) = &self.xss_protection.report_uri {
                xss_value.push_str(&format!("; report={report_uri}"));
            }
            headers.insert("X-XSS-Protection".to_string(), xss_value);
        }

        // Referrer-Policy
        let referrer_value = match &self.referrer_policy.policy {
            ReferrerPolicy::NoReferrer => "no-referrer",
            ReferrerPolicy::NoReferrerWhenDowngrade => "no-referrer-when-downgrade",
            ReferrerPolicy::Origin => "origin",
            ReferrerPolicy::OriginWhenCrossOrigin => "origin-when-cross-origin",
            ReferrerPolicy::SameOrigin => "same-origin",
            ReferrerPolicy::StrictOrigin => "strict-origin",
            ReferrerPolicy::StrictOriginWhenCrossOrigin => "strict-origin-when-cross-origin",
            ReferrerPolicy::UnsafeUrl => "unsafe-url",
        };
        headers.insert("Referrer-Policy".to_string(), referrer_value.to_string());

        // Permissions-Policy
        if self.permissions_policy.enabled && !self.permissions_policy.features.is_empty() {
            let permissions_value = self.build_permissions_policy_header();
            headers.insert("Permissions-Policy".to_string(), permissions_value);
        }

        // Server header
        if self.server_header.hide {
            headers.insert(
                "Server".to_string(),
                self.server_header
                    .custom_value
                    .clone()
                    .unwrap_or_else(|| "Loxone-MCP".to_string()),
            );
        }

        // Custom headers
        headers.extend(self.custom_headers.clone());

        headers
    }

    /// Build CSP header value
    fn build_csp_header(&self) -> String {
        let mut directives = Vec::new();

        for (directive, sources) in &self.csp.directives {
            if sources.is_empty() {
                directives.push(directive.clone());
            } else {
                directives.push(format!("{} {}", directive, sources.join(" ")));
            }
        }

        if let Some(report_uri) = &self.csp.report_uri {
            directives.push(format!("report-uri {}", report_uri));
        }

        directives.join("; ")
    }

    /// Build Permissions-Policy header value
    fn build_permissions_policy_header(&self) -> String {
        let mut policies = Vec::new();

        for (feature, directive) in &self.permissions_policy.features {
            let policy_value = match directive {
                PermissionDirective::All => format!("{}=*", feature),
                PermissionDirective::Self_ => format!("{}=(self)", feature),
                PermissionDirective::Origins(origins) => {
                    format!("{}=({})", feature, origins.join(" "))
                }
                PermissionDirective::None => format!("{}=()", feature),
            };
            policies.push(policy_value);
        }

        policies.join(", ")
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate HSTS
        if self.hsts.enabled && self.hsts.max_age.as_secs() < 300 {
            return Err(LoxoneError::invalid_input(
                "HSTS max-age should be at least 300 seconds",
            ));
        }

        // Validate CSP
        if self.csp.enabled && self.csp.directives.is_empty() {
            return Err(LoxoneError::invalid_input(
                "CSP is enabled but no directives are configured",
            ));
        }

        Ok(())
    }

    /// Check if configuration is secure
    pub fn is_secure(&self) -> bool {
        self.hsts.enabled
            && self.csp.enabled
            && self.content_type_options
            && self.xss_protection.enabled
            && matches!(
                self.frame_options.policy,
                FramePolicy::Deny | FramePolicy::SameOrigin
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_production_config() {
        let config = SecurityHeadersConfig::production();
        assert!(config.is_secure());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_development_config() {
        let config = SecurityHeadersConfig::development();
        assert!(!config.is_secure()); // Should not be secure
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_headers_generation() {
        let config = SecurityHeadersConfig::production();
        let headers = config.to_headers();

        assert!(headers.contains_key("Strict-Transport-Security"));
        assert!(headers.contains_key("Content-Security-Policy"));
        assert!(headers.contains_key("X-Frame-Options"));
        assert!(headers.contains_key("X-Content-Type-Options"));
        assert!(headers.contains_key("X-XSS-Protection"));
        assert!(headers.contains_key("Referrer-Policy"));
    }

    #[test]
    fn test_csp_header_building() {
        let config = SecurityHeadersConfig::production();
        let csp_header = config.build_csp_header();

        assert!(csp_header.contains("default-src 'self'"));
        assert!(csp_header.contains("object-src 'none'"));
    }

    #[test]
    fn test_permissions_policy_building() {
        let config = SecurityHeadersConfig::production();
        let permissions_header = config.build_permissions_policy_header();

        assert!(permissions_header.contains("camera=()"));
        assert!(permissions_header.contains("microphone=()"));
    }
}
