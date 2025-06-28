//! Streamable HTTP transport implementation for MCP
//!
//! This implements the newer streamable-http transport that MCP Inspector expects,
//! which replaces the deprecated SSE transport.

use crate::{RequestHandler, Transport, TransportError};
use async_trait::async_trait;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Configuration for Streamable HTTP transport
#[derive(Debug, Clone)]
pub struct StreamableHttpConfig {
    pub port: u16,
    pub host: String,
    pub enable_cors: bool,
}

impl Default for StreamableHttpConfig {
    fn default() -> Self {
        Self {
            port: 3001,
            host: "127.0.0.1".to_string(),
            enable_cors: true,
        }
    }
}

/// Session information
#[derive(Debug, Clone)]
struct SessionInfo {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    created_at: std::time::Instant,
}

/// Shared state
#[derive(Clone)]
struct AppState {
    handler: Arc<RequestHandler>,
    sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
}

/// Query parameters for SSE endpoint
#[derive(Debug, Deserialize)]
struct StreamQuery {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}

/// Streamable HTTP transport
pub struct StreamableHttpTransport {
    config: StreamableHttpConfig,
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

impl StreamableHttpTransport {
    pub fn new(port: u16) -> Self {
        Self {
            config: StreamableHttpConfig {
                port,
                ..Default::default()
            },
            server_handle: None,
        }
    }

    /// Create or get session
    async fn ensure_session(state: &AppState, session_id: Option<String>) -> String {
        if let Some(id) = session_id {
            // Check if session exists
            let sessions = state.sessions.read().await;
            if sessions.contains_key(&id) {
                return id;
            }
        }

        // Create new session
        let id = Uuid::new_v4().to_string();
        let session = SessionInfo {
            id: id.clone(),
            created_at: std::time::Instant::now(),
        };

        let mut sessions = state.sessions.write().await;
        sessions.insert(id.clone(), session);
        info!("Created new session: {}", id);

        id
    }
}

/// Handle POST requests for client-to-server messages
async fn handle_messages(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    debug!("Received POST /messages: {}", body);

    // Get or create session
    let session_id = headers
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let session_id = StreamableHttpTransport::ensure_session(&state, session_id).await;

    // Parse the request
    let request: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to parse request: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32700,
                        "message": "Parse error"
                    },
                    "id": null
                })),
            )
                .into_response();
        }
    };

    // Convert to MCP Request
    let mcp_request: pulseengine_mcp_protocol::Request = match serde_json::from_value(request.clone()) {
        Ok(r) => r,
        Err(e) => {
            warn!("Invalid request format: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32600,
                        "message": "Invalid request"
                    },
                    "id": request.get("id").cloned().unwrap_or(Value::Null)
                })),
            )
                .into_response();
        }
    };

    // Process through handler
    let response = (state.handler)(mcp_request).await;

    // Return JSON response with session header
    let mut headers = HeaderMap::new();
    headers.insert("Mcp-Session-Id", session_id.parse().unwrap());

    (StatusCode::OK, headers, Json(response)).into_response()
}

/// Handle SSE requests for server-to-client streaming
async fn handle_sse(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StreamQuery>,
) -> impl IntoResponse {
    info!("SSE connection request: {:?}", query);

    // For streamable-http, we need to handle this differently
    // The client expects an immediate response, not an SSE stream

    // Get or create session
    let session_id = StreamableHttpTransport::ensure_session(&state, query.session_id).await;

    // Return a simple response indicating the connection is established
    // This is what MCP Inspector expects for streamable-http
    let response = serde_json::json!({
        "type": "connection",
        "status": "connected",
        "sessionId": session_id,
        "transport": "streamable-http"
    });

    Json(response)
}

#[async_trait]
impl Transport for StreamableHttpTransport {
    async fn start(&mut self, handler: RequestHandler) -> Result<(), TransportError> {
        info!(
            "Starting Streamable HTTP transport on {}:{}",
            self.config.host, self.config.port
        );

        let state = Arc::new(AppState {
            handler: Arc::new(handler),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        });

        // Build router
        let app = Router::new()
            .route("/messages", post(handle_messages))
            .route("/sse", get(handle_sse))
            .route("/", get(|| async { "MCP Streamable HTTP Server" }))
            .layer(ServiceBuilder::new().layer(if self.config.enable_cors {
                CorsLayer::permissive()
            } else {
                CorsLayer::new()
            }))
            .with_state(state);

        // Start server
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| TransportError::Config(format!("Invalid address: {}", e)))?;

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| TransportError::Connection(format!("Failed to bind: {}", e)))?;

        info!("Streamable HTTP transport listening on {}", addr);
        info!("Endpoints:");
        info!("  POST http://{}/messages - MCP messages", addr);
        info!("  GET  http://{}/sse      - Session establishment", addr);

        let server_handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("Server error: {}", e);
            }
        });

        self.server_handle = Some(server_handle);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), TransportError> {
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
        }
        Ok(())
    }

    async fn health_check(&self) -> Result<(), TransportError> {
        if self.server_handle.is_some() {
            Ok(())
        } else {
            Err(TransportError::Connection("Not running".to_string()))
        }
    }
}
