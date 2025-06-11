"""Loxone MCP Server - Clean implementation without hardcoded UUIDs.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import asyncio
import datetime
import logging
import os
import signal
import sys
from collections.abc import AsyncGenerator
from contextlib import asynccontextmanager
from dataclasses import dataclass
from typing import Any

from mcp.server.fastmcp import FastMCP

from loxone_mcp.credentials import LoxoneSecrets
from loxone_mcp.dynamic_sensor_discovery import DiscoveredSensor, discover_sensors_automatically
from loxone_mcp.loxone_token_client import LoxoneTokenClient
from loxone_mcp.sensor_state_logger import get_state_logger
from loxone_mcp.weather_forecast import WeatherForecastClient

# Set up logging
logging.basicConfig(
    level=os.getenv("LOXONE_LOG_LEVEL", "INFO"),
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger(__name__)


@dataclass
class SystemCapabilities:
    """Detected system capabilities based on available devices."""

    has_lighting: bool = False
    has_blinds: bool = False
    has_weather: bool = False
    has_security: bool = False
    has_energy: bool = False
    has_audio: bool = False
    has_climate: bool = False
    has_sensors: bool = False

    # Detailed counts
    light_count: int = 0
    blind_count: int = 0
    weather_device_count: int = 0
    security_device_count: int = 0
    energy_device_count: int = 0
    audio_zone_count: int = 0
    climate_device_count: int = 0
    sensor_count: int = 0


@dataclass
class ServerContext:
    """Server context holding connections and cached data."""

    loxone: LoxoneTokenClient
    rooms: dict[str, str]  # UUID -> name mapping
    devices: dict[str, Any]  # UUID -> device info
    categories: dict[str, str]  # UUID -> category name mapping
    devices_by_category: dict[str, list[dict[str, Any]]]  # Category -> devices
    devices_by_type: dict[str, list[dict[str, Any]]]  # Device type -> devices
    devices_by_room: dict[str, list[dict[str, Any]]]  # Room UUID -> devices
    discovered_sensors: list[DiscoveredSensor]  # Dynamically discovered sensors
    capabilities: SystemCapabilities  # Detected system capabilities


# Global context
_context: ServerContext | None = None


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
}


# Global flag to track if initialization is in progress
_initializing = False
_initialization_error: Exception | None = None
_initialization_event = asyncio.Event()
_initialization_event.set()  # Initially not initializing


async def _ensure_connection() -> ServerContext:
    """Ensure Loxone connection is established (lazy initialization)."""
    global _context, _initializing, _initialization_error

    if _context:
        return _context

    if _initializing:
        # Wait for initialization to complete using Event
        await _initialization_event.wait()
        if _initialization_error:
            raise _initialization_error
        if _context:
            return _context
        raise RuntimeError("Initialization failed")

    _initializing = True
    _initialization_error = None
    _initialization_event.clear()

    try:
        logger.info("Initializing Loxone connection...")
        await _initialize_loxone_connection()
        if not _context:
            raise RuntimeError("Failed to initialize context")
        return _context
    except Exception as e:
        _initialization_error = e
        logger.error(f"Failed to initialize Loxone connection: {e}")
        raise
    finally:
        _initializing = False
        _initialization_event.set()


async def _initialize_loxone_connection() -> None:
    """Initialize the Loxone connection and context."""
    global _context

    logger.info("Starting Loxone MCP Server initialization")

    try:
        # Get credentials
        secrets = LoxoneSecrets()
        host = secrets.get("LOXONE_HOST")
        username = secrets.get("LOXONE_USER")
        password = secrets.get("LOXONE_PASS")

        if not all([host, username, password]):
            creds = [("host", host), ("username", username), ("password", password)]
            missing = [k for k, v in creds if not v]
            raise ValueError(f"Missing credentials: {', '.join(missing)}")

        # Initialize Loxone client with token authentication and reconnection support
        loxone = LoxoneTokenClient(
            host, username, password, max_reconnect_attempts=5, reconnect_delay=5.0
        )
        await loxone.connect()

        # Start connection monitor
        try:
            # Store reference to background task
            async def monitor_connection() -> None:
                try:
                    await loxone._monitor_connection()
                except Exception as e:
                    logger.warning(f"Connection monitor stopped: {e}")

            asyncio.create_task(monitor_connection())  # noqa: RUF006
            logger.info("Started connection health monitor")
        except Exception as e:
            logger.warning(f"Failed to start connection monitor: {e}")

        # Load structure
        structure = await loxone.get_structure_file()

        # Extract rooms, devices, and categories
        rooms = {}
        devices = {}
        categories = {}
        devices_by_category = {}
        devices_by_type = {}
        devices_by_room = {}

        for room_uuid, room_data in structure.get("rooms", {}).items():
            rooms[room_uuid] = room_data.get("name", f"Room {room_uuid}")
            devices_by_room[room_uuid] = []

        for device_uuid, device_data in structure.get("controls", {}).items():
            # Enrich device data with room and category names
            device_info = {
                **device_data,
                "uuid": device_uuid,
                "room_name": rooms.get(device_data.get("room"), "Unknown"),
                "category_name": None,  # Will be filled below
            }
            devices[device_uuid] = device_info

            # Organize by room
            room_uuid = device_data.get("room")
            if room_uuid in devices_by_room:
                devices_by_room[room_uuid].append(device_info)

            # Organize by device type
            device_type = device_data.get("type", "Unknown")
            if device_type not in devices_by_type:
                devices_by_type[device_type] = []
            devices_by_type[device_type].append(device_info)

        # Extract category definitions
        for cat_uuid, cat_data in structure.get("cats", {}).items():
            categories[cat_uuid] = cat_data.get("name", f"Category {cat_uuid}")

        # Organize devices by category and update category names
        for _device_uuid, device_info in devices.items():
            cat_uuid = device_info.get("cat")
            cat_name = categories.get(cat_uuid, "Uncategorized") if cat_uuid else "Uncategorized"

            # Update device with category name
            device_info["category_name"] = cat_name

            # Organize by category
            if cat_name not in devices_by_category:
                devices_by_category[cat_name] = []
            devices_by_category[cat_name].append(device_info)

        logger.info(
            f"Loaded {len(rooms)} rooms, {len(devices)} devices, {len(categories)} categories"
        )
        logger.info(f"Device types: {list(devices_by_type.keys())[:10]}...")  # Show first 10
        logger.info(f"Categories: {list(devices_by_category.keys())[:10]}...")  # Show first 10

        # Detect system capabilities
        capabilities = _detect_system_capabilities(devices, devices_by_category, devices_by_type)
        logger.info(f"Detected capabilities: {_format_capabilities(capabilities)}")

        # Start real-time monitoring for sensor discovery
        try:
            await loxone.start_realtime_monitoring()
            logger.info("Started real-time WebSocket monitoring")
        except Exception as e:
            logger.warning(
                f"Failed to start real-time monitoring: {e}, continuing without WebSocket"
            )
            # Continue without real-time monitoring - basic HTTP functionality will work

        # Start with empty sensors for fast startup, discover later
        logger.info("Starting with empty sensor list for fast startup")
        discovered_sensors = []

        # Schedule sensor discovery to run in background after server starts
        if loxone.websocket_client and loxone.realtime_monitoring:

            async def background_sensor_discovery() -> None:
                """Discover sensors in background after server startup."""
                try:
                    logger.info("Starting background sensor discovery...")
                    sensors = await discover_sensors_automatically(
                        loxone.websocket_client,
                        discovery_time=30.0,  # Full time for background discovery
                    )
                    if _context:
                        _context.discovered_sensors = sensors
                        logger.info(f"Background discovery found {len(sensors)} sensors")
                except Exception as e:
                    logger.warning(f"Background sensor discovery failed: {e}")

            # Start discovery in background
            asyncio.create_task(background_sensor_discovery())  # noqa: RUF006
        logger.info(f"Discovered {len(discovered_sensors)} door/window sensors")

        # Set up context
        _context = ServerContext(
            loxone=loxone,
            rooms=rooms,
            devices=devices,
            categories=categories,
            devices_by_category=devices_by_category,
            devices_by_type=devices_by_type,
            devices_by_room=devices_by_room,
            discovered_sensors=discovered_sensors,
            capabilities=capabilities,
        )

        logger.info(f"Connected to Loxone at {host}")
        logger.info(f"Loaded {len(rooms)} rooms and {len(devices)} devices")
        logger.info(f"Discovered {len(discovered_sensors)} sensors dynamically")

        logger.info(f"Connected to Loxone at {host}")
        logger.info(f"Loaded {len(rooms)} rooms and {len(devices)} devices")
        logger.info(
            f"Starting with {len(discovered_sensors)} sensors (background discovery active)"
        )

    except Exception as e:
        logger.error(f"Failed to initialize Loxone connection: {e}")
        raise


@asynccontextmanager
async def lifespan(_mcp: FastMCP) -> AsyncGenerator[None, None]:
    """Manage server lifecycle - minimal setup for fast startup."""
    logger.info("Starting Loxone MCP Server (fast startup mode)")

    try:
        # No blocking initialization - just start the server
        yield
    finally:
        # Graceful cleanup on shutdown
        logger.info("Server shutdown initiated...")
        global _context
        if _context and _context.loxone:
            try:
                logger.info("Shutting down Loxone connection...")

                # Shutdown state logger gracefully
                try:
                    state_logger = get_state_logger()
                    await state_logger.shutdown()
                    logger.debug("State logger shutdown complete")
                except Exception as e:
                    logger.warning(f"Error shutting down state logger: {e}")

                # Close Loxone connection with timeout
                try:
                    # Use asyncio.wait_for to prevent hanging during shutdown
                    await asyncio.wait_for(_context.loxone.close(), timeout=10.0)
                    logger.info("Loxone connection closed successfully")
                except TimeoutError:
                    logger.warning("Loxone connection close timed out (forced shutdown)")
                except Exception as e:
                    logger.warning(f"Error closing Loxone connection: {e}")

            except Exception as e:
                logger.error(f"Error during cleanup: {e}")
        logger.info("Server shutdown complete")


# Create MCP server with fast startup
mcp = FastMCP("Loxone Controller", lifespan=lifespan)


# === Room Management Tools ===


@mcp.tool()
async def list_rooms() -> list[dict[str, str]]:
    """
    List all available rooms in the Loxone system.

    Returns a list of rooms with their UUID and name.
    """
    try:
        context = await _ensure_connection()
        return [{"uuid": uuid, "name": name} for uuid, name in context.rooms.items()]
    except Exception as e:
        return [{"error": f"Failed to connect to Loxone: {e}"}]


@mcp.tool()
async def get_room_devices(room: str, device_type: str | None = None) -> list[dict[str, Any]]:
    """
    Get all devices in a specific room, optionally filtered by device type.

    Args:
        room: Room name or UUID
        device_type: Optional device type filter (e.g., "Jalousie", "LightController")

    Returns:
        List of devices in the room
    """
    try:
        context = await _ensure_connection()
    except Exception as e:
        return [{"error": f"Failed to connect to Loxone: {e}"}]

    # Find room UUID
    room_uuid = None
    for uuid, name in context.rooms.items():
        if room.lower() in name.lower() or room == uuid:
            room_uuid = uuid
            break

    if not room_uuid:
        return [{"error": f"Room '{room}' not found"}]

    # Find devices in room
    room_devices = []
    for device_uuid, device in context.devices.items():
        if device.get("room") == room_uuid:
            device_info = {
                "uuid": device_uuid,
                "name": device.get("name", "Unknown"),
                "type": device.get("type", "Unknown"),
                "room": context.rooms.get(room_uuid, "Unknown"),
            }

            # Filter by device type if specified
            if device_type is None or device_type.lower() in device_info["type"].lower():
                room_devices.append(device_info)

    return room_devices


# === Device Control Tools ===


@mcp.tool()
async def control_device(device: str, action: str, room: str | None = None) -> dict[str, Any]:
    """
    Control a Loxone device (lights, rolladen, etc.).

    Args:
        device: Device name or UUID
        action: Action to perform (on/off for lights, up/down/stop for rolladen)
        room: Optional room name to narrow search

    Returns:
        Result of the control action
    """
    try:
        context = await _ensure_connection()
    except Exception as e:
        return {"error": f"Failed to connect to Loxone: {e}"}

    # Check connection status
    if not context.loxone.connected:
        logger.info("Connection lost, attempting to reconnect...")
        try:
            await context.loxone._ensure_connection()
        except Exception as e:
            return {"error": f"Failed to reconnect: {e}"}

    # Normalize action using aliases
    action = ACTION_ALIASES.get(action.lower(), action.lower())

    # Find device
    target_device = None
    target_uuid = None

    for device_uuid, device_data in context.devices.items():
        device_name = device_data.get("name", "").lower()
        device_type = device_data.get("type", "").lower()

        # Check if device matches
        if device.lower() in device_name or device == device_uuid:
            # If room specified, check room match
            if room:
                device_room_uuid = device_data.get("room")
                if device_room_uuid:
                    room_name = context.rooms.get(device_room_uuid, "").lower()
                    if room.lower() not in room_name:
                        continue

            target_device = device_data
            target_uuid = device_uuid
            break

    if not target_device:
        return {"error": f"Device '{device}' not found" + (f" in room '{room}'" if room else "")}

    # Determine command based on device type and action
    device_type = target_device.get("type", "").lower()

    try:
        if "jalousie" in device_type or "rolladen" in device_type:
            # Rolladen/blind control
            if action in ["up", "hoch", "rauf", "öffnen", "auf"]:
                result = await context.loxone.send_command(f"jdev/sps/io/{target_uuid}/Up")
            elif action in ["down", "runter", "zu", "schließen"]:
                result = await context.loxone.send_command(f"jdev/sps/io/{target_uuid}/Down")
            elif action in ["stop", "stopp", "anhalten"]:
                result = await context.loxone.send_command(f"jdev/sps/io/{target_uuid}/stop")
            else:
                return {"error": f"Invalid action '{action}' for rolladen. Use: up, down, stop"}

        elif "light" in device_type:
            # Light control
            if action in ["on", "an", "ein"]:
                result = await context.loxone.send_command(f"jdev/sps/io/{target_uuid}/On")
            elif action in ["off", "aus"]:
                result = await context.loxone.send_command(f"jdev/sps/io/{target_uuid}/Off")
            else:
                return {"error": f"Invalid action '{action}' for light. Use: on, off"}

        else:
            # Generic control
            result = await context.loxone.send_command(f"jdev/sps/io/{target_uuid}/{action}")

        return {
            "device": target_device.get("name"),
            "type": target_device.get("type"),
            "action": action,
            "result": result,
        }

    except Exception as e:
        # Check if it's a connection error
        if "connection" in str(e).lower() or "network" in str(e).lower():
            return {
                "error": (
                    f"Connection error: {e}. The system will attempt to reconnect automatically."
                )
            }
        return {"error": f"Failed to control device: {e}"}


# === Generic Device Discovery and Control Tools ===


@mcp.tool()
async def discover_all_devices() -> dict[str, Any]:
    """
    Discover all devices in the Loxone system organized by categories and types.

    Returns:
        Complete overview of all devices, categories, and rooms
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    # Summary statistics
    total_devices = len(_context.devices)
    total_rooms = len(_context.rooms)
    total_categories = len(_context.categories)

    # Organize by categories (top level)
    categories_summary = {}
    for cat_name, devices_list in _context.devices_by_category.items():
        categories_summary[cat_name] = {
            "count": len(devices_list),
            "device_types": list({d.get("type", "Unknown") for d in devices_list}),
        }

    # Organize by device types
    types_summary = {}
    for device_type, devices_list in _context.devices_by_type.items():
        types_summary[device_type] = {
            "count": len(devices_list),
            "categories": list({d.get("category_name", "Unknown") for d in devices_list}),
        }

    return {
        "overview": {
            "total_devices": total_devices,
            "total_rooms": total_rooms,
            "total_categories": total_categories,
        },
        "categories": categories_summary,
        "device_types": types_summary,
        "rooms": dict(_context.rooms),
        "note": "Use get_devices_by_category() or get_devices_by_type() for detailed device lists",
    }


