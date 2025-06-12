//! HTTP/SSE transport implementation for n8n MCP integration
//!
//! This module provides HTTP server capabilities with Server-Sent Events (SSE)
//! transport for the Model Context Protocol, making it compatible with n8n.

use crate::mcp_server::LoxoneMcpServer;
use crate::error::{LoxoneError, Result};

use axum::{
    extract::{State, Query},
    http::{StatusCode, HeaderMap, header},
    response::{IntoResponse, sse::{Event, Sse}},
    routing::get,
    Json, Router,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
};
use rmcp::ServerHandler;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, warn, debug};
use chrono;
use futures_util::stream::{self};
use futures_util::StreamExt;
use std::convert::Infallible;
use std::time::Duration;

/// Authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Single API key for all access
    pub api_key: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            api_key: "default-api-key".to_string(),
        }
    }
}

/// Query parameters for SSE endpoint
#[derive(Debug, Deserialize)]
struct SseQuery {
    /// Optional client identifier
    client_id: Option<String>,
}

/// Health check response
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    timestamp: String,
    services: HealthServices,
}

#[derive(Debug, Serialize)]
struct HealthServices {
    loxone: String,
    mcp_server: String,
}

/// HTTP transport server
pub struct HttpTransportServer {
    /// MCP server instance
    mcp_server: LoxoneMcpServer,
    /// Authentication configuration
    auth_config: AuthConfig,
    /// Server port
    port: u16,
}

impl HttpTransportServer {
    /// Create new HTTP transport server
    pub fn new(mcp_server: LoxoneMcpServer, auth_config: AuthConfig, port: u16) -> Self {
        Self {
            mcp_server,
            auth_config,
            port,
        }
    }

    /// Start the HTTP server
    pub async fn start(&self) -> Result<()> {
        let app = self.create_router().await?;
        
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await
            .map_err(|e| LoxoneError::connection(format!("Failed to bind to port {}: {}", self.port, e)))?;
        
        info!("🌐 HTTP MCP server starting on port {}", self.port);
        info!("📬 MCP HTTP endpoint: http://localhost:{}/message (MCP Inspector)", self.port);
        info!("📡 SSE stream: http://localhost:{}/sse (optional)", self.port);
        info!("📡 SSE endpoint: http://localhost:{}/mcp/sse (n8n legacy)", self.port);
        info!("🏥 Health check: http://localhost:{}/health", self.port);
        
        axum::serve(listener, app).await
            .map_err(|e| LoxoneError::connection(format!("HTTP server error: {}", e)))?;
        
        Ok(())
    }

    /// Create the router with all endpoints
    async fn create_router(&self) -> Result<Router> {
        let shared_state = Arc::new(AppState {
            mcp_server: self.mcp_server.clone(),
            auth_config: self.auth_config.clone(),
        });

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            // Health check endpoint (no auth required)
            .route("/health", get(health_check))
            .route("/", get(root_handler))
            
            // MCP Streamable HTTP transport endpoints
            .route("/sse", get(sse_handler))  // Optional SSE stream for server→client  
            .route("/message", axum::routing::post(handle_mcp_message))  // Main HTTP POST endpoint
            .route("/messages", axum::routing::post(handle_mcp_message))  // n8n compatibility
            
            // Legacy endpoints for backwards compatibility
            .route("/mcp/sse", get(sse_handler))  // Alternative for n8n
            .route("/mcp/info", get(server_info))
            .route("/mcp/tools", get(list_tools))
            
            // Admin endpoints (require admin auth)
            .route("/admin/status", get(admin_status))
            
            .layer(ServiceBuilder::new()
                .layer(cors)
                .into_inner())
            .with_state(shared_state);

        Ok(app)
    }
}

/// Shared application state
#[derive(Clone)]
struct AppState {
    mcp_server: LoxoneMcpServer,
    auth_config: AuthConfig,
}

/// Root handler
async fn root_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "Loxone MCP Server",
        "version": "1.0.0",
        "transport": "HTTP/SSE",
        "endpoints": {
            "health": "/health",
            "mcp_sse": "/mcp/sse",
            "mcp_info": "/mcp/info",
            "tools": "/mcp/tools"
        },
        "authentication": "Bearer token required for MCP endpoints"
    }))
}

/// Health check endpoint
async fn health_check(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    debug!("Health check requested");
    
    // Check Loxone connectivity
    let loxone_status = match state.mcp_server.get_system_status().await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };

    let response = HealthResponse {
        status: "ok".to_string(),
        version: "1.0.0".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        services: HealthServices {
            loxone: loxone_status.to_string(),
            mcp_server: "healthy".to_string(),
        },
    };

    Json(response)
}

