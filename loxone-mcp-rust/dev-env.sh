#!/bin/bash
# Development environment setup for Loxone MCP Rust server
# This script sets up environment variables to avoid keychain password prompts

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🔧 Setting up Loxone MCP Rust development environment...${NC}"

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || ! grep -q "loxone-mcp-rust" Cargo.toml; then
    echo "❌ Please run this script from the loxone-mcp-rust directory"
    exit 1
fi

# Export development environment variables
export LOXONE_API_KEY="dev-api-key-$(date +%s)"
export RUST_LOG="debug,loxone_mcp_rust=trace"
export LOXONE_LOG_LEVEL="DEBUG"

# Try to get real credentials from environment or prompt
if [ -z "$LOXONE_USER" ]; then
    echo -e "${YELLOW}💡 LOXONE_USER not set. You can set it to avoid setup prompts.${NC}"
fi

if [ -z "$LOXONE_PASS" ]; then
    echo -e "${YELLOW}💡 LOXONE_PASS not set. You can set it to avoid setup prompts.${NC}"
fi

if [ -z "$LOXONE_HOST" ]; then
    echo -e "${YELLOW}💡 LOXONE_HOST not set. You can set it to avoid setup prompts.${NC}"
fi

echo -e "${GREEN}✅ Environment configured for development${NC}"
echo -e "${YELLOW}📋 Environment variables set:${NC}"
echo "   LOXONE_API_KEY=$LOXONE_API_KEY"
echo "   RUST_LOG=$RUST_LOG"
echo "   LOXONE_LOG_LEVEL=$LOXONE_LOG_LEVEL"

echo -e "${YELLOW}🚀 Now you can run:${NC}"
echo "   cargo run --bin loxone-mcp-server -- stdio    # For Claude Desktop"
echo "   cargo run --bin loxone-mcp-server -- http     # For n8n/web clients"
echo "   cargo run --bin loxone-mcp-server -- http --api-key YOUR_KEY  # With custom API key"
echo "   make dev-run                                  # Development HTTP server"
echo "   make dev                                      # Auto-reload development"

# Create a .env file for convenience
cat > .env.development << EOF
# Loxone MCP Rust Development Environment
# This file avoids keychain password prompts during development

# Authentication (development token)
LOXONE_API_KEY=$LOXONE_API_KEY
API_KEY=$LOXONE_API_KEY

# Logging
RUST_LOG=$RUST_LOG
LOXONE_LOG_LEVEL=$LOXONE_LOG_LEVEL

# Uncomment and set these to avoid credential setup prompts:
# LOXONE_USER=your_username
# LOXONE_PASS=your_password
# LOXONE_HOST=http://your-miniserver-ip
EOF

echo -e "${GREEN}📄 Created .env.development file for convenience${NC}"
echo -e "${YELLOW}💡 You can source it with: source .env.development${NC}"