@mcp.tool()
async def get_devices_by_category(category: str | None = None) -> dict[str, Any]:
    """
    Get devices filtered by Loxone category.

    Args:
        category: Category name (e.g., "Überwachung", "Beschattung", "Beleuchtung")
                 If None, returns all categories with device counts

    Returns:
        Devices in the specified category with current states
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    if category is None:
        # Return all categories with counts
        return {
            "available_categories": {
                cat_name: len(devices) for cat_name, devices in _context.devices_by_category.items()
            },
            "note": "Specify a category name to get detailed device information",
        }

    # Find devices in the specified category
    devices_in_category = _context.devices_by_category.get(category, [])

    if not devices_in_category:
        available = list(_context.devices_by_category.keys())
        return {
            "error": f"Category '{category}' not found",
            "available_categories": available[:10],  # Show first 10
            "total_categories": len(available),
        }

    # Get current states for all devices in category
    results = {"category": category, "device_count": len(devices_in_category), "devices": []}

    for device in devices_in_category:
        device_uuid = device["uuid"]
        device_info = {
            "uuid": device_uuid,
            "name": device.get("name", "Unknown"),
            "type": device.get("type", "Unknown"),
            "room": device.get("room_name", "Unknown"),
            "category": category,
        }

        # Try to get current state
        try:
            state = _context.loxone.get_realtime_state(device_uuid)
            if state is None:
                state = await _context.loxone.send_command(f"jdev/sps/io/{device_uuid}/state")
            device_info["current_state"] = state
        except Exception as e:
            device_info["state_error"] = str(e)

        results["devices"].append(device_info)

    return results


@mcp.tool()
async def get_devices_by_type(device_type: str | None = None) -> dict[str, Any]:
    """
    Get devices filtered by device type.

    Args:
        device_type: Device type (e.g., "Jalousie", "LightController", "WindowMonitor")
                    If None, returns all types with device counts

    Returns:
        Devices of the specified type with current states
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    if device_type is None:
        # Return all device types with counts
        return {
            "available_types": {
                dtype: len(devices) for dtype, devices in _context.devices_by_type.items()
            },
            "note": "Specify a device type to get detailed device information",
        }

    # Find devices of the specified type
    devices_of_type = _context.devices_by_type.get(device_type, [])

    if not devices_of_type:
        available = list(_context.devices_by_type.keys())
        return {
            "error": f"Device type '{device_type}' not found",
            "available_types": available[:10],  # Show first 10
            "total_types": len(available),
        }

    # Get current states for all devices of this type
    results = {"device_type": device_type, "device_count": len(devices_of_type), "devices": []}

    for device in devices_of_type:
        device_uuid = device["uuid"]
        device_info = {
            "uuid": device_uuid,
            "name": device.get("name", "Unknown"),
            "type": device_type,
            "room": device.get("room_name", "Unknown"),
            "category": device.get("category_name", "Unknown"),
        }

        # Try to get current state
        try:
            state = _context.loxone.get_realtime_state(device_uuid)
            if state is None:
                state = await _context.loxone.send_command(f"jdev/sps/io/{device_uuid}/state")
            device_info["current_state"] = state
        except Exception as e:
            device_info["state_error"] = str(e)

        results["devices"].append(device_info)

    return results


