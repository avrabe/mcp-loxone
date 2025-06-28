#!/bin/bash

echo "🚀 Starting MCP Server in offline mode for testing..."

# Clear any Loxone environment variables to force offline mode
unset LOXONE_USER
unset LOXONE_PASS  
unset LOXONE_HOST

# Set empty values to ensure offline mode
export LOXONE_USER=""
export LOXONE_PASS=""
export LOXONE_HOST=""

# Start the server
echo "Starting server on port 3001..."
cargo run --release --bin loxone-mcp-server -- http --port 3001