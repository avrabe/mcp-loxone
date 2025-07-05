# Modern Testing Strategy (2025)

This document outlines the comprehensive testing strategy for the Loxone MCP project, addressing the previous issues with race conditions, external dependencies, and disabled tests.

## Problems Addressed

### Previous Issues
1. **Race Conditions**: Environment variable interference between concurrent tests
2. **External Dependencies**: Tests requiring real Loxone hardware, Ollama instances, API keys
3. **Disabled Tests**: 80% of integration tests disabled with placeholder implementations
4. **Complex Environment Management**: Broken `ENV_TEST_MUTEX` and `TestEnvironment` patterns

### Solutions Implemented
1. **Modern Dependencies**: Added wiremock, mockall, serial_test, temp-env, rstest, testcontainers
2. **Proper Isolation**: Replaced mutex-based env management with `temp-env` and `serial_test`
3. **HTTP Mocking**: WireMock-based Loxone API simulation
4. **Test Fixtures**: Consistent test data and configuration with rstest

## Testing Infrastructure

### Dependencies Added

```toml
[dev-dependencies]
# Async test isolation
serial_test = "3.0"          # Sequential test execution for shared state
temp-env = "0.3"             # Environment variable isolation

# HTTP API mocking
wiremock = "0.6"             # HTTP mocking for Loxone API
httpmock = "0.7"             # Alternative HTTP mock server

# General mocking
mockall = "0.12"             # Trait mocking for dependencies

# Containers & external services
testcontainers = "0.15"      # Container-based testing
testcontainers-modules = "0.3" # Pre-built container modules

# Test utilities
rstest = "0.18"              # Test fixtures & parameterized tests
pretty_assertions = "1.4"    # Better assertion output
```

### Test Organization

```
tests/
├── common/
│   ├── mod.rs              # Common test infrastructure
│   ├── loxone_mock.rs      # WireMock-based Loxone API mocking
│   └── test_fixtures.rs    # Reusable test fixtures with rstest
├── modern_auth_tests.rs    # Example of new testing approach
├── llm_integration_tests.rs # Updated to use new patterns
└── [other test files]      # To be updated
```

## Testing Patterns

### 1. Environment Variable Isolation

**Old Pattern (Broken)**:
```rust
static ENV_TEST_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[tokio::test]
async fn test_with_env() {
    let _lock = ENV_TEST_MUTEX.lock().await;
    let _guard = TestEnvironment::clean().apply();
    // Test code - race conditions still possible
}
```

**New Pattern (2025)**:
```rust
use temp_env::with_vars;
use serial_test::serial;

#[tokio::test]
#[serial]  // Ensures sequential execution if needed
async fn test_with_env() {
    with_vars([
        ("LOXONE_USERNAME", Some("test")),
        ("LOXONE_PASSWORD", Some("test")),
        ("OPENAI_API_KEY", None), // Clear variable
    ], || {
        // Test code with isolated environment
        let config = Config::from_env();
        assert!(!config.openai.enabled);
    });
}
```

### 2. HTTP API Mocking

**Old Pattern (External Dependency)**:
```rust
#[tokio::test]
#[ignore = "Requires actual Loxone Miniserver connection"]
async fn test_auth() {
    // Hardcoded IP, requires real hardware
    let config = LoxoneConfig {
        url: Url::parse("http://192.168.1.100").unwrap(),
        // ...
    };
}
```

**New Pattern (Mocked)**:
```rust
#[tokio::test]
async fn test_auth_with_mock() {
    let mock_server = MockLoxoneServer::start().await;
    let config = test_loxone_config(mock_server.url());
    
    // Test against mock server - no external dependencies
    let client = create_client(&config, &credentials).await;
    assert!(client.is_ok());
}
```

### 3. Test Fixtures with rstest

**Old Pattern (Manual Setup)**:
```rust
#[tokio::test]
async fn test_device_control() {
    let config = LoxoneConfig {
        url: Url::parse("http://localhost:8080").unwrap(),
        username: "test".to_string(),
        // ... repeated setup code
    };
}
```

**New Pattern (Fixtures)**:
```rust
#[rstest]
#[tokio::test]
async fn test_device_control(test_loxone_config: LoxoneConfig) {
    // Config automatically provided by fixture
    let client = create_client(&test_loxone_config, &credentials).await;
}
```

## Mock Infrastructure

### MockLoxoneServer Features

