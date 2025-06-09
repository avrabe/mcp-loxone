"""Loxone MCP Server - Main server implementation.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import json
import logging
import os
import sys
from collections.abc import AsyncGenerator
from contextlib import asynccontextmanager
from dataclasses import dataclass
from typing import Any

from mcp.server.fastmcp import FastMCP

from loxone_mcp.credentials import LoxoneSecrets

# Set up logging
logging.basicConfig(
    level=os.getenv("LOXONE_LOG_LEVEL", "INFO"),
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger(__name__)

# Create the MCP server instance
mcp = FastMCP("Loxone Controller")

# Multilingual mappings for better LLM support
ACTION_ALIASES = {
    # Rolladen actions German -> English
    "hoch": "up",
    "rauf": "up",
    "öffnen": "up",
    "auf": "up",
    "oeffnen": "up",
    "runter": "down",
    "zu": "down",
    "schließen": "down",
    "schliessen": "down",
    "stop": "stop",
    "stopp": "stop",
    "anhalten": "stop",
    # Light actions German -> English
    "an": "on",
    "ein": "on",
    "einschalten": "on",
    "aus": "off",
    "ausschalten": "off",
    "umschalten": "toggle",
    "wechseln": "toggle",
    "dimmen": "dim",
}

# Floor mappings for room names
FLOOR_PATTERNS = {
    "og": ["og", "obergeschoss", "obergeschoß", "upstairs", "upper floor", "1st floor"],
    "eg": ["eg", "erdgeschoss", "erdgeschoß", "ground floor", "main floor"],
    "ug": ["ug", "untergeschoss", "basement", "lower floor", "keller"],
    "dg": ["dg", "dachgeschoss", "attic", "top floor", "dachboden"],
}


def normalize_action(action: str) -> str:
    """Normalize action from German/mixed to English."""
    action_lower = action.lower().strip()
    return ACTION_ALIASES.get(action_lower, action_lower)


def find_matching_room(room_query: str, available_rooms: dict[str, str]) -> list[tuple[str, str]]:
    """Find rooms matching the query, handling floor prefixes and German/English names."""
    query_lower = room_query.lower().strip()

    # Replace umlauts for better matching
    replacements = {"ä": "ae", "ö": "oe", "ü": "ue", "ß": "ss"}
    for old, new in replacements.items():
        query_lower = query_lower.replace(old, new)

    # Check if query contains floor indicators
    floor_prefix = None
    for floor_key, variations in FLOOR_PATTERNS.items():
        for variation in variations:
            if variation in query_lower:
                floor_prefix = floor_key.upper()
                # Remove the floor indicator from query for room matching
                for v in variations:
                    query_lower = query_lower.replace(v, "").strip()
                break
        if floor_prefix:
            break

    matching_rooms = []

    # If we have a floor prefix, prioritize rooms on that floor
    if floor_prefix:
        # First, try exact floor matches
        for uuid, name in available_rooms.items():
            if name.startswith(floor_prefix + " ") and (
                not query_lower or query_lower in name.lower()
            ):
                matching_rooms.append((uuid, name))

        # If query is just the floor (e.g., "OG"), return all rooms on that floor
        if not query_lower and matching_rooms:
            return matching_rooms

    # Standard room matching
    if not matching_rooms:
        for uuid, name in available_rooms.items():
            name_lower = name.lower()
            # Replace umlauts in room name too
            for old, new in replacements.items():
                name_lower = name_lower.replace(old, new)

            if query_lower in name_lower:
                matching_rooms.append((uuid, name))

    return matching_rooms


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
    Ruft alle Geräte in einem bestimmten Raum ab.

    Args:
        room: Room name (partial match supported) / Raumname (Teilübereinstimmung)
              Examples: "Wohnzimmer", "OG Büro", "kitchen", "upstairs office"
        device_type: Optional filter by device type / Optionaler Gerätetyp-Filter
                    Examples: "Light", "Jalousie", "Dimmer", "Switch"

    Returns:
        List of devices with their properties / Liste der Geräte mit Eigenschaften

    Examples:
        - get_room_devices("OG Büro") - All devices in upstairs office
        - get_room_devices("Wohnzimmer", "Light") - Only lights in living room
        - get_room_devices("OG") - All devices on upper floor
    """
    ctx: ServerContext = _context
    if not ctx:
        return []

    # Find matching room(s) with multilingual support
    matching_rooms = find_matching_room(room, ctx.rooms)

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
    Control rolladen/blinds in a room. Supports German and English commands.
    Steuert Rolladen/Jalousien in einem Raum. Unterstützt deutsche und englische Befehle.

    Args:
        room: Room name / Raumname
              Examples: "Wohnzimmer", "OG Büro", "living room", "upstairs office"
              Special: "OG" (all upstairs), "EG" (all ground floor)
        device: Specific device name (optional) / Spezifisches Gerät (optional)
        action: Control action / Steuerungsaktion
                - "up"/"hoch"/"auf"/"öffnen" - Open blinds / Rolladen öffnen
                - "down"/"runter"/"zu"/"schließen" - Close blinds / Rolladen schließen
                - "stop"/"stopp"/"anhalten" - Stop movement / Bewegung stoppen
                - "position" - Set specific position / Bestimmte Position
        position: Position 0-100 (only with action="position") / Position 0-100

    Returns:
        Result of the control operation / Ergebnis der Steuerung

    Examples:
        - control_rolladen("OG Büro", action="runter") - Close office blinds upstairs
        - control_rolladen("Wohnzimmer", action="auf") - Open living room blinds
        - control_rolladen("OG", action="down") - Close all upstairs blinds
        - control_rolladen("kitchen", action="position", position=50) - Set to 50%
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

            # Normalize action from German/English to standard
            normalized_action = normalize_action(action)

            # Map action to Loxone command
            if normalized_action == "position" and position is not None:
                command = f"moveToPosition/{position}"
            elif normalized_action == "up":
                command = "FullUp"
            elif normalized_action == "down":
                command = "FullDown"
            elif normalized_action == "stop":
                command = "Stop"
            else:
                results.append(
                    {
                        "device": dev["name"],
                        "error": f"Invalid action: {action} (normalized: {normalized_action})",
                    }
                )
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
    Steuert alle Rolladen in einem Raum mit einem einfachen Befehl.

    Args:
        room: Room name / Raumname (e.g., "OG Büro", "Wohnzimmer", "OG" for all upstairs)
        action: "up"/"hoch", "down"/"runter", "stop"/"stopp"

    Returns:
        Result of the control operation / Ergebnis der Steuerung
    """
    return await control_rolladen(room=room, action=action)


# === Light Control ===


@mcp.tool()
async def control_light(
    room: str, device: str | None = None, action: str = "toggle", brightness: int | None = None
) -> dict[str, Any]:
    """
    Control lights in a room. Supports German and English commands.
    Steuert Lichter in einem Raum. Unterstützt deutsche und englische Befehle.

    Args:
        room: Room name / Raumname
              Examples: "Wohnzimmer", "OG Bad", "kitchen", "upstairs bathroom"
              Special: "OG" (all upstairs), "EG" (all ground floor)
        device: Specific device name (optional) / Spezifisches Gerät (optional)
        action: Control action / Steuerungsaktion
                - "on"/"an"/"ein"/"einschalten" - Turn on / Einschalten
                - "off"/"aus"/"ausschalten" - Turn off / Ausschalten
                - "toggle"/"umschalten"/"wechseln" - Toggle state / Umschalten
                - "dim"/"dimmen" - Dim to specific level / Dimmen
        brightness: Brightness 0-100 (only with dim) / Helligkeit 0-100

    Returns:
        Result of the control operation / Ergebnis der Steuerung

    Examples:
        - control_light("OG Büro", action="an") - Turn on office lights upstairs
        - control_light("Wohnzimmer", action="aus") - Turn off living room lights
        - control_light("OG", action="off") - Turn off all upstairs lights
        - control_light("Bad", action="dimmen", brightness=30) - Dim bathroom to 30%
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

            # Normalize action from German/English to standard
            normalized_action = normalize_action(action)

            # Map action to Loxone command
            if normalized_action == "on":
                command = "On"
            elif normalized_action == "off":
                command = "Off"
            elif normalized_action == "toggle":
                command = "Pulse"
            elif normalized_action == "dim" and brightness is not None:
                command = str(brightness)  # Direct value for dimmers
            else:
                results.append(
                    {
                        "device": dev["name"],
                        "error": f"Invalid action: {action} (normalized: {normalized_action})",
                    }
                )
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
    Steuert alle Lichter in einem Raum.

    Args:
        room: Room name / Raumname (e.g., "OG Bad", "Wohnzimmer", "OG" for all upstairs)
        action: "on"/"an", "off"/"aus", "toggle"/"umschalten"
        brightness: Optional brightness / Optionale Helligkeit (0-100)

    Returns:
        Result of the control operation / Ergebnis der Steuerung
    """
    return await control_light(room=room, action=action, brightness=brightness)


# === Helper Tools for LLMs ===


@mcp.tool()
async def get_rooms_by_floor() -> dict[str, list[str]]:
    """
    Get all rooms organized by floor/level.
    Gibt alle Räume organisiert nach Stockwerk zurück.

    Returns rooms grouped by:
    - OG (Obergeschoss/Upstairs)
    - EG (Erdgeschoss/Ground Floor)
    - UG (Untergeschoss/Basement)
    - Other (Räume ohne Stockwerk-Präfix)

    Useful for LLMs to understand the house structure.
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    rooms_by_floor = {"OG": [], "EG": [], "UG": [], "DG": [], "Other": []}

    for _uuid, name in _context.rooms.items():
        if name.startswith("OG "):
            rooms_by_floor["OG"].append(name)
        elif name.startswith("EG "):
            rooms_by_floor["EG"].append(name)
        elif name.startswith("UG "):
            rooms_by_floor["UG"].append(name)
        elif name.startswith("DG "):
            rooms_by_floor["DG"].append(name)
        else:
            rooms_by_floor["Other"].append(name)

    # Remove empty floors
    return {floor: rooms for floor, rooms in rooms_by_floor.items() if rooms}


