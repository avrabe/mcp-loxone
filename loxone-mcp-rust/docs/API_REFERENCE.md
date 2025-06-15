# Loxone MCP API Reference

## Overview

The Loxone MCP server provides 30+ tools organized into functional categories for comprehensive home automation control.

## Authentication

All API requests require authentication using API keys:

```bash
# Using X-API-Key header
curl -H "X-API-Key: lmcp_admin_001_abc123def456" http://localhost:3001/api/devices

# Using Authorization Bearer
curl -H "Authorization: Bearer lmcp_admin_001_abc123def456" http://localhost:3001/api/devices
```

## Tool Categories

### 🎵 Audio Control (12 tools)

| Tool | Description | Required Role | Parameters |
|------|-------------|---------------|------------|
| `get_audio_zones` | List all audio zones | Monitor+ | None |
| `get_zone_status` | Get specific zone status | Monitor+ | `zone_id` |
| `set_zone_volume` | Set volume (0-100) | Operator+ | `zone_id`, `volume` |
| `play_audio` | Play audio in zone | Operator+ | `zone_id` |
| `pause_audio` | Pause audio in zone | Operator+ | `zone_id` |
| `stop_audio` | Stop audio in zone | Operator+ | `zone_id` |
| `next_track` | Skip to next track | Operator+ | `zone_id` |
| `previous_track` | Skip to previous track | Operator+ | `zone_id` |
| `set_audio_source` | Change audio source | Operator+ | `zone_id`, `source_id` |
| `get_audio_favorites` | List favorite stations | Monitor+ | None |
| `play_favorite` | Play favorite station | Operator+ | `zone_id`, `favorite_id` |
| `sync_audio_zones` | Sync multiple zones | Operator+ | `zone_ids[]` |

### 🌡️ Climate Control (8 tools)

| Tool | Description | Required Role | Parameters |
|------|-------------|---------------|------------|
| `get_climate_zones` | List all climate zones | Monitor+ | None |
| `get_zone_temperature` | Get current temperature | Monitor+ | `zone_id` |
| `set_target_temperature` | Set target temperature | Operator+ | `zone_id`, `temperature` |
| `get_hvac_mode` | Get HVAC mode | Monitor+ | `zone_id` |
| `set_hvac_mode` | Set HVAC mode | Operator+ | `zone_id`, `mode` |
| `get_climate_schedule` | Get zone schedule | Monitor+ | `zone_id` |
| `override_climate` | Temporary override | Operator+ | `zone_id`, `temperature`, `duration` |
| `reset_climate_override` | Cancel override | Operator+ | `zone_id` |

### 💡 Device Control (10 tools)

| Tool | Description | Required Role | Parameters |
|------|-------------|---------------|------------|
| `list_devices` | List all devices | Monitor+ | `type?`, `room?` |
| `get_device_state` | Get device state | Monitor+ | `device_id` |
| `turn_on` | Turn device on | Operator+ | `device_id` |
| `turn_off` | Turn device off | Operator+ | `device_id` |
| `toggle` | Toggle device state | Operator+ | `device_id` |
| `set_dimmer` | Set dimmer level | Operator+ | `device_id`, `level` |
| `open_blind` | Open blind/shutter | Operator+ | `device_id` |
| `close_blind` | Close blind/shutter | Operator+ | `device_id` |
| `set_blind_position` | Set blind position | Operator+ | `device_id`, `position` |
| `stop_blind` | Stop blind movement | Operator+ | `device_id` |

### 🏠 Room Management (6 tools)

| Tool | Description | Required Role | Parameters |
|------|-------------|---------------|------------|
| `list_rooms` | List all rooms | Monitor+ | None |
| `get_room_devices` | Get devices in room | Monitor+ | `room_id` |
| `get_room_state` | Get room state summary | Monitor+ | `room_id` |
| `room_all_on` | Turn on all devices | Operator+ | `room_id` |
| `room_all_off` | Turn off all devices | Operator+ | `room_id` |
| `set_room_scene` | Activate room scene | Operator+ | `room_id`, `scene_id` |

### 🔒 Security System (6 tools)

| Tool | Description | Required Role | Parameters |
|------|-------------|---------------|------------|
| `get_alarm_status` | Get alarm system status | Monitor+ | None |
| `arm_alarm` | Arm alarm system | Admin | `mode` |
| `disarm_alarm` | Disarm alarm system | Admin | `code` |
| `get_access_logs` | View access logs | Admin | `limit?` |
| `trigger_panic` | Trigger panic alarm | Admin | None |
| `test_alarm` | Test alarm system | Admin | None |

### 📊 Sensors & Monitoring (8 tools)

| Tool | Description | Required Role | Parameters |
|------|-------------|---------------|------------|
| `list_sensors` | List all sensors | Monitor+ | `type?` |
| `get_sensor_value` | Get sensor reading | Monitor+ | `sensor_id` |
| `get_temperature_sensors` | List temperature sensors | Monitor+ | None |
| `get_motion_sensors` | List motion sensors | Monitor+ | None |
| `get_door_window_sensors` | List door/window sensors | Monitor+ | None |
| `get_sensor_history` | Get sensor history | Monitor+ | `sensor_id`, `hours?` |
| `configure_sensor_alerts` | Set sensor alerts | Operator+ | `sensor_id`, `thresholds` |
| `test_sensor` | Test sensor operation | Operator+ | `sensor_id` |

### ⚡ Energy Management (4 tools)

| Tool | Description | Required Role | Parameters |
|------|-------------|---------------|------------|
| `get_power_consumption` | Current power usage | Monitor+ | None |
| `get_energy_statistics` | Energy statistics | Monitor+ | `period` |
| `get_device_consumption` | Device power usage | Monitor+ | `device_id` |
| `set_power_limit` | Set power limit | Admin | `limit_watts` |

### 🔄 Automation & Scenes (6 tools)

| Tool | Description | Required Role | Parameters |
|------|-------------|---------------|------------|
| `list_scenes` | List all scenes | Monitor+ | None |
| `activate_scene` | Activate a scene | Operator+ | `scene_id` |
| `list_automations` | List automations | Monitor+ | None |
| `enable_automation` | Enable automation | Operator+ | `automation_id` |
| `disable_automation` | Disable automation | Operator+ | `automation_id` |
| `test_automation` | Test automation | Operator+ | `automation_id` |

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