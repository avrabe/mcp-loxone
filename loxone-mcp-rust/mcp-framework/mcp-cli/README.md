# pulseengine-mcp-cli

**CLI integration and configuration framework for MCP servers**

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/avrabe/mcp-loxone/blob/main/LICENSE)

This crate provides automatic CLI generation, configuration management, and server setup for MCP servers. It eliminates boilerplate code and provides a modern, ergonomic API for building MCP servers.

## Features

- **Automatic CLI Generation**: Generate command-line interfaces from configuration structs
- **Configuration Management**: Type-safe configuration with environment variable support
- **Server Integration**: Seamless integration with the MCP server framework
- **Logging Setup**: Built-in structured logging configuration
- **Builder Patterns**: Fluent APIs for server configuration

## Quick Start

```toml
[dependencies]
pulseengine-mcp-cli = "0.2.0"
pulseengine-mcp-server = "0.2.0"
```

```rust
use pulseengine_mcp_cli::{McpConfig, run_server};
use clap::Parser;

#[derive(McpConfig, Parser)]
struct MyServerConfig {
    #[clap(short, long, default_value = "8080")]
    port: u16,
    
    #[clap(short, long)]
    database_url: String,
    
    #[mcp(auto_populate)]
    server_info: ServerInfo,
    
    #[mcp(logging(level = "info", format = "json"))]
    logging: LoggingConfig,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = MyServerConfig::parse();
    run_server(config).await?;
    Ok(())
}
```

## Current Status

**Early Development**: This crate is part of the upcoming v0.2.0 release of the MCP framework. APIs are subject to change.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.