# MCP HTTP Streaming Client Integration Guide

<!--
SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
-->

## Overview

This guide provides detailed instructions for MCP client developers on how to connect to and interact with MCP servers using the **HTTP Streaming transport mode**. This is the modern MCP transport method that provides real-time bidirectional communication over HTTP.

## Quick Start

```bash
# Start the MCP server in HTTP mode
cargo run --bin loxone-mcp-server -- http --port 3001

# Test connection with MCP Inspector
npx @modelcontextprotocol/inspector@latest http://localhost:3001
```

## HTTP Streaming Protocol Details

### 1. Connection Initialization

**Endpoint**: `POST http://localhost:3001/mcp`

**Headers**:
```http
Content-Type: application/json
Accept: application/json
User-Agent: your-mcp-client/1.0.0
```

**Initial Request** (Initialize):
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {
      "roots": {
        "listChanged": false
      },
      "sampling": {}
    },
    "clientInfo": {
      "name": "YourMCPClient",
      "version": "1.0.0"
    }
  }
}
```

**Expected Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2024-11-05",
    "capabilities": {
      "tools": {
        "listChanged": false
      },
      "resources": {
        "subscribe": true,
        "listChanged": false
      },
      "prompts": {
        "listChanged": false
      },
      "logging": {}
    },
    "serverInfo": {
      "name": "Loxone MCP Server",
      "version": "1.0.0"
    },
    "instructions": ""
  }
}
```

### 2. Session Management

After successful initialization, include the session identifier in subsequent requests:

**Headers for all subsequent requests**:
```http
Content-Type: application/json
Accept: application/json
Mcp-Session-Id: <session-id-from-server>
```

### 3. Discovering Available Tools

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list",
  "params": {}
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "tools": [
      {
        "name": "list_rooms",
        "description": "List all rooms with device counts and capabilities",
        "inputSchema": {
          "type": "object",
          "properties": {},
          "required": []
        }
      },
      {
        "name": "control_device",
        "description": "Control a specific Loxone device by name or UUID",
        "inputSchema": {
          "type": "object",
          "properties": {
            "device": {
              "type": "string",
              "description": "Device name or UUID to control"
            },
            "action": {
              "type": "string",
              "description": "Action to perform (on/off/up/down/stop/dim/bright)"
            },
            "value": {
              "type": "number",
              "description": "Optional value for dimming (0-100)"
            }
          },
          "required": ["device", "action"]
        }
      }
    ],
    "nextCursor": ""
  }
}
```

### 4. Calling Tools

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "list_rooms",
    "arguments": {}
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\"status\":\"success\",\"data\":[{\"uuid\":\"room-1\",\"name\":\"Living Room\",\"device_count\":5,\"devices_by_category\":{\"lighting\":3,\"blinds\":2}}],\"timestamp\":\"2025-01-15T10:30:00Z\"}"
      }
    ],
    "isError": false
  }
}
```

### 5. Working with Resources

**List available resources**:
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "resources/list",
  "params": {}
}
```

**Read a specific resource**:
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "resources/read",
  "params": {
    "uri": "loxone://devices/living-room"
  }
}
```

### 6. Error Handling

**Error Response Format**:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "error": {
    "code": -32602,
    "message": "Invalid params",
    "data": {
      "details": "Device 'unknown-device' not found"
    }
  }
}
```

**Common Error Codes**:
- `-32700`: Parse error
- `-32600`: Invalid Request
- `-32601`: Method not found
- `-32602`: Invalid params
- `-32603`: Internal error

## Implementation Examples

### JavaScript/Node.js Client

```javascript
class McpHttpClient {
  constructor(baseUrl) {
    this.baseUrl = baseUrl;
    this.sessionId = null;
    this.requestId = 1;
  }