@mcp.tool()
async def translate_command(command: str) -> dict[str, Any]:
    """
    Helper tool to understand German/English/mixed commands.
    Hilfs-Tool zum Verstehen von deutschen/englischen/gemischten Befehlen.

    Args:
        command: Natural language command in any language
                 Natürlichsprachlicher Befehl in beliebiger Sprache

    Returns:
        Parsed intent with suggested tool and parameters

    Examples:
        - "Rolladen OG Büro runter" → control_rolladen(room="OG Büro", action="down")
        - "Turn off all lights upstairs" → control_light(room="OG", action="off")
        - "Dimme Wohnzimmer auf 30%" → control_light(room="Wohnzimmer", action="dim", brightness=30)
    """
    command_lower = command.lower()

    # Detect device type
    device_type = None
    if any(word in command_lower for word in ["rolladen", "jalousien", "blind", "shutter"]):
        device_type = "rolladen"
    elif any(word in command_lower for word in ["licht", "light", "lampe", "beleuchtung"]):
        device_type = "light"

    # Detect action
    action = None
    brightness = None

    # Check for brightness/position values
    import re

    brightness_match = re.search(r"(\d+)\s*(%|prozent)?", command_lower)
    if brightness_match:
        brightness = int(brightness_match.group(1))

    # Detect action based on keywords
    for german, english in ACTION_ALIASES.items():
        if german in command_lower:
            action = english
            break

    # If no action found, check English keywords
    if not action:
        if any(word in command_lower for word in ["up", "open", "öffnen"]):
            action = "up"
        elif any(word in command_lower for word in ["down", "close", "schließen"]):
            action = "down"
        elif any(word in command_lower for word in ["on", "turn on"]):
            action = "on"
        elif any(word in command_lower for word in ["off", "turn off"]):
            action = "off"
        elif any(word in command_lower for word in ["dim", "dimmen"]) and brightness is not None:
            action = "dim"

    # Try to extract room
    room = None
    if _context:
        # Check all known rooms
        for _uuid, room_name in _context.rooms.items():
            if room_name.lower() in command_lower:
                room = room_name
                break

        # Check for floor indicators
        if not room:
            for floor_key, variations in FLOOR_PATTERNS.items():
                for variation in variations:
                    if variation in command_lower:
                        room = floor_key.upper()
                        break

    # Build response
    result = {
        "original_command": command,
        "detected_language": "mixed" if any(c in command_lower for c in "äöüß") else "unknown",
        "device_type": device_type,
        "action": action,
        "room": room,
        "brightness": brightness,
    }

    # Suggest tool and parameters
    if device_type == "rolladen" and room and action:
        result["suggested_tool"] = "control_rolladen"
        result["suggested_params"] = {"room": room, "action": action}
        if action == "position" and brightness:
            result["suggested_params"]["position"] = brightness
    elif device_type == "light" and room and action:
        result["suggested_tool"] = "control_light"
        result["suggested_params"] = {"room": room, "action": action}
        if action == "dim" and brightness:
            result["suggested_params"]["brightness"] = brightness

    return result


# === Sensor Monitoring Tools ===


@mcp.tool()
async def get_temperature_overview(room: str | None = None) -> dict[str, Any]:
    """
    Get temperature readings from all sensors or specific room.
    Ruft Temperaturwerte aller Sensoren oder eines bestimmten Raums ab.

    Args:
        room: Optional room name filter / Optionaler Raumname-Filter
              Examples: "Wohnzimmer", "OG Büro", "kitchen", "upstairs office"
              Special: "OG" (all upstairs), "EG" (all ground floor)

    Returns:
        Temperature readings organized by room / Temperaturwerte nach Raum organisiert

    Examples:
        - get_temperature_overview() - All temperature sensors
        - get_temperature_overview("OG Büro") - Office temperatures only
        - get_temperature_overview("OG") - All upstairs temperatures
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Find temperature sensors
    temp_sensors = []
    for device in ctx.devices.values():
        if device.type == "InfoOnlyAnalog" and "temperatur" in device.name.lower():
            temp_sensors.append(device)

    if not temp_sensors:
        return {"error": "No temperature sensors found"}

    # Filter by room if specified
    if room:
        matching_rooms = find_matching_room(room, ctx.rooms)
        if not matching_rooms:
            return {"error": f"No room found matching '{room}'"}

        room_uuids = {uuid for uuid, _ in matching_rooms}
        temp_sensors = [s for s in temp_sensors if s.room_uuid in room_uuids]

    # Read temperature values
    temperatures = {}
    for sensor in temp_sensors:
        try:
            # Get temperature value
            value = await ctx.loxone.send_command(f"jdev/sps/io/{sensor.uuid}/state")

            # Parse temperature (remove °C suffix if present)
            temp_str = str(value).replace("°", "").replace("C", "").strip()
            try:
                temp_value = float(temp_str)
            except ValueError:
                temp_value = value

            # Organize by room
            if sensor.room not in temperatures:
                temperatures[sensor.room] = []

            temperatures[sensor.room].append(
                {"name": sensor.name, "value": temp_value, "unit": "°C", "uuid": sensor.uuid}
            )

        except Exception as e:
            if sensor.room not in temperatures:
                temperatures[sensor.room] = []
            temperatures[sensor.room].append(
                {"name": sensor.name, "error": str(e), "uuid": sensor.uuid}
            )

    return {"room_filter": room, "total_sensors": len(temp_sensors), "temperatures": temperatures}


@mcp.tool()
async def get_humidity_overview(room: str | None = None) -> dict[str, Any]:
    """
    Get humidity readings from all sensors or specific room.
    Ruft Luftfeuchtigkeitswerte aller Sensoren oder eines bestimmten Raums ab.

    Args:
        room: Optional room name filter / Optionaler Raumname-Filter
              Examples: "Bad", "OG Büro", "bathroom", "upstairs office"
              Special: "OG" (all upstairs), "EG" (all ground floor)

    Returns:
        Humidity readings organized by room / Luftfeuchtigkeitswerte nach Raum organisiert

    Examples:
        - get_humidity_overview() - All humidity sensors
        - get_humidity_overview("Bad") - Bathroom humidity only
        - get_humidity_overview("OG") - All upstairs humidity
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Find humidity sensors
    humidity_sensors = []
    for device in ctx.devices.values():
        if device.type == "InfoOnlyAnalog" and any(
            word in device.name.lower() for word in ["humid", "luftfeuchte", "feucht"]
        ):
            humidity_sensors.append(device)

    if not humidity_sensors:
        return {"error": "No humidity sensors found"}

    # Filter by room if specified
    if room:
        matching_rooms = find_matching_room(room, ctx.rooms)
        if not matching_rooms:
            return {"error": f"No room found matching '{room}'"}

        room_uuids = {uuid for uuid, _ in matching_rooms}
        humidity_sensors = [s for s in humidity_sensors if s.room_uuid in room_uuids]

    # Read humidity values
    humidity_data = {}
    for sensor in humidity_sensors:
        try:
            # Get humidity value
            value = await ctx.loxone.send_command(f"jdev/sps/io/{sensor.uuid}/state")

            # Parse humidity (remove % suffix if present)
            humidity_str = str(value).replace("%", "").strip()
            try:
                humidity_value = float(humidity_str)
            except ValueError:
                humidity_value = value

            # Organize by room
            if sensor.room not in humidity_data:
                humidity_data[sensor.room] = []

            humidity_data[sensor.room].append(
                {"name": sensor.name, "value": humidity_value, "unit": "%", "uuid": sensor.uuid}
            )

        except Exception as e:
            if sensor.room not in humidity_data:
                humidity_data[sensor.room] = []
            humidity_data[sensor.room].append(
                {"name": sensor.name, "error": str(e), "uuid": sensor.uuid}
            )

    return {"room_filter": room, "total_sensors": len(humidity_sensors), "humidity": humidity_data}


@mcp.tool()
async def get_security_status() -> dict[str, Any]:
    """
    Check alarm systems, window states, and smoke detectors.
    Überprüft Alarmanlagen, Fensterzustände und Rauchmelder.

    Returns:
        Complete security system status / Vollständiger Sicherheitssystem-Status

    Shows:
        - Alarm system status (armed/disarmed)
        - Window/door monitoring
        - Smoke detector status
        - Overall security summary
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    security_status = {
        "alarms": [],
        "smoke_detectors": [],
        "window_monitor": None,
        "overall_status": "unknown",
    }

    # Check alarm systems
    for device in ctx.devices.values():
        if device.type == "Alarm":
            try:
                status = await ctx.loxone.send_command(f"jdev/sps/io/{device.uuid}/state")
                security_status["alarms"].append(
                    {"name": device.name, "status": status, "uuid": device.uuid}
                )
            except Exception as e:
                security_status["alarms"].append(
                    {"name": device.name, "error": str(e), "uuid": device.uuid}
                )

    # Check smoke detectors
    for device in ctx.devices.values():
        if device.type == "SmokeAlarm":
            try:
                status = await ctx.loxone.send_command(f"jdev/sps/io/{device.uuid}/state")
                security_status["smoke_detectors"].append(
                    {
                        "name": device.name,
                        "status": status,
                        "status_text": "Normal" if status == 0 else f"Alert Level {status}",
                        "uuid": device.uuid,
                    }
                )
            except Exception as e:
                security_status["smoke_detectors"].append(
                    {"name": device.name, "error": str(e), "uuid": device.uuid}
                )

    # Check window monitor
    for device in ctx.devices.values():
        if device.type == "WindowMonitor":
            try:
                status = await ctx.loxone.send_command(f"jdev/sps/io/{device.uuid}/state")
                security_status["window_monitor"] = {
                    "name": device.name,
                    "open_windows": status,
                    "status_text": "All closed" if status == 0 else f"{status} windows/doors open",
                    "uuid": device.uuid,
                }
            except Exception as e:
                security_status["window_monitor"] = {
                    "name": device.name,
                    "error": str(e),
                    "uuid": device.uuid,
                }
            break

    # Determine overall status
    has_errors = any(
        "error" in item for item in security_status["alarms"] + security_status["smoke_detectors"]
    )
    if security_status["window_monitor"] and "error" in security_status["window_monitor"]:
        has_errors = True

    if has_errors:
        security_status["overall_status"] = "error"
    else:
        # Check for any alerts
        smoke_alerts = any(
            item.get("status", 0) != 0 for item in security_status["smoke_detectors"]
        )
        open_windows = (
            security_status["window_monitor"]
            and float(security_status["window_monitor"].get("open_windows", 0)) > 0
        )

        if smoke_alerts:
            security_status["overall_status"] = "fire_alert"
        elif open_windows:
            security_status["overall_status"] = "windows_open"
        else:
            security_status["overall_status"] = "secure"

    return security_status


@mcp.tool()
async def get_climate_summary() -> dict[str, Any]:
    """
    Comprehensive climate overview: temperature, humidity, and air quality by room.
    Umfassende Klimaübersicht: Temperatur, Luftfeuchtigkeit und Luftqualität nach Raum.

    Returns:
        Complete climate data for all rooms / Vollständige Klimadaten für alle Räume

    Includes:
        - Temperature readings per room
        - Humidity levels per room
        - Air quality indicators
        - Comfort level analysis
        - Room-by-room summary
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Get temperature and humidity data
    temp_data = await get_temperature_overview()
    humidity_data = await get_humidity_overview()

    # Combine data by room
    climate_summary = {}

    # Process temperature data
    if "temperatures" in temp_data:
        for room, sensors in temp_data["temperatures"].items():
            if room not in climate_summary:
                climate_summary[room] = {"temperature": [], "humidity": [], "air_quality": None}
            climate_summary[room]["temperature"] = sensors

    # Process humidity data
    if "humidity" in humidity_data:
        for room, sensors in humidity_data["humidity"].items():
            if room not in climate_summary:
                climate_summary[room] = {"temperature": [], "humidity": [], "air_quality": None}
            climate_summary[room]["humidity"] = sensors

    # Add comfort analysis
    for room, data in climate_summary.items():
        comfort_analysis = {"status": "unknown", "issues": []}

        # Check temperature comfort (18-24°C ideal range)
        temp_values = [
            s.get("value") for s in data["temperature"] if isinstance(s.get("value"), int | float)
        ]
        if temp_values:
            avg_temp = sum(temp_values) / len(temp_values)
            if avg_temp < 18:
                comfort_analysis["issues"].append("Too cold")
            elif avg_temp > 24:
                comfort_analysis["issues"].append("Too warm")

        # Check humidity comfort (40-60% ideal range)
        humidity_values = [
            s.get("value") for s in data["humidity"] if isinstance(s.get("value"), int | float)
        ]
        if humidity_values:
            avg_humidity = sum(humidity_values) / len(humidity_values)
            if avg_humidity < 40:
                comfort_analysis["issues"].append("Air too dry")
            elif avg_humidity > 60:
                comfort_analysis["issues"].append("Air too humid")

        comfort_analysis["status"] = (
            "comfortable" if not comfort_analysis["issues"] else "needs_attention"
        )
        climate_summary[room]["comfort"] = comfort_analysis

    return {
        "timestamp": "now",
        "total_rooms": len(climate_summary),
        "rooms": climate_summary,
        "summary": {
            "comfortable_rooms": len(
                [r for r in climate_summary.values() if r["comfort"]["status"] == "comfortable"]
            ),
            "rooms_needing_attention": len(
                [r for r in climate_summary.values() if r["comfort"]["status"] == "needs_attention"]
            ),
        },
    }


