# pulseengine-mcp-security

**Security middleware and input validation for MCP servers**

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/avrabe/mcp-loxone/blob/main/LICENSE)

This crate provides security middleware for MCP servers, including input validation, request sanitization, and protection against common web vulnerabilities.

## What This Protects Against

**Input Validation:**
- JSON injection attacks
- XSS prevention in tool parameters
- SQL injection prevention (when parameters go to databases)
- Path traversal attacks in file operations
- Command injection in system tools

**Request Protection:**
- Request size limits to prevent DoS
- Rate limiting integration
- CORS policy enforcement
- Security headers (HSTS, CSP, etc.)
- Parameter type validation

## Real-World Testing

This security layer is actively used in the **Loxone MCP Server** where it:
- Validates 30+ tool parameters against injection attacks
- Sanitizes device names and commands for safe execution
- Protects file system operations from path traversal
- Enforces request size limits for HTTP transport
- Integrates with authentication for complete security

## Quick Start

```toml
[dependencies]
pulseengine-mcp-security = "0.1.1"
pulseengine-mcp-protocol = "0.1.1"
serde_json = "1.0"
```

## Basic Usage

### Input Validation

```rust
use pulseengine_mcp_security::{SecurityValidator, ValidationRules};
use mcp_protocol::CallToolRequestParam;

// Create validator with rules
let validator = SecurityValidator::new(ValidationRules {
    max_string_length: 1000,
    max_array_size: 100,
    allow_html: false,
    allow_scripts: false,
    max_depth: 10,
});

// Validate tool parameters
let request = CallToolRequestParam {
    name: "get_file".to_string(),
    arguments: Some(serde_json::json!({
        "path": "/safe/path/file.txt"
    })),
};

match validator.validate_tool_request(&request) {
    Ok(sanitized_request) => {
        // Safe to process
        println!("Request is safe: {:?}", sanitized_request);
    }
    Err(e) => {
        println!("Security violation: {}", e);
        // Return error to client
    }
}
```

### CORS Configuration

```rust
use pulseengine_mcp_security::{CorsConfig, SecurityMiddleware};

let cors_config = CorsConfig {
    allow_origins: vec![
        "https://your-dashboard.com".to_string(),
        "http://localhost:3000".to_string(), // Development
    ],
    allow_methods: vec!["GET".to_string(), "POST".to_string()],
    allow_headers: vec![
        "Content-Type".to_string(),
        "Authorization".to_string(),
    ],
    max_age: 3600,
};

let middleware = SecurityMiddleware::new(cors_config);
```

### Request Size Limits

```rust
use pulseengine_mcp_security::SecurityConfig;

let config = SecurityConfig {
    max_request_size: 1024 * 1024, // 1MB limit
    max_parameter_count: 50,
    max_string_length: 10000,
    enable_xss_protection: true,
    enable_sql_injection_protection: true,
};
```

## Current Status

**Solid foundation with room for growth.** The basic security validations work well and catch common attack vectors, but this area can always be improved.

**What works reliably:**
- ✅ Basic input sanitization and validation
- ✅ Request size and parameter limits
- ✅ CORS policy enforcement
- ✅ XSS prevention in string parameters
- ✅ Path traversal prevention
- ✅ Integration with mcp-server framework

**Areas for improvement:**
- 🔧 More sophisticated injection detection
- 📝 Better examples for different attack scenarios
- 🧪 Security testing utilities
- 🔧 More granular validation rules

## Validation Features

### String Sanitization

```rust
use pulseengine_mcp_security::sanitize_string;

// Remove potentially dangerous content
let clean = sanitize_string(
    "<script>alert('xss')</script>Hello World",
    &ValidationRules::default()
);
// Result: "Hello World"

// Validate file paths
let safe_path = sanitize_file_path("../../../etc/passwd")?;
// Error: Path traversal attempt detected
```

### Parameter Validation

