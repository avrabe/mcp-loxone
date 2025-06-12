# Setup Guide: Loxone MCP with Infisical

This guide shows you how to set up the Loxone MCP server with the new Infisical-first credential management.

## Quick Setup Options

### 🔐 Option 1: Infisical (Recommended for Teams)

1. **Sign up for Infisical**
   - Visit [https://app.infisical.com](https://app.infisical.com)
   - Create an account and new project

2. **Set up Universal Auth**
   - Go to Project Settings → Access Control → Machine Identities
   - Create new identity with Universal Auth
   - Copy the Client ID and Client Secret

3. **Set environment variables**
   ```bash
   export INFISICAL_PROJECT_ID="your-project-id-here"
   export INFISICAL_CLIENT_ID="your-client-id-here" 
   export INFISICAL_CLIENT_SECRET="your-client-secret-here"
   export INFISICAL_ENVIRONMENT="dev"  # or prod, staging, etc.
   ```

4. **Run setup**
   ```bash
   # Python version
   uvx --from . loxone-mcp setup
   
   # Or Rust version
   cd loxone-mcp-rust && cargo run --bin loxone-mcp-setup
   ```

### 🌍 Option 2: Environment Variables (CI/CD Friendly)

Simply set these environment variables:
```bash
export LOXONE_HOST="http://192.168.1.100"     # Your Miniserver IP
export LOXONE_USERNAME="admin"                # Your username  
export LOXONE_PASSWORD="your-password"        # Your password
export LOXONE_SSE_API_KEY="your-api-key"      # Optional for web integrations
```

### 🔑 Option 3: System Keychain (Individual Use)

Just run the setup wizard without Infisical environment variables:
```bash
# Python version
uvx --from . loxone-mcp setup

# Or Rust version  
cd loxone-mcp-rust && cargo run --bin loxone-mcp-setup
```

## Verification

Test your setup:
```bash
# Python
uvx --from . loxone-mcp verify

# Rust
cd loxone-mcp-rust && cargo run --bin loxone-mcp-verify
```

## Migration

If you have existing keychain credentials and want to migrate to Infisical:

1. Set up Infisical environment variables (see Option 1)
2. Run the migration:
   ```bash
   uvx --from . loxone-mcp migrate
   ```

## Usage

Once setup is complete:

```bash
# Run MCP server (for Claude Desktop)
uvx --from . loxone-mcp server

# Run SSE server (for web integrations)
uvx --from . loxone-mcp sse

# Test with MCP Inspector
uv run mcp dev src/loxone_mcp/server.py
```

## Team Sharing

With Infisical, sharing credentials with your team is easy:

1. **Share these environment variables** with team members:
   - `INFISICAL_PROJECT_ID` (same for everyone)
   - `INFISICAL_ENVIRONMENT` (same for everyone)
   
2. **Each team member gets their own**:
   - `INFISICAL_CLIENT_ID` (unique per person)
   - `INFISICAL_CLIENT_SECRET` (unique per person)

3. **Everyone can access the same Loxone credentials** stored in the shared project

## Troubleshooting

### Python: ImportError for infisicalsdk
```bash
# Install Infisical SDK
uv add infisicalsdk
```

### Rust: Missing features
```bash
# Build with Infisical support
cargo build --features infisical
```

### Connection issues
```bash
# Test connection directly
cargo run --bin loxone-mcp-test-connection
```

## Priority Order

The system tries credential backends in this order:
1. **Infisical** (if configured with env vars)
2. **Environment variables** (LOXONE_* vars)
3. **System keychain** (macOS Keychain, Windows Credential Manager, etc.)
4. **Local storage** (WASM environments only)

This ensures teams get Infisical while individuals can still use keychain storage.