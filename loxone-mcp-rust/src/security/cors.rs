//! CORS (Cross-Origin Resource Sharing) policy implementation

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

/// CORS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    /// Allowed origins
    pub allowed_origins: OriginPolicy,
    /// Allowed methods
    pub allowed_methods: HashSet<String>,
    /// Allowed headers
    pub allowed_headers: HeaderPolicy,
    /// Exposed headers
    pub exposed_headers: HashSet<String>,
    /// Allow credentials
    pub allow_credentials: bool,
    /// Preflight cache max age
    pub max_age: Option<Duration>,
    /// Vary header
    pub vary_header: bool,
}

/// Origin policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OriginPolicy {
    /// Allow any origin (*)
    Any,
    /// Allow specific origins
    List(HashSet<String>),
    /// Allow origins matching regex patterns
    Patterns(Vec<String>),
    /// Mirror request origin (echo back)
    Mirror,
    /// No origins allowed
    None,
}

/// Header policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HeaderPolicy {
    /// Allow any headers (*)
    Any,
    /// Allow specific headers
    List(HashSet<String>),
    /// Mirror request headers
    Mirror,
}

impl CorsConfig {
    /// Create a restrictive CORS configuration for production
    pub fn restrictive() -> Self {
        let mut allowed_methods = HashSet::new();
        allowed_methods.insert("GET".to_string());
        allowed_methods.insert("POST".to_string());
        allowed_methods.insert("OPTIONS".to_string());

        let mut allowed_headers = HashSet::new();
        allowed_headers.insert("Content-Type".to_string());
        allowed_headers.insert("Authorization".to_string());
        allowed_headers.insert("Accept".to_string());
        allowed_headers.insert("X-Requested-With".to_string());

        let mut exposed_headers = HashSet::new();
        exposed_headers.insert("X-RateLimit-Remaining".to_string());
        exposed_headers.insert("X-RateLimit-Reset".to_string());

        Self {
            allowed_origins: OriginPolicy::List(HashSet::new()), // No origins by default - must be configured
            allowed_methods,
            allowed_headers: HeaderPolicy::List(allowed_headers),
            exposed_headers,
            allow_credentials: true,
            max_age: Some(Duration::from_secs(86400)), // 24 hours
            vary_header: true,
        }
    }

    /// Create a permissive CORS configuration for development
    pub fn permissive() -> Self {
        let mut allowed_methods = HashSet::new();
        allowed_methods.insert("GET".to_string());
        allowed_methods.insert("POST".to_string());
        allowed_methods.insert("PUT".to_string());
        allowed_methods.insert("DELETE".to_string());
        allowed_methods.insert("OPTIONS".to_string());
        allowed_methods.insert("HEAD".to_string());
        allowed_methods.insert("PATCH".to_string());

        let mut development_origins = HashSet::new();
        development_origins.insert("http://localhost:3000".to_string());
        development_origins.insert("http://localhost:3001".to_string());
        development_origins.insert("http://localhost:8080".to_string());
        development_origins.insert("http://127.0.0.1:3000".to_string());
        development_origins.insert("http://127.0.0.1:3001".to_string());
        development_origins.insert("http://127.0.0.1:8080".to_string());

        Self {
            allowed_origins: OriginPolicy::List(development_origins),
            allowed_methods,
            allowed_headers: HeaderPolicy::Any,
            exposed_headers: HashSet::new(),
            allow_credentials: true,
            max_age: Some(Duration::from_secs(3600)), // 1 hour
            vary_header: true,
        }
    }

    /// Create a testing CORS configuration (very permissive)
    pub fn testing() -> Self {
        let mut allowed_methods = HashSet::new();
        allowed_methods.insert("*".to_string()); // Allow all methods

        Self {
            allowed_origins: OriginPolicy::Any,
            allowed_methods,
            allowed_headers: HeaderPolicy::Any,
            exposed_headers: HashSet::new(),
            allow_credentials: false, // Don't allow credentials with wildcard origins
            max_age: None,
            vary_header: false,
        }
    }

    /// Add allowed origin
    pub fn add_origin(&mut self, origin: String) -> Result<()> {
        match &mut self.allowed_origins {
            OriginPolicy::List(origins) => {
                origins.insert(origin);
                Ok(())
            }
            _ => Err(LoxoneError::invalid_input(
                "Cannot add origin to non-list policy",
            )),
        }
    }

    /// Add allowed method
    pub fn add_method(&mut self, method: String) {
        self.allowed_methods.insert(method);
    }

    /// Add allowed header
    pub fn add_header(&mut self, header: String) -> Result<()> {
        match &mut self.allowed_headers {
            HeaderPolicy::List(headers) => {
                headers.insert(header);
                Ok(())
            }
            _ => Err(LoxoneError::invalid_input(
                "Cannot add header to non-list policy",
            )),
        }
    }

