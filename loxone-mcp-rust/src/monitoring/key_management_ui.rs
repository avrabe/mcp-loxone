//! Web UI for API Key Management

use crate::error::Result;
use crate::security::key_store::{ApiKey, ApiKeyRole, KeyStore};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{delete, get, post, Router},
    Json,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// API key information for frontend display
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: String,
    pub role: String,
    pub created_by: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub active: bool,
    pub ip_whitelist: Vec<String>,
    pub last_used: Option<String>,
    pub usage_count: u64,
}

/// Request to generate a new API key
#[derive(Debug, Deserialize)]
pub struct GenerateKeyRequest {
    pub name: String,
    pub role: String,
    pub expires_days: Option<i32>,
    pub ip_whitelist: Option<Vec<String>>,
    pub devices: Option<Vec<String>>,
}

/// Request to update an existing API key
#[derive(Debug, Deserialize)]
pub struct UpdateKeyRequest {
    pub name: Option<String>,
    pub active: Option<bool>,
    pub expires_days: Option<i32>,
    pub ip_whitelist: Option<Vec<String>>,
}

/// Key management UI controller
#[derive(Clone)]
pub struct KeyManagementUI {
    key_store: Arc<KeyStore>,
}

impl KeyManagementUI {
    /// Create new key management UI
    pub fn new(key_store: Arc<KeyStore>) -> Self {
        Self { key_store }
    }

    /// Render the main key management page
    pub async fn render_page(&self) -> Html<String> {
        Html(Self::generate_html())
    }

    /// API: List all keys
    pub async fn list_keys(&self) -> Result<Json<Vec<ApiKeyInfo>>> {
        let keys = self.key_store.list_keys().await;
        let key_infos: Vec<ApiKeyInfo> = keys
            .into_iter()
            .map(|key| ApiKeyInfo {
                id: key.id,
                name: key.name,
                role: format!("{:?}", key.role),
                created_by: key.created_by,
                created_at: key.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                expires_at: key
                    .expires_at
                    .map(|e| e.format("%Y-%m-%d %H:%M:%S").to_string()),
                active: key.active,
                ip_whitelist: key.ip_whitelist,
                last_used: key
                    .last_used
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
                usage_count: key.usage_count,
            })
            .collect();
        Ok(Json(key_infos))
    }

    /// API: Generate new key
    pub async fn generate_key(
        &self,
        request: Json<GenerateKeyRequest>,
    ) -> Result<Json<ApiKeyInfo>> {
        let role = match request.role.to_lowercase().as_str() {
            "admin" => ApiKeyRole::Admin,
            "operator" => ApiKeyRole::Operator,
            "monitor" => ApiKeyRole::Monitor,
            "device" => ApiKeyRole::Device {
                allowed_devices: request.devices.clone().unwrap_or_default(),
            },
            _ => return Err(crate::error::LoxoneError::config("Invalid role")),
        };

        let key_id = Self::generate_key_id(&role);
        let key = ApiKey {
            id: key_id.clone(),
            name: request.name.clone(),
            role,
            created_by: "Web UI".to_string(),
            created_at: Utc::now(),
            expires_at: request
                .expires_days
                .filter(|&days| days > 0)
                .map(|days| Utc::now() + Duration::days(days as i64)),
            ip_whitelist: request.ip_whitelist.clone().unwrap_or_default(),
            active: true,
            last_used: None,
            usage_count: 0,
            metadata: Default::default(),
        };

        self.key_store.add_key(key.clone()).await?;

        Ok(Json(ApiKeyInfo {
            id: key.id,
            name: key.name,
            role: format!("{:?}", key.role),
            created_by: key.created_by,
            created_at: key.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            expires_at: key
                .expires_at
                .map(|e| e.format("%Y-%m-%d %H:%M:%S").to_string()),
            active: key.active,
            ip_whitelist: key.ip_whitelist,
            last_used: None,
            usage_count: 0,
        }))
    }

