//! HTTP transport with Server-Sent Events (SSE) support

use crate::{
    Transport, TransportError, RequestHandler,
    validation::{validate_message_string},
    batch::{JsonRpcMessage, process_batch},
};
use async_trait::async_trait;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header::{CONTENT_TYPE, ORIGIN, AUTHORIZATION}},
    response::{Response as AxumResponse, Sse, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use axum::response::sse::{Event, KeepAlive};
// futures_util used for async_stream
// mcp_protocol types are imported via batch module
use serde::Deserialize;
use serde_json;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::sync::{broadcast, RwLock, Mutex};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for HTTP transport
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// Port to bind to
    pub port: u16,
    /// Host to bind to (default: localhost)
    pub host: String,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Enable CORS
    pub enable_cors: bool,
    /// Allowed origins (None = allow any)
    pub allowed_origins: Option<Vec<String>>,
    /// Enable message validation
    pub validate_messages: bool,
    /// Session timeout in seconds
    pub session_timeout_secs: u64,
    /// Enable authentication
    pub require_auth: bool,
    /// Valid bearer tokens (for testing)
    pub valid_tokens: Vec<String>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            port: 3000,
            host: "127.0.0.1".to_string(),
            max_message_size: 10 * 1024 * 1024, // 10MB
            enable_cors: true,
            allowed_origins: None,
            validate_messages: true,
            session_timeout_secs: 300, // 5 minutes
            require_auth: false,
            valid_tokens: vec![],
        }
    }
}

/// Session information for HTTP clients
#[derive(Clone)]
struct SessionInfo {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    created_at: std::time::Instant,
    last_activity: std::time::Instant,
    event_sender: broadcast::Sender<String>,
    // Keep at least one receiver alive to prevent channel closure
    #[allow(dead_code)]
    _keepalive_receiver: Arc<Mutex<broadcast::Receiver<String>>>,
}

/// Shared state for HTTP transport
#[derive(Clone)]
struct HttpState {
    handler: Arc<RequestHandler>,
    config: HttpConfig,
    sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
}

/// Query parameters for SSE endpoint
#[derive(Debug, Deserialize)]
struct SseQuery {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "lastEventId")]
    #[allow(dead_code)]
    last_event_id: Option<String>,
    #[serde(rename = "transportType")]
    #[allow(dead_code)]
    transport_type: Option<String>,
    #[allow(dead_code)]
    url: Option<String>,
}

/// Request body for POST endpoint
#[derive(Debug, Deserialize)]
struct McpRequest {
    #[serde(flatten)]
    message: serde_json::Value,
}

/// HTTP transport for MCP protocol
/// 
/// Implements the MCP HTTP transport specification:
/// - HTTP POST for client-to-server messages
/// - Server-Sent Events (SSE) for server-to-client streaming
/// - Session management with Mcp-Session-Id header
/// - Origin validation and CORS support
/// - Authentication support
pub struct HttpTransport {
    config: HttpConfig,
    state: Option<HttpState>,
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

impl HttpTransport {
    /// Create a new HTTP transport with default configuration
    pub fn new(port: u16) -> Self {
        let config = HttpConfig {
            port,
            ..Default::default()
        };
        
        Self {
            config,
            state: None,
            server_handle: None,
        }
    }
    
    /// Send a message to all connected SSE clients
    pub async fn broadcast_message(&self, message: &str) -> Result<(), TransportError> {
        if let Some(ref state) = self.state {
            let sessions = state.sessions.read().await;
            for (session_id, session) in sessions.iter() {
                if let Err(e) = session.event_sender.send(message.to_string()) {
                    debug!("Failed to send to session {}: {}", session_id, e);
                }
            }
            Ok(())
        } else {
            Err(TransportError::Connection("Transport not started".to_string()))
        }
    }
    
    /// Create a new HTTP transport with custom configuration
    pub fn with_config(config: HttpConfig) -> Self {
        Self {
            config,
            state: None,
            server_handle: None,
        }
    }
    
    /// Create a new session
    async fn create_session(state: Arc<HttpState>) -> String {
        let session_id = Uuid::new_v4().to_string();
        // Create a broadcast channel with a reasonable buffer size
        // Keep at least one receiver alive to prevent the channel from closing
        let (tx, keepalive_rx) = broadcast::channel(1024);
        
        let session_info = SessionInfo {
            id: session_id.clone(),
            created_at: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
            event_sender: tx,
            _keepalive_receiver: Arc::new(Mutex::new(keepalive_rx)),
        };
        
        {
            let mut sessions = state.sessions.write().await;
            sessions.insert(session_id.clone(), session_info);
        }
        
        debug!("Created new session: {}", session_id);
        session_id
    }
    
