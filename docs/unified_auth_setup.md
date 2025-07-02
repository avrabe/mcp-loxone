# Unified Authentication System Setup Guide

This guide explains how to set up and use the new unified authentication system for the Loxone MCP Server.

## Overview

The unified authentication system provides:
- **API Key Management**: Create, rotate, and manage API keys
- **Role-Based Access Control**: Admin, Operator, Monitor, Device, and Custom roles
- **SSH-Style Security**: Secure credential storage with proper file permissions
- **Rate Limiting**: Protection against brute-force attacks
- **Audit Logging**: Comprehensive security event tracking

## Initial Setup

### 1. Install the Authentication CLI Tool

```bash
cargo install --path . --bin loxone-mcp-auth
```

### 2. Create Your First API Key

Create an admin key for initial setup:

```bash
loxone-mcp-auth create \
  --name "Admin Key" \
  --role admin \
  --created-by "setup"
```

This will output:
```
✅ API key created successfully!
Key ID: 550e8400-e29b-41d4-a716-446655440000
Secret: lmk_live_abcd1234efgh5678ijkl9012mnop3456
```

**Important**: Save the secret key securely - it won't be shown again!

### 3. Verify Installation

Check that credentials are stored securely:

```bash
loxone-mcp-auth security --check-only
```

Expected output:
```
✅ Checking security of credential files...
✅ Directory ~/.loxone-mcp has secure permissions (700)
✅ File ~/.loxone-mcp/credentials.json has secure permissions (600)
✅ All security checks passed!
```

## Using API Keys

### HTTP Headers

Include your API key in requests using the `Authorization` header:

```bash
curl -H "Authorization: Bearer lmk_live_your_secret_key" \
  http://localhost:3001/api/devices
```

### Query Parameters

Alternatively, use the `api_key` query parameter:

```bash
curl http://localhost:3001/api/devices?api_key=lmk_live_your_secret_key
```

### WebSocket Authentication

For WebSocket connections, include the API key in the connection URL:

```javascript
const ws = new WebSocket('ws://localhost:3001/ws?api_key=lmk_live_your_secret_key');
```

## Role-Based Access Control

### Available Roles

1. **Admin** - Full access to all operations
   - Create/delete API keys
   - Modify system configuration
   - Access all devices and sensors

2. **Operator** - Standard operational access
   - Control devices
   - View sensor data
   - Cannot manage API keys

3. **Monitor** - Read-only access
   - View device states
   - Read sensor data
   - No control operations

4. **Device** - Limited device access
   - Control specific devices
   - No configuration access

5. **Custom** - Configurable permissions
   - Define specific permissions as needed

### Creating Keys with Different Roles

```bash
# Create operator key
loxone-mcp-auth create --name "Operator" --role operator

# Create read-only monitoring key
loxone-mcp-auth create --name "Monitor" --role monitor

# Create device control key
loxone-mcp-auth create --name "Device Control" --role device
```

## Key Management

### List All Keys

```bash
loxone-mcp-auth list
```

### Delete a Key

```bash
loxone-mcp-auth delete --key-id 550e8400-e29b-41d4-a716-446655440000
```

### Test Authentication

```bash
loxone-mcp-auth test \
  --secret lmk_live_your_secret_key \
  --ip 127.0.0.1
```

## Security Best Practices

### 1. Key Rotation

Regularly rotate API keys, especially for production:

```bash
# Create new key
loxone-mcp-auth create --name "New Admin Key" --role admin

# Update your applications with new key

# Delete old key
loxone-mcp-auth delete --key-id old_key_id
```

### 2. Least Privilege

Always use the minimum required role:
- Use `monitor` role for dashboards
- Use `device` role for automation scripts
- Reserve `admin` role for configuration tasks

### 3. Environment-Specific Keys

Create separate keys for different environments:
```bash
loxone-mcp-auth create --name "Development" --role operator
loxone-mcp-auth create --name "Production" --role operator
```

### 4. Secure Storage

The system automatically ensures secure file permissions:
- Directory: `~/.loxone-mcp/` (700 - owner only)
- Files: `credentials.json` (600 - owner read/write only)

To verify security:
```bash
loxone-mcp-auth security --auto-fix
```

## Integration Examples

### Python Client

```python
import httpx

API_KEY = "lmk_live_your_secret_key"

async with httpx.AsyncClient() as client:
    response = await client.get(
        "http://localhost:3001/api/devices",
        headers={"Authorization": f"Bearer {API_KEY}"}
    )
    devices = response.json()
```

### JavaScript/Node.js

```javascript
const API_KEY = 'lmk_live_your_secret_key';

const response = await fetch('http://localhost:3001/api/devices', {
  headers: {
    'Authorization': `Bearer ${API_KEY}`
  }
});
const devices = await response.json();
```

### Home Assistant

```yaml
rest:
  - authentication: bearer
    headers:
      Authorization: !secret loxone_api_key
    resource: http://localhost:3001/api/devices
```

## Troubleshooting

### Authentication Failures

1. Check key validity:
   ```bash
   loxone-mcp-auth test --secret your_key --ip your_ip
   ```

2. Verify key exists:
   ```bash
   loxone-mcp-auth list | grep your_key_id
   ```

3. Check audit logs:
   ```bash
   loxone-mcp-auth audit --limit 10
   ```

### Permission Denied

If you get "permission denied" errors:

1. Check file permissions:
   ```bash
   ls -la ~/.loxone-mcp/
   ```

2. Fix permissions:
   ```bash
   loxone-mcp-auth security --auto-fix
   ```

### Rate Limiting

If you're being rate-limited:
- Default: 10 failed attempts per IP per minute
- Wait 60 seconds before retrying
- Check for typos in your API key

## Migration from Legacy Systems

If you're migrating from environment variables or legacy authentication:

1. Create new API keys using the CLI
2. Update your applications to use the new keys
3. Remove old environment variables:
   ```bash
   unset HTTP_API_KEY
   unset LOXONE_API_KEY
   ```

## Advanced Configuration

### Custom Storage Location

Set a custom credential storage path:

```bash
export LOXONE_CREDENTIAL_PATH=/custom/path/credentials.json
loxone-mcp-auth create --name "Custom Path Key" --role admin
```

### Memory-Only Storage (Testing)

For testing without persistence:

```bash
export LOXONE_AUTH_STORAGE=memory
cargo run --bin loxone-mcp-server
```

## Support

For issues or questions:
1. Check the audit logs: `loxone-mcp-auth audit`
2. Verify security: `loxone-mcp-auth security`
3. Review this guide
4. Open an issue on GitHub