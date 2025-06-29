# Architecture Overview

This document describes the architecture of the Loxone MCP Server implementation.

## System Architecture

```
┌─────────────────┐     ┌─────────────────┐
│ Claude Desktop  │     │   Web Client    │
│    (stdio)      │     │  (HTTP/SSE)     │
└────────┬────────┘     └────────┬────────┘
         │                       │
         └───────────┬───────────┘
                     │
        ┌────────────┴────────────┐
        │   MCP Server Core      │
        │  (PulseEngine MCP)     │
        └────────────┬────────────┘
                     │
        ┌────────────┴────────────┐
        │  Tool Adapters +       │
        │ Resource Handlers      │
        │ (17 Tools, 25+ Rsrc)  │
        └────────────┬────────────┘
                     │
        ┌────────────┴────────────┐
        │   Loxone Client        │
        │  (HTTP/WebSocket)      │
        └────────────┬────────────┘
                     │
        ┌────────────┴────────────┐
        │  Loxone Miniserver     │
        │   (Gen 1/Gen 2)        │
        └─────────────────────────┘
```

## Core Components

### 1. Transport Layer

The server supports multiple transport mechanisms:

- **stdio Transport**: For Claude Desktop integration
  - JSON-RPC over standard input/output
  - Single-user, desktop application model
  
- **HTTP/SSE Transport**: For web clients and integrations
  - RESTful HTTP endpoints
  - Server-Sent Events for streaming responses
  - CORS support for browser-based clients

### 2. MCP Framework (PulseEngine)

The server is built on the PulseEngine MCP framework, providing:

- Protocol compliance and validation
- Request/response handling
- Tool registration and discovery
- Resource management
- Error handling and logging

### 3. Tool Adapters

Tools are organized in a centralized adapter system (`src/tools/adapters.rs`):

- Single source of truth for all tool implementations
- Consistent error handling across tools
- Shared context and state management
- Type-safe parameter validation

### 4. Loxone Client

The client layer handles communication with the Miniserver:

- **HTTP Client** (`src/client/http_client.rs`)
  - Primary communication method
  - Basic authentication
  - Connection pooling
  - Retry logic with exponential backoff
  
- **WebSocket Client** (`src/client/websocket_client.rs`)
  - For future real-time updates
  - Encrypted communication support
  - Currently not fully integrated

### 5. State Management

Efficient state management through multiple layers:

- **Connection Pool**: Manages HTTP connections efficiently
- **Response Cache**: TTL-based caching of device states
- **Structure Cache**: Caches device structure (rooms, devices)
- **Background Refresh**: Automatic cache updates

## Data Flow

### Request Processing

1. **Transport receives request** (stdio or HTTP)
2. **Framework validates** against MCP protocol
3. **Tool adapter called** with validated parameters
4. **Client queries** Loxone Miniserver
5. **Response cached** if applicable
6. **Result returned** through transport

### Example: Light Control

```
User: "Turn on living room lights"
  │
  ├─> MCP Server receives tool call: control_lights_unified
  │     └─> Parameters: {scope: "room", target: "Living Room", command: "on"}
  │
  ├─> Tool Adapter validates parameters
  │     └─> Checks room exists, command is valid
  │
  ├─> Loxone Client builds request
  │     └─> GET /jdev/sps/io/{uuid}/on for each light
  │
  ├─> Connection pool provides connection
  │     └─> Reuses existing or creates new
  │
  ├─> Miniserver processes commands
  │     └─> Returns success/failure for each
  │
  └─> Response formatted and returned
        └─> Cache updated with new states
```

## Security Architecture

### Authentication Flow

```
┌────────┐     ┌────────────┐     ┌──────────────┐
│ Client │────>│ MCP Server │────>│  Miniserver  │
└────────┘     └────────────┘     └──────────────┘
    │               │                     │
    │ API Key       │ Basic Auth         │
    └───────────────┴─────────────────────┘
```

### Security Layers

1. **API Key Authentication**
   - Role-based access control (admin, operator, viewer)
   - Key storage using system keychain
   - Optional IP whitelisting

2. **Input Validation**
   - UUID format validation
   - Command sanitization
   - Parameter type checking

3. **Rate Limiting**
   - Per-role limits
   - Sliding window algorithm
   - Graceful degradation

4. **Audit Logging**
   - All actions logged with user context
   - Configurable retention
   - Export capabilities

## Performance Optimizations

### Connection Management

- **Pool Size**: Configurable (default: 10)
- **Keep-Alive**: Maintains persistent connections
- **Circuit Breaker**: Prevents cascade failures

### Caching Strategy

- **Device States**: 30-second TTL
- **Structure Data**: 5-minute TTL
- **Weather Data**: 1-minute TTL
- **Energy Data**: 10-second TTL

### Concurrency

- **Async/Await**: Non-blocking I/O throughout
- **Tokio Runtime**: Multi-threaded executor
- **Batch Operations**: Parallel device commands

## Configuration

### Environment Variables

```bash
# Loxone Connection
LOXONE_HOST=http://192.168.1.100
LOXONE_USER=username
LOXONE_PASS=password

# Server Configuration
LOXONE_LOG_LEVEL=info
LOXONE_CACHE_TTL=30
LOXONE_MAX_CONNECTIONS=10

# Security
LOXONE_API_KEY=your-api-key
LOXONE_RATE_LIMIT=100
```

### Configuration File

```toml
[server]
host = "127.0.0.1"
port = 3001
transport = "http"

[loxone]
host = "http://192.168.1.100"
verify_ssl = false
timeout = 30

[cache]
device_ttl = 30
structure_ttl = 300

[security]
enable_auth = true
rate_limit = 100
```

## Error Handling

### Error Types

1. **Connection Errors**: Network and timeout issues
2. **Authentication Errors**: Invalid credentials or permissions
3. **Validation Errors**: Invalid parameters or commands
4. **Device Errors**: Device not found or command failed
5. **Protocol Errors**: MCP protocol violations

### Error Propagation

```rust
LoxoneError
  ├─> Connection(String)
  ├─> Authentication(String)
  ├─> Validation(String)
  ├─> DeviceControl(String)
  └─> Protocol(String)
```

## Monitoring and Observability

### Logging

- **Structured Logging**: Using `tracing` crate
- **Log Levels**: TRACE, DEBUG, INFO, WARN, ERROR
- **Context**: Request IDs, user info, timing

### Metrics

- Request count and latency
- Cache hit/miss rates
- Connection pool statistics
- Error rates by type

### Health Checks

- `/health` - Basic server health
- `/ready` - Miniserver connectivity
- `/metrics` - Prometheus-compatible metrics

## Future Considerations

### Planned Enhancements

1. **WebSocket Integration**: Real-time device updates
2. **Event Subscriptions**: Push notifications for state changes
3. **Advanced Caching**: Predictive cache warming
4. **Distributed Deployment**: Multiple server instances

### WASM Support

Currently disabled due to tokio limitations. Future implementation would require:

- Alternative async runtime for WASM
- Platform-specific transport implementations
- Modified security layer for browser environment

## Development Guidelines

### Code Organization

```
src/
├── server/          # MCP protocol handling
├── tools/           # Tool implementations
│   └── adapters.rs  # All 17 tools + resource routing
├── client/          # Loxone communication
├── security/        # Auth and validation
├── performance/     # Monitoring and metrics
└── lib.rs           # Public API
```

### Testing Strategy

- **Unit Tests**: Individual component testing
- **Integration Tests**: End-to-end tool testing
- **Performance Tests**: Load and stress testing
- **Security Tests**: Penetration testing

### Contributing

See [contributing.md](../contributing.md) for development setup and guidelines.