- **Automatic Setup**: Default endpoints for common API calls
- **Customizable**: Add specific mocks for test scenarios
- **Realistic**: Mimics actual Loxone API responses
- **Isolated**: Each test gets its own server instance

```rust
// Basic usage
let mock_server = MockLoxoneServer::start().await;

// Custom mocks
mock_server.mock_sensor_data("device-uuid", "temperature", 23.5).await;
mock_server.mock_error_response("/invalid/path", 404, "Not found").await;

// Use in tests
let config = test_loxone_config(mock_server.url());
```

### Supported Endpoints

- Structure file (`/data/LoxAPP3.json`)
- Authentication (`/jdev/sys/getkey2/*`, `/jdev/cfg/api`)
- Device states (`/jdev/sps/io/*`)
- Device controls (`/jdev/sps/io/*/On`, `/jdev/sps/io/*/FullUp`, etc.)
- Custom endpoints via `add_mock()`

## Container-Based Testing

For complex scenarios requiring real services:

```rust
use testcontainers::*;

#[tokio::test]
async fn test_with_database() {
    let container = GenericImage::new("libsql/sqld", "latest")
        .with_exposed_port(8080)
        .start()
        .await;

    let port = container.get_host_port_ipv4(8080).await;
    let db_url = format!("http://localhost:{}/", port);
    
    // Test with real database
}
```

## Migration Guide

### Step 1: Update Test Dependencies
Already completed - new dependencies added to `Cargo.toml`.

### Step 2: Convert Environment Tests
Replace `ENV_TEST_MUTEX` usage with `temp-env`:

```rust
// Before
let _lock = ENV_TEST_MUTEX.lock().await;
let _guard = TestEnvironment::clean().apply();

// After  
#[serial]  // Add if shared state
with_vars(get_clean_env(), || {
    // Test code
});
```

### Step 3: Add HTTP Mocking
Replace hardcoded URLs with mock servers:

```rust
// Before
#[ignore = "Requires actual Loxone Miniserver connection"]

// After
let mock_server = MockLoxoneServer::start().await;
let config = test_loxone_config(mock_server.url());
```

### Step 4: Use Test Fixtures
Replace manual config setup with fixtures:

```rust
// Before
let config = LoxoneConfig { /* manual setup */ };

// After
#[rstest]
fn test_function(test_loxone_config: LoxoneConfig) {
    // Use provided config
}
```

## Test Categories

### Unit Tests ✅
- **Status**: Working (289 tests passing)
- **Pattern**: Standard Rust unit tests
- **Coverage**: Core logic, error handling, utilities

### Integration Tests 🔄
- **Status**: Being modernized
- **Pattern**: Mock-based with WireMock
- **Coverage**: API integration, client behavior

### End-to-End Tests 🔄
- **Status**: Optional with containers
- **Pattern**: Testcontainers for real services
- **Coverage**: Full system behavior

### LLM Provider Tests ✅
- **Status**: Fixed race conditions
- **Pattern**: `temp-env` + `serial_test`
- **Coverage**: Environment-based configuration

## Benefits

### Development Experience
- ✅ **Parallel Execution**: Tests run concurrently without interference
- ✅ **No External Dependencies**: Tests work without real hardware/services
- ✅ **Fast Feedback**: Mock servers start instantly
- ✅ **Deterministic**: Consistent results across environments

### CI/CD Integration
- ✅ **Reliable**: No network dependencies or timeouts
- ✅ **Fast**: No waiting for external services
- ✅ **Comprehensive**: All scenarios testable via mocking
- ✅ **Scalable**: Easy to add new test scenarios

### Maintenance
- ✅ **Clear Patterns**: Consistent testing approaches
- ✅ **Reusable**: Common fixtures and utilities
- ✅ **Extensible**: Easy to add new mock endpoints
- ✅ **Debuggable**: Clear separation of concerns

## Next Steps

1. **Phase 3**: Complete HTTP mocking infrastructure ✅
2. **Phase 4**: Update remaining disabled tests
3. **Phase 5**: Add container-based tests for complex scenarios
4. **Phase 6**: Performance benchmarking with criterion
5. **Phase 7**: Property-based testing with proptest

## Examples

See `tests/modern_auth_tests.rs` for a complete example of the new testing approach, demonstrating:
- WireMock HTTP mocking
- rstest fixtures
- Environment isolation
- Serial test execution
- Comprehensive mock endpoints