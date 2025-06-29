# Loxone MCP API Reference

## Overview

The Loxone MCP server provides 17 tools for device control and 25+ resources for data access, organized into functional categories for comprehensive home automation control.

## Authentication

All API requests require authentication using API keys:

```bash
# Using X-API-Key header
curl -H "X-API-Key: lmcp_admin_001_abc123def456" http://localhost:3001/api/devices

# Using Authorization Bearer
curl -H "Authorization: Bearer lmcp_admin_001_abc123def456" http://localhost:3001/api/devices
```

## Tool Categories

The server implements 17 action-based tools, organized by functionality:

### Device Control (2 tools)
- `control_device` - Control individual devices by UUID
- `control_multiple_devices` - Batch device operations

### Lighting Control (3 tools)
- `control_lights_unified` - Unified lighting control with scope targeting
- `control_room_lights` - Control all lights in a specific room
- `control_all_lights` - Control all lights in the entire system

### Blinds/Rolladen Control (4 tools)
- `control_rolladen_unified` - Unified rolladen control with scope targeting
- `discover_rolladen_capabilities` - Discover rolladen devices and capabilities
- `control_room_rolladen` - Control all rolladen in a specific room
- `control_all_rolladen` - Control all rolladen in the entire system

### Climate Control (2 tools)
- `set_room_temperature` - Set target temperature for room climate control
- `set_room_mode` - Set heating/cooling mode for room climate control

### Audio Control (2 tools)
- `control_audio_zone` - Control audio playback in zones
- `set_audio_volume` - Set volume for audio zones

### Security Control (2 tools)
- `arm_alarm` - Arm the security alarm system
- `disarm_alarm` - Disarm the security alarm system

### Workflow Management (2 tools)
- `create_workflow` - Create new automation workflows
- `execute_workflow_demo` - Execute demonstration workflows

## Resources for Read-Only Data

For data retrieval, use the 25+ resources available via the MCP Resources protocol:

### Room Data
- `loxone://rooms` - List all rooms
- `loxone://rooms/{room}/devices` - Devices in specific room
- `loxone://rooms/{room}/overview` - Room overview with statistics

### Device Data
- `loxone://devices/all` - All devices
- `loxone://devices/category/{category}` - Devices by category
- `loxone://devices/type/{type}` - Devices by type

### System Information
- `loxone://system/status` - System status
- `loxone://system/capabilities` - System capabilities
- `loxone://system/categories` - Category overview

### Sensor Data
- `loxone://sensors/door-window` - Door/window sensors
- `loxone://sensors/temperature` - Temperature sensors
- `loxone://sensors/motion` - Motion sensors
- And many more...

See [resources.md](resources.md) for complete resource documentation.

## HTTP Endpoints

### MCP Protocol Endpoints

```bash
# Main MCP message endpoint (for MCP Inspector)
POST /message
Content-Type: application/json
X-API-Key: your-api-key

{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "list_devices",
    "arguments": {}
  },
  "id": 1
}

# Server-Sent Events stream
GET /sse
X-API-Key: your-api-key
```

### Admin Endpoints

```bash
# API Key Management UI
GET /admin/keys

# API Key REST endpoints
GET    /admin/api/keys      # List all keys
POST   /admin/api/keys      # Create new key
PUT    /admin/api/keys/:id  # Update key
DELETE /admin/api/keys/:id  # Delete key

# System Status
GET /admin/status           # System status
GET /admin/rate-limits      # Rate limit status
```

### Monitoring Endpoints

```bash
# Dashboard
GET /dashboard/             # Real-time monitoring dashboard
GET /dashboard/ws           # WebSocket for live updates
GET /dashboard/api/status   # Dashboard API status
GET /dashboard/api/data     # Dashboard data

# Metrics
GET /metrics                # Prometheus metrics
GET /health                 # Health check
```

## Error Responses

All errors follow a consistent format:

```json
{
  "error": {
    "code": "INVALID_API_KEY",
    "message": "The provided API key is invalid or expired",
    "details": {
      "key_id": "lmcp_***_***"
    }
  }
}
```

Common error codes:
- `INVALID_API_KEY` - Invalid or missing API key
- `INSUFFICIENT_PERMISSIONS` - Role lacks required permissions
- `RATE_LIMIT_EXCEEDED` - Too many requests
- `INVALID_PARAMETERS` - Missing or invalid parameters
- `DEVICE_NOT_FOUND` - Device ID not found
- `CONNECTION_ERROR` - Cannot connect to Loxone Miniserver

## Rate Limiting

Default rate limits by role:
- **Admin**: 1000 requests/minute
- **Operator**: 500 requests/minute  
- **Monitor**: 200 requests/minute
- **Device**: 100 requests/minute

Rate limit headers:
```
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1673884800
```

## WebSocket Events

Connect to `/dashboard/ws` for real-time updates:

```javascript
const ws = new WebSocket('ws://localhost:3001/dashboard/ws');

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Event:', data.type, data.payload);
};
```

Event types:
- `device_state_change` - Device state updated
- `sensor_update` - Sensor value changed
- `alarm_event` - Security system event
- `energy_update` - Power consumption update
- `system_event` - System-level event

## Examples

### Turn on all lights in living room
```bash
curl -X POST http://localhost:3001/message \
  -H "Content-Type: application/json" \
  -H "X-API-Key: lmcp_operator_001_abc123" \
  -d '{
    "jsonrpc": "2.0",
    "method": "tools/call",
    "params": {
      "name": "room_all_on",
      "arguments": {
        "room_id": "living_room"
      }
    },
    "id": 1
  }'
```

### Get current temperature
```bash
curl -X POST http://localhost:3001/message \
  -H "Content-Type: application/json" \
  -H "X-API-Key: lmcp_monitor_001_xyz789" \
  -d '{
    "jsonrpc": "2.0",
    "method": "tools/call",
    "params": {
      "name": "get_zone_temperature",
      "arguments": {
        "zone_id": "bedroom"
      }
    },
    "id": 1
  }'
```

### Set dimmer to 50%
```bash
curl -X POST http://localhost:3001/message \
  -H "Content-Type: application/json" \
  -H "X-API-Key: lmcp_operator_001_abc123" \
  -d '{
    "jsonrpc": "2.0",
    "method": "tools/call",
    "params": {
      "name": "set_dimmer",
      "arguments": {
        "device_id": "bedroom_light",
        "level": 50
      }
    },
    "id": 1
  }'
```