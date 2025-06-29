# pulseengine-mcp-server

**Framework for building MCP servers with pluggable backends**

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/avrabe/mcp-loxone/blob/main/LICENSE)

This crate provides the server infrastructure for building Model Context Protocol servers in Rust. You implement a backend trait, and this handles the protocol details, transport layer, and infrastructure concerns.

## What This Crate Does

The main idea is simple: you implement the `McpBackend` trait for your domain (databases, APIs, file systems, etc.), and this crate handles all the MCP protocol work.

**Infrastructure handled for you:**
- Protocol compliance and message routing
- Multiple transport support (stdio, HTTP, WebSocket)
- Authentication and security middleware integration
- Request validation and error handling
- Monitoring and health checks

**You focus on:**
- Your domain logic (what tools/resources you provide)
- How to execute operations in your system
- Your specific business rules and validation

## Real-World Example

This framework currently powers the **Loxone MCP Server**, which implements 30+ tools for home automation. It successfully works with MCP Inspector, Claude Desktop, and HTTP clients like n8n.

## Quick Start

```toml
[dependencies]
pulseengine-mcp-server = "0.2.0"
pulseengine-mcp-protocol = "0.2.0"
pulseengine-mcp-transport = "0.2.0"
tokio = { version = "1.0", features = ["full"] }
async-trait = "0.1"
```

```rust
use pulseengine_mcp_server::{McpServer, McpBackend, ServerConfig};
use pulseengine_mcp_protocol::*;
use pulseengine_mcp_transport::TransportConfig;
use async_trait::async_trait;
use std::collections::HashMap;

// Your backend implementation
#[derive(Clone)]
struct MyBackend {
    data: HashMap<String, String>,
}

#[async_trait]
impl McpBackend for MyBackend {
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Config = ();
    
    async fn initialize(_config: Self::Config) -> Result<Self, Self::Error> {
        Ok(MyBackend {
            data: HashMap::new(),
        })
    }
    
    fn get_server_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability { list_changed: Some(false) }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "My MCP Server".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some("A simple key-value store server".to_string()),
        }
    }
    
    async fn list_tools(&self, _request: PaginatedRequestParam) -> Result<ListToolsResult, Self::Error> {
        Ok(ListToolsResult {
            tools: vec![
                Tool {
                    name: "store_value".to_string(),
                    description: "Store a key-value pair".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "key": {"type": "string"},
                            "value": {"type": "string"}
                        },
                        "required": ["key", "value"]
                    }),
                }
            ],
            next_cursor: String::new(),
        })
    }
    
    async fn call_tool(&self, request: CallToolRequestParam) -> Result<CallToolResult, Self::Error> {
        match request.name.as_str() {
            "store_value" => {
                // Your business logic here
                Ok(CallToolResult {
                    content: vec![Content::text("Value stored successfully".to_string())],
                    is_error: Some(false),
                })
            }
            _ => Err("Unknown tool".into()),
        }
    }
    
    // Implement other required methods (can return empty/default for unused features)
    async fn list_resources(&self, _: PaginatedRequestParam) -> Result<ListResourcesResult, Self::Error> {
        Ok(ListResourcesResult { resources: vec![], next_cursor: String::new() })
    }
    
    async fn read_resource(&self, _: ReadResourceRequestParam) -> Result<ReadResourceResult, Self::Error> {
        Err("No resources available".into())
    }
    
    async fn list_prompts(&self, _: PaginatedRequestParam) -> Result<ListPromptsResult, Self::Error> {
        Ok(ListPromptsResult { prompts: vec![], next_cursor: String::new() })
    }
    
    async fn get_prompt(&self, _: GetPromptRequestParam) -> Result<GetPromptResult, Self::Error> {
        Err("No prompts available".into())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = MyBackend::initialize(()).await?;
    
    let config = ServerConfig {
        server_info: backend.get_server_info(),
        transport_config: TransportConfig::Stdio, // or Http { port: 3001 }
        ..Default::default()
    };
    
    let mut server = McpServer::new(backend, config).await?;
    server.run().await?;
    
    Ok(())
}
```

## Current Status

**Works well for basic to complex use cases.** The Loxone implementation proves this can handle real-world complexity with multiple tools, resources, and concurrent operations.

**What's solid:**
- ✅ Backend trait is stable and well-tested
- ✅ Transport integration works (stdio, HTTP, WebSocket)
- ✅ Authentication middleware integrates cleanly
- ✅ Error handling and protocol compliance
- ✅ Async/await throughout with good performance

**Areas for improvement:**
- 📝 More examples for different use cases
- 🧪 Better testing utilities for backend implementations
- 🔧 Some advanced features could be more polished
- 📊 More comprehensive monitoring hooks

## Features

### Multiple Transports

```rust
// For Claude Desktop integration
TransportConfig::Stdio

// For HTTP clients (n8n, web apps)
TransportConfig::Http { port: 3001 }

// For real-time applications
TransportConfig::WebSocket { port: 3001 }
```

### Built-in Security

When enabled, you get authentication, rate limiting, and input validation automatically:

```rust
let mut config = ServerConfig::default();
config.auth_config.enabled = true; // Requires API keys
config.security_config.rate_limit = Some(100); // Requests per minute
```

### Monitoring Integration

The framework provides hooks for monitoring and observability:

```rust
// Health checks, metrics, and request tracing work automatically
// Custom monitoring can be added through the backend trait
```

## Comparing to the Loxone Implementation

The **Loxone MCP Server** is our main reference implementation. It shows how to:
- Handle 30+ tools with complex business logic
- Integrate with external systems (HTTP APIs)
- Manage connection pooling and caching
- Structure tools by domain (lighting, climate, security)
- Handle real-world error conditions

If you're building something similar in complexity, the Loxone code is a good reference for patterns and organization.

## Contributing

This crate is actively used and maintained as part of the Loxone MCP server. Improvements often come from real-world usage patterns we discover.

Most useful contributions:
1. **Better examples** - Show patterns for different domains
2. **Testing utilities** - Make it easier to test backend implementations  
3. **Documentation** - Especially for complex integration scenarios
4. **Performance improvements** - Based on real usage patterns

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

**Repository:** https://github.com/avrabe/mcp-loxone

**Note:** This crate is part of a larger MCP framework that will be published as a separate repository.