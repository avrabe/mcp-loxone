# Credential Migration Guide

This guide helps you migrate from the legacy environment variable credential system to the new Credential ID system.

## 🚀 Quick Migration

If you just want to get started quickly with the new system:

```bash
# Generate credentials with ID (interactive)
cargo run --bin loxone-mcp-setup --generate-id

# Then use the generated ID with the server
cargo run --bin loxone-mcp-server stdio --credential-id <generated-id>
```

## 📋 What Changed?

### Before (Legacy)
```bash
# Required environment variables
export LOXONE_USER="admin"
export LOXONE_PASS="password123" 
export LOXONE_HOST="192.168.1.100"

# Run server
cargo run --bin loxone-mcp-server stdio
```

### After (Credential ID System)
```bash
# Store credentials once
cargo run --bin loxone-mcp-auth store \
  --name "Main House" \
  --host 192.168.1.100 \
  --username admin \
  --password password123

# Use with generated ID
cargo run --bin loxone-mcp-server stdio --credential-id abc123def-456-789
```

## 🔄 Migration Steps

### Step 1: Export Your Current Credentials

If you have existing environment variables, note them down:

```bash
echo "Current credentials:"
echo "Host: $LOXONE_HOST"
echo "User: $LOXONE_USER"
echo "Pass: [hidden]"
```

### Step 2: Store Credentials with ID

```bash
# Store your existing credentials
cargo run --bin loxone-mcp-auth store \
  --name "My Loxone Server" \
  --host "$LOXONE_HOST" \
  --username "$LOXONE_USER" \
  --password "$LOXONE_PASS"
```

This will output something like:
```
✅ Credentials stored successfully!
📋 Credential ID: abc123def-456-789-012
```

### Step 3: Test the New System

```bash
# Test the stored credentials
cargo run --bin loxone-mcp-auth test abc123def-456-789-012

# Run server with credential ID
cargo run --bin loxone-mcp-server stdio --credential-id abc123def-456-789-012
```

### Step 4: Update Your Scripts

Update any scripts or aliases you have:

**Before:**
```bash
#!/bin/bash
export LOXONE_USER="admin"
export LOXONE_PASS="password123"
export LOXONE_HOST="192.168.1.100"
cargo run --bin loxone-mcp-server stdio
```

**After:**
```bash
#!/bin/bash
cargo run --bin loxone-mcp-server stdio --credential-id abc123def-456-789-012
```

### Step 5: Clean Up (Optional)

Once you've verified the new system works, you can remove the old environment variables:

```bash
# Remove from your shell profile (.bashrc, .zshrc, etc.)
unset LOXONE_USER
unset LOXONE_PASS
unset LOXONE_HOST
```

## 🏢 Multiple Servers

The new system makes it easy to manage multiple Loxone servers:

```bash
# Store credentials for multiple locations
cargo run --bin loxone-mcp-auth store --name "Home" --host 192.168.1.100 --username admin --password home123
cargo run --bin loxone-mcp-auth store --name "Office" --host 192.168.2.100 --username admin --password office456  
cargo run --bin loxone-mcp-auth store --name "Vacation House" --host 10.0.1.100 --username admin --password vacation789

# List all stored credentials
cargo run --bin loxone-mcp-auth list

# Use different servers easily
cargo run --bin loxone-mcp-server stdio --credential-id home-id
cargo run --bin loxone-mcp-server stdio --credential-id office-id
cargo run --bin loxone-mcp-server stdio --credential-id vacation-id
```

## 🔧 Credential Management Commands

### Store Credentials
```bash
# Interactive mode
cargo run --bin loxone-mcp-auth store

# Non-interactive mode
cargo run --bin loxone-mcp-auth store \
  --name "Server Name" \
  --host 192.168.1.100 \
  --username admin \
  --password secure123
```

### List Credentials
```bash
# Simple list
cargo run --bin loxone-mcp-auth list

# Detailed view
cargo run --bin loxone-mcp-auth list --detailed
```

### View Credential Details
```bash
# Basic info
cargo run --bin loxone-mcp-auth show abc123def-456-789

# Include sensitive information
cargo run --bin loxone-mcp-auth show abc123def-456-789 --include-sensitive
```

### Update Credentials
```bash
# Update username
cargo run --bin loxone-mcp-auth update abc123def-456-789 --username newuser

# Update password
cargo run --bin loxone-mcp-auth update abc123def-456-789 --password newpass

# Update name
cargo run --bin loxone-mcp-auth update abc123def-456-789 --name "New Server Name"
```

### Test Connections
```bash
# Basic test
cargo run --bin loxone-mcp-auth test abc123def-456-789

# Verbose test with connection details
cargo run --bin loxone-mcp-auth test abc123def-456-789 --verbose
```

### Delete Credentials
```bash
# Safe delete (with confirmation)
cargo run --bin loxone-mcp-auth delete abc123def-456-789

# Force delete (no confirmation)
cargo run --bin loxone-mcp-auth delete abc123def-456-789 --force
```

## 🔐 Security Benefits

The new credential system provides several security advantages:

1. **Secure Storage**: Credentials are stored using the same secure backends (keychain, Infisical, etc.)
2. **No Environment Variables**: Sensitive data isn't exposed in shell history or process lists
3. **Multiple Servers**: Easy management without credential conflicts
4. **Audit Trail**: Track when credentials were created and last used
5. **Easy Rotation**: Update passwords without affecting multiple scripts

## 🤝 Backward Compatibility

The system maintains full backward compatibility:

- Environment variables still work exactly as before
- Old scripts and integrations continue to function
- You can migrate gradually without breaking existing setups
- Both systems can coexist during transition

## 🆘 Troubleshooting

### "Credential not found" errors
```bash
# List available credentials
cargo run --bin loxone-mcp-auth list

# Check the exact ID
cargo run --bin loxone-mcp-auth show <partial-id>
```

### "Failed to load credentials" errors
```bash
# Test the credential
cargo run --bin loxone-mcp-auth test <credential-id> --verbose

# Verify credential storage backend
cargo run --bin loxone-mcp-verify
```

### Converting from environment variables
```bash
# If you have existing env vars, create a credential from them
cargo run --bin loxone-mcp-auth store \
  --name "From Environment" \
  --host "$LOXONE_HOST" \
  --username "$LOXONE_USER" \
  --password "$LOXONE_PASS"
```

## 📚 Additional Resources

- **Setup Guide**: Run `cargo run --bin loxone-mcp-setup --help`
- **Auth Commands**: Run `cargo run --bin loxone-mcp-auth --help`
- **Server Options**: Run `cargo run --bin loxone-mcp-server --help`

### 🗑️ Removed Legacy Tools
The following utilities have been removed and replaced with `loxone-mcp-auth`:
- `loxone-mcp-verify` → Use `loxone-mcp-auth test <id>`
- `loxone-mcp-update-host` → Use `loxone-mcp-auth update <id>`
- `loxone-mcp-test-connection` → Use `loxone-mcp-auth test <id> --verbose`

## 💡 Tips

1. **Use Descriptive Names**: Instead of "server1", use "Home Automation" or "Office Building"
2. **Test First**: Always test credentials before storing them permanently
3. **Backup IDs**: Keep a record of your credential IDs in a secure location
4. **Regular Rotation**: Update passwords periodically using the update command
5. **Multiple Environments**: Use different credentials for development, staging, and production

---

Need help? Open an issue at the project repository or run any command with `--help` for detailed usage information.