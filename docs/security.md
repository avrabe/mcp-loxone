# Security Documentation

This document outlines the security features and best practices for the Loxone MCP Server.

## Overview

The Loxone MCP Server implements multiple layers of security to protect both the MCP interface and the underlying Loxone system.

## Authentication

### API Key Authentication

The server uses API key authentication for all requests:

```bash
# Generate a new API key
cargo run --bin loxone-mcp-auth create --name "My Integration" --role operator

# Example output:
# API Key: lmk_1234567890abcdef
# Key ID: key_abc123
# Role: operator
```

### Roles and Permissions

| Role | Permissions | Rate Limit |
|------|-------------|------------|
| `admin` | Full access to all tools | 1000/min |
| `operator` | Device control, no security changes | 100/min |
| `viewer` | Read-only access | 10/min |

### Using API Keys

**HTTP Header**:
```
Authorization: Bearer lmk_1234567890abcdef
```

**Environment Variable**:
```bash
export LOXONE_API_KEY=lmk_1234567890abcdef
```

## Loxone Credentials

### Secure Storage

Credentials are stored securely using the system keychain:

- **macOS**: Keychain Access
- **Linux**: Secret Service API
- **Windows**: Windows Credential Store

### Setup Process

```bash
# Interactive setup (recommended)
cargo run --bin loxone-mcp-setup

# Verify credentials
cargo run --bin loxone-mcp-auth test <credential-id>
```

### Environment Variables (Development Only)

For development environments only:

```bash
export LOXONE_HOST="http://192.168.1.100"
export LOXONE_USER="username"
export LOXONE_PASS="password"
```

**Warning**: Never use environment variables for production deployments.

## Input Validation

All inputs are validated before processing:

### UUID Validation

```rust
// Valid Loxone UUID format
"0f869a3f-0155-8b3f-ffff403fb0c34b9e"

// Validation includes:
- Format checking
- Character set validation
- Length verification
```

### Command Sanitization

Commands are validated against allowed values:

- Light commands: `on`, `off`, `0-100`
- Blind commands: `up`, `down`, `stop`, `shade`, `0-100`
- Climate modes: `comfort`, `eco`, `off`

### Parameter Validation

- String parameters: Length limits, character restrictions
- Numeric parameters: Range validation
- Arrays: Size limits, element validation

## Rate Limiting

### Implementation

Rate limiting uses a sliding window algorithm:

```toml
[security.rate_limits]
admin = 1000      # per minute
operator = 100    # per minute
viewer = 10       # per minute
```

### Headers

Rate limit information in response headers:

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1706532000
```

### Bypass for Local Development

```bash
# Disable rate limiting for development
export LOXONE_DISABLE_RATE_LIMIT=true
```

## Network Security

### TLS/HTTPS

For production deployments:

```bash
# Run with TLS
./loxone-mcp-server http --port 3001 --tls-cert cert.pem --tls-key key.pem
```

### IP Whitelisting

Restrict access by IP address:

```bash
# Configure allowed IPs
cargo run --bin loxone-mcp-auth update key_abc123 --ip-whitelist "192.168.1.0/24,10.0.0.5"
```

### CORS Configuration

For web clients, configure CORS:

```toml
[security.cors]
allowed_origins = ["https://app.example.com"]
allowed_methods = ["GET", "POST"]
max_age = 3600
```

## Audit Logging

### What's Logged

All security-relevant events are logged:

- Authentication attempts (success/failure)
- Authorization failures
- Rate limit violations
- Configuration changes
- Device control commands

### Log Format

```json
{
  "timestamp": "2024-01-29T10:30:00Z",
  "event": "auth.success",
  "user": "key_abc123",
  "ip": "192.168.1.100",
  "action": "control_lights_unified",
  "result": "success"
}
```

### Viewing Audit Logs

```bash
# View recent audit events
cargo run --bin loxone-mcp-auth audit --limit 50

# Filter by event type
cargo run --bin loxone-mcp-auth audit --event auth.failure
```

## Security Best Practices

### 1. Principle of Least Privilege

- Create separate API keys for different integrations
- Use `viewer` role for monitoring/dashboard applications
- Use `operator` role for automation systems
- Reserve `admin` role for configuration changes

### 2. Key Rotation

Regularly rotate API keys:

```bash
# Create new key
cargo run --bin loxone-mcp-auth create --name "New Key" --role operator

# Revoke old key
cargo run --bin loxone-mcp-auth revoke key_old123
```

### 3. Network Isolation

- Run MCP server in isolated network segment
- Use firewall rules to restrict access
- Enable TLS for all production deployments

### 4. Monitoring

Monitor for suspicious activity:

```bash
# Check for failed auth attempts
cargo run --bin loxone-mcp-auth audit --event auth.failure --limit 100

# Review rate limit violations
cargo run --bin loxone-mcp-auth audit --event rate_limit.exceeded
```

## Vulnerability Reporting

If you discover a security vulnerability:

1. **Do NOT** create a public GitHub issue
2. Email security details to: security@[domain]
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

We aim to respond within 48 hours and provide fixes promptly.

## Security Checklist

Before deploying to production:

- [ ] API keys generated and stored securely
- [ ] Loxone credentials in keychain (not environment variables)
- [ ] TLS enabled with valid certificates
- [ ] IP whitelisting configured if needed
- [ ] Rate limits appropriate for use case
- [ ] Audit logging enabled and monitored
- [ ] Network properly segmented
- [ ] Regular key rotation scheduled
- [ ] Monitoring alerts configured
- [ ] Incident response plan in place

## Compliance

The server includes features to support:

- **GDPR**: Audit logs, data minimization
- **SOC 2**: Access controls, audit trails
- **ISO 27001**: Security controls, monitoring

Note: Compliance certification is the responsibility of the deployment organization.

## Updates and Patches

Stay secure with regular updates:

```bash
# Check for updates
cargo update

# Run security audit
cargo audit

# Update to latest version
git pull && cargo build --release
```

Subscribe to security announcements:
- GitHub repository watches
- Security mailing list (if available)

## Additional Resources

- [OWASP Security Guidelines](https://owasp.org/)
- [Rust Security Best Practices](https://anssi-fr.github.io/rust-guide/)
- [MCP Security Considerations](https://modelcontextprotocol.io/docs/security)