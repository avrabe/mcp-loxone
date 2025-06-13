# Loxone MCP Rust Server

A high-performance Model Context Protocol (MCP) server for Loxone Generation 1 home automation systems, implemented in Rust with WASM support.

## ✨ New Features & Improvements

### 🔐 Enhanced Security
- **Comprehensive input validation** for all parameters (UUID, names, actions, IPs)
- **Credential sanitization** in logs - passwords and API keys automatically masked
- **Request size limits** to prevent DoS attacks
- **Batch operation limits** (max 100 devices per batch)

### 🚀 Performance Optimizations  
- **True parallel execution** for batch commands using `futures::join_all`
- **Connection pooling** and reuse
- **Efficient caching** of structure data
- **Optimized WASM builds** with size constraints

### 📝 Professional Logging
- **File-based logging** with automatic daily rotation
- **Structured logging** with tracing framework
- **Request/response logging** with sanitization
- **Performance metrics** for slow operation detection

### 🐳 Docker Support
- **Multi-stage Dockerfile** for minimal images
- **Docker Compose** for development and production
- **Health checks** and non-root execution
- **Environment-based configuration**

### 🧪 Comprehensive Testing
- **Unit tests** for validation logic
- **Integration tests** for parallel execution
- **Security tests** for input sanitization
- **CI/CD pipeline** with GitHub Actions

## 🚀 Quick Start

### Development (Avoiding Keychain Prompts)

The easiest way to develop without keychain password prompts:

```bash
# Setup development environment
./dev-env.sh

# Run development server (HTTP/n8n mode)
make dev-run

# Or run stdio server (Claude Desktop mode)  
make dev-stdio

# Build with automatic code signing (macOS)
make build
```

### Environment Variables (Recommended for Development)

Set these environment variables to avoid keychain access:

```bash
export LOXONE_USERNAME="your_username"
export LOXONE_PASSWORD="your_password" 
export LOXONE_HOST="http://192.168.1.100"
export LOXONE_API_KEY="your-api-key"
```

## 🔐 macOS Keychain & Code Signing

### The Keychain Password Problem

On macOS, accessing the keychain triggers password prompts. The original issue was:

1. **Multiple keychain access calls** - username, password, host, API key accessed separately
2. **Unsigned binaries** require additional authentication
3. **Each credential read** triggered a separate prompt (4-8 prompts total)

### Solutions Implemented

#### Solution 1: Batched Keychain Access ✅
The server now loads all credentials in a single batch operation:
```rust
// Old: 4 separate keychain calls (4-8 prompts)
let username = get_username_from_keychain();
let password = get_password_from_keychain(); 
let host = get_host_from_keychain();
let api_key = get_api_key_from_keychain();

// New: 1 batched call (4 prompts, clustered)
let (credentials, host) = get_credentials_with_host();
```

#### Solution 2: Automatic Code Signing ✅
The build process now automatically signs binaries on macOS:

```bash
# Build with automatic signing (recommended)
make build

# Or use the signing wrapper directly
./cargo-build-sign.sh --release

# Manual signing (if needed)  
./sign-binary.sh
```

**Automatic code signing features:**
- Integrated into `make build` and `make dev-build`
- Signs binary immediately after compilation
- Uses ad-hoc signing for development (no certificate needed)
- Set `CODESIGN_IDENTITY` for production signing
- Set `SKIP_CODESIGN=1` to disable signing

**Code signing reduces keychain prompts** by establishing trust with the system.

### Solution 2: Environment Variables (Development)

Use environment variables instead of keychain during development:

```bash
# Quick development without keychain
LOXONE_USERNAME=admin LOXONE_PASSWORD=admin LOXONE_HOST=http://192.168.1.100 cargo run -- http
```

### Solution 3: Production Keyring Setup

For production, properly configure keyring access:

```bash
# Setup credentials in keychain
cargo run --bin loxone-mcp-setup

# Verify keychain access works
cargo run --bin loxone-mcp-verify
```

## 📦 Building

### Native Build

```bash
# Standard build with signing
make build

# Development build (faster)
make dev-build

# Build with size analysis
make build && make size-analysis
```

### WASM Build

```bash
# Setup WASM environment
make setup-wasm-env

# Build WASM (all targets)
make build-wasm-all

# Optimized WASM for production
make optimize-wasm
```

## 🧪 Testing

```bash
# Run all tests
make test

# WASM-specific tests
make test-wasm

# Run tests with coverage
cargo test --workspace --all-features

# Integration tests
make test-integration
```

## 🚀 Running the Server

### Stdio Mode (Claude Desktop)

```bash
# Production
cargo run --bin loxone-mcp-server -- stdio

# Development (with environment variables)
make dev-stdio
```

### HTTP Mode (n8n, web clients)

```bash
# Production  
cargo run --bin loxone-mcp-server -- http --port 3001

# Development (with auto-reload)
make dev

# Development (single run)
make dev-run
```

### Authentication Tokens

```bash
# With custom authentication tokens
cargo run --bin loxone-mcp-server -- http \
  --n8n-token "your-n8n-token" \
  --ai-token "your-ai-token" \
  --admin-token "your-admin-token"
```