# === Weather Monitoring Tools ===


@mcp.tool()
async def get_weather_overview() -> dict[str, Any]:
    """
    Get comprehensive weather overview including outdoor conditions and light levels.
    Ruft umfassende Wetterübersicht einschließlich Außenbedingungen und Lichtstärke ab.

    Returns:
        Weather data including outdoor temperature, humidity, and brightness levels
        Wetterdaten einschließlich Außentemperatur, Luftfeuchtigkeit und Helligkeit

    Includes:
        - Outdoor temperature (current and average)
        - Outdoor humidity levels
        - Brightness/light sensors across the property
        - Weather summary and comfort analysis
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    weather_data = {
        "outdoor_temperature": {"current": None, "average": None},
        "outdoor_humidity": None,
        "brightness_sensors": [],
        "weather_summary": {},
    }

    # Get outdoor temperature and humidity from room controllers
    room_controllers = [
        device for device in ctx.devices.values() if device.type == "IRoomControllerV2"
    ]

    if room_controllers:
        controller = room_controllers[0]  # Use first controller
        states = controller.states or {}

        # Get outdoor temperature
        if "actualOutdoorTemp" in states:
            try:
                temp_uuid = states["actualOutdoorTemp"]
                temp_value = await ctx.loxone.send_command(f"jdev/sps/state/{temp_uuid}")
                weather_data["outdoor_temperature"]["current"] = float(temp_value)
            except Exception as e:
                weather_data["outdoor_temperature"]["current"] = f"Error: {e}"

        # Get average outdoor temperature
        if "averageOutdoorTemp" in states:
            try:
                avg_temp_uuid = states["averageOutdoorTemp"]
                avg_temp_value = await ctx.loxone.send_command(f"jdev/sps/state/{avg_temp_uuid}")
                weather_data["outdoor_temperature"]["average"] = float(avg_temp_value)
            except Exception as e:
                weather_data["outdoor_temperature"]["average"] = f"Error: {e}"

        # Get outdoor humidity
        if "humidityActual" in states:
            try:
                humidity_uuid = states["humidityActual"]
                humidity_value = await ctx.loxone.send_command(f"jdev/sps/state/{humidity_uuid}")
                weather_data["outdoor_humidity"] = float(humidity_value)
            except Exception as e:
                weather_data["outdoor_humidity"] = f"Error: {e}"

    # Get brightness sensors
    brightness_sensors = [
        device
        for device in ctx.devices.values()
        if device.type == "InfoOnlyAnalog" and "helligkeit" in device.name.lower()
    ]

    for sensor in brightness_sensors:
        try:
            value = await ctx.loxone.send_command(f"jdev/sps/io/{sensor.uuid}/state")
            # Parse brightness value (remove Lx suffix if present)
            brightness_str = str(value).replace("Lx", "").strip()
            try:
                brightness_value = float(brightness_str)
            except ValueError:
                brightness_value = value

            weather_data["brightness_sensors"].append(
                {
                    "name": sensor.name,
                    "room": sensor.room,
                    "value": brightness_value,
                    "unit": "Lx",
                    "uuid": sensor.uuid,
                }
            )
        except Exception as e:
            weather_data["brightness_sensors"].append(
                {"name": sensor.name, "room": sensor.room, "error": str(e), "uuid": sensor.uuid}
            )

    # Generate weather summary
    current_temp = weather_data["outdoor_temperature"]["current"]
    humidity = weather_data["outdoor_humidity"]

    summary = {"conditions": [], "comfort_level": "unknown"}

    if isinstance(current_temp, int | float):
        if current_temp < 0:
            summary["conditions"].append("Freezing conditions")
        elif current_temp < 10:
            summary["conditions"].append("Cold weather")
        elif current_temp < 20:
            summary["conditions"].append("Cool weather")
        elif current_temp < 25:
            summary["conditions"].append("Pleasant weather")
        else:
            summary["conditions"].append("Warm weather")

    if isinstance(humidity, int | float):
        if humidity < 30:
            summary["conditions"].append("Very dry air")
        elif humidity > 70:
            summary["conditions"].append("High humidity")
        else:
            summary["conditions"].append("Comfortable humidity")

    # Analyze brightness levels
    valid_brightness = [
        s["value"]
        for s in weather_data["brightness_sensors"]
        if isinstance(s.get("value"), int | float)
    ]
    if valid_brightness:
        avg_brightness = sum(valid_brightness) / len(valid_brightness)
        if avg_brightness < 10:
            summary["conditions"].append("Dark/nighttime")
        elif avg_brightness < 100:
            summary["conditions"].append("Low light conditions")
        elif avg_brightness < 1000:
            summary["conditions"].append("Moderate lighting")
        else:
            summary["conditions"].append("Bright conditions")

    # Overall comfort assessment
    temp_comfortable = isinstance(current_temp, int | float) and 15 <= current_temp <= 25
    humidity_comfortable = isinstance(humidity, int | float) and 40 <= humidity <= 60

    if temp_comfortable and humidity_comfortable:
        summary["comfort_level"] = "very_comfortable"
    elif temp_comfortable or humidity_comfortable:
        summary["comfort_level"] = "comfortable"
    else:
        summary["comfort_level"] = "challenging"

    weather_data["weather_summary"] = summary

    return weather_data


@mcp.tool()
async def get_outdoor_temperature() -> dict[str, Any]:
    """
    Get current and average outdoor temperature readings.
    Ruft aktuelle und durchschnittliche Außentemperaturwerte ab.

    Returns:
        Outdoor temperature data with current and average values
        Außentemperaturdaten mit aktuellen und durchschnittlichen Werten

    Examples:
        - get_outdoor_temperature() - Current outdoor conditions
        Shows both real-time and averaged temperature readings
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    temperature_data = {
        "current_temperature": None,
        "average_temperature": None,
        "unit": "°C",
        "timestamp": "now",
    }

    # Find room controller with outdoor temperature
    room_controllers = [
        device for device in ctx.devices.values() if device.type == "IRoomControllerV2"
    ]

    if not room_controllers:
        return {"error": "No room controllers found for outdoor temperature"}

    controller = room_controllers[0]
    states = controller.states or {}

    # Get current outdoor temperature
    if "actualOutdoorTemp" in states:
        try:
            temp_uuid = states["actualOutdoorTemp"]
            temp_value = await ctx.loxone.send_command(f"jdev/sps/state/{temp_uuid}")
            temperature_data["current_temperature"] = float(temp_value)
        except Exception as e:
            temperature_data["current_temperature"] = f"Error: {e}"

    # Get average outdoor temperature
    if "averageOutdoorTemp" in states:
        try:
            avg_temp_uuid = states["averageOutdoorTemp"]
            avg_temp_value = await ctx.loxone.send_command(f"jdev/sps/state/{avg_temp_uuid}")
            temperature_data["average_temperature"] = float(avg_temp_value)
        except Exception as e:
            temperature_data["average_temperature"] = f"Error: {e}"

    return temperature_data


