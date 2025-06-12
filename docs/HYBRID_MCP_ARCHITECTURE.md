# Hybrid MCP Ecosystem Architecture

## Overview

This document outlines our hybrid MCP (Model Context Protocol) ecosystem where n8n acts as both an MCP client (consuming our Loxone MCP server) and an MCP server (exposing automation workflows to AI agents).

## Architecture Components

### 1. **Loxone MCP Server (Rust)**
- **Role**: Specialized Loxone device control
- **Transport**: HTTP/SSE for n8n compatibility
- **Authentication**: Bearer token
- **Tools**: Device control, room management, climate, sensors

### 2. **n8n MCP Hub**
- **Role**: Central automation orchestrator
- **Functions**: 
  - MCP Server: Exposes complex workflows as simple tools
  - MCP Client: Consumes Loxone and other MCP servers
  - Workflow Engine: Handles complex automation logic

### 3. **AI Agents (Claude, etc.)**
- **Role**: High-level automation control
- **Access**: Via n8n MCP server endpoints
- **Capabilities**: Natural language → complex home automation

## Component Details

### Loxone MCP Server Configuration

```toml
# Enhanced Cargo.toml for HTTP transport
[dependencies]
rmcp = { version = "0.1", features = ["server", "transport-sse-server"] }
axum = "0.7"  # For HTTP server
tower = "0.4"
tower-http = { version = "0.5", features = ["cors"] }
```

**Server Implementation:**
```rust
use rmcp::transport::sse::SseServerTransport;
use axum::{Router, routing::get};
use tower_http::cors::CorsLayer;

impl LoxoneMcpServer {
    pub async fn run_http(&self, port: u16) -> Result<()> {
        let app = Router::new()
            .route("/mcp/sse", get(sse_handler))
            .route("/health", get(health_check))
            .layer(CorsLayer::permissive());
        
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}
```

### n8n MCP Server Configuration

**MCP Server Trigger Node Setup:**
```json
{
  "node": "MCP Server Trigger",
  "parameters": {
    "serverUrl": "https://your-domain.com/mcp/v1/",
    "authentication": "bearer",
    "bearerToken": "n8n-automation-token",
    "tools": [
      {
        "name": "home_security_mode",
        "description": "Set home security mode (home, away, sleep, party)",
        "parameters": {
          "mode": {"type": "string", "enum": ["home", "away", "sleep", "party"]},
          "rooms": {"type": "array", "items": {"type": "string"}}
        }
      },
      {
        "name": "energy_optimization",
        "description": "Optimize energy usage based on occupancy and weather",
        "parameters": {
          "optimization_level": {"type": "string", "enum": ["eco", "comfort", "performance"]}
        }
      }
    ]
  }
}
```

**MCP Client Tool Node Setup:**
```json
{
  "node": "MCP Client Tool",
  "parameters": {
    "sseEndpoint": "http://localhost:3001/mcp/sse",
    "authentication": "bearer",
    "bearerToken": "loxone-server-token",
    "toolsToInclude": "selected",
    "selectedTools": [
      "list_rooms",
      "control_device", 
      "set_room_temperature",
      "get_system_status"
    ]
  }
}
```

## Workflow Examples

### 1. **Home Security Mode Workflow**

```
[MCP Trigger: home_security_mode] 
  → [Switch: mode]
    → [away]: 
      → [MCP Client: control_device] (all lights off)
      → [MCP Client: set_room_temperature] (energy saving)
      → [HTTP: Notify security system]
    → [home]:
      → [MCP Client: control_device] (entrance lights on)
      → [MCP Client: set_room_temperature] (comfort mode)
    → [sleep]:
      → [MCP Client: control_device] (night lights only)
      → [HTTP: Enable sleep sensors]
```

### 2. **Energy Optimization Workflow**

```
[MCP Trigger: energy_optimization]
  → [HTTP: Get weather forecast]
  → [HTTP: Get energy prices]
  → [Function: Calculate optimal settings]
  → [MCP Client: control_device] (adjust based on calculation)
  → [MCP Client: set_room_temperature] (optimize HVAC)
  → [Return: Energy savings report]
```

