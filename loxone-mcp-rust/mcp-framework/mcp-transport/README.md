# pulseengine-mcp-transport

**Transport layer implementations for MCP servers**

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/avrabe/mcp-loxone/blob/main/LICENSE)

This crate handles the network transport layer for MCP servers. It provides multiple transport options (stdio, HTTP, WebSocket) so your MCP server can work with different types of clients.

## What This Solves

Different MCP clients need different ways to connect:
- **Claude Desktop** uses stdio transport
- **Web applications** use HTTP
- **Real-time applications** use WebSocket
- **MCP Inspector** can use HTTP with Server-Sent Events

This crate handles all of these so you don't have to worry about transport details.

## Real-World Testing

This transport layer has been thoroughly tested with:
- ✅ **MCP Inspector** - Both legacy SSE and modern HTTP streaming
- ✅ **Claude Desktop** - stdio transport integration
- ✅ **HTTP clients** - RESTful access from web apps and n8n
- ✅ **WebSocket clients** - Real-time bidirectional communication

The HTTP transport was specifically debugged and fixed to work with MCP Inspector's content negotiation requirements.

## Quick Start

```toml
[dependencies]
pulseengine-mcp-transport = "0.2.0"
pulseengine-mcp-protocol = "0.2.0"
tokio = { version = "1.0", features = ["full"] }
```

## Usage Examples

### Basic HTTP Transport

```rust
use pulseengine_mcp_transport::{TransportConfig, create_transport};
use pulseengine_mcp_protocol::{Request, Response};

// Create HTTP transport on port 3001
let config = TransportConfig::Http { port: 3001 };
let mut transport = create_transport(config)?;

// Define your request handler
let handler = Box::new(|request: Request| {
    Box::pin(async move {
        // Process the MCP request and return response
        Response::success(serde_json::json!({"result": "handled"}))
    })
});

// Start the transport
transport.start(handler).await?;
```

### Stdio Transport (for Claude Desktop)

```rust
use mcp_transport::TransportConfig;

// Simple stdio configuration
let config = TransportConfig::Stdio;
let mut transport = create_transport(config)?;
// ... same handler setup as above
```

### WebSocket Transport

```rust
use mcp_transport::TransportConfig;

// WebSocket on port 3001
let config = TransportConfig::WebSocket { port: 3001 };
let mut transport = create_transport(config)?;
// ... handler setup
```

## Current Status

**Solid foundation with known limitations.** The core transport functionality works well in production, but there are areas for improvement.

**What works reliably:**
- ✅ HTTP transport with proper MCP Inspector compatibility
- ✅ stdio transport for Claude Desktop integration
- ✅ WebSocket transport for real-time applications
- ✅ Content negotiation (JSON vs Server-Sent Events)
- ✅ Session management and CORS handling

**Areas that need work:**
- 📝 Better examples for each transport type
- 🧪 More comprehensive error handling in edge cases
- 🔧 WebSocket transport could use more testing
- 📊 Better connection lifecycle management

## Transport Details

### HTTP Transport

The HTTP transport supports both traditional request/response and Server-Sent Events:

```rust
// Handles both:
// POST /mcp with Content-Type: application/json
// POST /mcp with Accept: text/event-stream
```

**MCP Inspector compatibility:** We specifically fixed content negotiation issues to work with MCP Inspector's mixed Accept headers (`application/json, text/event-stream`).

### stdio Transport

Clean integration with Claude Desktop and other stdio-based MCP clients:

```rust
// Reads JSON-RPC from stdin, writes responses to stdout
// Handles proper buffering and line-based communication
```

### WebSocket Transport

Real-time bidirectional communication:

```rust
// Full-duplex communication over WebSocket
// Supports both text and binary frames
// Handles connection lifecycle properly
```

## Integration with MCP Framework

This crate integrates cleanly with other framework components:

```rust
use mcp_server::{McpServer, ServerConfig};
use mcp_transport::TransportConfig;

let config = ServerConfig {
    transport_config: TransportConfig::Http { port: 3001 },
    // ... other config
};

// The server handles transport setup automatically
let server = McpServer::new(backend, config).await?;
```

## Examples

The crate includes several working examples:

- **test_http_sse.rs** - HTTP with Server-Sent Events
- **test_mcp_inspector.rs** - MCP Inspector integration testing
- **test_streamable_http.rs** - Modern HTTP streaming transport
- **complete_mcp_server.rs** - Full server example

Run an example:

```bash
cargo run --example test_http_sse
```

## Debugging and Testing

If you're having connectivity issues:

1. **Check the examples** - They show working configurations
2. **Test with curl** - Verify basic HTTP transport functionality
3. **Use MCP Inspector** - Good for debugging protocol issues
4. **Check logs** - Enable debug logging to see transport details

```bash
RUST_LOG=debug cargo run --example test_mcp_inspector
```

## Contributing

Transport layer improvements often come from real-world integration issues. The most helpful contributions:

1. **Client compatibility** - Testing with new MCP clients
2. **Error handling** - Better handling of network edge cases
3. **Performance** - Connection pooling, request batching
4. **Examples** - More real-world usage patterns

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

**Repository:** https://github.com/avrabe/mcp-loxone

**Note:** This crate is part of a larger MCP framework that will be published as a separate repository.