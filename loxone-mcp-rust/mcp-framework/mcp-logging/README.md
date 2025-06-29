# pulseengine-mcp-logging

**Structured logging framework for MCP servers**

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/avrabe/mcp-loxone/blob/main/LICENSE)

This crate provides structured logging specifically designed for MCP servers, with automatic credential sanitization, request correlation, and security-focused log management.

## What This Provides

**Structured Logging:**
- JSON output with consistent field names
- Correlation IDs for tracking requests across components
- Log levels with appropriate filtering
- Contextual information (tool names, client IPs, etc.)

**Security Features:**
- Automatic credential scrubbing from logs
- Sensitive parameter filtering
- Request sanitization before logging
- Audit trail capabilities

**MCP-Specific Features:**
- Tool execution logging with parameters
- Protocol message logging (request/response)
- Transport layer activity tracking
- Performance metrics integration

## Real-World Usage

This logging system is actively used in the **Loxone MCP Server** where it:
- Logs all 30+ tool executions with sanitized parameters
- Tracks authentication attempts and API key usage
- Provides audit trails for home automation commands
- Integrates with system monitoring for alerting
- Sanitizes device credentials and API keys from logs

## Quick Start

```toml
[dependencies]
pulseengine-mcp-logging = "0.2.0"
tracing = "0.1"
serde_json = "1.0"
```

## Basic Usage

### Initialize Logging

```rust
use pulseengine_mcp_logging::{LoggingConfig, init_logging};

// Configure structured logging
let config = LoggingConfig {
    level: "info".to_string(),
    format: mcp_logging::LogFormat::Json,
    enable_correlation_ids: true,
    enable_sanitization: true,
    output_file: Some("/var/log/mcp-server.log".to_string()),
};

// Initialize the logging system
init_logging(config)?;
```

### Basic Logging

```rust
use tracing::{info, warn, error};
use pulseengine_mcp_logging::log_tool_execution;

// Standard structured logging
info!(
    tool = "get_weather",
    location = "San Francisco",
    duration_ms = 150,
    "Tool executed successfully"
);

// MCP-specific logging
log_tool_execution(
    "control_device",
    &serde_json::json!({"device": "living_room_light", "action": "on"}),
    Ok("Device controlled successfully"),
    150, // duration in ms
);
```

### Request Correlation

```rust
use pulseengine_mcp_logging::{CorrelationId, with_correlation_id};

// Generate correlation ID for a request
let correlation_id = CorrelationId::new();

// All logs within this scope will include the correlation ID
with_correlation_id(correlation_id, async {
    info!("Processing MCP request");
    
    // Your request handling logic
    handle_tool_call().await?;
    
    info!("Request completed successfully");
}).await;
```

## Current Status

**Solid foundation with good security practices.** The logging system handles the most important concerns well and integrates cleanly with the rest of the framework.

**What works well:**
- ✅ Structured JSON logging with consistent format
- ✅ Automatic credential sanitization
- ✅ Request correlation tracking
- ✅ MCP-specific logging utilities
- ✅ Integration with standard Rust logging ecosystem
- ✅ File rotation and output management

**Areas for improvement:**
- 📊 Better integration with metrics systems
- 🔧 More sophisticated log analysis tools
- 📝 More examples for different deployment scenarios
- 🧪 Testing utilities for log validation

## Security and Sanitization

### Automatic Credential Scrubbing

```rust
use pulseengine_mcp_logging::sanitize_for_logging;

// Automatically removes sensitive data
let safe_params = sanitize_for_logging(&serde_json::json!({
    "username": "admin",
    "password": "secret123",  // Will be redacted
    "api_key": "abc123",      // Will be redacted
    "device": "living_room_light"
}));

info!(params = ?safe_params, "Tool called");
// Logs: {"params": {"username": "admin", "password": "[REDACTED]", "api_key": "[REDACTED]", "device": "living_room_light"}}
```

### Custom Sanitization Rules

```rust
use pulseengine_mcp_logging::{SanitizationConfig, SanitizationRule};

let sanitization_config = SanitizationConfig {
    rules: vec![
        SanitizationRule::field_name("password"),
        SanitizationRule::field_name("token"),
        SanitizationRule::field_name("secret"),
        SanitizationRule::pattern(r"(?i)api[_-]?key"),
        SanitizationRule::custom("device_credential", |value| {
            // Custom scrubbing logic
            "[DEVICE_CREDENTIAL]".to_string()
        }),
    ],
    redaction_text: "[SANITIZED]".to_string(),
};
```

### Security Audit Logging

```rust
use pulseengine_mcp_logging::audit_log;

// Log security-relevant events
audit_log!(
    event = "authentication_attempt",
    client_ip = "192.168.1.100",
    api_key_id = "key_123",
    success = true,
    "Client authenticated successfully"
);

audit_log!(
    event = "privileged_tool_access",
    tool = "control_all_devices",
    user_role = "admin",
    client_ip = "192.168.1.100",
    "Admin executed system-wide control"
);
```