@mcp.tool()
async def rediscover_sensors(discovery_time: float = 30.0) -> dict[str, Any]:
    """
    Trigger a new sensor discovery process.

    Args:
        discovery_time: How long to monitor for discovery (seconds)

    Returns:
        Result of the discovery process
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    try:
        logger.info(f"Starting sensor rediscovery for {discovery_time} seconds...")

        # Make sure WebSocket monitoring is active
        if not _context.loxone.realtime_monitoring:
            await _context.loxone.start_realtime_monitoring()

        # Discover sensors
        discovered_sensors = await discover_sensors_automatically(
            _context.loxone.websocket_client, discovery_time=discovery_time
        )

        # Update context
        _context.discovered_sensors = discovered_sensors

        return {
            "success": True,
            "discovery_time": discovery_time,
            "sensors_found": len(discovered_sensors),
            "sensors": [
                {
                    "uuid": sensor.uuid,
                    "current_value": sensor.current_value,
                    "update_count": sensor.update_count,
                    "is_binary": sensor.is_binary,
                    "is_door_window": sensor.is_door_window,
                }
                for sensor in discovered_sensors
            ],
        }

    except Exception as e:
        logger.error(f"Sensor rediscovery failed: {e}")
        return {"error": f"Failed to rediscover sensors: {e}"}


@mcp.tool()
async def get_all_categories_overview() -> dict[str, Any]:
    """
    Get all devices organized by Loxone categories from the structure file.

    Returns:
        Dictionary with devices organized by category from Loxone structure
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    devices = _context.devices
    categories = _context.categories

    if not devices:
        return {"error": "No devices loaded"}

    # Organize devices by category
    device_categories = {}
    uncategorized_devices = []

    for device_uuid, device_data in devices.items():
        cat_uuid = device_data.get("cat")
        cat_name = categories.get(cat_uuid, "Unknown Category") if cat_uuid else None

        device_info = {
            "uuid": device_uuid[-12:],  # Show last 12 chars for readability
            "name": device_data.get("name", "Unknown"),
            "type": device_data.get("type", "Unknown"),
            "room": _context.rooms.get(device_data.get("room"), "Unknown"),
        }

        if cat_name:
            if cat_name not in device_categories:
                device_categories[cat_name] = {"category_uuid": cat_uuid, "count": 0, "devices": []}

            device_categories[cat_name]["count"] += 1
            device_categories[cat_name]["devices"].append(device_info)
        else:
            uncategorized_devices.append(device_info)

    # Sort devices within each category by name
    for category_data in device_categories.values():
        category_data["devices"].sort(key=lambda d: d["name"])
        # Show first 15 devices per category
        if len(category_data["devices"]) > 15:
            category_data["sample_devices"] = category_data["devices"][:15]
            category_data["total_devices"] = len(category_data["devices"])
            del category_data["devices"]
        else:
            category_data["devices"] = category_data["devices"]

    return {
        "total_devices": len(devices),
        "total_categories": len(categories),
        "categorized_devices": len(devices) - len(uncategorized_devices),
        "uncategorized_devices": len(uncategorized_devices),
        "categories": device_categories,
        "uncategorized": uncategorized_devices[:10] if uncategorized_devices else [],
        "note": "Devices organized by Loxone structure categories",
    }


@mcp.tool()
async def get_sensor_categories() -> dict[str, Any]:
    """
    Get sensor categorization overview showing all discovered sensors by category.

    Returns:
        Dictionary with sensors organized by category and confidence scores
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    sensors = _context.discovered_sensors
    if not sensors:
        return {"error": "No sensors discovered"}

    # Organize sensors by category
    categories = {}
    for sensor in sensors:
        category = sensor.category
        if category not in categories:
            categories[category] = {
                "name": category.replace("_", " ").title(),
                "count": 0,
                "sensors": [],
            }

        categories[category]["count"] += 1
        categories[category]["sensors"].append(
            {
                "uuid": sensor.uuid[-12:],  # Show last 12 chars for readability
                "confidence": round(sensor.confidence, 2),
                "pattern_score": round(sensor.pattern_score, 2),
                "updates": sensor.update_count,
                "current_value": sensor.current_value,
            }
        )

    # Sort sensors within each category by confidence
    for category_data in categories.values():
        category_data["sensors"].sort(key=lambda s: s["confidence"], reverse=True)
        # Limit to top 10 per category
        category_data["sensors"] = category_data["sensors"][:10]

    return {
        "total_sensors": len(sensors),
        "categories": categories,
        "note": "Sensors are automatically categorized based on behavior patterns",
    }


@mcp.tool()
async def list_discovered_sensors() -> dict[str, Any]:
    """
    List all dynamically discovered sensors with their details.

    Returns:
        List of all discovered sensors with analytics
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    sensors = _context.discovered_sensors

    return {
        "total_sensors": len(sensors),
        "door_window_sensors": len([s for s in sensors if s.is_door_window]),
        "binary_sensors": len([s for s in sensors if s.is_binary]),
        "sensors": [
            {
                "uuid": sensor.uuid,
                "current_value": sensor.current_value,
                "update_count": sensor.update_count,
                "is_binary": sensor.is_binary,
                "is_door_window": sensor.is_door_window,
                "first_seen": sensor.first_seen,
                "last_updated": sensor.last_updated,
                "value_history": sensor.value_history[:10],  # Last 10 values
            }
            for sensor in sensors
        ],
        "discovery_method": "dynamic_websocket",
        "note": "Sensors discovered automatically during startup via WebSocket monitoring",
    }


