#!/bin/bash

# Test MCP SSE implementation

echo "Testing MCP HTTP/SSE Transport"
echo "=============================="

# Start test in background
echo "1. Testing SSE connection..."
curl -N -H "Accept: text/event-stream" http://localhost:3001/sse &
SSE_PID=$!

sleep 2

echo -e "\n2. Sending test message via POST..."
curl -X POST http://localhost:3001/messages \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "test.echo",
    "params": {"message": "Hello MCP"},
    "id": 1
  }'

echo -e "\n\n3. Waiting for SSE events..."
sleep 5

# Clean up
kill $SSE_PID 2>/dev/null

echo -e "\nTest complete."