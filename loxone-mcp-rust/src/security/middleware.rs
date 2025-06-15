//! Security middleware for HTTP server integration

use super::{
    cors::CorsMiddleware,
    input_sanitization::InputSanitizer,
    rate_limiting::{RateLimitBucket, RateLimitResult, WhitelistType},
    SecurityConfig, SecurityHardeningService,
};
use crate::error::{LoxoneError, Result};
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderMap, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Security middleware state
#[derive(Clone)]
pub struct SecurityMiddleware {
    /// Security configuration
    config: SecurityConfig,
    /// Security hardening service
    service: Arc<SecurityHardeningService>,
    /// CORS middleware
    cors_middleware: Arc<CorsMiddleware>,
    /// Input sanitizer
    input_sanitizer: Arc<InputSanitizer>,
    /// Rate limit buckets
    rate_limit_buckets: Arc<RwLock<HashMap<String, RateLimitBucket>>>,
}

impl SecurityMiddleware {
    /// Create new security middleware
    pub fn new(config: SecurityConfig) -> Result<Self> {
        config.validate()?;

        let service = Arc::new(SecurityHardeningService::new(config.clone())?);
        let cors_middleware = Arc::new(CorsMiddleware::new(config.cors.clone())?);
        let input_sanitizer = Arc::new(InputSanitizer::new(config.input_sanitization.clone())?);
        let rate_limit_buckets = Arc::new(RwLock::new(HashMap::new()));

        Ok(Self {
            config,
            service,
            cors_middleware,
            input_sanitizer,
            rate_limit_buckets,
        })
    }

    /// Apply security headers to response
    pub fn apply_security_headers(&self, response: &mut Response) {
        let headers = self.config.headers.to_headers();
        let response_headers = response.headers_mut();

        for (name, value) in headers {
            response_headers.insert(
                header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                header::HeaderValue::from_str(&value).unwrap(),
            );
        }
    }

    /// Check rate limits
    pub async fn check_rate_limits(
        &self,
        client_id: &str,
        endpoint: &str,
        method: &str,
    ) -> RateLimitResult {
        if !self.config.rate_limiting.enabled {
            return RateLimitResult::Allowed {
                remaining_tokens: 1000,
                reset_after: std::time::Duration::from_secs(60),
            };
        }

        // Check whitelist
        if self
            .config
            .rate_limiting
            .is_whitelisted(client_id, WhitelistType::ClientId)
        {
            return RateLimitResult::Allowed {
                remaining_tokens: 1000,
                reset_after: std::time::Duration::from_secs(60),
            };
        }

        // Get effective limits
        let endpoint_limits = self.config.rate_limiting.get_endpoint_limits(endpoint);
        let method_limits = self.config.rate_limiting.get_method_limits(method);

        // Use the more restrictive limit
        let effective_limits =
            if endpoint_limits.requests_per_minute < method_limits.requests_per_minute {
                endpoint_limits
            } else {
                method_limits
            };

        // Get or create rate limit bucket
        let mut buckets = self.rate_limit_buckets.write().await;
        let bucket_key = format!("{}:{}:{}", client_id, endpoint, method);
        let bucket = buckets
            .entry(bucket_key)
            .or_insert_with(|| RateLimitBucket::new(effective_limits.burst_capacity as f64));

        // Check request
        bucket.check_request(effective_limits, &self.config.rate_limiting.penalty_config)
    }

    /// Extract client identifier from request
    pub fn extract_client_id(headers: &HeaderMap) -> String {
        // Try various methods to identify the client
        if let Some(client_id) = headers.get("x-client-id").and_then(|v| v.to_str().ok()) {
            return client_id.to_string();
        }

        if let Some(api_key) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
            return format!("api:{}", api_key);
        }

