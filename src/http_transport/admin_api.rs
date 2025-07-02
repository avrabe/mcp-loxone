//! Admin API endpoints for key management
//!
//! This module provides HTTP endpoints for managing API keys through
//! the web interface, complementing the CLI tool.

use crate::auth::{
    models::{ApiKey, Role},
    AuthenticationManager,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

/// Request to create a new API key
#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    /// Human-readable name for the key
    pub name: String,
    /// Role for the API key
    pub role: RoleDto,
    /// Expiration in days (None = never expires)
    pub expires_days: Option<u32>,
    /// IP whitelist (empty = all IPs allowed)
    #[serde(default)]
    pub ip_whitelist: Vec<String>,
}

/// Request to update an existing API key
#[derive(Debug, Deserialize)]
pub struct UpdateKeyRequest {
    /// New name (optional)
    pub name: Option<String>,
    /// Activate or deactivate the key
    pub active: Option<bool>,
    /// New expiration in days from now (0 = never expires)
    pub expires_days: Option<u32>,
    /// New IP whitelist (empty = all IPs allowed)
    pub ip_whitelist: Option<Vec<String>>,
}

/// Role DTO for API serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RoleDto {
    Admin,
    Operator,
    Monitor,
    Device { allowed_devices: Vec<String> },
    Custom { permissions: Vec<String> },
}

impl From<RoleDto> for Role {
    fn from(dto: RoleDto) -> Self {
        match dto {
            RoleDto::Admin => Role::Admin,
            RoleDto::Operator => Role::Operator,
            RoleDto::Monitor => Role::Monitor,
            RoleDto::Device { allowed_devices } => Role::Device { allowed_devices },
            RoleDto::Custom { permissions } => Role::Custom { permissions },
        }
    }
}

impl From<Role> for RoleDto {
    fn from(role: Role) -> Self {
        match role {
            Role::Admin => RoleDto::Admin,
            Role::Operator => RoleDto::Operator,
            Role::Monitor => RoleDto::Monitor,
            Role::Device { allowed_devices } => RoleDto::Device { allowed_devices },
            Role::Custom { permissions } => RoleDto::Custom { permissions },
        }
    }
}

/// API key response DTO (without secret)
#[derive(Debug, Serialize)]
pub struct ApiKeyDto {
    pub id: String,
    pub name: String,
    pub role: RoleDto,
    pub created_by: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub ip_whitelist: Vec<String>,
    pub active: bool,
    pub last_used: Option<String>,
    pub usage_count: u64,
}

impl From<ApiKey> for ApiKeyDto {
    fn from(key: ApiKey) -> Self {
        Self {
            id: key.id,
            name: key.name,
            role: key.role.into(),
            created_by: key.created_by,
            created_at: key.created_at.to_rfc3339(),
            expires_at: key.expires_at.map(|dt| dt.to_rfc3339()),
            ip_whitelist: key.ip_whitelist,
            active: key.active,
            last_used: key.last_used.map(|dt| dt.to_rfc3339()),
            usage_count: key.usage_count,
        }
    }
}

/// API key creation response (includes secret)
#[derive(Debug, Serialize)]
pub struct ApiKeyCreatedDto {
    #[serde(flatten)]
    pub key: ApiKeyDto,
    /// The secret token (only returned on creation)
    pub secret: String,
}

/// List all API keys
pub async fn list_keys(
    State(auth_manager): State<Arc<AuthenticationManager>>,
) -> impl IntoResponse {
    let keys = auth_manager.list_keys().await;
    let key_dtos: Vec<ApiKeyDto> = keys.into_iter().map(Into::into).collect();

    Json(serde_json::json!({
        "keys": key_dtos,
        "total": key_dtos.len()
    }))
}

/// Get a specific API key
pub async fn get_key(
    State(auth_manager): State<Arc<AuthenticationManager>>,
    Path(key_id): Path<String>,
) -> impl IntoResponse {
    if let Some(key) = auth_manager.get_key(&key_id).await {
        Json(ApiKeyDto::from(key)).into_response()
    } else {
        (StatusCode::NOT_FOUND, "API key not found").into_response()
    }
}

