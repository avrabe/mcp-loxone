#!/bin/bash
# Test the server with environment variables

echo "🧪 Testing Loxone MCP Rust server with environment variables"
echo "This bypasses keychain access to avoid password prompts"
echo ""

# Set credentials (you'll need to set the password)
export LOXONE_USER="Ralf"
export LOXONE_HOST="http://192.168.178.10"

# Check if password is set
if [ -z "$LOXONE_PASS" ]; then
    echo "⚠️  Please set your Loxone password:"
    echo "  export LOXONE_PASS='your_password'"
    echo "  ./test-server-env.sh"
    exit 1
fi

echo "📋 Using credentials:"
echo "   Host: $LOXONE_HOST"
echo "   User: $LOXONE_USER"
echo "   Pass: ***"
echo ""

# Enable logging
export RUST_LOG=info

# Run with timeout to see initial connection
echo "🚀 Starting server..."
timeout 15s cargo run --bin loxone-mcp-server -- stdio