#!/bin/bash
# Code signing script for Loxone MCP Rust server
# This prevents keychain password prompts during development

set -e

BINARY_NAME="loxone-mcp-server"
TARGET_DIR="target/release"
BINARY_PATH="$TARGET_DIR/$BINARY_NAME"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🔐 Code signing Loxone MCP Rust server...${NC}"

# Check if binary exists
if [ ! -f "$BINARY_PATH" ]; then
    echo -e "${RED}❌ Binary not found at $BINARY_PATH${NC}"
    echo -e "${YELLOW}💡 Run 'cargo build --release' first${NC}"
    exit 1
fi

# Check if binary is already signed
if codesign -v "$BINARY_PATH" 2>/dev/null; then
    echo -e "${GREEN}✅ Binary is already signed${NC}"
    codesign -dv "$BINARY_PATH" 2>&1 | grep "Authority"
    exit 0
fi

# Get available signing identities
echo -e "${YELLOW}🔍 Available signing identities:${NC}"
IDENTITIES=$(security find-identity -v -p codesigning | grep "Developer ID Application" | head -5)

if [ -z "$IDENTITIES" ]; then
    echo -e "${YELLOW}⚠️  No Developer ID found, using ad-hoc signing${NC}"
    # Ad-hoc signing (for local development)
    codesign -s - "$BINARY_PATH"
    echo -e "${GREEN}✅ Binary signed with ad-hoc signature${NC}"
else
    echo "$IDENTITIES"
    echo
    
    # Try to find the first valid Developer ID
    IDENTITY=$(echo "$IDENTITIES" | head -1 | sed 's/.*) \([^"]*\).*/\1/')
    
    if [ -n "$IDENTITY" ]; then
        echo -e "${YELLOW}🔑 Using identity: $IDENTITY${NC}"
        codesign -s "$IDENTITY" "$BINARY_PATH"
        echo -e "${GREEN}✅ Binary signed with Developer ID${NC}"
    else
        echo -e "${YELLOW}⚠️  Using ad-hoc signing as fallback${NC}"
        codesign -s - "$BINARY_PATH"
        echo -e "${GREEN}✅ Binary signed with ad-hoc signature${NC}"
    fi
fi

# Verify signing
echo -e "${YELLOW}🔍 Verifying signature...${NC}"
if codesign -v "$BINARY_PATH"; then
    echo -e "${GREEN}✅ Signature verification successful${NC}"
    
    # Show signature details
    echo -e "${YELLOW}📋 Signature details:${NC}"
    codesign -dv "$BINARY_PATH" 2>&1 || true
else
    echo -e "${RED}❌ Signature verification failed${NC}"
    exit 1
fi

echo -e "${GREEN}🎉 Code signing completed successfully!${NC}"
echo -e "${YELLOW}💡 This should prevent keychain password prompts during development${NC}"