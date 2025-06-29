# Contributing to Loxone MCP Server

Thank you for your interest in contributing to the Loxone MCP Server project. This guide will help you get started.

## Code of Conduct

By participating in this project, you agree to maintain a respectful and inclusive environment for all contributors.

## Getting Started

### Prerequisites

- Rust 1.70 or higher
- Git
- A Loxone Miniserver (for testing)

### Development Setup

1. **Fork and clone the repository**:
   ```bash
   git clone https://github.com/yourusername/loxone-mcp-rust
   cd loxone-mcp-rust
   ```

2. **Install development dependencies**:
   ```bash
   # Install Rust toolchain components
   rustup component add rustfmt clippy
   
   # Install development tools
   cargo install cargo-watch cargo-audit
   ```

3. **Set up pre-commit hooks** (optional):
   ```bash
   cp .githooks/pre-commit .git/hooks/
   chmod +x .git/hooks/pre-commit
   ```

## Development Workflow

### 1. Create a Feature Branch

```bash
git checkout -b feature/your-feature-name
```

### 2. Make Your Changes

Follow the coding standards and ensure your changes:
- Have appropriate tests
- Pass all existing tests
- Include documentation updates
- Follow Rust idioms

### 3. Run Quality Checks

Before committing:

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Run tests
cargo test

# Check for security issues
cargo audit
```

### 4. Commit Your Changes

Write clear, descriptive commit messages:

```
feat: add support for scene activation

- Implement get_light_scenes tool
- Add set_light_scene for scene control
- Include tests for scene validation
```

Commit message format:
- `feat:` New features
- `fix:` Bug fixes
- `docs:` Documentation changes
- `test:` Test additions/changes
- `refactor:` Code refactoring
- `chore:` Maintenance tasks

### 5. Submit a Pull Request

1. Push to your fork
2. Create a pull request against `main`
3. Fill out the PR template
4. Wait for review

## Coding Standards

### Rust Style

Follow the official Rust style guide:

```rust
// Good
pub fn control_device(uuid: &str, command: &str) -> Result<()> {
    validate_uuid(uuid)?;
    // Implementation
}

// Avoid
pub fn ControlDevice(UUID: &str, cmd: &str) -> Result<()> {
    // Non-idiomatic naming
}
```

### Error Handling

Use the custom error types:

```rust
use crate::error::{LoxoneError, Result};

fn process_command(cmd: &str) -> Result<()> {
    if cmd.is_empty() {
        return Err(LoxoneError::validation("Command cannot be empty"));
    }
    // Process command
    Ok(())
}
```

### Documentation

Document all public APIs:

```rust
/// Controls a Loxone device by UUID.
///
/// # Arguments
/// * `uuid` - The device UUID in Loxone format
/// * `command` - Command to send (e.g., "on", "off", "50")
///
/// # Returns
/// * `Ok(())` on success
/// * `Err(LoxoneError)` on failure
///
/// # Example
/// ```
/// control_device("0f869a3f-0155-8b3f-ffff403fb0c34b9e", "on")?;
/// ```
pub fn control_device(uuid: &str, command: &str) -> Result<()> {
    // Implementation
}
```

### Testing

Write tests for new functionality:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid_validation() {
        assert!(validate_uuid("0f869a3f-0155-8b3f-ffff403fb0c34b9e").is_ok());
        assert!(validate_uuid("invalid-uuid").is_err());
    }

    #[tokio::test]
    async fn test_device_control() {
        let client = MockClient::new();
        let result = control_device_with_client(&client, "uuid", "on").await;
        assert!(result.is_ok());
    }
}
```

## Adding New Features

### Adding a New MCP Tool

1. **Add to `src/tools/adapters.rs`**:
   ```rust
   pub async fn my_new_tool(
       context: ToolContext,
       params: MyToolParams,
   ) -> ToolResult {
       // Implementation
   }
   ```

2. **Define parameters**:
   ```rust
   #[derive(Debug, Deserialize, JsonSchema)]
   pub struct MyToolParams {
       #[serde(description = "Parameter description")]
       pub param_name: String,
   }
   ```

3. **Register in tool system**:
   ```rust
   Tool::new("my_new_tool")
       .description("Tool description")
       .parameter("param_name", ParameterType::String, "Parameter description")
       .handler(my_new_tool)
   ```

4. **Add tests**:
   ```rust
   #[test]
   fn test_my_new_tool() {
       // Test implementation
   }
   ```

5. **Update documentation**:
   - Add to `docs/tools_reference.md`
   - Update README if significant feature

### Adding Device Support

For new Loxone device types:

1. Check device type in Miniserver structure
2. Add filtering logic in relevant tools
3. Test with actual device
4. Document device-specific behavior

## Testing

### Unit Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture
```

### Integration Tests

```bash
# Requires configured Loxone credentials
cargo test --test integration_tests
```

### Manual Testing

1. Build the server: `cargo build`
2. Run with test credentials
3. Use MCP Inspector or curl to test tools
4. Verify Miniserver state changes

## Documentation

### Where to Document

- **API Changes**: Update `docs/tools_reference.md`
- **Architecture**: Update `docs/architecture.md`
- **Security**: Update `docs/security.md`
- **Examples**: Add to relevant tool documentation

### Documentation Style

- Use clear, concise language
- Include code examples
- Explain rationale for design decisions
- Keep formatting consistent

## Performance Considerations

### Benchmarking

For performance-critical changes:

```rust
#[bench]
fn bench_cache_lookup(b: &mut Bencher) {
    let cache = build_test_cache();
    b.iter(|| {
        cache.get("test_key")
    });
}
```

### Optimization Guidelines

- Profile before optimizing
- Maintain readability
- Document performance tricks
- Add benchmarks for regressions

## Security

### Security Review

All PRs touching security-sensitive areas require:

1. Careful review of input validation
2. Authentication/authorization checks
3. No hardcoded credentials
4. Proper error messages (no info leakage)

### Reporting Security Issues

See [security.md](docs/security.md) for vulnerability reporting.

## Release Process

1. Update version in `Cargo.toml`
2. Update CHANGELOG.md
3. Create release PR
4. After merge, tag release
5. GitHub Actions builds releases

## Getting Help

- **Questions**: Open a GitHub issue
- **Discussions**: GitHub Discussions
- **Real-time**: Community chat (if available)

## Recognition

Contributors are recognized in:
- GitHub contributors page
- CHANGELOG.md (for significant contributions)
- Release notes

Thank you for contributing to make Loxone home automation more accessible!