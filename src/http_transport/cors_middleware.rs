//! CORS middleware integration for HTTP transport
//!
//! This module provides seamless integration of enhanced CORS functionality
//! with the HTTP transport layer, including Axum middleware and automatic
//! request context extraction.

use crate::security::enhanced_cors::{
    CorsRequestContext, EnhancedCorsConfig, EnhancedCorsMiddleware, EnhancedCorsResult,
};
use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, warn};

/// CORS middleware for Axum HTTP server
pub struct AxumCorsMiddleware {
    cors_middleware: Arc<EnhancedCorsMiddleware>,
}

impl AxumCorsMiddleware {
    /// Create new Axum CORS middleware
    pub fn new(config: EnhancedCorsConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let cors_middleware = Arc::new(EnhancedCorsMiddleware::new(config)?);
        Ok(Self { cors_middleware })
    }

    /// Create production-ready CORS middleware
    pub fn production() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new(EnhancedCorsConfig::production())
    }

    /// Create development-friendly CORS middleware
    pub fn development() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new(EnhancedCorsConfig::development())
    }

    /// Process HTTP request through CORS middleware
    pub async fn process(
        &self,
        request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        let (parts, body) = request.into_parts();
        
        // Extract CORS context from request
        let context = self.extract_cors_context(&parts.headers, &parts.method, &parts.uri);

        // Process CORS request
        let cors_result = self.cors_middleware.process_request(context).await;

        match cors_result {
            EnhancedCorsResult::Allowed { headers, metadata } => {
                // Continue with the request
                let request = Request::from_parts(parts, body);
                let mut response = next.run(request).await;

                // Add CORS headers to response
                let response_headers = response.headers_mut();
                for (name, value) in headers {
                    if let (Ok(header_name), Ok(header_value)) = (
                        axum::http::HeaderName::try_from(name),
                        HeaderValue::try_from(value),
                    ) {
                        response_headers.insert(header_name, header_value);
                    }
                }

                // Add metadata headers for debugging
                if cfg!(debug_assertions) {
                    if let Ok(security_level) = HeaderValue::try_from(format!("{:?}", metadata.security_level)) {
                        response_headers.insert("x-cors-security-level", security_level);
                    }
                    if let Ok(origin_type) = HeaderValue::try_from(format!("{:?}", metadata.origin_type)) {
                        response_headers.insert("x-cors-origin-type", origin_type);
                    }
                }

                debug!("CORS request allowed with security level: {:?}", metadata.security_level);
                Ok(response)
            }
            EnhancedCorsResult::Preflight { headers, cache_duration } => {
                // Return preflight response
                let mut response = Response::builder().status(StatusCode::NO_CONTENT);

                // Add CORS headers
                for (name, value) in headers {
                    if let (Ok(header_name), Ok(header_value)) = (
                        axum::http::HeaderName::try_from(name),
                        HeaderValue::try_from(value),
                    ) {
                        response = response.header(header_name, header_value);
                    }
                }

                // Add cache information
                if let Some(duration) = cache_duration {
                    if let Ok(cache_control) = HeaderValue::try_from(format!("max-age={}", duration.as_secs())) {
                        response = response.header("cache-control", cache_control);
                    }
                }

                debug!("CORS preflight request handled");
                Ok(response.body(axum::body::Body::empty()).unwrap())
            }
            EnhancedCorsResult::Forbidden { reason, code } => {
                warn!("CORS request forbidden: {} ({})", reason, code);
                
                let mut response = Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(axum::body::Body::from(format!("CORS Error: {}", reason)))
                    .unwrap();

                // Add error information headers
                if let Ok(error_code) = HeaderValue::try_from(code) {
                    response.headers_mut().insert("x-cors-error-code", error_code);
                }

                Ok(response)
            }
            EnhancedCorsResult::Blocked { reason, severity } => {
                warn!("CORS request blocked: {} (severity: {})", reason, severity);
                
                let mut response = Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(axum::body::Body::from("Request blocked by CORS security policy"))
                    .unwrap();

                // Add security information headers
                if let Ok(severity_header) = HeaderValue::try_from(severity) {
                    response.headers_mut().insert("x-cors-block-severity", severity_header);
                }

                Ok(response)
            }
        }
    }

    /// Extract CORS request context from HTTP request
    fn extract_cors_context(
        &self,
        headers: &HeaderMap,
        method: &Method,
        uri: &axum::http::Uri,
    ) -> CorsRequestContext {
        // Extract Origin header
        let origin = headers
            .get("origin")
            .and_then(|h| h.to_str().ok())
            .map(String::from);

        // Extract other headers
        let mut header_map = HashMap::new();
        for (name, value) in headers.iter() {
            if let Ok(value_str) = value.to_str() {
                header_map.insert(name.to_string(), value_str.to_string());
            }
        }

        // Extract User-Agent
        let user_agent = headers
            .get("user-agent")
            .and_then(|h| h.to_str().ok())
            .map(String::from);

        // Extract Referer
        let referer = headers
            .get("referer")
            .and_then(|h| h.to_str().ok())
            .map(String::from);

        // Extract client IP (try various headers)
        let client_ip = self.extract_client_ip(headers);

        // Determine if HTTPS
        let is_https = uri.scheme_str() == Some("https")
            || headers.get("x-forwarded-proto").and_then(|h| h.to_str().ok()) == Some("https")
            || headers.get("x-forwarded-ssl").is_some();

        CorsRequestContext {
            origin,
            method: method.to_string(),
            headers: header_map,
            user_agent,
            referer,
            client_ip,
            is_https,
            timestamp: SystemTime::now(),
        }
    }

    /// Extract client IP from various headers
    fn extract_client_ip(&self, headers: &HeaderMap) -> Option<String> {
        // Try X-Forwarded-For first (most common)
        if let Some(xff) = headers.get("x-forwarded-for").and_then(|h| h.to_str().ok()) {
            // Take the first IP in the chain
            if let Some(first_ip) = xff.split(',').next() {
                return Some(first_ip.trim().to_string());
            }
        }

        // Try X-Real-IP
        if let Some(real_ip) = headers.get("x-real-ip").and_then(|h| h.to_str().ok()) {
            return Some(real_ip.to_string());
        }

        // Try X-Client-IP
        if let Some(client_ip) = headers.get("x-client-ip").and_then(|h| h.to_str().ok()) {
            return Some(client_ip.to_string());
        }

        // Try CF-Connecting-IP (Cloudflare)
        if let Some(cf_ip) = headers.get("cf-connecting-ip").and_then(|h| h.to_str().ok()) {
            return Some(cf_ip.to_string());
        }

        None
    }

    /// Get CORS statistics
    pub async fn get_stats(&self) -> crate::security::enhanced_cors::CorsStats {
        self.cors_middleware.get_stats().await
    }

    /// Cleanup expired cache entries
    pub async fn cleanup_cache(&self) {
        self.cors_middleware.cleanup_cache().await;
    }
}

