# Publication Guide for PulseEngine MCP Framework

This guide documents the current state and next steps for publishing the remaining MCP framework crates to crates.io.

## Current Status

### ✅ Successfully Published (4/7 crates)
- **pulseengine-mcp-protocol v0.1.0** - Core MCP types and validation
- **pulseengine-mcp-logging v0.1.0** - Structured logging framework  
- **pulseengine-mcp-transport v0.1.0** - HTTP/WebSocket/stdio transports
- **pulseengine-mcp-auth v0.1.0** - Authentication and authorization

### ⏳ Ready to Publish (3/7 crates)
- **pulseengine-mcp-security** - Security middleware and validation
- **pulseengine-mcp-monitoring** - Metrics and observability  
- **pulseengine-mcp-server** - Complete server framework

### 🔒 Rate Limit Status
- **Blocked until**: Sat, 28 Jun 2025 08:23:58 GMT
- **Reason**: Published 4 crates in short period (normal for new publishers)
- **Solutions**: 
  - Wait for automatic reset (~24 hours)
  - Email help@crates.io to request limit increase

## Local Development Setup

### Cargo Patch System
The workspace uses Cargo's patch system for seamless local development:

```toml
# [patch.crates-io] section no longer needed
# All framework crates are now published to crates.io as version 0.3.1
```

### How It Works
- **Development**: Local changes propagate immediately via patches
- **Published**: External users see clean published version dependencies
- **CI**: Can disable patches by setting `CARGO_PATCH_DISABLE=1`

## Migration Complete ✅

### Framework Published Successfully

All MCP framework crates have been published to crates.io as version 0.3.1:

- ✅ `pulseengine-mcp-protocol`
- ✅ `pulseengine-mcp-server` 
- ✅ `pulseengine-mcp-transport`
- ✅ `pulseengine-mcp-auth`
- ✅ `pulseengine-mcp-security`
- ✅ `pulseengine-mcp-monitoring`
- ✅ `pulseengine-mcp-logging`
- ✅ `pulseengine-mcp-cli`
- ✅ `pulseengine-mcp-cli-derive`

### Using Published Crates

Add to your Cargo.toml:
```toml
[dependencies]
pulseengine-mcp-server = "0.3.1"
pulseengine-mcp-protocol = "0.3.1"

# Test workspace compiles
cargo check --workspace
```

## Repository Structure

```
mcp-framework/
├── mcp-protocol/      # ✅ Published v0.1.0
├── mcp-logging/       # ✅ Published v0.1.0
├── mcp-transport/     # ✅ Published v0.1.0  
├── mcp-auth/          # ✅ Published v0.1.0
├── mcp-security/      # ⏳ Ready to publish
├── mcp-monitoring/    # ⏳ Ready to publish
├── mcp-server/        # ⏳ Ready to publish
└── examples/
    └── hello-world/   # Example using published crates
```

## Dependency Graph

```
mcp-protocol (published) ←── mcp-transport (published)
     ↑                            ↑
     ├── mcp-auth (published)     │
     ├── mcp-security (ready) ────┤
     ├── mcp-monitoring (ready) ──┤
     └── mcp-server (ready) ←──────┘
```

## Technical Details

### All Crates Use Published Versions
Every framework crate now references published versions in their Cargo.toml:
```toml
[dependencies]
pulseengine-mcp-protocol = "0.1.0"
```

### Local Development Seamless
Thanks to the patch system, local changes work immediately:
- Edit any framework crate source
- Changes propagate instantly to dependent crates
- No version juggling required

### External Users See Clean Dependencies
When published, users will see normal semantic versioning:
```toml
[dependencies]
pulseengine-mcp-server = "0.1.0"
```

## Publishing Checklist

Before publishing any crate:
- [ ] Code compiles without errors
- [ ] Documentation is complete
- [ ] Keywords are under 20 characters
- [ ] License is "MIT OR Apache-2.0"
- [ ] Repository URL is correct
- [ ] Dependencies use published versions

## Troubleshooting

### "Package not found" Errors
This is expected for unpublished crates. The server crate depends on monitoring/security which aren't published yet.

### Rate Limit Errors
```
error: You have published too many new crates in a short period of time
```
This is normal for new publishers. Wait ~24 hours or email help@crates.io.

### Local Development Issues
If patch system isn't working:
1. Run `cargo clean`
2. Run `cargo check --workspace`
3. Verify patch paths in workspace Cargo.toml

## Future Versions

For version bumps:
1. Update version in individual crate Cargo.toml
2. Update dependency versions in dependent crates
3. The patch system will handle local development automatically

## Framework Usage

Once all crates are published, users can depend on the framework:

```toml
[dependencies]
pulseengine-mcp-server = "0.1.0"
```

The server crate re-exports everything needed:
```rust
use pulseengine_mcp_server::{McpServer, McpBackend, ServerConfig};
```