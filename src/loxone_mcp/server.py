"""Loxone MCP Server - Main server implementation."""

import json
import logging
import os
import sys
from collections.abc import AsyncGenerator
from contextlib import asynccontextmanager
from dataclasses import dataclass
from typing import Any

from mcp.server.fastmcp import FastMCP

from loxone_mcp.secrets import LoxoneSecrets

# Set up logging
logging.basicConfig(
    level=os.getenv("LOXONE_LOG_LEVEL", "INFO"),
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger(__name__)

# Create the MCP server instance
mcp = FastMCP("Loxone Controller")


@dataclass
class LoxoneDevice:
    """Represents a Loxone device with its properties."""

    uuid: str
    name: str
    type: str
    room: str
    room_uuid: str
    category: str | None = None
    states: dict[str, Any] | None = None
    details: dict[str, Any] | None = None


@dataclass
class ServerContext:
    """Server context with Loxone connection and device cache."""

    loxone: Any  # PyLoxone instance
    structure: dict[str, Any]
    devices: dict[str, LoxoneDevice]
    rooms: dict[str, str]  # uuid -> name mapping


# Global context storage
_context: ServerContext | None = None


@asynccontextmanager
async def lifespan(_server: FastMCP) -> AsyncGenerator[Any, None]:
    """Manage server lifecycle - initialize Loxone connection."""
    global _context

    logger.info("Starting Loxone MCP Server...")

    # Validate credentials
    if not LoxoneSecrets.validate():
        raise ValueError("Missing Loxone credentials. Run 'setup' command first.")

    # Get credentials
    host = LoxoneSecrets.get(LoxoneSecrets.HOST_KEY)
    username = LoxoneSecrets.get(LoxoneSecrets.USER_KEY)
    password = LoxoneSecrets.get(LoxoneSecrets.PASS_KEY)

    logger.info(f"Connecting to Loxone Miniserver at {host}...")

    try:
        # Import our Loxone client
        # Use HTTP client since WebSocket requires encrypted auth
        from loxone_mcp.loxone_http_client import Loxone

        # Create connection
        loxone = Loxone(host=host, username=username, password=password)

        # Connect and start
        await loxone.connect()
        await loxone.start()

        logger.info("Successfully connected to Loxone Miniserver")

        # Get structure file
        structure = await loxone.get_structure_file()

        # Parse rooms
        rooms = {
            uuid: data.get("name", "Unknown") for uuid, data in structure.get("rooms", {}).items()
        }
        logger.info(f"Found {len(rooms)} rooms")

        # Parse devices
        devices = {}
        for uuid, control in structure.get("controls", {}).items():
            room_uuid = control.get("room", "")
            room_name = rooms.get(room_uuid, "Unknown")

            device = LoxoneDevice(
                uuid=uuid,
                name=control.get("name", "Unknown"),
                type=control.get("type", "Unknown"),
                room=room_name,
                room_uuid=room_uuid,
                category=control.get("cat"),
                states=control.get("states", {}),
                details=control.get("details", {}),
            )
            devices[uuid] = device

        logger.info(f"Found {len(devices)} devices")

        # Log device type summary
        device_types = {}
        for device in devices.values():
            device_types[device.type] = device_types.get(device.type, 0) + 1

        logger.info("Device types found:")
        for dtype, count in sorted(device_types.items()):
            logger.info(f"  - {dtype}: {count}")

        # Create context
        context = ServerContext(loxone=loxone, structure=structure, devices=devices, rooms=rooms)

        # Store globally
        _context = context

        yield context

    except ImportError as e:
        logger.error(f"Failed to import required module: {e}")
        raise
    except Exception as e:
        logger.error(f"Failed to connect to Loxone: {e}")
        raise
    finally:
        logger.info("Shutting down Loxone connection...")
        _context = None
        try:
            await loxone.stop()
            await loxone.close()
        except Exception:
            pass


# Attach lifespan to server
mcp = FastMCP("Loxone Controller", lifespan=lifespan)


# === Room Management Tools ===


@mcp.tool()
async def list_rooms() -> list[dict[str, str]]:
    """
    List all available rooms in the Loxone system.

    Returns a list of rooms with their UUID and name.
    """
    if not _context:
        return [{"error": "Not connected to Loxone"}]

    return [{"uuid": uuid, "name": name} for uuid, name in _context.rooms.items()]


@mcp.tool()
async def get_room_devices(room: str, device_type: str | None = None) -> list[dict[str, Any]]:
    """
    Get all devices in a specific room.

    Args:
        room: Room name (partial match supported)
        device_type: Optional filter by device type (e.g., "Light", "Jalousie")

    Returns:
        List of devices with their properties
    """
    ctx: ServerContext = _context
    if not ctx:
        return []

    # Find matching room(s)
    room_lower = room.lower()
    matching_rooms = [
        (uuid, name) for uuid, name in ctx.rooms.items() if room_lower in name.lower()
    ]

    if not matching_rooms:
        return []

    # Get devices in matching rooms
    devices = []
    for room_uuid, _room_name in matching_rooms:
        room_devices = [
            {
                "uuid": device.uuid,
                "name": device.name,
                "type": device.type,
                "room": device.room,
                "category": device.category,
            }
            for device in ctx.devices.values()
            if device.room_uuid == room_uuid and (device_type is None or device.type == device_type)
        ]
        devices.extend(room_devices)

    return devices


# === Rolladen (Blinds) Control ===


@mcp.tool()
async def control_rolladen(
    room: str, device: str | None = None, action: str = "stop", position: int | None = None
) -> dict[str, Any]:
    """
    Control rolladen (blinds) in a room.

    Args:
        room: Room name (partial match)
        device: Specific device name (optional, controls all if not specified)
        action: "up", "down", "stop", or "position"
        position: Position 0-100 (only used with action="position")

    Returns:
        Result of the control operation
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Get rolladen devices in room
    devices = await get_room_devices(room, "Jalousie")

    if not devices:
        return {"error": f"No rolladen found in room '{room}'"}

    # Filter by device name if specified
    if device:
        device_lower = device.lower()
        devices = [d for d in devices if device_lower in d["name"].lower()]

        if not devices:
            return {"error": f"No rolladen named '{device}' found in room '{room}'"}

    # Execute commands
    results = []
    for dev in devices:
        try:
            uuid = dev["uuid"]

            # Map action to Loxone command
            if action == "position" and position is not None:
                command = f"moveToPosition/{position}"
            elif action == "up":
                command = "FullUp"
            elif action == "down":
                command = "FullDown"
            elif action == "stop":
                command = "Stop"
            else:
                results.append({"device": dev["name"], "error": f"Invalid action: {action}"})
                continue

            # Send command
            await ctx.loxone.send_command(f"jdev/sps/io/{uuid}/{command}")

            results.append(
                {
                    "device": dev["name"],
                    "action": action,
                    "success": True,
                    "position": position if action == "position" else None,
                }
            )

        except Exception as e:
            results.append({"device": dev["name"], "error": str(e)})

    return {
        "room": room,
        "controlled": len([r for r in results if r.get("success")]),
        "results": results,
    }


@mcp.tool()
async def control_room_rolladen(room: str, action: str = "stop") -> dict[str, Any]:
    """
    Control all rolladen in a room with a simple command.

    Args:
        room: Room name
        action: "up", "down", or "stop"

    Returns:
        Result of the control operation
    """
    return await control_rolladen(room=room, action=action)


# === Light Control ===


@mcp.tool()
async def control_light(
    room: str, device: str | None = None, action: str = "toggle", brightness: int | None = None
) -> dict[str, Any]:
    """
    Control lights in a room.

    Args:
        room: Room name (partial match)
        device: Specific device name (optional)
        action: "on", "off", "toggle", or "dim"
        brightness: Brightness 0-100 (only used with action="dim")

    Returns:
        Result of the control operation
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Get light devices - handle different light types
    light_types = ["Light", "LightController", "Dimmer", "Switch"]
    all_devices = []

    for light_type in light_types:
        devices = await get_room_devices(room, light_type)
        all_devices.extend(devices)

    if not all_devices:
        return {"error": f"No lights found in room '{room}'"}

    # Remove duplicates
    seen = set()
    devices = []
    for d in all_devices:
        if d["uuid"] not in seen:
            seen.add(d["uuid"])
            devices.append(d)

    # Filter by device name if specified
    if device:
        device_lower = device.lower()
        devices = [d for d in devices if device_lower in d["name"].lower()]

        if not devices:
            return {"error": f"No light named '{device}' found in room '{room}'"}

    # Execute commands
    results = []
    for dev in devices:
        try:
            uuid = dev["uuid"]

            # Map action to Loxone command
            if action == "on":
                command = "On"
            elif action == "off":
                command = "Off"
            elif action == "toggle":
                command = "Pulse"
            elif action == "dim" and brightness is not None:
                command = str(brightness)  # Direct value for dimmers
            else:
                results.append({"device": dev["name"], "error": f"Invalid action: {action}"})
                continue

            # Send command
            await ctx.loxone.send_command(f"jdev/sps/io/{uuid}/{command}")

            results.append(
                {
                    "device": dev["name"],
                    "action": action,
                    "success": True,
                    "brightness": brightness if action == "dim" else None,
                }
            )

        except Exception as e:
            results.append({"device": dev["name"], "error": str(e)})

    return {
        "room": room,
        "controlled": len([r for r in results if r.get("success")]),
        "results": results,
    }


@mcp.tool()
async def control_room_lights(
    room: str, action: str = "toggle", brightness: int | None = None
) -> dict[str, Any]:
    """
    Control all lights in a room.

    Args:
        room: Room name
        action: "on", "off", or "toggle"
        brightness: Optional brightness for dimmable lights

    Returns:
        Result of the control operation
    """
    return await control_light(room=room, action=action, brightness=brightness)


# === Device Status ===


@mcp.tool()
async def get_device_status(device_uuid: str) -> dict[str, Any]:
    """
    Get the current status of a specific device.

    Args:
        device_uuid: The UUID of the device

    Returns:
        Current device status and states
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    if device_uuid not in ctx.devices:
        return {"error": f"Device {device_uuid} not found"}

    device = ctx.devices[device_uuid]

    # Get current states
    states = {}
    if device.states:
        for state_name, state_uuid in device.states.items():
            try:
                # Get state value from Loxone
                value = await ctx.loxone.get_state(state_uuid)
                states[state_name] = value
            except Exception as e:
                states[state_name] = f"Error: {e!s}"

    return {
        "uuid": device.uuid,
        "name": device.name,
        "type": device.type,
        "room": device.room,
        "category": device.category,
        "states": states,
    }


@mcp.tool()
async def get_all_devices() -> list[dict[str, Any]]:
    """
    Get a list of all available devices grouped by room.

    Returns:
        All devices organized by room
    """
    ctx: ServerContext = _context
    if not ctx:
        return []

    # Group devices by room
    rooms = {}
    for device in ctx.devices.values():
        room_name = device.room
        if room_name not in rooms:
            rooms[room_name] = []

        rooms[room_name].append(
            {
                "uuid": device.uuid,
                "name": device.name,
                "type": device.type,
                "category": device.category,
            }
        )

    # Sort devices within each room
    for room_devices in rooms.values():
        room_devices.sort(key=lambda x: x["name"])

    return [{"room": room, "devices": devices} for room, devices in sorted(rooms.items())]


# === MCP Prompts ===


@mcp.prompt("room-status")
async def prompt_room_status(room_name: str) -> str:
    """
    Check the status of all devices in a specific room.

    Args:
        room_name: Name of the room to check
    """
    return f"""Please check the status of all devices in the {room_name}.

List all:
1. Lights - are they on/off?
2. Blinds/Rolladen - what position are they in?
3. Any other devices in the room

Provide a clear summary of the current state of the room."""


@mcp.prompt("movie-mode")
async def prompt_movie_mode(room_name: str, light_level: int = 20) -> str:
    """
    Set up a room for watching movies (dim lights, close blinds).

    Args:
        room_name: Room to set up for movie watching
        light_level: Desired light level (0-100, default 20)
    """
    return f"""Please set up the {room_name} for watching a movie:

1. Close all blinds/rolladen completely
2. Dim all lights to {light_level}%
3. Confirm when the room is ready

Create the perfect movie atmosphere!"""


@mcp.prompt("goodnight")
async def prompt_goodnight(rooms: str = "all") -> str:
    """
    Turn off all lights and close blinds in specified rooms or whole house.

    Args:
        rooms: Comma-separated list of rooms, or 'all' for whole house
    """
    if rooms.lower() == "all":
        return """Goodnight routine for the entire house:

1. Turn off ALL lights in every room
2. Close ALL blinds/rolladen completely
3. Report which rooms were affected

Make sure the house is ready for the night."""
    else:
        return f"""Goodnight routine for specific rooms: {rooms}

1. Turn off all lights in these rooms
2. Close all blinds/rolladen in these rooms
3. Leave other rooms unchanged

Prepare these rooms for the night."""


@mcp.prompt("morning-routine")
async def prompt_morning_routine(rooms: str, blind_position: int = 80) -> str:
    """
    Open blinds and turn on lights for the morning.

    Args:
        rooms: Comma-separated list of rooms to prepare for morning
        blind_position: How far to open blinds (0-100, default 80)
    """
    return f"""Good morning! Please prepare these rooms for the day: {rooms}

1. Open all blinds/rolladen to {blind_position}%
2. Turn on all lights
3. Report the status after completion

Let's brighten up the home for a new day!"""


@mcp.prompt("energy-save")
async def prompt_energy_save(exclude_rooms: str = "") -> str:
    """
    Turn off all unnecessary devices to save energy.

    Args:
        exclude_rooms: Comma-separated list of rooms to exclude (optional)
    """
    if exclude_rooms:
        return f"""Activate energy saving mode:

1. Turn off all lights EXCEPT in these rooms: {exclude_rooms}
2. Adjust all blinds to 50% (balanced position for energy efficiency)
3. Report how many devices were turned off

Help save energy while keeping essential rooms lit."""
    else:
        return """Activate energy saving mode:

1. Turn off ALL lights in the entire house
2. Set all blinds to 50% (balanced position for energy efficiency)
3. Report total energy savings actions taken

Maximize energy efficiency throughout the home."""


@mcp.prompt("leaving-home")
async def prompt_leaving_home(duration: str = "short") -> str:
    """
    Prepare the house for when you're leaving.

    Args:
        duration: How long you'll be away (short/long/vacation)
    """
    if duration == "vacation":
        return """Vacation mode - Prepare house for extended absence:

1. Turn off ALL lights
2. Close ALL blinds completely for security
3. List all actions taken

Secure the home for your vacation."""
    elif duration == "long":
        return """Away mode - Prepare house for being away all day:

1. Turn off all lights
2. Set blinds to 30% (security position but allowing some light)
3. Confirm all rooms are secured

Set up the home for daytime absence."""
    else:
        return """Quick leave - Basic away settings:

1. Turn off all lights
2. Leave blinds in current position
3. Quick confirmation of lights off

Quick setup for short absence."""


# === MCP Resources ===


@mcp.resource("loxone://structure")
async def get_structure() -> str:
    """Get the complete Loxone structure file as a resource."""
    if not _context:
        return json.dumps({"error": "Not connected to Loxone"})
    return json.dumps(_context.structure, indent=2)


@mcp.resource("loxone://rooms")
async def get_rooms_resource() -> str:
    """Get all rooms as a resource."""
    rooms = await list_rooms()
    return json.dumps(rooms, indent=2)


@mcp.resource("loxone://devices")
async def get_devices_resource() -> str:
    """Get all devices as a resource."""
    devices = await get_all_devices()
    return json.dumps(devices, indent=2)


# === Entry Point ===


def run() -> None:
    """Main entry point for the MCP server."""
    # Handle command line arguments
    if len(sys.argv) > 1:
        command = sys.argv[1]

        if command == "setup":
            LoxoneSecrets.setup()
            sys.exit(0)
        elif command == "validate":
            if LoxoneSecrets.validate():
                print("✅ All credentials are configured")
                sys.exit(0)
            else:
                sys.exit(1)
        elif command == "clear":
            LoxoneSecrets.clear_all()
            sys.exit(0)
        elif command in ["--help", "-h", "help"]:
            print("Loxone MCP Server")
            print("\nCommands:")
            print("  setup     - Configure Loxone credentials")
            print("  validate  - Check if credentials are configured")
            print("  clear     - Remove stored credentials")
            print("  (none)    - Start the MCP server")
            sys.exit(0)
        else:
            print(f"Unknown command: {command}")
            print("Use --help for available commands")
            sys.exit(1)

    # Run the MCP server
    try:
        logger.info("Starting Loxone MCP Server...")
        mcp.run()
    except KeyboardInterrupt:
        logger.info("Server stopped by user")
    except Exception as e:
        logger.error(f"Server error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    run()