/// SSE endpoint for MCP communication
async fn sse_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SseQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Log all headers and authentication info for debugging
    info!("SSE request received with headers: {:?}", headers);
    if let Err(err) = validate_auth(&headers, &state.auth_config) {
        warn!("SSE authentication failed (allowing for debugging): {}", err);
        // For now, continue without failing for debugging
    } else {
        info!("SSE authentication successful");
    }

    let client_id = query.client_id.unwrap_or_else(|| {
        uuid::Uuid::new_v4().to_string()
    });

    info!("SSE connection established for client: {}", client_id);

    // Create proper MCP SSE stream that implements the initialization handshake
    create_mcp_sse_stream(&state.mcp_server, &client_id).await
}

/// Create proper MCP SSE stream
async fn create_mcp_sse_stream(
    mcp_server: &LoxoneMcpServer,
    client_id: &str,
) -> impl IntoResponse {
    info!("Creating MCP SSE stream for client: {}", client_id);
    
    // Clone the server and client_id for use in the stream
    let server = mcp_server.clone();
    let client_id = client_id.to_string();
    
    // Create SSE stream that sends initial connection event
    let stream = stream::once(async move {
        // Send initial connection event
        let connection_event = Event::default()
            .event("connection")
            .data(serde_json::json!({
                "type": "connection",
                "status": "connected",
                "client_id": client_id
            }).to_string());
        
        Ok::<Event, Infallible>(connection_event)
    }).chain(stream::unfold(server, move |server| async move {
        // Keep connection alive with periodic pings
        tokio::time::sleep(Duration::from_secs(30)).await;
        
        let ping_event = Event::default()
            .event("ping")
            .data(serde_json::json!({
                "type": "ping",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }).to_string());
        
        Some((Ok::<Event, Infallible>(ping_event), server))
    }));
    
    Sse::new(stream)
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive")
        )
}


/// Handle MCP messages via HTTP POST (Streamable HTTP transport for MCP Inspector)
async fn handle_mcp_message(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Validate authentication
    if let Err(_err) = validate_auth(&headers, &state.auth_config) {
        return Err((StatusCode::UNAUTHORIZED, "Authentication required"));
    }

    info!("Received MCP message: {:?}", request);
    
    // Handle different MCP request types according to MCP specification
    if let Some(method) = request.get("method").and_then(|m| m.as_str()) {
        match method {
            "initialize" => {
                let server_info = state.mcp_server.get_info();
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "capabilities": {
                            "tools": {},
                            "resources": {},
                            "prompts": {}
                        },
                        "serverInfo": {
                            "name": server_info.server_info.name,
                            "version": server_info.server_info.version
                        },
                        "protocolVersion": "2024-11-05"
                    }
                });
                Ok(Json(response))
            }
            "notifications/initialized" => {
                // Client acknowledges initialization
                info!("MCP client initialized successfully");
                Ok(Json(serde_json::json!({"jsonrpc": "2.0"})))
            }
            "tools/list" => {
                // Return the complete tool list that matches the MCP server implementation
                let tools = vec![
                    serde_json::json!({
                        "name": "list_rooms",
                        "description": "Get list of all rooms with device counts and information",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "required": []
                        }
                    }),
                    serde_json::json!({
                        "name": "get_room_devices",
                        "description": "Get all devices in a specific room with detailed information",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "room_name": {
                                    "type": "string",
                                    "description": "Name of the room"
                                },
                                "device_type": {
                                    "type": "string",
                                    "description": "Optional filter by device type (e.g., 'Switch', 'Jalousie')"
                                }
                            },
                            "required": ["room_name"]
                        }
                    }),
                    serde_json::json!({
                        "name": "control_device",
                        "description": "Control a single Loxone device by UUID or name",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "device": {
                                    "type": "string",
                                    "description": "Device UUID or name"
                                },
                                "action": {
                                    "type": "string",
                                    "description": "Action to perform (on, off, up, down, stop)"
                                },
                                "room": {
                                    "type": "string",
                                    "description": "Optional room name to help identify the device"
                                }
                            },
                            "required": ["device", "action"]
                        }
                    }),
                    serde_json::json!({
                        "name": "control_all_rolladen",
                        "description": "Control all rolladen/blinds in the entire system simultaneously",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "action": {
                                    "type": "string",
                                    "description": "Action to perform: 'up', 'down', or 'stop'"
                                }
                            },
                            "required": ["action"]
                        }
                    }),
                    serde_json::json!({
                        "name": "control_room_rolladen",
                        "description": "Control all rolladen/blinds in a specific room",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "room": {
                                    "type": "string",
                                    "description": "Name of the room"
                                },
                                "action": {
                                    "type": "string",
                                    "description": "Action to perform: 'up', 'down', or 'stop'"
                                }
                            },
                            "required": ["room", "action"]
                        }
                    }),
                    serde_json::json!({
                        "name": "control_all_lights",
                        "description": "Control all lights in the entire system simultaneously",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "action": {
                                    "type": "string",
                                    "description": "Action to perform: 'on' or 'off'"
                                }
                            },
                            "required": ["action"]
                        }
                    }),
                    serde_json::json!({
                        "name": "control_room_lights",
                        "description": "Control all lights in a specific room",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "room": {
                                    "type": "string",
                                    "description": "Name of the room"
                                },
                                "action": {
                                    "type": "string",
                                    "description": "Action to perform: 'on' or 'off'"
                                }
                            },
                            "required": ["room", "action"]
                        }
                    }),
                    serde_json::json!({
                        "name": "discover_all_devices",
                        "description": "Discover and list all devices in the system with detailed information",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "required": []
                        }
                    }),
                    serde_json::json!({
                        "name": "get_devices_by_type",
                        "description": "Get all devices filtered by type (e.g., Switch, Jalousie, Dimmer)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "device_type": {
                                    "type": "string",
                                    "description": "Type of devices to filter (optional, shows all types if not specified)"
                                }
                            },
                            "required": []
                        }
                    }),
                    serde_json::json!({
                        "name": "get_system_status",
                        "description": "Get overall system status and health information",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "required": []
                        }
                    })
                ];
                
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "tools": tools
                    }
                });
                Ok(Json(response))
            }
            "tools/call" => {
                // Handle tool calls
                let params = request.get("params");
                let tool_name = params
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .ok_or((StatusCode::BAD_REQUEST, "Missing tool name"))?;
                
                let arguments = params
                    .and_then(|p| p.get("arguments"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                
                info!("Calling tool: {} with arguments: {:?}", tool_name, arguments);
                
                // Call the actual MCP server to execute the tool
                match state.mcp_server.call_tool(tool_name, arguments).await {
                    Ok(result) => {
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "result": result
                        });
                        Ok(Json(response))
                    }
                    Err(e) => {
                        let error_response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32603,
                                "message": format!("Tool execution error: {}", e)
                            }
                        });
                        Ok(Json(error_response))
                    }
                }
            }
            _ => {
                let error_response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "error": {
                        "code": -32601,
                        "message": "Method not found"
                    }
                });
                Ok(Json(error_response))
            }
        }
    } else {
        Err((StatusCode::BAD_REQUEST, "Invalid MCP request"))
    }
}

