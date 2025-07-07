# CLAUDE.md

<!--
SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
-->

This file provides guidance to Claude Code (claude.ai/code) when working with this **Rust-based** Loxone MCP server project.

## 🦀 Rust Project Overview

This is a **Rust implementation** of a Model Context Protocol (MCP) server for Loxone home automation systems. The project consists of:

- **Rust codebase** across multiple modules
- **17 working MCP tools** for device control and state modification
- **25+ MCP resources** for read-only data access with caching
- **Enterprise security** with API key authentication and role-based access
- **Production-ready** with comprehensive monitoring and dashboards
- **WASM compilation support** (temporarily disabled)

## 🛠️ Common Development Commands

### Setup & Installation
```bash
# Install Rust toolchain (if needed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and setup development environment
git clone <repo> && cd mcp-loxone-gen1
./dev-env.sh

# Install dependencies and build
cargo build
```

### Credential Configuration

#### 🆕 Credential ID System (Recommended)
```bash
# Setup with credential ID generation (recommended)
cargo run --bin loxone-mcp-setup --generate-id --name "Main House"

# Use stored credentials with ID
cargo run --bin loxone-mcp-server stdio --credential-id abc123def-456-789

# Manage multiple credentials
cargo run --bin loxone-mcp-auth store --name "Office" --host 192.168.2.100 --username admin --password secure123
cargo run --bin loxone-mcp-auth list
cargo run --bin loxone-mcp-auth show abc123def-456-789
cargo run --bin loxone-mcp-auth test abc123def-456-789
```

#### Legacy Environment Variables
```bash
# Setup Loxone credentials using environment variables
export LOXONE_USER="your-username"
export LOXONE_PASS="your-password"
export LOXONE_HOST="your-miniserver-ip"

# Traditional interactive setup
cargo run --bin loxone-mcp-setup

# Verify credentials work
cargo run --bin loxone-mcp-verify
```

### 🔄 Migration Guide

If you're migrating from environment variables to the new credential ID system, see [CREDENTIAL_MIGRATION_GUIDE.md](CREDENTIAL_MIGRATION_GUIDE.md) for detailed instructions.

### Credential Management (New)
```bash
# Store and manage Loxone server credentials
cargo run --bin loxone-mcp-auth store --name "Home" --host 192.168.1.100 --username admin --password secret123
cargo run --bin loxone-mcp-auth list
cargo run --bin loxone-mcp-auth show abc123def-456-789
cargo run --bin loxone-mcp-auth test abc123def-456-789 --verbose
cargo run --bin loxone-mcp-auth update abc123def-456-789 --name "Main House"
cargo run --bin loxone-mcp-auth delete abc123def-456-789
```

### Authentication Management (Legacy)
```bash
# Create API keys for secure access  
cargo run --bin loxone-mcp-auth create --name "Admin Key" --role admin
cargo run --bin loxone-mcp-auth create --name "Dev Key" --role operator --expires 30

# List and manage API keys
cargo run --bin loxone-mcp-auth list
cargo run --bin loxone-mcp-auth show key_id
cargo run --bin loxone-mcp-auth update key_id --ip-whitelist "192.168.1.0/24"

# Security validation and audit
cargo run --bin loxone-mcp-auth security --check-only
cargo run --bin loxone-mcp-auth audit --limit 50
```

### Running the Server

#### With Credential ID (Recommended)
```bash
# Claude Desktop integration
cargo run --bin loxone-mcp-server stdio --credential-id abc123def-456-789

# Web/HTTP clients (n8n, MCP Inspector)
cargo run --bin loxone-mcp-server http --port 3001 --credential-id abc123def-456-789

# Streamable HTTP for new MCP Inspector
cargo run --bin loxone-mcp-server streamable-http --port 3001 --credential-id abc123def-456-789
```

#### Legacy Environment Variable Mode
```bash
# Development mode with MCP Inspector (requires env vars)
cargo run --bin loxone-mcp-server stdio
# Or for n8n/web clients:
cargo run --bin loxone-mcp-server http --port 3001

# Quick development server with hot reload
make dev-run

# Production build
cargo build --release
```

### Testing & Code Quality
```bash
# Run the comprehensive test suite
cargo test --lib --verbose

# Code formatting and linting
cargo fmt
cargo clippy -- -W clippy::all

# Security audit
cargo audit

# Run all quality checks
make check
```

### WASM Compilation
```bash
# Build for WebAssembly (WASIP2 target)
make wasm

# Or manually:
cargo build --target wasm32-wasip2 --release

# Test WASM binary
wasmtime target/wasm32-wasip2/release/loxone-mcp-server.wasm
```

