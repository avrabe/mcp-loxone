# 🚀 Quick Start Guide

Get your Loxone MCP server running in 5 minutes!

## Prerequisites
- macOS, Linux, or Windows with WSL
- Python 3.10 or higher
- Loxone Miniserver on your network

## Step 1: Install uv (if not already installed)

**macOS (recommended):**
```bash
brew install uv
```

**Other systems:**
```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

## Step 2: Set up credentials

```bash
cd /path/to/mcp-loxone
chmod +x setup.sh
./setup.sh
```

Enter your Loxone details when prompted:
- Miniserver IP (e.g., 192.168.1.100)
- Username
- Password

## Step 3: Test the server

```bash
uv run mcp dev src/loxone_mcp/server.py
```

This opens the MCP Inspector at http://localhost:6274

## Step 4: Configure Claude Desktop

Copy the example configuration:
```bash
cat claude_desktop_config.json.example
```

Add it to your Claude Desktop config:
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

## Step 5: Restart Claude Desktop

After restarting, you can use commands like:
- "List all my rooms"
- "Turn on the lights in the living room"
- "Close all rolladen"
- "What's the status of my devices?"

## Troubleshooting

**Can't connect to Loxone?**
```bash
# Test your credentials
uvx --from . loxone-mcp validate

# Re-run setup if needed
uvx --from . loxone-mcp setup
```

**View server logs:**
```bash
export LOXONE_LOG_LEVEL=DEBUG
uvx --from . loxone-mcp-server
```

## Next Steps

- Read the [README](README.md) for detailed documentation
- Check [DEVELOPMENT.md](DEVELOPMENT.md) to extend functionality
- Report issues on GitHub

---

**Need help?** Create an issue on GitHub with:
1. Your error message
2. Output of `uv run python test_server.py`
3. Loxone Miniserver version
