# Development Guide

## Overview

This guide covers development setup, contribution guidelines, and best practices for the Loxone MCP Rust server.

## Development Environment Setup

### Prerequisites

- Rust 1.70+ with cargo
- Git
- Optional: Docker for containerized development
- Optional: VS Code with rust-analyzer extension

### Initial Setup

```bash
# Clone the repository
git clone https://github.com/your-repo/loxone-mcp-rust.git
cd loxone-mcp-rust

# Install development dependencies
rustup component add rustfmt clippy
rustup target add wasm32-wasip2  # For WASM builds

# Setup pre-commit hooks
./scripts/setup-dev.sh

# Configure environment
cp .env.example .env
# Edit .env with your Loxone credentials
```

### Development Commands

```bash
# Build in debug mode (fast compilation)
cargo build

# Run with hot reload
cargo watch -x 'run --bin loxone-mcp-server -- http'

# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy -- -W clippy::all

# Check everything before committing
make check
```

## Project Structure

```
loxone-mcp-rust/
├── src/
│   ├── main.rs              # Binary entry point
│   ├── lib.rs               # Library root
│   ├── server/              # MCP server implementation
│   │   ├── mod.rs           # Server module root
│   │   ├── handlers.rs      # Request handlers
│   │   ├── resources.rs     # Resource definitions
│   │   └── rmcp_impl.rs     # RMCP implementation
│   ├── tools/               # MCP tools (17 commands)
│   │   ├── audio.rs         # Audio control tools
│   │   ├── climate.rs       # Climate control tools
│   │   ├── devices.rs       # Device control tools
│   │   └── ...              # Other tool categories
│   ├── client/              # Loxone client implementations
│   │   ├── http_client.rs   # HTTP client
│   │   └── ws_client.rs     # WebSocket client
│   ├── security/            # Security features
│   │   ├── key_store.rs     # API key management
│   │   ├── middleware.rs    # Security middleware
│   │   └── validation.rs    # Input validation
│   └── monitoring/          # Monitoring & dashboards
│       ├── dashboard.rs     # Web dashboard
│       └── metrics.rs       # Metrics collection
├── tests/                   # Integration tests
├── benches/                 # Performance benchmarks
└── examples/                # Usage examples
```

## Adding New Features

### Adding a New MCP Tool

1. Create a new file in `src/tools/` for your tool category:

```rust
// src/tools/my_new_tool.rs
use crate::client::LoxoneClient;
use rmcp::tools::{Tool, ToolBuilder, ToolError};
use serde_json::{json, Value};

pub fn create_my_tool() -> Tool {
    ToolBuilder::new("my_tool_name")
        .description("Description of what this tool does")
        .param("param1", "string", "Description of param1")
        .param("param2", "number", "Optional param2")
        .optional()
        .handler(|context, params| Box::pin(async move {
            let param1 = params["param1"].as_str()
                .ok_or_else(|| ToolError::InvalidParams("param1 required".into()))?;
            
            // Implementation here
            let client = context.get::<LoxoneClient>()?;
            let result = client.some_operation(param1).await?;
            
            Ok(json!({
                "success": true,
                "result": result
            }))
        }))
        .build()
}
```

2. Register the tool in `src/tools/mod.rs`:

```rust
pub fn register_all_tools(server: &mut McpServer) {
    // ... existing tools ...
    server.add_tool(my_new_tool::create_my_tool());
}
```

3. Add tests in `tests/tools_test.rs`:

```rust
#[tokio::test]
async fn test_my_new_tool() {
    let server = create_test_server().await;
    let result = server.call_tool("my_tool_name", json!({
        "param1": "test_value"
    })).await;
    
    assert!(result.is_ok());
    assert_eq!(result["success"], true);
}
```

### Adding a New Resource

1. Create resource definition in `src/server/resources.rs`:

```rust
pub fn create_my_resource() -> Resource {
    ResourceBuilder::new("loxone://my-resource/{id}")
        .description("My resource description")
        .mime_type("application/json")
        .handler(|context, uri| Box::pin(async move {
            let id = extract_id_from_uri(&uri)?;
            let client = context.get::<LoxoneClient>()?;
            
            let data = client.get_my_data(id).await?;
            
            Ok(ResourceContent {
                uri: uri.clone(),
                mime_type: "application/json".into(),
                text: Some(serde_json::to_string(&data)?),
                ..Default::default()
            })
        }))
        .build()
}
```

