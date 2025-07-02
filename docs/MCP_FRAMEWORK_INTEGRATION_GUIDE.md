# MCP Framework Integration Guide for Backend Developers

<!--
SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
-->

## Overview

This guide provides detailed instructions for developers who want to create new MCP server implementations using the **mcp-framework**. The framework provides all the infrastructure (transport, authentication, security, monitoring) while you focus on implementing your domain-specific business logic.

## Framework Architecture

The mcp-framework consists of 7 specialized crates that handle all MCP protocol concerns:

```
mcp-protocol    → Core MCP types and serialization
mcp-transport   → HTTP, WebSocket, SSE, Stdio transports  
mcp-server      → Server infrastructure and routing
mcp-auth        → Authentication and authorization
mcp-security    → Input validation and sanitization
mcp-performance → Monitoring and optimization
mcp-monitoring  → Health checks and observability
```

Your backend implementation plugs into this framework by implementing the `McpBackend` trait.

## Quick Start Example

Let's create a simple "File Manager" MCP server to demonstrate the integration:

### 1. Create New Project

```bash
# Create new Rust project
cargo new --bin file-manager-mcp
cd file-manager-mcp

# Add framework dependencies to Cargo.toml
```

**Cargo.toml**:
```toml
[dependencies]
# MCP Framework (from crates.io)
pulseengine-mcp-server = "0.3.1"
pulseengine-mcp-protocol = "0.3.1"
pulseengine-mcp-transport = "0.3.1"
pulseengine-mcp-auth = "0.3.1"

# Standard dependencies
tokio = { version = "1.0", features = ["full"] }
async-trait = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

### 2. Implement Your Backend

**src/main.rs**:
```rust
//! File Manager MCP Server Example
//!
//! This demonstrates how to create a complete MCP server using the mcp-framework.

use mcp_server::{McpServer, ServerConfig, McpBackend};
use mcp_protocol::*;
use mcp_transport::TransportConfig;

use async_trait::async_trait;
use serde_json::json;
use thiserror::Error;
use tracing::info;
use tracing_subscriber::EnvFilter;
use std::path::PathBuf;