  async initialize() {
    const response = await fetch(`${this.baseUrl}/mcp`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Accept': 'application/json'
      },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: this.requestId++,
        method: 'initialize',
        params: {
          protocolVersion: '2024-11-05',
          capabilities: {
            roots: { listChanged: false },
            sampling: {}
          },
          clientInfo: {
            name: 'CustomMCPClient',
            version: '1.0.0'
          }
        }
      })
    });

    const result = await response.json();
    
    // Extract session ID from response headers if provided
    this.sessionId = response.headers.get('Mcp-Session-Id');
    
    return result;
  }

  async callTool(name, arguments = {}) {
    const headers = {
      'Content-Type': 'application/json',
      'Accept': 'application/json'
    };
    
    if (this.sessionId) {
      headers['Mcp-Session-Id'] = this.sessionId;
    }

    const response = await fetch(`${this.baseUrl}/mcp`, {
      method: 'POST',
      headers,
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: this.requestId++,
        method: 'tools/call',
        params: { name, arguments }
      })
    });

    return await response.json();
  }

  async listTools() {
    return await this.makeRequest('tools/list', {});
  }

  async makeRequest(method, params) {
    const headers = {
      'Content-Type': 'application/json',
      'Accept': 'application/json'
    };
    
    if (this.sessionId) {
      headers['Mcp-Session-Id'] = this.sessionId;
    }

    const response = await fetch(`${this.baseUrl}/mcp`, {
      method: 'POST',
      headers,
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: this.requestId++,
        method,
        params
      })
    });

    return await response.json();
  }
}

// Usage example
async function main() {
  const client = new McpHttpClient('http://localhost:3001');
  
  // Initialize connection
  const initResult = await client.initialize();
  console.log('Initialized:', initResult);
  
  // List available tools
  const tools = await client.listTools();
  console.log('Available tools:', tools);
  
  // Call a tool
  const rooms = await client.callTool('list_rooms');
  console.log('Rooms:', rooms);
  
  // Control a device
  const deviceControl = await client.callTool('control_device', {
    device: 'Living Room Light',
    action: 'on'
  });
  console.log('Device control result:', deviceControl);
}
```

### Python Client

```python
import requests
import json

class McpHttpClient:
    def __init__(self, base_url):
        self.base_url = base_url
        self.session_id = None
        self.request_id = 1
    
    def initialize(self):
        headers = {
            'Content-Type': 'application/json',
            'Accept': 'application/json'
        }
        
        payload = {
            'jsonrpc': '2.0',
            'id': self.request_id,
            'method': 'initialize',
            'params': {
                'protocolVersion': '2024-11-05',
                'capabilities': {
                    'roots': {'listChanged': False},
                    'sampling': {}
                },
                'clientInfo': {
                    'name': 'PythonMCPClient',
                    'version': '1.0.0'
                }
            }
        }
        
        self.request_id += 1
        
        response = requests.post(f'{self.base_url}/mcp', 
                               headers=headers, 
                               json=payload)
        
        # Extract session ID if provided
        self.session_id = response.headers.get('Mcp-Session-Id')
        
        return response.json()
    
    def call_tool(self, name, arguments=None):
        if arguments is None:
            arguments = {}
            
        return self._make_request('tools/call', {
            'name': name,
            'arguments': arguments
        })
    
    def list_tools(self):
        return self._make_request('tools/list', {})
    
    def _make_request(self, method, params):
        headers = {
            'Content-Type': 'application/json',
            'Accept': 'application/json'
        }
        
        if self.session_id:
            headers['Mcp-Session-Id'] = self.session_id
        
        payload = {
            'jsonrpc': '2.0',
            'id': self.request_id,
            'method': method,
            'params': params
        }
        
        self.request_id += 1
        
        response = requests.post(f'{self.base_url}/mcp', 
                               headers=headers, 
                               json=payload)
        
        return response.json()

# Usage example
if __name__ == '__main__':
    client = McpHttpClient('http://localhost:3001')
    
    # Initialize
    init_result = client.initialize()
    print('Initialized:', json.dumps(init_result, indent=2))
    
    # List tools
    tools = client.list_tools()
    print('Tools:', json.dumps(tools, indent=2))
    
    # Call a tool
    rooms = client.call_tool('list_rooms')
    print('Rooms:', json.dumps(rooms, indent=2))
