//! Authentication models

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// API key for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub key: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// User role
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    Admin,
    Operator,
    Viewer,
}

/// Authentication result
#[derive(Debug)]
pub struct AuthResult {
    pub success: bool,
    pub user_id: Option<String>,
    pub roles: Vec<Role>,
    pub message: Option<String>,
}

/// Authentication context
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Option<String>,
    pub roles: Vec<Role>,
    pub api_key_id: Option<String>,
}