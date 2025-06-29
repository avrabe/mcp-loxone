# MCP Tools Reference

This document provides a complete reference for all 34 MCP tools available in the Loxone MCP Server.

## Table of Contents

- [Room Management](#room-management)
- [Device Control](#device-control)
- [Lighting](#lighting)
- [Blinds/Rolladen](#blindsrolladen)
- [Climate Control](#climate-control)
- [Audio System](#audio-system)
- [Sensors](#sensors)
- [Weather](#weather)
- [Energy Management](#energy-management)
- [Security](#security)
- [Workflows](#workflows)

## Room Management

### `list_rooms`
Lists all configured rooms in the Loxone system.

**Parameters**: None

**Returns**:
```json
{
  "rooms": [
    {
      "uuid": "0f869a3f-0155-8b3f-ffff403fb0c34b9e",
      "name": "Living Room",
      "devices_count": 12
    }
  ]
}
```

### `get_room_devices`
Gets all devices in a specific room.

**Parameters**:
- `room_name` (string, required): Name of the room

**Returns**:
```json
{
  "room": "Living Room",
  "devices": [
    {
      "uuid": "0cd88f1e-0156-7a9f-ffff403fb0c34b9e",
      "name": "Ceiling Light",
      "type": "LightController",
      "states": {}
    }
  ]
}
```

### `get_room_overview`
Provides a comprehensive overview of all rooms and their devices.

**Parameters**: None

**Returns**: Complete room and device hierarchy

## Device Control

### `discover_all_devices`
Discovers all available devices in the Loxone system.

**Parameters**: None

**Returns**: List of all devices with their capabilities

### `control_device`
Controls a specific device by UUID.

**Parameters**:
- `uuid` (string, required): Device UUID
- `command` (string, required): Command to send (e.g., "on", "off", "50")

**Returns**: Command execution result

### `get_devices_by_category`
Filters devices by their category/type.

**Parameters**:
- `category` (string, required): Device category (e.g., "lights", "blinds", "sensors")

**Returns**: Filtered device list

### `control_multiple_devices`
Controls multiple devices in a single operation.

**Parameters**:
- `devices` (array, required): Array of {uuid, command} objects

**Returns**: Batch operation results

## Lighting

### `control_lights_unified`
Unified lighting control with scope-based targeting.

**Parameters**:
- `scope` (string, required): "all", "room", or "device"
- `target` (string, optional): Room name or device UUID (required for room/device scope)
- `command` (string, required): "on", "off", or brightness value (0-100)

**Example**:
```json
{
  "scope": "room",
  "target": "Living Room",
  "command": "50"
}
```

### `get_light_scenes`
Retrieves available lighting scenes.

**Parameters**:
- `room` (string, optional): Filter scenes by room

**Returns**: List of available scenes

### `set_light_scene`
Activates a specific lighting scene.

**Parameters**:
- `scene_id` (string, required): Scene identifier
- `room` (string, optional): Apply to specific room only

## Blinds/Rolladen

### `control_rolladen_unified`
Unified control for blinds and shutters.

**Parameters**:
- `scope` (string, required): "all", "room", or "device"
- `room` (string, optional): Room name (required for room scope)
- `uuid` (string, optional): Device UUID (required for device scope)
- `command` (string, required): "up", "down", "stop", "shade", or position (0-100)

### `discover_rolladen_capabilities`
Discovers capabilities of blinds/shutters in the system.

**Parameters**: None

**Returns**: List of rolladen devices with their features

### `control_all_rolladen` (Legacy)
Controls all blinds at once.

**Parameters**:
- `command` (string, required): "up", "down", or "shade"

### `control_room_rolladen` (Legacy)
Controls all blinds in a specific room.

**Parameters**:
- `room_name` (string, required): Room name
- `command` (string, required): "up", "down", or "shade"

## Climate Control

### `get_climate_control`
Gets the main climate control system status.

**Parameters**: None

**Returns**: HVAC system status and settings

### `get_room_climate`
Retrieves climate data for a specific room.

**Parameters**:
- `room_name` (string, required): Room name

**Returns**:
```json
{
  "room": "Living Room",
  "temperature": 21.5,
  "target_temperature": 22.0,
  "humidity": 45,
  "mode": "comfort"
}
```

### `set_room_temperature`
Sets the target temperature for a room.

**Parameters**:
- `room_name` (string, required): Room name
- `temperature` (number, required): Target temperature in Celsius

### `get_temperature_readings`
Gets all temperature sensor readings.

**Parameters**: None

**Returns**: Map of sensor locations to temperature values

### `set_room_mode`
Sets the climate mode for a room.

**Parameters**:
- `room_name` (string, required): Room name
- `mode` (string, required): "comfort", "eco", or "off"

## Audio System

### `get_audio_zones`
Lists all configured audio zones.

**Parameters**: None

**Returns**: List of audio zones with current status

### `control_audio_zone`
Controls playback in an audio zone.

**Parameters**:
- `zone_id` (string, required): Audio zone identifier
- `action` (string, required): "play", "pause", "stop", "next", "previous"

### `get_audio_sources`
Lists available audio sources.

**Parameters**: None

**Returns**: List of audio sources (radio, streaming services, etc.)

### `set_audio_volume`
Sets volume for an audio zone.

**Parameters**:
- `zone_id` (string, required): Audio zone identifier
- `volume` (number, required): Volume level (0-100)

## Sensors

### `get_all_door_window_sensors`
Retrieves status of all door and window sensors.

**Parameters**: None

**Returns**:
```json
{
  "sensors": [
    {
      "location": "Front Door",
      "state": "closed",
      "last_change": "2024-01-29T10:30:00Z"
    }
  ]
}
```

### `get_temperature_sensors`
Gets readings from all temperature sensors.

**Parameters**: None

**Returns**: Temperature readings by location

### `get_motion_sensors`
Retrieves motion sensor status.

**Parameters**: None

**Returns**: Motion detection status by location

### `discover_sensor_capabilities`
Discovers all sensor types and their capabilities.

**Parameters**: None

**Returns**: Comprehensive sensor inventory

## Weather

### `get_weather_station_data`
Retrieves data from connected weather station.

**Parameters**: None

**Returns**:
```json
{
  "temperature": 15.2,
  "humidity": 68,
  "pressure": 1013.25,
  "wind_speed": 12.5,
  "wind_direction": "NW",
  "rain": 0.0
}
```

## Energy Management

### `get_energy_consumption`
Retrieves current energy consumption data.

**Parameters**:
- `timeframe` (string, optional): "current", "day", "week", "month"

**Returns**: Energy consumption metrics

## Security

### `get_alarm_status`
Gets the current alarm system status.

**Parameters**: None

**Returns**:
```json
{
  "armed": false,
  "mode": "disarmed",
  "zones": []
}
```

### `arm_alarm`
Arms the alarm system.

**Parameters**:
- `mode` (string, required): "away" or "home"
- `zones` (array, optional): Specific zones to arm

### `disarm_alarm`
Disarms the alarm system.

**Parameters**:
- `code` (string, required): Disarm code

## Workflows

### `create_workflow`
Creates a new automation workflow.

**Parameters**:
- `name` (string, required): Workflow name
- `triggers` (array, required): Trigger conditions
- `actions` (array, required): Actions to execute

### `execute_workflow_demo`
Executes a demonstration workflow.

**Parameters**:
- `workflow_name` (string, required): Name of demo workflow

### `list_predefined_workflows`
Lists all predefined workflows.

**Parameters**: None

**Returns**: Available workflow templates

### `get_workflow_examples`
Provides workflow examples and templates.

**Parameters**: None

**Returns**: Example workflow configurations

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
- **Viewer**: 10 requests/minute (read-only tools)

## Notes

- All UUID parameters should be in Loxone's standard format
- Temperature values are in Celsius
- Percentage values are 0-100
- Times are in ISO 8601 format
- Some tools may return cached data for performance