    /// Check if origin is allowed
    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        match &self.allowed_origins {
            OriginPolicy::Any => true,
            OriginPolicy::List(origins) => origins.contains(origin),
            OriginPolicy::Patterns(patterns) => {
                patterns.iter().any(|pattern| {
                    // Simple pattern matching (production should use proper regex)
                    if pattern.contains('*') {
                        let prefix = pattern.trim_end_matches('*');
                        origin.starts_with(prefix)
                    } else {
                        pattern == origin
                    }
                })
            }
            OriginPolicy::Mirror => true, // Mirror policy accepts any origin
            OriginPolicy::None => false,
        }
    }

    /// Check if method is allowed
    pub fn is_method_allowed(&self, method: &str) -> bool {
        self.allowed_methods.contains("*")
            || self.allowed_methods.contains(method)
            || self.allowed_methods.contains(&method.to_uppercase())
    }

    /// Check if header is allowed
    pub fn is_header_allowed(&self, header: &str) -> bool {
        match &self.allowed_headers {
            HeaderPolicy::Any => true,
            HeaderPolicy::List(headers) => {
                headers.contains(header)
                    || headers.contains(&header.to_lowercase())
                    || headers.contains(&header.to_uppercase())
            }
            HeaderPolicy::Mirror => true,
        }
    }

    /// Generate CORS headers for a request
    pub fn generate_headers(
        &self,
        request_origin: Option<&str>,
        _request_method: Option<&str>,
    ) -> Vec<(String, String)> {
        let mut headers = Vec::new();

        // Access-Control-Allow-Origin
        let origin_header = match (&self.allowed_origins, request_origin) {
            (OriginPolicy::Any, _) if !self.allow_credentials => "*".to_string(),
            (OriginPolicy::Any, Some(origin)) if self.allow_credentials => origin.to_string(),
            (OriginPolicy::List(origins), Some(origin)) if origins.contains(origin) => {
                origin.to_string()
            }
            (OriginPolicy::Patterns(_), Some(origin)) if self.is_origin_allowed(origin) => {
                origin.to_string()
            }
            (OriginPolicy::Mirror, Some(origin)) => origin.to_string(),
            _ => return headers, // No CORS headers if origin not allowed
        };
        headers.push(("Access-Control-Allow-Origin".to_string(), origin_header));

        // Access-Control-Allow-Credentials
        if self.allow_credentials {
            headers.push((
                "Access-Control-Allow-Credentials".to_string(),
                "true".to_string(),
            ));
        }

        // Access-Control-Allow-Methods
        if !self.allowed_methods.is_empty() {
            let methods = if self.allowed_methods.contains("*") {
                "*".to_string()
            } else {
                self.allowed_methods
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            headers.push(("Access-Control-Allow-Methods".to_string(), methods));
        }

        // Access-Control-Allow-Headers
        match &self.allowed_headers {
            HeaderPolicy::Any => {
                headers.push(("Access-Control-Allow-Headers".to_string(), "*".to_string()));
            }
            HeaderPolicy::List(allowed_headers) if !allowed_headers.is_empty() => {
                let headers_str = allowed_headers
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                headers.push(("Access-Control-Allow-Headers".to_string(), headers_str));
            }
            HeaderPolicy::Mirror => {
                // Would need request headers to mirror - simplified for now
                headers.push(("Access-Control-Allow-Headers".to_string(), "*".to_string()));
            }
            _ => {}
        }

        // Access-Control-Expose-Headers
        if !self.exposed_headers.is_empty() {
            let exposed = self
                .exposed_headers
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            headers.push(("Access-Control-Expose-Headers".to_string(), exposed));
        }

        // Access-Control-Max-Age
        if let Some(max_age) = self.max_age {
            headers.push((
                "Access-Control-Max-Age".to_string(),
                max_age.as_secs().to_string(),
            ));
        }

        // Vary header
        if self.vary_header {
            headers.push(("Vary".to_string(), "Origin".to_string()));
        }

        headers
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Check for conflicting configuration
        if self.allow_credentials && matches!(self.allowed_origins, OriginPolicy::Any) {
            return Err(LoxoneError::invalid_input(
                "Cannot allow credentials with wildcard origins for security reasons",
            ));
        }

        // Validate origin patterns
        if let OriginPolicy::Patterns(patterns) = &self.allowed_origins {
            for pattern in patterns {
                if pattern.is_empty() {
                    return Err(LoxoneError::invalid_input("Empty origin pattern"));
                }
            }
        }

        // Validate max age
        if let Some(max_age) = self.max_age {
            if max_age.as_secs() > 86400 * 7 {
                // 7 days
                return Err(LoxoneError::invalid_input(
                    "Max age should not exceed 7 days",
                ));
            }
        }

        Ok(())
    }

    /// Check if configuration is restrictive (suitable for production)
    pub fn is_restrictive(&self) -> bool {
        !matches!(self.allowed_origins, OriginPolicy::Any)
            && !matches!(self.allowed_headers, HeaderPolicy::Any)
            && !self.allowed_methods.contains("*")
            && !self.allow_credentials
    }

    /// Get security recommendations
    pub fn get_security_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();

        if matches!(self.allowed_origins, OriginPolicy::Any) {
            recommendations.push(
                "Consider restricting allowed origins instead of using wildcard (*)".to_string(),
            );
        }

        if matches!(self.allowed_headers, HeaderPolicy::Any) {
            recommendations.push(
                "Consider specifying explicit allowed headers instead of wildcard".to_string(),
            );
        }

        if self.allowed_methods.contains("*") {
            recommendations.push("Consider restricting allowed HTTP methods".to_string());
        }

        if self.allow_credentials && matches!(self.allowed_origins, OriginPolicy::Any) {
            recommendations
                .push("Allowing credentials with wildcard origins is a security risk".to_string());
        }

        if self.max_age.is_none() {
            recommendations.push("Consider setting a max age for preflight cache".to_string());
        }

        if !self.vary_header {
            recommendations.push("Consider enabling Vary header for better caching".to_string());
        }

        recommendations
    }
}

