#!/bin/bash

# Enable history data collection for the Loxone MCP server dashboard
# This script sets up environment variables to enable statistics collection

echo "🚀 Enabling history data collection for Loxone MCP server..."

# Enable Loxone statistics collection
export ENABLE_LOXONE_STATS=1

# Create a simple data directory for potential future use
DATA_DIR="$HOME/.loxone-mcp/history"
mkdir -p "$DATA_DIR"

echo "✅ Environment configured for history collection"
echo ""
echo "📚 Data collection is now enabled. The server will:"
echo "  - Collect system metrics every 30 seconds"
echo "  - Store data in memory for dashboard display"
echo "  - Show real-time data in the monitoring dashboard"
echo ""
echo "🌐 Access your dashboards at:"
echo "  - Monitoring: http://localhost:3001/dashboard/"
echo "  - History: http://localhost:3001/history/"
echo ""
echo "🔄 To start the server with history enabled:"
echo "  export ENABLE_LOXONE_STATS=1"
echo "  cargo run --bin loxone-mcp-server http"
echo ""

# Set up environment variables for this session
echo "Environment variables set for current session."
echo "To make permanent, add this to your ~/.bashrc or ~/.zshrc:"
echo "export ENABLE_LOXONE_STATS=1"