#!/bin/bash
# Build script for WASM-WASIP2 component compilation
# This script builds the Loxone MCP server as a WASM component

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
WASM_TARGET="wasm32-wasip2"
COMPONENT_NAME="loxone-mcp-component"
OUTPUT_DIR="target/wasm32-wasip2/release"
DIST_DIR="dist"

echo -e "${BLUE}🔧 Building Loxone MCP WASM Component${NC}"
echo "======================================"

# Check if required tools are installed
check_tool() {
    if ! command -v "$1" &> /dev/null; then
        echo -e "${RED}❌ Error: $1 is not installed${NC}"
        echo "Please install $1 and try again"
        exit 1
    fi
}

echo -e "${YELLOW}🔍 Checking prerequisites...${NC}"
check_tool "cargo"
check_tool "wasm-tools"

# Check for cargo-component
if ! cargo component --version &> /dev/null; then
    echo -e "${YELLOW}📦 Installing cargo-component...${NC}"
    cargo install cargo-component
fi

# Add WASM target if not already added
echo -e "${YELLOW}🎯 Ensuring WASM target is available...${NC}"
rustup target add $WASM_TARGET

# Clean previous builds
echo -e "${YELLOW}🧹 Cleaning previous builds...${NC}"
cargo clean --target $WASM_TARGET
rm -rf $DIST_DIR
mkdir -p $DIST_DIR

# Build the WASM component
echo -e "${YELLOW}🔨 Building WASM component...${NC}"

# Set environment variables for WASM build
export CARGO_TARGET_WASM32_WASIP2_RUNNER="wasm-tools run"

# Build with component features
cargo component build \
    --target $WASM_TARGET \
    --release \
    --features "infisical,wasi-keyvalue" \
    --config $WASM_TARGET

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ WASM component built successfully${NC}"
else
    echo -e "${RED}❌ WASM component build failed${NC}"
    exit 1
fi

# Find the generated component file
COMPONENT_FILE=$(find $OUTPUT_DIR -name "*.wasm" | head -1)

if [ -z "$COMPONENT_FILE" ]; then
    echo -e "${RED}❌ No WASM component file found${NC}"
    exit 1
fi

echo -e "${YELLOW}📦 Processing component...${NC}"

# Copy component to dist directory
cp "$COMPONENT_FILE" "$DIST_DIR/$COMPONENT_NAME.wasm"

# Validate the component
echo -e "${YELLOW}🔍 Validating component...${NC}"
wasm-tools validate "$DIST_DIR/$COMPONENT_NAME.wasm"

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Component validation passed${NC}"
else
    echo -e "${RED}❌ Component validation failed${NC}"
    exit 1
fi

# Get component info
echo -e "${YELLOW}📊 Component information:${NC}"
wasm-tools component wit "$DIST_DIR/$COMPONENT_NAME.wasm" 2>/dev/null || echo "WIT information not available"

# Get file size
COMPONENT_SIZE=$(wc -c < "$DIST_DIR/$COMPONENT_NAME.wasm")
COMPONENT_SIZE_KB=$((COMPONENT_SIZE / 1024))

echo -e "${GREEN}🎉 Build completed successfully!${NC}"
echo "=================================="
echo "Component: $DIST_DIR/$COMPONENT_NAME.wasm"
echo "Size: ${COMPONENT_SIZE_KB}KB"
echo "Target: $WASM_TARGET"
echo "Features: infisical, wasi-keyvalue"

# Generate usage instructions
cat > "$DIST_DIR/README.md" << EOF
# Loxone MCP WASM Component

This directory contains the WASM component build of the Loxone MCP server.

## Files

- \`$COMPONENT_NAME.wasm\` - The main WASM component (${COMPONENT_SIZE_KB}KB)

## Usage

### With Wasmtime

\`\`\`bash
# Install wasmtime if not already installed
curl https://wasmtime.dev/install.sh -sSf | bash

# Run the component
wasmtime serve \\
  --wasi-modules=common \\
  --allow-http \\
  --env LOXONE_USERNAME=your-username \\
  --env LOXONE_PASSWORD=your-password \\
  --env LOXONE_HOST=http://your-miniserver \\
  $COMPONENT_NAME.wasm
\`\`\`

### With Infisical Configuration

\`\`\`bash
wasmtime serve \\
  --wasi-modules=common \\
  --allow-http \\
  --env INFISICAL_PROJECT_ID=your-project-id \\
  --env INFISICAL_CLIENT_ID=your-client-id \\
  --env INFISICAL_CLIENT_SECRET=your-client-secret \\
  --env INFISICAL_ENVIRONMENT=dev \\
  $COMPONENT_NAME.wasm
\`\`\`

### With WASI Keyvalue Store

\`\`\`bash
wasmtime serve \\
  --wasi-modules=common,keyvalue \\
  --allow-http \\
  --keyvalue-store default \\
  $COMPONENT_NAME.wasm
\`\`\`

## Features

- **Infisical Integration**: Centralized secret management
- **WASI Keyvalue**: Component-native credential storage
- **HTTP Client**: WASI HTTP interface support
- **Environment Variables**: Traditional env var support
- **Multi-backend Fallback**: Automatic backend selection

## Environment Variables

### Loxone Configuration
- \`LOXONE_HOST\` - Miniserver URL (required)
- \`LOXONE_USERNAME\` - Username (required)
- \`LOXONE_PASSWORD\` - Password (required)

### Infisical Configuration (Optional)
- \`INFISICAL_PROJECT_ID\` - Project ID
- \`INFISICAL_CLIENT_ID\` - Client ID for universal auth
- \`INFISICAL_CLIENT_SECRET\` - Client secret
- \`INFISICAL_ENVIRONMENT\` - Environment (default: dev)
- \`INFISICAL_HOST\` - Custom host for self-hosted instances

### Runtime Configuration
- \`RUST_LOG\` - Log level (debug, info, warn, error)
- \`MCP_TRANSPORT\` - Transport type (default: wasm)

Built on $(date) with Rust $(rustc --version)
EOF

echo -e "${BLUE}📝 Usage instructions written to $DIST_DIR/README.md${NC}"
echo -e "${GREEN}🚀 Ready for deployment!${NC}"