    /// Update session activity
    async fn update_session_activity(state: Arc<HttpState>, session_id: &str) {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.last_activity = std::time::Instant::now();
        }
    }
    
    /// Clean up expired sessions
    async fn cleanup_sessions(state: Arc<HttpState>) {
        let timeout = Duration::from_secs(state.config.session_timeout_secs);
        let now = std::time::Instant::now();
        
        let mut sessions = state.sessions.write().await;
        sessions.retain(|id, session| {
            let expired = now.duration_since(session.last_activity) > timeout;
            if expired {
                debug!("Removing expired session: {}", id);
            }
            !expired
        });
    }
    
    /// Validate origin header
    fn validate_origin(config: &HttpConfig, headers: &HeaderMap) -> Result<(), TransportError> {
        if let Some(allowed_origins) = &config.allowed_origins {
            if let Some(origin) = headers.get(ORIGIN) {
                let origin_str = origin.to_str()
                    .map_err(|_| TransportError::Protocol("Invalid Origin header".to_string()))?;
                
                if !allowed_origins.contains(&origin_str.to_string()) {
                    return Err(TransportError::Protocol(format!("Origin not allowed: {}", origin_str)));
                }
            } else {
                return Err(TransportError::Protocol("Missing Origin header".to_string()));
            }
        }
        
        Ok(())
    }
    
    /// Validate authentication
    fn validate_auth(config: &HttpConfig, headers: &HeaderMap) -> Result<(), TransportError> {
        if !config.require_auth {
            return Ok(());
        }
        
        let auth_header = headers.get(AUTHORIZATION)
            .ok_or_else(|| TransportError::Protocol("Missing Authorization header".to_string()))?;
        
        let auth_str = auth_header.to_str()
            .map_err(|_| TransportError::Protocol("Invalid Authorization header".to_string()))?;
        
        if let Some(token) = auth_str.strip_prefix("Bearer ") {
            if config.valid_tokens.contains(&token.to_string()) {
                Ok(())
            } else {
                Err(TransportError::Protocol("Invalid bearer token".to_string()))
            }
        } else {
            Err(TransportError::Protocol("Invalid Authorization format, expected Bearer token".to_string()))
        }
    }
}

/// Query parameters for POST messages endpoint
#[derive(Debug, Deserialize)]
struct PostQuery {
    session_id: Option<String>,
}

