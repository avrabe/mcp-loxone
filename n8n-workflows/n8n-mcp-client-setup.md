# n8n MCP Client Configuration Guide

This guide explains how to configure n8n to consume the Loxone MCP server using the MCP Client nodes.

## Prerequisites

1. n8n instance (v1.0+ with MCP support)
2. Loxone MCP server running with HTTP/SSE transport
3. API key for authentication

## Installation

### 1. Install MCP Nodes for n8n

```bash
# In your n8n installation directory
npm install @n8n/n8n-nodes-mcp

# Or using n8n's community nodes
# Go to Settings → Community Nodes → Install → Search for "MCP"
```

### 2. Configure MCP Server Connection

#### Create MCP Credentials

1. In n8n, go to **Credentials** → **New**
2. Search for "MCP Server"
3. Configure:
   ```
   Name: Loxone MCP Server
   Server URL: http://localhost:8080
   Transport: HTTP/SSE
   Authentication: API Key
   API Key: your-secret-api-key
   ```

## Available MCP Nodes

### 1. MCP Tools Node

Execute MCP tool calls:

```javascript
{
  "tool": "list_rooms",
  "arguments": {}
}
```

**Common Tools**:
- `list_rooms` - Get all rooms
- `get_room_devices` - Get devices in a room
- `control_device` - Control a specific device
- `control_room_lights` - Control all lights in a room
- `get_sensor_values` - Get sensor readings

### 2. MCP SSE Trigger Node

Subscribe to real-time events:

```javascript
{
  "events": [
    "device.state_changed",
    "sensor.value_updated",
    "security.alarm_triggered",
    "energy.threshold_exceeded"
  ]
}
```

### 3. MCP Prompt Node

Send natural language commands:

```javascript
{
  "prompt": "Turn on the lights in the living room and set temperature to 22°C"
}
```

## Example Workflows

### Basic Device Control

```json
{
  "nodes": [
    {
      "type": "@n8n/n8n-nodes-mcp.mcpTools",
      "parameters": {
        "tool": "control_device",
        "arguments": {
          "device_id": "uuid-here",
          "action": "on"
        }
      }
    }
  ]
}
```

### Room Automation

```json
{
  "nodes": [
    {
      "type": "@n8n/n8n-nodes-mcp.mcpTools",
      "parameters": {
        "tool": "get_room_devices",
        "arguments": {
          "room_name": "{{ $json.room }}"
        }
      }
    },
    {
      "type": "n8n-nodes-base.code",
      "parameters": {
        "jsCode": "// Filter lights\nreturn $input.all().filter(device => \n  device.json.type === 'light'\n);"
      }
    },
    {
      "type": "@n8n/n8n-nodes-mcp.mcpTools",
      "parameters": {
        "tool": "control_device",
        "arguments": {
          "device_id": "{{ $json.uuid }}",
          "action": "on"
        }
      }
    }
  ]
}
```

### Event-Driven Automation

```json
{
  "nodes": [
    {
      "type": "@n8n/n8n-nodes-mcp.mcpSseTrigger",
      "parameters": {
        "events": ["sensor.motion_detected"],
        "filters": {
          "room": "Entrance"
        }
      }
    },
    {
      "type": "n8n-nodes-base.if",
      "parameters": {
        "conditions": {
          "boolean": [
            {
              "value1": "={{ $json.armed }}",
              "value2": true
            }
          ]
        }
      }
    },
    {
      "type": "@n8n/n8n-nodes-mcp.mcpTools",
      "parameters": {
        "tool": "trigger_alarm",
        "arguments": {
          "zone": "{{ $json.zone }}"
        }
      }
    }
  ]
}
```

## Advanced Configuration

### Connection Options

```javascript
{
  "server": {
    "url": "http://loxone-mcp.local:8080",
    "timeout": 30000,
    "reconnect": true,
    "reconnectInterval": 5000
  },
  "sse": {
    "heartbeat": 30000,
    "compression": true
  }
}
```

### Error Handling

```javascript
{
  "retry": {
    "attempts": 3,
    "delay": 1000,
    "backoff": 2
  },
  "fallback": {
    "onError": "continue",
    "defaultValue": null
  }
}
```

