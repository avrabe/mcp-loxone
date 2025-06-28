//! Debug server to check query parameter parsing

use axum::{extract::Query, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Deserialize, Serialize)]
struct TestQuery {
    url: Option<String>,
    #[serde(rename = "transportType")]
    transport_type: Option<String>,
}

async fn test_handler(
    Query(params): Query<HashMap<String, String>>,
    Query(typed_params): Query<TestQuery>,
) -> Json<serde_json::Value> {
    info!("Raw query params: {:?}", params);
    info!("Typed query params: {:?}", typed_params);

    Json(serde_json::json!({
        "raw_params": params,
        "typed_params": typed_params,
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = Router::new().route("/test", get(test_handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3002")
        .await
        .unwrap();

    info!("Test server running on http://127.0.0.1:3002");
    info!("Try: curl 'http://127.0.0.1:3002/test?url=http://localhost:3001/sse&transportType=sse'");

    axum::serve(listener, app).await.unwrap();
}