@mcp.tool()
async def get_sensor_details(uuid: str) -> dict[str, Any]:
    """
    Get detailed information about a specific discovered sensor.

    Args:
        uuid: Sensor UUID to get details for

    Returns:
        Detailed sensor information
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    # Find sensor in discovered sensors
    sensor = None
    for s in _context.discovered_sensors:
        if s.uuid == uuid:
            sensor = s
            break

    if not sensor:
        return {"error": f"Sensor {uuid} not found in discovered sensors"}

    # Get current real-time state
    current_state = _context.loxone.get_realtime_state(uuid)

    return {
        "uuid": sensor.uuid,
        "current_state": current_state,
        "stored_value": sensor.current_value,
        "update_count": sensor.update_count,
        "is_binary": sensor.is_binary,
        "is_door_window": sensor.is_door_window,
        "first_seen": sensor.first_seen,
        "last_updated": sensor.last_updated,
        "value_history": sensor.value_history,
        "unique_values": list(set(sensor.value_history)),
        "analysis": {
            "appears_functional": sensor.update_count > 0,
            "likely_door_window": sensor.is_door_window,
            "binary_behavior": sensor.is_binary,
            "activity_level": "high" if sensor.update_count > 10 else "low",
        },
    }


# === Weather and Environmental Tools ===


@mcp.tool()
async def get_weather_data() -> dict[str, Any]:
    """
    Get weather data from Loxone weather stations and environmental sensors.

    Returns:
        Current weather conditions from connected Loxone weather devices
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    # Check if weather capability is available
    if not _context.capabilities.has_weather:
        return {
            "error": "No weather devices available",
            "note": (
                "Your Loxone system doesn't have weather stations or "
                "environmental sensors configured"
            ),
            "available_features": _get_available_features(_context.capabilities),
        }

    # Look for weather-related devices
    weather_categories = ["Wetter", "Weather", "Klima", "Außen", "Outdoor"]
    weather_types = ["WeatherServer", "TemperatureSensor", "HumiditySensor", "WindSensor"]

    weather_devices = []

    # Find weather devices by category
    for category_name in weather_categories:
        if category_name in _context.devices_by_category:
            for device in _context.devices_by_category[category_name]:
                weather_devices.append(
                    {**device, "discovery_method": "category", "category_matched": category_name}
                )

    # Find weather devices by type
    for device_type in weather_types:
        if device_type in _context.devices_by_type:
            for device in _context.devices_by_type[device_type]:
                # Avoid duplicates
                if not any(d["uuid"] == device["uuid"] for d in weather_devices):
                    weather_devices.append(
                        {**device, "discovery_method": "type", "type_matched": device_type}
                    )

    # Also look for devices with weather-related names
    weather_keywords = ["temp", "humidity", "wind", "rain", "weather", "außen", "outdoor"]
    for device_uuid, device in _context.devices.items():
        device_name = device.get("name", "").lower()
        if any(keyword in device_name for keyword in weather_keywords) and not any(
            d["uuid"] == device_uuid for d in weather_devices
        ):
            weather_devices.append(
                {**device, "discovery_method": "name", "name_matched": device_name}
            )

    if not weather_devices:
        return {
            "error": "No weather devices found",
            "note": "Searched categories: Wetter, Weather, Klima, Außen, Outdoor",
            "suggestion": "Check if your Loxone system has weather stations configured",
        }

    # Get current states for all weather devices
    weather_data = {"timestamp": datetime.datetime.now().isoformat(), "devices": [], "summary": {}}

    temperature_readings = []
    humidity_readings = []

    for device in weather_devices:
        device_uuid = device["uuid"]
        device_info = {
            "uuid": device_uuid,
            "name": device.get("name", "Unknown"),
            "type": device.get("type", "Unknown"),
            "category": device.get("category_name", "Unknown"),
            "room": device.get("room_name", "Unknown"),
            "discovery_method": device.get("discovery_method", "unknown"),
        }

        # Get current state
        try:
            state = _context.loxone.get_realtime_state(device_uuid)
            if state is None:
                state = await _context.loxone.send_command(f"jdev/sps/io/{device_uuid}/state")

            device_info["current_value"] = state
            device_info["unit"] = _guess_weather_unit(
                device.get("name", ""), device.get("type", "")
            )

            # Collect readings for summary
            if state is not None and isinstance(state, int | float):
                device_name_lower = device.get("name", "").lower()
                if "temp" in device_name_lower or "temperatur" in device_name_lower:
                    temperature_readings.append(state)
                elif "humid" in device_name_lower or "feucht" in device_name_lower:
                    humidity_readings.append(state)

        except Exception as e:
            device_info["error"] = str(e)

        weather_data["devices"].append(device_info)

    # Generate summary
    if temperature_readings:
        weather_data["summary"]["temperature"] = {
            "average": round(sum(temperature_readings) / len(temperature_readings), 1),
            "min": min(temperature_readings),
            "max": max(temperature_readings),
            "readings": len(temperature_readings),
        }

    if humidity_readings:
        weather_data["summary"]["humidity"] = {
            "average": round(sum(humidity_readings) / len(humidity_readings), 1),
            "min": min(humidity_readings),
            "max": max(humidity_readings),
            "readings": len(humidity_readings),
        }

    return weather_data


@mcp.tool()
async def get_outdoor_conditions() -> dict[str, Any]:
    """
    Get current outdoor environmental conditions from Loxone sensors.

    Returns:
        Summary of outdoor temperature, humidity, and other environmental data
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    # Get weather data first
    weather_data = await get_weather_data()

    if "error" in weather_data:
        return weather_data

    # Filter for outdoor devices only
    outdoor_devices = []
    outdoor_keywords = [
        "außen",
        "outdoor",
        "aussen",
        "outside",
        "garten",
        "garden",
        "terrasse",
        "terrace",
    ]

    for device in weather_data["devices"]:
        device_name = device.get("name", "").lower()
        room_name = device.get("room", "").lower()

        if any(keyword in device_name or keyword in room_name for keyword in outdoor_keywords):
            outdoor_devices.append(device)

    # Generate conditions summary
    conditions = {
        "timestamp": datetime.datetime.now().isoformat(),
        "status": "unknown",
        "temperature": None,
        "humidity": None,
        "conditions_text": "",
        "devices": outdoor_devices,
    }

    # Analyze outdoor conditions
    temp_values = []
    humidity_values = []

    for device in outdoor_devices:
        if "current_value" in device and isinstance(device["current_value"], int | float):
            device_name = device.get("name", "").lower()
            if "temp" in device_name:
                temp_values.append(device["current_value"])
            elif "humid" in device_name or "feucht" in device_name:
                humidity_values.append(device["current_value"])

    if temp_values:
        avg_temp = sum(temp_values) / len(temp_values)
        conditions["temperature"] = {
            "value": round(avg_temp, 1),
            "unit": "°C",
            "status": _get_temperature_status(avg_temp),
        }

    if humidity_values:
        avg_humidity = sum(humidity_values) / len(humidity_values)
        conditions["humidity"] = {
            "value": round(avg_humidity, 1),
            "unit": "%",
            "status": _get_humidity_status(avg_humidity),
        }

    # Generate human-readable conditions
    conditions_parts = []
    if conditions["temperature"]:
        temp_info = conditions["temperature"]
        conditions_parts.append(f"{temp_info['value']}{temp_info['unit']} ({temp_info['status']})")

    if conditions["humidity"]:
        humidity_info = conditions["humidity"]
        conditions_parts.append(
            f"{humidity_info['value']}{humidity_info['unit']} humidity ({humidity_info['status']})"
        )

    conditions["conditions_text"] = (
        ", ".join(conditions_parts) if conditions_parts else "No outdoor sensor data available"
    )

    # Overall status
    if conditions["temperature"] or conditions["humidity"]:
        conditions["status"] = "available"
    else:
        conditions["status"] = "no_outdoor_sensors"

    return conditions


@mcp.tool()
async def get_weather_forecast_daily(days: int = 7, provider: str = "open-meteo") -> dict[str, Any]:
    """
    Get daily weather forecast for the location configured in Loxone.

    Args:
        days: Number of days to forecast (default 7, max 16)
        provider: Weather API provider ("open-meteo" or "openweathermap")

    Returns:
        Daily weather forecast with temperature, precipitation, and conditions
    """
    try:
        context = await _ensure_connection()
    except Exception as e:
        return {"error": f"Failed to connect to Loxone: {e}"}

    # Get location from Loxone structure
    structure = await context.loxone.get_structure_file()

    # Initialize weather client
    weather_client = WeatherForecastClient(provider=provider)

    try:
        # Get location from Loxone config
        lat, lon = await weather_client.get_location_from_loxone(structure)
        weather_client.latitude = lat
        weather_client.longitude = lon

        # Get daily forecast
        forecast = await weather_client.get_daily_forecast(days=days)

        return forecast

    except Exception as e:
        logger.error(f"Failed to get daily weather forecast: {e}")
        return {"error": f"Failed to get weather forecast: {e}"}
    finally:
        await weather_client.close()


@mcp.tool()
async def get_weather_forecast_hourly(
    hours: int = 48, provider: str = "open-meteo"
) -> dict[str, Any]:
    """
    Get hourly weather forecast for the location configured in Loxone.

    Args:
        hours: Number of hours to forecast (default 48, max 384)
        provider: Weather API provider ("open-meteo" or "openweathermap")

    Returns:
        Hourly weather forecast with temperature, precipitation, and conditions
    """
    try:
        context = await _ensure_connection()
    except Exception as e:
        return {"error": f"Failed to connect to Loxone: {e}"}

    # Get location from Loxone structure
    structure = await context.loxone.get_structure_file()

    # Initialize weather client
    weather_client = WeatherForecastClient(provider=provider)

    try:
        # Get location from Loxone config
        lat, lon = await weather_client.get_location_from_loxone(structure)
        weather_client.latitude = lat
        weather_client.longitude = lon

        # Get hourly forecast
        forecast = await weather_client.get_hourly_forecast(hours=hours)

        return forecast

    except Exception as e:
        logger.error(f"Failed to get hourly weather forecast: {e}")
        return {"error": f"Failed to get weather forecast: {e}"}
    finally:
        await weather_client.close()


def _guess_weather_unit(device_name: str, device_type: str) -> str:
    """Guess the unit for a weather device based on name and type."""
    name_lower = device_name.lower()
    type_lower = device_type.lower()

    if "temp" in name_lower or "temperatur" in name_lower or "temperature" in type_lower:
        return "°C"
    elif "humid" in name_lower or "feucht" in name_lower or "humidity" in type_lower:
        return "%"
    elif "wind" in name_lower or "wind" in type_lower:
        return "km/h"
    elif "rain" in name_lower or "regen" in name_lower:
        return "mm"
    elif "pressure" in name_lower or "druck" in name_lower:
        return "hPa"
    else:
        return ""


def _get_temperature_status(temp: float) -> str:
    """Get human-readable temperature status."""
    if temp < 0:
        return "freezing"
    elif temp < 10:
        return "cold"
    elif temp < 20:
        return "cool"
    elif temp < 25:
        return "comfortable"
    elif temp < 30:
        return "warm"
    else:
        return "hot"


def _get_humidity_status(humidity: float) -> str:
    """Get human-readable humidity status."""
    if humidity < 30:
        return "dry"
    elif humidity < 60:
        return "comfortable"
    elif humidity < 80:
        return "humid"
    else:
        return "very humid"


# === Security and Monitoring Tools ===


@mcp.tool()
async def get_security_status() -> dict[str, Any]:
    """
    Get security system status including alarms, motion sensors, and access control.

    Returns:
        Current security status from all security-related devices
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    # Check if security capability is available
    if not _context.capabilities.has_security:
        return {
            "error": "No security devices available",
            "note": "Your Loxone system doesn't have security devices configured",
            "available_features": _get_available_features(_context.capabilities),
        }

    # Look for security-related devices
    security_categories = ["Alarm", "Sicherheit", "Security", "Überwachung", "Zutritt", "Access"]
    security_types = ["Alarm", "MotionDetector", "Switch", "InfoOnlyDigital", "WindowMonitor"]

    security_devices = []

    # Find security devices by category
    for category_name in security_categories:
        if category_name in _context.devices_by_category:
            for device in _context.devices_by_category[category_name]:
                security_devices.append(
                    {**device, "discovery_method": "category", "category_matched": category_name}
                )

    # Find by device type with security keywords in name
    security_keywords = [
        "alarm",
        "motion",
        "bewegung",
        "sensor",
        "überwach",
        "monitor",
        "tür",
        "door",
        "fenster",
        "window",
    ]
    for device_uuid, device in _context.devices.items():
        device_name = device.get("name", "").lower()
        device_type = device.get("type", "")

        if (
            device_type in security_types
            and any(keyword in device_name for keyword in security_keywords)
            and not any(d["uuid"] == device_uuid for d in security_devices)
        ):
            security_devices.append(
                {**device, "discovery_method": "type_and_name", "type_matched": device_type}
            )

    if not security_devices:
        return {
            "error": "No security devices found",
            "note": "Searched categories: Alarm, Sicherheit, Überwachung, Zutritt",
            "suggestion": "Check if your Loxone system has security devices configured",
        }

    # Get current states
    security_status = {
        "timestamp": datetime.datetime.now().isoformat(),
        "overall_status": "unknown",
        "devices": [],
        "summary": {
            "total_devices": len(security_devices),
            "active_alarms": 0,
            "motion_detected": 0,
            "doors_open": 0,
            "windows_open": 0,
        },
    }

    for device in security_devices:
        device_uuid = device["uuid"]
        device_info = {
            "uuid": device_uuid,
            "name": device.get("name", "Unknown"),
            "type": device.get("type", "Unknown"),
            "category": device.get("category_name", "Unknown"),
            "room": device.get("room_name", "Unknown"),
            "security_type": _classify_security_device(device),
        }

        # Get current state
        try:
            state = _context.loxone.get_realtime_state(device_uuid)
            if state is None:
                state = await _context.loxone.send_command(f"jdev/sps/io/{device_uuid}/state")

            device_info["current_state"] = state
            device_info["status"] = _interpret_security_state(device, state)

            # Update summary
            if device_info["status"] == "ALARM":
                security_status["summary"]["active_alarms"] += 1
            elif device_info["status"] == "MOTION":
                security_status["summary"]["motion_detected"] += 1
            elif device_info["status"] == "OPEN" and "door" in device.get("name", "").lower():
                security_status["summary"]["doors_open"] += 1
            elif device_info["status"] == "OPEN" and "window" in device.get("name", "").lower():
                security_status["summary"]["windows_open"] += 1

        except Exception as e:
            device_info["error"] = str(e)

        security_status["devices"].append(device_info)

    # Determine overall status
    if security_status["summary"]["active_alarms"] > 0:
        security_status["overall_status"] = "ALARM"
    elif security_status["summary"]["motion_detected"] > 0:
        security_status["overall_status"] = "MOTION_DETECTED"
    elif (
        security_status["summary"]["doors_open"] > 0
        or security_status["summary"]["windows_open"] > 0
    ):
        security_status["overall_status"] = "UNSECURED"
    else:
        security_status["overall_status"] = "SECURE"

    return security_status