/// Axum middleware function for CORS
pub async fn cors_middleware(
    cors: Arc<AxumCorsMiddleware>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    cors.process(request, next).await
}

/// Builder for CORS middleware configuration
pub struct CorsBuilder {
    config: EnhancedCorsConfig,
}

impl CorsBuilder {
    /// Create new CORS builder
    pub fn new() -> Self {
        Self {
            config: EnhancedCorsConfig::default(),
        }
    }

    /// Set allowed origins
    pub fn allow_origins(mut self, origins: Vec<String>) -> Self {
        for origin in origins {
            if let Err(e) = self.config.base.add_origin(origin) {
                eprintln!("Failed to add origin: {}", e);
            }
        }
        self
    }

    /// Set allowed methods
    pub fn allow_methods(mut self, methods: Vec<String>) -> Self {
        for method in methods {
            self.config.base.add_method(method);
        }
        self
    }

    /// Set allowed headers
    pub fn allow_headers(mut self, headers: Vec<String>) -> Self {
        for header in headers {
            if let Err(e) = self.config.base.add_header(header) {
                eprintln!("Failed to add header: {}", e);
            }
        }
        self
    }

    /// Enable credentials
    pub fn allow_credentials(mut self, allow: bool) -> Self {
        self.config.base.allow_credentials = allow;
        self
    }

    /// Add trusted pattern
    pub fn add_trusted_pattern(mut self, pattern: String) -> Self {
        if let Err(e) = self.config.add_trusted_pattern(pattern) {
            eprintln!("Failed to add trusted pattern: {}", e);
        }
        self
    }

    /// Enable development mode
    pub fn development_mode(mut self, enable: bool) -> Self {
        if enable {
            self.config = EnhancedCorsConfig::development();
        }
        self
    }

    /// Enable security policies
    pub fn security_policies(mut self, block_malicious: bool, enforce_https: bool) -> Self {
        self.config.security_policies.block_malicious_origins = block_malicious;
        self.config.security_policies.enforce_https_credentials = enforce_https;
        self
    }

    /// Build the middleware
    pub fn build(self) -> Result<AxumCorsMiddleware, Box<dyn std::error::Error + Send + Sync>> {
        AxumCorsMiddleware::new(self.config)
    }
}

impl Default for CorsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderName, HeaderValue, Method, Uri};

    #[tokio::test]
    async fn test_cors_middleware_creation() {
        let middleware = AxumCorsMiddleware::development().unwrap();
        let stats = middleware.get_stats().await;
        assert_eq!(stats.total_requests, 0);
    }

    #[test]
    fn test_cors_builder() {
        let middleware = CorsBuilder::new()
            .allow_origins(vec!["https://example.com".to_string()])
            .allow_methods(vec!["GET".to_string(), "POST".to_string()])
            .allow_credentials(true)
            .build();
        
        assert!(middleware.is_ok());
    }

    #[test]
    fn test_client_ip_extraction() {
        let middleware = AxumCorsMiddleware::development().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-forwarded-for"),
            HeaderValue::from_static("192.168.1.1, 10.0.0.1"),
        );

        let method = Method::GET;
        let uri = Uri::from_static("https://example.com/api");
        
        let context = middleware.extract_cors_context(&headers, &method, &uri);
        assert_eq!(context.client_ip, Some("192.168.1.1".to_string()));
        assert!(context.is_https);
    }
}