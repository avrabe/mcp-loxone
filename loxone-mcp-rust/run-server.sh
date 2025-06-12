#!/bin/bash
# Run Loxone MCP Rust server with credentials from environment
# This avoids the macOS keychain password prompt

# Try to get credentials from keychain without prompting
# If it fails (requires password), use the values we know
export LOXONE_USER="${LOXONE_USER:-Ralf}"
export LOXONE_PASS="${LOXONE_PASS:-$(security find-generic-password -a "LOXONE_PASS" -s "LoxoneMCP" -w 2>/dev/null || echo "")}"
export LOXONE_HOST="${LOXONE_HOST:-http://192.168.178.10}"

# Check if we have credentials
if [ -z "$LOXONE_PASS" ]; then
    echo "⚠️  Warning: Could not get password from keychain"
    echo "Please set LOXONE_PASS environment variable:"
    echo "  export LOXONE_PASS='your_password'"
    exit 1
fi

echo "🚀 Starting Loxone MCP Rust server with environment credentials"
echo "   Host: $LOXONE_HOST"
echo "   User: $LOXONE_USER"
echo "   Pass: ***"
echo ""

# Run the server with the specified transport (default: stdio)
TRANSPORT="${1:-stdio}"
cargo run --bin loxone-mcp-server -- "$TRANSPORT"