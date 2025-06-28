#!/bin/bash

# Script to publish remaining MCP framework crates
# Run this when the crates.io rate limit clears

set -e  # Exit on any error

echo "🚀 Publishing remaining PulseEngine MCP Framework crates..."

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if we're in the right directory
if [[ ! -f "Cargo.toml" ]] || [[ ! -d "mcp-framework" ]]; then
    echo -e "${RED}❌ Error: Run this script from the project root directory${NC}"
    exit 1
fi

# Function to publish a crate with verification
publish_crate() {
    local crate_path=$1
    local crate_name=$2
    
    echo -e "\n${BLUE}📦 Publishing ${crate_name}...${NC}"
    
    cd "$crate_path"
    
    # Dry run first
    echo "   🧪 Testing with dry run..."
    if ! cargo publish --dry-run; then
        echo -e "${RED}❌ Dry run failed for ${crate_name}${NC}"
        exit 1
    fi
    
    # Ask for confirmation
    echo -e "${BLUE}❓ Publish ${crate_name} to crates.io? (y/N)${NC}"
    read -r response
    if [[ "$response" =~ ^[Yy]$ ]]; then
        # Actual publish
        echo "   📤 Publishing to crates.io..."
        if cargo publish; then
            echo -e "${GREEN}✅ Successfully published ${crate_name}${NC}"
        else
            echo -e "${RED}❌ Failed to publish ${crate_name}${NC}"
            exit 1
        fi
    else
        echo "   ⏭️  Skipped ${crate_name}"
    fi
    
    cd - > /dev/null
}

# Check current status
echo -e "${BLUE}📋 Checking current publication status...${NC}"

# Verify workspace compiles
echo "   🔍 Verifying workspace compiles..."
if ! cargo check --workspace --quiet; then
    echo -e "${RED}❌ Workspace compilation failed${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Workspace compiles successfully${NC}"

# Publication order (dependencies first)
echo -e "\n${BLUE}📋 Publication order:${NC}"
echo "   1. pulseengine-mcp-security"
echo "   2. pulseengine-mcp-monitoring" 
echo "   3. pulseengine-mcp-server"

# Publish in dependency order
publish_crate "mcp-framework/mcp-security" "pulseengine-mcp-security"
publish_crate "mcp-framework/mcp-monitoring" "pulseengine-mcp-monitoring"
publish_crate "mcp-framework/mcp-server" "pulseengine-mcp-server"

echo -e "\n${GREEN}🎉 All remaining crates published successfully!${NC}"
echo -e "${BLUE}📚 Framework is now complete on crates.io:${NC}"
echo "   • pulseengine-mcp-protocol"
echo "   • pulseengine-mcp-logging"
echo "   • pulseengine-mcp-transport"
echo "   • pulseengine-mcp-auth"
echo "   • pulseengine-mcp-security"
echo "   • pulseengine-mcp-monitoring"
echo "   • pulseengine-mcp-server"

echo -e "\n${BLUE}🔗 View published crates:${NC}"
echo "   https://crates.io/search?q=pulseengine-mcp"

echo -e "\n${BLUE}📖 Next steps:${NC}"
echo "   1. Update main Loxone implementation to use published framework"
echo "   2. Test end-to-end functionality"
echo "   3. Update documentation with published examples"