### 3. **Morning Routine Workflow**

```
[Schedule Trigger: 7:00 AM]
  → [HTTP: Check calendar for meetings]
  → [If: Has early meeting]
    → [MCP Client: control_device] (bathroom lights on)
    → [MCP Client: set_room_temperature] (increase)
    → [HTTP: Start coffee machine]
    → [Wait: 15 minutes]
    → [MCP Client: control_device] (kitchen lights on)
```

## Authentication & Security

### Multi-layer Security
1. **Bearer Tokens**: Different tokens for each service layer
2. **CORS**: Restricted to known domains
3. **Rate Limiting**: Prevent abuse
4. **Audit Logging**: Track all MCP tool executions

```rust
// Loxone MCP Server Auth
#[derive(Clone)]
pub struct AuthConfig {
    pub n8n_token: String,
    pub claude_token: String,
    pub admin_token: String,
}

impl AuthMiddleware {
    fn validate_token(&self, token: &str, required_role: Role) -> bool {
        match required_role {
            Role::N8nClient => token == self.config.n8n_token,
            Role::AIAgent => token == self.config.claude_token,
            Role::Admin => token == self.config.admin_token,
        }
    }
}
```

## Deployment Architecture

### Development Setup
```bash
# Terminal 1: Loxone MCP Server
cd loxone-mcp-rust
cargo run -- --http-port 3001

# Terminal 2: n8n with MCP support
docker run -it --rm \
  -p 5678:5678 \
  -e N8N_COMMUNITY_PACKAGES_ALLOW_TOOL_USAGE=true \
  -v ~/.n8n:/home/node/.n8n \
  n8nio/n8n

# Terminal 3: Gateway proxy for Claude Desktop
npx @modelcontextprotocol/inspector \
  --url http://localhost:5678/mcp/v1/
```

### Production Setup
```yaml
# docker-compose.yml
version: '3.8'
services:
  loxone-mcp:
    build: ./loxone-mcp-rust
    ports:
      - "3001:3001"
    environment:
      - LOXONE_URL=${LOXONE_URL}
      - LOXONE_USERNAME=${LOXONE_USERNAME}
      - LOXONE_PASSWORD=${LOXONE_PASSWORD}
      - MCP_AUTH_TOKEN=${LOXONE_MCP_TOKEN}
    
  n8n:
    image: n8nio/n8n:latest
    ports:
      - "5678:5678"
    environment:
      - N8N_COMMUNITY_PACKAGES_ALLOW_TOOL_USAGE=true
      - N8N_MCP_LOXONE_URL=http://loxone-mcp:3001/mcp/sse
      - N8N_MCP_LOXONE_TOKEN=${LOXONE_MCP_TOKEN}
    volumes:
      - n8n_data:/home/node/.n8n
    depends_on:
      - loxone-mcp
    
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf
      - ./ssl:/etc/ssl
    depends_on:
      - n8n
```

## Benefits of This Architecture

### 1. **Separation of Concerns**
- **Loxone MCP Server**: Focused, reliable device control
- **n8n Hub**: Complex automation logic and integrations
- **AI Agents**: High-level reasoning and natural language

### 2. **Scalability**
- Add new MCP servers without changing n8n workflows
- Scale n8n horizontally for automation processing
- Independent deployment and updates

### 3. **Flexibility**
- Multiple AI agents can use the same automation tools
- Easy to add new automation patterns
- Rich ecosystem of n8n integrations

### 4. **Reliability**
- Direct Loxone control remains available if n8n fails
- Graceful degradation of automation features
- Clear error boundaries and logging

## Next Steps

1. **Phase 1**: Enhance Loxone MCP server with HTTP/SSE transport
2. **Phase 2**: Build core n8n automation workflows  
3. **Phase 3**: Deploy production environment with monitoring
4. **Phase 4**: Add more specialized MCP servers (security, energy, etc.)

## Monitoring & Debugging

### Key Metrics
- MCP tool execution times
- Authentication failures
- Workflow success rates
- Loxone device response times
- n8n workflow execution logs

### Debugging Tools
- n8n execution logs
- MCP server request/response logging
- Health check endpoints
- Performance metrics dashboard