@mcp.tool()
async def get_brightness_levels() -> dict[str, Any]:
    """
    Get current brightness/light levels from all sensors around the property.
    Ruft aktuelle Helligkeit/Lichtstärke von allen Sensoren rund um das Grundstück ab.

    Returns:
        Brightness readings from all light sensors organized by location
        Helligkeitswerte aller Lichtsensoren organisiert nach Standort

    Useful for:
        - Determining if it's day or night
        - Assessing natural light availability
        - Automating lighting decisions
        - Understanding property lighting conditions
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Find brightness sensors
    brightness_sensors = [
        device
        for device in ctx.devices.values()
        if device.type == "InfoOnlyAnalog" and "helligkeit" in device.name.lower()
    ]

    if not brightness_sensors:
        return {"error": "No brightness sensors found"}

    brightness_data = {
        "sensors": [],
        "summary": {
            "total_sensors": len(brightness_sensors),
            "average_brightness": 0,
            "lighting_condition": "unknown",
        },
    }

    valid_readings = []

    for sensor in brightness_sensors:
        try:
            value = await ctx.loxone.send_command(f"jdev/sps/io/{sensor.uuid}/state")

            # Parse brightness value
            brightness_str = str(value).replace("Lx", "").strip()
            try:
                brightness_value = float(brightness_str)
                valid_readings.append(brightness_value)
            except ValueError:
                brightness_value = value

            brightness_data["sensors"].append(
                {
                    "name": sensor.name,
                    "room": sensor.room,
                    "value": brightness_value,
                    "unit": "Lx",
                    "uuid": sensor.uuid,
                }
            )

        except Exception as e:
            brightness_data["sensors"].append(
                {"name": sensor.name, "room": sensor.room, "error": str(e), "uuid": sensor.uuid}
            )

    # Calculate summary
    if valid_readings:
        avg_brightness = sum(valid_readings) / len(valid_readings)
        brightness_data["summary"]["average_brightness"] = round(avg_brightness, 1)

        # Determine lighting condition
        if avg_brightness < 10:
            condition = "dark"
        elif avg_brightness < 100:
            condition = "dim"
        elif avg_brightness < 1000:
            condition = "moderate"
        elif avg_brightness < 10000:
            condition = "bright"
        else:
            condition = "very_bright"

        brightness_data["summary"]["lighting_condition"] = condition

    return brightness_data


@mcp.tool()
async def get_environmental_summary() -> dict[str, Any]:
    """
    Complete environmental overview combining weather, climate, and lighting data.
    Vollständige Umgebungsübersicht mit Wetter-, Klima- und Beleuchtungsdaten.

    Returns:
        Comprehensive environmental data for both indoor and outdoor conditions
        Umfassende Umgebungsdaten für Innen- und Außenbedingungen

    Includes:
        - Outdoor weather conditions
        - Indoor climate summary
        - Lighting conditions
        - Environmental comfort analysis
        - Recommendations for comfort optimization
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Get all environmental data
    weather_data = await get_weather_overview()
    climate_data = await get_climate_summary()
    brightness_data = await get_brightness_levels()

    # Combine into comprehensive summary
    environmental_summary = {
        "timestamp": "now",
        "outdoor_conditions": {
            "temperature": weather_data.get("outdoor_temperature", {}),
            "humidity": weather_data.get("outdoor_humidity"),
            "weather_summary": weather_data.get("weather_summary", {}),
        },
        "indoor_conditions": {
            "total_rooms": climate_data.get("total_rooms", 0),
            "comfortable_rooms": climate_data.get("summary", {}).get("comfortable_rooms", 0),
            "rooms_needing_attention": climate_data.get("summary", {}).get(
                "rooms_needing_attention", 0
            ),
        },
        "lighting_conditions": {
            "average_brightness": brightness_data.get("summary", {}).get("average_brightness", 0),
            "lighting_condition": brightness_data.get("summary", {}).get(
                "lighting_condition", "unknown"
            ),
            "total_sensors": brightness_data.get("summary", {}).get("total_sensors", 0),
        },
        "overall_assessment": {},
        "recommendations": [],
    }

    # Generate overall assessment
    outdoor_temp = weather_data.get("outdoor_temperature", {}).get("current")
    indoor_comfortable = climate_data.get("summary", {}).get("comfortable_rooms", 0)
    total_rooms = climate_data.get("total_rooms", 1)
    indoor_comfort_ratio = indoor_comfortable / total_rooms if total_rooms > 0 else 0

    assessment = {"comfort_score": 0, "conditions": []}

    # Outdoor comfort
    if isinstance(outdoor_temp, int | float):
        if 15 <= outdoor_temp <= 25:
            assessment["conditions"].append("Pleasant outdoor temperature")
            assessment["comfort_score"] += 3
        elif 10 <= outdoor_temp <= 30:
            assessment["conditions"].append("Acceptable outdoor temperature")
            assessment["comfort_score"] += 2
        else:
            assessment["conditions"].append("Challenging outdoor temperature")
            assessment["comfort_score"] += 1

    # Indoor comfort
    if indoor_comfort_ratio >= 0.8:
        assessment["conditions"].append("Most rooms are comfortable")
        assessment["comfort_score"] += 3
    elif indoor_comfort_ratio >= 0.5:
        assessment["conditions"].append("Some rooms need attention")
        assessment["comfort_score"] += 2
    else:
        assessment["conditions"].append("Many rooms need climate adjustment")
        assessment["comfort_score"] += 1

    # Lighting assessment
    lighting_condition = brightness_data.get("summary", {}).get("lighting_condition", "unknown")
    if lighting_condition in ["bright", "moderate"]:
        assessment["conditions"].append("Good natural lighting")
        assessment["comfort_score"] += 2
    elif lighting_condition == "dim":
        assessment["conditions"].append("Limited natural light")
        assessment["comfort_score"] += 1
    else:
        assessment["conditions"].append("Low light conditions")

    # Overall comfort level
    if assessment["comfort_score"] >= 7:
        assessment["overall_comfort"] = "excellent"
    elif assessment["comfort_score"] >= 5:
        assessment["overall_comfort"] = "good"
    elif assessment["comfort_score"] >= 3:
        assessment["overall_comfort"] = "fair"
    else:
        assessment["overall_comfort"] = "needs_improvement"

    environmental_summary["overall_assessment"] = assessment

    # Generate recommendations
    recommendations = []

    if isinstance(outdoor_temp, int | float) and outdoor_temp < 10:
        recommendations.append("Consider warming indoor spaces due to cold outdoor conditions")
    elif isinstance(outdoor_temp, int | float) and outdoor_temp > 30:
        recommendations.append("Consider cooling measures due to hot outdoor weather")

    if indoor_comfort_ratio < 0.7:
        recommendations.append("Review climate settings in uncomfortable rooms")

    if lighting_condition in ["dark", "dim"]:
        recommendations.append("Consider increasing artificial lighting")

    environmental_summary["recommendations"] = recommendations

    return environmental_summary


# === Weather Service Tools ===


@mcp.tool()
async def get_weather_service_status() -> dict[str, Any]:
    """
    Get the status of the Loxone Weather Service.
    Ruft den Status des Loxone Wetterdienstes ab.

    Returns:
        Weather service status, configuration, and available data types
        Wetterdienst-Status, Konfiguration und verfügbare Datentypen

    Examples:
        - get_weather_service_status() - Check if weather service is working
        Shows status, location, and available weather data types
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    weather_server = ctx.structure.get("weatherServer", {})

    if not weather_server:
        return {
            "status": "not_configured",
            "message": "Weather Service is not configured in this Loxone system",
            "recommendation": "Add a Weather Service block in Loxone Config",
        }

    # Get weather service states
    weather_states = weather_server.get("states", {})
    actual_uuid = weather_states.get("actual")
    forecast_uuid = weather_states.get("forecast")

    status_info = {
        "status": "configured",
        "location": ctx.structure.get("msInfo", {}).get("location", "Not set"),
        "states": {"actual_uuid": actual_uuid, "forecast_uuid": forecast_uuid},
        "field_types": {},
        "weather_types": {},
        "formats": weather_server.get("format", {}),
    }

    # Get current status values
    if actual_uuid:
        try:
            actual_status = await ctx.loxone.send_command(f"jdev/sps/state/{actual_uuid}")
            status_info["actual_status"] = actual_status

            # Interpret status codes
            if actual_status == 5:
                status_info["status"] = "error_or_inactive"
                status_info["message"] = (
                    "Weather Service appears to be inactive or has an error (status code 5)"
                )
                status_info["recommendations"] = [
                    "Check internet connection on Miniserver",
                    "Verify location is properly set in project properties",
                    "Check if Weather Service block is enabled in Loxone Config",
                    "Restart Weather Service block if needed",
                ]
        except Exception as e:
            status_info["actual_status_error"] = str(e)

    if forecast_uuid:
        try:
            forecast_status = await ctx.loxone.send_command(f"jdev/sps/state/{forecast_uuid}")
            status_info["forecast_status"] = forecast_status
        except Exception as e:
            status_info["forecast_status_error"] = str(e)

    # Add available field types (translated to English)
    field_types = weather_server.get("weatherFieldTypes", {})
    for field_id, field_info in field_types.items():
        field_name = field_info.get("name", "Unknown")
        # Translate German field names to English
        translations = {
            "Temperatur": "Temperature",
            "Taupunkt": "Dewpoint",
            "Relative Luftfeuchte": "Relative Humidity",
            "Windgeschwindigkeit": "Wind Speed",
            "Windrichtung": "Wind Direction",
            "Böen": "Wind Gusts",
            "Absolute Bestrahlungsstärke": "Absolute Solar Radiation",
            "Relative Bestrahlungsstärke": "Relative Solar Radiation",
            "Niederschlag": "Precipitation",
            "Wettertyp": "Weather Type",
            "Luftdruck": "Barometric Pressure",
            "Gefühlte Temperatur": "Perceived Temperature",
            "Solare Bestrahlungsstärke": "Solar Radiation",
            "Niederschlagswahrscheinlichkeit": "Precipitation Probability",
            "Schneeanteil": "Snow Percentage",
            "Niedrige Bewölkung": "Low Clouds",
            "Mittlere Bewölkung": "Medium Clouds",
            "Hohe Bewölkung": "High Clouds",
            "Sonnenschein": "Sunshine",
            "Feinstaubbelastung": "Fine Dust Pollution",
            "Gefahrenwarnung": "Weather Warning",
        }

        english_name = translations.get(field_name, field_name)
        status_info["field_types"][field_id] = {
            "name": english_name,
            "german_name": field_name,
            "unit": field_info.get("unit", ""),
            "format": field_info.get("format", ""),
            "analog": field_info.get("analog", True),
        }

    # Add weather type descriptions (translated)
    weather_types = weather_server.get("weatherTypeTexts", {})
    for type_id, german_desc in weather_types.items():
        # Translate weather descriptions
        translations = {
            "wolkenlos": "clear sky",
            "heiter": "fair",
            "wolkig": "cloudy",
            "stark bewölkt": "heavily cloudy",
            "bedeckt": "overcast",
            "Nebel": "fog",
            "Hochnebel": "high fog",
            "leichter Regen": "light rain",
            "Regen": "rain",
            "starker Regen": "heavy rain",
            "Nieseln": "drizzle",
            "leichter gefrierender Regen": "light freezing rain",
            "starker gefrierender Regen": "heavy freezing rain",
            "leichter Regenschauer": "light showers",
            "kräftiger Regenschauer": "heavy showers",
            "Gewitter": "thunderstorm",
            "kräftiges Gewitter": "severe thunderstorm",
            "leichter Schneefall": "light snow",
            "Schneefall": "snow",
            "starker Schneefall": "heavy snow",
            "leichter Schneeschauer": "light snow showers",
            "starker Schneeschauer": "heavy snow showers",
            "leichter Schneeregen": "light sleet",
            "Schneeregen": "sleet",
            "starker Schneeregen": "heavy sleet",
            "leichter Schneeregenschauer": "light snow/rain showers",
            "kräftiger Schneeregenschauer": "heavy snow/rain showers",
        }

        english_desc = translations.get(german_desc, german_desc)
        status_info["weather_types"][type_id] = {
            "description": english_desc,
            "german_description": german_desc,
        }

    return status_info


@mcp.tool()
async def get_weather_forecast() -> dict[str, Any]:
    """
    Get weather forecast data from the Loxone Weather Service.
    Ruft Wettervorhersagedaten vom Loxone Wetterdienst ab.

    Returns:
        Weather forecast data including temperature, precipitation, wind forecasts
        Wettervorhersagedaten einschließlich Temperatur-, Niederschlags- und Windvorhersagen

    Examples:
        - get_weather_forecast() - Get forecast for upcoming days
        Shows weather predictions and conditions
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    weather_server = ctx.structure.get("weatherServer", {})
    if not weather_server:
        return {"error": "Weather Service not configured"}

    weather_states = weather_server.get("states", {})
    forecast_uuid = weather_states.get("forecast")

    if not forecast_uuid:
        return {"error": "Weather forecast state not found"}

    forecast_data = {"service_status": "checking", "forecast_available": False, "message": ""}

    try:
        # Get forecast state value
        forecast_value = await ctx.loxone.send_command(f"jdev/sps/state/{forecast_uuid}")
        forecast_data["raw_forecast_value"] = forecast_value

        if forecast_value == 5:
            forecast_data["service_status"] = "inactive_or_error"
            forecast_data["message"] = "Weather Service appears to be inactive or has an error"
            forecast_data["recommendations"] = [
                "Check if Weather Service is properly configured in Loxone Config",
                "Verify internet connection on the Miniserver",
                "Ensure location coordinates are set in project properties",
                "Try restarting the Weather Service block",
            ]
        elif isinstance(forecast_value, int | float) and forecast_value != 5:
            forecast_data["service_status"] = "active"
            forecast_data["message"] = f"Weather Service is responding (status: {forecast_value})"

            # Note: The actual forecast data might be available through other methods
            # or require WebSocket connection for real-time updates
            forecast_data["note"] = (
                "Forecast data access may require WebSocket connection or additional configuration"
            )
        else:
            forecast_data["service_status"] = "unknown"
            forecast_data["message"] = f"Unexpected forecast value: {forecast_value}"

    except Exception as e:
        forecast_data["error"] = f"Failed to access forecast data: {e!s}"

    # Add information about weather field types available
    field_types = weather_server.get("weatherFieldTypes", {})
    if field_types:
        forecast_data["available_data_types"] = []
        for field_id, field_info in field_types.items():
            forecast_data["available_data_types"].append(
                {
                    "id": field_id,
                    "name": field_info.get("name", "Unknown"),
                    "unit": field_info.get("unit", ""),
                    "format": field_info.get("format", ""),
                }
            )

    return forecast_data


