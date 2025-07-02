# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2025-06-30

### Added

#### MCP Inspector Compatibility Enhancement
- **NEW: `StreamableHttp` transport variant** - Added MCP Inspector compatibility to the PulseEngine framework
  - Added `TransportConfig::StreamableHttp` enum variant with port and host configuration
  - Added `TransportConfig::streamable_http(port)` helper method for easy one-line configuration
  - Updated `create_transport()` function to handle StreamableHttp transport creation
  - Implements required `/sse` and `/messages` endpoints for MCP Inspector connectivity
  - Maintains full backward compatibility with existing HTTP transport usage

#### Framework Integration
- **Enhanced Developer Experience** - Framework applications can now enable MCP Inspector support with a single line change:
  - **Before**: `TransportConfig::http(3000)` (not Inspector compatible)
  - **After**: `TransportConfig::streamable_http(3000)` (Inspector compatible)
  - No breaking changes to existing applications
  - Framework-level solution eliminates need to bypass `create_transport()`

### Changed
- Updated all framework package versions from `0.2.0` to `0.3.0` for consistency
- Enhanced transport layer documentation with Inspector compatibility examples

### Technical Details
- **StreamableHttp Transport**: Implements the streamable-http protocol that MCP Inspector expects
- **Session Management**: Automatic session creation and management via `Mcp-Session-Id` headers  
- **Endpoint Support**: 
  - `GET /sse?sessionId=<id>` - Session establishment for Inspector
  - `POST /messages` - MCP message exchange endpoint
  - `GET /` - Basic server status endpoint
- **Framework Integration**: Seamless integration with PulseEngine framework's `create_transport()` function

### Migration Guide
For applications wanting MCP Inspector compatibility, update transport configuration:

```rust
// Old (still works, but not Inspector compatible)
let transport = create_transport(TransportConfig::http(3001))?;

// New (Inspector compatible)
let transport = create_transport(TransportConfig::streamable_http(3001))?;
```

This change enables MCP Inspector connectivity while maintaining full backward compatibility.

## [0.2.0] - 2025-06-29

This is a major release that introduces significant improvements to the MCP framework architecture, code quality, and developer experience.

### Added

#### Framework Enhancements
- **NEW: `mcp-cli` crate** - Command-line interface generation with derive macros
  - `McpConfig` derive macro for automatic CLI argument parsing
  - `McpBackend` derive macro with advanced configuration options
  - Integrated with `clap` for robust CLI experience
  - Support for environment variable integration

- **NEW: Server Builder Pattern** - Fluent API for server configuration
  - `ServerBuilder` with method chaining for cleaner setup
  - Transport configuration methods (`with_stdio()`, `with_http()`, `with_websocket()`)
  - Security configuration integration
  - CORS policy configuration
  - Middleware support framework

- **Enhanced Authentication System**
  - API key management with roles and permissions
  - IP whitelist support
  - Expiration and audit logging
  - Security validation tools

#### Developer Experience
- **Comprehensive Examples** - Real-world usage patterns for all framework components
- **Improved Documentation** - Updated README files with current version numbers
- **Better Error Messages** - More descriptive error handling throughout framework

### Changed

#### Version Alignment
- **ALL CRATES** updated to version 0.2.0 for consistency
- **Dependency Alignment** - All internal framework dependencies now reference 0.2.0
- **Documentation Updates** - All README files updated with correct version numbers

#### Code Quality Improvements
- **Complete Clippy Cleanup** - Fixed all clippy warnings across entire codebase
  - Addressed performance optimizations (unnecessary clones, string allocations)
  - Fixed style issues (redundant pattern matching, verbose syntax)
  - Improved error handling patterns
  - Enhanced async/await usage

#### Framework Architecture
- **Separation of Concerns** - Clear distinction between framework and application code
- **Trait Improvements** - More flexible backend trait implementations
- **Transport Layer** - Enhanced HTTP, WebSocket, and stdio transport reliability

### Technical Details

#### Affected Crates
- `loxone-mcp-rust`: 0.1.1 → 0.2.0
- `pulseengine-mcp-protocol`: 0.1.2 → 0.2.0
- `pulseengine-mcp-server`: 0.1.2 → 0.2.0
- `pulseengine-mcp-transport`: 0.1.2 → 0.2.0
- `pulseengine-mcp-auth`: 0.1.2 → 0.2.0
- `pulseengine-mcp-security`: 0.1.2 → 0.2.0
- `pulseengine-mcp-monitoring`: 0.1.2 → 0.2.0
- `pulseengine-mcp-logging`: 0.1.2 → 0.2.0
- `pulseengine-mcp-cli`: 0.2.0 (new)

#### CLI Framework Features
- Environment variable support with `LOXONE_` prefix
- Configuration validation and helpful error messages
- Automatic help generation with detailed descriptions
- Integration with existing credential management

#### Server Builder Capabilities
```rust
McpServer::builder()
    .with_backend(backend)
    .with_http(3001)
    .with_cors_policy(cors_config)
    .with_middleware(security_middleware)
    .build()
```

### Compatibility

#### Maintained Compatibility
- **API Stability** - Core `McpBackend` trait remains compatible
- **Transport Protocols** - No breaking changes to MCP protocol implementation
- **Client Compatibility** - Continues to work with MCP Inspector, Claude Desktop

#### Breaking Changes
- **Version Dependencies** - Internal crate versions now require 0.2.0
- **CLI Integration** - New derive macros change backend initialization patterns

### Migration Guide

#### For Framework Users
1. Update dependency versions to 0.2.0 in `Cargo.toml`
2. Consider adopting new CLI derive macros for better UX
3. Optional: Migrate to `ServerBuilder` pattern for cleaner configuration

#### For Framework Contributors
1. All clippy warnings must be addressed before commits
2. New code should follow established patterns from this cleanup
3. Documentation updates should reference 0.2.0 versions

### Future Planning

This release prepares the framework for:
- **Separate Repository Publication** - Framework will be published as standalone crates
- **Community Adoption** - Clean, well-documented foundation for MCP implementations
- **Enhanced Examples** - More comprehensive real-world usage demonstrations

### Acknowledgments

This release represents a significant investment in code quality and developer experience, establishing a solid foundation for the MCP framework's future growth.

---

## [0.1.2] - Previous Release

Initial framework implementation with basic MCP protocol support, authentication, and transport layers.

## [0.1.1] - Initial Release

Basic Loxone MCP server implementation.