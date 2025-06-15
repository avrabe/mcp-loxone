# Loxone MCP Troubleshooting Guide

## Common Issues & Solutions

### 1. Connection Issues

#### Cannot connect to Loxone Miniserver
```bash
# Error: "Failed to connect to Loxone Miniserver"

# Solution 1: Check network connectivity
ping 192.168.1.100  # Replace with your Miniserver IP

# Solution 2: Verify credentials
export LOXONE_HOST=192.168.1.100
export LOXONE_USER=admin
export LOXONE_PASS=your-password

# Solution 3: Test connection manually
curl -u admin:password http://192.168.1.100/jdev/sps/LoxAPP3.json

# Solution 4: Check firewall rules
sudo ufw status
```

#### Connection timeout errors
```bash
# Increase timeout settings
export LOXONE_REQUEST_TIMEOUT=60
export LOXONE_CONNECTION_TIMEOUT=30

# Check if Miniserver is overloaded
# Reduce connection pool size
export LOXONE_CONNECTION_POOL_SIZE=10
```

### 2. Authentication Problems

#### Invalid API key errors
```bash
# List all API keys
cargo run --bin loxone-mcp-keys -- list

# Check if key is active
cargo run --bin loxone-mcp-keys -- show lmcp_operator_001_abc123

# Generate new key if needed
cargo run --bin loxone-mcp-keys -- generate --role operator --name "New Key"

# Access web UI to manage keys
http://localhost:3001/admin/keys
```

#### API key not working
```bash
# 1. Check key format (should be lmcp_{role}_{seq}_{random})
echo $API_KEY

# 2. Verify key is not expired
cargo run --bin loxone-mcp-keys -- show $API_KEY | grep expires

# 3. Check IP restrictions
# Your IP must match whitelist if configured

# 4. Verify role permissions
# Admin > Operator > Monitor > Device
```

#### Legacy HTTP_API_KEY not working
```bash
# The old HTTP_API_KEY is deprecated
# Migrate to new multi-user system:

# 1. Generate new API key
cargo run --bin loxone-mcp-keys -- generate --role admin --name "Migrated"

# 2. Use new key format
curl -H "X-API-Key: lmcp_admin_001_newkey" http://localhost:3001/api/devices
```

### 3. Permission Errors

#### "Insufficient permissions" error
```bash
# Check your API key role
cargo run --bin loxone-mcp-keys -- show your-key-id

# Role permissions:
# - Admin: Full access
# - Operator: Device control + monitoring
# - Monitor: Read-only access
# - Device: Specific device control only

# Generate appropriate key
cargo run --bin loxone-mcp-keys -- generate --role operator --name "Control Key"
```

#### Cannot access admin endpoints
```bash
# Admin endpoints require admin role
# Generate admin key:
cargo run --bin loxone-mcp-keys -- generate --role admin --name "Admin Access"

# If using reverse proxy, check IP restrictions
# Admin endpoints may be IP-restricted
```

### 4. Performance Issues

#### Slow response times
```bash
# 1. Enable performance monitoring
export LOXONE_PERFORMANCE_MODE=development

# 2. Check server metrics
curl http://localhost:3001/metrics

# 3. Reduce concurrent connections
export LOXONE_CONNECTION_POOL_SIZE=25

# 4. Enable debug logging
export RUST_LOG=debug
cargo run --bin loxone-mcp-server -- http 2>&1 | grep "response_time"
```

#### High memory usage
```bash
# 1. Check current usage
ps aux | grep loxone-mcp

# 2. Limit connection pool
export LOXONE_CONNECTION_POOL_SIZE=10

# 3. Disable unnecessary features
export ENABLE_LOXONE_STATS=0
export DISABLE_PERFORMANCE=1

# 4. Use WASM build for lower memory
make wasm
```

### 5. WebSocket/SSE Issues

#### SSE connection drops frequently
```bash
# 1. Check keep-alive settings
export LOXONE_SSE_KEEPALIVE=30

# 2. Enable debug logging for SSE
export RUST_LOG=loxone_mcp::http_transport::sse=debug

# 3. Test SSE directly
curl -N -H "X-API-Key: your-key" http://localhost:3001/sse
```

#### WebSocket dashboard not updating
```bash
# 1. Check browser console for errors
# F12 > Console in browser

# 2. Verify WebSocket upgrade
curl -i -N \
  -H "Connection: Upgrade" \
  -H "Upgrade: websocket" \
  http://localhost:3001/dashboard/ws

# 3. Check CORS settings if behind proxy
# Ensure WebSocket headers are forwarded
```

### 6. Docker/Container Issues

#### Container won't start
```bash
# 1. Check logs
docker logs loxone-mcp

# 2. Verify environment variables
docker exec loxone-mcp env | grep LOXONE

# 3. Test with minimal config
docker run -it --rm \
  -e LOXONE_HOST=192.168.1.100 \
  -e LOXONE_USER=admin \
  -e LOXONE_PASS=password \
  loxone-mcp:latest

# 4. Check port binding
docker ps | grep 3001
netstat -tlnp | grep 3001
```

#### Cannot access from host
```bash
# 1. Check port mapping
docker ps # Should show 0.0.0.0:3001->3001/tcp

# 2. Test from container
docker exec loxone-mcp curl http://localhost:3001/health

# 3. Check firewall
sudo ufw allow 3001/tcp
```

