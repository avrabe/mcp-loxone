<div align="center">
  <img src="mcp-loxone-gen1.png" alt="Loxone MCP Server" width="250"/>
  
  # Loxone MCP Server

  Model Context Protocol (MCP) server for Loxone Generation 1 home automation systems. Enables AI assistants to control lights, blinds, sensors, and weather data through natural language commands.

  **[📖 Landing Page](https://avrabe.github.io/mcp-loxone-gen1/)** | **[⚡ Quick Start](#quick-start)** | **[📋 Documentation](CLAUDE.md)** | **[🔧 Development Guide](DEVELOPMENT.md)**
</div>

## Key Features

- **27 MCP Tools**: Room control, lighting presets, weather forecasts, security monitoring
- **4 Deployment Modes**: Claude Desktop, HTTP/SSE server, STDIN CLI, Docker
- **Multiple Languages**: German, English, and mixed-language command support  
- **Secure Storage**: System keychain credential management (no plaintext files)
- **117 Device Support**: Lights, blinds, sensors, weather station, alarms
- **Gen 1 API**: HTTP basic auth for Loxone Generation 1 Miniservers

## Prerequisites

- Python 3.10+
- Loxone Miniserver Generation 1
- `uv` package manager ([install instructions](https://github.com/astral-sh/uv))

## Quick Start

```bash
git clone https://github.com/avrabe/mcp-loxone-gen1.git
cd mcp-loxone-gen1
uv sync
uvx --from . loxone-mcp setup  # Configure credentials
```

**Test Installation:**
```bash
uv run mcp dev src/loxone_mcp/server.py  # Opens MCP Inspector
```

## Deployment Options

### 1. Claude Desktop
```json
// ~/Library/Application Support/Claude/claude_desktop_config.json
{
  "mcpServers": {
    "loxone": {
      "command": "uvx", 
      "args": ["--from", "/path/to/mcp-loxone-gen1", "loxone-mcp-server"]
    }
  }
}
```

### 2. HTTP Server (SSE)
```bash
uvx --from . loxone-mcp-sse  # Starts on http://localhost:8080
```

### 3. Command Line (STDIN)
```bash
echo '{"method":"list_rooms","params":{},"id":1}' | uvx --from . loxone-mcp-server
```

### 4. Docker
```bash
docker-compose -f docker-compose.sse.yml up
```

### Environment Variables
```bash
export LOXONE_HOST="192.168.1.100"
export LOXONE_USER="your-username" 
export LOXONE_PASS="your-password"
```

## Usage Examples

Once configured in Claude Desktop, you can use natural language commands:

### English Commands
- "Turn on all lights in the living room"
- "Close all rolladen in the bedroom"
- "What's the status of lights in the kitchen?"
- "Open the rolladen in the office to 50%"

### German Commands (Deutsche Befehle)
- "Licht im Wohnzimmer einschalten"
- "Alle Rolladen im Schlafzimmer schließen"
- "Rolladen OG Büro runter"
- "Dimme die Lichter im Bad auf 30%"

### Mixed Language (Gemischte Sprache)
- "Turn off Licht in Küche"
- "Rolladen in living room hochfahren"
- "Dimmen OG bedroom lights auf 20%"

### Floor-based Commands (Stockwerk-Befehle)
- "All lights upstairs off" (Alle Lichter im OG aus)
- "Close all OG blinds" (Alle Rolladen im Obergeschoss zu)
- "Turn on lights in OG Büro" (Licht im OG Büro an)

## Available Tools

### Room Management
- `list_rooms()` - Get all available rooms
- `get_room_devices(room, device_type)` - List devices in a specific room

### Rolladen Control
- `control_rolladen(room, device, action, position)` - Control specific blind
- `control_room_rolladen(room, action)` - Control all blinds in a room

### Light Control
- `control_light(room, device, action, brightness)` - Control specific light
- `control_room_lights(room, action, brightness)` - Control all lights in a room

### Device Status
- `get_device_status(device_uuid)` - Get current state of any device
- `get_all_devices()` - List all available devices

## Security Considerations

- Credentials are stored in your system's secure keychain
- Only local network access is supported (no HTTPS on Gen 1)
- Consider using VPN for remote access
- The server implements read-only access by default for safety

## Troubleshooting

### Connection Issues
- Verify your Loxone Miniserver is accessible on the network
- Check that the IP address is correct
- Ensure your user has sufficient permissions

### Authentication Errors
- Re-run the setup: `uvx --from . loxone-mcp setup`
- Verify credentials in Loxone Config software

### Missing Devices
- Refresh structure: Restart the MCP server
- Check device visibility in Loxone Config

## Development

### Running locally
```bash
uv sync
uv run python -m loxone_mcp
```

### Testing multilingual support
```bash
python test_multilingual.py
```

### Adding new device types
1. Add control logic in `src/loxone_mcp/devices/`
2. Register new tools in `server.py`
3. Update README with examples
4. Add multilingual aliases in `i18n_aliases.py`

### LLM Integration
For optimal integration with language models like Qwen3:14b:
1. See `QWEN3_SETUP.md` for specific setup instructions
2. Check `LLM_INTEGRATION.md` for general LLM integration guide
3. Use the provided templates in `llm_templates.py`

## License

MIT

## Acknowledgments

- Built with [FastMCP](https://github.com/jlowin/fastmcp)
- Custom WebSocket implementation for Loxone communication
- Implements the [Model Context Protocol](https://modelcontextprotocol.io)