@mcp.tool()
async def get_weather_current() -> dict[str, Any]:
    """
    Get current weather conditions from the Loxone Weather Service.
    Ruft aktuelle Wetterbedingungen vom Loxone Wetterdienst ab.

    Returns:
        Current weather data including temperature, humidity, wind, precipitation
        Aktuelle Wetterdaten einschließlich Temperatur, Luftfeuchtigkeit, Wind, Niederschlag

    Examples:
        - get_weather_current() - Get current weather conditions
        Shows real-time weather measurements
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    weather_server = ctx.structure.get("weatherServer", {})
    if not weather_server:
        return {"error": "Weather Service not configured"}

    weather_states = weather_server.get("states", {})
    actual_uuid = weather_states.get("actual")

    if not actual_uuid:
        return {"error": "Weather actual state not found"}

    current_data = {"service_status": "checking", "weather_available": False, "measurements": {}}

    try:
        # Get current weather state value
        actual_value = await ctx.loxone.send_command(f"jdev/sps/state/{actual_uuid}")
        current_data["raw_actual_value"] = actual_value

        if actual_value == 5:
            current_data["service_status"] = "inactive_or_error"
            current_data["message"] = "Weather Service appears to be inactive or has an error"
            current_data["recommendations"] = [
                "Check if Weather Service is properly configured",
                "Verify internet connection",
                "Ensure location is set in project properties",
                "Check Weather Service block in Loxone Config",
            ]
        else:
            current_data["service_status"] = "responding"
            current_data["message"] = f"Weather Service is responding (status: {actual_value})"

    except Exception as e:
        current_data["error"] = f"Failed to access current weather data: {e!s}"

    # Look for weather-related InfoOnlyAnalog controls that might contain weather data
    weather_controls = []
    for device in ctx.devices.values():
        if device.type == "InfoOnlyAnalog" and any(
            keyword in device.name.lower()
            for keyword in ["rel.luftfeuchte", "humidity", "weather", "wetter", "outdoor", "außen"]
        ):
            try:
                value = await ctx.loxone.send_command(f"jdev/sps/state/{device.uuid}")
                weather_controls.append(
                    {"name": device.name, "uuid": device.uuid, "room": device.room, "value": value}
                )
            except Exception:
                pass

    if weather_controls:
        current_data["weather_controls"] = weather_controls

    # Add format information for interpreting values when they become available
    formats = weather_server.get("format", {})
    if formats:
        current_data["value_formats"] = formats

    return current_data


@mcp.tool()
async def diagnose_weather_service() -> dict[str, Any]:
    """
    Diagnose Weather Service issues and provide troubleshooting guidance.
    Diagnostiziert Probleme mit dem Wetterdienst und bietet Anleitung zur Fehlerbehebung.

    Returns:
        Diagnostic information and troubleshooting steps for Weather Service
        Diagnoseinformationen und Fehlerbehebungsschritte für den Wetterdienst

    Examples:
        - diagnose_weather_service() - Troubleshoot weather service issues
        Provides detailed diagnosis and recommendations
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    diagnosis = {
        "weather_service_configured": False,
        "location_set": False,
        "internet_connectivity": "unknown",
        "service_status": "unknown",
        "issues_found": [],
        "recommendations": [],
    }

    # Check if Weather Service is configured
    weather_server = ctx.structure.get("weatherServer", {})
    if weather_server:
        diagnosis["weather_service_configured"] = True
        diagnosis["configuration"] = {
            "states_available": list(weather_server.get("states", {}).keys()),
            "field_types_count": len(weather_server.get("weatherFieldTypes", {})),
            "weather_types_count": len(weather_server.get("weatherTypeTexts", {})),
            "formats_available": list(weather_server.get("format", {}).keys()),
        }
    else:
        diagnosis["issues_found"].append("Weather Service is not configured")
        diagnosis["recommendations"].append("Add a Weather Service block in Loxone Config")

    # Check location setting
    location = ctx.structure.get("msInfo", {}).get("location")
    if location:
        diagnosis["location_set"] = True
        diagnosis["location"] = location
    else:
        diagnosis["issues_found"].append("Location not set in project properties")
        diagnosis["recommendations"].append(
            "Set location coordinates in Loxone Config project properties"
        )

    # Test Weather Service states
    if weather_server:
        weather_states = weather_server.get("states", {})
        actual_uuid = weather_states.get("actual")
        forecast_uuid = weather_states.get("forecast")

        state_results = {}

        if actual_uuid:
            try:
                actual_value = await ctx.loxone.send_command(f"jdev/sps/state/{actual_uuid}")
                state_results["actual"] = actual_value

                if actual_value == 5:
                    diagnosis["issues_found"].append(
                        "Weather Service actual state returns error code 5"
                    )
                    diagnosis["service_status"] = "error"
                elif actual_value == 0:
                    diagnosis["service_status"] = "inactive"
                else:
                    diagnosis["service_status"] = "active"

            except Exception as e:
                state_results["actual_error"] = str(e)
                diagnosis["issues_found"].append(f"Cannot access actual weather state: {e}")

        if forecast_uuid:
            try:
                forecast_value = await ctx.loxone.send_command(f"jdev/sps/state/{forecast_uuid}")
                state_results["forecast"] = forecast_value

                if forecast_value == 5:
                    diagnosis["issues_found"].append(
                        "Weather Service forecast state returns error code 5"
                    )

            except Exception as e:
                state_results["forecast_error"] = str(e)
                diagnosis["issues_found"].append(f"Cannot access forecast state: {e}")

        diagnosis["state_test_results"] = state_results

    # Check for weather-related controls
    weather_related_controls = []
    for device in ctx.devices.values():
        if any(
            keyword in device.name.lower()
            for keyword in ["weather", "wetter", "rel.luftfeuchte", "humidity", "outdoor", "außen"]
        ):
            weather_related_controls.append(
                {"name": device.name, "type": device.type, "uuid": device.uuid, "room": device.room}
            )

    diagnosis["weather_related_controls"] = weather_related_controls

    # Add comprehensive recommendations
    if diagnosis["service_status"] == "error":
        diagnosis["recommendations"].extend(
            [
                "Check Miniserver internet connection",
                "Verify Weather Service block is enabled in Loxone Config",
                "Restart the Weather Service block",
                "Check if location coordinates are valid",
                "Verify Loxone Cloud access is working",
                "Check for any firewall blocking weather service access",
            ]
        )
    elif diagnosis["service_status"] == "inactive":
        diagnosis["recommendations"].extend(
            [
                "Enable Weather Service block in Loxone Config",
                "Check if automatic updates are enabled",
                "Verify internet connection on Miniserver",
            ]
        )

    # Priority recommendations
    if not diagnosis["weather_service_configured"]:
        diagnosis["priority"] = "high"
        diagnosis["main_issue"] = "Weather Service not configured"
    elif not diagnosis["location_set"]:
        diagnosis["priority"] = "high"
        diagnosis["main_issue"] = "Location not set - required for weather data"
    elif diagnosis["service_status"] == "error":
        diagnosis["priority"] = "medium"
        diagnosis["main_issue"] = "Weather Service configured but not working"
    else:
        diagnosis["priority"] = "low"
        diagnosis["main_issue"] = "Weather Service appears functional"

    return diagnosis


# === Lighting Presets & Moods ===