```

## Transport-Specific Considerations

### Content Negotiation

The server supports multiple response formats based on the `Accept` header:

1. **JSON Responses** (`Accept: application/json`):
   - Single request/response
   - Suitable for traditional REST-like interactions

2. **Server-Sent Events** (`Accept: text/event-stream`):
   - Real-time streaming responses
   - Event-based communication

3. **Mixed Headers** (`Accept: application/json, text/event-stream`):
   - Server prioritizes the first format listed
   - Client preference determines response type

### Session Management

- Sessions are maintained server-side for connection state
- Include `Mcp-Session-Id` header for session continuity
- Sessions may timeout after periods of inactivity

### Rate Limiting

The server implements rate limiting to prevent abuse:

- Default: 100 requests per minute per client
- Rate limit headers included in responses:
  ```
  X-RateLimit-Limit: 100
  X-RateLimit-Remaining: 95
  X-RateLimit-Reset: 1642176000
  ```

## Security Considerations

### Authentication

For production deployments, configure API key authentication:

```bash
# Create an API key
cargo run --bin loxone-mcp-auth create --name "MyClient" --role operator

# Include in requests
curl -H "Authorization: Bearer your-api-key" \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
     http://localhost:3001/mcp
```

### Input Validation

All tool parameters are validated against JSON schemas:

- Invalid parameters return `-32602` (Invalid params) errors
- UUID validation for device identifiers
- Range validation for numeric values (e.g., 0-100 for dimming)

### CORS

For web-based clients, configure CORS appropriately:

```rust
// Server configuration
cors_config: CorsConfig {
    allow_origins: vec!["https://your-domain.com".to_string()],
    allow_methods: vec!["POST".to_string()],
    allow_headers: vec!["Content-Type".to_string(), "Accept".to_string()],
}
```

## Troubleshooting

### Common Issues

1. **"Connection refused"**:
   - Ensure server is running: `cargo run --bin loxone-mcp-server -- http --port 3001`
   - Check firewall settings
   - Verify port availability

2. **"Invalid JSON-RPC format"**:
   - Ensure proper JSON-RPC 2.0 format
   - Include required fields: `jsonrpc`, `id`, `method`
   - Check Content-Type header

3. **"Method not found"**:
   - Use `tools/list` to discover available methods
   - Check method name spelling and case sensitivity

4. **"Invalid params"**:
   - Validate parameters against tool's `inputSchema`
   - Ensure required parameters are provided
   - Check parameter types and formats

### Debug Mode

Enable debug logging for detailed troubleshooting:

```bash
RUST_LOG=debug cargo run --bin loxone-mcp-server -- http --port 3001
```

### Testing with curl

```bash
# Test initialization
curl -X POST http://localhost:3001/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "capabilities": {"roots": {"listChanged": false}},
      "clientInfo": {"name": "curl-test", "version": "1.0.0"}
    }
  }'

# Test tool call
curl -X POST http://localhost:3001/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/list",
    "params": {}
  }'
```

## Performance Optimization

### Request Coalescing

Multiple identical requests are automatically coalesced:
- Reduces server load
- Improves response times
- Transparent to client

### Response Caching

Frequently requested data is cached server-side:
- Tool responses cached for 30 seconds
- Resource data cached for 60 seconds
- Automatic cache invalidation on state changes

### Batch Operations

For multiple device operations, use batch tools when available:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "control_room_devices",
    "arguments": {
      "room": "Living Room",
      "action": "off",
      "device_types": ["lighting"]
    }
  }
}
```

## Integration with Popular MCP Clients

### MCP Inspector

```bash
# Test with official MCP Inspector
npx @modelcontextprotocol/inspector@latest http://localhost:3001
```

### Claude Desktop

Add to Claude Desktop configuration:
```json
{
  "mcpServers": {
    "loxone": {
      "command": "curl",
      "args": [
        "-X", "POST",
        "-H", "Content-Type: application/json",
        "-H", "Accept: application/json",
        "http://localhost:3001/mcp"
      ]
    }
  }
}
```

This guide should provide comprehensive instructions for any MCP client developer looking to integrate with your HTTP streaming server implementation.