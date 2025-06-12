#!/bin/bash
# Create new keychain entries for Loxone MCP Rust Server

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🔐 Creating New Keychain Entries for Loxone MCP${NC}"
echo "==============================================="
echo ""

# Get current binary path for permissions
BINARY_PATH="$(pwd)/target/release/loxone-mcp-server"

# Function to create keychain entry
create_keychain_entry() {
    local service="$1"
    local account="$2"
    local prompt="$3"
    local is_password="$4"
    
    echo -n "$prompt: "
    if [ "$is_password" = "true" ]; then
        read -s value
        echo ""
    else
        read value
    fi
    
    if [ -n "$value" ]; then
        # Delete existing entry first
        security delete-generic-password -s "$service" -a "$account" 2>/dev/null || true
        
        # Create new entry
        security add-generic-password -s "$service" -a "$account" -w "$value" -T "$BINARY_PATH" -U
        
        if [ $? -eq 0 ]; then
            echo -e "${GREEN}✅ Stored $account${NC}"
        else
            echo -e "${YELLOW}⚠️  Failed to store $account${NC}"
        fi
    else
        echo -e "${YELLOW}⚠️  Skipped $account (empty value)${NC}"
    fi
}

# Clear existing entries first
echo -e "${YELLOW}Clearing existing entries...${NC}"
security delete-generic-password -s "LoxoneMCP" -a "LOXONE_USER" 2>/dev/null && echo "   Cleared LOXONE_USER" || true
security delete-generic-password -s "LoxoneMCP" -a "LOXONE_PASS" 2>/dev/null && echo "   Cleared LOXONE_PASS" || true
security delete-generic-password -s "LoxoneMCP" -a "LOXONE_HOST" 2>/dev/null && echo "   Cleared LOXONE_HOST" || true
security delete-generic-password -s "LoxoneMCP" -a "LOXONE_API_KEY" 2>/dev/null && echo "   Cleared LOXONE_API_KEY" || true
security delete-generic-password -s "LoxoneMCP" -a "LOXONE_SSE_API_KEY" 2>/dev/null && echo "   Cleared LOXONE_SSE_API_KEY (legacy)" || true

echo ""
echo -e "${YELLOW}Creating new entries...${NC}"

# Create entries
create_keychain_entry "LoxoneMCP" "LOXONE_HOST" "Enter Miniserver URL (e.g., http://192.168.178.10)" "false"
create_keychain_entry "LoxoneMCP" "LOXONE_USER" "Enter username" "false"
create_keychain_entry "LoxoneMCP" "LOXONE_PASS" "Enter password" "true"

echo ""
echo -n "Do you want to set an API key? (y/N): "
read -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    create_keychain_entry "LoxoneMCP" "LOXONE_API_KEY" "Enter API key" "false"
fi

echo ""
echo -e "${YELLOW}Testing new credentials...${NC}"

# Test with verification tool
if ./target/release/loxone-mcp-verify; then
    echo -e "${GREEN}✅ Keychain entries created successfully!${NC}"
    echo ""
    echo -e "${YELLOW}You can now use:${NC}"
    echo "   ./target/release/loxone-mcp-server stdio    # For Claude Desktop"
    echo "   ./target/release/loxone-mcp-server http     # For n8n/web"
    echo "   make run-stdio                              # Production stdio"
    echo "   make run-http                               # Production HTTP"
else
    echo -e "${YELLOW}⚠️  Verification had issues, but entries may still work${NC}"
fi