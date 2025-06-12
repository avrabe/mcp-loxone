# WASM Deployment Guide

This guide covers how to build, test, and deploy the Loxone MCP server as WebAssembly.

## Quick Start

```bash
# Setup WASM environment
make setup-wasm-env

# Build and test WASM
make wasm-workflow

# Run WASM server
wasmtime target/wasm32-wasip2/release/loxone_mcp_rust.wasm
```

## WASM Targets

The project supports multiple WASM targets:

### 1. WASM32-WASIP2 (Recommended)
- **Target**: `wasm32-wasip2`
- **Use case**: Server environments with WASI runtime
- **Runtime**: wasmtime, wasmer, etc.
- **Features**: Full system access, file I/O

```bash
# Build for WASI
cargo build --target wasm32-wasip2 --release

# Run with wasmtime
wasmtime target/wasm32-wasip2/release/loxone_mcp_rust.wasm
```

### 2. Browser WASM
- **Target**: `wasm32-unknown-unknown`
- **Use case**: Web applications, browser extensions
- **Runtime**: Browser WebAssembly engine
- **Features**: Browser APIs, localStorage

```bash
# Build for browser
wasm-pack build --target web

# Use in HTML
<script type="module">
  import init, { WasmLoxoneServer } from './pkg/loxone_mcp_rust.js';
  
  async function run() {
    await init();
    const server = new WasmLoxoneServer();
    await server.init();
    await server.start();
  }
  
  run();
</script>
```

### 3. Node.js WASM
- **Target**: `wasm32-unknown-unknown` with Node.js bindings
- **Use case**: Node.js applications, serverless functions
- **Runtime**: Node.js with WASM support

```bash
# Build for Node.js
wasm-pack build --target nodejs

# Use in Node.js
const { WasmLoxoneServer } = require('./pkg-node/loxone_mcp_rust');

async function main() {
  const server = new WasmLoxoneServer();
  await server.init();
  await server.start();
}

main();
```

## Build Commands

### Development Build
```bash
# Quick development build
cargo build --target wasm32-wasip2

# With specific features
cargo build --target wasm32-wasip2 --features wasm-storage
```

### Release Build
```bash
# Optimized release build
cargo build --target wasm32-wasip2 --release

# With size optimization
make optimize-wasm
```

### All WASM Targets
```bash
# Build for all WASM targets
make build-wasm-all
```

## Testing

### Unit Tests
```bash
# Run WASM-compatible tests
cargo test --target wasm32-wasip2 --features wasm-storage
```

### Browser Tests
```bash
# Test in Chrome and Firefox
wasm-pack test --headless --chrome --firefox

# Test specific browser
wasm-pack test --headless --chrome
```

### Node.js Tests
```bash
# Test in Node.js environment
wasm-pack test --node
```

### Performance Tests
```bash
# Run performance benchmarks
make bench --target wasm32-wasip2
```

## Size Optimization

### Build Profiles
The project includes several optimization profiles:

```toml
[profile.release]
opt-level = "s"      # Optimize for size
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit
panic = "abort"     # Abort on panic
strip = true        # Strip debug symbols

[profile.wasm-release]
inherits = "release"
opt-level = "z"     # Aggressive size optimization
```

### wasm-opt Optimization
```bash
# Install binaryen tools
# Download from: https://github.com/WebAssembly/binaryen/releases

# Optimize WASM binary
wasm-opt -Oz input.wasm -o optimized.wasm

# Or use make target
make optimize-wasm
```

### Size Analysis
```bash
# Analyze binary sizes
make size-analysis

# Expected sizes:
# - Unoptimized: ~2-5MB
# - Release: ~1-2MB  
# - wasm-opt -Oz: ~500KB-1MB
```

## Runtime Requirements

### WASI Runtime (Recommended)
```bash
# Install wasmtime
curl https://wasmtime.dev/install.sh -sSf | bash

# Run WASM binary
wasmtime --allow-ip=127.0.0.1 target/wasm32-wasip2/release/loxone_mcp_rust.wasm
```

### Browser Environment
Requirements:
- Modern browser with WebAssembly support
- ES6 modules support
- Local storage access (for credentials)

### Node.js Environment
Requirements:
- Node.js 16+ with WASM support
- ES modules or CommonJS support

## Configuration

### WASI Configuration
```bash
# Environment variables
export LOXONE_URL=http://192.168.1.100
export LOXONE_USERNAME=admin
export LOXONE_PASSWORD=secret

# Or use command line arguments
wasmtime target/wasm32-wasip2/release/loxone_mcp_rust.wasm
```

