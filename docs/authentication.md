# Authentication Documentation

## Overview

The Loxone MCP Server supports multiple authentication methods to work with different Loxone Miniserver versions and deployment scenarios.

## Authentication Methods

### 1. Unified API Key Authentication (Recommended)

The new unified authentication system provides enterprise-grade security with API key management, role-based access control, and comprehensive audit logging.

**Setup Guide**: See [UNIFIED_AUTH_SETUP.md](./UNIFIED_AUTH_SETUP.md) for detailed instructions.

**Key Features**:
- SSH-style secure credential storage
- Role-based access control (Admin, Operator, Monitor, Device, Custom)
- Rate limiting and brute-force protection (blocks after 4 failed attempts)
- IP whitelisting with CIDR notation support (e.g., `192.168.1.0/24`)
- Comprehensive audit logging
- API key rotation and expiration
- Automatic background cache refresh

**Usage**:
```bash
# HTTP Header
curl -H "Authorization: Bearer lmk_live_your_api_key" http://localhost:3001/api/devices

# Query Parameter
curl http://localhost:3001/api/devices?api_key=lmk_live_your_api_key
```

### 2. Loxone Token Authentication (Native)

For direct integration with Loxone Miniserver's native authentication system. Supports Loxone V10+ with RSA encryption and JWT tokens.

**Documentation**: See [TOKEN_AUTHENTICATION.md](./TOKEN_AUTHENTICATION.md) for implementation details.

**Features**:
- Full Loxone token flow implementation
- RSA public key exchange
- HMAC-SHA256 signature generation
- JWT token management with automatic refresh
- AES session key encryption
- WebSocket token authentication (shares tokens with HTTP client)

**Usage**:
```rust
let config = LoxoneConfig {
    auth_method: AuthMethod::Token,
    // ... other config
};
```

### 3. Basic HTTP Authentication (Legacy)

For older Loxone Miniservers (V8 and below) or simple deployments.

**Features**:
- Standard HTTP Basic authentication
- Compatible with all Miniserver versions
- Simple username/password credentials

**Usage**:
```rust
let config = LoxoneConfig {
    auth_method: AuthMethod::Basic,
    // ... other config
};
```

## Security Best Practices

### For Production Deployments

1. **Use Unified API Key Authentication**
   - Provides the best security and control
   - Enables role-based access control
   - Includes audit logging

2. **Enable HTTPS/TLS**
   - Always use encrypted connections in production
   - Configure proper SSL certificates

3. **Implement Rate Limiting**
   - Built into unified auth system
   - Prevents brute-force attacks

4. **Regular Key Rotation**
   - Rotate API keys periodically
   - Remove unused keys promptly

5. **Configure IP Whitelisting**
   - Restrict API keys to specific IP ranges
   - Use CIDR notation for subnets:
     ```bash
     # Allow specific IP
     loxone-mcp-auth update key_id --ip-whitelist "192.168.1.100"
     
     # Allow subnet
     loxone-mcp-auth update key_id --ip-whitelist "192.168.1.0/24"
     
     # Allow multiple ranges
     loxone-mcp-auth update key_id --ip-whitelist "192.168.1.0/24,10.0.0.0/16"
     ```

### For Development

1. **Use Memory Storage**
   ```bash
   export LOXONE_AUTH_STORAGE=memory
   ```

2. **Create Development Keys**
   ```bash
   loxone-mcp-auth create --name "Dev Key" --role operator
   ```

## Authentication Flow Comparison

| Feature | Unified API Key | Loxone Token | Basic Auth |
|---------|----------------|--------------|------------|
| Security Level | High | High | Medium |
| Miniserver Version | All | V10+ | All |
| Role-Based Access | ✅ | ❌ | ❌ |
| Audit Logging | ✅ | ❌ | ❌ |
| Rate Limiting | ✅ | ❌ | ❌ |
| IP Whitelisting | ✅ (CIDR) | ❌ | ❌ |
| WebSocket Support | ✅ | ✅ | ✅ |
| Key Rotation | ✅ | ✅ | ❌ |
| Background Cache | ✅ | ❌ | ❌ |
| Setup Complexity | Low | Medium | Low |

## Migration Guide

### From Environment Variables

If you were using `HTTP_API_KEY` environment variable:

1. Create a new API key:
   ```bash
   loxone-mcp-auth create --name "Migrated Key" --role admin
   ```

2. Update your application to use the new key

3. Remove the old environment variable:
   ```bash
   unset HTTP_API_KEY
   ```

### From Basic Auth to Unified Auth

1. Keep basic auth for Loxone communication
2. Add unified auth for client access:
   ```rust
   // Loxone connection still uses basic auth
   let loxone_config = LoxoneConfig {
       auth_method: AuthMethod::Basic,
       // ...
   };
   
   // Client access uses unified auth
   // Configured automatically via middleware
   ```

## Troubleshooting

### Common Issues

1. **"Authentication failed"**
   - Verify API key is correct
   - Check key hasn't expired
   - Ensure proper role permissions

2. **"Rate limited"**
   - Too many failed attempts (5 attempts in 15 minutes)
   - Wait 30 minutes before retrying

3. **"Permission denied"**
   - Key lacks required role
   - Check with `loxone-mcp-auth list`

### Debug Mode

Enable debug logging:
```bash
export RUST_LOG=debug
cargo run --bin loxone-mcp-server
```

## API Reference

### Authentication Endpoints

- `GET /api/auth/verify` - Verify current authentication
- `GET /api/auth/permissions` - List current permissions
- `POST /api/auth/refresh` - Refresh token (Loxone tokens only)

### Headers

- `Authorization: Bearer {api_key}` - Unified API key
- `Authorization: LoxToken {token}` - Loxone JWT token
- `Authorization: Basic {base64}` - HTTP Basic auth

## Further Reading

- [Unified Auth Setup Guide](./UNIFIED_AUTH_SETUP.md)
- [Token Authentication Details](./TOKEN_AUTHENTICATION.md)
- [Security Best Practices](./SECURITY.md)