#!/bin/bash

# Test enhanced tools with the updated HTTP MCP server

echo "=== Testing Enhanced Tools ==="

echo "1. Testing improved list_rooms..."
curl -s -X POST http://localhost:3001/messages \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer default-api-key" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
      "name": "list_rooms",
      "arguments": {}
    }
  }' | jq '.result.content[0].text' -r | head -20

echo -e "\n2. Testing tools/list to see all available tools..."
curl -s -X POST http://localhost:3001/messages \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer default-api-key" \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/list",
    "params": {}
  }' | jq '.result.tools[].name' -r

echo -e "\n3. Testing get_devices_by_type (show available types)..."
curl -s -X POST http://localhost:3001/messages \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer default-api-key" \
  -d '{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "tools/call",
    "params": {
      "name": "get_devices_by_type",
      "arguments": {}
    }
  }' | jq '.result.content[0].text' -r

echo -e "\n4. Testing enhanced get_system_status..."
curl -s -X POST http://localhost:3001/messages \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer default-api-key" \
  -d '{
    "jsonrpc": "2.0",
    "id": 4,
    "method": "tools/call",
    "params": {
      "name": "get_system_status",
      "arguments": {}
    }
  }' | jq '.result.content[0].text' -r

echo -e "\n5. Testing discover_all_devices (first 10 devices)..."
curl -s -X POST http://localhost:3001/messages \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer default-api-key" \
  -d '{
    "jsonrpc": "2.0",
    "id": 5,
    "method": "tools/call",
    "params": {
      "name": "discover_all_devices",
      "arguments": {}
    }
  }' | jq '.result.content[0].text' -r | head -30