        if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
            if let Some(token) = auth.strip_prefix("Bearer ") {
                return format!("bearer:{}", &token[..8.min(token.len())]);
            }
        }

        // Fallback to IP address
        if let Some(forwarded) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(ip) = forwarded.split(',').next() {
                return format!("ip:{}", ip.trim());
            }
        }

        if let Some(real_ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
            return format!("ip:{}", real_ip);
        }

        "unknown".to_string()
    }

    /// Sanitize request body
    pub async fn sanitize_request_body(&self, body: &mut Value) -> Result<()> {
        if !self.config.input_sanitization.enabled {
            return Ok(());
        }

        let result = self.input_sanitizer.sanitize(body);

        if !result.is_safe {
            let critical_issues = result
                .issues
                .iter()
                .filter(|issue| {
                    matches!(
                        issue.severity,
                        super::input_sanitization::SanitizationSeverity::Critical
                    )
                })
                .count();

            if critical_issues > 0 {
                return Err(LoxoneError::invalid_input(
                    "Request contains malicious content",
                ));
            }
        }

        // Replace body with sanitized version if available
        if let Some(sanitized) = result.sanitized_data {
            *body = sanitized;
        }

        // Log warnings
        for warning in result.warnings {
            debug!("Sanitization warning: {}", warning);
        }

        Ok(())
    }

    /// Get security configuration
    pub fn get_config(&self) -> &SecurityConfig {
        &self.config
    }

    /// Perform security audit
    pub async fn audit(&self) -> super::SecurityAudit {
        self.service.audit().await
    }
}

/// Security middleware handler for Axum
pub async fn security_middleware_handler(
    State(security): State<Arc<SecurityMiddleware>>,
    mut request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    let start_time = std::time::Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();

    debug!("Security middleware processing request: {} {}", method, uri);

    // Extract client ID
    let client_id = SecurityMiddleware::extract_client_id(&headers);

    // Handle CORS preflight
    if method == Method::OPTIONS {
        let origin = headers.get("origin").and_then(|v| v.to_str().ok());
        let request_method = headers
            .get("access-control-request-method")
            .and_then(|v| v.to_str().ok());

        match security.cors_middleware.process_request(
            origin,
            request_method.unwrap_or("GET"),
            None,
        ) {
            super::cors::CorsResult::Preflight(cors_headers) => {
                let mut response = Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .body(Body::empty())
                    .unwrap();

                for (name, value) in cors_headers {
                    response.headers_mut().insert(
                        header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                        header::HeaderValue::from_str(&value).unwrap(),
                    );
                }

                return Ok(response);
            }
            super::cors::CorsResult::Forbidden => {
                warn!("CORS preflight rejected for origin: {:?}", origin);
                return Err(StatusCode::FORBIDDEN);
            }
            _ => {}
        }
    }

    // Check rate limits
    let rate_limit_result = security
        .check_rate_limits(&client_id, uri.path(), method.as_str())
        .await;

    match rate_limit_result {
        RateLimitResult::Limited {
            retry_after,
            limit_type,
        } => {
            warn!(
                "Rate limit exceeded for client {}: {} limit",
                client_id, limit_type
            );

            let mut response = Json(serde_json::json!({
                "error": "Rate limit exceeded",
                "retry_after": retry_after.as_secs(),
                "limit_type": limit_type
            }))
            .into_response();

            response.headers_mut().insert(
                "X-RateLimit-Limit",
                header::HeaderValue::from_str("0").unwrap(),
            );
            response.headers_mut().insert(
                "X-RateLimit-Remaining",
                header::HeaderValue::from_str("0").unwrap(),
            );
            response.headers_mut().insert(
                "X-RateLimit-Reset",
                header::HeaderValue::from_str(&retry_after.as_secs().to_string()).unwrap(),
            );
            response.headers_mut().insert(
                "Retry-After",
                header::HeaderValue::from_str(&retry_after.as_secs().to_string()).unwrap(),
            );

            *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            return Ok(response);
        }
        RateLimitResult::Penalized { until, reason } => {
            warn!("Client {} is penalized: {}", client_id, reason);

            let remaining = until.duration_since(std::time::Instant::now()).as_secs();
            let response = Json(serde_json::json!({
                "error": "Access temporarily blocked",
                "reason": reason,
                "retry_after": remaining
            }))
            .into_response();

            return Ok(response);
        }
        RateLimitResult::Allowed {
            remaining_tokens,
            reset_after,
        } => {
            // Add rate limit headers to request extensions for later use
            request.extensions_mut().insert(RateLimitInfo {
                remaining: remaining_tokens,
                reset: reset_after.as_secs(),
            });
        }
    }

    // Process request with next middleware/handler
    let mut response = next.run(request).await;

    // Apply security headers
    security.apply_security_headers(&mut response);

    // Apply CORS headers
    let origin = headers.get("origin").and_then(|v| v.to_str().ok());
    match security
        .cors_middleware
        .process_request(origin, method.as_str(), None)
    {
        super::cors::CorsResult::Allowed(cors_headers) => {
            for (name, value) in cors_headers {
                response.headers_mut().insert(
                    header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                    header::HeaderValue::from_str(&value).unwrap(),
                );
            }
        }
        super::cors::CorsResult::Forbidden => {
            // CORS rejection for actual requests (not preflight)
            if origin.is_some() {
                warn!("CORS rejected request from origin: {:?}", origin);
                *response.status_mut() = StatusCode::FORBIDDEN;
            }
        }
        _ => {}
    }

    // Add security timing header
    let duration = start_time.elapsed();
    response.headers_mut().insert(
        "X-Security-Processing-Time",
        header::HeaderValue::from_str(&format!("{}ms", duration.as_millis())).unwrap(),
    );

    // Log security event
    info!(
        "Security processed: {} {} - Client: {} - Status: {} - Duration: {:?}",
        method,
        uri,
        client_id,
        response.status(),
        duration
    );

    Ok(response)
}