2. Register in `src/server/mod.rs`.

## Testing

### Unit Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_my_new_tool

# Run with output
cargo test -- --nocapture

# Run tests in parallel
cargo test -- --test-threads=4
```

### Integration Tests

```bash
# Run integration tests against mock server
cargo test --test integration_tests

# Run against real Loxone (requires setup)
LOXONE_TEST_REAL=1 cargo test --test real_device_tests
```

### Performance Testing

```bash
# Run benchmarks
cargo bench

# Profile with flamegraph
cargo flamegraph --bin loxone-mcp-server
```

## Code Style & Conventions

### Rust Style

- Use `rustfmt` for formatting (enforced by CI)
- Follow Rust naming conventions
- Document public APIs with rustdoc comments
- Use `clippy` for linting

### Error Handling

```rust
// Use the custom error type
use crate::error::{LoxoneError, Result};

// Return errors appropriately
pub async fn some_function() -> Result<String> {
    let data = fetch_data().await
        .map_err(|e| LoxoneError::connection(format!("Failed to fetch: {}", e)))?;
    
    Ok(data)
}

// Use error context
let result = operation()
    .context("Failed to perform operation")?;
```

### Async Best Practices

```rust
// Use tokio for async runtime
#[tokio::main]
async fn main() -> Result<()> {
    // ...
}

// Prefer concurrent operations
let (result1, result2) = tokio::join!(
    async_operation1(),
    async_operation2()
);

// Use timeouts for external calls
let result = tokio::time::timeout(
    Duration::from_secs(30),
    client.request()
).await??;
```

## Debugging

### Enable Debug Logging

```bash
# Set log level
export RUST_LOG=debug
export LOXONE_LOG_LEVEL=trace

# Run with backtrace
RUST_BACKTRACE=1 cargo run

# Use pretty env logger
cargo run --features pretty-log
```

### VS Code Launch Configuration

```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug HTTP Server",
            "cargo": {
                "args": [
                    "build",
                    "--bin=loxone-mcp-server",
                    "--package=loxone-mcp-rust"
                ],
                "filter": {
                    "name": "loxone-mcp-server",
                    "kind": "bin"
                }
            },
            "args": ["http", "--port", "3001"],
            "env": {
                "RUST_LOG": "debug",
                "LOXONE_HOST": "192.168.1.100"
            }
        }
    ]
}
```

## Performance Optimization

### Profiling

```bash
# CPU profiling
cargo build --release --features profiling
perf record --call-graph=dwarf target/release/loxone-mcp-server
perf report

# Memory profiling
valgrind --tool=massif target/release/loxone-mcp-server
ms_print massif.out.*
```

### Optimization Tips

1. **Connection Pooling**: Reuse HTTP connections
```rust
// Configured via environment
export LOXONE_CONNECTION_POOL_SIZE=50
```

2. **Caching**: Use the built-in cache for structure data
```rust
// Cache is automatic for structure queries
let structure = client.get_structure_cached().await?;
```

3. **Batch Operations**: Group multiple operations
```rust
// Use batch endpoints when available
let results = client.batch_control(device_commands).await?;
```

## Contributing

### Pull Request Process

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Add tests for new functionality
5. Run `make check` to ensure quality
6. Commit with descriptive message
7. Push and create pull request

### Commit Message Format

```
type(scope): subject

body

footer
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting
- `refactor`: Code restructuring
- `test`: Adding tests
- `chore`: Maintenance

Example:
```
feat(tools): add scene activation tool

Adds new tool for activating Loxone scenes by name or ID.
Includes support for room-specific scenes.

Closes #123
```

### Release Process

1. Update version in `Cargo.toml`
2. Update CHANGELOG.md
3. Create git tag: `git tag -a v1.2.3 -m "Release v1.2.3"`
4. Push tag: `git push origin v1.2.3`
5. GitHub Actions will build and release

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Documentation](https://tokio.rs)
- [MCP Specification](https://modelcontextprotocol.io)
- [Loxone API Docs](https://www.loxone.com/enen/kb/api/)

## Getting Help

- GitHub Issues for bugs/features
- GitHub Discussions for questions
- Discord community (if available)
- Stack Overflow with `loxone-mcp` tag