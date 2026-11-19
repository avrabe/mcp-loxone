<div align="center">

# 🏠 Loxone MCP Server

<sup>Control your Loxone smart home through the Model Context Protocol</sup>

&nbsp;

![Rust](https://img.shields.io/badge/Rust-CE422B?style=flat-square&logo=rust&logoColor=white&labelColor=1a1b27)
![MCP](https://img.shields.io/badge/MCP-0.17-654FF0?style=flat-square&labelColor=1a1b27)
![Edition](https://img.shields.io/badge/Edition-2024-blue?style=flat-square&labelColor=1a1b27)
[![CI](https://img.shields.io/github/actions/workflow/status/avrabe/mcp-loxone/ci.yml?style=flat-square&label=CI&labelColor=1a1b27)](https://github.com/avrabe/mcp-loxone/actions)
[![License](https://img.shields.io/badge/License-MIT%20%7C%20Apache--2.0-blue?style=flat-square&labelColor=1a1b27)](LICENSE-MIT)

</div>

&nbsp;

An async Rust MCP server that connects AI assistants to Loxone Miniservers. Control lights, blinds, climate, security, audio, and more — all through standardized MCP tools and resources.

> [!NOTE]
> Built on [rmcp](https://github.com/modelcontextprotocol/rust-sdk), the official Rust SDK for the Model Context Protocol (stdio and streamable HTTP transports).

## Features

- 🔌 **17 MCP Tools** — Control lights, blinds, HVAC, security, audio, door locks, intercoms, and scenes — all wired to real Miniserver commands
- 📊 **25+ MCP Resources** — Read-only access to rooms, devices, sensors, energy, weather, and system status with live state
- 🚀 **Three Transports** — stdio (Claude Desktop), HTTP/SSE (n8n, web clients), and Streamable HTTP
- 🔐 **Security by Default** — SSL verification on, UUID validation, rate limiting, input sanitization, dev-mode restricted to localhost
- ⚡ **Async Rust** — Connection pooling, intelligent caching, batch operations
- 🧊 **Nix Flake** — Reproducible builds with OpenClaw plugin integration
- 🔑 **Credential Management** — Credential ID system, environment variables, or Infisical vault

## Quick Start

```bash
# Build
cargo build --release

# Setup credentials
cargo run --bin loxone-mcp-setup -- --generate-id --name "My Home"

# Run with Claude Desktop
cargo run --bin loxone-mcp-server -- stdio --credential-id <your-id>
```

## Installation

### From Source

```bash
git clone https://github.com/avrabe/mcp-loxone
cd mcp-loxone
cargo build --release
```

### Nix Flake

```bash
nix build github:avrabe/mcp-loxone
# Or run directly
nix run github:avrabe/mcp-loxone
```

### Docker

```bash
docker build -t loxone-mcp .
docker run -e LOXONE_HOST=192.168.1.100 \
           -e LOXONE_USER=admin \
           -e LOXONE_PASS=secret \
           -p 3001:3001 loxone-mcp
```

## Configuration

### Credential ID (Recommended)

```bash
# Interactive setup — generates a unique credential ID
cargo run --bin loxone-mcp-setup -- --generate-id --name "Main House"

# Or store manually
cargo run --bin loxone-mcp-auth -- store \
  --name "Office" --host 192.168.1.100 \
  --username admin --password secure123

# Manage credentials
cargo run --bin loxone-mcp-auth -- list
cargo run --bin loxone-mcp-auth -- test <credential-id>
```

### Environment Variables

```bash
export LOXONE_HOST="192.168.1.100"
export LOXONE_USER="admin"
export LOXONE_PASS="password"
```

### Infisical Vault (Production)

```bash
export INFISICAL_PROJECT_ID="your-project-id"
export INFISICAL_CLIENT_ID="your-client-id"
export INFISICAL_CLIENT_SECRET="your-client-secret"
```

> [!TIP]
> Migrating from environment variables? See the [Credential Migration Guide](CREDENTIAL_MIGRATION_GUIDE.md).

## Usage

### Claude Desktop

Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "loxone": {
      "command": "loxone-mcp-server",
      "args": ["stdio", "--credential-id", "abc123def-456-789"]
    }
  }
}
```

### HTTP Server (n8n, MCP Inspector)

```bash
# Standard HTTP/SSE
loxone-mcp-server http --port 3001 --credential-id <id>

# Streamable HTTP (new MCP Inspector)
loxone-mcp-server streamable-http --port 3001 --credential-id <id>
```

### OpenClaw Integration

The flake exports an `openclawPlugin` for [nix-openclaw](https://github.com/openclaw/nix-openclaw):

```nix
customPlugins = [
  {
    source = "github:avrabe/mcp-loxone";
    config.env = {
      LOXONE_HOST = "/path/to/secrets/loxone-host";
      LOXONE_USER = "/path/to/secrets/loxone-user";
      LOXONE_PASS = "/path/to/secrets/loxone-pass";
    };
  }
];
```

## Tools & Resources

### Tools (Actions)

| Category | Tools | Description |
|----------|-------|-------------|
| **Lighting** | `control_light` | On/off, dim 0-100% |
| **Blinds** | `control_blind` | Up/down/stop, position 0-100% |
| **Climate** | `set_temperature` | Target temperature with safe range validation |
| **Security** | `set_security_mode` | Arm, disarm, night, away modes |
| **Doors** | `control_door_lock` | Lock, unlock, open |
| **Intercom** | `control_intercom` | Answer, decline, open door |
| **Audio** | `control_audio` | Play, pause, volume per zone |
| **Scenes** | `activate_scene` | Trigger named scenes |
| **General** | `control_device`, `get_*_status` | Direct device control, live status queries |

### Resources (Read-Only)

| URI Pattern | Data |
|-------------|------|
| `loxone://rooms` | Room listing with device counts |
| `loxone://rooms/{room}/devices` | Devices in a specific room |
| `loxone://devices/all` | Full device inventory |
| `loxone://devices/category/{cat}` | Devices by category |
| `loxone://sensors/*` | Door/window, temperature, motion |
| `loxone://audio/zones` | Audio zone configuration |
| `loxone://system/status` | Miniserver status and capabilities |
| `loxone://energy/*` | Power monitoring and consumption |

## Architecture

```
AI Assistant (Claude, n8n, OpenClaw)
        │
   MCP Protocol (stdio / HTTP / Streamable HTTP)
        │
   ┌────▼─────────────────────────┐
   │   loxone-mcp-server          │
   │  ┌─────────────────────────┐ │
   │  │ Security Layer          │ │
   │  │ Auth · Rate Limit · TLS │ │
   │  ├─────────────────────────┤ │
   │  │ Tool Handlers           │ │
   │  │ 17 tools → send_command │ │
   │  ├─────────────────────────┤ │
   │  │ Loxone Client           │ │
   │  │ HTTP · WebSocket · Cache│ │
   │  └─────────────────────────┘ │
   └────┬─────────────────────────┘
        │
   HTTP /jdev/sps/io/{uuid}/{cmd}
        │
   Loxone Miniserver
        │
   Physical Devices
```

## Development

### Requirements

- Rust 1.85+ (2024 edition)
- Loxone Miniserver (Gen 1 or Gen 2)

### Building & Testing

```bash
cargo build                                    # Dev build
cargo test --lib                               # Unit tests
cargo fmt && cargo clippy -- -D warnings       # Format + lint
cargo audit                                    # Security audit
```

### Live Miniserver Testing

```bash
# Requires network access to Miniserver
LOXONE_LIVE_TEST=1 cargo test \
  --test live_miniserver_tests \
  --features test-utils -- --nocapture
```

### Project Structure

```
src/
├── server/          # MCP protocol, tool handlers (macro_backend.rs)
├── client/          # HTTP/WebSocket clients with UUID validation
├── config/          # Credentials, master key auto-persistence
├── security/        # Input sanitization, rate limiting, CORS
├── monitoring/      # Metrics, dashboards, InfluxDB
├── history/         # Time-series data storage
├── discovery/       # mDNS network discovery
└── main.rs          # CLI, transport selection, startup
```

### Binaries

| Binary | Purpose |
|--------|---------|
| `loxone-mcp-server` | Main MCP server (stdio/HTTP/streamable-http) |
| `loxone-mcp-auth` | Credential management (store, list, test, delete) |
| `loxone-mcp-setup` | Interactive setup with credential ID generation |
| `loxone-mcp-test-endpoints` | API endpoint testing (development) |

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

at your option.

<div align="center">

&nbsp;

<sub>Built on <a href="https://github.com/modelcontextprotocol/rust-sdk">rmcp</a>, the official Rust SDK for the Model Context Protocol</sub>

</div>