@mcp.tool()
async def get_lighting_presets(room: str | None = None) -> dict[str, Any]:
    """
    Get available lighting presets/moods for rooms with LightControllerV2.
    Ruft verfügbare Lichtvoreinstellungen/Stimmungen für Räume mit LightControllerV2 ab.

    Args:
        room: Optional room name filter / Optionaler Raumname-Filter
              Examples: "Wohnzimmer", "OG Büro", "kitchen", "upstairs office"
              Special: "OG" (all upstairs), "EG" (all ground floor)

    Returns:
        Available lighting presets organized by room / Verfügbare Lichtvoreinstellungen nach Raum

    Examples:
        - get_lighting_presets() - All rooms with lighting presets
        - get_lighting_presets("Wohnzimmer") - Living room presets only
        - get_lighting_presets("OG") - All upstairs lighting presets
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Find LightControllerV2 devices
    light_controllers = []
    for device in ctx.devices.values():
        if device.type == "LightControllerV2":
            light_controllers.append(device)

    if not light_controllers:
        return {"error": "No light controllers found"}

    # Filter by room if specified
    if room:
        matching_rooms = find_matching_room(room, ctx.rooms)
        if not matching_rooms:
            return {"error": f"No room found matching '{room}'"}

        room_uuids = {uuid for uuid, _ in matching_rooms}
        light_controllers = [lc for lc in light_controllers if lc.room_uuid in room_uuids]

    # Get lighting preset information
    lighting_presets = {}

    for controller in light_controllers:
        room_name = controller.room
        controller_info = {
            "controller_name": controller.name,
            "uuid": controller.uuid,
            "active_moods": None,
            "mood_list": None,
            "favorite_moods": None,
            "additional_moods": None,
            "circuit_names": None,
            "preset_summary": {},
        }

        # Get mood-related states
        states = controller.states or {}

        try:
            # Get active moods
            if "activeMoods" in states:
                active = await ctx.loxone.send_command(f"jdev/sps/io/{controller.uuid}/activeMoods")
                controller_info["active_moods"] = active

            # Get mood list (available presets)
            if "moodList" in states:
                moods = await ctx.loxone.send_command(f"jdev/sps/io/{controller.uuid}/moodList")
                controller_info["mood_list"] = moods

            # Get favorite moods
            if "favoriteMoods" in states:
                favorites = await ctx.loxone.send_command(
                    f"jdev/sps/io/{controller.uuid}/favoriteMoods"
                )
                controller_info["favorite_moods"] = favorites

            # Get additional moods
            if "additionalMoods" in states:
                additional = await ctx.loxone.send_command(
                    f"jdev/sps/io/{controller.uuid}/additionalMoods"
                )
                controller_info["additional_moods"] = additional

            # Get circuit names
            if "circuitNames" in states:
                circuits = await ctx.loxone.send_command(
                    f"jdev/sps/io/{controller.uuid}/circuitNames"
                )
                controller_info["circuit_names"] = circuits

            # Create preset summary
            summary = {}
            if controller_info["active_moods"] is not None:
                summary["has_active_preset"] = controller_info["active_moods"] != 0
                summary["active_preset_id"] = controller_info["active_moods"]

            if controller_info["mood_list"] is not None:
                summary["available_presets"] = controller_info["mood_list"] != 0

            if controller_info["favorite_moods"] is not None:
                summary["has_favorites"] = controller_info["favorite_moods"] != 0

            controller_info["preset_summary"] = summary

        except Exception as e:
            controller_info["error"] = str(e)

        lighting_presets[room_name] = controller_info

    return {
        "room_filter": room,
        "total_controllers": len(light_controllers),
        "lighting_presets": lighting_presets,
    }


@mcp.tool()
async def set_lighting_mood(room: str, mood_id: int) -> dict[str, Any]:
    """
    Activate a specific lighting mood/preset in a room.
    Aktiviert eine bestimmte Lichtstimmung/Voreinstellung in einem Raum.

    Args:
        room: Room name / Raumname
              Examples: "Wohnzimmer", "OG Büro", "kitchen", "upstairs office"
        mood_id: Mood/preset ID number / Stimmungs-/Voreinstellungs-ID-Nummer
                Typically 0-10, use get_lighting_presets() to see available options

    Returns:
        Result of setting the lighting mood / Ergebnis der Lichtstimmungseinstellung

    Examples:
        - set_lighting_mood("Wohnzimmer", 1) - Activate preset 1 in living room
        - set_lighting_mood("OG Büro", 2) - Activate preset 2 in upstairs office
        - set_lighting_mood("Schlafzimmer", 0) - Turn off presets (basic lighting)
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Find matching room(s)
    matching_rooms = find_matching_room(room, ctx.rooms)
    if not matching_rooms:
        return {"error": f"No room found matching '{room}'"}

    results = []

    for room_uuid, room_name in matching_rooms:
        # Find light controller for this room
        light_controller = None
        for device in ctx.devices.values():
            if device.type == "LightControllerV2" and device.room_uuid == room_uuid:
                light_controller = device
                break

        if not light_controller:
            results.append({"room": room_name, "error": "No light controller found"})
            continue

        try:
            # Set the mood using the controller
            # Try different mood setting commands
            success = False

            # Method 1: Direct mood command
            try:
                await ctx.loxone.send_command(f"jdev/sps/io/{light_controller.uuid}/mood/{mood_id}")
                success = True
            except Exception:
                pass

            # Method 2: Try as a direct value
            if not success:
                try:
                    await ctx.loxone.send_command(f"jdev/sps/io/{light_controller.uuid}/{mood_id}")
                    success = True
                except Exception:
                    pass

            # Method 3: Try pulse command for mood activation
            if not success:
                try:
                    await ctx.loxone.send_command(f"jdev/sps/io/{light_controller.uuid}/pulse")
                    success = True
                except Exception:
                    pass

            if success:
                # Verify the mood was set by checking active moods
                try:
                    active_mood = await ctx.loxone.send_command(
                        f"jdev/sps/io/{light_controller.uuid}/activeMoods"
                    )
                    results.append(
                        {
                            "room": room_name,
                            "success": True,
                            "mood_id": mood_id,
                            "active_mood": active_mood,
                            "controller": light_controller.name,
                        }
                    )
                except Exception:
                    results.append(
                        {
                            "room": room_name,
                            "success": True,
                            "mood_id": mood_id,
                            "controller": light_controller.name,
                            "note": "Mood set but verification failed",
                        }
                    )
            else:
                results.append(
                    {
                        "room": room_name,
                        "error": f"Failed to set mood {mood_id}",
                        "controller": light_controller.name,
                    }
                )

        except Exception as e:
            results.append(
                {
                    "room": room_name,
                    "error": str(e),
                    "controller": light_controller.name if light_controller else "Unknown",
                }
            )

    return {
        "target_room": room,
        "mood_id": mood_id,
        "controlled_rooms": len([r for r in results if r.get("success")]),
        "results": results,
    }


@mcp.tool()
async def get_active_lighting_moods() -> dict[str, Any]:
    """
    Show current active lighting presets across all rooms.
    Zeigt aktuell aktive Lichtvoreinstellungen in allen Räumen.

    Returns:
        Current lighting mood status for all rooms with light controllers
        Aktueller Lichtstimmungsstatus für alle Räume mit Lichtsteuerung

    Useful for:
        - Understanding current lighting state across the house
        - Identifying which rooms have presets active
        - Planning lighting changes
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Find all light controllers
    light_controllers = [
        device for device in ctx.devices.values() if device.type == "LightControllerV2"
    ]

    if not light_controllers:
        return {"error": "No light controllers found"}

    active_moods = {}
    summary_stats = {
        "total_controllers": len(light_controllers),
        "rooms_with_active_presets": 0,
        "rooms_with_no_presets": 0,
        "most_common_mood": None,
    }

    mood_counts = {}

    for controller in light_controllers:
        room_name = controller.room
        room_info = {
            "controller_name": controller.name,
            "uuid": controller.uuid,
            "active_mood": None,
            "has_active_preset": False,
            "error": None,
        }

        try:
            # Get current active mood
            if "activeMoods" in (controller.states or {}):
                active = await ctx.loxone.send_command(f"jdev/sps/io/{controller.uuid}/activeMoods")
                room_info["active_mood"] = active
                room_info["has_active_preset"] = active != 0

                if active != 0:
                    summary_stats["rooms_with_active_presets"] += 1
                    mood_counts[active] = mood_counts.get(active, 0) + 1
                else:
                    summary_stats["rooms_with_no_presets"] += 1

        except Exception as e:
            room_info["error"] = str(e)
            summary_stats["rooms_with_no_presets"] += 1

        active_moods[room_name] = room_info

    # Find most common mood
    if mood_counts:
        summary_stats["most_common_mood"] = max(mood_counts.items(), key=lambda x: x[1])

    return {"timestamp": "now", "summary": summary_stats, "room_moods": active_moods}


@mcp.tool()
async def control_central_lighting(action: str, mood_id: int | None = None) -> dict[str, Any]:
    """
    Control the central lighting system across all connected rooms.
    Steuert das zentrale Beleuchtungssystem in allen angeschlossenen Räumen.

    Args:
        action: Control action / Steuerungsaktion
                - "on" - Turn on all lights
                - "off" - Turn off all lights
                - "mood" - Set specific mood (requires mood_id)
                - "status" - Get status of central controller
        mood_id: Mood ID for "mood" action / Stimmungs-ID für "mood"-Aktion

    Returns:
        Result of central lighting control / Ergebnis der zentralen Lichtsteuerung

    Examples:
        - control_central_lighting("on") - Turn on all house lights
        - control_central_lighting("off") - Turn off all house lights
        - control_central_lighting("mood", 2) - Set all lights to mood 2
        - control_central_lighting("status") - Check central controller status
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Find central light controller
    central_controller = None
    for device in ctx.devices.values():
        if device.type == "CentralLightController":
            central_controller = device
            break

    if not central_controller:
        return {"error": "No central light controller found"}

    try:
        if action == "status":
            # Get status of central controller
            try:
                events = await ctx.loxone.send_command(
                    f"jdev/sps/io/{central_controller.uuid}/events"
                )
                return {
                    "controller": central_controller.name,
                    "uuid": central_controller.uuid,
                    "events": events,
                    "controlled_lights": len(central_controller.details.get("controls", [])),
                    "action": "status_check",
                }
            except Exception as e:
                return {
                    "controller": central_controller.name,
                    "uuid": central_controller.uuid,
                    "controlled_lights": len(central_controller.details.get("controls", [])),
                    "action": "status_check",
                    "error": str(e),
                }

        elif action in ["on", "off"]:
            # Control all lights via central controller
            command = "On" if action == "on" else "Off"

            try:
                await ctx.loxone.send_command(f"jdev/sps/io/{central_controller.uuid}/{command}")

                return {
                    "controller": central_controller.name,
                    "action": action,
                    "success": True,
                    "controlled_lights": len(central_controller.details.get("controls", [])),
                    "message": f"All house lights turned {action}",
                }
            except Exception as e:
                return {"controller": central_controller.name, "action": action, "error": str(e)}

        elif action == "mood" and mood_id is not None:
            # Set mood via central controller (if supported)
            try:
                await ctx.loxone.send_command(
                    f"jdev/sps/io/{central_controller.uuid}/mood/{mood_id}"
                )

                return {
                    "controller": central_controller.name,
                    "action": "mood",
                    "mood_id": mood_id,
                    "success": True,
                    "controlled_lights": len(central_controller.details.get("controls", [])),
                    "message": f"All house lights set to mood {mood_id}",
                }
            except Exception as e:
                return {
                    "controller": central_controller.name,
                    "action": "mood",
                    "mood_id": mood_id,
                    "error": str(e),
                    "note": "Central mood control may not be supported",
                }

        else:
            return {
                "error": f"Invalid action '{action}' or missing mood_id for mood action",
                "valid_actions": ["on", "off", "mood", "status"],
            }

    except Exception as e:
        return {"controller": central_controller.name, "action": action, "error": str(e)}


