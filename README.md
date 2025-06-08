<div align="center">
  <img src="mcp-loxone-gen1.png" alt="Loxone MCP Server" width="250"/>
  
  # Loxone MCP Server

  A Model Context Protocol (MCP) server for controlling Loxone Generation 1 home automation systems. This server enables AI assistants like Claude Desktop to interact with your Loxone Miniserver, providing natural language control over lights, blinds (rolladen), and other connected devices.
</div>

## Features

- 🏠 **Room-based control** - Control devices organized by room
- 🪟 **Rolladen (blinds) control** - Up/Down/Stop commands with position support
- 💡 **Light control** - On/Off/Dim functionality 
- 🔐 **Secure credential storage** - Uses system keychain instead of plaintext files
- 🔄 **Real-time updates** - WebSocket connection for live state synchronization
- 🧩 **Extensible** - Easy to add support for more device types
- 🌐 **Multilingual support** - Accepts commands in German, English, or mixed languages
- 🤖 **LLM optimized** - Special support for Qwen3:14b, Llama 3.2, and other models

## Prerequisites

- Python 3.10+
- Loxone Miniserver Generation 1
- `uv` package manager ([install instructions](https://github.com/astral-sh/uv))

## Installation

### 1. Clone the repository

```bash
git clone https://github.com/yourusername/mcp-loxone-gen1.git
cd mcp-loxone-gen1
```

### 2. Set up credentials (one-time setup)

```bash
uvx --from . loxone-mcp setup
```

This will prompt you for:
- Loxone Miniserver IP address
- Username
- Password

Credentials are stored securely in your system keychain.

### 3. Test the server

```bash
uv run mcp dev src/loxone_mcp/server.py
```

This opens the MCP Inspector where you can test available tools.

## Configuration

### Claude Desktop

Add to your Claude Desktop configuration (`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "loxone": {
      "command": "uvx",
      "args": ["--from", "/Users/r/git/mcp-loxone-gen1", "loxone-mcp-server"],
      "env": {
        "LOXONE_LOG_LEVEL": "INFO"
      }
    }
  }
}
```

### Environment Variables (Optional)

For CI/CD or when you prefer environment variables over keychain:

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