/// Backend-specific error types
#[derive(Debug, Error)]
pub enum FileManagerError {
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("File not found: {0}")]
    FileNotFound(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Convert backend errors to MCP protocol errors
impl From<FileManagerError> for Error {
    fn from(err: FileManagerError) -> Self {
        match err {
            FileManagerError::InvalidPath(msg) => Error::invalid_params(msg),
            FileManagerError::PermissionDenied(msg) => Error::invalid_params(msg),
            FileManagerError::FileNotFound(msg) => Error::invalid_params(msg),
            FileManagerError::IoError(err) => Error::internal_error(err.to_string()),
        }
    }
}

/// Backend configuration
#[derive(Debug, Clone)]
pub struct FileManagerConfig {
    pub base_directory: PathBuf,
    pub read_only: bool,
}

impl Default for FileManagerConfig {
    fn default() -> Self {
        Self {
            base_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            read_only: false,
        }
    }
}

/// File Manager backend implementation
#[derive(Clone)]
pub struct FileManagerBackend {
    config: FileManagerConfig,
}

#[async_trait]
impl McpBackend for FileManagerBackend {
    type Error = FileManagerError;
    type Config = FileManagerConfig;
    
    /// Initialize the backend
    async fn initialize(config: Self::Config) -> std::result::Result<Self, Self::Error> {
        info!("Initializing File Manager backend with base directory: {:?}", config.base_directory);
        
        // Validate base directory exists
        if !config.base_directory.exists() {
            return Err(FileManagerError::FileNotFound(
                format!("Base directory does not exist: {:?}", config.base_directory)
            ));
        }
        
        Ok(Self { config })
    }
    
    /// Define server capabilities and information
    fn get_server_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                resources: Some(ResourcesCapability {
                    subscribe: Some(false),
                    list_changed: Some(false),
                }),
                prompts: Some(PromptsCapability {
                    list_changed: Some(false),
                }),
                logging: Some(LoggingCapability {}),
                sampling: None,
            },
            server_info: Implementation {
                name: "File Manager MCP Server".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some("File management operations with security restrictions".to_string()),
        }
    }
    
    /// Health check implementation
    async fn health_check(&self) -> std::result::Result<(), Self::Error> {
        // Verify base directory is still accessible
        if !self.config.base_directory.exists() {
            return Err(FileManagerError::FileNotFound(
                "Base directory no longer exists".to_string()
            ));
        }
        Ok(())
    }
    
    /// List available tools
    async fn list_tools(&self, _request: PaginatedRequestParam) -> std::result::Result<ListToolsResult, Self::Error> {
        let mut tools = vec![
            Tool {
                name: "list_files".to_string(),
                description: "List files and directories in a path".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path to list (relative to base directory)",
                            "default": "."
                        }
                    },
                    "required": []
                }),
            },
            Tool {
                name: "read_file".to_string(),
                description: "Read contents of a text file".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path to read (relative to base directory)"
                        }
                    },
                    "required": ["path"]
                }),
            },
            Tool {
                name: "get_file_info".to_string(),
                description: "Get detailed information about a file or directory".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File or directory path"
                        }
                    },
                    "required": ["path"]
                }),
            },
        ];
        
        // Add write operations if not read-only
        if !self.config.read_only {
            tools.push(Tool {
                name: "write_file".to_string(),
                description: "Write content to a file".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path to write to"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write to the file"
                        }
                    },
                    "required": ["path", "content"]
                }),
            });
        }
        
        Ok(ListToolsResult {
            tools,
            next_cursor: String::new(),
        })
    }
    
    /// Handle tool execution
    async fn call_tool(&self, request: CallToolRequestParam) -> std::result::Result<CallToolResult, Self::Error> {
        let args = request.arguments.unwrap_or(serde_json::Value::Object(Default::default()));
        
        match request.name.as_str() {
            "list_files" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                
                let full_path = self.resolve_path(path)?;
                let entries = self.list_directory(&full_path).await?;
                
                Ok(CallToolResult {
                    content: vec![Content::text(serde_json::to_string_pretty(&entries)?)],
                    is_error: Some(false),
                })
            }
            
            "read_file" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| FileManagerError::InvalidPath("path is required".to_string()))?;
                
                let full_path = self.resolve_path(path)?;
                let content = self.read_file_content(&full_path).await?;
                
                Ok(CallToolResult {
                    content: vec![Content::text(content)],
                    is_error: Some(false),
                })
            }
            
            "get_file_info" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| FileManagerError::InvalidPath("path is required".to_string()))?;
                
                let full_path = self.resolve_path(path)?;
                let info = self.get_file_metadata(&full_path).await?;
                
                Ok(CallToolResult {
                    content: vec![Content::text(serde_json::to_string_pretty(&info)?)],
                    is_error: Some(false),
                })
            }
            
            "write_file" => {
                if self.config.read_only {
                    return Err(FileManagerError::PermissionDenied(
                        "Server is in read-only mode".to_string()
                    ));
                }
                
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| FileManagerError::InvalidPath("path is required".to_string()))?;
                
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| FileManagerError::InvalidPath("content is required".to_string()))?;
                
                let full_path = self.resolve_path(path)?;
                self.write_file_content(&full_path, content).await?;
                
                Ok(CallToolResult {
                    content: vec![Content::text(format!("Successfully wrote to {}", path))],
                    is_error: Some(false),
                })
            }
            
            _ => Err(FileManagerError::InvalidPath(format!("Unknown tool: {}", request.name))),
        }
    }
    
    /// List available resources
    async fn list_resources(&self, _request: PaginatedRequestParam) -> std::result::Result<ListResourcesResult, Self::Error> {
        let resources = vec![
            Resource {
                uri: "file:///directory-tree".to_string(),
                name: "Directory Tree".to_string(),
                description: Some("Complete directory tree structure".to_string()),
                mime_type: Some("application/json".to_string()),
            }
        ];
        
        Ok(ListResourcesResult {
            resources,
            next_cursor: String::new(),
        })
    }
    
    /// Read a specific resource
    async fn read_resource(&self, request: ReadResourceRequestParam) -> std::result::Result<ReadResourceResult, Self::Error> {
        match request.uri.as_str() {
            "file:///directory-tree" => {
                let tree = self.build_directory_tree().await?;
                Ok(ReadResourceResult {
                    contents: vec![ResourceContents {
                        uri: request.uri,
                        mime_type: Some("application/json".to_string()),
                        text: Some(serde_json::to_string_pretty(&tree)?),
                        blob: None,
                    }],
                })
            }
            _ => Err(FileManagerError::FileNotFound(format!("Resource not found: {}", request.uri))),
        }
    }
    
    /// List available prompts
    async fn list_prompts(&self, _request: PaginatedRequestParam) -> std::result::Result<ListPromptsResult, Self::Error> {
        let prompts = vec![
            Prompt {
                name: "analyze_directory".to_string(),
                description: "Analyze directory structure and provide insights".to_string(),
                arguments: Some(vec![
                    PromptArgument {
                        name: "directory".to_string(),
                        description: "Directory to analyze".to_string(),
                        required: Some(true),
                    }
                ]),
            }
        ];
        
        Ok(ListPromptsResult {
            prompts,
            next_cursor: String::new(),
        })
    }
    
    /// Execute a prompt
    async fn get_prompt(&self, request: GetPromptRequestParam) -> std::result::Result<GetPromptResult, Self::Error> {
        match request.name.as_str() {
            "analyze_directory" => {
                let directory = request.arguments
                    .as_ref()
                    .and_then(|args| args.get("directory"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                
                let analysis = self.analyze_directory(directory).await?;
                
                Ok(GetPromptResult {
                    description: Some("Directory analysis complete".to_string()),
                    messages: vec![PromptMessage {
                        role: Role::User,
                        content: Content::text(analysis),
                    }],
                })
            }
            _ => Err(FileManagerError::InvalidPath(format!("Unknown prompt: {}", request.name))),
        }
    }
}

impl FileManagerBackend {
    /// Resolve and validate file path
    fn resolve_path(&self, path: &str) -> Result<PathBuf, FileManagerError> {
        let path = PathBuf::from(path);
        
        // Prevent path traversal attacks
        if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(FileManagerError::InvalidPath(
                "Path traversal not allowed".to_string()
            ));
        }
        
        let full_path = self.config.base_directory.join(path);
        
        // Ensure resolved path is still within base directory
        if !full_path.starts_with(&self.config.base_directory) {
            return Err(FileManagerError::InvalidPath(
                "Path outside base directory".to_string()
            ));
        }
        
        Ok(full_path)
    }
    
    /// List directory contents
    async fn list_directory(&self, path: &PathBuf) -> Result<serde_json::Value, FileManagerError> {
        use tokio::fs;
        
        let mut entries = Vec::new();
        let mut dir_reader = fs::read_dir(path).await?;
        
        while let Some(entry) = dir_reader.next_entry().await? {
            let metadata = entry.metadata().await?;
            let file_type = if metadata.is_dir() { "directory" } else { "file" };
            
            entries.push(json!({
                "name": entry.file_name().to_string_lossy(),
                "type": file_type,
                "size": metadata.len(),
                "modified": metadata.modified().ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
            }));
        }
        
        Ok(json!({
            "path": path.to_string_lossy(),
            "entries": entries,
            "total": entries.len()
        }))
    }
    
    /// Read file content
    async fn read_file_content(&self, path: &PathBuf) -> Result<String, FileManagerError> {
        use tokio::fs;
        
        if !path.is_file() {
            return Err(FileManagerError::FileNotFound(
                format!("Not a file: {:?}", path)
            ));
        }
        
        let content = fs::read_to_string(path).await?;
        Ok(content)
    }
    
    /// Write file content
    async fn write_file_content(&self, path: &PathBuf, content: &str) -> Result<(), FileManagerError> {
        use tokio::fs;
        
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        fs::write(path, content).await?;
        Ok(())
    }
    
    /// Get file metadata
    async fn get_file_metadata(&self, path: &PathBuf) -> Result<serde_json::Value, FileManagerError> {
        use tokio::fs;
        
        let metadata = fs::metadata(path).await?;
        
        Ok(json!({
            "path": path.to_string_lossy(),
            "is_file": metadata.is_file(),
            "is_directory": metadata.is_dir(),
            "size": metadata.len(),
            "readonly": metadata.permissions().readonly(),
            "modified": metadata.modified().ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
            "created": metadata.created().ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
        }))
    }
    
    /// Build complete directory tree
    async fn build_directory_tree(&self) -> Result<serde_json::Value, FileManagerError> {
        // Implementation would recursively build tree structure
        Ok(json!({
            "root": self.config.base_directory.to_string_lossy(),
            "tree": "Directory tree would be built here..."
        }))
    }
    
    /// Analyze directory for insights
    async fn analyze_directory(&self, directory: &str) -> Result<String, FileManagerError> {
        let path = self.resolve_path(directory)?;
        let listing = self.list_directory(&path).await?;
        
        let analysis = format!(
            "Directory Analysis for {:?}:\n\n\
            Based on the directory listing:\n{}\n\n\
            This directory contains various files and subdirectories. \
            Consider organizing files by type and removing any unnecessary items.",
            path,
            serde_json::to_string_pretty(&listing)?
        );
        
        Ok(analysis)
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("file_manager_mcp=debug,mcp_server=debug"))
        )
        .init();
    
    info!("🚀 Starting File Manager MCP Server");
    
    // Create backend configuration
    let backend_config = FileManagerConfig {
        base_directory: std::env::var("FILE_MANAGER_BASE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap()),
        read_only: std::env::var("FILE_MANAGER_READ_ONLY")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false),
    };
    
    info!("Base directory: {:?}", backend_config.base_directory);
    info!("Read-only mode: {}", backend_config.read_only);
    
    // Initialize backend
    let backend = FileManagerBackend::initialize(backend_config).await?;
    
    // Create server configuration
    let mut auth_config = mcp_auth::default_config();
    auth_config.enabled = std::env::var("MCP_AUTH_ENABLED")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);
    
    let transport_mode = std::env::var("MCP_TRANSPORT").unwrap_or_else(|_| "stdio".to_string());
    let transport_config = match transport_mode.as_str() {
        "http" => {
            let port = std::env::var("MCP_PORT")
                .unwrap_or_else(|_| "3001".to_string())
                .parse::<u16>()
                .unwrap_or(3001);
            TransportConfig::Http { port }
        }
        "websocket" => {
            let port = std::env::var("MCP_PORT")
                .unwrap_or_else(|_| "3001".to_string())
                .parse::<u16>()
                .unwrap_or(3001);
            TransportConfig::WebSocket { port }
        }
        _ => TransportConfig::Stdio,
    };
    
    let server_config = ServerConfig {
        server_info: backend.get_server_info(),
        auth_config,
        transport_config,
        ..Default::default()
    };
    
    // Create and start server
    let mut server = McpServer::new(backend, server_config).await?;
    
    info!("✅ File Manager MCP Server started successfully");
    info!("🔧 Available tools: list_files, read_file, get_file_info{}", 
          if backend_config.read_only { "" } else { ", write_file" });
    info!("📁 Resources: directory-tree");
    info!("💡 Prompts: analyze_directory");
    info!("🔗 Transport: {:?}", transport_config);
    
    // Run server until shutdown
    server.run().await?;
    
    info!("👋 File Manager MCP Server stopped");
    Ok(())
}
```

## Framework Integration Patterns

### 1. Error Handling

The framework expects your backend errors to implement `From<YourError> for mcp_protocol::Error`:

```rust
#[derive(Debug, Error)]
pub enum MyBackendError {
    #[error("Validation failed: {0}")]
    Validation(String),
    