@mcp.tool()
async def get_energy_consumption() -> dict[str, Any]:
    """
    Get energy consumption data from power meters and energy monitoring devices.

    Returns:
        Current energy consumption and power usage information
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    # Check if energy capability is available
    if not _context.capabilities.has_energy:
        return {
            "error": "No energy monitoring devices available",
            "note": "Your Loxone system doesn't have power meters or energy monitoring configured",
            "available_features": _get_available_features(_context.capabilities),
        }

    # Look for energy-related devices
    energy_categories = ["Energie", "Energy", "Strom", "Power", "Verbrauch"]
    energy_types = ["PowerMeter", "EnergyMeter", "Meter"]

    energy_devices = []

    # Find energy devices by category and type
    for category_name in energy_categories:
        if category_name in _context.devices_by_category:
            for device in _context.devices_by_category[category_name]:
                energy_devices.append(device)

    for device_type in energy_types:
        if device_type in _context.devices_by_type:
            for device in _context.devices_by_type[device_type]:
                if not any(d["uuid"] == device["uuid"] for d in energy_devices):
                    energy_devices.append(device)

    # Also look for devices with energy keywords
    energy_keywords = ["power", "energie", "strom", "watt", "kwh", "verbrauch", "zähler", "meter"]
    for device_uuid, device in _context.devices.items():
        device_name = device.get("name", "").lower()
        if any(keyword in device_name for keyword in energy_keywords) and not any(
            d["uuid"] == device_uuid for d in energy_devices
        ):
            energy_devices.append(device)

    if not energy_devices:
        return {
            "error": "No energy monitoring devices found",
            "note": "Searched for power meters, energy meters, and consumption monitors",
            "suggestion": "Check if your Loxone system has energy monitoring configured",
        }

    # Get current readings
    energy_data = {
        "timestamp": datetime.datetime.now().isoformat(),
        "devices": [],
        "summary": {"total_power": 0, "total_energy": 0, "device_count": len(energy_devices)},
    }

    for device in energy_devices:
        device_uuid = device["uuid"]
        device_info = {
            "uuid": device_uuid,
            "name": device.get("name", "Unknown"),
            "type": device.get("type", "Unknown"),
            "category": device.get("category_name", "Unknown"),
            "room": device.get("room_name", "Unknown"),
        }

        # Get current reading
        try:
            state = _context.loxone.get_realtime_state(device_uuid)
            if state is None:
                state = await _context.loxone.send_command(f"jdev/sps/io/{device_uuid}/state")

            device_info["current_value"] = state
            device_info["unit"] = _guess_energy_unit(device.get("name", ""), device.get("type", ""))

            # Add to summary if it's a numeric value
            if isinstance(state, int | float):
                if "watt" in device_info["unit"].lower() or device_info["unit"].lower() == "w":
                    energy_data["summary"]["total_power"] += state
                elif "kwh" in device_info["unit"].lower():
                    energy_data["summary"]["total_energy"] += state

        except Exception as e:
            device_info["error"] = str(e)

        energy_data["devices"].append(device_info)

    return energy_data


@mcp.tool()
async def get_audio_zones() -> dict[str, Any]:
    """
    Get audio system information including zones, sources, and playback status.

    Returns:
        Current audio system status and available zones
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    # Check if audio capability is available
    if not _context.capabilities.has_audio:
        return {
            "error": "No audio devices available",
            "note": "Your Loxone system doesn't have audio zones or music systems configured",
            "available_features": _get_available_features(_context.capabilities),
        }

    # Look for audio-related devices
    audio_categories = ["Audio", "Musik", "Music", "Multimedia"]
    audio_types = ["AudioZone", "Radio", "MediaPlayer", "Intercom"]

    audio_devices = []

    # Find audio devices
    for category_name in audio_categories:
        if category_name in _context.devices_by_category:
            for device in _context.devices_by_category[category_name]:
                audio_devices.append(device)

    for device_type in audio_types:
        if device_type in _context.devices_by_type:
            for device in _context.devices_by_type[device_type]:
                if not any(d["uuid"] == device["uuid"] for d in audio_devices):
                    audio_devices.append(device)

    # Look for audio keywords
    audio_keywords = ["audio", "musik", "music", "radio", "speaker", "lautsprecher", "zone"]
    for device_uuid, device in _context.devices.items():
        device_name = device.get("name", "").lower()
        if any(keyword in device_name for keyword in audio_keywords) and not any(
            d["uuid"] == device_uuid for d in audio_devices
        ):
            audio_devices.append(device)

    if not audio_devices:
        return {
            "error": "No audio devices found",
            "note": "Searched for audio zones, music systems, and speakers",
            "suggestion": "Check if your Loxone system has audio components configured",
        }

    # Get audio status
    audio_status = {
        "timestamp": datetime.datetime.now().isoformat(),
        "zones": [],
        "summary": {"total_zones": len(audio_devices), "playing": 0, "stopped": 0},
    }

    for device in audio_devices:
        device_uuid = device["uuid"]
        zone_info = {
            "uuid": device_uuid,
            "name": device.get("name", "Unknown"),
            "type": device.get("type", "Unknown"),
            "room": device.get("room_name", "Unknown"),
        }

        # Get current state
        try:
            state = _context.loxone.get_realtime_state(device_uuid)
            if state is None:
                state = await _context.loxone.send_command(f"jdev/sps/io/{device_uuid}/state")

            zone_info["current_state"] = state
            zone_info["status"] = _interpret_audio_state(state)

            if zone_info["status"] == "playing":
                audio_status["summary"]["playing"] += 1
            else:
                audio_status["summary"]["stopped"] += 1

        except Exception as e:
            zone_info["error"] = str(e)

        audio_status["zones"].append(zone_info)

    return audio_status


