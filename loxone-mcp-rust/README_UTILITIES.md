# Loxone MCP Rust Server Utilities

This directory contains several utility programs to help manage and test the Loxone MCP Rust server.

## Setup and Configuration

### 1. Setup Utility
```bash
cargo run --bin loxone-mcp-setup
```
Interactive setup wizard that stores credentials in the system keychain (compatible with Python version).

### 2. Verify Credentials
```bash
cargo run --bin loxone-mcp-verify
```
Checks if credentials are properly stored and accessible from the keychain.

### 3. Update Host URL
```bash
cargo run --bin loxone-mcp-update-host
```
Updates the host URL in the keychain to ensure it has the proper `http://` prefix.

## Running the Server

### Option 1: Using Environment Variables (Recommended)
To avoid macOS keychain password prompts, use the provided script:
```bash
./run-server.sh         # Runs with stdio transport (Claude Desktop)
./run-server.sh http    # Runs with HTTP/SSE transport (n8n)
```

You can also set credentials manually:
```bash
export LOXONE_USER="your_username"
export LOXONE_PASS="your_password"
export LOXONE_HOST="http://192.168.178.10"
cargo run --bin loxone-mcp-server -- stdio
```

### Option 2: Direct Execution (Requires Keychain Access)
```bash
cargo run --bin loxone-mcp-server -- stdio
```
Note: This will prompt for your macOS password to access the keychain.

## Troubleshooting

### Keychain Access Issues
If you see "Unable to obtain authorization for this operation":
1. The server is trying to access the keychain and needs your password
2. Use environment variables instead (see above)
3. Or grant permanent access in Keychain Access app

### Connection Issues
Test the connection directly:
```bash
cargo run --bin test_connection
```

### HTTP/SSE Mode
For n8n integration, run with HTTP transport:
```bash
LOXONE_API_KEY="your-api-key" ./run-server.sh http
```

## Integration with Claude Desktop

Add to your Claude Desktop configuration:
```json
{
  "mcpServers": {
    "loxone-rust": {
      "command": "/path/to/loxone-mcp-rust/run-server.sh",
      "args": ["stdio"],
      "env": {
        "LOXONE_USER": "your_username",
        "LOXONE_PASS": "your_password",
        "LOXONE_HOST": "http://192.168.178.10"
      }
    }
  }
}
```