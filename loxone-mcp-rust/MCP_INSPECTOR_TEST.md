# Testing with MCP Inspector

## Server Status

The Loxone MCP Server is now running with proper HTTP/SSE transport that is compatible with MCP Inspector.

### Endpoints

- **SSE Endpoint**: `http://localhost:3002/sse` - Server-Sent Events stream
- **Messages Endpoint**: `http://localhost:3002/messages` - POST endpoint for JSON-RPC messages
- **Health Check**: `http://localhost:3002/health` - GET endpoint for health status

### Running the Server

```bash
# Run in development mode (no Loxone connection required)
cargo run --bin loxone-mcp-server -- http --port 3002 --dev-mode

# Or run with actual Loxone credentials
cargo run --bin loxone-mcp-server -- http --port 3002
```

### Testing with MCP Inspector

1. Open MCP Inspector in your browser
2. Connect to: `http://localhost:3002`
3. The server will:
   - Respond to the SSE connection with an "endpoint" event
   - Provide the messages URL for posting JSON-RPC requests
   - Stream responses back through the SSE connection

### Manual Testing

```bash
# Test SSE endpoint
curl -N http://localhost:3002/sse

# Test health endpoint
curl http://localhost:3002/health

# Test initialize request
curl -X POST http://localhost:3002/messages \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"1.0.0","capabilities":{"roots":{"listChangedNotifications":false},"experimental":{},"sampling":{}}},"id":1}'
```

### Key Changes Made

1. **Switched from StreamableHttpTransport to HttpTransport**
   - StreamableHttpTransport was returning JSON responses instead of SSE streams
   - HttpTransport properly implements SSE with "endpoint" events

2. **Proper SSE Implementation**
   - Sends "endpoint" event first with the messages URL
   - Streams "message" events for responses
   - Includes keep-alive pings

3. **MCP Inspector Compatibility**
   - The server now follows the exact SSE protocol that MCP Inspector expects
   - All responses are sent through the SSE stream, not as HTTP responses

### Architecture

```
MCP Inspector → GET /sse → Server sends "endpoint" event
             → POST /messages → Server processes request
             ← SSE stream ← Server sends response as "message" event
```

This implementation follows the MCP specification for HTTP/SSE transport and is fully compatible with MCP Inspector.