def _classify_security_device(device: dict[str, Any]) -> str:
    """Classify a security device by its characteristics."""
    name = device.get("name", "").lower()
    device_type = device.get("type", "")

    if "motion" in name or "bewegung" in name:
        return "motion_sensor"
    elif "door" in name or "tür" in name:
        return "door_sensor"
    elif "window" in name or "fenster" in name:
        return "window_sensor"
    elif "alarm" in name:
        return "alarm"
    elif device_type == "WindowMonitor":
        return "window_monitor"
    else:
        return "security_device"


def _interpret_security_state(device: dict[str, Any], state: Any) -> str:
    """Interpret security device state."""
    if state == 1 or state == 1.0:
        device_type = _classify_security_device(device)
        if device_type in ["door_sensor", "window_sensor", "window_monitor"]:
            return "CLOSED"
        elif device_type == "motion_sensor":
            return "MOTION"
        else:
            return "ACTIVE"
    elif state == 0 or state == 0.0:
        device_type = _classify_security_device(device)
        if device_type in ["door_sensor", "window_sensor", "window_monitor"]:
            return "OPEN"
        else:
            return "INACTIVE"
    else:
        return f"UNKNOWN({state})"


def _guess_energy_unit(device_name: str, _device_type: str) -> str:
    """Guess the unit for an energy device."""
    name_lower = device_name.lower()

    if "kwh" in name_lower:
        return "kWh"
    elif "watt" in name_lower or "power" in name_lower:
        return "W"
    elif "ampere" in name_lower or "current" in name_lower:
        return "A"
    elif "volt" in name_lower:
        return "V"
    else:
        return ""


def _interpret_audio_state(state: Any) -> str:
    """Interpret audio device state."""
    if isinstance(state, int | float):
        if state > 0:
            return "playing"
        else:
            return "stopped"
    elif isinstance(state, str):
        return state.lower()
    else:
        return "unknown"


@mcp.tool()
async def get_climate_control() -> dict[str, Any]:
    """
    Get climate control information including heating, cooling, and ventilation systems.

    Returns:
        Current HVAC and climate control status
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    # Check if climate capability is available
    if not _context.capabilities.has_climate:
        return {
            "error": "No climate control devices available",
            "note": "Your Loxone system doesn't have HVAC, heating, or climate control configured",
            "available_features": _get_available_features(_context.capabilities),
        }

    # Look for climate-related devices
    climate_categories = [
        "Heizung",
        "Heating",
        "Klima",
        "Climate",
        "Lüftung",
        "Ventilation",
        "HVAC",
    ]
    climate_types = ["IRoomController", "Thermostat", "VentilationController", "HeatingController"]

    climate_devices = []

    # Find climate devices
    for category_name in climate_categories:
        if category_name in _context.devices_by_category:
            for device in _context.devices_by_category[category_name]:
                climate_devices.append(device)

    for device_type in climate_types:
        if device_type in _context.devices_by_type:
            for device in _context.devices_by_type[device_type]:
                if not any(d["uuid"] == device["uuid"] for d in climate_devices):
                    climate_devices.append(device)

    # Look for climate keywords
    climate_keywords = [
        "heizung",
        "heating",
        "klima",
        "climate",
        "temp",
        "thermostat",
        "lüftung",
        "ventilation",
    ]
    for device_uuid, device in _context.devices.items():
        device_name = device.get("name", "").lower()
        if any(keyword in device_name for keyword in climate_keywords) and not any(
            d["uuid"] == device_uuid for d in climate_devices
        ):
            climate_devices.append(device)

    if not climate_devices:
        return {
            "error": "No climate control devices found",
            "note": "Searched for heating, cooling, and ventilation systems",
            "suggestion": "Check if your Loxone system has HVAC components configured",
        }

    # Get climate status
    climate_status = {
        "timestamp": datetime.datetime.now().isoformat(),
        "devices": [],
        "summary": {
            "total_devices": len(climate_devices),
            "heating_zones": 0,
            "cooling_zones": 0,
            "average_temperature": None,
        },
    }

    temperatures = []

    for device in climate_devices:
        device_uuid = device["uuid"]
        device_info = {
            "uuid": device_uuid,
            "name": device.get("name", "Unknown"),
            "type": device.get("type", "Unknown"),
            "category": device.get("category_name", "Unknown"),
            "room": device.get("room_name", "Unknown"),
            "climate_type": _classify_climate_device(device),
        }

        # Get current state
        try:
            state = _context.loxone.get_realtime_state(device_uuid)
            if state is None:
                state = await _context.loxone.send_command(f"jdev/sps/io/{device_uuid}/state")

            device_info["current_state"] = state
            device_info["unit"] = _guess_climate_unit(
                device.get("name", ""), device.get("type", "")
            )

            # Collect temperature readings
            if isinstance(state, int | float) and (
                "temp" in device.get("name", "").lower() or device_info["unit"] == "°C"
            ):
                temperatures.append(state)

            # Count heating/cooling zones
            if device_info["climate_type"] == "heating":
                climate_status["summary"]["heating_zones"] += 1
            elif device_info["climate_type"] == "cooling":
                climate_status["summary"]["cooling_zones"] += 1

        except Exception as e:
            device_info["error"] = str(e)

        climate_status["devices"].append(device_info)

    # Calculate average temperature
    if temperatures:
        climate_status["summary"]["average_temperature"] = {
            "value": round(sum(temperatures) / len(temperatures), 1),
            "unit": "°C",
            "readings": len(temperatures),
        }

    return climate_status


def _classify_climate_device(device: dict[str, Any]) -> str:
    """Classify a climate device by its characteristics."""
    name = device.get("name", "").lower()
    device_type = device.get("type", "")

    if "heating" in name or "heizung" in name:
        return "heating"
    elif "cooling" in name or "kühlung" in name:
        return "cooling"
    elif "ventilation" in name or "lüftung" in name:
        return "ventilation"
    elif "thermostat" in name or device_type == "IRoomController":
        return "thermostat"
    else:
        return "climate_device"


def _guess_climate_unit(device_name: str, _device_type: str) -> str:
    """Guess the unit for a climate device."""
    name_lower = device_name.lower()

    if "temp" in name_lower or "thermostat" in name_lower:
        return "°C"
    elif "humidity" in name_lower or "feucht" in name_lower:
        return "%"
    elif "flow" in name_lower or "durchfluss" in name_lower:
        return "l/min"
    else:
        return ""


# === State Logging Tools ===


@mcp.tool()
async def get_sensor_state_history(uuid: str) -> dict[str, Any]:
    """
    Get complete state change history for a specific sensor.

    Args:
        uuid: Sensor UUID to get history for

    Returns:
        Complete history of state changes for the sensor
    """
    state_logger = get_state_logger()
    history = state_logger.get_sensor_history(uuid)

    if not history:
        return {"error": f"No history found for sensor {uuid}"}

    return {
        "uuid": history.uuid,
        "first_seen": history.first_seen,
        "last_updated": history.last_updated,
        "total_changes": history.total_changes,
        "current_state": history.current_state,
        "recent_events": [
            {
                "timestamp": event.timestamp,
                "old_value": event.old_value,
                "new_value": event.new_value,
                "human_readable": event.human_readable,
                "time_ago": datetime.datetime.now().timestamp() - event.timestamp,
            }
            for event in history.state_events[-20:]  # Last 20 events
        ],
    }


@mcp.tool()
async def get_recent_sensor_changes(limit: int = 50) -> dict[str, Any]:
    """
    Get recent sensor state changes across all sensors.

    Args:
        limit: Maximum number of recent changes to return

    Returns:
        Recent state changes with timestamps
    """
    state_logger = get_state_logger()
    recent_changes = state_logger.get_recent_changes(limit)

    return {
        "total_changes": len(recent_changes),
        "limit": limit,
        "changes": [
            {
                "uuid": event.uuid,
                "timestamp": event.timestamp,
                "old_value": event.old_value,
                "new_value": event.new_value,
                "human_readable": event.human_readable,
                "time_ago": datetime.datetime.now().timestamp() - event.timestamp,
            }
            for event in recent_changes
        ],
    }


@mcp.tool()
async def get_door_window_activity(hours: int = 24) -> dict[str, Any]:
    """
    Get door/window activity summary for the last N hours.

    Args:
        hours: Number of hours to look back (default: 24)

    Returns:
        Activity summary with opens/closes per sensor
    """
    state_logger = get_state_logger()
    activity = state_logger.get_door_window_activity(hours)

    # Add human-readable timestamps
    for _sensor_uuid, sensor_activity in activity.get("sensor_activity", {}).items():
        if sensor_activity.get("last_change"):
            sensor_activity["last_change_ago"] = (
                datetime.datetime.now().timestamp() - sensor_activity["last_change"]
            )

    return activity


@mcp.tool()
async def get_logging_statistics() -> dict[str, Any]:
    """
    Get overall sensor state logging statistics.

    Returns:
        Statistics about logged sensor data
    """
    state_logger = get_state_logger()
    stats = state_logger.get_statistics()

    if stats.get("status") == "no_data":
        return {
            "status": "no_data",
            "message": "No sensor state changes have been logged yet",
            "note": "State changes will be logged automatically once WebSocket monitoring begins",
        }

    # Add human-readable time information
    current_time = datetime.datetime.now().timestamp()
    stats["session_duration"] = current_time - stats["session_start"]

    return stats


# === System Information Tools ===


@mcp.tool()
async def get_available_capabilities() -> dict[str, Any]:
    """
    Get available system capabilities based on your specific Loxone configuration.

    Returns:
        List of available features that can be used with this system
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    capabilities = _context.capabilities

    available_features = {}

    if capabilities.has_lighting:
        available_features["lighting"] = {
            "available": True,
            "device_count": capabilities.light_count,
            "tools": [
                "control_device_by_uuid",
                "get_devices_by_type('LightController')",
                "find_devices('light')",
            ],
            "description": "Control lights, dimmers, and switches",
        }

    if capabilities.has_blinds:
        available_features["blinds_rolladen"] = {
            "available": True,
            "device_count": capabilities.blind_count,
            "tools": [
                "control_device_by_uuid",
                "get_devices_by_type('Jalousie')",
                "find_devices('rolladen')",
            ],
            "description": "Control blinds, shutters, and rolladen (up/down/stop)",
        }

    if capabilities.has_weather:
        available_features["weather"] = {
            "available": True,
            "device_count": capabilities.weather_device_count,
            "tools": ["get_weather_data", "get_outdoor_conditions"],
            "description": "Weather stations and environmental sensors",
        }

    if capabilities.has_security:
        available_features["security"] = {
            "available": True,
            "device_count": capabilities.security_device_count,
            "tools": ["get_security_status"],
            "description": "Motion sensors, door/window sensors, and alarm systems",
        }

    if capabilities.has_energy:
        available_features["energy"] = {
            "available": True,
            "device_count": capabilities.energy_device_count,
            "tools": ["get_energy_consumption"],
            "description": "Power meters and energy monitoring",
        }

    if capabilities.has_audio:
        available_features["audio"] = {
            "available": True,
            "device_count": capabilities.audio_zone_count,
            "tools": ["get_audio_zones"],
            "description": "Multi-room audio and music systems",
        }

    if capabilities.has_climate:
        available_features["climate"] = {
            "available": True,
            "device_count": capabilities.climate_device_count,
            "tools": ["get_climate_control"],
            "description": "HVAC, heating, and climate control",
        }

    # Always available
    available_features["device_discovery"] = {
        "available": True,
        "device_count": len(_context.devices),
        "tools": [
            "discover_all_devices",
            "get_devices_by_category",
            "find_devices",
            "control_device_by_uuid",
        ],
        "description": "Generic device discovery and control",
    }

    return {
        "timestamp": datetime.datetime.now().isoformat(),
        "total_devices": len(_context.devices),
        "available_features": available_features,
        "unavailable_note": "Features not listed are not available in your Loxone configuration",
        "usage_tip": "Use the tools listed for each available feature to interact with your system",
    }


