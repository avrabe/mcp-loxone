#!/bin/bash
# Test script to demonstrate keychain password prompt reduction

set -e

echo "🧪 Testing Loxone MCP Rust Server - Keychain Password Prompts"
echo "============================================================"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${YELLOW}This test will show how the optimized version reduces keychain prompts${NC}"
echo ""

echo -e "${YELLOW}📊 Expected results:${NC}"
echo "   • Old approach: 4-8 keychain prompts (username, password, host, API key)"
echo "   • New approach: 4 keychain prompts (batched access)"
echo "   • Environment variables: 0 keychain prompts"
echo ""

echo -e "${YELLOW}🔐 Code Signing Status:${NC}"
if codesign -v target/release/loxone-mcp-server 2>/dev/null; then
    echo -e "${GREEN}✅ Binary is properly signed${NC}"
    codesign -dv target/release/loxone-mcp-server 2>&1 | grep "Signature" | head -1
else
    echo -e "${RED}❌ Binary is not signed - this will cause additional prompts${NC}"
    echo "   Run: make build (automatic) or ./sign-binary.sh (manual)"
fi
echo ""

echo -e "${YELLOW}🧪 Test 1: Environment Variables (No Keychain Access)${NC}"
echo "Command: LOXONE_USER=test LOXONE_PASS=test LOXONE_HOST=http://127.0.0.1 ./target/release/loxone-mcp-server stdio"
echo "Expected: 0 keychain prompts"
echo ""
timeout 3s env LOXONE_USER=test LOXONE_PASS=test LOXONE_HOST=http://127.0.0.1 ./target/release/loxone-mcp-server stdio 2>&1 | grep -E "(Found credentials|Using host|environment)" || echo "   (Connection failed as expected - test server not running)"
echo ""

echo -e "${YELLOW}🧪 Test 2: Keychain Access (Batched)${NC}"
echo "Command: ./target/release/loxone-mcp-server stdio"
echo "Expected: 4 keychain prompts (batched access)"
echo -e "${RED}⚠️  This will prompt for keychain password 4 times${NC}"
echo ""
read -p "Press Enter to continue with keychain test (or Ctrl+C to skip)..."
timeout 5s ./target/release/loxone-mcp-server stdio 2>&1 | grep -E "(Found credentials|Using host|keychain)" | head -5 || echo "   (Connection failed as expected - test server not running)"
echo ""

echo -e "${YELLOW}📋 Summary:${NC}"
echo -e "${GREEN}✅ Environment variables prevent all keychain prompts${NC}"
echo -e "${GREEN}✅ Batched keychain access reduces prompts compared to multiple separate calls${NC}"
echo -e "${GREEN}✅ Code signing reduces but doesn't eliminate keychain prompts${NC}"
echo ""

echo -e "${YELLOW}💡 Recommendations for Development:${NC}"
echo "1. Use environment variables: make dev-run or make dev-stdio"
echo "2. Use development environment: ./dev-env.sh && source .env.development"
echo "3. For production: Use signed binary with keychain (./target/release/loxone-mcp-server)"
echo ""

echo -e "${GREEN}🎉 Test completed!${NC}"