/// Handle POST requests (client-to-server messages)
async fn handle_post(
    State(state): State<Arc<HttpState>>,
    Query(query): Query<PostQuery>,
    headers: HeaderMap,
    Json(request): Json<McpRequest>,
) -> Result<AxumResponse<String>, StatusCode> {
    info!("Received POST request with session query: {:?}", query);
    info!("Request message: {:?}", request.message);
    
    // Validate origin
    if let Err(e) = HttpTransport::validate_origin(&state.config, &headers) {
        warn!("Origin validation failed: {}", e);
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Validate authentication
    if let Err(e) = HttpTransport::validate_auth(&state.config, &headers) {
        warn!("Authentication failed: {}", e);
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    // Get session ID from query parameter (MCP standard) or header (fallback)
    let session_id = if let Some(id) = query.session_id {
        id
    } else if let Some(id) = headers.get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string()) {
        id
    } else {
        // Create new session for first request
        HttpTransport::create_session(state.clone()).await
    };
    
    // Validate message
    let message_json = serde_json::to_string(&request.message)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    if state.config.validate_messages {
        if let Err(e) = validate_message_string(&message_json, Some(state.config.max_message_size)) {
            warn!("Message validation failed: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    
    // Parse and process message
    let message = JsonRpcMessage::parse(&message_json)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Validate JSON-RPC structure
    if let Err(e) = message.validate() {
        warn!("JSON-RPC validation failed: {}", e);
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Update session activity
    {
        HttpTransport::update_session_activity(state.clone(), &session_id).await;
    }
    
    // Process the message
    match process_batch(message, &state.handler).await {
        Ok(Some(response_message)) => {
            let response_json = response_message.to_string()
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            // Implement proper MCP backwards compatibility protocol
            let accept_header = headers.get("accept").and_then(|v| v.to_str().ok()).unwrap_or("");
            
            // MCP Inspector expects immediate JSON responses for streamable HTTP transport
            // If Accept header contains "application/json" but NOT "text/event-stream", 
            // it's trying to use the new streamable HTTP transport
            let wants_json_response = accept_header.contains("application/json") && 
                                    !accept_header.contains("text/event-stream");
            
            if wants_json_response {
                // New Streamable HTTP transport - return response directly
                info!("Using Streamable HTTP transport, returning response directly for session: {}, Accept: {}", session_id, accept_header);
                debug!("Direct response: {}", response_json);
                Ok(AxumResponse::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .header("Mcp-Session-Id", session_id)
                    .body(response_json)
                    .unwrap())
            } else {
                // Legacy HTTP+SSE transport - send through SSE
                info!("Using legacy HTTP+SSE transport for session: {}, Accept: {}", session_id, accept_header);
                debug!("Response to send through SSE: {}", response_json);
                
                let sessions = state.sessions.read().await;
                info!("Active sessions: {}", sessions.len());
                
                if let Some(session) = sessions.get(&session_id) {
                    info!("Found session {}, sending response", session_id);
                    match session.event_sender.send(response_json.clone()) {
                        Ok(num_receivers) => {
                            info!("Response sent successfully to {} receivers on session: {}", num_receivers, session_id);
                        }
                        Err(e) => {
                            warn!("Failed to send response through SSE: {}", e);
                        }
                    }
                } else {
                    warn!("Session {} not found for response, trying any active session", session_id);
                    // Fallback: try to send to any active session (for MCP Inspector compatibility)
                    let mut sent = false;
                    for (sid, session) in sessions.iter() {
                        match session.event_sender.send(response_json.clone()) {
                            Ok(num_receivers) => {
                                info!("Response sent successfully to {} receivers on fallback session: {}", num_receivers, sid);
                                sent = true;
                                break;
                            }
                            Err(e) => {
                                debug!("Failed to send to session {}: {}", sid, e);
                            }
                        }
                    }
                    if !sent {
                        warn!("No active sessions available to send response");
                    }
                }
                
                // Return 204 No Content (response sent through SSE)
                Ok(AxumResponse::builder()
                    .status(StatusCode::NO_CONTENT)
                    .header("Mcp-Session-Id", session_id)
                    .body("".to_string())
                    .unwrap())
            }
        }
        Ok(None) => {
            // No response needed (notifications only)
            Ok(AxumResponse::builder()
                .status(StatusCode::NO_CONTENT)
                .body("".to_string())
                .unwrap())
        }
        Err(e) => {
            error!("Failed to process message: {}", e);
            
            // Create error response
            let error_response = mcp_protocol::Response {
                jsonrpc: "2.0".to_string(),
                id: serde_json::Value::Null,
                result: None,
                error: Some(mcp_protocol::Error::internal_error(e.to_string())),
            };
            
            if let Ok(error_json) = serde_json::to_string(&error_response) {
                // Use same transport detection logic as for success responses
                let accept_header = headers.get("accept").and_then(|v| v.to_str().ok()).unwrap_or("");
                let wants_json_response = accept_header.contains("application/json") && 
                                        !accept_header.contains("text/event-stream");
                
                if wants_json_response {
                    // New Streamable HTTP transport - return error directly
                    debug!("Using Streamable HTTP transport, returning error directly: {}", error_json);
                    Ok(AxumResponse::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/json")
                        .header("Mcp-Session-Id", session_id)
                        .body(error_json)
                        .unwrap())
                } else {
                    // Legacy HTTP+SSE transport - send through SSE
                    debug!("Using legacy HTTP+SSE transport, sending error through SSE: {}", error_json);
                    let sessions = state.sessions.read().await;
                    if let Some(session) = sessions.get(&session_id) {
                        if let Err(e) = session.event_sender.send(error_json.clone()) {
                            warn!("Failed to send error through SSE: {}", e);
                        } else {
                            debug!("Error response sent successfully to session: {}", session_id);
                        }
                    } else {
                        warn!("Session {} not found for error response, trying any active session", session_id);
                        // Fallback: try to send to any active session (for MCP Inspector compatibility)
                        let mut sent = false;
                        for (sid, session) in sessions.iter() {
                            if let Ok(_) = session.event_sender.send(error_json.clone()) {
                                debug!("Error response sent successfully to fallback session: {}", sid);
                                sent = true;
                                break;
                            }
                        }
                        if !sent {
                            warn!("No active sessions available to send error response");
                        }
                    }
                    
                    // Return 204 No Content (error sent through SSE)
                    Ok(AxumResponse::builder()
                        .status(StatusCode::NO_CONTENT)
                        .body("".to_string())
                        .unwrap())
                }
            } else {
                // Fallback to HTTP error
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

/// Handle SSE requests (server-to-client streaming)
async fn handle_sse(
    uri: axum::http::Uri,
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
    Query(query): Query<SseQuery>,
) -> Result<axum::response::Response, StatusCode> {
    info!("Received SSE request - URI: {}, query string: {:?}, parsed query: {:?}", 
          uri, uri.query(), query);
    info!("Headers: {:?}", headers);
    
    // Validate origin
    if let Err(e) = HttpTransport::validate_origin(&state.config, &headers) {
        warn!("Origin validation failed: {}", e);
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Validate authentication
    if let Err(e) = HttpTransport::validate_auth(&state.config, &headers) {
        warn!("Authentication failed: {}", e);
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    // Get or create session
    let session_id = if let Some(session_id) = query.session_id {
        // Verify session exists
        let sessions = state.sessions.read().await;
        if sessions.contains_key(&session_id) {
            session_id
        } else {
            return Err(StatusCode::BAD_REQUEST);
        }
    } else {
        // Create new session
        let new_session_id = HttpTransport::create_session(state.clone()).await;
        info!("Created new SSE session: {}", new_session_id);
        new_session_id
    };
    
    // All MCP clients expect SSE with "endpoint" event first (based on official Python SDK)
    info!("Creating MCP-compliant SSE stream with endpoint event");
    
    // Get the event receiver for this session
    let receiver = {
        let sessions = state.sessions.read().await;
        sessions.get(&session_id)
            .map(|session| session.event_sender.subscribe())
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
    };
    
    info!("Starting SSE stream for session: {}", session_id);
    
    // Create SSE stream following official MCP Python SDK pattern
    let stream = async_stream::stream! {
        // Send "endpoint" event first (as per official MCP SDK)
        let endpoint_url = format!("/messages?session_id={}", session_id);
        info!("Sending 'endpoint' event for session: {} with URL: {}", session_id, endpoint_url);
        yield Ok::<_, axum::Error>(Event::default()
            .event("endpoint")
            .data(endpoint_url));
        
        // Stream events from the receiver
        let mut receiver = receiver;
        loop {
            tokio::select! {
                Ok(data) = receiver.recv() => {
                    yield Ok::<_, axum::Error>(Event::default()
                        .event("message")
                        .data(data));
                }
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    // Send periodic ping to keep connection alive
                    yield Ok::<_, axum::Error>(Event::default()
                        .event("ping")
                        .data(serde_json::json!({
                            "type": "ping",
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }).to_string()));
                }
            }
        }
    };
    
    // Build SSE response with proper headers and keep-alive
    let sse = Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive")
        );
    
    // Convert to Response and add headers
    let mut response = sse.into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        "no-cache".parse().unwrap()
    );
    response.headers_mut().insert(
        axum::http::header::CONNECTION,
        "keep-alive".parse().unwrap()
    );
    response.headers_mut().insert(
        "X-Accel-Buffering",
        "no".parse().unwrap()
    );
    
    Ok(response)
}

/// Handle health check requests
async fn handle_health() -> &'static str {
    "OK"
}

#[async_trait]
impl Transport for HttpTransport {
    async fn start(&mut self, handler: RequestHandler) -> Result<(), TransportError> {
        info!("Starting HTTP transport on {}:{}", self.config.host, self.config.port);
        
        let state = Arc::new(HttpState {
            handler: Arc::new(handler),
            config: self.config.clone(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        });
        
        // Build CORS layer - be very permissive for MCP Inspector
        let cors = CorsLayer::very_permissive()
            .expose_headers(vec![
                axum::http::header::HeaderName::from_static("mcp-session-id"),
                axum::http::header::HeaderName::from_static("content-type"),
            ]);
        
        // Build router
        let app = Router::new()
            .route("/messages", post(handle_post))
            .route("/sse", get(handle_sse))
            .route("/health", get(handle_health))
            .layer(ServiceBuilder::new().layer(cors))
            .with_state(state.clone());
        
        // Start session cleanup task
        let cleanup_state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                HttpTransport::cleanup_sessions(cleanup_state.clone()).await;
            }
        });
        
        // Start server
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| TransportError::Config(format!("Invalid address: {}", e)))?;
        
        let listener = tokio::net::TcpListener::bind(addr).await
            .map_err(|e| TransportError::Connection(format!("Failed to bind to {}: {}", addr, e)))?;
        
        info!("HTTP transport listening on {}", addr);
        info!("Endpoints:");
        info!("  POST   http://{}/messages   - MCP messages", addr);
        info!("  GET    http://{}/sse        - Server-Sent Events", addr);
        info!("  GET    http://{}/health     - Health check", addr);
        
        let server_handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("HTTP server error: {}", e);
            }
        });
        
        self.state = Some(HttpState {
            handler: state.handler.clone(),
            config: state.config.clone(),
            sessions: state.sessions.clone(),
        });
        self.server_handle = Some(server_handle);
        
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<(), TransportError> {
        info!("Stopping HTTP transport");
        
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
        }
        
        self.state = None;
        Ok(())
    }
    
    async fn health_check(&self) -> Result<(), TransportError> {
        if self.state.is_some() {
            Ok(())
        } else {
            Err(TransportError::Connection("HTTP transport not running".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Mock handler for testing
    fn mock_handler(request: mcp_protocol::Request) -> std::pin::Pin<Box<dyn std::future::Future<Output = mcp_protocol::Response> + Send>> {
        Box::pin(async move {
            mcp_protocol::Response {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(json!({"echo": request.method})),
                error: None,
            }
        })
    }

    #[test]
    fn test_http_config() {
        let config = HttpConfig {
            port: 8080,
            host: "0.0.0.0".to_string(),
            max_message_size: 1024,
            enable_cors: false,
            allowed_origins: Some(vec!["http://localhost:3000".to_string()]),
            validate_messages: true,
            session_timeout_secs: 600,
            require_auth: true,
            valid_tokens: vec!["test-token".to_string()],
        };
        
        let transport = HttpTransport::with_config(config.clone());
        assert_eq!(transport.config.port, 8080);
        assert_eq!(transport.config.host, "0.0.0.0");
        assert!(!transport.config.enable_cors);
        assert!(transport.config.require_auth);
    }

    #[test]
    fn test_validate_origin() {
        let mut config = HttpConfig::default();
        config.allowed_origins = Some(vec!["http://localhost:3000".to_string()]);
        
        let mut headers = HeaderMap::new();
        headers.insert(ORIGIN, "http://localhost:3000".parse().unwrap());
        
        assert!(HttpTransport::validate_origin(&config, &headers).is_ok());
        
        headers.insert(ORIGIN, "http://evil.com".parse().unwrap());
        assert!(HttpTransport::validate_origin(&config, &headers).is_err());
    }

    #[test]
    fn test_validate_auth() {
        let mut config = HttpConfig::default();
        config.require_auth = true;
        config.valid_tokens = vec!["valid-token".to_string()];
        
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer valid-token".parse().unwrap());
        
        assert!(HttpTransport::validate_auth(&config, &headers).is_ok());
        
        headers.insert(AUTHORIZATION, "Bearer invalid-token".parse().unwrap());
        assert!(HttpTransport::validate_auth(&config, &headers).is_err());
        
        headers.remove(AUTHORIZATION);
        assert!(HttpTransport::validate_auth(&config, &headers).is_err());
    }

    #[tokio::test]
    async fn test_session_management() {
        let config = HttpConfig::default();
        let state = Arc::new(HttpState {
            handler: Arc::new(Box::new(mock_handler)),
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        });
        
        // Create session
        let session_id = HttpTransport::create_session(state.clone()).await;
        assert!(!session_id.is_empty());
        
        // Verify session exists
        {
            let sessions = state.sessions.read().await;
            assert!(sessions.contains_key(&session_id));
        }
        
        // Update activity
        HttpTransport::update_session_activity(state.clone(), &session_id).await;
        
        // Cleanup (should not remove recent session)
        HttpTransport::cleanup_sessions(state.clone()).await;
        {
            let sessions = state.sessions.read().await;
            assert!(sessions.contains_key(&session_id));
        }
    }
}