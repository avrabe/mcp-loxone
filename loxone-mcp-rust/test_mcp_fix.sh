#!/bin/bash

# Test script to verify MCP Inspector backwards compatibility fix

echo "Testing MCP Inspector backwards compatibility fix..."
echo "=================================================="

# Test 1: Check if server returns JSON directly for application/json requests
echo ""
echo "Test 1: Streamable HTTP transport (should return JSON directly)"
echo "--------------------------------------------------------------"

# Send a request with Accept: application/json (no text/event-stream)
response=$(curl -s -X POST \
  "http://localhost:3001/messages" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -H "X-API-Key: 1234" \
  -w "HTTP_CODE:%{http_code}" \
  -d '{
    "jsonrpc": "2.0",
    "id": 0,
    "method": "initialize",
    "params": {
      "protocolVersion": "2025-03-26",
      "capabilities": {
        "roots": {"listChanged": true},
        "sampling": {}
      },
      "clientInfo": {
        "name": "test-client",
        "version": "1.0.0"
      }
    }
  }')

# Extract HTTP code and body
http_code=$(echo "$response" | grep -o "HTTP_CODE:[0-9]*" | cut -d: -f2)
body=$(echo "$response" | sed 's/HTTP_CODE:[0-9]*$//')

echo "HTTP Status: $http_code"
echo "Response Body: $body"

if [ "$http_code" = "200" ] && [[ "$body" == *"protocolVersion"* ]]; then
    echo "✅ Test 1 PASSED: Streamable HTTP transport working"
else
    echo "❌ Test 1 FAILED: Expected 200 with JSON response"
fi

echo ""
echo "Test 2: Legacy SSE transport (should return 204 No Content)"
echo "----------------------------------------------------------"

# Send a request with Accept: text/event-stream (legacy transport)
response2=$(curl -s -X POST \
  "http://localhost:3001/messages" \
  -H "Content-Type: application/json" \
  -H "Accept: text/event-stream" \
  -H "X-API-Key: 1234" \
  -w "HTTP_CODE:%{http_code}" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2025-03-26",
      "capabilities": {
        "roots": {"listChanged": true},
        "sampling": {}
      },
      "clientInfo": {
        "name": "test-client-sse",
        "version": "1.0.0"
      }
    }
  }')

# Extract HTTP code and body
http_code2=$(echo "$response2" | grep -o "HTTP_CODE:[0-9]*" | cut -d: -f2)
body2=$(echo "$response2" | sed 's/HTTP_CODE:[0-9]*$//')

echo "HTTP Status: $http_code2"
echo "Response Body: '$body2'"

if [ "$http_code2" = "204" ]; then
    echo "✅ Test 2 PASSED: Legacy SSE transport working (response sent via SSE)"
else
    echo "❌ Test 2 FAILED: Expected 204 No Content for SSE transport"
fi

echo ""
echo "Summary:"
echo "========"
if [ "$http_code" = "200" ] && [ "$http_code2" = "204" ]; then
    echo "🎉 ALL TESTS PASSED! MCP Inspector should now connect properly."
    echo ""
    echo "The server correctly:"
    echo "- Returns JSON directly for Accept: application/json (Streamable HTTP)"
    echo "- Returns 204 No Content for Accept: text/event-stream (Legacy SSE)"
else
    echo "❌ Some tests failed. Check the server implementation."
fi