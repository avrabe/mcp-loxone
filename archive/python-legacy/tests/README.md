# Test Suite Documentation

This directory contains comprehensive tests for the Loxone MCP server, designed to work without requiring a real Loxone server for CI/CD compatibility.

## Test Coverage Overview

### Current Coverage: **18%** (up from 11%)
- **Total Tests:** 146 (up from 10)
- **All tests pass in CI environment**
- **Zero dependency on real hardware**

### Module-by-Module Coverage:
- `__init__.py`: **100%** ✅
- `__main__.py`: **100%** ✅  
- `credentials.py`: **55%** (critical security module)
- `loxone_http_client.py`: **85%** (primary communication layer)
- `sse_server.py`: **37%** (event streaming)
- `server.py`: **11%** (complex async MCP tools)

## Test Files Structure

### Core Test Files
- `test_basic_coverage.py` - Fundamental functionality and imports
- `test_server.py` - Server integration and room management
- `test_server_coverage_boost.py` - Extended server functionality
- `test_coverage_final_push.py` - Additional comprehensive tests

### Deep Coverage Files
- `test_credentials_deep_coverage.py` - Credential management (setup, validation, discovery)
- `test_http_client_deep_coverage.py` - HTTP client (async/sync methods, error handling)
- `test_sse_server_deep_coverage.py` - SSE server (configuration, lifecycle)

## Test Categories

### 1. Import and Structure Tests
- Module imports work correctly
- Package structure validation
- Class and function existence checks
- Constants and configuration validation

### 2. Credential Management Tests
- Environment variable handling
- Keychain integration (mocked)
- Server discovery simulation
- Validation workflows
- Error handling scenarios

### 3. HTTP Client Tests
- Client initialization and configuration
- Async method testing (connect, get_structure_file, send_command)
- Sync method testing (start, stop, authenticate)
- Error handling (timeouts, connection errors, auth failures)
- URL generation and credential storage

### 4. Server Tool Tests
- All 31 MCP tools existence verification
- Tool categorization (lighting, weather, environmental, scenes)
- Helper function testing (normalize_action, find_matching_room)
- Constants and mapping validation

### 5. SSE Server Tests
- Configuration handling
- Environment variable processing
- Lifecycle management
- Error condition handling

### 6. Integration Tests
- Room management workflows
- Device filtering and querying
- Context handling patterns

## Mock Strategy

### What We Mock:
- **Keychain operations** (`keyring` library)
- **HTTP requests** (`httpx.AsyncClient`)
- **Network discovery** (UDP and HTTP scanning)
- **File system access** (when needed)
- **Async server creation** (`asyncio.create_server`)

### What We Don't Mock:
- **Basic data structures** (dataclasses, dictionaries)
- **String manipulation** and helper functions
- **Import mechanisms**
- **Environment variable access**
- **Configuration parsing**

## Running Tests

### Full Test Suite
```bash
# Run all tests with coverage
uv run pytest tests/ --cov=loxone_mcp --cov-report=term-missing

# Run with HTML coverage report
uv run pytest tests/ --cov=loxone_mcp --cov-report=html

# Run specific test file
uv run pytest tests/test_credentials_deep_coverage.py -v
```

### Test Performance
```bash
# Run with timing information
uv run pytest tests/ --durations=10

# Run with minimal output
uv run pytest tests/ -q

# Stop on first failure
uv run pytest tests/ -x
```

## CI/CD Compatibility

### Key Design Principles:
1. **No Real Server Required:** All tests use mocks and stubs
2. **Environment Variable Testing:** Comprehensive env var handling
3. **Error Resilience:** Tests handle expected failures gracefully
4. **Fast Execution:** Minimal network timeouts and async delays
5. **Deterministic Results:** No flaky tests or race conditions

### Environment Variables Used in Tests:
- `LOXONE_HOST` - Miniserver hostname/IP
- `LOXONE_USER` - Username for authentication
- `LOXONE_PASS` - Password for authentication
- `LOXONE_SSE_PORT` - SSE server port override
- `LOXONE_SSE_HOST` - SSE server host override
- `LOXONE_LOG_LEVEL` - Logging level configuration

## Test Quality Metrics

### Coverage Quality:
- **High-value modules** (credentials, HTTP client) have >50% coverage
- **Critical security functions** are thoroughly tested
- **All public APIs** have basic functionality tests
- **Error handling paths** are validated

### Test Reliability:
- **Zero flaky tests** - all tests pass consistently
- **Proper async handling** - no "coroutine was never awaited" errors
- **Resource cleanup** - no memory leaks or hanging connections
- **Mock isolation** - tests don't interfere with each other

## Adding New Tests

### Guidelines for New Tests:
1. **Follow naming convention:** `test_[module]_[functionality].py`
2. **Use appropriate mocks:** Mock external dependencies, not business logic
3. **Test edge cases:** Include error conditions and boundary values
4. **Document test purpose:** Clear docstrings explaining what is tested
5. **Maintain CI compatibility:** No real server dependencies

### Example Test Structure:
```python
class TestNewFunctionality:
    \"\"\"Test new functionality for better coverage.\"\"\"

    @patch('external.dependency')
    def test_success_case(self, mock_dependency):
        \"\"\"Test successful operation.\"\"\"
        # Arrange
        mock_dependency.return_value = "expected_result"
        
        # Act
        result = function_under_test()
        
        # Assert
        assert result == "expected_result"

    def test_error_case(self):
        \"\"\"Test error handling.\"\"\"
        with pytest.raises(ExpectedError):
            function_under_test(invalid_input)
```

## Troubleshooting

### Common Issues:
1. **Import Errors:** Ensure all dependencies are installed (`uv sync`)
2. **Async Warnings:** Make sure async functions are properly awaited
3. **Mock Failures:** Check that mocks match actual function signatures
4. **Environment Conflicts:** Use `patch.dict(os.environ, {}, clear=True)` to isolate

### Debug Commands:
```bash
# Verbose output with print statements
uv run pytest tests/ -v -s

# Run specific test with debugging
uv run pytest tests/test_specific.py::TestClass::test_method -v -s

# Check coverage for specific module
uv run pytest tests/ --cov=loxone_mcp.credentials --cov-report=term-missing
```

## Future Improvements

### Potential Enhancements:
1. **Server.py Coverage:** Add more comprehensive async function testing
2. **Integration Scenarios:** End-to-end workflow testing
3. **Performance Tests:** Load testing for high-volume scenarios
4. **Security Tests:** Additional credential validation scenarios
5. **Error Recovery:** Testing recovery from various failure modes

### Coverage Goals:
- **Short-term:** Reach 25% overall coverage
- **Medium-term:** 40% coverage with focus on server.py
- **Long-term:** 60% coverage with comprehensive integration tests