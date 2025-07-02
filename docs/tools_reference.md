# MCP Tools Reference

This document provides a complete reference for all 17 MCP tools available in the Loxone MCP Server.

## Overview

The Loxone MCP server implements a clean separation between **tools** (for actions that modify state) and **resources** (for read-only data access). This follows the MCP specification and provides better caching and organization.

- **17 Tools**: For device control and state modification
- **25+ Resources**: For data retrieval (see [resources.md](resources.md))

## Tools by Category

### Device Control (2 tools)

#### `control_device`
Controls a specific device by UUID.

**Parameters**:
- `device_id` (string, required): Device UUID
- `action` (string, required): Action to perform (on, off, toggle, etc.)
- `value` (number, optional): Optional value for the action (e.g., brightness level)

**Valid Actions**: on, off, toggle, up, down, stop

**Example**:
```json
{
  "device_id": "0cd88f1e-0156-7a9f-ffff403fb0c34b9e",
  "action": "on"
}
```

#### `control_multiple_devices`
Controls multiple devices simultaneously.

**Parameters**:
- `devices` (array, required): Array of device UUIDs
- `action` (string, required): Action to perform on all devices

**Example**:
```json
{
  "devices": ["uuid1", "uuid2", "uuid3"],
  "action": "off"
}
```

### Lighting Control (3 tools)

#### `control_lights_unified`
Unified lighting control with scope-based targeting.

**Parameters**:
- `scope` (string, required): "device", "room", or "all"
- `target` (string, optional): Device ID or room name (required for device/room scope)
- `action` (string, required): "on", "off", "dim", "bright", or "toggle"
- `brightness` (integer, optional): Brightness level (0-100) for dim/bright actions

**Example**:
```json
{
  "scope": "room",
  "target": "Living Room",
  "action": "dim",
  "brightness": 50
}
```

#### `control_room_lights` (Legacy)
Controls all lights in a specific room.

**Parameters**:
- `room` (string, required): Name of the room
- `action` (string, required): "on" or "off"

#### `control_all_lights` (Legacy)
Controls all lights in the entire system.

**Parameters**:
- `action` (string, required): "on" or "off"

### Blinds/Rolladen Control (4 tools)

#### `control_rolladen_unified`
Unified rolladen/blinds control with scope-based targeting.

**Parameters**:
- `scope` (string, required): "device", "room", "system", or "all"
- `target` (string, optional): Device ID/name or room name (required for device/room scope)
- `action` (string, required): "up", "down", "stop", "position", "hoch", "runter", "stopp"
- `position` (integer, optional): Position percentage (0-100) where 0=fully up, 100=fully down

**Example**:
```json
{
  "scope": "room",
  "target": "Bedroom",
  "action": "position",
  "position": 75
}
```

#### `discover_rolladen_capabilities`
Discovers all rolladen/blinds capabilities and devices in the system.

**Parameters**: None

**Returns**: Information about available rolladen devices and their capabilities

#### `control_room_rolladen` (Legacy)
Controls all rolladen/blinds in a specific room.

**Parameters**:
- `room` (string, required): Name of the room
- `action` (string, required): "up", "down", or "stop"

#### `control_all_rolladen` (Legacy)
Controls all rolladen/blinds in the entire system.

**Parameters**:
- `action` (string, required): "up", "down", or "stop"

### Climate Control (2 tools)

#### `set_room_temperature`
Sets the target temperature for a room's climate controller.

**Parameters**:
- `room_name` (string, required): Name of the room to control
- `temperature` (number, required): Target temperature in Celsius (5.0 - 35.0)

**Example**:
```json
{
  "room_name": "Living Room",
  "temperature": 22.5
}
```

#### `set_room_mode`
Controls heating/cooling mode for a room's climate controller.

**Parameters**:
- `room_name` (string, required): Name of the room to control
- `mode` (string, required): "heating", "cooling", "auto", or "off"