    /// API: Update key
    pub async fn update_key(
        &self,
        key_id: Path<String>,
        request: Json<UpdateKeyRequest>,
    ) -> Result<StatusCode> {
        let mut key = self
            .key_store
            .get_key(&key_id)
            .await
            .ok_or_else(|| crate::error::LoxoneError::not_found("Key not found"))?;

        if let Some(name) = &request.name {
            key.name = name.clone();
        }
        if let Some(active) = request.active {
            key.active = active;
        }
        if let Some(expires_days) = request.expires_days {
            key.expires_at = if expires_days > 0 {
                Some(Utc::now() + Duration::days(expires_days as i64))
            } else {
                None
            };
        }
        if let Some(ip_whitelist) = &request.ip_whitelist {
            key.ip_whitelist = ip_whitelist.clone();
        }

        self.key_store.update_key(key).await?;
        Ok(StatusCode::OK)
    }

    /// API: Delete key
    pub async fn delete_key(&self, key_id: Path<String>) -> Result<StatusCode> {
        self.key_store.remove_key(&key_id).await?;
        Ok(StatusCode::OK)
    }

    /// Generate a new key ID
    fn generate_key_id(role: &ApiKeyRole) -> String {
        let role_prefix = match role {
            ApiKeyRole::Admin => "admin",
            ApiKeyRole::Operator => "operator",
            ApiKeyRole::Monitor => "monitor",
            ApiKeyRole::Device { .. } => "device",
            ApiKeyRole::Custom { .. } => "custom",
        };

        let seq = chrono::Utc::now().timestamp_millis() % 1000;
        let random = &Uuid::new_v4().to_string()[..12];

        format!("lmcp_{}_{:03}_{}", role_prefix, seq, random)
    }

    /// Generate the HTML for the key management UI
    fn generate_html() -> String {
        // Use the new styled version
        crate::monitoring::key_management_ui_new::generate_html()
    }
}

/// Create the key management router
pub fn create_key_management_router(key_store: Arc<KeyStore>) -> Router {
    Router::new()
        .route("/keys", get(render_page))
        .route("/keys/api/list", get(list_keys_handler))
        .route("/keys/api/generate", post(generate_key_handler))
        .route("/keys/api/:key_id", delete(delete_key_handler))
        .route("/keys/api/:key_id/activate", post(activate_key_handler))
        .route("/keys/api/:key_id/deactivate", post(deactivate_key_handler))
        .with_state(key_store)
}

async fn render_page(State(key_store): State<Arc<KeyStore>>) -> Html<String> {
    let ui = KeyManagementUI::new(key_store);
    ui.render_page().await
}

async fn list_keys_handler(
    State(key_store): State<Arc<KeyStore>>,
) -> impl axum::response::IntoResponse {
    let ui = KeyManagementUI::new(key_store);
    match ui.list_keys().await {
        Ok(response) => response.into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error: {}", e),
        )
            .into_response(),
    }
}

async fn generate_key_handler(
    State(key_store): State<Arc<KeyStore>>,
    request: Json<GenerateKeyRequest>,
) -> impl axum::response::IntoResponse {
    let ui = KeyManagementUI::new(key_store);
    match ui.generate_key(request).await {
        Ok(response) => response.into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error: {}", e),
        )
            .into_response(),
    }
}

async fn delete_key_handler(
    State(key_store): State<Arc<KeyStore>>,
    key_id: Path<String>,
) -> impl axum::response::IntoResponse {
    let ui = KeyManagementUI::new(key_store);
    match ui.delete_key(key_id).await {
        Ok(status) => status.into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error: {}", e),
        )
            .into_response(),
    }
}

async fn activate_key_handler(
    State(_key_store): State<Arc<KeyStore>>,
    _key_id: Path<String>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement activation logic
    StatusCode::OK
}

async fn deactivate_key_handler(
    State(_key_store): State<Arc<KeyStore>>,
    _key_id: Path<String>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement deactivation logic
    StatusCode::OK
}
