# Loxone MCP Security Architecture

## Overview

The Loxone MCP server implements a comprehensive multi-user security system with role-based access control (RBAC), API key management, and flexible storage backends.

## Key Concepts

### Security Levels

```bash
# Set the overall security level
export SECURITY_LEVEL=production  # Options: development, staging, production
```

- **Development**: Relaxed security for local development
- **Staging**: Moderate security with API keys required
- **Production**: Full security with strict policies

### Multi-User API Keys

Each API key follows the format: `lmcp_{role}_{sequence}_{random}`

Example: `lmcp_admin_001_abc123def456`

## API Key Management

### Web UI Management (Recommended)

The easiest way to manage API keys is through the built-in web interface:

1. Start the server: `cargo run --bin loxone-mcp-server -- http`
2. Open your browser: `http://localhost:3001/admin/keys`
3. Use the web interface to:
   - Generate new keys with specific roles
   - View all keys with usage statistics
   - Edit key properties (name, expiration, status)
   - Delete keys
   - Manage IP whitelists
   - Copy key IDs to clipboard

### 1. CLI Key Generation

```bash
# Generate an admin key
cargo run --bin loxone-mcp-keys -- generate --role admin --name "Main Admin"

# Generate an operator key with 30-day expiration
cargo run --bin loxone-mcp-keys -- generate --role operator --name "Home Assistant" --expires 30

# Generate a monitor key restricted to specific IPs
cargo run --bin loxone-mcp-keys -- generate --role monitor --name "Dashboard" --ip "192.168.1.0/24,10.0.0.0/8"

# Generate a device-specific key
cargo run --bin loxone-mcp-keys -- generate --role device --name "Bedroom Controller" --devices "bedroom-light,bedroom-blinds"
```

### 2. List Keys

```bash
# List all keys
cargo run --bin loxone-mcp-keys -- list

# List only active keys
cargo run --bin loxone-mcp-keys -- list --active

# Export as JSON
cargo run --bin loxone-mcp-keys -- list --format json
```

### 3. Manage Keys

```bash
# Show key details
cargo run --bin loxone-mcp-keys -- show lmcp_admin_001_abc123def456

# Revoke a key
cargo run --bin loxone-mcp-keys -- revoke lmcp_operator_002_xyz789

# Update key properties
cargo run --bin loxone-mcp-keys -- update lmcp_monitor_003_abc --name "New Dashboard" --expires 90
```

## Storage Backends

### File-Based (Default)

Keys are stored in `~/.config/loxone-mcp/keys.toml`:

```toml
[[keys]]
id = "lmcp_admin_001_abc123def456"
name = "Main Admin Key"
role = "admin"
created_by = "admin"
created_at = "2024-01-15T10:00:00Z"
active = true
```

### Environment Variable

For containerized deployments:

```bash
export LOXONE_API_KEYS='[
  {
    "id": "lmcp_admin_001_abc123def456",
    "name": "Admin Key",
    "role": "admin",
    "active": true
  }
]'
```

### Export/Import

```bash
# Export keys
cargo run --bin loxone-mcp-keys -- export --format json --output keys.json

# Import keys
cargo run --bin loxone-mcp-keys -- import keys.json --skip-existing
```

## Role-Based Access Control

### Admin Role
- Full system access
- Can manage other API keys
- Access to all devices and configurations

### Operator Role
- Device control (on/off, dimming, positioning)
- Monitor device states
- Cannot manage API keys or system configuration

### Monitor Role
- Read-only access to all resources
- View device states and sensor data
- Cannot control devices

### Device Role
- Limited to specific devices
- Can control only assigned devices
- Useful for room-specific controllers

### Custom Role
- Define specific permissions
- Fine-grained access control

## Web-Based Key Management UI

The server includes a comprehensive web interface for managing API keys:

### Features
- **Visual Dashboard**: See all keys at a glance with status indicators
- **Easy Key Generation**: Create keys with role selection and optional restrictions
- **IP Whitelisting**: Add IP restrictions with visual tag management
- **Usage Tracking**: View last used time and usage count for each key
- **Real-time Updates**: Changes are immediately reflected
- **Responsive Design**: Works on desktop and mobile devices

### Accessing the UI
```bash
# Start the HTTP server
cargo run --bin loxone-mcp-server -- http --port 3001

# Open in browser
http://localhost:3001/admin/keys
```

### Security Notes
- The `/admin/keys` endpoint itself should be protected in production
- Consider using a reverse proxy with additional authentication
- Always use HTTPS in production environments