    #[error("External service error: {0}")]
    ExternalService(String),
}

impl From<MyBackendError> for mcp_protocol::Error {
    fn from(err: MyBackendError) -> Self {
        match err {
            MyBackendError::Validation(msg) => Error::invalid_params(msg),
            MyBackendError::ExternalService(msg) => Error::internal_error(msg),
        }
    }
}
```

### 2. Tool Schema Definition

Use JSON Schema for robust parameter validation:

```rust
Tool {
    name: "complex_operation".to_string(),
    description: "Perform a complex operation with validation".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "string",
                "pattern": "^[a-zA-Z0-9_-]+$",
                "description": "Alphanumeric identifier"
            },
            "amount": {
                "type": "number",
                "minimum": 0,
                "maximum": 1000,
                "description": "Amount between 0 and 1000"
            },
            "options": {
                "type": "object",
                "properties": {
                    "async": {"type": "boolean", "default": false},
                    "timeout": {"type": "integer", "minimum": 1, "maximum": 300}
                }
            }
        },
        "required": ["id", "amount"]
    }),
}
```

### 3. Resource Management

Implement dynamic resources that can be discovered:

```rust
async fn list_resources(&self, request: PaginatedRequestParam) -> Result<ListResourcesResult, Self::Error> {
    // Dynamic resource discovery
    let mut resources = Vec::new();
    
    // Add static resources
    resources.push(Resource {
        uri: "myapp://config".to_string(),
        name: "Application Configuration".to_string(),
        description: Some("Current app configuration".to_string()),
        mime_type: Some("application/json".to_string()),
    });
    
    // Add dynamic resources based on backend state
    for item in self.get_dynamic_items().await? {
        resources.push(Resource {
            uri: format!("myapp://items/{}", item.id),
            name: item.name,
            description: Some(item.description),
            mime_type: Some("application/json".to_string()),
        });
    }
    
    // Handle pagination
    let start = request.cursor
        .as_ref()
        .and_then(|c| c.parse::<usize>().ok())
        .unwrap_or(0);
    
    let page_size = 50;
    let end = std::cmp::min(start + page_size, resources.len());
    let page_resources = resources[start..end].to_vec();
    
    let next_cursor = if end < resources.len() {
        end.to_string()
    } else {
        String::new()
    };
    
    Ok(ListResourcesResult {
        resources: page_resources,
        next_cursor,
    })
}
```

### 4. Prompt Implementation

Create interactive prompts for complex workflows:

```rust
async fn get_prompt(&self, request: GetPromptRequestParam) -> Result<GetPromptResult, Self::Error> {
    match request.name.as_str() {
        "workflow_assistant" => {
            let context = request.arguments
                .as_ref()
                .and_then(|args| args.get("context"))
                .and_then(|v| v.as_str())
                .unwrap_or("general");
            
            let system_prompt = self.build_system_prompt(context).await?;
            let user_context = self.gather_user_context().await?;
            
            Ok(GetPromptResult {
                description: Some(format!("Workflow assistant for {}", context)),
                messages: vec![
                    PromptMessage {
                        role: Role::System,
                        content: Content::text(system_prompt),
                    },
                    PromptMessage {
                        role: Role::User,
                        content: Content::text(user_context),
                    }
                ],
            })
        }
        _ => Err(MyBackendError::Validation(format!("Unknown prompt: {}", request.name))),
    }
}
```

## Advanced Framework Features

### 1. Authentication Integration

```rust
// In your server configuration
let mut auth_config = mcp_auth::default_config();
auth_config.enabled = true;
auth_config.require_api_key = true;
auth_config.default_role = "user".to_string();

