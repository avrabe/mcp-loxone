# Migration Guide: Python to Rust

This guide helps you migrate from the Python implementation to the new high-performance Rust implementation.

## Why Migrate?

The Rust implementation provides significant advantages:

- **🚀 Performance**: 10-100x faster execution with zero-cost abstractions
- **🛡️ Security**: Enhanced consent management and audit trails
- **🔧 Features**: 23+ MCP tools vs ~10 in Python version
- **🌐 Deployment**: WASM, native binaries, Docker, HTTP/SSE
- **📊 Monitoring**: Health checks, metrics, connection pooling
- **🔄 Integration**: n8n workflow engine support

## Pre-Migration Checklist

### 1. Backup Current Configuration
```bash
# Backup Python configuration
mkdir -p ~/backup/python-loxone-mcp
cp ~/.config/loxone-mcp/* ~/backup/python-loxone-mcp/ 2>/dev/null || true

# Backup Claude Desktop config
cp ~/Library/Application\ Support/Claude/claude_desktop_config.json ~/backup/
```

### 2. Document Current Setup
- Note your current Loxone host IP
- List devices and rooms you frequently control
- Record any custom automation scripts

## Migration Steps

### Step 1: Install Rust (if needed)
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Step 2: Build Rust Implementation
```bash
cd mcp-loxone/loxone-mcp-rust
cargo build --release
```

### Step 3: Setup Credentials
```bash
# Interactive setup
./target/release/loxone-mcp-rust setup

# Or use environment variables
export LOXONE_HOST="192.168.1.100"
export LOXONE_USERNAME="your-username"
export LOXONE_PASSWORD="your-password"
```

### Step 4: Test Connection
```bash
# Health check
./target/release/loxone-mcp-rust health-check

# List available tools
./target/release/loxone-mcp-rust list-tools
```

### Step 5: Update Claude Desktop Configuration
```json
{
  "mcpServers": {
    "loxone": {
      "command": "/full/path/to/loxone-mcp-rust/target/release/loxone-mcp-rust",
      "args": ["--mcp-mode"]
    }
  }
}
```

### Step 6: Install Globally (Optional)
```bash
cd loxone-mcp-rust
cargo install --path .
# Now you can use: loxone-mcp-rust from anywhere
```

## Feature Mapping

### Python → Rust Tool Equivalents

| Python Tool | Rust Equivalent | Notes |
|-------------|-----------------|-------|
| `list_rooms()` | `list_rooms()` | Same interface |
| `get_room_devices()` | `get_room_devices()` | Enhanced filtering |
| `control_light()` | `control_device()` | Unified device control |
| `control_rolladen()` | `control_device()` | Unified device control |
| `control_room_lights()` | `control_room_lights()` | Enhanced batch operations |
| `control_room_rolladen()` | `control_room_rolladen()` | Enhanced batch operations |
| N/A | `get_audio_zones()` | **New: Audio control** |
| N/A | `get_health_check()` | **New: Health monitoring** |
| N/A | `get_energy_consumption()` | **New: Energy management** |
| N/A | `get_weather_data()` | **New: Weather integration** |

### New Capabilities

The Rust version includes many new features not available in Python:

#### 🎯 Batch Operations
- `control_all_lights()` - System-wide light control
- `control_all_rolladen()` - System-wide blind control
- Parallel execution with automatic optimization

#### 🔐 Security & Consent
- Interactive consent management for sensitive operations
- Audit trails for all device control actions
- Configurable security policies

#### 📊 Monitoring & Health
- `get_health_check()` - Comprehensive system health
- `get_system_status()` - Real-time status information
- Connection pool monitoring and metrics

#### 🌐 Advanced Deployment
- WASM builds for browser deployment
- HTTP/SSE server with WebSocket fallback
- Docker containerization with health checks

## Common Migration Issues

### 1. Tool Name Changes
**Issue**: Some tools have been renamed for consistency.

**Solution**: Update your prompts to use the new unified tool names:
- `control_light()` → `control_device(device="light_name", action="on")`
- `control_rolladen()` → `control_device(device="blind_name", action="down")`

### 2. Authentication Methods
**Issue**: Python used simple keychain storage.

**Solution**: Rust supports multiple credential backends:
```bash
# Use Infisical (recommended for teams)
export INFISICAL_PROJECT_ID="your-project"
./target/release/loxone-mcp-rust setup

# Or traditional keychain (default)
./target/release/loxone-mcp-rust setup
```

### 3. Performance Differences
**Issue**: Python had slower response times.

**Solution**: The Rust version is much faster, but you might need to adjust:
- Reduce polling intervals if you had them
- Remove artificial delays in automation scripts
- Update timeout values if they were set very high

### 4. Configuration Location
**Issue**: Python stored config in `~/.config/loxone-mcp/`

**Solution**: Rust uses system-specific locations:
- **macOS**: `~/Library/Application Support/loxone-mcp/`
- **Linux**: `~/.config/loxone-mcp/`
- **Windows**: `%APPDATA%\loxone-mcp\`

## Validation

### Test Your Migration
1. **Basic connectivity**:
   ```bash
   loxone-mcp-rust health-check
   ```

2. **Device discovery**:
   ```bash
   loxone-mcp-rust discover-devices
   ```

3. **Room listing**:
   ```bash
   loxone-mcp-rust list-rooms
   ```

4. **Claude Desktop integration**:
   - Restart Claude Desktop
   - Try: "List all rooms in my Loxone system"
   - Try: "Turn on the living room lights"

### Performance Verification
The Rust implementation should show:
- **Response times**: 10-50ms vs 100-500ms in Python
- **Memory usage**: 10-20MB vs 50-100MB in Python  
- **CPU usage**: Near zero vs 5-10% in Python
- **Startup time**: 100-200ms vs 1-2s in Python

## Rollback Plan

If you need to rollback to Python:

1. **Restore Python environment**:
   ```bash
   cd archive/python-legacy
   uv sync
   ```

2. **Restore Claude Desktop config**:
   ```bash
   cp ~/backup/claude_desktop_config.json ~/Library/Application\ Support/Claude/
   ```

3. **Restore credentials**:
   ```bash
   cp ~/backup/python-loxone-mcp/* ~/.config/loxone-mcp/
   ```

## Getting Help

- **Documentation**: [loxone-mcp-rust/README.md](loxone-mcp-rust/README.md)
- **Issues**: Create a GitHub issue with `migration` label
- **Discussions**: GitHub Discussions for questions
- **Health Check**: `loxone-mcp-rust health-check --verbose`

## Post-Migration Cleanup

Once you've confirmed the Rust version works:

1. **Remove Python dependencies** (optional):
   ```bash
   rm -rf .venv
   rm pyproject.toml uv.lock
   ```

2. **Update documentation references**
3. **Clean up old log files**
4. **Update any automation scripts**

The Python implementation remains available in `archive/python-legacy/` for reference.