```rust
use pulseengine_mcp_security::validate_parameters;

let params = serde_json::json!({
    "device_name": "Living Room Light",
    "action": "on",
    "brightness": 75
});

// Validate against schema and security rules
let validated = validate_parameters(&params, &schema, &security_rules)?;
```

### SQL Injection Prevention

```rust
use pulseengine_mcp_security::check_sql_injection;

let user_input = "'; DROP TABLE users; --";
if check_sql_injection(user_input) {
    return Err("SQL injection attempt detected".into());
}
```

## Middleware Integration

### With HTTP Transport

```rust
use pulseengine_mcp_security::SecurityMiddleware;
use axum::{Router, middleware};

let app = Router::new()
    .route("/mcp", post(handle_mcp_request))
    .layer(middleware::from_fn(SecurityMiddleware::validate_request))
    .layer(middleware::from_fn(SecurityMiddleware::cors_handler));
```

### With MCP Server

```rust
use mcp_server::ServerConfig;
use pulseengine_mcp_security::SecurityConfig;

let security_config = SecurityConfig {
    enable_validation: true,
    max_request_size: 1024 * 1024,
    // ... other security settings
};

let server_config = ServerConfig {
    security_config,
    // ... other config
};

// Security validation happens automatically
```

## Security Rules

### Predefined Rule Sets

```rust
use pulseengine_mcp_security::{ValidationRules, SecurityLevel};

// Strict security for public-facing servers
let strict_rules = ValidationRules::strict();

// Moderate security for internal tools
let moderate_rules = ValidationRules::moderate();

// Minimal security for development
let dev_rules = ValidationRules::development();
```

### Custom Validation Rules

```rust
use pulseengine_mcp_security::ValidationRules;

let custom_rules = ValidationRules {
    max_string_length: 500,
    max_array_size: 20,
    allow_html: false,
    allow_scripts: false,
    max_depth: 5,
    blocked_patterns: vec![
        r"(?i)(union|select|insert|delete|drop|exec)".to_string(),
        r"<script[^>]*>.*?</script>".to_string(),
    ],
    required_patterns: vec![
        // Must match UUID format for device IDs
        r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$".to_string(),
    ],
};
```

## Security Headers

```rust
use pulseengine_mcp_security::SecurityHeaders;

let headers = SecurityHeaders::strict()
    .with_hsts(31536000) // 1 year
    .with_csp("default-src 'self'; script-src 'none'")
    .with_frame_options("DENY")
    .with_content_type_options()
    .with_referrer_policy("strict-origin-when-cross-origin");
```

## Real-World Examples

### File Access Validation

```rust
// From Loxone implementation - validating file paths
use pulseengine_mcp_security::validate_file_path;

fn handle_read_file(path: &str) -> Result<String, SecurityError> {
    // Prevent path traversal
    let safe_path = validate_file_path(path)?;
    
    // Ensure it's within allowed directory
    if !safe_path.starts_with("/safe/directory/") {
        return Err(SecurityError::unauthorized_path(path));
    }
    
    // Safe to read file
    std::fs::read_to_string(safe_path)
}
```

### Device Command Validation

```rust
// Validate device control commands
fn validate_device_command(device: &str, action: &str) -> Result<(), SecurityError> {
    // Validate device identifier (prevent injection)
    if !device.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err(SecurityError::invalid_device_name(device));
    }
    
    // Validate action against whitelist
    let allowed_actions = ["on", "off", "dim", "up", "down", "stop"];
    if !allowed_actions.contains(&action) {
        return Err(SecurityError::invalid_action(action));
    }
    
    Ok(())
}
```

## Contributing

Security is an ongoing concern and improvements are always welcome. Most valuable contributions:

1. **Security research** - Finding new attack vectors or validation gaps
2. **Performance optimization** - Security checks with minimal overhead
3. **Testing utilities** - Tools for security testing and validation
4. **Real-world examples** - Security patterns from actual deployments

If you find a security issue, please follow responsible disclosure practices.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

**Repository:** https://github.com/avrabe/mcp-loxone