## MCP-Specific Logging

### Tool Execution Logging

```rust
use pulseengine_mcp_logging::{log_tool_start, log_tool_success, log_tool_error};

// Start tool execution
let execution_id = log_tool_start("get_device_status", &params);

// Tool execution logic...
match execute_tool().await {
    Ok(result) => {
        log_tool_success(execution_id, &result, duration_ms);
    }
    Err(error) => {
        log_tool_error(execution_id, &error, duration_ms);
    }
}
```

### Protocol Message Logging

```rust
use pulseengine_mcp_logging::{log_request, log_response};

// Log incoming requests
log_request(&mcp_request, correlation_id, client_info);

// Log outgoing responses
log_response(&mcp_response, correlation_id, response_time_ms);
```

### Transport Activity

```rust
use pulseengine_mcp_logging::log_transport_event;

// Log transport-specific events
log_transport_event!(
    transport = "http",
    event = "connection_established",
    client_ip = "192.168.1.100",
    user_agent = "MCP-Inspector/1.0",
    "New HTTP connection"
);

log_transport_event!(
    transport = "websocket",
    event = "message_received",
    message_type = "tool_call",
    size_bytes = 256,
    "WebSocket message processed"
);
```

## Configuration

### Log Levels and Filtering

```rust
use pulseengine_mcp_logging::LoggingConfig;

let config = LoggingConfig {
    level: "info".to_string(),
    module_filters: vec![
        ("mcp_server".to_string(), "debug".to_string()),
        ("hyper".to_string(), "warn".to_string()),
        ("tokio".to_string(), "error".to_string()),
    ],
    // ... other config
};
```

### Output Destinations

```rust
let config = LoggingConfig {
    outputs: vec![
        LogOutput::Stdout,
        LogOutput::File {
            path: "/var/log/mcp-server.log".to_string(),
            rotate_size_mb: 100,
            max_files: 10,
        },
        LogOutput::Syslog {
            facility: "daemon".to_string(),
            identifier: "mcp-server".to_string(),
        },
    ],
    // ... other config
};
```

### JSON vs Human-Readable Format

```rust
// For production - structured JSON
let prod_config = LoggingConfig {
    format: LogFormat::Json,
    include_timestamps: true,
    include_correlation_ids: true,
    // ...
};

// For development - human-readable
let dev_config = LoggingConfig {
    format: LogFormat::Pretty,
    enable_colors: true,
    // ...
};
```

## Integration Examples

### With MCP Server

```rust
use mcp_server::ServerConfig;
use pulseengine_mcp_logging::LoggingConfig;

let logging_config = LoggingConfig {
    level: "info".to_string(),
    format: LogFormat::Json,
    enable_sanitization: true,
    // ... other config
};

// Initialize logging before starting server
mcp_logging::init_logging(logging_config)?;

let server = McpServer::new(backend, config).await?;
server.run().await?;
```

### With Authentication System

```rust
use mcp_auth::AuthManager;
use pulseengine_mcp_logging::audit_log;

// Log authentication events
let auth_result = auth_manager.validate_request(&request).await;
match auth_result {
    Ok(auth_info) => {
        audit_log!(
            event = "auth_success",
            key_id = auth_info.key_id,
            role = ?auth_info.role,
            client_ip = ?request.client_ip,
            "Authentication successful"
        );
    }
    Err(e) => {
        audit_log!(
            event = "auth_failure",
            error = %e,
            client_ip = ?request.client_ip,
            "Authentication failed"
        );
    }
}
```

## Real-World Examples

### Loxone Server Logging

```rust
// Log home automation commands
info!(
    tool = "control_rolladen",
    room = "living_room",
    action = "down", 
    device_count = 3,
    duration_ms = 1200,
    "Rolladen control completed"
);

// Log device status queries
debug!(
    tool = "get_climate_status",
    room_count = 6,
    sensor_count = 12,
    cache_hit = true,
    duration_ms = 45,
    "Climate status retrieved"
);

// Log security events
audit_log!(
    event = "device_control",
    tool = "control_all_lights",
    action = "off",
    affected_devices = 15,
    client_ip = "192.168.1.50",
    "System-wide light control executed"
);
```

## Contributing

Logging is fundamental to operational visibility. Most valuable contributions:

1. **Security improvements** - Better sanitization rules and audit capabilities
2. **Performance optimization** - Low-overhead logging for high-throughput servers
3. **Integration examples** - How to integrate with log aggregation systems
4. **Analysis tools** - Utilities for analyzing MCP server logs

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

**Repository:** https://github.com/avrabe/mcp-loxone