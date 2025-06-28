# Hello World MCP Server Example

A minimal example showing how to build an MCP server using the framework. This demonstrates the basic structure without overwhelming complexity.

## What This Example Shows

- **Basic backend implementation** - How to implement the `McpBackend` trait
- **Simple tools** - Two tools that demonstrate parameter handling and state management
- **Error handling** - How to define and handle backend-specific errors
- **Logging integration** - Basic structured logging setup

## Running the Example

```bash
# From the hello-world directory
cargo run --bin hello-world-server

# Or from the framework root
cargo run --example hello-world
```

The server will start in stdio mode, which means it communicates via standard input/output. This is perfect for testing with MCP clients.

## Testing with MCP Inspector

```bash
# Start the server
cargo run --bin hello-world-server

# In another terminal, test with MCP Inspector
npx @modelcontextprotocol/inspector stdio -- cargo run --bin hello-world-server
```

## Available Tools

### `say_hello`
Says hello to someone with an optional custom greeting.

**Parameters:**
- `name` (required): The name to greet
- `greeting` (optional): Custom greeting, defaults to "Hello"

**Example:**
```json
{
  "name": "say_hello",
  "arguments": {
    "name": "World",
    "greeting": "Hi"
  }
}
```

**Response:** "Hi, World! 👋"

### `count_greetings`
Returns the total number of greetings sent since the server started.

**Parameters:** None

**Example response:** "Total greetings sent: 5"

## What's Not Included

This is intentionally a minimal example. It doesn't include:
- Authentication (see mcp-auth crate)
- Security validation (see mcp-security crate)
- Advanced monitoring (see mcp-monitoring crate)
- Resources or prompts (focus is on tools)
- Complex business logic (keeps the example simple)

For a more comprehensive example, check out the Loxone MCP server implementation, which uses all framework features to manage home automation with 30+ tools.

## Key Patterns Demonstrated

### Backend Structure
```rust
#[derive(Clone)]
pub struct HelloWorldBackend {
    // Your backend state here
    greeting_count: Arc<AtomicU64>,
}

#[async_trait]
impl McpBackend for HelloWorldBackend {
    type Error = HelloWorldError;
    type Config = HelloWorldConfig;
    
    // Implementation methods...
}
```

### Error Handling
```rust
#[derive(Debug, Error)]
pub enum HelloWorldError {
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    // ... other error types
}

impl From<HelloWorldError> for mcp_protocol::Error {
    fn from(err: HelloWorldError) -> Self {
        // Convert to MCP protocol errors
    }
}
```

### Tool Implementation
```rust
async fn call_tool(&self, request: CallToolRequestParam) -> Result<CallToolResult, Self::Error> {
    match request.name.as_str() {
        "say_hello" => {
            // Extract parameters
            // Do the work
            // Return result
        }
        _ => Err(HelloWorldError::InvalidParameter("Unknown tool".to_string()))
    }
}
```

This pattern scales well - the Loxone implementation uses the same structure but with 30+ tools and more complex business logic.