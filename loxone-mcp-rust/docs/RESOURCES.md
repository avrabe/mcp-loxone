# Loxone MCP Resources Documentation

This document describes the resource URI scheme and available resources for the Loxone MCP server.

## Overview

The Loxone MCP server implements the Model Context Protocol (MCP) Resources specification, providing structured read-only access to Loxone home automation data. Resources use a custom URI scheme (`loxone://`) to identify and access different types of data.

## URI Scheme

All resources follow the pattern: `loxone://category[/subcategory][/{parameter}][?query_params]`

### Components:
- **Scheme**: Always `loxone://`
- **Category**: Main resource category (e.g., `rooms`, `devices`, `system`)
- **Subcategory**: Optional subcategory for nested resources
- **Parameters**: Dynamic path parameters enclosed in `{}`
- **Query Parameters**: Optional filters and modifiers

## Available Resources

### Room Resources

#### List All Rooms
- **URI**: `loxone://rooms`
- **Description**: Returns a list of all rooms with device counts and information
- **Response**: Array of room objects with name, UUID, and device statistics

#### Get Room Devices
- **URI**: `loxone://rooms/{roomName}/devices`
- **Description**: Returns all devices in a specific room
- **Parameters**:
  - `roomName`: Name of the room (URL-encoded if contains spaces)
- **Query Parameters**:
  - `type`: Filter by device type
  - `category`: Filter by device category
- **Example**: `loxone://rooms/Living%20Room/devices?type=Switch`

### Device Resources

#### All Devices
- **URI**: `loxone://devices/all`
- **Description**: Complete list of all devices in the system
- **Query Parameters**:
  - `room`: Filter by room name
  - `category`: Filter by category
  - `sort`: Sort order (`name`, `type`, `room`, `category`, `-name` for descending)
  - `limit`: Maximum number of results
  - `offset`: Pagination offset

#### Devices by Type
- **URI**: `loxone://devices/type/{deviceType}`
- **Description**: All devices filtered by type
- **Parameters**:
  - `deviceType`: Device type (e.g., `Switch`, `Dimmer`, `Jalousie`)
- **Query Parameters**:
  - `room`: Filter by room
  - `limit`: Maximum results

#### Devices by Category
- **URI**: `loxone://devices/category/{category}`
- **Description**: All devices filtered by category
- **Parameters**:
  - `category`: Device category (`lighting`, `blinds`, `climate`, `sensors`, `audio`)
- **Query Parameters**:
  - `room`: Filter by room
  - `type`: Further filter by type

### System Resources

#### System Status
- **URI**: `loxone://system/status`
- **Description**: Overall system status and health information
- **Response**: System health metrics, connection status, and statistics

#### System Capabilities
- **URI**: `loxone://system/capabilities`
- **Description**: Available system capabilities and features
- **Response**: List of supported device types, control actions, and features

#### Categories Overview
- **URI**: `loxone://system/categories`
- **Description**: Overview of all device categories with counts
- **Response**: Category statistics and example devices

### Audio Resources

#### Audio Zones
- **URI**: `loxone://audio/zones`
- **Description**: All audio zones and their current status
- **Response**: List of audio zones with playback state and volume

#### Audio Sources
- **URI**: `loxone://audio/sources`
- **Description**: Available audio sources and their status
- **Response**: List of configured audio sources

### Sensor Resources

#### Door/Window Sensors
- **URI**: `loxone://sensors/door-window`
- **Description**: All door and window sensors with current state
- **Response**: List of sensors with open/closed status

#### Temperature Sensors
- **URI**: `loxone://sensors/temperature`
- **Description**: All temperature sensors and their current readings
- **Response**: List of sensors with temperature values and units

#### Discovered Sensors
- **URI**: `loxone://sensors/discovered`
- **Description**: Dynamically discovered sensors with metadata
- **Response**: List of discovered sensors with type, location, and last update

## Query Parameter Reference

### Common Parameters
- `limit`: Maximum number of results (integer)
- `offset`: Skip first N results for pagination (integer)
- `sort`: Sort field with optional `-` prefix for descending order

### Filter Parameters
- `room`: Filter by room name (string, URL-encoded)
- `type`: Filter by device type (string)
- `category`: Filter by category (string)
- `state`: Filter by state (varies by resource)

## Response Format

All resources return JSON data with the following structure:

```json
{
  "uri": "loxone://resource/path",
  "timestamp": "2024-01-20T10:30:00Z",
  "data": {
    // Resource-specific data
  },
  "metadata": {
    "total_count": 100,
    "returned_count": 20,
    "cache_ttl": 300
  }
}
```

## Caching

Resources implement intelligent caching with TTL values based on data volatility:

| Resource Type | Default TTL | Description |
|--------------|-------------|-------------|
| Room lists | 3600s | Room structure rarely changes |
| Device lists | 600s | Device configuration is relatively stable |
| System status | 60s | Health checks need to be current |
| Sensor data | 30s | Sensor readings change frequently |
| Audio status | 10s | Playback state is highly dynamic |

## Error Handling

Invalid resource requests return standard MCP error responses:

```json
{
  "error": {
    "code": -32602,
    "message": "Invalid resource URI: {details}"
  }
}
```

Common error codes:
- `-32602`: Invalid parameters (bad URI format, missing required parameters)
- `-32002`: Resource not found
- `-32603`: Internal error accessing resource

## Examples

### Get all lights in the living room:
```
loxone://rooms/Living%20Room/devices?type=LightController
```

### Get all devices sorted by name (descending):
```
loxone://devices/all?sort=-name&limit=50
```

### Get temperature sensors in the bedroom:
```
loxone://sensors/temperature?room=Bedroom
```

### Get all blinds/rolladen devices:
```
loxone://devices/category/blinds
```

## Integration with MCP

Resources are accessed through the standard MCP protocol:

1. List available resources:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "resources/list"
}
```

2. Read a specific resource:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "resources/read",
  "params": {
    "uri": "loxone://rooms/Kitchen/devices"
  }
}
```

## Best Practices

1. **Use Resources for Read-Only Data**: Resources are optimized for data retrieval. Use tools for actions that modify state.

2. **Leverage Caching**: Resources are cached automatically. Frequent reads of the same resource are efficient.

3. **Pagination for Large Results**: Use `limit` and `offset` parameters to handle large datasets efficiently.

4. **URL Encoding**: Always URL-encode parameters that may contain spaces or special characters.

5. **Error Handling**: Implement proper error handling for invalid URIs and missing resources.

## Migration from Tools

The following read-only tools have been migrated to resources:

| Old Tool | New Resource |
|----------|--------------|
| `list_rooms` | `loxone://rooms` |
| `get_room_devices` | `loxone://rooms/{roomName}/devices` |
| `discover_all_devices` | `loxone://devices/all` |
| `get_devices_by_type` | `loxone://devices/type/{type}` |
| `get_devices_by_category` | `loxone://devices/category/{category}` |
| `get_system_status` | `loxone://system/status` |
| `get_available_capabilities` | `loxone://system/capabilities` |
| `get_all_categories_overview` | `loxone://system/categories` |
| `get_audio_zones` | `loxone://audio/zones` |
| `get_audio_sources` | `loxone://audio/sources` |
| `get_all_door_window_sensors` | `loxone://sensors/door-window` |
| `get_temperature_sensors` | `loxone://sensors/temperature` |
| `list_discovered_sensors` | `loxone://sensors/discovered` |