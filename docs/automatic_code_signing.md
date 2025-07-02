# Automatic Code Signing on macOS

This document describes the automatic code signing implementation for the Loxone MCP Rust server on macOS.

## Overview

To reduce keychain password prompts on macOS, the build system now automatically signs binaries after compilation. This is integrated into the build process through:

1. **build.rs** - Sets up code signing infrastructure during build
2. **cargo-build-sign.sh** - Wrapper script that builds and signs
3. **Makefile** - Integrates signing into standard build commands

## How It Works

### 1. Build Script Setup (`build.rs`)

The build script detects macOS and:
- Creates a post-build signing script
- Sets environment variables for code signing
- Provides user instructions

```rust
fn setup_macos_codesigning() {
    // Skip if SKIP_CODESIGN is set
    if env::var("SKIP_CODESIGN").is_ok() {
        return;
    }
    
    // Create post-build script
    create_post_build_script();
    
    // Notify user about signing options
    println!("cargo:warning=macOS detected: Use './cargo-build-sign.sh' or 'make build' for automatic code signing");
}
```

### 2. Build Wrapper (`cargo-build-sign.sh`)

This script:
1. Runs the standard cargo build command
2. Automatically signs the resulting binary
3. Verifies the signature

```bash
# Build with cargo
cargo build "$@"

# Sign the binary (ad-hoc or with identity)
if [ -n "$CODESIGN_IDENTITY" ]; then
    codesign -s "$CODESIGN_IDENTITY" --force --deep "$BINARY_PATH"
else
    codesign -s - --force --deep "$BINARY_PATH"
fi
```

### 3. Makefile Integration

The Makefile automatically uses the signing wrapper on macOS:

```makefile
build:
    @if [ "$(shell uname)" = "Darwin" ]; then \
        ./cargo-build-sign.sh --release; \
    else \
        cargo build --release; \
    fi
```

## Usage

### Basic Usage

```bash
# Recommended: Use make commands
make build          # Release build with signing
make dev-build      # Debug build with signing

# Direct usage
./cargo-build-sign.sh --release
./cargo-build-sign.sh --bin loxone-mcp-server
```

### Environment Variables

- `CODESIGN_IDENTITY` - Certificate identity for production signing (optional)
- `SKIP_CODESIGN` - Set to any value to disable automatic signing

### Examples

```bash
# Development (ad-hoc signing)
make build

# Production (with certificate)
CODESIGN_IDENTITY="Developer ID Application: Your Name" make build

# Skip signing
SKIP_CODESIGN=1 make build
```

## Benefits

1. **Reduced Keychain Prompts**: Signed binaries are trusted by macOS
2. **Automatic Process**: No manual signing steps required
3. **Flexible**: Supports both ad-hoc and certificate signing
4. **Integrated**: Works with existing build commands

## Troubleshooting

### Binary Not Signed

If the binary isn't signed after building:

```bash
# Check signature
codesign --verify --verbose=2 target/release/loxone-mcp-server

# Sign manually
./sign-binary.sh
```

### Code Signing Failed

Common issues:
- Xcode command line tools not installed
- Certificate not found (check `CODESIGN_IDENTITY`)
- Binary locked by another process

### Verification

To verify a binary is properly signed:

```bash
# Basic verification
codesign --verify target/release/loxone-mcp-server

# Detailed verification
codesign --verify --verbose=2 target/release/loxone-mcp-server

# Check signature details
codesign --display --verbose=2 target/release/loxone-mcp-server
```

## Implementation Notes

### Why Not in build.rs?

The binary doesn't exist when `build.rs` runs, so we can't sign during the build script. Instead, we:
1. Create signing infrastructure in `build.rs`
2. Use a wrapper script for the actual signing
3. Integrate into the build system via Makefile

### Ad-hoc vs Certificate Signing

- **Ad-hoc** (`codesign -s -`): Good for development, no certificate needed
- **Certificate** (`codesign -s "Identity"`): Required for distribution

### Future Improvements

1. **Cargo Post-Build Hook**: When Cargo supports post-build hooks, we can integrate more directly
2. **Automatic Certificate Detection**: Could auto-detect available signing certificates
3. **Notarization Support**: For distribution outside the App Store

## Related Documentation

- [README.md](../README.md) - Main documentation
- [Makefile](../Makefile) - Build commands
- [build.rs](../build.rs) - Build script implementation