## 🔧 Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `LOXONE_USERNAME` | Username for Loxone authentication | - |
| `LOXONE_PASSWORD` | Password for Loxone authentication | - |
| `LOXONE_HOST` | Miniserver URL | `http://127.0.0.1:80` |
| `LOXONE_API_KEY` | API key for authentication | - |
| `RUST_LOG` | Logging level | `info` |
| `MCP_TRANSPORT` | Transport type (stdio/http) | `stdio` |
| `MCP_PORT` | HTTP server port | `3001` |

### Credential Storage

The server supports multiple credential storage backends:

1. **Keyring** (default on native): Uses system keychain
2. **Environment**: Uses environment variables  
3. **LocalStorage** (WASM): Uses browser local storage
4. **FileSystem**: Uses file system (WASI)

### Development Configuration

```bash
# Source development environment
source .env.development

# Or set variables manually
export LOXONE_USERNAME=admin
export LOXONE_PASSWORD=admin  
export LOXONE_HOST=http://192.168.1.100
export RUST_LOG=debug
```

## 🌐 WASM Deployment

### Web Deployment

```bash
# Build for web
make build-wasm-web

# Serve with Python
cd pkg-web && python -m http.server 8080
```

### Node.js Integration

```bash
# Build for Node.js
make build-wasm-node

# Use in Node.js project
npm install ./pkg-node
```

### WASI Runtime

```bash
# Build WASM with WASI
make build-wasm

# Run with wasmtime
wasmtime target/wasm32-wasip2/release/loxone_mcp_rust.wasm
```

## 🛠️ Development Tools

### Available Make Targets

```bash
make help              # Show all available commands
make dev               # Development server with auto-reload
make dev-run           # Development server (single run)
make dev-stdio         # Development stdio server
make build             # Build with automatic signing
make test              # Run test suite
make lint              # Run linting (clippy)
make format            # Format code
make clean             # Clean build artifacts
make docs              # Generate documentation
```

### Code Quality

```bash
# Run all checks
make check

# Format code
make format

# Security audit
make audit

# Size analysis
make size-analysis
```

## 🔒 Security Best Practices

### Development

1. **Use environment variables** for credentials during development
2. **Never commit credentials** to version control
3. **Use code signing** to avoid keychain prompts
4. **Enable logging** for debugging: `RUST_LOG=debug`

### Production

1. **Use keyring storage** for credentials
2. **Enable SSL verification**: Set `verify_ssl: true`
3. **Use strong API keys** for authentication
4. **Monitor logs** for security events
5. **Keep dependencies updated**: `make update-deps`

## 🐛 Troubleshooting

### Keychain Password Prompts

**Problem**: Server asks for keychain password on every start

**Root Cause**: Often caused by keychain entries created by Python version having different permissions than Rust version expects.

**🎯 Recommended Solution**: Reset keychain entries
```bash
# This clears Python-created entries and recreates them with proper Rust permissions
make reset-keychain
```

**Alternative Solutions**:
1. ✅ **Batched keychain access**: Reduced from 8 to 4 prompts
2. ✅ **Security command fallback**: Uses `security` command-line tool (often no prompts)
3. ✅ **Code signing**: `make build` applies ad-hoc signature 
4. ✅ **Environment variable priority**: Set `LOXONE_USERNAME`, `LOXONE_PASSWORD`, `LOXONE_HOST`

**Current Status**: 
- Fresh keychain entries: 0 prompts (after reset) ✅
- Environment variables: 0 prompts ✅
- Legacy keychain entries: ~4 prompts (reduced from 8) ✅
- Development mode: 0 prompts ✅

**For Development**:
Use `make dev-run` or `make dev-stdio` (uses environment variables automatically)

### Build Issues

**Problem**: Build fails with missing dependencies

**Solution**: Install development dependencies
```bash
make install-deps
rustup target add wasm32-wasip2
```

### Connection Issues

**Problem**: Cannot connect to Miniserver

**Solutions**:
1. **Check URL**: Verify `LOXONE_HOST` is correct
2. **Check credentials**: Verify `LOXONE_USERNAME` and `LOXONE_PASSWORD`
3. **Network access**: Ensure Miniserver is reachable
4. **Firewall**: Check firewall settings on both sides

### WASM Issues

**Problem**: WASM build fails

**Solution**: Setup WASM environment
```bash
make setup-wasm-env
make check-wasm
```

## 📚 API Reference

### MCP Tools Available

- 30+ device control and monitoring tools
- Room-based device organization  
- Climate control (6 room controllers)
- Real-time sensor discovery
- Weather monitoring
- Energy consumption tracking
- Security system integration

### HTTP Endpoints

- `POST /mcp` - MCP JSON-RPC endpoint
- `GET /health` - Health check
- `POST /messages` - Traditional JSON-RPC (n8n compatible)
- `GET /sse` - Server-sent events

## 🤝 Contributing

1. **Follow code style**: `make format`
2. **Run tests**: `make test`
3. **Update documentation**: `make docs`
4. **Check security**: `make audit`

## 📄 License

MIT - See LICENSE file for details.