@mcp.tool()
async def get_system_status() -> dict[str, Any]:
    """
    Get overall system status and configuration.

    Returns:
        System status including connection, sensors, and configuration
    """
    if not _context:
        return {"error": "Not connected to Loxone"}

    sensors = _context.discovered_sensors

    # Get logging statistics
    state_logger = get_state_logger()
    logging_stats = state_logger.get_statistics()

    # Get category and type summaries
    top_categories = sorted(
        _context.devices_by_category.items(), key=lambda x: len(x[1]), reverse=True
    )[:5]

    top_device_types = sorted(
        _context.devices_by_type.items(), key=lambda x: len(x[1]), reverse=True
    )[:5]

    return {
        "connection": "Connected",
        "loxone_host": _context.loxone.host,
        "structure": {
            "rooms": len(_context.rooms),
            "devices": len(_context.devices),
            "categories": len(_context.categories),
            "top_categories": {cat: len(devices) for cat, devices in top_categories},
            "top_device_types": {dtype: len(devices) for dtype, devices in top_device_types},
        },
        "detected_capabilities": {
            "lighting": {
                "available": _context.capabilities.has_lighting,
                "count": _context.capabilities.light_count,
            },
            "blinds": {
                "available": _context.capabilities.has_blinds,
                "count": _context.capabilities.blind_count,
            },
            "weather": {
                "available": _context.capabilities.has_weather,
                "count": _context.capabilities.weather_device_count,
            },
            "security": {
                "available": _context.capabilities.has_security,
                "count": _context.capabilities.security_device_count,
            },
            "energy": {
                "available": _context.capabilities.has_energy,
                "count": _context.capabilities.energy_device_count,
            },
            "audio": {
                "available": _context.capabilities.has_audio,
                "count": _context.capabilities.audio_zone_count,
            },
            "climate": {
                "available": _context.capabilities.has_climate,
                "count": _context.capabilities.climate_device_count,
            },
        },
        "dynamic_sensors": {
            "total_discovered": len(sensors),
            "door_window_sensors": len([s for s in sensors if s.is_door_window]),
            "binary_sensors": len([s for s in sensors if s.is_binary]),
        },
        "capabilities": {
            "device_discovery": "Generic - all categories and types",
            "device_control": "UUID-based with intelligent action mapping",
            "real_time_monitoring": _context.loxone.realtime_monitoring,
            "state_logging": logging_stats.get("status") != "no_data",
            "connection_health": "Connected" if _context.loxone.connected else "Disconnected",
            "auto_reconnect": "Enabled",
        },
        "usage_tips": [
            "Use discover_all_devices() to see everything",
            "Use get_devices_by_category() for specific categories",
            "Use find_devices('search_term') to locate devices",
            "Use control_device_by_uuid() to control any device",
            "Use get_weather_data() for weather station information",
            "Use get_security_status() for alarm and motion sensors",
            "Use get_energy_consumption() for power monitoring",
            "Use get_audio_zones() for music system status",
            "Use get_climate_control() for HVAC and heating systems",
            "Use get_weather_forecast_daily() for daily weather forecast",
            "Use get_weather_forecast_hourly() for hourly weather forecast",
        ],
        "timestamp": datetime.datetime.now().isoformat(),
    }


def _get_available_features(capabilities: SystemCapabilities) -> list[str]:
    """Get list of available feature names for error messages."""
    available = []
    if capabilities.has_lighting:
        available.append("lighting")
    if capabilities.has_blinds:
        available.append("blinds")
    if capabilities.has_weather:
        available.append("weather")
    if capabilities.has_security:
        available.append("security")
    if capabilities.has_energy:
        available.append("energy")
    if capabilities.has_audio:
        available.append("audio")
    if capabilities.has_climate:
        available.append("climate")

    return available if available else ["basic device control"]


