//! Debug server to capture full request details

use axum::{
    extract::{Query, Request},
    http::Uri,
    response::Json,
    routing::get,
    Router,
};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Deserialize)]
struct DebugQuery {
    url: Option<String>,
    #[serde(rename = "transportType")]
    transport_type: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}

async fn debug_handler(
    uri: Uri,
    Query(raw_params): Query<HashMap<String, String>>,
    Query(typed_params): Query<DebugQuery>,
    request: Request,
) -> Json<serde_json::Value> {
    info!("=== FULL REQUEST DEBUG ===");
    info!("URI: {}", uri);
    info!("Query string: {:?}", uri.query());
    info!("Raw params: {:?}", raw_params);
    info!("Typed params: {:?}", typed_params);
    info!("Headers: {:?}", request.headers());
    info!("Method: {}", request.method());
    info!("==========================");
    
    Json(serde_json::json!({
        "uri": uri.to_string(),
        "query_string": uri.query(),
        "raw_params": raw_params,
        "typed_params": typed_params,
        "method": request.method().to_string(),
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let app = Router::new()
        .route("/sse", get(debug_handler))
        .route("/debug", get(debug_handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3003")
        .await
        .unwrap();
        
    info!("Debug server running on http://127.0.0.1:3003");
    info!("Point MCP Inspector to: http://localhost:3003");
    info!("Or test manually:");
    info!("  curl 'http://127.0.0.1:3003/sse?url=test&transportType=sse'");
    
    axum::serve(listener, app).await.unwrap();
}