# Development Guide

This guide helps you develop and extend the Loxone MCP Server.

## Project Structure

```
mcp-loxone/
├── src/loxone_mcp/        # Main package
│   ├── __init__.py        # Package initialization
│   ├── __main__.py        # Entry point for python -m
│   ├── server.py          # MCP server implementation
│   └── secrets.py         # Credential management
├── pyproject.toml         # Project configuration
├── README.md             # User documentation
├── DEVELOPMENT.md        # This file
├── setup.sh              # Quick setup script
└── test_server.py        # Basic test suite
```

## Development Setup

1. **Clone and enter the directory:**
   ```bash
   git clone <your-repo>
   cd mcp-loxone
   ```

2. **Install dependencies:**
   ```bash
   uv sync
   ```

3. **Set up credentials:**
   ```bash
   ./setup.sh
   # Or directly:
   uvx --from . loxone-mcp setup
   ```

## Running the Server

### Development Mode (with MCP Inspector)
```bash
uv run mcp dev src/loxone_mcp/server.py
```
This opens a web interface at http://localhost:6274 for testing.

### Direct Execution
```bash
# Using uvx (recommended)
uvx --from . loxone-mcp-server

# Using Python directly
uv run python -m loxone_mcp
```

### Available Commands
```bash
# Configure credentials
uvx --from . loxone-mcp setup

# Validate credentials
uvx --from . loxone-mcp validate

# Clear credentials
uvx --from . loxone-mcp clear
```

## Adding New Features

### 1. Adding a New Device Type

To support a new device type (e.g., temperature sensors):

```python
# In server.py

@mcp.tool()
async def get_temperature_sensors(room: Optional[str] = None) -> List[Dict[str, Any]]:
    """Get temperature readings from sensors."""
    ctx: ServerContext = mcp.get_context()
    
    # Filter for temperature sensor types
    sensors = [
        device for device in ctx.devices.values()
        if device.type in ["InfoOnlyAnalog", "Temperature"]
        and (room is None or room.lower() in device.room.lower())
    ]
    
    results = []
    for sensor in sensors:
        # Get current value
        if "value" in sensor.states:
            value = await ctx.loxone.get_state(sensor.states["value"])
            results.append({
                "name": sensor.name,
                "room": sensor.room,
                "value": value,
                "unit": sensor.details.get("unit", "°C")
            })
    
    return results
```

### 2. Adding New MCP Resources

Resources provide read-only data access:

```python
@mcp.resource("loxone://device/{uuid}")
async def get_device_details(uuid: str) -> str:
    """Get detailed information about a specific device."""
    ctx: ServerContext = mcp.get_context()
    
    if uuid not in ctx.devices:
        return json.dumps({"error": "Device not found"})
    
    device = ctx.devices[uuid]
    return json.dumps({
        "device": device.__dict__,
        "states": await get_device_status(uuid)
    }, indent=2)
```

### 3. Adding MCP Prompts

Prompts provide reusable templates:

```python
@mcp.prompt()
async def scene_control(room: str) -> str:
    """Generate a prompt for scene control."""
    devices = await get_room_devices(room)
    
    return f"""
    Room: {room}
    Available devices: {len(devices)}
    
    Device types:
    - Lights: {len([d for d in devices if "Light" in d["type"]])}
    - Rolladen: {len([d for d in devices if d["type"] == "Jalousie"])}
    
    What scene would you like to activate?
    """
```

## Testing

### Unit Tests
```bash
# Run the basic test suite
uv run python test_server.py
```

### Integration Testing with MCP Inspector
1. Start the server with inspector:
   ```bash
   uv run mcp dev src/loxone_mcp/server.py
   ```

2. Test each tool:
   - Click on a tool
   - Fill in parameters
   - Execute and verify results

### Manual Testing with Claude Desktop
1. Update your Claude config
2. Restart Claude Desktop
3. Test commands like:
   - "List all rooms"
   - "Turn on lights in kitchen"
   - "Close all rolladen"

## Debugging

### Enable Debug Logging
```bash
export LOXONE_LOG_LEVEL=DEBUG
uvx --from . loxone-mcp-server
```

### Common Issues

**Connection Refused**
- Check Loxone IP address
- Verify Miniserver is accessible
- Check firewall settings

**Authentication Failed**
- Re-run setup: `uvx --from . loxone-mcp setup`
- Verify credentials in Loxone Config
- Check user permissions

**Missing Devices**
- Refresh Loxone Config
- Check device visibility settings
- Verify room assignments

## Publishing

### Local Testing
```bash
# Build the package
uv build

# Install locally
uv pip install dist/*.whl
```

### Publishing to PyPI
```bash
# Build
uv build

# Upload to TestPyPI first
uv publish --repository testpypi

# Test installation
uvx --index https://test.pypi.org/simple/ loxone-mcp-server

# Publish to PyPI
uv publish
```

## Code Style

- Use type hints for all functions
- Add docstrings to all public functions
- Follow PEP 8 conventions
- Keep tools focused and single-purpose

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## Useful Resources

- [MCP Documentation](https://modelcontextprotocol.io)
- [FastMCP Documentation](https://github.com/jlowin/fastmcp)
- [Loxone WebSocket API](https://www.loxone.com/enen/kb/api/)
- [Loxone Web Services Documentation](https://www.loxone.com/enen/kb/web-services/)