def _detect_system_capabilities(
    devices: dict[str, Any],
    devices_by_category: dict[str, list[dict[str, Any]]],
    devices_by_type: dict[str, list[dict[str, Any]]],
) -> SystemCapabilities:
    """Detect available system capabilities based on actual devices."""
    capabilities = SystemCapabilities()

    # Detect lighting capabilities
    light_types = ["LightController", "Dimmer", "Switch"]
    light_categories = ["Beleuchtung", "Lighting", "Licht", "Light"]

    for light_type in light_types:
        if light_type in devices_by_type:
            capabilities.light_count += len(devices_by_type[light_type])

    for category in light_categories:
        if category in devices_by_category:
            for device in devices_by_category[category]:
                if not any(
                    device["uuid"] == d["uuid"] for d in devices_by_type.get("LightController", [])
                ):
                    capabilities.light_count += 1

    capabilities.has_lighting = capabilities.light_count > 0

    # Detect blinds/rolladen capabilities
    blind_types = ["Jalousie", "Blind", "Shutter"]
    blind_categories = ["Beschattung", "Shading", "Rolladen", "Jalousie"]

    for blind_type in blind_types:
        if blind_type in devices_by_type:
            capabilities.blind_count += len(devices_by_type[blind_type])

    for category in blind_categories:
        if category in devices_by_category:
            capabilities.blind_count += len(devices_by_category[category])

    capabilities.has_blinds = capabilities.blind_count > 0

    # Detect weather capabilities
    weather_categories = ["Wetter", "Weather", "Klima", "Außen", "Outdoor"]
    weather_types = ["WeatherServer", "TemperatureSensor", "HumiditySensor", "WindSensor"]
    weather_keywords = ["temp", "humidity", "wind", "rain", "weather", "außen", "outdoor"]

    for category in weather_categories:
        if category in devices_by_category:
            capabilities.weather_device_count += len(devices_by_category[category])

    for device_type in weather_types:
        if device_type in devices_by_type:
            capabilities.weather_device_count += len(devices_by_type[device_type])

    # Count weather devices by name keywords
    for device in devices.values():
        device_name = device.get("name", "").lower()
        if any(keyword in device_name for keyword in weather_keywords):
            capabilities.weather_device_count += 1

    capabilities.has_weather = capabilities.weather_device_count > 0

    # Detect security capabilities
    security_categories = ["Alarm", "Sicherheit", "Security", "Überwachung", "Zutritt", "Access"]
    security_types = ["Alarm", "MotionDetector", "WindowMonitor", "InfoOnlyDigital"]

    for category in security_categories:
        if category in devices_by_category:
            capabilities.security_device_count += len(devices_by_category[category])

    for device_type in security_types:
        if device_type in devices_by_type:
            capabilities.security_device_count += len(devices_by_type[device_type])

    capabilities.has_security = capabilities.security_device_count > 0

    # Detect energy capabilities
    energy_categories = ["Energie", "Energy", "Strom", "Power", "Verbrauch"]
    energy_types = ["PowerMeter", "EnergyMeter", "Meter"]

    for category in energy_categories:
        if category in devices_by_category:
            capabilities.energy_device_count += len(devices_by_category[category])

    for device_type in energy_types:
        if device_type in devices_by_type:
            capabilities.energy_device_count += len(devices_by_type[device_type])

    capabilities.has_energy = capabilities.energy_device_count > 0

    # Detect audio capabilities
    audio_categories = ["Audio", "Musik", "Music", "Multimedia"]
    audio_types = ["AudioZone", "Radio", "MediaPlayer", "Intercom"]

    for category in audio_categories:
        if category in devices_by_category:
            capabilities.audio_zone_count += len(devices_by_category[category])

    for device_type in audio_types:
        if device_type in devices_by_type:
            capabilities.audio_zone_count += len(devices_by_type[device_type])

    capabilities.has_audio = capabilities.audio_zone_count > 0

    # Detect climate capabilities
    climate_categories = [
        "Heizung",
        "Heating",
        "Klima",
        "Climate",
        "Lüftung",
        "Ventilation",
        "HVAC",
    ]
    climate_types = ["IRoomController", "Thermostat", "VentilationController", "HeatingController"]

    for category in climate_categories:
        if category in devices_by_category:
            capabilities.climate_device_count += len(devices_by_category[category])

    for device_type in climate_types:
        if device_type in devices_by_type:
            capabilities.climate_device_count += len(devices_by_type[device_type])

    capabilities.has_climate = capabilities.climate_device_count > 0

    # Count total sensors (any device that might provide sensor data)
    capabilities.sensor_count = len(devices)
    capabilities.has_sensors = capabilities.sensor_count > 0

    return capabilities


def _format_capabilities(capabilities: SystemCapabilities) -> str:
    """Format capabilities for logging."""
    active_features = []

    if capabilities.has_lighting:
        active_features.append(f"Lighting({capabilities.light_count})")
    if capabilities.has_blinds:
        active_features.append(f"Blinds({capabilities.blind_count})")
    if capabilities.has_weather:
        active_features.append(f"Weather({capabilities.weather_device_count})")
    if capabilities.has_security:
        active_features.append(f"Security({capabilities.security_device_count})")
    if capabilities.has_energy:
        active_features.append(f"Energy({capabilities.energy_device_count})")
    if capabilities.has_audio:
        active_features.append(f"Audio({capabilities.audio_zone_count})")
    if capabilities.has_climate:
        active_features.append(f"Climate({capabilities.climate_device_count})")

    return ", ".join(active_features) if active_features else "Basic device control only"


async def _shutdown_handler() -> None:
    """Handle graceful shutdown on signals."""
    logger.info("Shutdown signal received, cleaning up...")
    global _context
    if _context and _context.loxone:
        try:
            await _context.loxone.close()
        except Exception as e:
            logger.warning(f"Error during signal shutdown: {e}")


def _setup_signal_handlers() -> None:
    """Set up signal handlers for graceful shutdown."""
    if sys.platform != "win32":
        # Unix signal handling
        def signal_handler(signum: int, _frame: Any) -> None:
            logger.info(f"Received signal {signum}, initiating shutdown...")
            # Create new event loop if needed for cleanup
            try:
                loop = asyncio.get_event_loop()
            except RuntimeError:
                loop = asyncio.new_event_loop()
                asyncio.set_event_loop(loop)

            if loop.is_running():
                # Schedule shutdown in running loop
                loop.create_task(_shutdown_handler())  # noqa: RUF006
                # Don't await here as we're in a signal handler
            else:
                # Run shutdown in new loop
                loop.run_until_complete(_shutdown_handler())

        signal.signal(signal.SIGINT, signal_handler)
        signal.signal(signal.SIGTERM, signal_handler)


@mcp.prompt()
async def loxone_system_overview() -> str:
    """Provides a comprehensive overview of your Loxone home automation system.

    This prompt helps you understand your current Loxone setup, including:
    - Available rooms and their devices
    - System capabilities and features
    - Device categories and types
    - Current system status

    Use this when you want to get oriented with your Loxone system or need
    a starting point for automation and control tasks.
    """
    try:
        # Initialize connection if needed
        if not _context.loxone.connected:
            await _ensure_connection()

        # Get system overview
        system_status = await get_system_status()
        rooms = await list_rooms()
        capabilities = await get_available_capabilities()
        categories = await get_all_categories_overview()

        overview = f"""# Loxone System Overview

## System Status
{system_status.get('content', 'Unable to retrieve system status')}

## Available Rooms ({len(rooms.get('rooms', []))})
{rooms.get('content', 'No rooms found')}

## System Capabilities
{capabilities.get('content', 'No capabilities detected')}

## Device Categories
{categories.get('content', 'No categories found')}

## Getting Started
Use these tools to explore and control your system:
- `list_rooms()` - See all rooms and their devices
- `get_room_devices(room_name)` - Get devices in a specific room
- `control_device(device_name, action)` - Control lights, blinds, etc.
- `get_system_status()` - Check overall system health
- `discover_all_devices()` - Refresh device list

For more advanced features, explore climate control, energy monitoring,
security status, and weather data tools.
"""
        return overview

    except Exception as e:
        return f"""# Loxone System Overview - Connection Error

Unable to connect to your Loxone system: {e!s}

## Troubleshooting Steps:
1. Verify your Loxone credentials are configured: `uvx --from . loxone-mcp setup`
2. Check if your Miniserver is accessible on the network
3. Ensure your Loxone system is powered on and connected
4. Try running with debug logging:
   `LOXONE_LOG_LEVEL=DEBUG uv run mcp dev src/loxone_mcp/server.py`

## Available Commands (without connection):
- System setup and credential management
- Basic MCP server operations
- Diagnostic tools

Once connected, you'll have access to full device control and monitoring capabilities.
"""


if __name__ == "__main__":
    _setup_signal_handlers()
    import uvicorn

    uvicorn.run(mcp, host="127.0.0.1", port=8000)
