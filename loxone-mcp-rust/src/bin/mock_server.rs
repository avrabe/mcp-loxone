//! Mock Loxone server for local testing
//!
//! This creates a simple HTTP server (configurable host:port) that mimics
//! basic Loxone Miniserver responses for development and testing.

use axum::{
    http::{Request, StatusCode},
    middleware,
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use base64::prelude::*;
use serde_json::json;
use std::sync::OnceLock;
use tracing::info;

/// Global credentials for the mock server
static MOCK_CREDENTIALS: OnceLock<String> = OnceLock::new();

/// Get or generate mock server credentials
fn get_mock_credentials() -> &'static str {
    MOCK_CREDENTIALS.get_or_init(|| {
        // Try environment variables first
        if let (Ok(user), Ok(pass)) = (std::env::var("MOCK_USER"), std::env::var("MOCK_PASS")) {
            format!("{}:{}", user, pass)
        } else {
            // Use secure default credentials (same as setup.rs)
            "mock_admin:mock_secure".to_string()
        }
    })
}

/// Basic auth middleware
async fn basic_auth_middleware(
    request: Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Result<Response, StatusCode> {
    // Check for Authorization header
    if let Some(auth_header) = request.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(encoded) = auth_str.strip_prefix("Basic ") {
                if let Ok(decoded) = BASE64_STANDARD.decode(encoded) {
                    if let Ok(credentials) = String::from_utf8(decoded) {
                        if credentials == get_mock_credentials() {
                            return Ok(next.run(request).await);
                        }
                    }
                }
            }
        }
    }

    // Return 401 Unauthorized with WWW-Authenticate header
    let mut response = Response::new(axum::body::Body::from("Unauthorized"));
    *response.status_mut() = StatusCode::UNAUTHORIZED;
    response.headers_mut().insert(
        "WWW-Authenticate",
        "Basic realm=\"Loxone Mock Server\"".parse().unwrap(),
    );
    Ok(response)
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    // Get configurable host and port
    let host = std::env::var("MOCK_SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("MOCK_SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let bind_addr = format!("{}:{}", host, port);

    // Get credentials (this will generate them if needed)
    let credentials = get_mock_credentials();
    let (_username, _password) = credentials
        .split_once(':')
        .unwrap_or(("unknown", "unknown"));

    info!("🧪 Starting Loxone Mock Server on http://{}", bind_addr);

    // In production builds, sanitize credential logging
    #[cfg(debug_assertions)]
    {
        info!("   Credentials for this session:");
        info!("   Username: {}", _username);
        info!("   Password: {}", _password);
        info!("");
        info!("   Test with:");
        info!("   export LOXONE_HOST=\"{}\"", bind_addr);
        info!("   export LOXONE_USER=\"{}\"", _username);
        info!("   export LOXONE_PASS=\"{}\"", _password);
    }
    #[cfg(not(debug_assertions))]
    {
        info!("   Credentials configured for this session");
        info!("   Export environment variables to connect");
        info!("   (Use debug build for detailed credential output)");
    }
    info!("");
    if std::env::var("MOCK_USER").is_err() || std::env::var("MOCK_PASS").is_err() {
        info!("   💡 Tip: Set MOCK_USER and MOCK_PASS environment variables for consistent credentials");
    }
    if std::env::var("MOCK_SERVER_HOST").is_err() || std::env::var("MOCK_SERVER_PORT").is_err() {
        info!("   💡 Tip: Set MOCK_SERVER_HOST and MOCK_SERVER_PORT environment variables for custom bind address");
    }

    // Create mock structure data
    let app = Router::new()
        .route("/data/LoxAPP3.json", get(get_structure))
        .route("/jdev/sps/io/:uuid/:value", get(control_device))
        .route("/jdev/sps/status", get(get_status))
        .route("/jdev/sys/getversion", get(get_version))
        .route("/", get(root))
        .layer(middleware::from_fn(basic_auth_middleware));

    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();

    info!("✅ Mock server ready!");
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> impl IntoResponse {
    (StatusCode::OK, "Loxone Mock Server")
}

async fn get_version() -> impl IntoResponse {
    Json(json!({
        "LL": {
            "control": "dev/sys/getversion",
            "value": "12.0.0.0",
            "Code": "200"
        }
    }))
}

async fn get_structure() -> impl IntoResponse {
    Json(json!({
        "msInfo": {
            "serialNr": "MOCK-SERIAL",
            "msName": "Mock Miniserver",
            "projectName": "Mock Home",
            "swVersion": "12.0.0.0"
        },
        "rooms": {
            "11111111-1111-1111-1111-111111111111": {
                "name": "Wohnzimmer",
                "uuid": "11111111-1111-1111-1111-111111111111",
                "type": 1
            },
            "22222222-2222-2222-2222-222222222222": {
                "name": "Küche",
                "uuid": "22222222-2222-2222-2222-222222222222",
                "type": 1
            }
        },
        "controls": {
            "aaaa1111-1111-1111-1111-111111111111": {
                "name": "Wohnzimmer Licht",
                "type": "Switch",
                "uuidAction": "aaaa1111-1111-1111-1111-111111111111",
                "room": "11111111-1111-1111-1111-111111111111",
                "states": {
                    "active": "aaaa1111-1111-1111-1111-111111111111"
                }
            },
            "bbbb2222-2222-2222-2222-222222222222": {
                "name": "Küche Licht",
                "type": "Switch",
                "uuidAction": "bbbb2222-2222-2222-2222-222222222222",
                "room": "22222222-2222-2222-2222-222222222222",
                "states": {
                    "active": "bbbb2222-2222-2222-2222-222222222222"
                }
            },
            "cccc3333-3333-3333-3333-333333333333": {
                "name": "Wohnzimmer Jalousie",
                "type": "Jalousie",
                "uuidAction": "cccc3333-3333-3333-3333-333333333333",
                "room": "11111111-1111-1111-1111-111111111111",
                "states": {
                    "position": "cccc3333-3333-3333-3333-333333333333",
                    "up": "cccc3333-3333-3333-3333-333333333333/up",
                    "down": "cccc3333-3333-3333-3333-333333333333/down"
                }
            }
        }
    }))
}

async fn control_device(
    axum::extract::Path((uuid, value)): axum::extract::Path<(String, String)>,
) -> impl IntoResponse {
    info!("Mock: Control device {} with value {}", uuid, value);

    Json(json!({
        "LL": {
            "control": format!("dev/sps/io/{}/{}", uuid, value),
            "value": value,
            "Code": "200"
        }
    }))
}

async fn get_status() -> impl IntoResponse {
    // Return some mock status values
    Json(json!({
        "LL": {
            "value": {
                "aaaa1111-1111-1111-1111-111111111111": 1,  // Light on
                "bbbb2222-2222-2222-2222-222222222222": 0,  // Light off
                "cccc3333-3333-3333-3333-333333333333": 0.5 // Jalousie 50%
            }
        }
    }))
}
