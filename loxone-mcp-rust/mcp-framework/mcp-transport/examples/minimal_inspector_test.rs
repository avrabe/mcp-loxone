//! Minimal test to determine what MCP Inspector expects

use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Deserialize)]
struct AnyQuery {
    #[serde(flatten)]
    params: HashMap<String, String>,
}

async fn handle_any_sse(uri: Uri, Query(query): Query<AnyQuery>, headers: HeaderMap) -> Response {
    info!("=== SSE REQUEST ===");
    info!("URI: {}", uri);
    info!("Query string: {:?}", uri.query());
    info!("Parsed params: {:?}", query.params);
    info!("Headers: {:?}", headers);
    info!("==================");

    // Try different response formats

    // 1. Simple JSON response (for streamable-http)
    if headers
        .get("accept")
        .map(|v| v.to_str().unwrap_or(""))
        .unwrap_or("")
        .contains("text/event-stream")
    {
        info!("Client wants SSE, but returning JSON to test Inspector");
    }

    let response = json!({
        "type": "connection",
        "status": "connected",
        "sessionId": "test-session-123",
        "transport": "streamable-http",
        "server": "minimal-test"
    });

    Json(response).into_response()
}

async fn handle_any_post(uri: Uri, headers: HeaderMap, body: String) -> Response {
    info!("=== POST REQUEST ===");
    info!("URI: {}", uri);
    info!("Headers: {:?}", headers);
    info!("Body: {}", body);
    info!("===================");

    // Try to parse as JSON
    if let Ok(json_body) = serde_json::from_str::<serde_json::Value>(&body) {
        info!("Parsed JSON: {:?}", json_body);

        // Simple echo response
        let response = json!({
            "jsonrpc": "2.0",
            "id": json_body.get("id"),
            "result": {
                "echo": "received",
                "method": json_body.get("method")
            }
        });

        return Json(response).into_response();
    }

    StatusCode::BAD_REQUEST.into_response()
}

async fn handle_root() -> &'static str {
    "MCP Inspector Test Server - Connect Inspector here"
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = Router::new()
        .route("/", get(handle_root))
        .route("/sse", get(handle_any_sse))
        .route("/messages", post(handle_any_post))
        // Also try common MCP endpoints
        .route("/mcp", post(handle_any_post))
        .route("/mcp/sse", get(handle_any_sse));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3004")
        .await
        .unwrap();

    info!("🔍 MCP Inspector Debug Server");
    info!("Running on: http://127.0.0.1:3004");
    info!("Point MCP Inspector to this URL and check logs");

    axum::serve(listener, app).await.unwrap();
}