### Docker Development
```bash
# Build and run with Docker Compose
docker-compose up --build

# Development with live reload
docker-compose -f docker-compose.dev.yml up

# Production deployment
docker build -t loxone-mcp:latest .
docker run -p 3001:3001 loxone-mcp:latest
```

## 🏗️ Project Architecture

### Directory Structure
```
src/
├── server/          # MCP protocol implementation (10+ files)
├── tools/           # 30+ Loxone device tools (audio, climate, etc.)
├── client/          # HTTP/WebSocket clients (7 files)
├── config/          # Credential management (7 files)
├── security/        # Input validation, CORS, rate limiting
├── performance/     # Monitoring, profiling, metrics
├── monitoring/      # Dashboard, InfluxDB integration
├── history/         # Time-series data storage
├── wasm/           # WebAssembly optimizations
├── validation/      # Request/response validation
├── discovery/       # Network device discovery
└── main.rs         # Binary entry points
```

### Key Modules
1. **Server** (`src/server/`): Core MCP protocol implementation with resource management
2. **Tools** (`src/tools/`): 30+ MCP tools for device control and monitoring
3. **Client** (`src/client/`): HTTP and WebSocket clients for Loxone communication
4. **Security** (`src/security/`): Production-grade security with rate limiting and validation
5. **Performance** (`src/performance/`): Real-time monitoring and profiling
6. **WASM** (`src/wasm/`): WebAssembly compilation optimizations

### MCP Tools Available
- **Audio**: Volume control, zone management
- **Climate**: Temperature, HVAC control
- **Devices**: Lights, switches, dimmers
- **Energy**: Power monitoring, consumption tracking
- **Rooms**: Room-based device organization
- **Security**: Alarm systems, access control
- **Sensors**: Temperature, motion, door/window sensors
- **Weather**: Weather station integration
- **Workflows**: Automation and scene control

## 🔧 Development Guidelines

### Adding New Features
1. **New MCP Tools**: Add to `src/tools/` following existing patterns
2. **Client Extensions**: Extend `src/client/` for new communication methods
3. **Security Features**: Add to `src/security/` with comprehensive validation
4. **Performance Monitoring**: Extend `src/performance/` for new metrics

### Code Style
- Follow Rust 2021 edition conventions
- Use `cargo fmt` for consistent formatting
- Address all `cargo clippy` warnings
- Write comprehensive tests for new functionality
- Document public APIs with rustdoc comments

### Testing Strategy
- **Unit tests**: Test individual functions and modules
- **Integration tests**: Test MCP protocol compliance
- **Security tests**: Test input validation and sanitization
- **Performance tests**: Benchmark critical paths
- **WASM tests**: Verify WebAssembly compatibility

## 🚀 Deployment Options

### Native Binary
```bash
# Build optimized release
cargo build --release

# Run production server
./target/release/loxone-mcp-server http --port 3001
```

### WebAssembly (Edge/Browser)
```bash
# Build WASM component
make wasm

# Deploy to edge runtime
wasmtime --serve target/wasm32-wasip2/release/loxone-mcp-server.wasm
```

### Docker Container
```bash
# Multi-stage production build
docker build -t loxone-mcp:latest .

# Run with environment configuration
docker run -e LOXONE_HOST=192.168.1.10 -p 3001:3001 loxone-mcp:latest
```

## 🛡️ Security Considerations

### Credential Management
- Use environment variables for development
- Production: Infisical or secure secret management
- Never commit credentials to version control
- Rotate credentials regularly

### Input Validation
- All user inputs are validated against injection attacks
- UUID, IP address, and parameter validation built-in
- Rate limiting prevents abuse
- CORS policies configurable for web deployment

### Network Security
- TLS/HTTPS recommended for production
- IP whitelisting available
- Request size limits prevent DoS
- Audit logging for security events

## 📊 Performance Optimization

### Development Profiling
```bash
# Enable performance monitoring
export LOXONE_PERFORMANCE_MODE=development

# Run with profiling
cargo run --release --features profiling

# Generate performance reports
cargo bench
```

### WASM Optimization
```bash
# Optimize for size
cargo build --target wasm32-wasip2 --release

# Check binary size
ls -lh target/wasm32-wasip2/release/loxone-mcp-server.wasm

# Strip debug symbols
wasm-strip target/wasm32-wasip2/release/loxone-mcp-server.wasm
```

## 🤝 Contributing

### Before Committing Changes
**IMPORTANT**: Always run these checks before adding files to git:
```bash
# Format all code
cargo fmt --all

# Run clippy with CI settings
cargo clippy --all --all-features -- -D warnings

# Run tests
cargo test --lib --verbose

# Security audit
cargo audit
```

Alternatively, run all checks at once:
```bash
make check
```

### Before Submitting Changes
1. Run full test suite: `cargo test`
2. Check formatting: `cargo fmt --check`
3. Run linting: `cargo clippy`
4. Update documentation if needed
5. Test WASM compilation: `make wasm`
6. Verify Docker build: `docker build .`

