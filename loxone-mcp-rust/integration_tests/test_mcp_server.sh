#!/bin/bash

# MCP Server Integration Test Script

set -e

SERVER_URL="http://localhost:3003"
API_KEY="1234"

echo "=== MCP Server Integration Tests ==="

# Test 1: Health check
echo "1. Testing health endpoint..."
curl -s "$SERVER_URL/health" || (echo "Health check failed" && exit 1)
echo " ✓ Health check passed"

# Test 2: SSE connection
echo "2. Testing SSE connection..."
# Use gtimeout on macOS, timeout on Linux, or fallback without timeout
if command -v timeout >/dev/null 2>&1; then
    timeout 5 curl -s -N -H "Accept: text/event-stream" -H "X-API-Key: $API_KEY" "$SERVER_URL/sse" | head -5 | grep -q "endpoint" || (echo "SSE test failed" && exit 1)
elif command -v gtimeout >/dev/null 2>&1; then
    gtimeout 5 curl -s -N -H "Accept: text/event-stream" -H "X-API-Key: $API_KEY" "$SERVER_URL/sse" | head -5 | grep -q "endpoint" || (echo "SSE test failed" && exit 1)
else
    echo "⚠️ No timeout command available, running without timeout..."
    curl -s -N -H "Accept: text/event-stream" -H "X-API-Key: $API_KEY" "$SERVER_URL/sse" | head -5 | grep -q "endpoint" || (echo "SSE test failed" && exit 1)
fi
echo " ✓ SSE connection established"

# Test 3: Streamable HTTP initialize
echo "3. Testing Streamable HTTP transport..."
INIT_RESPONSE=$(curl -s -X POST \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -H "X-API-Key: $API_KEY" \
  -d '{"jsonrpc":"2.0","id":"test","method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}' \
  "$SERVER_URL/messages")

echo "$INIT_RESPONSE" | jq -e '.result.server_info.name == "loxone-mcp-server"' > /dev/null || (echo "Initialize test failed" && exit 1)
echo " ✓ Initialize request successful"

# Test 4: Tools list
echo "4. Testing tools/list..."
TOOLS_RESPONSE=$(curl -s -X POST \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -H "X-API-Key: $API_KEY" \
  -d '{"jsonrpc":"2.0","id":"tools","method":"tools/list","params":{}}' \
  "$SERVER_URL/messages")

echo "$TOOLS_RESPONSE" | jq -e '.result.tools | length > 0' > /dev/null || (echo "Tools list test failed" && exit 1)
echo " ✓ Tools list retrieved"

echo ""
echo "🎉 All tests passed! MCP server is working correctly."