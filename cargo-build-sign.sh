#!/bin/bash
# Cargo build wrapper that automatically signs binaries on macOS
# Usage: ./cargo-build-sign.sh [cargo build arguments]

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Run the actual cargo build command
echo -e "${BLUE}🔨 Building with cargo...${NC}"
cargo build "$@"

# Only sign on macOS
if [[ "$OSTYPE" == "darwin"* ]]; then
    # Skip if SKIP_CODESIGN is set
    if [ -n "$SKIP_CODESIGN" ]; then
        echo -e "${YELLOW}⚠️  Skipping code signing (SKIP_CODESIGN set)${NC}"
        exit 0
    fi
    
    # Determine the profile (release or debug)
    PROFILE="debug"
    for arg in "$@"; do
        if [ "$arg" = "--release" ] || [ "$arg" = "-r" ]; then
            PROFILE="release"
            break
        fi
    done
    
    # Find the binary to sign
    BINARY_PATH="target/$PROFILE/loxone-mcp-server"
    
    if [ -f "$BINARY_PATH" ]; then
        echo -e "${BLUE}🔐 Code signing $BINARY_PATH...${NC}"
        
        # Always sign the binary to ensure it's properly signed after build
        # This handles cases where the binary exists but isn't signed
        
        # Sign the binary
        if [ -n "$CODESIGN_IDENTITY" ]; then
            echo "Using identity: $CODESIGN_IDENTITY"
            codesign -s "$CODESIGN_IDENTITY" --force --deep --preserve-metadata=entitlements "$BINARY_PATH"
        else
            # Ad-hoc signing for development
            codesign -s - --force --deep "$BINARY_PATH"
        fi
        
        if [ $? -eq 0 ]; then
            echo -e "${GREEN}✅ Code signing successful${NC}"
            # Verify the signature
            codesign --verify --verbose=2 "$BINARY_PATH" 2>&1 | grep -E "(valid|satisfies)" || true
        else
            echo -e "${YELLOW}⚠️  Code signing failed (non-fatal)${NC}"
        fi
    else
        echo -e "${YELLOW}⚠️  Binary not found at $BINARY_PATH${NC}"
    fi
fi

echo -e "${GREEN}✅ Build complete${NC}"