### Batch Operations

```javascript
{
  "batch": {
    "enabled": true,
    "size": 10,
    "timeout": 5000
  }
}
```

## Authentication Methods

### 1. API Key (Recommended)

```
Authorization: Bearer your-api-key
```

### 2. JWT Token

```javascript
{
  "auth": {
    "type": "jwt",
    "token": "eyJhbGc...",
    "refresh": true
  }
}
```

### 3. mTLS (Production)

```javascript
{
  "auth": {
    "type": "mtls",
    "cert": "/path/to/client.crt",
    "key": "/path/to/client.key",
    "ca": "/path/to/ca.crt"
  }
}
```

## Performance Optimization

### 1. Connection Pooling

```javascript
{
  "pool": {
    "min": 2,
    "max": 10,
    "idle": 10000
  }
}
```

### 2. Caching

```javascript
{
  "cache": {
    "enabled": true,
    "ttl": 300,
    "tools": ["list_rooms", "get_room_devices"]
  }
}
```

### 3. Compression

```javascript
{
  "compression": {
    "enabled": true,
    "type": "gzip",
    "level": 6
  }
}
```

## Monitoring

### Health Checks

```javascript
// Add to workflow
{
  "type": "@n8n/n8n-nodes-mcp.mcpHealth",
  "parameters": {
    "interval": 60000,
    "timeout": 5000
  }
}
```

### Metrics Collection

```javascript
{
  "metrics": {
    "enabled": true,
    "endpoint": "/metrics",
    "include": ["latency", "errors", "throughput"]
  }
}
```

### Logging

```javascript
{
  "logging": {
    "level": "info",
    "format": "json",
    "fields": ["timestamp", "tool", "duration", "status"]
  }
}
```

## Troubleshooting

### Common Issues

1. **Connection Refused**
   ```
   Error: connect ECONNREFUSED 127.0.0.1:8080
   ```
   - Check if MCP server is running
   - Verify URL and port
   - Check firewall settings

2. **Authentication Failed**
   ```
   Error: 401 Unauthorized
   ```
   - Verify API key
   - Check token expiration
   - Ensure correct auth header format

3. **Tool Not Found**
   ```
   Error: Unknown tool: control_device
   ```
   - List available tools first
   - Check tool name spelling
   - Verify server version

### Debug Mode

Enable debug logging:

```javascript
{
  "debug": {
    "enabled": true,
    "verbose": true,
    "includePayload": true
  }
}
```

### Test Connection

```javascript
// Test workflow node
{
  "type": "@n8n/n8n-nodes-mcp.mcpTest",
  "parameters": {
    "tests": [
      "connection",
      "authentication", 
      "tools",
      "sse"
    ]
  }
}
```

## Best Practices

1. **Use Connection Pooling** for high-frequency operations
2. **Implement Circuit Breakers** for resilience
3. **Cache Frequently Used Data** (room lists, device states)
4. **Handle SSE Reconnections** gracefully
5. **Log All Security Events** for audit trails
6. **Version Your Tool Calls** for compatibility
7. **Implement Idempotency** for critical operations
8. **Use Correlation IDs** for request tracking

## Security Considerations

1. **Always Use HTTPS** in production
2. **Rotate API Keys** regularly
3. **Implement Rate Limiting**
4. **Use Least Privilege** principle for access
5. **Audit All Actions** especially security-related
6. **Encrypt Sensitive Data** in workflows
7. **Validate All Inputs** before sending to MCP
8. **Monitor for Anomalies** in usage patterns

## Example Integration

Complete example connecting multiple services:

```javascript
// Weather → Loxone → Slack
{
  "trigger": "weather.api",
  "condition": "temperature > 30",
  "actions": [
    {
      "mcp": {
        "tool": "control_room_blinds",
        "args": { "room": "all", "action": "close" }
      }
    },
    {
      "mcp": {
        "tool": "set_temperature",
        "args": { "target": 24 }
      }
    },
    {
      "slack": {
        "message": "Heat wave detected. Blinds closed and AC adjusted."
      }
    }
  ]
}
```