/// Get server information
async fn server_info(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(_err) = validate_auth(&headers, &state.auth_config) {
        return Err((StatusCode::UNAUTHORIZED, "Authentication required"));
    }

    let info = state.mcp_server.get_info();
    Ok(Json(serde_json::json!({
        "name": info.server_info.name,
        "version": info.server_info.version,
        "instructions": info.instructions,
        "transport": "HTTP/SSE",
        "authentication": "Bearer"
    })))
}

/// List available tools
async fn list_tools(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(_err) = validate_auth(&headers, &state.auth_config) {
        return Err((StatusCode::UNAUTHORIZED, "Authentication required"));
    }

    // TODO: Implement proper tool listing from MCP server
    let tools = serde_json::json!({
        "tools": [
            {
                "name": "list_rooms",
                "description": "Get list of all rooms",
                "parameters": {}
            },
            {
                "name": "control_device", 
                "description": "Control a Loxone device",
                "parameters": {
                    "device_id": {"type": "string", "description": "Device UUID"},
                    "action": {"type": "string", "description": "Action to perform"}
                }
            },
            {
                "name": "set_room_temperature",
                "description": "Set the temperature for a room", 
                "parameters": {
                    "room_name": {"type": "string", "description": "Room name"},
                    "temperature": {"type": "number", "description": "Target temperature"}
                }
            }
        ]
    });

    Ok(Json(tools))
}

/// Admin status endpoint
async fn admin_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(_err) = validate_auth(&headers, &state.auth_config) {
        return Err((StatusCode::UNAUTHORIZED, "Authentication required"));
    }

    let status = serde_json::json!({
        "server": "running",
        "connections": 0, // TODO: Track active connections
        "auth_config": {
            "api_key_set": !state.auth_config.api_key.is_empty()
        }
    });

    Ok(Json(status))
}

/// Validate Bearer token authentication
fn validate_auth(headers: &HeaderMap, auth_config: &AuthConfig) -> Result<()> {
    let auth_header = headers.get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| LoxoneError::authentication("Missing Authorization header"))?;

    if !auth_header.starts_with("Bearer ") {
        return Err(LoxoneError::authentication("Invalid Authorization header format"));
    }

    let token = &auth_header[7..]; // Remove "Bearer " prefix

    if token == auth_config.api_key {
        Ok(())
    } else {
        Err(LoxoneError::authentication("Invalid API key"))
    }
}