### Browser Configuration
```javascript
// Configure before initialization
const config = {
  loxone: {
    url: "http://192.168.1.100",
    username: "admin"
  },
  mcp: {
    transport: "http",
    port: 8080
  }
};

const server = new WasmLoxoneServer();
await server.init(JSON.stringify(config));
```

### Node.js Configuration
```javascript
// Load from environment or config file
const config = {
  loxone: {
    url: process.env.LOXONE_URL,
    username: process.env.LOXONE_USERNAME
  }
};

const server = new WasmLoxoneServer();
await server.init(JSON.stringify(config));
```

## Performance Considerations

### Memory Usage
- WASM has linear memory model
- Default memory limit: 64KB initial, grows as needed
- Monitor memory usage in production

### Startup Time
- WASM compilation happens at runtime
- Consider using WebAssembly streaming compilation
- Cache compiled WASM modules when possible

### Network Access
- WASI: Full network access with runtime flags
- Browser: Subject to CORS and security policies
- Node.js: Full network access

## Deployment Examples

### 1. Serverless Function (Cloudflare Workers)
```javascript
import { WasmLoxoneServer } from './loxone_mcp_rust.js';

export default {
  async fetch(request, env) {
    const server = new WasmLoxoneServer();
    await server.init(env.CONFIG);
    
    // Handle MCP requests
    return new Response('OK');
  }
}
```

### 2. Web Application
```html
<!DOCTYPE html>
<html>
<head>
  <title>Loxone MCP Web Client</title>
</head>
<body>
  <script type="module">
    import init, { WasmLoxoneServer } from './pkg/loxone_mcp_rust.js';
    
    async function initServer() {
      await init();
      
      const server = new WasmLoxoneServer();
      await server.init();
      
      // Server is ready
      console.log('Loxone MCP server ready');
    }
    
    initServer();
  </script>
</body>
</html>
```

### 3. Docker Container with WASI
```dockerfile
FROM wasmtime/wasmtime:latest

COPY target/wasm32-wasip2/release/loxone_mcp_rust.wasm /app/
WORKDIR /app

EXPOSE 8080

CMD ["wasmtime", "--allow-ip=0.0.0.0:8080", "loxone_mcp_rust.wasm"]
```

### 4. GitHub Actions with WASM
```yaml
- name: Deploy WASM to Pages
  run: |
    wasm-pack build --target web
    cp -r pkg/* dist/
    # Deploy to GitHub Pages
```

## Troubleshooting

### Common Issues

1. **Large Binary Size**
   ```bash
   # Solution: Enable optimizations
   cargo build --release --target wasm32-wasip2
   wasm-opt -Oz input.wasm -o output.wasm
   ```

2. **Runtime Errors**
   ```bash
   # Solution: Enable panic hooks
   console_error_panic_hook::set_once();
   ```

3. **Network Access Denied**
   ```bash
   # Solution: Configure runtime permissions
   wasmtime --allow-ip=127.0.0.1 app.wasm
   ```

4. **Memory Limit Exceeded**
   ```bash
   # Solution: Increase memory limit
   wasmtime --max-memory=128MB app.wasm
   ```

### Debug Build
```bash
# Build with debug info
cargo build --target wasm32-wasip2

# Enable debug logging
RUST_LOG=debug wasmtime app.wasm
```

### Browser Developer Tools
```javascript
// Enable WASM debugging in browser
console.log('WASM module loaded');

// Check memory usage
console.log('WASM memory:', WebAssembly.Memory);
```

## Security Considerations

### WASI Sandbox
- WASI provides sandboxed execution
- Limited file system access
- Network access requires explicit permissions

### Browser Security
- Subject to same-origin policy
- Credentials stored in localStorage (not secure for production)
- Use HTTPS for credential transmission

### Production Deployment
- Use environment variables for secrets
- Implement proper authentication
- Monitor resource usage
- Use secure transport (HTTPS/WSS)

## Performance Benchmarks

Expected performance compared to native:
- **Startup time**: 2-5x slower (compilation overhead)
- **Runtime performance**: 80-95% of native
- **Memory usage**: 10-20% higher
- **Binary size**: 50-80% smaller than equivalent native

## Further Reading

- [WebAssembly Specification](https://webassembly.org/specs/)
- [WASI Documentation](https://wasi.dev/)
- [wasm-pack Book](https://rustwasm.github.io/wasm-pack/)
- [Wasmtime Documentation](https://docs.wasmtime.dev/)