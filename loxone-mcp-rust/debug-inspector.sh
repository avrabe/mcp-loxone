#!/bin/bash

echo "Starting debug session..."

# Kill any existing processes
pkill -f "@modelcontextprotocol/inspector" || true
pkill -f "loxone-mcp-server" || true

# Start the inspector with environment variables to avoid keychain
echo "Starting MCP Inspector..."
LOXONE_USER=admin LOXONE_PASS=admin LOXONE_HOST=http://192.168.1.100 \
npx @modelcontextprotocol/inspector ./target/release/loxone-mcp-server stdio &

INSPECTOR_PID=$!
echo "Inspector PID: $INSPECTOR_PID"

# Wait for inspector to start
sleep 3

# Check if server process started
echo -e "\nChecking for server process..."
ps aux | grep loxone-mcp-server | grep -v grep

# Try to connect to the inspector
echo -e "\nTrying to connect to inspector..."
curl -s http://127.0.0.1:6274/ | head -5

# Check what's listening
echo -e "\nPorts in use:"
lsof -i :6274 -i :6277 | grep LISTEN

# Wait for user
echo -e "\nInspector should be running at http://127.0.0.1:6274"
echo "Press Enter to stop..."
read

# Cleanup
kill $INSPECTOR_PID 2>/dev/null
pkill -f "@modelcontextprotocol/inspector" || true
pkill -f "loxone-mcp-server" || true