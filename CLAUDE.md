# CLAUDE.md

<!--
SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
-->

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Development Commands

### Setup & Installation
```bash
# Install dependencies
uv sync

# Configure Loxone credentials (one-time setup)
uvx --from . loxone-mcp setup
# Or directly:
./setup.sh

# Verify existing credentials
uvx --from . loxone-mcp verify

# Clear all stored credentials (if needed)
uvx --from . loxone-mcp clear
```

### Running the Server
```bash
# Development mode with MCP Inspector (recommended for testing)
uv run mcp dev src/loxone_mcp/server.py

# Direct execution
uvx --from . loxone-mcp-server
# Or:
uv run python -m loxone_mcp
```

### Testing
```bash
# Run the test suite
uv run pytest tests/ -v --cov=loxone_mcp --cov-report=term-missing

# Basic integration test
uv run python test_server.py

# Validate MCP implementation (for CI)
uv run python validate_mcp.py
```

### Code Quality
```bash
# Run linting
uv run ruff check src/ tests/
# Or use make:
make lint

# Format code
uv run ruff format src/ tests/
# Or use make:
make format

# Type checking
uv run mypy src/
# Or use make:
make type-check

# Security checks
make security

# Run all checks before committing
make check
```

### Building & Publishing
```bash
# Build distribution packages
uv build

# Clean build artifacts
make clean
```

## High-Level Architecture

### Project Structure
The codebase implements a Model Context Protocol (MCP) server for controlling Loxone Generation 1 home automation systems. Key components:

1. **MCP Server** (`src/loxone_mcp/server.py`):
   - Built with FastMCP framework
   - Implements tools for device control (lights, rolladen/blinds) and sensor monitoring
   - Manages connection lifecycle and caching
   - Room-based organization of devices
   - Configurable sensor management without hardcoded UUIDs

2. **Loxone Clients**:
   - `loxone_http_client.py`: Primary HTTP-based client using basic auth
   - `loxone_client.py`: WebSocket client (for future encrypted auth support)
   - Both implement async communication with the Miniserver

3. **Credential Management** (`credentials.py`):
   - Uses system keychain (via `keyring` library) for secure storage
   - Falls back to environment variables for CI/CD
   - Provides CLI commands: setup, validate, clear

4. **Sensor Configuration** (`sensor_config.py`):
   - Configurable sensor management without hardcoded UUIDs
   - JSON-based configuration file (`sensor_config.json`)
   - Tools for adding/removing/discovering sensors dynamically
   - Separates sensor logic from main server code

### Key Design Patterns

1. **Server Context Pattern**:
   - Global `ServerContext` dataclass holds connection and cached data
   - Devices and rooms are parsed once at startup from structure file
   - Sensor configuration manager provides clean separation of concerns

2. **Device Abstraction**:
   - `LoxoneDevice` dataclass represents any controllable device
   - Devices have UUID, name, type, room assignment, and states
   - Control methods filter by device type (e.g., "Jalousie" for blinds)

3. **Configurable Sensor Management**:
   - No hardcoded UUIDs in server code
   - All sensor UUIDs stored in `sensor_config.json`
   - Runtime sensor discovery and configuration
   - Separates sensor logic from main server implementation

4. **Tool Organization**:
   - Tools organized by functionality: room queries, device control, sensor management
   - Each tool has clear single responsibility
   - Tools handle both specific device and room-wide operations

### Important Implementation Details

1. **Authentication**: Gen 1 Miniservers use basic HTTP auth. The WebSocket client exists for future encrypted auth support but HTTP is currently used.

2. **State Management**: Device states are fetched on-demand via HTTP requests. The structure is cached at startup but states are always fresh.

3. **Sensor Configuration**: Sensors are configured via `sensor_config.json` with no hardcoded UUIDs. Configuration includes discovery tools and runtime management.

4. **Error Handling**: Connection errors, auth failures, and missing devices are handled gracefully with informative error messages.

5. **Logging**: Controlled via `LOXONE_LOG_LEVEL` environment variable (default: INFO).

## Adding New Features

When adding support for new device types:
1. Check the device type in the structure file
2. Add filtering logic in server.py
3. Create appropriate MCP tools following existing patterns
4. Test with MCP Inspector before integration

## Sensor Management

The server uses a configurable sensor system:
1. **Configuration**: Edit `sensor_config.json` to add/remove sensors
2. **Discovery**: Use `discover_new_sensors()` tool to find working sensor UUIDs
3. **Management**: Use `add_sensor()`, `remove_sensor()`, `list_configured_sensors()` tools
4. **Monitoring**: Use `get_all_door_window_sensors()` for real-time status

Example sensor configuration:
```json
{
  "sensors": [
    {
      "uuid": "0CD8C06B.855703.I2",
      "name": "Kitchen Window",
      "type": "door_window", 
      "room": "Kitchen",
      "description": "Main kitchen window",
      "enabled": true
    }
  ]
}
```

## Dependencies

Core dependencies:
- `fastmcp`: MCP server framework
- `httpx`: Async HTTP client
- `keyring`: Secure credential storage
- `websockets` & `aiohttp`: For future WebSocket support

Development tools are configured in `pyproject.toml` with ruff for linting/formatting and mypy for type checking.