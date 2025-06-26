# MCP Server Integration Tests

This directory contains comprehensive integration tests for the Loxone MCP server to ensure compatibility with external tools and prevent regressions.

## Test Structure

### 1. Quick Bash Tests (`test_mcp_server.sh`)
Fast, lightweight tests using standard CLI tools:
- Health endpoint verification
- SSE connection establishment
- Streamable HTTP transport testing
- Tools listing verification

```bash
# Run quick tests
make integration-quick
# Or directly:
./integration_tests/test_mcp_server.sh
```

### 2. Comprehensive Python Tests (`test_mcp_compatibility.py`)
Detailed test suite using pytest:
- Protocol compliance testing
- Transport mode detection
- Error response format validation
- Session management testing
- CORS compatibility

```bash
# Install dependencies
make install-test-deps
# Run full tests
make integration-full
# Or directly:
python -m pytest integration_tests/test_mcp_compatibility.py -v
```

### 3. MCP Inspector Integration (`test_with_mcp_inspector.py`)
Tests compatibility with the official MCP Inspector:
- Automatic server and inspector startup
- Connection establishment verification
- Protocol handshake testing

```bash
# Test with MCP Inspector
make integration-inspector
# Or directly:
python integration_tests/test_with_mcp_inspector.py
```

## Dependencies

### Python Dependencies
- `pytest` - Test framework
- `requests` - HTTP client
- `sseclient-py` - Server-Sent Events client

### System Dependencies
- `curl` - Command-line HTTP client
- `jq` - JSON processor
- `npx` - Node.js package runner (for MCP Inspector)

## Running Tests

### All Tests
```bash
make integration-test
```

### Individual Test Suites
```bash
# Quick bash tests (fastest)
make integration-quick

# Comprehensive Python tests
make integration-full

# MCP Inspector compatibility
make integration-inspector
```

### Manual Testing

#### Test SSE Connection
```bash
curl -N -H "Accept: text/event-stream" -H "X-API-Key: 1234" \
  http://localhost:3003/sse
```

#### Test Initialize Request
```bash
curl -X POST -H "Content-Type: application/json" -H "Accept: application/json" \
  -H "X-API-Key: 1234" \
  -d '{"jsonrpc":"2.0","id":"test","method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}' \
  http://localhost:3003/messages
```

#### Test Tools List
```bash
curl -X POST -H "Content-Type: application/json" -H "Accept: application/json" \
  -H "X-API-Key: 1234" \
  -d '{"jsonrpc":"2.0","id":"tools","method":"tools/list","params":{}}' \
  http://localhost:3003/messages
```

## Test Coverage

### Protocol Compliance
- ✅ JSON-RPC 2.0 format
- ✅ MCP protocol version negotiation
- ✅ Proper error response format
- ✅ Field serialization (omitting null fields)

### Transport Modes
- ✅ Streamable HTTP transport (new)
- ✅ Legacy HTTP+SSE transport
- ✅ Transport mode detection via Accept headers
- ✅ Session management and fallback

### External Tool Compatibility
- ✅ curl (command-line HTTP client)
- ✅ MCP Inspector (official testing tool)
- ✅ Python requests library
- ✅ httpie (user-friendly HTTP client)

### Error Handling
- ✅ Connection errors
- ✅ Protocol errors
- ✅ Session management errors
- ✅ Timeout handling

## CI/CD Integration

The tests are automatically run in GitHub Actions:
- On every push to main/develop branches
- On all pull requests
- Tests both Ubuntu and compatibility scenarios

See `.github/workflows/integration-tests.yml` for the full CI configuration.

## Troubleshooting

### Server Won't Start
```bash
# Check if port is in use
lsof -i :3003

# Kill existing servers
pkill -f loxone-mcp-server

# Check server logs
tail -f /tmp/mcp_test_server.log
```

### Tests Failing
```bash
# Run with verbose output
python -m pytest integration_tests/test_mcp_compatibility.py -v -s

# Check server is responding
curl http://localhost:3003/health

# Test specific endpoint
curl -v http://localhost:3003/sse
```

### MCP Inspector Issues
```bash
# Check Node.js is installed
node --version
npx --version

# Install/update MCP Inspector
npm install -g @modelcontextprotocol/inspector

# Run inspector manually
npx @modelcontextprotocol/inspector
```

## Adding New Tests

### Add Bash Test
1. Edit `test_mcp_server.sh`
2. Add new test section with descriptive echo
3. Ensure cleanup on failure

### Add Python Test
1. Add test method to appropriate class in `test_mcp_compatibility.py`
2. Use existing fixtures (`mcp_server`, `mcp_client`)
3. Follow pytest conventions

### Add External Tool Test
1. Create new test file or add to existing
2. Document tool requirements in README
3. Add to CI workflow if needed

## Best Practices

- Always clean up server processes
- Use timeouts to prevent hanging
- Test both success and error cases
- Document external dependencies
- Keep tests fast and reliable
- Use descriptive test names and messages