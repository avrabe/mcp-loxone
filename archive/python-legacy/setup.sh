#!/bin/bash
# Make executable: chmod +x setup.sh

# Quick setup script for Loxone MCP Server

echo "🏠 Loxone MCP Server Setup"
echo "=========================="
echo ""

# Check if uv is installed
if ! command -v uv &> /dev/null; then
    echo "❌ uv is not installed."
    echo ""
    echo "Please install uv first:"
    echo "  macOS: brew install uv"
    echo "  Other: curl -LsSf https://astral.sh/uv/install.sh | sh"
    echo ""
    exit 1
fi

# Check if we're in the right directory
if [ ! -f "pyproject.toml" ]; then
    echo "❌ Please run this script from the mcp-loxone-gen1 directory"
    exit 1
fi

echo "✅ Found uv installation"
echo ""

# Run credential setup
echo "📝 Setting up Loxone credentials..."
echo ""
uvx --from . loxone-mcp setup

echo ""
echo "🎉 Setup complete!"
echo ""
echo "Next steps:"
echo "1. Test the server:"
echo "   uv run mcp dev src/loxone_mcp/server.py"
echo ""
echo "2. Configure in Claude Desktop:"
echo "   See claude_desktop_config.json.example"
echo ""
