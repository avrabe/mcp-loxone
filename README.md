<!--
SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
-->

<div align="center">
  <img src="mcp-loxone-gen1.png" alt="Loxone MCP Server" width="250"/>
  
  # Loxone MCP Server

  High-performance Rust implementation of Model Context Protocol (MCP) server for Loxone Generation 1 home automation systems. Enables AI assistants to control lights, blinds, sensors, and weather data through natural language commands.

  **[📖 Landing Page](https://avrabe.github.io/mcp-loxone-gen1/)** | **[⚡ Quick Start](#quick-start)** | **[📋 Documentation](loxone-mcp-rust/README.md)** | **[🐍 Python Legacy](archive/python-legacy/)**
</div>

## 🦀 Rust Implementation (Recommended)

This project has been **completely rewritten in Rust** for superior performance, reliability, and deployment flexibility.

### Key Features

- **🚀 10-100x Performance**: Rust's zero-cost abstractions and async runtime
- **🔧 23+ MCP Tools**: Comprehensive home automation control
- **🌐 Multi-Platform**: WASM, Docker, native binaries, HTTP/SSE
- **🔐 Advanced Security**: Infisical integration, consent management, audit trails  
- **🎯 Batch Operations**: Parallel device control with automatic optimization
- **📊 Health Monitoring**: Real-time metrics, connection pooling, error tracking
- **🔄 Workflow Engine**: n8n integration with visual automation builder

### Quick Start

```bash
git clone https://github.com/avrabe/mcp-loxone-gen1.git
cd mcp-loxone-gen1/loxone-mcp-rust
cargo build --release
./target/release/loxone-mcp-rust --help
```

**Continue reading**: [loxone-mcp-rust/README.md](loxone-mcp-rust/README.md)

## Prerequisites

- Rust 1.75+ ([install via rustup](https://rustup.rs/))
- Loxone Miniserver Generation 1
- Optional: Docker for containerized deployment

## Deployment Options

### 1. Native Binary
```bash
cd loxone-mcp-rust
cargo install --path .
loxone-mcp-rust --host 192.168.1.100 --username admin
```

### 2. Docker
```bash
cd loxone-mcp-rust
docker build -t loxone-mcp .
docker run -p 8080:8080 loxone-mcp
```

### 3. WASM (Browser)
```bash
cd loxone-mcp-rust
wasm-pack build --target web
# Serve with any HTTP server
```

### 4. Claude Desktop
```json
// ~/Library/Application Support/Claude/claude_desktop_config.json
{
  "mcpServers": {
    "loxone": {
      "command": "loxone-mcp-rust",
      "args": ["--mcp-mode"]
    }
  }
}
```

## Migration from Python

The **Python implementation has been archived** to `archive/python-legacy/`. The new Rust version provides:

- ⚡ **10-100x faster execution** - Zero-cost async operations
- 🛡️ **Enhanced security** - Consent management and audit logging  
- 🔧 **More tools** - 23+ vs ~10 in Python version
- 🌐 **Better deployment** - WASM, native binaries, containerization
- 📊 **Monitoring** - Health checks, metrics, connection pooling

### Migration Guide

1. **Archive your Python config** (if needed):
   ```bash
   cp ~/.config/loxone-mcp/* ~/backup/
   ```

2. **Switch to Rust version**:
   ```bash
   cd loxone-mcp-rust
   cargo build --release
   ./target/release/loxone-mcp-rust setup
   ```

3. **Update Claude Desktop config** to use the new binary path

## Advanced Features

### 🎯 Batch Operations
- Parallel device control with automatic optimization
- Rate limiting and connection pooling
- Graceful error handling and partial success reporting

### 🔐 Consent Management
- Interactive approval for sensitive operations
- Configurable security policies
- Comprehensive audit trails

### 📊 Health Monitoring
- Real-time connection and performance metrics
- Automatic retry logic with exponential backoff
- Health check endpoints for monitoring systems

### 🔄 Workflow Integration
- n8n visual automation builder
- Event-driven architecture
- Custom workflow templates

## Security Considerations

- **Multi-backend credential storage**: Infisical → Keychain → Environment variables
- **Consent management**: Interactive approval for sensitive operations
- **Audit trails**: Comprehensive logging of all security-related actions
- **Connection security**: TLS support with certificate validation
- **Rate limiting**: Protection against API abuse and DoS attacks

## Troubleshooting

### Connection Issues
- Verify your Loxone Miniserver is accessible: `loxone-mcp-rust health-check`
- Check network connectivity and firewall settings
- Ensure your user has sufficient permissions in Loxone Config

### Authentication Errors
- Re-run setup: `loxone-mcp-rust setup`
- Verify credentials in Loxone Config software
- Check credential backend status: `loxone-mcp-rust verify-credentials`

### Performance Issues
- Monitor connection pool: `loxone-mcp-rust stats`
- Check resource usage: `loxone-mcp-rust health-check --detailed`
- Review error logs for patterns

## Development

### Building from Source
```bash
cd loxone-mcp-rust
cargo build --release
cargo test
```

### Contributing
1. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines
2. Run tests: `cargo test`
3. Check linting: `cargo clippy`
4. Format code: `cargo fmt`

### Adding New Features
1. Add tool implementation in `src/tools/`
2. Register in `src/mcp_server.rs`
3. Add tests in `tests/`
4. Update documentation in `src/tools/documentation.rs`

## License

MIT License - Copyright (c) 2025 Ralf Anton Beier

## Acknowledgments

- Built with [FastMCP](https://github.com/jlowin/fastmcp)
- Custom WebSocket implementation for Loxone communication
- Implements the [Model Context Protocol](https://modelcontextprotocol.io)