# === House Scene Management ===


@mcp.tool()
async def get_house_scenes() -> dict[str, Any]:
    """
    Get available house-wide scenes and their current status.
    Ruft verfügbare hausweite Szenen und deren aktuellen Status ab.

    Returns:
        Available house scenes and their current activation status
        Verfügbare Hausszenen und deren aktueller Aktivierungsstatus

    Shows:
        - Central lighting controller status
        - Central blinds controller status
        - House sleep mode status
        - Alarm clock scenes
        - Scene summary
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    scenes = {
        "central_lighting": None,
        "central_blinds": None,
        "house_sleep_mode": None,
        "alarm_clocks": [],
        "scene_summary": {
            "available_scenes": 0,
            "active_scenes": 0,
        },
    }

    # Find central light controller
    for device in ctx.devices.values():
        if device.type == "CentralLightController":
            try:
                events = await ctx.loxone.send_command(f"jdev/sps/io/{device.uuid}/events")
                scenes["central_lighting"] = {
                    "name": device.name,
                    "uuid": device.uuid,
                    "room": device.room,
                    "events": events,
                    "controlled_devices": len(device.details.get("controls", [])),
                    "is_active": events != 0,
                }
                scenes["scene_summary"]["available_scenes"] += 1
                if events != 0:
                    scenes["scene_summary"]["active_scenes"] += 1
            except Exception as e:
                scenes["central_lighting"] = {
                    "name": device.name,
                    "uuid": device.uuid,
                    "error": str(e),
                }
            break

    # Find central blinds controller
    for device in ctx.devices.values():
        if device.type == "CentralJalousie" and "zentral" in device.name.lower():
            try:
                events = await ctx.loxone.send_command(f"jdev/sps/io/{device.uuid}/events")
                scenes["central_blinds"] = {
                    "name": device.name,
                    "uuid": device.uuid,
                    "room": device.room,
                    "events": events,
                    "controlled_devices": len(device.details.get("controls", [])),
                    "is_active": events != 0,
                }
                scenes["scene_summary"]["available_scenes"] += 1
                if events != 0:
                    scenes["scene_summary"]["active_scenes"] += 1
            except Exception as e:
                scenes["central_blinds"] = {
                    "name": device.name,
                    "uuid": device.uuid,
                    "error": str(e),
                }
            break

    # Find house sleep mode switch
    for device in ctx.devices.values():
        if device.type == "Switch" and "tiefschlaf" in device.name.lower():
            try:
                active = await ctx.loxone.send_command(f"jdev/sps/io/{device.uuid}/active")
                locked_on = await ctx.loxone.send_command(f"jdev/sps/io/{device.uuid}/lockedOn")
                scenes["house_sleep_mode"] = {
                    "name": device.name,
                    "uuid": device.uuid,
                    "room": device.room,
                    "active": active,
                    "locked_on": locked_on,
                    "is_active": active != 0 or locked_on != 0,
                }
                scenes["scene_summary"]["available_scenes"] += 1
                if active != 0 or locked_on != 0:
                    scenes["scene_summary"]["active_scenes"] += 1
            except Exception as e:
                scenes["house_sleep_mode"] = {
                    "name": device.name,
                    "uuid": device.uuid,
                    "error": str(e),
                }
            break

    # Find alarm clocks (scene triggers)
    for device in ctx.devices.values():
        if device.type == "AlarmClock":
            try:
                is_enabled = await ctx.loxone.send_command(f"jdev/sps/io/{device.uuid}/isEnabled")
                alarm_clock = {
                    "name": device.name,
                    "uuid": device.uuid,
                    "room": device.room,
                    "is_enabled": is_enabled,
                    "is_active": is_enabled != 0,
                }

                # Try to get next alarm info
                states = device.states or {}
                if "nextEntry" in states:
                    try:
                        next_entry = await ctx.loxone.send_command(
                            f"jdev/sps/io/{device.uuid}/nextEntry"
                        )
                        alarm_clock["next_entry"] = next_entry
                    except Exception:
                        pass

                scenes["alarm_clocks"].append(alarm_clock)
                scenes["scene_summary"]["available_scenes"] += 1
                if is_enabled != 0:
                    scenes["scene_summary"]["active_scenes"] += 1

            except Exception as e:
                scenes["alarm_clocks"].append(
                    {"name": device.name, "uuid": device.uuid, "error": str(e)}
                )

    return {"timestamp": "now", "scenes": scenes}


@mcp.tool()
async def activate_house_scene(scene_type: str, action: str = "on") -> dict[str, Any]:
    """
    Activate or control house-wide scenes.
    Aktiviert oder steuert hausweite Szenen.

    Args:
        scene_type: Type of scene / Szenentyp
                   - "lighting" - Central lighting control / Zentrale Lichtsteuerung
                   - "blinds" - Central blinds control / Zentrale Beschattungssteuerung
                   - "sleep_mode" - House sleep mode / Haus-Schlafmodus
                   - "all_on" - Turn everything on / Alles einschalten
                   - "all_off" - Turn everything off / Alles ausschalten
                   - "night_mode" - Activate night scene / Nachtszene aktivieren
                   - "morning_mode" - Activate morning scene / Morgenszene aktivieren
        action: Action to perform / Auszuführende Aktion
               - "on" - Turn on/activate / Einschalten/aktivieren
               - "off" - Turn off/deactivate / Ausschalten/deaktivieren
               - "toggle" - Toggle state / Status umschalten

    Returns:
        Result of scene activation / Ergebnis der Szenenaktivierung

    Examples:
        - activate_house_scene("lighting", "on") - Turn on all house lights
        - activate_house_scene("blinds", "off") - Close all blinds
        - activate_house_scene("sleep_mode", "on") - Activate house sleep mode
        - activate_house_scene("night_mode") - Activate complete night scene
        - activate_house_scene("all_off") - Turn everything off
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    results = {"scene_type": scene_type, "action": action, "components": [], "success": False}

    try:
        if scene_type == "lighting":
            # Control central lighting
            central_light = None
            for device in ctx.devices.values():
                if device.type == "CentralLightController":
                    central_light = device
                    break

            if central_light:
                command = "On" if action == "on" else "Off"
                if action == "toggle":
                    # Get current state and toggle
                    current_events = await ctx.loxone.send_command(
                        f"jdev/sps/io/{central_light.uuid}/events"
                    )
                    command = "Off" if current_events != 0 else "On"

                await ctx.loxone.send_command(f"jdev/sps/io/{central_light.uuid}/{command}")
                results["components"].append(
                    {
                        "type": "central_lighting",
                        "name": central_light.name,
                        "action": command,
                        "success": True,
                        "controlled_devices": len(central_light.details.get("controls", [])),
                    }
                )
                results["success"] = True
            else:
                results["components"].append(
                    {"type": "central_lighting", "error": "No central light controller found"}
                )

        elif scene_type == "blinds":
            # Control central blinds
            central_blinds = None
            for device in ctx.devices.values():
                if device.type == "CentralJalousie" and "zentral" in device.name.lower():
                    central_blinds = device
                    break

            if central_blinds:
                if action in ["on", "off"]:
                    command = "FullUp" if action == "on" else "FullDown"
                elif action == "toggle":
                    # Get current state and toggle
                    current_events = await ctx.loxone.send_command(
                        f"jdev/sps/io/{central_blinds.uuid}/events"
                    )
                    command = "FullDown" if current_events != 0 else "FullUp"
                else:
                    command = "Stop"

                await ctx.loxone.send_command(f"jdev/sps/io/{central_blinds.uuid}/{command}")
                results["components"].append(
                    {
                        "type": "central_blinds",
                        "name": central_blinds.name,
                        "action": command,
                        "success": True,
                        "controlled_devices": len(central_blinds.details.get("controls", [])),
                    }
                )
                results["success"] = True
            else:
                results["components"].append(
                    {"type": "central_blinds", "error": "No central blinds controller found"}
                )

        elif scene_type == "sleep_mode":
            # Control house sleep mode
            sleep_switch = None
            for device in ctx.devices.values():
                if device.type == "Switch" and "tiefschlaf" in device.name.lower():
                    sleep_switch = device
                    break

            if sleep_switch:
                if action == "on":
                    command = "On"
                elif action == "off":
                    command = "Off"
                else:  # toggle
                    # Get current state and toggle
                    current_active = await ctx.loxone.send_command(
                        f"jdev/sps/io/{sleep_switch.uuid}/active"
                    )
                    command = "Off" if current_active != 0 else "On"

                await ctx.loxone.send_command(f"jdev/sps/io/{sleep_switch.uuid}/{command}")
                results["components"].append(
                    {
                        "type": "sleep_mode",
                        "name": sleep_switch.name,
                        "action": command,
                        "success": True,
                    }
                )
                results["success"] = True
            else:
                results["components"].append(
                    {"type": "sleep_mode", "error": "No sleep mode switch found"}
                )

        elif scene_type in ["all_on", "all_off", "night_mode", "morning_mode"]:
            # Complex scene orchestration
            success_count = 0

            # Determine actions for each component
            if scene_type == "all_on":
                light_action, blind_action, sleep_action = "on", "on", "off"
            elif scene_type == "all_off":
                light_action, blind_action, sleep_action = "off", "off", "off"
            elif scene_type == "night_mode":
                light_action, blind_action, sleep_action = "off", "off", "on"
            else:  # morning_mode
                light_action, blind_action, sleep_action = "on", "on", "off"

            # Execute lighting control
            for device in ctx.devices.values():
                if device.type == "CentralLightController":
                    try:
                        command = "On" if light_action == "on" else "Off"
                        await ctx.loxone.send_command(f"jdev/sps/io/{device.uuid}/{command}")
                        results["components"].append(
                            {
                                "type": "central_lighting",
                                "name": device.name,
                                "action": command,
                                "success": True,
                                "controlled_devices": len(device.details.get("controls", [])),
                            }
                        )
                        success_count += 1
                    except Exception as e:
                        results["components"].append(
                            {"type": "central_lighting", "name": device.name, "error": str(e)}
                        )
                    break

            # Execute blinds control
            for device in ctx.devices.values():
                if device.type == "CentralJalousie" and "zentral" in device.name.lower():
                    try:
                        command = "FullUp" if blind_action == "on" else "FullDown"
                        await ctx.loxone.send_command(f"jdev/sps/io/{device.uuid}/{command}")
                        results["components"].append(
                            {
                                "type": "central_blinds",
                                "name": device.name,
                                "action": command,
                                "success": True,
                                "controlled_devices": len(device.details.get("controls", [])),
                            }
                        )
                        success_count += 1
                    except Exception as e:
                        results["components"].append(
                            {"type": "central_blinds", "name": device.name, "error": str(e)}
                        )
                    break

            # Execute sleep mode control
            for device in ctx.devices.values():
                if device.type == "Switch" and "tiefschlaf" in device.name.lower():
                    try:
                        command = "On" if sleep_action == "on" else "Off"
                        await ctx.loxone.send_command(f"jdev/sps/io/{device.uuid}/{command}")
                        results["components"].append(
                            {
                                "type": "sleep_mode",
                                "name": device.name,
                                "action": command,
                                "success": True,
                            }
                        )
                        success_count += 1
                    except Exception as e:
                        results["components"].append(
                            {"type": "sleep_mode", "name": device.name, "error": str(e)}
                        )
                    break

            results["success"] = success_count > 0
            results["successful_components"] = success_count

        else:
            return {
                "error": f"Unknown scene type '{scene_type}'",
                "valid_scenes": [
                    "lighting",
                    "blinds",
                    "sleep_mode",
                    "all_on",
                    "all_off",
                    "night_mode",
                    "morning_mode",
                ],
            }

    except Exception as e:
        results["error"] = str(e)

    return results