// The framework handles all authentication automatically
// Your backend receives only authenticated requests
```

### 2. Rate Limiting and Security

```rust
// Framework automatically applies:
// - Input validation and sanitization
// - Rate limiting per client
// - Request size limits
// - CORS policies
// - Security headers

// Your backend code remains focused on business logic
```

### 3. Monitoring and Observability

```rust
// Framework provides automatic monitoring:
// - Request/response metrics
// - Error tracking  
// - Performance profiling
// - Health check endpoints

// Access metrics in your backend:
async fn health_check(&self) -> Result<(), Self::Error> {
    // Your custom health checks
    self.verify_external_dependencies().await?;
    
    // Framework automatically includes:
    // - Memory usage
    // - Request latency
    // - Error rates
    Ok(())
}
```

### 4. Performance Optimization

```rust
// Framework provides automatic optimizations:
// - Response caching
// - Request coalescing  
// - Connection pooling
// - Batch request handling

// Configure caching for your tools:
async fn call_tool(&self, request: CallToolRequestParam) -> Result<CallToolResult, Self::Error> {
    match request.name.as_str() {
        "expensive_operation" => {
            // Framework caches responses automatically based on parameters
            let result = self.perform_expensive_operation(&request.arguments).await?;
            Ok(CallToolResult {
                content: vec![Content::text(result)],
                is_error: Some(false),
            })
        }
        _ => // ... other tools
    }
}
```

## Deployment Configurations

### 1. Development Mode

```bash
# Run with stdio transport for MCP Inspector
cargo run

