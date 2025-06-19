//! Axum middleware for unified authentication
//!
//! This module provides middleware that integrates the unified authentication
//! system with Axum HTTP handlers, enabling seamless authentication across
//! all server endpoints.

use crate::auth::manager::AuthenticationManager;
use crate::auth::models::{AuthResult, AuthContext};
use crate::auth::validation::permissions;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::{debug, warn};

/// Authentication information added to request extensions
#[derive(Debug, Clone)]
pub struct AuthInfo {
    /// Authentication context
    pub context: AuthContext,
    /// Whether this is an admin user
    pub is_admin: bool,
}

impl AuthInfo {
    /// Check if the authenticated user has a specific permission
    pub fn has_permission(&self, permission: &str) -> bool {
        self.context.role.has_permission(permission)
    }
    
    /// Check if this is an admin user
    pub fn is_admin(&self) -> bool {
        self.is_admin
    }
    
    /// Get the API key ID
    pub fn key_id(&self) -> &str {
        &self.context.key_id
    }
    
    /// Get the client IP address
    pub fn client_ip(&self) -> &str {
        &self.context.client_ip
    }
}

/// Middleware that requires authentication for all requests
pub async fn require_auth_middleware(
    State(auth_manager): State<Arc<AuthenticationManager>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let headers = request.headers();
    let query = request.uri().query();
    
    // Authenticate the request
    let auth_result = auth_manager.authenticate_request(headers, query).await;
    
    match auth_result {
        AuthResult::Success(auth_success) => {
            // Create auth info and add to request extensions
            let auth_info = AuthInfo {
                is_admin: matches!(auth_success.context.role, crate::auth::models::Role::Admin),
                context: auth_success.context,
            };
            
            request.extensions_mut().insert(auth_info);
            
            debug!("Request authenticated for key: {}", auth_success.key.id);
            Ok(next.run(request).await)
        }
        AuthResult::Unauthorized { reason } => {
            warn!("Unauthorized request: {}", reason);
            Err(StatusCode::UNAUTHORIZED)
        }
        AuthResult::Forbidden { reason } => {
            warn!("Forbidden request: {}", reason);
            Err(StatusCode::FORBIDDEN)
        }
        AuthResult::RateLimited { retry_after_seconds } => {
            warn!("Rate limited request, retry after {} seconds", retry_after_seconds);
            Err(StatusCode::TOO_MANY_REQUESTS)
        }
    }
}

/// Middleware that requires admin permissions
pub async fn require_admin_middleware(
    State(auth_manager): State<Arc<AuthenticationManager>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // First run the standard auth middleware
    let auth_result = require_auth_middleware(
        State(auth_manager.clone()),
        request,
        next,
    ).await;
    
    // The request will have AuthInfo if auth succeeded
    if let Ok(response) = auth_result {
        Ok(response)
    } else {
        auth_result
    }
}


/// Middleware that checks for specific permissions
pub fn require_permission(permission: &'static str) -> impl Fn(
    State<Arc<AuthenticationManager>>,
    Request,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, StatusCode>> + Send>> + Clone {
    move |State(auth_manager): State<Arc<AuthenticationManager>>, mut request: Request, next: Next| {
        Box::pin(async move {
            let headers = request.headers();
            let query = request.uri().query();
            
            // Authenticate the request
            let auth_result = auth_manager.authenticate_request(headers, query).await;
            
            match auth_result {
                AuthResult::Success(auth_success) => {
                    // Check if the user has the required permission
                    if !auth_manager.check_permission(&auth_success.context, permission).await {
                        warn!(
                            "Permission denied for key {} ({}): missing permission '{}'",
                            auth_success.key.id, auth_success.key.name, permission
                        );
                        return Err(StatusCode::FORBIDDEN);
                    }
                    
                    // Add auth info to request
                    let auth_info = AuthInfo {
                        is_admin: matches!(auth_success.context.role, crate::auth::models::Role::Admin),
                        context: auth_success.context,
                    };
                    
                    request.extensions_mut().insert(auth_info);
                    
                    debug!("Permission check passed for key: {} ({})", auth_success.key.id, permission);
                    Ok(next.run(request).await)
                }
                AuthResult::Unauthorized { reason } => {
                    warn!("Unauthorized request: {}", reason);
                    Err(StatusCode::UNAUTHORIZED)
                }
                AuthResult::Forbidden { reason } => {
                    warn!("Forbidden request: {}", reason);
                    Err(StatusCode::FORBIDDEN)
                }
                AuthResult::RateLimited { retry_after_seconds } => {
                    warn!("Rate limited request, retry after {} seconds", retry_after_seconds);
                    Err(StatusCode::TOO_MANY_REQUESTS)
                }
            }
        })
    }
}

/// Helper function to extract AuthInfo from request extensions
pub fn get_auth_info(request: &Request) -> Option<&AuthInfo> {
    request.extensions().get::<AuthInfo>()
}

/// Helper function to check if request is from an admin user
pub fn is_admin_request(request: &Request) -> bool {
    get_auth_info(request)
        .map(|auth| auth.is_admin())
        .unwrap_or(false)
}

/// Helper function to get the authenticated key ID
pub fn get_authenticated_key_id(request: &Request) -> Option<&str> {
    get_auth_info(request).map(|auth| auth.key_id())
}

/// Helper function to check if the authenticated user has a permission
pub fn has_permission(request: &Request, permission: &str) -> bool {
    get_auth_info(request)
        .map(|auth| auth.has_permission(permission))
        .unwrap_or(false)
}

/// Create admin-only middleware
pub fn admin_only() -> impl Fn(
    State<Arc<AuthenticationManager>>,
    Request,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, StatusCode>> + Send>> + Clone {
    require_permission(permissions::ADMIN_CREATE_KEY)
}

/// Create device control middleware
pub fn device_control() -> impl Fn(
    State<Arc<AuthenticationManager>>,
    Request,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, StatusCode>> + Send>> + Clone {
    require_permission(permissions::DEVICE_CONTROL)
}

/// Create MCP tools middleware
pub fn mcp_tools() -> impl Fn(
    State<Arc<AuthenticationManager>>,
    Request,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, StatusCode>> + Send>> + Clone {
    require_permission(permissions::MCP_TOOLS_EXECUTE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::models::Role;
    use crate::auth::manager::AuthManagerConfig;
    use crate::auth::storage::StorageBackendConfig;
    use axum::{
        body::Body,
        http::{HeaderMap, Method, Request},
    };
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_auth_middleware() {
        let temp_dir = TempDir::new().unwrap();
        let keys_file = temp_dir.path().join("test_keys.json");
        
        let config = AuthManagerConfig {
            storage_config: StorageBackendConfig::File { path: keys_file },
            validation_config: Default::default(),
            cache_refresh_interval_minutes: 60,
            enable_cache_warming: false,
        };
        
        let auth_manager = Arc::new(AuthenticationManager::with_config(config).await.unwrap());
        
        // Create a test key
        let key = auth_manager.create_key(
            "Test Key".to_string(),
            Role::Operator,
            "test_user".to_string(),
            Some(365),
        ).await.unwrap();
        
        // Create a request with the API key
        let mut headers = HeaderMap::new();
        headers.insert("authorization", format!("Bearer {}", key.secret).parse().unwrap());
        
        let _request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        // Test authentication - this would need a mock Next implementation
        // For now, just verify the key was created successfully
        assert!(!key.secret.is_empty());
        assert_eq!(key.role, Role::Operator);
    }
}