/// Create a new API key
pub async fn create_key(
    State(auth_manager): State<Arc<AuthenticationManager>>,
    Json(request): Json<CreateKeyRequest>,
) -> impl IntoResponse {
    // TODO: Get the actual authenticated user from request context
    let created_by = "web_admin".to_string();

    let key_result = auth_manager
        .create_key(
            request.name,
            request.role.into(),
            created_by,
            request.expires_days,
        )
        .await;

    let mut key = match key_result {
        Ok(k) => k,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create key: {e}"),
            )
                .into_response();
        }
    };

    // Set IP whitelist if provided
    if !request.ip_whitelist.is_empty() {
        key.ip_whitelist = request.ip_whitelist;
        if let Err(e) = auth_manager.update_key(key.clone()).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to update key: {e}"),
            )
                .into_response();
        }
    }

    info!("Created new API key via web interface: {}", key.id);

    let response = ApiKeyCreatedDto {
        secret: key.secret.clone(),
        key: key.into(),
    };

    (StatusCode::CREATED, Json(response)).into_response()
}

/// Update an existing API key
pub async fn update_key(
    State(auth_manager): State<Arc<AuthenticationManager>>,
    Path(key_id): Path<String>,
    Json(request): Json<UpdateKeyRequest>,
) -> impl IntoResponse {
    if let Some(mut key) = auth_manager.get_key(&key_id).await {
        let mut updated = false;

        if let Some(new_name) = request.name {
            key.name = new_name;
            updated = true;
        }

        if let Some(is_active) = request.active {
            key.active = is_active;
            updated = true;
        }

        if let Some(expires_days) = request.expires_days {
            key.expires_at = if expires_days == 0 {
                None
            } else {
                Some(chrono::Utc::now() + chrono::Duration::days(expires_days as i64))
            };
            updated = true;
        }

        if let Some(ip_whitelist) = request.ip_whitelist {
            key.ip_whitelist = ip_whitelist;
            updated = true;
        }

        if updated {
            if let Err(e) = auth_manager.update_key(key.clone()).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to update key: {e}"),
                )
                    .into_response();
            }
            info!("Updated API key via web interface: {}", key_id);
            Json(ApiKeyDto::from(key)).into_response()
        } else {
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "message": "No changes specified"
                })),
            )
                .into_response()
        }
    } else {
        (StatusCode::NOT_FOUND, "API key not found").into_response()
    }
}

/// Delete an API key
pub async fn delete_key(
    State(auth_manager): State<Arc<AuthenticationManager>>,
    Path(key_id): Path<String>,
) -> impl IntoResponse {
    match auth_manager.delete_key(&key_id).await {
        Ok(true) => {
            info!("Deleted API key via web interface: {}", key_id);
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => {
            warn!("Attempted to delete non-existent API key: {}", key_id);
            StatusCode::NOT_FOUND.into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete key: {e}"),
        )
            .into_response(),
    }
}

/// Get authentication statistics
pub async fn get_auth_stats(
    State(auth_manager): State<Arc<AuthenticationManager>>,
) -> impl IntoResponse {
    let stats = auth_manager.get_auth_stats().await;

    Json(serde_json::json!({
        "total_keys": stats.total_keys,
        "active_keys": stats.active_keys,
        "expired_keys": stats.expired_keys,
        "blocked_ips": stats.currently_blocked_ips,
        "failed_attempts": stats.total_failed_attempts,
    }))
}

/// Get recent audit events
pub async fn get_audit_events(
    State(auth_manager): State<Arc<AuthenticationManager>>,
) -> impl IntoResponse {
    match auth_manager.get_audit_events(100).await {
        Ok(events) => Json(serde_json::json!({
            "events": events,
            "total": events.len()
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get audit events: {e}"),
        )
            .into_response(),
    }
}
