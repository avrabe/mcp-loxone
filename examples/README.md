# Loxone MCP Examples

This directory contains example configurations and workflows for integrating the Loxone MCP server with various tools.

## Files

### n8n-loxone-workflow.json
An example n8n workflow that demonstrates:
- Listing all rooms
- Toggling lights in the first room
- Getting devices in a room
- Processing the results with a function node

**To use:**
1. Start the SSE server: `../run-sse-server.sh`
2. Open n8n
3. Import the workflow (Settings → Import from File)
4. Execute the workflow

### loxone-mcp-sse.service
Systemd service file for running the SSE server as a system service on Linux.

**To install:**
```bash
# Copy to systemd directory
sudo cp loxone-mcp-sse.service /etc/systemd/system/

# Edit the file and update YOUR_USERNAME
sudo nano /etc/systemd/system/loxone-mcp-sse.service

# Reload systemd
sudo systemctl daemon-reload

# Enable and start the service
sudo systemctl enable loxone-mcp-sse
sudo systemctl start loxone-mcp-sse

# Check status
sudo systemctl status loxone-mcp-sse
```

## n8n Integration Examples

### Basic HTTP Request
```javascript
// Node: HTTP Request
{
  "url": "http://localhost:8080/webhook",
  "method": "POST",
  "body": {
    "method": "list_rooms",
    "params": {},
    "id": 1
  }
}
```

### Control Lights with Parameters
```javascript
// Using expressions from previous nodes
{
  "url": "http://localhost:8080/webhook",
  "method": "POST",
  "body": {
    "method": "control_light",
    "params": {
      "room": "{{ $json.room_name }}",
      "action": "{{ $json.action }}",
      "brightness": "{{ $json.brightness }}"
    },
    "id": 2
  }
}
```

### Webhook Trigger
1. Create a Webhook node in n8n
2. Copy the webhook URL
3. Configure external systems to POST to that URL
4. The SSE server can forward events to n8n

### Schedule-based Automation
```javascript
// Cron node: Every day at sunset
// Followed by HTTP Request:
{
  "method": "control_room_rolladen",
  "params": {
    "room": "all",
    "action": "down"
  }
}
```

## Home Assistant Integration

While not included here, you can integrate with Home Assistant using:
- RESTful switches
- RESTful sensors
- Command line switches
- Node-RED (with n8n webhook)

Example REST switch configuration:
```yaml
switch:
  - platform: rest
    name: Living Room Lights
    resource: http://localhost:8080/webhook
    method: POST
    body_on: '{"method":"control_room_lights","params":{"room":"Living Room","action":"on"},"id":1}'
    body_off: '{"method":"control_room_lights","params":{"room":"Living Room","action":"off"},"id":1}'
    headers:
      Content-Type: application/json
```

## Testing Tools

Use these commands to test the integration:

```bash
# List all available methods
curl http://localhost:8080/api/methods | jq

# Get all rooms
curl -X POST http://localhost:8080/webhook \
  -H "Content-Type: application/json" \
  -d '{"method":"list_rooms","params":{},"id":1}' | jq

# Control lights
curl -X POST http://localhost:8080/webhook \
  -H "Content-Type: application/json" \
  -d '{
    "method":"control_room_lights",
    "params":{"room":"Kitchen","action":"on"},
    "id":2
  }' | jq
```

## Docker Deployment

See the main project's Docker files:
- `../Dockerfile.sse` - Docker image for the SSE server
- `../docker-compose.sse.yml` - Docker Compose configuration

## Troubleshooting

1. **Check server logs**: Run with `LOXONE_LOG_LEVEL=DEBUG`
2. **Test endpoints**: Use `../test-n8n-integration.py`
3. **Verify CORS**: Check browser console for CORS errors
4. **Monitor requests**: All requests are logged by the server