# Or with HTTP for web testing
MCP_TRANSPORT=http MCP_PORT=3001 cargo run
```

### 2. Production Deployment

```bash
# Build optimized release
cargo build --release

# Configure for production
export MCP_TRANSPORT=http
export MCP_PORT=3001
export MCP_AUTH_ENABLED=true
export RUST_LOG=info

# Run with authentication
./target/release/your-mcp-server
```

### 3. Docker Deployment

**Dockerfile**:
```dockerfile
FROM rust:1.75 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/your-mcp-server /usr/local/bin/
EXPOSE 3001
CMD ["your-mcp-server"]
```

## Testing Your Backend

### 1. Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_backend_initialization() {
        let config = MyBackendConfig::default();
        let backend = MyBackend::initialize(config).await.unwrap();
        assert!(backend.health_check().await.is_ok());
    }
    
    #[tokio::test]
    async fn test_tool_execution() {
        let backend = create_test_backend().await;
        let request = CallToolRequestParam {
            name: "my_tool".to_string(),
            arguments: Some(json!({"param": "value"})),
        };
        
        let result = backend.call_tool(request).await.unwrap();
        assert!(!result.is_error.unwrap_or(true));
    }
}
```

### 2. Integration Tests

```rust
#[tokio::test]
async fn test_full_mcp_workflow() {
    use mcp_server::McpServer;
    
    let backend = MyBackend::initialize(MyBackendConfig::default()).await.unwrap();
    let server_config = ServerConfig::default();
    let server = McpServer::new(backend, server_config).await.unwrap();
    
    // Test initialize -> list_tools -> call_tool workflow
    // Framework provides test utilities for this
}
```

### 3. MCP Inspector Testing

```bash
# Test your server with MCP Inspector
cargo run &
SERVER_PID=$!

# Wait for startup
sleep 2

# Test with MCP Inspector
npx @modelcontextprotocol/inspector@latest stdio -- cargo run --quiet

# Cleanup
kill $SERVER_PID
```

## Best Practices

### 1. Error Handling
- Use specific error types for different failure modes
- Provide helpful error messages for users
- Map errors appropriately to MCP protocol errors

### 2. Schema Design
- Use JSON Schema for comprehensive validation
- Provide clear descriptions and examples
- Include default values where appropriate

### 3. Resource Organization
- Use URI schemes that make sense for your domain
- Implement pagination for large resource lists
- Provide meaningful resource descriptions

### 4. Performance
- Cache expensive operations appropriately
- Use async/await throughout for non-blocking I/O
- Implement proper connection pooling for external services

### 5. Security
- Validate and sanitize all inputs
- Implement proper access controls
- Use secure defaults in configuration

This framework integration guide provides everything needed to build production-ready MCP servers using the mcp-framework architecture.