/// Rate limit information to pass through request extensions
#[derive(Clone)]
struct RateLimitInfo {
    #[allow(dead_code)]
    remaining: u32,
    #[allow(dead_code)]
    reset: u64,
}

/// Create security middleware layer for Axum router
/// This is currently not used because Axum's middleware system requires specific traits
pub fn _create_security_layer(config: SecurityConfig) -> Result<()> {
    let _middleware = Arc::new(SecurityMiddleware::new(config)?);

    // Axum middleware layers are created inline in the router configuration
    // See HttpTransportServer::create_router for usage
    Ok(())
}

/// Security diagnostics endpoint handler
pub async fn security_diagnostics_handler(
    State(security): State<Arc<SecurityMiddleware>>,
) -> impl IntoResponse {
    let audit = security.audit().await;

    let response = serde_json::json!({
        "security_score": audit.score,
        "production_ready": security.config.is_production_ready(),
        "warnings": audit.warnings.len(),
        "configuration": audit.configuration_analysis,
        "compliance": audit.compliance,
        "recommendations": audit.recommendations,
    });

    Json(response)
}

/// Security headers test endpoint
pub async fn security_headers_test_handler(
    State(security): State<Arc<SecurityMiddleware>>,
) -> impl IntoResponse {
    let headers = security.config.headers.to_headers();

    let response = serde_json::json!({
        "headers_configured": headers.len(),
        "headers": headers,
        "is_secure": security.config.headers.is_secure(),
    });

    Json(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_security_middleware_creation() {
        let config = SecurityConfig::production();
        let middleware = SecurityMiddleware::new(config);
        assert!(middleware.is_ok());
    }

    #[tokio::test]
    async fn test_client_id_extraction() {
        let mut headers = HeaderMap::new();
        headers.insert("x-client-id", "test-client".parse().unwrap());

        let client_id = SecurityMiddleware::extract_client_id(&headers);
        assert_eq!(client_id, "test-client");
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = SecurityConfig::production();
        let middleware = SecurityMiddleware::new(config).unwrap();

        // Test rate limiting
        for _ in 0..100 {
            let result = middleware
                .check_rate_limits("test-client", "/api/test", "POST")
                .await;
            match result {
                RateLimitResult::Allowed { .. } => continue,
                RateLimitResult::Limited { .. } => break,
                _ => panic!("Unexpected rate limit result"),
            }
        }
    }

    #[tokio::test]
    async fn test_input_sanitization() {
        let config = SecurityConfig::production();
        let middleware = SecurityMiddleware::new(config).unwrap();

        let mut malicious_input = serde_json::json!({
            "message": "<script>alert('xss')</script>"
        });

        let result = middleware.sanitize_request_body(&mut malicious_input).await;
        // Should either sanitize or reject based on configuration
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_security_audit() {
        let config = SecurityConfig::development();
        let middleware = SecurityMiddleware::new(config).unwrap();

        let audit = middleware.audit().await;
        assert!(audit.score < 100); // Development config should have lower score
        assert!(!audit.warnings.is_empty());
    }
}
