# 🏁 Quick Start Guide

**Get your Loxone MCP Rust server running in 5 minutes!**

## 📋 Prerequisites

- **Rust 1.70+** (we'll install if needed)
- **Loxone Miniserver**
- **5 minutes** of your time

## 🚀 One-Command Setup

```bash
# Install everything and run the server
curl -sSL https://raw.githubusercontent.com/your-repo/main/quick-start.sh | bash
```

This script will:
1. ✅ Install Rust (if not present)
2. ✅ Clone the repository
3. ✅ Setup credentials interactively
4. ✅ Build and run the server

## 🔧 Manual Setup (Alternative)

### Step 1: Install Rust

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Step 2: Clone & Build

```bash
# Clone repository
git clone https://github.com/your-repo/loxone-mcp-rust.git
cd loxone-mcp-rust

# Build the project
cargo build --release
```

### Step 3: Configure Credentials

Choose one of these methods:

#### Option A: Environment Variables (Recommended for Development)
```bash
export LOXONE_HOST="192.168.1.10"  # Your Miniserver IP
export LOXONE_USER="admin"
export LOXONE_PASS="your-password"
```

#### Option B: Interactive Setup
```bash
./dev-env.sh
# Follow the prompts to enter your credentials
```

#### Option C: Configuration File
```bash
# Create config file
cat > ~/.loxone-mcp/config.toml << EOF
[loxone]
host = "192.168.1.10"
user = "admin"
pass = "your-password"
EOF
```

### Step 4: Run the Server

#### For Claude Desktop
```bash
cargo run --bin loxone-mcp-server -- stdio
```

Add to Claude Desktop config:
```json
{
  "mcpServers": {
    "loxone": {
      "command": "/path/to/loxone-mcp-rust/target/release/loxone-mcp-server",
      "args": ["stdio"]
    }
  }
}
```

#### For n8n/Web API
```bash
cargo run --bin loxone-mcp-server -- http --port 3001
```

Access at: `http://localhost:3001`

## 🐳 Docker Quick Start

```bash
# Using Docker Compose
docker-compose up

# Or standalone Docker
docker run -e LOXONE_HOST=192.168.1.10 \
           -e LOXONE_USER=admin \
           -e LOXONE_PASS=yourpass \
           -p 3001:3001 \
           loxone-mcp:latest
```

## 🌐 WASM Quick Start

```bash
# Build WASM binary
make wasm

# Run with Wasmtime
wasmtime serve target/wasm32-wasip2/release/loxone-mcp-server.wasm
```

## ✅ Verify Installation

### Test Basic Connection
```bash
# Check server health
curl http://localhost:3001/health

# List available tools
curl http://localhost:3001/tools
```

### Test Device Control
```bash
# Turn on a light
curl -X POST http://localhost:3001/tools/call \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "control_device",
    "arguments": {
      "device": "Living Room Light",
      "action": "on"
    }
  }'
```

## 🔍 Troubleshooting

### Common Issues

| Problem | Solution |
|---------|----------|
| **Connection refused** | Check Miniserver IP and firewall |
| **Authentication failed** | Verify credentials with `cargo run --bin verify-connection` |
| **Build errors** | Update Rust: `rustup update` |
| **WASM errors** | Install target: `rustup target add wasm32-wasip2` |

### Debug Mode
```bash
# Enable detailed logging
export RUST_LOG=debug
cargo run -- stdio
```

## 🎯 Next Steps

1. **Explore Tools**: See [API Reference](API_REFERENCE.md) for all 30+ tools
2. **Production Setup**: Check [Deployment Guide](DEPLOYMENT.md)
3. **Customize**: Read [Development Guide](DEVELOPMENT.md)
4. **Monitor**: Setup [Dashboard](../monitoring/README.md)

## 💡 Quick Examples

### Control Multiple Lights
```bash
# Batch control example
curl -X POST http://localhost:3001/tools/call \
  -d '{
    "tool": "control_room_devices",
    "arguments": {
      "room": "Living Room",
      "device_type": "lights",
      "action": "off"
    }
  }'
```

### Read Temperature Sensors
```bash
# Get all temperature readings
curl -X POST http://localhost:3001/tools/call \
  -d '{
    "tool": "get_temperature_sensors",
    "arguments": {}
  }'
```

### Set Climate Control
```bash
# Set room temperature
curl -X POST http://localhost:3001/tools/call \
  -d '{
    "tool": "set_room_temperature",
    "arguments": {
      "room": "Bedroom",
      "temperature": 22.5
    }
  }'
```

---

**🎉 Congratulations!** You now have a fully functional Loxone MCP server running.

Need help? Check our [Troubleshooting Guide](TROUBLESHOOTING.md) or [open an issue](https://github.com/your-repo/issues).