## Server Configuration

### Simple Setup

```bash
# 1. Set security level
export SECURITY_LEVEL=production

# 2. Configure Loxone connection
export LOXONE_HOST=192.168.1.100
export LOXONE_USER=admin
export LOXONE_PASS=your-password

# 3. Generate an API key
cargo run --bin loxone-mcp-keys -- generate --role admin --name "Main"
# Output: lmcp_admin_001_abc123def456

# 4. Start server (reads keys from ~/.config/loxone-mcp/keys.toml)
cargo run --bin loxone-mcp-server http
```

### Using Specific Key Store

```bash
# Use custom key store location
cargo run --bin loxone-mcp-server http --key-store /etc/loxone-mcp/keys.toml

# Use environment variable backend
export LOXONE_KEY_BACKEND=env
export LOXONE_API_KEYS='[{"id":"lmcp_admin_001_abc","role":"admin","active":true}]'
cargo run --bin loxone-mcp-server http
```

## Client Usage

### HTTP Headers

```bash
# Using X-API-Key header
curl -H "X-API-Key: lmcp_admin_001_abc123def456" http://localhost:3001/api/devices

# Using Authorization Bearer
curl -H "Authorization: Bearer lmcp_admin_001_abc123def456" http://localhost:3001/api/devices
```

### Different Roles

```bash
# Admin - Full access
curl -H "X-API-Key: lmcp_admin_001_abc" -X POST http://localhost:3001/api/devices/control

# Monitor - Read only (this will fail)
curl -H "X-API-Key: lmcp_monitor_003_xyz" -X POST http://localhost:3001/api/devices/control
# Error: Insufficient permissions

# Device - Limited scope
curl -H "X-API-Key: lmcp_device_004_bed" -X POST http://localhost:3001/api/devices/bedroom-light/on
# Success (if bedroom-light is in allowed devices)
```

## Security Best Practices

### 1. Key Rotation

```bash
# Generate new key
NEW_KEY=$(cargo run --bin loxone-mcp-keys -- generate --role admin --name "Admin Rotated")

# Update applications with new key

# Revoke old key
cargo run --bin loxone-mcp-keys -- revoke lmcp_admin_001_old
```

### 2. IP Restrictions

Always use IP whitelisting in production:

```toml
[[keys]]
id = "lmcp_operator_002_abc"
name = "Home Assistant"
ip_whitelist = ["192.168.1.50"]  # Only from HA server
```

### 3. Expiration Policies

Set expiration for non-admin keys:

```bash
# 90-day expiration for operator keys
cargo run --bin loxone-mcp-keys -- generate --role operator --expires 90

# 30-day expiration for temporary access
cargo run --bin loxone-mcp-keys -- generate --role monitor --expires 30
```

### 4. Audit and Monitoring

The system tracks:
- Last used timestamp
- Usage count
- Failed authentication attempts
- IP addresses of requests

## Migration from Legacy

### From Single API Key

```bash
# Old system
export LOXONE_API_KEY=my-secret-key

# New system - generate proper key
cargo run --bin loxone-mcp-keys -- generate --role admin --name "Migrated"
```

### From Environment Variables

```bash
# Export current config
echo $LOXONE_API_KEY > old-key.txt

# Generate new keys
cargo run --bin loxone-mcp-keys -- generate --role admin --name "Main"

# Update deployment scripts
```

## Troubleshooting

### Key Not Working

1. Check key is active:
   ```bash
   cargo run --bin loxone-mcp-keys -- show lmcp_admin_001_abc
   ```

2. Verify IP restrictions:
   - Ensure your IP is in the whitelist
   - Check CIDR notation is correct

3. Check expiration:
   - Keys may have expired
   - Generate new key if needed

### Permission Denied

1. Verify role has required permissions
2. Admin role needed for:
   - Key management
   - System configuration
3. Operator role needed for:
   - Device control
4. Monitor role limited to:
   - Read-only access

## Summary

The new security architecture provides:

1. **Multi-user support** with individual API keys
2. **Role-based access control** for different permission levels
3. **Flexible storage** (file, env, SQLite)
4. **Security features** (expiration, IP restrictions, audit logging)
5. **Easy management** via CLI tools
6. **Backward compatibility** during migration

This eliminates confusion by having:
- One security level setting (`SECURITY_LEVEL`)
- Clear role definitions
- Standard key format
- Comprehensive CLI tools for management