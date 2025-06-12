#!/bin/bash
# Reset keychain entries and reinitialize with Rust tools
# This should resolve permission issues between Python and Rust keychain access

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔄 Resetting Loxone MCP Keychain Entries${NC}"
echo "=========================================="
echo ""

echo -e "${YELLOW}This will:${NC}"
echo "1. Clear existing keychain entries (created by Python version)"
echo "2. Reinitialize with Rust tools for proper permissions"
echo "3. Test keychain access to verify no prompts"
echo ""

read -p "Continue? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 0
fi

echo ""
echo -e "${YELLOW}📋 Step 1: Backing up current credentials${NC}"

# Try to get current credentials for backup
USERNAME=$(security find-generic-password -s "LoxoneMCP" -a "LOXONE_USER" -w 2>/dev/null || echo "")
HOST=$(security find-generic-password -s "LoxoneMCP" -a "LOXONE_HOST" -w 2>/dev/null || echo "")

if [ -n "$USERNAME" ]; then
    echo "   Found username: $USERNAME"
    echo "   Found host: $HOST"
    echo "   (Password and API key will be re-entered)"
else
    echo "   No existing credentials found"
fi

echo ""
echo -e "${YELLOW}📋 Step 2: Clearing existing keychain entries${NC}"

# Clear existing entries
security delete-generic-password -s "LoxoneMCP" -a "LOXONE_USER" 2>/dev/null && echo "   ✅ Cleared LOXONE_USER" || echo "   ⚪ LOXONE_USER not found"
security delete-generic-password -s "LoxoneMCP" -a "LOXONE_PASS" 2>/dev/null && echo "   ✅ Cleared LOXONE_PASS" || echo "   ⚪ LOXONE_PASS not found"
security delete-generic-password -s "LoxoneMCP" -a "LOXONE_HOST" 2>/dev/null && echo "   ✅ Cleared LOXONE_HOST" || echo "   ⚪ LOXONE_HOST not found"
security delete-generic-password -s "LoxoneMCP" -a "LOXONE_API_KEY" 2>/dev/null && echo "   ✅ Cleared LOXONE_API_KEY" || echo "   ⚪ LOXONE_API_KEY not found"
security delete-generic-password -s "LoxoneMCP" -a "LOXONE_SSE_API_KEY" 2>/dev/null && echo "   ✅ Cleared LOXONE_SSE_API_KEY (legacy)" || echo "   ⚪ LOXONE_SSE_API_KEY not found"

echo ""
echo -e "${YELLOW}📋 Step 3: Rebuilding Rust binary${NC}"

# Ensure we have a fresh signed binary
if ! make build; then
    echo -e "${RED}❌ Failed to build Rust binary${NC}"
    exit 1
fi

echo ""
echo -e "${YELLOW}📋 Step 4: Setting up credentials with Rust tools${NC}"

# Use the Rust setup tool
echo "Running Rust credential setup..."
if [ -n "$USERNAME" ] && [ -n "$HOST" ]; then
    echo "Pre-filling with backed up values..."
    echo "Username: $USERNAME"
    echo "Host: $HOST"
    echo ""
fi

./target/release/loxone-mcp-setup || {
    echo -e "${RED}❌ Setup failed${NC}"
    exit 1
}

echo ""
echo -e "${YELLOW}📋 Step 5: Testing keychain access${NC}"

echo "Testing credential verification (should not prompt)..."
if ./target/release/loxone-mcp-verify; then
    echo -e "${GREEN}✅ Keychain access working without prompts!${NC}"
else
    echo -e "${RED}❌ Keychain access still prompting${NC}"
    echo "You may need to:"
    echo "1. Run: security unlock-keychain ~/Library/Keychains/login.keychain-db"
    echo "2. Restart Terminal"
    echo "3. Try again"
    exit 1
fi

echo ""
echo -e "${YELLOW}📋 Step 6: Testing server startup${NC}"

echo "Testing server startup (should not prompt)..."
timeout 5s ./target/release/loxone-mcp-server stdio 2>&1 | head -10 | grep -E "(Found credentials|Using host|keychain)" || echo "   (Connection test completed)"

echo ""
echo -e "${GREEN}🎉 Keychain reset completed successfully!${NC}"
echo ""
echo -e "${YELLOW}📋 Summary:${NC}"
echo "✅ Old keychain entries cleared"
echo "✅ New entries created with Rust tools"
echo "✅ Proper permissions established"
echo "✅ Server should now start without prompts"
echo ""
echo -e "${YELLOW}💡 Next steps:${NC}"
echo "   Production: ./target/release/loxone-mcp-server stdio"
echo "   Development: make dev-stdio (uses environment variables)"
echo ""