### 7. Build/Compilation Errors

#### Cargo build fails
```bash
# 1. Update Rust
rustup update

# 2. Clean build
cargo clean
cargo build

# 3. Check dependencies
cargo tree | grep -E "error|conflict"

# 4. Specific feature issues
cargo build --no-default-features
cargo build --features influxdb
```

#### WASM build fails
```bash
# 1. Install WASM target
rustup target add wasm32-wasip2

# 2. Check WASM tools
cargo install wasm-bindgen-cli
cargo install wasm-opt

# 3. Build with verbose output
make wasm VERBOSE=1

# 4. Try minimal WASM build
cargo build --target wasm32-wasip2 --no-default-features
```

### 8. Dashboard Issues

#### Dashboard shows no data
```bash
# 1. Enable statistics collection
export ENABLE_LOXONE_STATS=1

# 2. Check history store
curl http://localhost:3001/dashboard/api/status

# 3. Verify data collection
export RUST_LOG=loxone_mcp::monitoring=debug
# Look for "Collecting statistics" messages

# 4. Clear cache and reload
# Ctrl+Shift+R in browser
```

#### InfluxDB connection failed
```bash
# 1. Check InfluxDB is running
curl http://localhost:8086/health

# 2. Verify credentials
export INFLUXDB_URL=http://localhost:8086
export INFLUXDB_TOKEN=your-token
export INFLUXDB_ORG=your-org
export INFLUXDB_BUCKET=loxone

# 3. Test connection
influx ping

# 4. Check bucket exists
influx bucket list
```

### 9. API Key Management Issues

#### Cannot access key management UI
```bash
# 1. Verify server is running with HTTP transport
cargo run --bin loxone-mcp-server -- http

# 2. Check URL (note: /admin/keys not /keys)
http://localhost:3001/admin/keys

# 3. Try with curl
curl http://localhost:3001/admin/keys

# 4. Check browser console for errors
# F12 > Console
```

#### Key generation fails
```bash
# 1. Check key store permissions
ls -la ~/.config/loxone-mcp/keys.toml

# 2. Try CLI generation
cargo run --bin loxone-mcp-keys -- generate --role operator --name "Test"

# 3. Check for duplicate IDs (rare)
cargo run --bin loxone-mcp-keys -- list | grep lmcp_

# 4. Use memory-only store for testing
export LOXONE_KEY_BACKEND=memory
```

### 10. Security & SSL Issues

#### HTTPS/TLS errors
```bash
# 1. For development, use HTTP
cargo run --bin loxone-mcp-server -- http

# 2. For production with reverse proxy
# Ensure proxy handles SSL termination

# 3. Check certificate validity
openssl s_client -connect localhost:443 -servername mcp.yourdomain.com

# 4. Disable certificate validation (dev only!)
export NODE_TLS_REJECT_UNAUTHORIZED=0
```

#### CORS errors
```bash
# 1. Check CORS headers
curl -I -X OPTIONS http://localhost:3001/api/devices

# 2. For development, CORS is permissive by default

# 3. Behind proxy, ensure headers are forwarded
# nginx: proxy_set_header Origin $http_origin;
```

## Diagnostic Commands

### Health Check Suite
```bash
#!/bin/bash
# health-check.sh

echo "=== Loxone MCP Health Check ==="

# 1. Server health
echo -n "Server Health: "
curl -s http://localhost:3001/health | jq -r .status

# 2. Loxone connection
echo -n "Loxone Connection: "
curl -s -H "X-API-Key: $API_KEY" http://localhost:3001/admin/status | jq -r .services.loxone

# 3. Active connections
echo -n "Active Connections: "
netstat -an | grep :3001 | grep ESTABLISHED | wc -l

# 4. Memory usage
echo -n "Memory Usage: "
ps aux | grep loxone-mcp | awk '{print $6/1024 " MB"}'

# 5. API keys
echo -n "Active API Keys: "
cargo run --bin loxone-mcp-keys -- list --active | wc -l
```

### Debug Information Collection
```bash
# Collect debug info for support
mkdir -p debug-info
cd debug-info

# System info
uname -a > system.txt
cargo --version >> system.txt
rustc --version >> system.txt

# Configuration (sanitized)
env | grep LOXONE | sed 's/PASS=.*/PASS=***/' > config.txt

# Logs (last 1000 lines)
journalctl -u loxone-mcp -n 1000 > service.log

# Server status
curl -s http://localhost:3001/health > health.json
curl -s -H "X-API-Key: $API_KEY" http://localhost:3001/admin/status > status.json

# Create archive
tar -czf debug-info-$(date +%Y%m%d-%H%M%S).tar.gz .
```

## Getting Help

If you're still experiencing issues:

1. **Check logs with debug enabled**:
   ```bash
   export RUST_LOG=debug
   cargo run --bin loxone-mcp-server -- http 2>&1 | tee debug.log
   ```

2. **Search existing issues**:
   - GitHub Issues: [https://github.com/your-repo/issues](https://github.com/your-repo/issues)

3. **Create detailed bug report** including:
   - Exact error message
   - Steps to reproduce
   - Environment details (OS, Rust version)
   - Debug logs
   - Configuration (sanitized)

4. **Community support**:
   - GitHub Discussions
   - Discord/Slack community
   - Stack Overflow with `loxone-mcp` tag