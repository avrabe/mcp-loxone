# mcp-protocol

**Core types and validation for the Model Context Protocol in Rust**

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/avrabe/mcp-loxone/blob/main/LICENSE)

This crate provides the fundamental types and validation logic for building MCP (Model Context Protocol) servers and clients in Rust. It's been developed as part of a working Loxone home automation MCP server and handles the protocol details so you can focus on your application logic.

## What This Crate Provides

- **Core MCP types** - Request, Response, Tool, Resource, and other protocol structures
- **JSON-RPC 2.0 support** - Proper message formatting and parsing
- **Input validation** - Schema validation for tool parameters and responses
- **Error handling** - Standard MCP error types and conversion utilities
- **Protocol compliance** - Follows MCP specification version 2024-11-05

## Quick Example

```rust
use mcp_protocol::{Tool, Content, CallToolResult};
use serde_json::json;

// Define a tool with proper schema
let tool = Tool {
    name: "get_weather".to_string(),
    description: "Get current weather for a location".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "location": {
                "type": "string",
                "description": "City name or coordinates"
            }
        },
        "required": ["location"]
    }),
};

// Create a tool response
let result = CallToolResult {
    content: vec![Content::text("Current weather: 22°C, sunny".to_string())],
    is_error: Some(false),
};
```

## Real-World Usage

This crate is currently used in production by:
- **Loxone MCP Server** - A home automation server with 30+ tools that successfully integrates with MCP Inspector and Claude Desktop

## Current Status

**Stable core functionality** - The basic protocol types and validation work well. We've tested this with real MCP clients and it handles the protocol correctly.

**What works:**
- ✅ All core MCP types (Tool, Resource, Prompt, etc.)
- ✅ JSON-RPC 2.0 message handling
- ✅ Schema validation for tool inputs
- ✅ Error types that map to MCP specification
- ✅ Compatibility with MCP Inspector and Claude Desktop

**What's still developing:**
- 📝 API documentation could be more comprehensive
- 🧪 Test coverage could be broader
- 🔧 Some edge cases in validation might need refinement

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
mcp-protocol = "0.1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Basic Usage

### Defining Tools

```rust
use mcp_protocol::Tool;
use serde_json::json;

let tool = Tool {
    name: "calculate".to_string(),
    description: "Perform basic calculations".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "expression": {
                "type": "string",
                "description": "Mathematical expression to evaluate"
            }
        },
        "required": ["expression"]
    }),
};
```

### Handling Requests and Responses

```rust
use mcp_protocol::{CallToolRequestParam, CallToolResult, Content};

// Parse a tool call request
let request = CallToolRequestParam {
    name: "calculate".to_string(),
    arguments: Some(json!({"expression": "2 + 2"})),
};

// Create a response
let response = CallToolResult {
    content: vec![Content::text("4".to_string())],
    is_error: Some(false),
};
```

### Error Handling

```rust
use mcp_protocol::Error;

// Create standard MCP errors
let error = Error::invalid_params("Missing required parameter: location");
let internal_error = Error::internal_error("Database connection failed");
```

## Integration with Other Framework Crates

This crate works well with other parts of the MCP framework:

- **mcp-server** - Uses these types for the backend trait
- **mcp-transport** - Handles serialization of these types over HTTP/WebSocket
- **mcp-auth** - Validates requests using these error types

## Contributing

This crate is part of the larger MCP framework for Rust. The Loxone MCP server serves as our main testing ground for new features and improvements.

If you find issues or have suggestions:
1. Check if it works with the Loxone implementation first
2. Open an issue with a minimal reproduction case
3. Consider how changes might affect existing users

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

**Repository:** https://github.com/avrabe/mcp-loxone