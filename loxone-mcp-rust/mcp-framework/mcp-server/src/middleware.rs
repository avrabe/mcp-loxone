//! Middleware stack for request/response processing

use crate::context::RequestContext;
use pulseengine_mcp_auth::AuthenticationManager;
use pulseengine_mcp_monitoring::MetricsCollector;
use pulseengine_mcp_protocol::*;
use pulseengine_mcp_security::SecurityMiddleware;

use async_trait::async_trait;
use std::sync::Arc;
use tracing::debug;

/// Trait for middleware components
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Process incoming request
    async fn process_request(
        &self,
        request: Request,
        context: &RequestContext,
    ) -> std::result::Result<Request, Error>;

    /// Process outgoing response
    async fn process_response(
        &self,
        response: Response,
        context: &RequestContext,
    ) -> std::result::Result<Response, Error>;
}

/// Stack of middleware components
#[derive(Clone)]
pub struct MiddlewareStack {
    security: Option<SecurityMiddleware>,
    auth: Option<Arc<AuthenticationManager>>,
    monitoring: Option<Arc<MetricsCollector>>,
}

impl MiddlewareStack {
    /// Create a new middleware stack
    pub fn new() -> Self {
        Self {
            security: None,
            auth: None,
            monitoring: None,
        }
    }

    /// Add security middleware
    pub fn with_security(mut self, security: SecurityMiddleware) -> Self {
        self.security = Some(security);
        self
    }

    /// Add authentication middleware
    pub fn with_auth(mut self, auth: Arc<AuthenticationManager>) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Add monitoring middleware
    pub fn with_monitoring(mut self, monitoring: Arc<MetricsCollector>) -> Self {
        self.monitoring = Some(monitoring);
        self
    }

    /// Process request through middleware stack
    pub async fn process_request(
        &self,
        mut request: Request,
        context: &RequestContext,
    ) -> std::result::Result<Request, crate::handler::HandlerError> {
        debug!("Processing request through middleware stack");

        // Security middleware (first)
        if let Some(security) = &self.security {
            let sec_context = pulseengine_mcp_security::middleware::RequestContext {
                request_id: context.request_id,
            };
            request = security.process_request(request, &sec_context).await?;
        }

        // Authentication middleware
        if let Some(auth) = &self.auth {
            let auth_context = pulseengine_mcp_auth::manager::RequestContext {
                user_id: context.authenticated_user.clone(),
                roles: context
                    .roles
                    .iter()
                    .map(|_r| pulseengine_mcp_auth::models::Role::Admin)
                    .collect(), // TODO: proper role mapping
            };
            request = auth
                .process_request(request, &auth_context)
                .await
                .map_err(|e| crate::handler::HandlerError::Authentication(e.to_string()))?;
        }

        // Monitoring middleware (last)
        if let Some(monitoring) = &self.monitoring {
            let mon_context = pulseengine_mcp_monitoring::collector::RequestContext {
                request_id: context.request_id,
            };
            request = monitoring.process_request(request, &mon_context).await?;
        }

        Ok(request)
    }

    /// Process response through middleware stack (reverse order)
    pub async fn process_response(
        &self,
        mut response: Response,
        context: &RequestContext,
    ) -> std::result::Result<Response, crate::handler::HandlerError> {
        debug!("Processing response through middleware stack");

        // Monitoring middleware (first on response)
        if let Some(monitoring) = &self.monitoring {
            let mon_context = pulseengine_mcp_monitoring::collector::RequestContext {
                request_id: context.request_id,
            };
            response = monitoring.process_response(response, &mon_context).await?;
        }

        // Authentication middleware
        if let Some(auth) = &self.auth {
            let auth_context = pulseengine_mcp_auth::manager::RequestContext {
                user_id: context.authenticated_user.clone(),
                roles: context
                    .roles
                    .iter()
                    .map(|_r| pulseengine_mcp_auth::models::Role::Admin)
                    .collect(), // TODO: proper role mapping
            };
            response = auth
                .process_response(response, &auth_context)
                .await
                .map_err(|e| crate::handler::HandlerError::Authentication(e.to_string()))?;
        }

        // Security middleware (last on response)
        if let Some(security) = &self.security {
            let sec_context = pulseengine_mcp_security::middleware::RequestContext {
                request_id: context.request_id,
            };
            response = security.process_response(response, &sec_context).await?;
        }

        Ok(response)
    }
}

impl Default for MiddlewareStack {
    fn default() -> Self {
        Self::new()
    }
}
