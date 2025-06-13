# CI/CD Pipeline Documentation

This document describes the Continuous Integration and Continuous Deployment (CI/CD) pipeline for the Loxone MCP Rust server.

## Overview

The CI/CD pipeline ensures code quality, security, and cross-platform compatibility through automated checks that run on every push and pull request.

## GitHub Actions Workflows

### 1. CI Workflow (`.github/workflows/ci.yml`)

The main CI workflow runs comprehensive quality checks on multiple platforms:

#### Jobs:
- **Format Check** (`fmt`): Verifies code formatting with `rustfmt`
- **Clippy Check** (`clippy`): Runs linting and static analysis
- **Test Suite** (`test`): Builds and tests on Ubuntu, Windows, and macOS with stable and beta Rust
- **Security Audit** (`security`): Scans for security vulnerabilities
- **WASM Build** (`wasm`): Verifies WebAssembly compilation
- **Documentation** (`docs`): Builds documentation with warnings as errors

#### Triggers:
- Push to `main` or `develop` branches
- Pull requests to `main` branch

#### Features:
- ✅ Multi-platform testing (Linux, Windows, macOS)
- ✅ Multiple Rust versions (stable, beta)
- ✅ Caching for faster builds
- ✅ Parallel job execution
- ✅ Zero-warning policy

### 2. Release Workflow (`.github/workflows/release.yml`)

Automates the release process and creates binaries for multiple platforms:

#### Jobs:
- **Create Release**: Creates a GitHub release
- **Build Release**: Builds optimized binaries for:
  - Linux (x86_64)
  - macOS (x86_64 and ARM64)
  - Windows (x86_64)
  - WebAssembly (wasm32-wasip2)

#### Triggers:
- Git tags matching `v*` (e.g., `v1.0.0`)

#### Artifacts:
- Native binaries for all major platforms
- WebAssembly modules for web deployment
- Compressed archives (tar.gz for Unix, zip for Windows)

## Local Development

### Quick Commands

```bash
# Run all CI checks locally
make ci-check

# Individual checks (matches CI jobs)
make ci-format    # Format check
make ci-clippy    # Clippy linting
make ci-test      # Test suite
make ci-security  # Security audit
make ci-wasm      # WASM build
make ci-docs      # Documentation

# Development helpers
make check        # Quick check for development
make format       # Fix formatting issues
make lint         # Run clippy with fixes
```

### Pre-commit Hooks

Install the pre-commit hook to run quality checks automatically:

```bash
# Install the hook
cp .githooks/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit

# The hook will now run on every commit
git commit -m "Your commit message"

# To bypass (use sparingly)
git commit --no-verify -m "Emergency fix"
```

The pre-commit hook runs:
1. ✅ Code formatting check
2. ✅ Clippy linting
3. ✅ Build verification
4. ✅ Quick test suite
5. ⚠️ Security audit (warning only)

## Quality Standards

### Code Quality
- **Zero Warnings**: All code must compile without warnings
- **Clippy Clean**: All clippy suggestions must be addressed
- **Formatted**: Code must be formatted with `rustfmt`
- **Tested**: All new code should include appropriate tests

### Security
- **Audit Clean**: No known security vulnerabilities
- **Dependency Updates**: Regular security audits of dependencies
- **Safe Code**: Follow Rust security best practices

### Cross-Platform
- **Multi-Platform**: Code must work on Linux, Windows, and macOS
- **WASM Compatible**: Core functionality available in WebAssembly
- **Architecture Support**: Support for x86_64 and ARM64

## Debugging CI Issues

### Common Issues and Solutions

1. **Format Check Failures**
   ```bash
   # Fix locally
   make format
   git add -A && git commit --amend --no-edit
   ```

2. **Clippy Warnings**
   ```bash
   # See warnings
   make lint
   # Fix common issues automatically
   cargo fix --allow-dirty --allow-staged
   ```

3. **Test Failures**
   ```bash
   # Run tests locally
   make test
   # Run specific test
   cargo test test_name -- --nocapture
   ```

4. **WASM Build Issues**
   ```bash
   # Install WASM target
   rustup target add wasm32-wasip2
   # Test WASM build
   make ci-wasm
   ```

### CI Environment Variables

The CI pipeline uses these environment variables:

- `CARGO_TERM_COLOR=always`: Colored output in CI logs
- `RUST_BACKTRACE=1`: Full backtraces for debugging
- `RUSTDOCFLAGS="-D warnings"`: Treat doc warnings as errors

### Matrix Strategy

The test job uses a matrix strategy to test multiple configurations:

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, windows-latest, macos-latest]
    rust: [stable, beta]
    exclude:
      # Only test beta on Linux to save CI time
      - os: windows-latest
        rust: beta
      - os: macos-latest
        rust: beta
```

## Performance Optimization

### Caching Strategy

The CI pipeline uses aggressive caching to speed up builds:

1. **Cargo Registry Cache**: Caches downloaded crates
2. **Cargo Index Cache**: Caches crate index
3. **Build Cache**: Caches compiled dependencies

### Parallel Execution

Jobs are designed to run in parallel where possible:
- Format and clippy checks run independently
- Test matrix runs all combinations in parallel
- Security and documentation builds run separately

## Security Considerations

### Dependency Management
- Regular `cargo audit` runs in CI
- Dependabot updates for security patches
- Manual review of dependency updates

### Secret Management
- No secrets required for CI builds
- Release workflow uses GitHub tokens
- All builds are reproducible

## Monitoring and Notifications

### Build Status
- GitHub status checks on pull requests
- Build badges in README
- Email notifications for failed builds

### Performance Tracking
- Build time monitoring
- Binary size tracking
- Test execution time analysis

## Contributing

When contributing to this project:

1. **Fork and Branch**: Create a feature branch from `develop`
2. **Local Testing**: Run `make ci-check` before pushing
3. **Pull Request**: Ensure all CI checks pass
4. **Code Review**: Address feedback and maintain quality standards

### Release Process

1. **Create Tag**: `git tag v1.0.0 && git push origin v1.0.0`
2. **Automatic Release**: GitHub Actions creates release and binaries
3. **Manual Review**: Verify release artifacts and update release notes

## Troubleshooting

### Failed Builds
1. Check CI logs for specific errors
2. Reproduce locally with `make ci-check`
3. Fix issues and push updates
4. Re-run failed jobs if needed

### Performance Issues
1. Check cache hit rates
2. Review build times by job
3. Optimize dependencies if needed
4. Consider parallelization improvements

For questions or issues with the CI/CD pipeline, please open an issue or contact the maintainers.