# Loxone MCP Server

> **Bridging Loxone home automation with the Model Context Protocol ecosystem through high-performance Rust implementation**

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![CI](https://github.com/avrabe/mcp-loxone/actions/workflows/ci.yml/badge.svg)](https://github.com/avrabe/mcp-loxone/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

A Model Context Protocol (MCP) server that enables programmatic control of Loxone home automation systems. This implementation provides comprehensive device control through 17 specialized tools and 25+ resources, supporting both stdio (for Claude Desktop) and HTTP transports.

## Features

- **Comprehensive Control**: 17 MCP tools for device control + 25+ resources for data access
- **MCP Compliant**: Proper separation of tools (actions) and resources (read-only data)
- **Multiple Transports**: stdio for Claude Desktop, HTTP/SSE for web integrations  
- **Enhanced Security**: Framework-based authentication with advanced features
- **Performance Optimized**: Connection pooling, intelligent caching, batch operations
- **Framework Integration**: Built on PulseEngine MCP framework for standardized protocol handling

## Requirements

- Rust 1.70 or higher
- Loxone Miniserver (Gen 1 or Gen 2)
- Network access to Miniserver

## Installation

### From Source

```bash
git clone https://github.com/avrabe/mcp-loxone
cd mcp-loxone
cargo build --release
```

### Configuration

1. **Basic Setup**:
   ```bash
   cargo run --bin loxone-mcp-setup
   ```

2. **Environment variables**:
   ```bash
   export LOXONE_HOST="http://192.168.1.100"
   export LOXONE_USER="your-username"
   export LOXONE_PASS="your-password"
   ```

3. **Production with Infisical** (optional):
   ```bash
   export INFISICAL_PROJECT_ID="your-project-id"
   export INFISICAL_CLIENT_ID="your-client-id"
   export INFISICAL_CLIENT_SECRET="your-client-secret"
   export INFISICAL_ENVIRONMENT="production"
   ```

## Usage

### Claude Desktop Integration

Add to your Claude Desktop configuration:

```json
{
  "mcpServers": {
    "loxone": {
      "command": "/path/to/loxone-mcp-server",
      "args": ["stdio"]
    }
  }
}
```

### HTTP Server (for n8n, web clients)

```bash
./loxone-mcp-server http --port 3001
```

### Verify Installation

```bash
cargo run --bin loxone-mcp-verify
```

## Available Tools & Resources

The server implements 17 tools for actions and 25+ resources for data access:

### Tools (Actions)
- **Device Control**: `control_device`, `control_multiple_devices`
- **Lighting**: `control_lights_unified`, `control_all_lights`, `control_room_lights` 
- **Blinds/Rolladen**: `control_rolladen_unified`, `control_all_rolladen`, `control_room_rolladen`, `discover_rolladen_capabilities`
- **Climate**: `set_room_temperature`, `set_room_mode`
- **Audio**: `control_audio_zone`, `set_audio_volume`
- **Security**: `arm_alarm`, `disarm_alarm`
- **Workflows**: `create_workflow`, `execute_workflow_demo`

### Resources (Read-Only Data)
- **Rooms**: `loxone://rooms`, `loxone://rooms/{room}/devices`, `loxone://rooms/{room}/overview`
- **Devices**: `loxone://devices/all`, `loxone://devices/category/{category}`, `loxone://devices/type/{type}`
- **System**: `loxone://system/status`, `loxone://system/capabilities`, `loxone://system/categories`
- **Sensors**: `loxone://sensors/door-window`, `loxone://sensors/temperature`, `loxone://sensors/motion`
- **Audio**: `loxone://audio/zones`, `loxone://audio/sources`
- **And more...** (weather, energy, security, climate)

[Full tool documentation →](docs/tools_reference.md) | [Resource documentation →](docs/resources.md)

## Architecture

The server uses an async Rust architecture with:

- **Transport Layer**: Supports stdio and HTTP/SSE
- **Tool Layer**: Modular tool implementations
- **Client Layer**: HTTP and WebSocket clients for Miniserver communication
- **Security Layer**: Authentication, rate limiting, input validation
- **Cache Layer**: Intelligent state caching with TTL

[Architecture details →](docs/architecture.md)

## Security

- **Framework Authentication**: Enhanced security with PulseEngine MCP v0.4.0
  - API Key Management with role-based permissions
  - JWT tokens for stateless sessions  
  - Encrypted storage with AES-GCM
  - Vault integration (Infisical support)
  - 8 predefined security profiles
- **Rate Limiting**: Configurable per-role limits
- **Input Validation**: All inputs sanitized and validated
- **Audit Logging**: Comprehensive activity logging

[Security documentation →](docs/security.md)

## Development

### Building from Source

```bash
# Development build with debug symbols
cargo build

# Run tests
cargo test

# Format and lint
cargo fmt && cargo clippy
```

### Project Structure

```
src/
├── server/          # MCP protocol implementation
├── tools/           # Tool implementations
├── client/          # Loxone client
├── security/        # Auth and validation
└── main.rs          # Entry point
```

## Limitations

- **WASM Support**: Currently disabled due to tokio compatibility issues
- **Real-time Updates**: WebSocket subscriptions planned but not yet implemented
- **Miniserver Version**: Tested with Gen 1 and Gen 2, newer versions may have differences

## Contributing

Contributions are welcome! Please see [contributing.md](contributing.md) for guidelines.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Acknowledgments

Built on the PulseEngine MCP framework. Special thanks to the Loxone community for protocol documentation.