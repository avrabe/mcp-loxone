//! Request context and tracing for MCP server
//!
//! This module provides request ID tracking and context management for better
//! debugging and observability in distributed systems.

use crate::logging::structured::{StructuredContext, StructuredLogger};

use std::time::Instant;
use uuid::Uuid;

/// Request context for tracking and debugging
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Unique request identifier
    pub id: String,

    /// Request start time for performance tracking
    pub start_time: Instant,

    /// Tool being executed
    pub tool_name: String,

    /// Client identifier (if available)
    pub client_id: Option<String>,

    /// User agent (for HTTP requests)
    pub user_agent: Option<String>,

    /// Parent request ID for nested operations
    pub parent_id: Option<String>,
}

impl RequestContext {
    /// Create a new request context
    pub fn new(tool_name: String) -> Self {
        Self {
            id: generate_request_id(),
            start_time: Instant::now(),
            tool_name,
            client_id: None,
            user_agent: None,
            parent_id: None,
        }
    }

    /// Create a new request context with client information
    pub fn with_client(
        tool_name: String,
        client_id: Option<String>,
        user_agent: Option<String>,
        parent_id: Option<String>,
    ) -> Self {
        Self {
            id: generate_request_id(),
            start_time: Instant::now(),
            tool_name,
            client_id,
            user_agent,
            parent_id,
        }
    }

    /// Get elapsed time since request start
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed().as_millis() as u64
    }

    /// Create a child context for sub-operations
    pub fn child(&self, operation: &str) -> Self {
        Self {
            id: generate_request_id(),
            start_time: Instant::now(),
            tool_name: format!("{}::{}", self.tool_name, operation),
            client_id: self.client_id.clone(),
            user_agent: self.user_agent.clone(),
            parent_id: Some(self.id.clone()),
        }
    }
}

/// Generate a unique request ID
fn generate_request_id() -> String {
    // Use a shorter format for better readability in logs
    let uuid = Uuid::new_v4();
    let bytes = uuid.as_bytes();

    // Take first 8 bytes and encode as hex for shorter IDs
    hex::encode(&bytes[..8])
}

/// Generate a short ID for child contexts
#[allow(dead_code)]
fn generate_short_id() -> String {
    let uuid = Uuid::new_v4();
    let bytes = uuid.as_bytes();

    // Take first 4 bytes for very short child IDs
    hex::encode(&bytes[..4])
}

/// Request tracking middleware
pub struct RequestTracker;

impl RequestTracker {
    /// Log request start with structured context
    pub fn log_request_start(ctx: &RequestContext, params: &serde_json::Value) {
        let structured_ctx = Self::to_structured_context(ctx);
        StructuredLogger::log_request_start(&structured_ctx, params);
    }

    /// Log request completion with structured context
    pub fn log_request_end(
        ctx: &RequestContext,
        success: bool,
        error: Option<&crate::error::LoxoneError>,
    ) {
        let structured_ctx = Self::to_structured_context(ctx);
        StructuredLogger::log_request_end(&structured_ctx, success, error, None);
    }

    /// Log slow requests with structured context
    pub fn log_if_slow(ctx: &RequestContext, threshold_ms: u64) {
        let structured_ctx = Self::to_structured_context(ctx);
        StructuredLogger::log_slow_request(&structured_ctx, threshold_ms);
    }

    /// Create a tracing span for the request
    pub fn create_span(ctx: &RequestContext) -> tracing::Span {
        let structured_ctx = Self::to_structured_context(ctx);
        StructuredLogger::create_span(&structured_ctx)
    }

    /// Convert RequestContext to StructuredContext
    fn to_structured_context(ctx: &RequestContext) -> StructuredContext {
        let mut structured_ctx = StructuredContext::new(ctx.tool_name.clone());
        structured_ctx.request_id = ctx.id.clone();
        structured_ctx.parent_request_id = ctx.parent_id.clone();
        structured_ctx.client_id = ctx.client_id.clone();
        structured_ctx.user_agent = ctx.user_agent.clone();
        structured_ctx.start_time = ctx.start_time;
        structured_ctx
    }
}

/// Macro for easier request context creation
#[macro_export]
macro_rules! with_request_context {
    ($tool:expr, $block:block) => {{
        let ctx = $crate::server::request_context::RequestContext::new($tool.to_string());
        let _span = $crate::server::request_context::RequestTracker::create_span(&ctx);
        let _guard = _span.enter();

        $block
    }};

    ($tool:expr, $client_id:expr, $user_agent:expr, $block:block) => {{
        let ctx = $crate::server::request_context::RequestContext::with_client(
            $tool.to_string(),
            $client_id,
            $user_agent,
        );
        let _span = $crate::server::request_context::RequestTracker::create_span(&ctx);
        let _guard = _span.enter();

        $block
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_context_creation() {
        let ctx = RequestContext::new("test_tool".to_string());
        assert_eq!(ctx.tool_name, "test_tool");
        assert!(ctx.client_id.is_none());
        assert!(ctx.user_agent.is_none());
        assert!(ctx.parent_id.is_none());
        assert_eq!(ctx.id.len(), 16); // 8 bytes = 16 hex chars
    }

    #[test]
    fn test_request_context_with_client() {
        let ctx = RequestContext::with_client(
            "test_tool".to_string(),
            Some("client123".to_string()),
            Some("TestAgent/1.0".to_string()),
            None,
        );
        assert_eq!(ctx.tool_name, "test_tool");
        assert_eq!(ctx.client_id, Some("client123".to_string()));
        assert_eq!(ctx.user_agent, Some("TestAgent/1.0".to_string()));
    }

    #[test]
    fn test_child_context() {
        let parent = RequestContext::new("parent_tool".to_string());
        let child = parent.child("sub_operation");

        assert_eq!(child.parent_id, Some(parent.id.clone()));
        assert_eq!(child.tool_name, "parent_tool::sub_operation");
        assert_eq!(child.client_id, parent.client_id);
    }

    #[test]
    fn test_elapsed_time() {
        let ctx = RequestContext::new("test_tool".to_string());
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(ctx.elapsed_ms() >= 10);
    }
}
