//! Request context for MCP operations

use mcp_protocol::Implementation;
use std::collections::HashMap;
use uuid::Uuid;

/// Request context containing metadata and client information
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Unique request ID
    pub request_id: Uuid,
    /// Request metadata
    pub metadata: HashMap<String, String>,
    /// Client information
    pub client_info: Option<Implementation>,
    /// Authentication information
    pub authenticated_user: Option<String>,
    /// Authorization roles
    pub roles: Vec<String>,
}

impl RequestContext {
    /// Create a new request context
    pub fn new() -> Self {
        Self {
            request_id: Uuid::new_v4(),
            metadata: HashMap::new(),
            client_info: None,
            authenticated_user: None,
            roles: vec![],
        }
    }
    
    /// Create a request context with specific ID
    pub fn with_id(request_id: Uuid) -> Self {
        Self {
            request_id,
            metadata: HashMap::new(),
            client_info: None,
            authenticated_user: None,
            roles: vec![],
        }
    }
    
    /// Set client information
    pub fn with_client_info(mut self, client_info: Implementation) -> Self {
        self.client_info = Some(client_info);
        self
    }
    
    /// Set authenticated user
    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.authenticated_user = Some(user.into());
        self
    }
    
    /// Add a role
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.roles.push(role.into());
        self
    }
    
    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    
    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
    
    /// Check if user has a specific role
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(&role.to_string())
    }
    
    /// Check if user is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.authenticated_user.is_some()
    }
}

impl Default for RequestContext {
    fn default() -> Self {
        Self::new()
    }
}