### Audio Control (2 tools)

#### `control_audio_zone`
Controls an audio zone (play, stop, volume control).

**Parameters**:
- `zone_name` (string, required): Name of the audio zone to control
- `action` (string, required): "play", "stop", "pause", "volume", "mute", "unmute", "next", "previous", "start"
- `value` (number, optional): Value for volume actions (0-100)

**Example**:
```json
{
  "zone_name": "Living Room",
  "action": "volume",
  "value": 75
}
```

#### `set_audio_volume`
Sets volume for an audio zone.

**Parameters**:
- `zone_name` (string, required): Name of the audio zone
- `volume` (number, required): Volume level (0-100)

### Security Control (2 tools)

#### `arm_alarm`
Arms the alarm system for security monitoring.

**Parameters**:
- `mode` (string, optional): Alarm mode to set ("home", "away", "full"), default: "away"

**Example**:
```json
{
  "mode": "away"
}
```

#### `disarm_alarm`
Disarms the alarm system.

**Parameters**: None

### Workflow Management (2 tools)

#### `create_workflow`
Creates a new automation workflow by chaining multiple tools together.

**Parameters**:
- `name` (string, required): Name of the workflow
- `description` (string, required): Description of what the workflow does
- `steps` (array, required): Array of workflow steps to execute
- `timeout_seconds` (number, optional): Maximum execution time in seconds
- `variables` (object, optional): Initial variables for the workflow

**Example**:
```json
{
  "name": "Morning Routine",
  "description": "Turn on lights and open blinds",
  "steps": [
    {"type": "tool", "name": "control_all_lights", "args": {"action": "on"}},
    {"type": "tool", "name": "control_all_rolladen", "args": {"action": "up"}}
  ]
}
```

#### `execute_workflow_demo`
Executes a demonstration workflow to show automation capabilities.

**Parameters**:
- `workflow_name` (string, required): Name of the demo workflow ("home_automation", "morning_routine", "security_check")
- `variables` (object, optional): Variables to pass to the workflow

## Resources for Read-Only Data

The following operations are now handled by **resources** instead of tools:

| Operation | Resource URI |
|-----------|--------------|
| List rooms | `loxone://rooms` |
| Get room devices | `loxone://rooms/{room}/devices` |
| Get room overview | `loxone://rooms/{room}/overview` |
| List all devices | `loxone://devices/all` |
| Get devices by category | `loxone://devices/category/{category}` |
| Get system capabilities | `loxone://system/capabilities` |
| Get system categories | `loxone://system/categories` |
| Get audio zones | `loxone://audio/zones` |
| Get audio sources | `loxone://audio/sources` |
| Get door/window sensors | `loxone://sensors/door-window` |
| Get temperature sensors | `loxone://sensors/temperature` |
| Get weather data | `loxone://weather/current` |
| Get energy consumption | `loxone://energy/consumption` |
| Get alarm status | `loxone://security/status` |
| Get climate data | `loxone://climate/overview` |
| Get predefined workflows | `loxone://workflows/predefined` |
| Get workflow examples | `loxone://workflows/examples` |

See [resources.md](resources.md) for detailed resource documentation.

## Error Handling

All tools follow consistent error handling:

- **Invalid parameters**: Returns error with parameter validation details
- **Device not found**: Returns error with device UUID
- **Connection errors**: Returns error with connection details
- **Permission denied**: Returns error for insufficient permissions

## Rate Limiting

Tools are subject to rate limiting based on user role:
- **Admin**: 1000 requests/minute
- **Operator**: 100 requests/minute
- **Viewer**: 10 requests/minute (read-only access only)

## Migration Notes

This server has been updated to follow MCP best practices by separating tools and resources:

- **Tools**: Used for actions that modify device state
- **Resources**: Used for read-only data access with caching

This improves performance through intelligent caching and provides a cleaner API structure that follows the MCP specification.