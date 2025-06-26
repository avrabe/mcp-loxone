# MCP Inspector Setup Instructions

## 1. Start your Loxone MCP Server
```bash
export LOXONE_USERNAME="admin"
export LOXONE_PASSWORD="password" 
export LOXONE_HOST="192.168.178.10"
cargo run --bin loxone-mcp-server -- http --port 3001
```

## 2. Start MCP Inspector
```bash
npx @modelcontextprotocol/inspector
```

## 3. Configure MCP Inspector
1. Open browser to: `http://127.0.0.1:6274`
2. In the MCP Inspector UI, configure:
   - **Transport**: `sse`
   - **URL**: `http://localhost:3001/sse`
   - **Headers**: Click "Add Header"
     - Name: `X-API-Key`
     - Value: `1234`

## 4. Connect
Click "Connect" and you should see:
- ✅ Connection successful
- ✅ Tools list showing 34+ Loxone tools
- ✅ Ability to call tools and see responses

## Server Capabilities
Your Loxone MCP server provides:
- 🏠 Room management and device discovery
- 💡 Lighting control (14 lights)
- 🪟 Blinds/rolladen control (23 devices)  
- 🌡️ Temperature and sensor monitoring (37 sensors)
- 🎵 Audio zone control
- 🔒 Security and alarm system
- ⚡ Workflow automation

## Troubleshooting
- **Connection Error**: Verify server is running on port 3001
- **Health Check**: `curl http://localhost:3001/health` should return "OK"
- **SSE Test**: `curl -N -H "Accept: text/event-stream" -H "X-API-Key: 1234" http://localhost:3001/sse`