@mcp.tool()
async def get_alarm_clocks() -> dict[str, Any]:
    """
    Get status and configuration of alarm clocks that can trigger house scenes.
    Ruft Status und Konfiguration der Wecker ab, die Hausszenen auslösen können.

    Returns:
        Alarm clock information and scene trigger capabilities
        Wecker-Informationen und Szenen-Auslöse-Fähigkeiten

    Shows:
        - Current alarm settings
        - Next scheduled alarms
        - Enable/disable status
        - Scene triggering potential
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    alarm_clocks = []
    summary = {"total_alarms": 0, "enabled_alarms": 0, "next_alarm": None}

    for device in ctx.devices.values():
        if device.type == "AlarmClock":
            alarm_info = {
                "name": device.name,
                "uuid": device.uuid,
                "room": device.room,
                "states": {},
                "scene_potential": "Can trigger wake-up scenes",
            }

            try:
                # Get basic alarm states
                states = device.states or {}
                for state_name in ["isEnabled", "isAlarmActive", "nextEntry", "nextEntryTime"]:
                    if state_name in states:
                        try:
                            value = await ctx.loxone.send_command(
                                f"jdev/sps/io/{device.uuid}/{state_name}"
                            )
                            alarm_info["states"][state_name] = value
                        except Exception as e:
                            alarm_info["states"][state_name] = f"Error: {e}"

                # Count statistics
                summary["total_alarms"] += 1
                if alarm_info["states"].get("isEnabled") == 1:
                    summary["enabled_alarms"] += 1

                # Track next alarm
                next_time = alarm_info["states"].get("nextEntryTime")
                if next_time and (
                    summary["next_alarm"] is None or next_time < summary["next_alarm"]
                ):
                    summary["next_alarm"] = next_time

            except Exception as e:
                alarm_info["error"] = str(e)

            alarm_clocks.append(alarm_info)

    return {"timestamp": "now", "summary": summary, "alarm_clocks": alarm_clocks}


@mcp.tool()
async def set_alarm_clock(alarm_name: str, enabled: bool) -> dict[str, Any]:
    """
    Enable or disable an alarm clock.
    Aktiviert oder deaktiviert einen Wecker.

    Args:
        alarm_name: Name of the alarm clock / Name des Weckers
                   Examples: "Bett Li.", "Bett Re."
        enabled: Whether to enable the alarm / Ob der Wecker aktiviert werden soll

    Returns:
        Result of alarm setting / Ergebnis der Wecker-Einstellung

    Examples:
        - set_alarm_clock("Bett Li.", True) - Enable left bedside alarm
        - set_alarm_clock("Bett Re.", False) - Disable right bedside alarm
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Find matching alarm clock
    alarm_clock = None
    for device in ctx.devices.values():
        if device.type == "AlarmClock" and alarm_name.lower() in device.name.lower():
            alarm_clock = device
            break

    if not alarm_clock:
        return {"error": f"No alarm clock found matching '{alarm_name}'"}

    try:
        # Set alarm enabled state
        command = "enable" if enabled else "disable"
        await ctx.loxone.send_command(f"jdev/sps/io/{alarm_clock.uuid}/{command}")

        # Verify the change
        try:
            is_enabled = await ctx.loxone.send_command(f"jdev/sps/io/{alarm_clock.uuid}/isEnabled")
            verification = is_enabled == (1 if enabled else 0)
        except Exception:
            verification = None

        return {
            "alarm_name": alarm_clock.name,
            "uuid": alarm_clock.uuid,
            "room": alarm_clock.room,
            "action": command,
            "success": True,
            "enabled": enabled,
            "verification": verification,
        }

    except Exception as e:
        return {
            "alarm_name": alarm_clock.name,
            "uuid": alarm_clock.uuid,
            "action": command,
            "error": str(e),
        }


@mcp.tool()
async def get_scene_status_overview() -> dict[str, Any]:
    """
    Get comprehensive overview of all house scene capabilities and current status.
    Ruft umfassende Übersicht aller Haus-Szenen-Fähigkeiten und aktuellen Status ab.

    Returns:
        Complete scene management overview / Vollständige Szenen-Management-Übersicht

    Shows:
        - Available scene types
        - Current activation status
        - Device counts and capabilities
        - Scene orchestration potential
        - Quick status summary
    """
    ctx: ServerContext = _context
    if not ctx:
        return {"error": "Not connected to Loxone"}

    # Get all scene data
    house_scenes = await get_house_scenes()
    alarm_clocks = await get_alarm_clocks()

    # Create comprehensive overview
    overview = {
        "timestamp": "now",
        "scene_capabilities": {
            "central_lighting": house_scenes.get("scenes", {}).get("central_lighting") is not None,
            "central_blinds": house_scenes.get("scenes", {}).get("central_blinds") is not None,
            "sleep_mode": house_scenes.get("scenes", {}).get("house_sleep_mode") is not None,
            "alarm_triggers": len(alarm_clocks.get("alarm_clocks", [])) > 0,
        },
        "current_status": {},
        "device_counts": {
            "controlled_lights": 0,
            "controlled_blinds": 0,
            "alarm_clocks": alarm_clocks.get("summary", {}).get("total_alarms", 0),
        },
        "quick_actions": [
            "activate_house_scene('all_on') - Turn everything on",
            "activate_house_scene('all_off') - Turn everything off",
            "activate_house_scene('night_mode') - Night scene",
            "activate_house_scene('morning_mode') - Morning scene",
            "activate_house_scene('lighting', 'on') - All lights on",
            "activate_house_scene('blinds', 'off') - Close all blinds",
        ],
        "available_scenes": [
            "lighting",
            "blinds",
            "sleep_mode",
            "all_on",
            "all_off",
            "night_mode",
            "morning_mode",
        ],
    }

    # Extract current status and device counts
    scenes_data = house_scenes.get("scenes", {})

    if scenes_data.get("central_lighting"):
        overview["current_status"]["lighting"] = {
            "active": scenes_data["central_lighting"].get("is_active", False),
            "name": scenes_data["central_lighting"].get("name", "Unknown"),
        }
        overview["device_counts"]["controlled_lights"] = scenes_data["central_lighting"].get(
            "controlled_devices", 0
        )

    if scenes_data.get("central_blinds"):
        overview["current_status"]["blinds"] = {
            "active": scenes_data["central_blinds"].get("is_active", False),
            "name": scenes_data["central_blinds"].get("name", "Unknown"),
        }
        overview["device_counts"]["controlled_blinds"] = scenes_data["central_blinds"].get(
            "controlled_devices", 0
        )

    if scenes_data.get("house_sleep_mode"):
        overview["current_status"]["sleep_mode"] = {
            "active": scenes_data["house_sleep_mode"].get("is_active", False),
            "name": scenes_data["house_sleep_mode"].get("name", "Unknown"),
        }

    # Add alarm status
    overview["current_status"]["alarms"] = {
        "total": alarm_clocks.get("summary", {}).get("total_alarms", 0),
        "enabled": alarm_clocks.get("summary", {}).get("enabled_alarms", 0),
    }

    # Calculate overall scene health
    total_devices = (
        overview["device_counts"]["controlled_lights"]
        + overview["device_counts"]["controlled_blinds"]
        + overview["device_counts"]["alarm_clocks"]
    )

    available_controllers = sum(1 for v in overview["scene_capabilities"].values() if v)

    overview["scene_health"] = {
        "total_controlled_devices": total_devices,
        "available_controllers": available_controllers,
        "scene_readiness": (
            "excellent"
            if available_controllers >= 3
            else "good"
            if available_controllers >= 2
            else "basic"
        ),
    }

    return overview


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


@mcp.prompt("hilfe")
async def prompt_hilfe() -> str:
    """
    Show help for German commands.
    Zeige Hilfe für deutsche Befehle.
    """
    return """Loxone Steuerung - Deutsche Befehle:

ROLLADEN/JALOUSIEN:
- "Rolladen [Raum] hoch/runter/stop"
- "Jalousien [Raum] öffnen/schließen"
- "Rolladen [Raum] auf [0-100]%"
- "Alle Rolladen im OG runter"

LICHTER:
- "Licht [Raum] an/aus/umschalten"
- "Lichter [Raum] einschalten/ausschalten"
- "Licht [Raum] auf [0-100]% dimmen"
- "Alle Lichter im OG aus"

RÄUME:
- Nutze Raumnamen wie: Wohnzimmer, Küche, Bad, Büro
- Mit Stockwerk: OG Büro, OG Schlafzimmer, EG Wohnzimmer
- Ganze Stockwerke: OG (Obergeschoss), EG (Erdgeschoss)

BEISPIELE:
- "Rolladen OG Büro runter"
- "Licht Wohnzimmer einschalten"
- "Alle Lichter im OG ausschalten"
- "Jalousien Küche auf 50%"

Du kannst auch Englisch oder gemischte Befehle verwenden!"""


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
