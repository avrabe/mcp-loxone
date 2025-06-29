# Loxone MCP Server

> **Bridging Loxone home automation with the Model Context Protocol ecosystem through high-performance Rust implementation**

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![CI](https://github.com/avrabe/mcp-loxone/actions/workflows/ci.yml/badge.svg)](https://github.com/avrabe/mcp-loxone/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

A Model Context Protocol (MCP) server that enables programmatic control of Loxone home automation systems. This implementation provides comprehensive device control through 34 specialized tools, supporting both stdio (for Claude Desktop) and HTTP transports.

## Features

- **Comprehensive Control**: 34 MCP tools covering lights, blinds, climate, audio, sensors, and more
- **Multiple Transports**: stdio for Claude Desktop, HTTP/SSE for web integrations
- **Enterprise Security**: API key authentication with role-based access control
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
cd loxone-mcp-rust
cargo build --release
```

### Configuration

1. **Set up credentials** (interactive):
   ```bash
   cargo run --bin loxone-mcp-setup
   ```

2. **Or use environment variables**:
   ```bash
   export LOXONE_HOST="http://192.168.1.100"
   export LOXONE_USER="your-username"
   export LOXONE_PASS="your-password"
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

## Available Tools

The server implements 34 tools organized by category:

### Device Control
- `control_device` - Direct device control by UUID
- `control_multiple_devices` - Batch device operations
- `discover_all_devices` - List all available devices
- `get_devices_by_category` - Filter devices by type

### Lighting
- `control_lights_unified` - Control lights by room or globally
- `get_light_scenes` - Available lighting scenes
- `set_light_scene` - Activate lighting scenes

### Climate Control
- `get_room_climate` - Temperature and humidity data
- `set_room_temperature` - Adjust room temperature
- `get_climate_control` - HVAC system status
- `set_room_mode` - Set comfort/eco/off modes

### Audio
- `get_audio_zones` - List audio zones
- `control_audio_zone` - Play/pause/stop audio
- `set_audio_volume` - Volume control
- `get_audio_sources` - Available audio sources

### Sensors
- `get_all_door_window_sensors` - Security sensor status
- `get_temperature_sensors` - Temperature readings
- `get_motion_sensors` - Motion detection status
- `discover_sensor_capabilities` - Available sensor types

[Full tool documentation →](docs/tools_reference.md)

## Architecture

The server uses an async Rust architecture with:

- **Transport Layer**: Supports stdio and HTTP/SSE
- **Tool Layer**: Modular tool implementations
- **Client Layer**: HTTP and WebSocket clients for Miniserver communication
- **Security Layer**: Authentication, rate limiting, input validation
- **Cache Layer**: Intelligent state caching with TTL

[Architecture details →](docs/architecture.md)

## Security

- **Authentication**: API key based with role support (admin, operator, viewer)
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