/// CORS middleware result
#[derive(Debug)]
pub enum CorsResult {
    /// Request is allowed, headers to add
    Allowed(Vec<(String, String)>),
    /// Preflight request handled
    Preflight(Vec<(String, String)>),
    /// Request forbidden
    Forbidden,
}

/// CORS middleware
pub struct CorsMiddleware {
    config: CorsConfig,
}

impl CorsMiddleware {
    /// Create new CORS middleware
    pub fn new(config: CorsConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Process CORS request
    pub fn process_request(
        &self,
        origin: Option<&str>,
        method: &str,
        _headers: Option<&str>,
    ) -> CorsResult {
        // Check if origin is allowed
        if let Some(origin) = origin {
            if !self.config.is_origin_allowed(origin) {
                return CorsResult::Forbidden;
            }
        }

        // Handle preflight requests
        if method.to_uppercase() == "OPTIONS" {
            let cors_headers = self.config.generate_headers(origin, Some(method));
            return CorsResult::Preflight(cors_headers);
        }

        // Check if method is allowed
        if !self.config.is_method_allowed(method) {
            return CorsResult::Forbidden;
        }

        // Generate headers for actual request
        let cors_headers = self.config.generate_headers(origin, Some(method));
        CorsResult::Allowed(cors_headers)
    }

    /// Get configuration
    pub fn get_config(&self) -> &CorsConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restrictive_config() {
        let config = CorsConfig::restrictive();
        assert!(config.validate().is_ok());
        assert!(config.is_restrictive());
    }

    #[test]
    fn test_permissive_config() {
        let config = CorsConfig::permissive();
        assert!(config.validate().is_ok());
        assert!(!config.is_restrictive());
    }

    #[test]
    fn test_origin_checking() {
        let mut config = CorsConfig::restrictive();
        config
            .add_origin("https://example.com".to_string())
            .unwrap();

        assert!(config.is_origin_allowed("https://example.com"));
        assert!(!config.is_origin_allowed("https://evil.com"));
    }

    #[test]
    fn test_method_checking() {
        let config = CorsConfig::restrictive();

        assert!(config.is_method_allowed("GET"));
        assert!(config.is_method_allowed("POST"));
        assert!(!config.is_method_allowed("DELETE"));
    }

    #[test]
    fn test_header_generation() {
        let mut config = CorsConfig::restrictive();
        config
            .add_origin("https://example.com".to_string())
            .unwrap();

        let headers = config.generate_headers(Some("https://example.com"), Some("GET"));

        assert!(!headers.is_empty());
        assert!(headers
            .iter()
            .any(|(name, _)| name == "Access-Control-Allow-Origin"));
    }

    #[test]
    fn test_cors_middleware() {
        let mut config = CorsConfig::restrictive();
        config
            .add_origin("https://example.com".to_string())
            .unwrap();

        let middleware = CorsMiddleware::new(config).unwrap();

        // Test allowed request
        match middleware.process_request(Some("https://example.com"), "GET", None) {
            CorsResult::Allowed(_) => (),
            _ => panic!("Request should be allowed"),
        }

        // Test forbidden request
        match middleware.process_request(Some("https://evil.com"), "GET", None) {
            CorsResult::Forbidden => (),
            _ => panic!("Request should be forbidden"),
        }
    }

    #[test]
    fn test_preflight_handling() {
        let mut config = CorsConfig::restrictive();
        config
            .add_origin("https://example.com".to_string())
            .unwrap();

        let middleware = CorsMiddleware::new(config).unwrap();

        match middleware.process_request(Some("https://example.com"), "OPTIONS", None) {
            CorsResult::Preflight(_) => (),
            _ => panic!("Should handle preflight request"),
        }
    }

    #[test]
    fn test_invalid_config() {
        let mut config = CorsConfig::testing();
        config.allow_credentials = true; // Invalid with wildcard origins

        assert!(config.validate().is_err());
    }
}