### Documentation Updates
- Update relevant markdown files in `docs/`
- Update CHANGELOG.md for user-facing changes
- Update inline rustdoc for API changes
- Test examples in documentation

## 📚 Official Loxone Documentation References

### Core Protocol Documentation
- **Communication Protocol**: [CommunicatingWithMiniserver.pdf](https://www.loxone.com/wp-content/uploads/datasheets/CommunicatingWithMiniserver.pdf)
  - Authentication methods (HTTP Basic Auth, Token Auth, WebSocket encryption)
  - Binary WebSocket protocol specification
  - HTTP API endpoints and commands
  - Connection establishment and session management

- **Structure File Format**: [StructureFile.pdf](https://www.loxone.com/wp-content/uploads/datasheets/StructureFile.pdf)
  - JSON schema for Loxone structure files
  - Device types, categories, and control blocks
  - Room organization and UUID hierarchies
  - State management and value types

- **API Commands Reference**: [API-Commands.pdf](https://www.loxone.com/wp-content/uploads/datasheets/API-Commands.pdf)
  - Complete command reference for device control
  - Parameter syntax and response formats
  - Error codes and status messages
  - Rate limiting and usage guidelines

- **Network Configuration**: [Loxone_PortsDomains.pdf](https://www.loxone.com/wp-content/uploads/datasheets/Loxone_PortsDomains.pdf)
  - Required network ports and firewall settings
  - Cloud service endpoints and domains
  - Security protocols and certificate management
  - Remote access configuration

- **PMS Access API**: [pms-access-api.zip](https://www.loxone.com/dede/wp-content/uploads/sites/2/2025/04/pms-access-api.zip)
  - Property Management System integration
  - Extended API for commercial applications
  - Advanced device control and monitoring

### Implementation Status vs Official Documentation

#### ✅ **Fully Implemented** (95%+ Coverage)
- **HTTP Basic Authentication** - Complete with credential management
- **WebSocket Connection** - Binary protocol with token refresh
- **Structure File Parsing** - JSON parsing with device categorization
- **Device Control Commands** - Lights, blinds, climate, audio controls
- **Real-time State Updates** - WebSocket event filtering with regex
- **Room Organization** - Device grouping and room-based operations
- **Binary Message Parsing** - All message types (0x00000000-0x07000000)

#### 🔄 **Partially Implemented** (60-90% Coverage)
- **Token Authentication** - JWT implemented but needs encryption key handling
- **Advanced Device Types** - Intercom, cameras added; missing some specialty devices
- **Batch Operations** - Framework complete but needs optimization
- **Weather Integration** - External APIs added; missing Loxone weather station protocol
- **Security Features** - Input validation present; missing full CORS implementation

#### ❌ **Missing/Needs Implementation** (0-40% Coverage)
- **Encrypted WebSocket Communication** - AES encryption for sensitive data
- **Advanced Binary Protocols** - Some specialized message formats
- **PMS Integration** - Commercial property management features
- **Full Cloud API** - Remote access through Loxone Cloud
- **Device Discovery Protocol** - Automatic Miniserver detection
- **Advanced Error Recovery** - Connection resilience and failover

### Additional Resources

- **API Documentation**: `cargo doc --open`
- **Architecture Guide**: `docs/ARCHITECTURE.md`
- **Deployment Guide**: `docs/DEPLOYMENT.md`
- **Troubleshooting**: `docs/TROUBLESHOOTING.md`
- **Examples**: See `examples/` directory
- **Migration Guide**: `CREDENTIAL_MIGRATION_GUIDE.md`

## 📦 Available Binaries

The project provides several command-line tools:

### 🚀 Core Tools
- **`loxone-mcp-server`** - Main MCP server with stdio/HTTP transport support
- **`loxone-mcp-auth`** - Complete credential management (store, list, update, delete, test)
- **`loxone-mcp-setup`** - Interactive credential setup with credential ID generation

### 🔧 Development Tools
- **`loxone-mcp-test-endpoints`** - Test multiple API endpoints (for development/debugging)

## 🔍 Debugging & Troubleshooting

### Common Issues
- **Build failures**: Check Rust version with `rustc --version`
- **WASM issues**: Ensure `wasm32-wasip2` target installed
- **Credential errors**: Verify environment variables
- **Connection issues**: Check Loxone Miniserver accessibility

### Debug Logging
```bash
# Enable debug logging
export RUST_LOG=debug
export LOXONE_LOG_LEVEL=debug

# Run with verbose output
cargo run -- --verbose
```

---

**Important**: This is a Rust project with WASM support, not a Python project. All commands and development workflows are Rust-based using Cargo, not Python tools like `uv` or `pip`.