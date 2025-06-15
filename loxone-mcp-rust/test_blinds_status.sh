#!/bin/bash

# Test script to verify the new get_all_blinds_status tool
echo "Testing get_all_blinds_status tool..."

# Set environment variables
export LOXONE_USERNAME="admin"
export LOXONE_PASSWORD="HomerSimpson"
export LOXONE_HOST="192.168.1.10"

# Create MCP test request for the new tool
cat << 'EOF' | timeout 30s ./target/release/loxone-mcp-server stdio
{"jsonrpc": "2.0", "method": "tools/call", "params": {"name": "get_all_blinds_status", "arguments": {}}, "id": 1}
EOF

echo "Test completed."