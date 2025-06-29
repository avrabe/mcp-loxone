# pulseengine-mcp-auth

**Authentication and authorization framework for MCP servers**

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/avrabe/mcp-loxone/blob/main/LICENSE)

This crate provides authentication and authorization for MCP servers. It includes API key management, role-based access control, and security features like rate limiting and audit logging.

## What This Provides

**API Key Management:**
- Secure key generation with cryptographic randomness
- Multiple key types (live, test) with different permissions
- Key expiration and rotation support
- Storage with file system permissions (600/700)

**Role-Based Access:**
- Predefined roles: Admin, Operator, Monitor, Device
- Custom role definitions with specific permissions
- Tool-level access control
- IP address whitelisting

**Security Features:**
- Rate limiting with configurable thresholds
- Failed attempt tracking and cooldown periods
- Request size limits
- Audit logging of all authentication events

## Real-World Usage

This authentication system is currently used in production by the **Loxone MCP Server** where it:
- Manages API keys for different client types (admin tools, automation systems, monitoring)
- Enforces role-based access to 30+ home automation tools
- Provides audit trails for security compliance
- Integrates with HTTP and WebSocket transports

## Quick Start

```toml
[dependencies]
pulseengine-mcp-auth = "0.1.1"
pulseengine-mcp-protocol = "0.1.1"
tokio = { version = "1.0", features = ["full"] }
```

## Basic Usage

### Setting Up Authentication

```rust
use pulseengine_mcp_auth::{AuthConfig, AuthManager, Role};

// Configure authentication
let mut config = AuthConfig::default();
config.enabled = true;
config.require_api_key = true;
config.default_role = Role::Monitor; // Least privileged by default

// Initialize the auth manager
let auth_manager = AuthManager::new(config).await?;
```

### Creating API Keys

```rust
use pulseengine_mcp_auth::{CreateKeyRequest, KeyType};

// Create an admin key
let request = CreateKeyRequest {
    name: "Admin Dashboard".to_string(),
    role: Role::Admin,
    key_type: KeyType::Live,
    expires_in_days: Some(90),
    ip_whitelist: Some(vec!["192.168.1.0/24".to_string()]),
    created_by: "setup-script".to_string(),
};

let key_info = auth_manager.create_key(request).await?;
println!("Created key: {}", key_info.secret); // lmk_live_xxxx...
```

### Validating Requests

```rust
use pulseengine_mcp_auth::AuthRequest;

// In your request handler
let auth_request = AuthRequest {
    api_key: Some("lmk_live_1234567890abcdef".to_string()),
    client_ip: "192.168.1.100".parse().ok(),
    tool_name: Some("control_device".to_string()),
    user_agent: Some("MyClient/1.0".to_string()),
};

match auth_manager.validate_request(&auth_request).await {
    Ok(auth_result) => {
        println!("Authenticated as: {}", auth_result.key_info.name);
        println!("Role: {:?}", auth_result.key_info.role);
        // Process the request
    }
    Err(e) => {
        println!("Authentication failed: {}", e);
        // Return 401 Unauthorized
    }
}
```

## Current Status

**Production-ready for basic to intermediate needs.** The core authentication works well and has been tested in real deployment scenarios.

**What's solid:**
- ✅ API key generation and validation
- ✅ Role-based access control
- ✅ Rate limiting and security features
- ✅ File system storage with proper permissions
- ✅ Integration with HTTP transport layer
- ✅ Audit logging and security events

**What could be improved:**
- 🔧 Key rotation could be more automated
- 📝 More examples for different deployment scenarios
- 🧪 Testing utilities for auth scenarios
- 🔧 More granular permission systems

## Key Management

### Creating Different Types of Keys

```rust
// Admin key with full access
let admin_key = CreateKeyRequest {
    name: "System Administrator".to_string(),
    role: Role::Admin,
    key_type: KeyType::Live,
    expires_in_days: Some(365),
    ip_whitelist: None, // Access from anywhere
    created_by: "admin".to_string(),
};

// Operator key for automation systems
let automation_key = CreateKeyRequest {
    name: "Home Automation".to_string(),
    role: Role::Operator,
    key_type: KeyType::Live,
    expires_in_days: None, // No expiration
    ip_whitelist: Some(vec!["192.168.1.10".to_string()]), // Specific device
    created_by: "admin".to_string(),
};

// Monitor key for dashboards
let monitor_key = CreateKeyRequest {
    name: "Status Dashboard".to_string(),
    role: Role::Monitor,
    key_type: KeyType::Live,
    expires_in_days: Some(30),
    ip_whitelist: Some(vec!["192.168.1.0/24".to_string()]), // Local network
    created_by: "admin".to_string(),
};
```

### Managing Keys

```rust
// List all keys
let keys = auth_manager.list_keys().await?;
for key in keys {
    println!("Key: {} ({}), Role: {:?}", key.name, key.id, key.role);
}

// Update key permissions
auth_manager.update_key_whitelist(
    "key-id-here",
    vec!["10.0.0.0/8".to_string()]
).await?;

// Revoke a key
auth_manager.revoke_key("key-id-here").await?;
```

## Role-Based Access Control

### Predefined Roles

```rust
use pulseengine_mcp_auth::Role;

// Admin: Full access to all tools and management functions
Role::Admin

// Operator: Can use tools but not manage keys
Role::Operator

// Monitor: Read-only access to status and monitoring tools
Role::Monitor

// Device: Limited access for IoT devices
Role::Device
```

### Custom Permissions

```rust
use pulseengine_mcp_auth::{Permission, CustomRole};

let custom_role = CustomRole {
    name: "Climate Controller".to_string(),
    permissions: vec![
        Permission::UseTool("get_climate_status".to_string()),
        Permission::UseTool("set_temperature".to_string()),
        Permission::UseResource("climate_data".to_string()),
    ],
};
```

## Security Features

### Rate Limiting

```rust
let mut config = AuthConfig::default();
config.rate_limit.max_requests_per_minute = 60;
config.rate_limit.max_failed_attempts = 5;
config.rate_limit.lockout_duration_minutes = 30;
```

### Audit Logging

```rust
// Authentication events are automatically logged
// Check logs for security monitoring
auth_manager.get_audit_log(None, Some(100)).await?;
```

## Integration with MCP Server

```rust
use mcp_server::ServerConfig;
use pulseengine_mcp_auth::AuthConfig;

let mut auth_config = AuthConfig::default();
auth_config.enabled = true;

let server_config = ServerConfig {
    auth_config,
    // ... other config
};

// Authentication is handled automatically by the server
```

## CLI Tool

The auth system includes a command-line tool for key management:

```bash
# Create a new key
cargo run --bin auth-manager create --name "Test Key" --role operator

# List all keys
cargo run --bin auth-manager list

# Revoke a key
cargo run --bin auth-manager revoke --key-id abc123
```

## Contributing

This authentication system grows from real deployment needs. The most valuable contributions are:

1. **Security improvements** - Better cryptographic practices, security auditing
2. **Integration examples** - How to integrate with different transport layers
3. **Management tools** - Better CLI tools, web interfaces for key management
4. **Testing utilities** - Helpers for testing authenticated endpoints

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

**Repository:** https://github.com/avrabe/mcp-loxone