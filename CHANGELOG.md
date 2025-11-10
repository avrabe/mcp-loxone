# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.0] - 2025-11-10

### Added
- **MCP Protocol 2025-06-18 Support**: Upgraded to pulseengine-mcp 0.13.0
  - Full MCP specification 2025-06-18 compliance
  - JSON serialization for all tool responses (breaking change from v0.13.0)
  - `NumberOrString` type support for flexible request IDs
  - `_meta` fields across protocol types for extensibility

- **Enhanced Server Capabilities**:
  - ✅ **Sampling capability enabled** - Server can now initiate LLM calls
  - ✅ **Elicitation capability enabled** - Server can request structured user input
  - Better integration with Claude Desktop and MCP Inspector
  - Server-initiated agentic behaviors for intelligent automation

### Changed
- **BREAKING: All Tool Return Types Must Implement Serialize**
  - Tool responses now use JSON serialization instead of Debug format
  - Ensures proper structured content in MCP tool call results
  - Fixed `LoxoneWeatherConfig` to include Serialize/Deserialize traits

- **Framework Dependencies**:
  - Updated all pulseengine-mcp crates from 0.5.0 → 0.13.0
  - `pulseengine-mcp-protocol`: 0.5.0 → 0.13.0
  - `pulseengine-mcp-server`: 0.5.0 → 0.13.0
  - `pulseengine-mcp-transport`: 0.5.0 → 0.13.0
  - `pulseengine-mcp-auth`: 0.5.0 → 0.13.0
  - `pulseengine-mcp-security`: 0.5.0 → 0.13.0
  - `pulseengine-mcp-monitoring`: 0.5.0 → 0.13.0
  - `pulseengine-mcp-cli`: 0.5.0 → 0.13.0
  - `pulseengine-mcp-cli-derive`: 0.5.0 → 0.13.0
  - `pulseengine-mcp-logging`: 0.5.0 → 0.13.0

### Fixed
- **Serialization Compliance**: Added Serialize/Deserialize to `LoxoneWeatherConfig`
- **Type Safety**: All 30+ tools now fully compliant with JSON serialization requirement
- **Protocol Compatibility**: Updated to latest MCP specification (2025-06-18)

### Migration Guide

#### For Developers
If you've added custom tool types, ensure they implement `Serialize`:
```rust
// Before (0.6.0)
#[derive(Debug, Clone)]
pub struct MyToolResult { ... }

// After (0.7.0) - Required!
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyToolResult { ... }
```

#### New Capabilities Available
The server now advertises sampling and elicitation capabilities:
- **Sampling**: Use for server-initiated LLM completions in automation scenarios
- **Elicitation**: Request structured input from users for better UX

### Technical Details
- **Tested Against**: MCP Inspector, Claude Desktop
- **Protocol Version**: MCP 2025-06-18
- **Rust Version**: 1.79+
- **All Tests Passing**: 270+ tests verified with new framework

## [0.6.0] - 2025-01-20

### Added
- **Framework Upgrade**: Migration to pulseengine-mcp 0.5.0
  - Enhanced server capabilities with elicitation support
  - Tool output schema definitions for better type safety
  - Structured content support in tool call results
  - Improved framework authentication and security features

### Changed
- **BREAKING: Environment Variable Standardization**:
  - `LOXONE_USERNAME` → `LOXONE_USER` (across all components)
  - `LOXONE_PASSWORD` → `LOXONE_PASS` (across all components)  
  - `MCP_API_KEY` → `LOXONE_API_KEY` (namespace consistency)
  - Updated 277 tests and 15+ source files for complete consistency

- **Authentication System Consolidation**:
  - Consolidated duplicate credential structures into shared `credential_registry.rs`
  - Unified credential loading with clear precedence order
  - Removed duplicate StoredCredential/CredentialRegistry implementations from binaries

### Removed
- **Security Enhancement**: Complete keyring/keychain removal
  - Removed unmaintained keyring dependency (security risk)
  - Removed entire `src/auth/` module (8 files, 200+ lines)
  - Simplified credential storage to Environment and Infisical only
  - All clippy warnings resolved and code quality improved

### Fixed
- **Framework Compatibility**: Updated all pulseengine-mcp dependencies 0.4.0 → 0.5.0
- **API Updates**: Fixed breaking changes in framework structures
- **Code Quality**: Zero clippy warnings with strict settings
- **Testing**: All 270 tests pass with new standardized variables

### Migration Guide
Users upgrading from 0.5.0 must update environment variables:
```bash
# Old (0.5.0)
export LOXONE_USERNAME="admin"
export LOXONE_PASSWORD="secret"
export MCP_API_KEY="key123"

# New (0.6.0)  
export LOXONE_USER="admin"
export LOXONE_PASS="secret"
export LOXONE_API_KEY="key123"
```

## [0.5.0] - 2025-01-07

### Added
- **Credential ID System**: Complete UUID-based credential identification system
  - New `loxone-mcp-auth` binary with full CRUD operations (create, read, update, delete, test)
  - Credential registry with JSON metadata storage (ID, name, host, port, timestamps)
  - Integration with existing credential backends (keychain, Infisical, environment variables)
  - `--credential-id` parameter support in main server and setup tools
  - Comprehensive migration guide from environment variables to credential IDs

### Enhanced
- **Authentication Management**: 
  - Centralized credential metadata management with CredentialRegistry
  - Enhanced credential loading infrastructure in main.rs
  - Interactive and non-interactive credential management workflows
  - Connection testing and validation functionality
  
- **Developer Experience**:
  - Clear migration path from environment variables
  - Detailed error messages and user guidance
  - Updated documentation across all guides and security docs
  
### Removed
- **Legacy Binaries**: Cleaned up redundant tools
  - `test_connection.rs` - replaced by `loxone-mcp-auth test`
  - `verify_credentials.rs` - replaced by credential validation in auth binary
  - `update_host.rs` - replaced by `loxone-mcp-auth update`
  - `test_auth_framework.rs` - no longer needed after framework migration

### Security
- Enhanced credential security with UUID-based identification
- Backward compatibility maintained with environment variables
- Only metadata stored in registry, sensitive data remains in secure backends
- Comprehensive validation and error handling

## [0.4.0] - 2025-01-06

### Changed
- **BREAKING: Complete removal of custom authentication system** 
  - Removed entire `src/auth/` directory containing custom auth implementation
  - Removed `src/framework_integration/` directory with transition code
  - Fully migrated to PulseEngine MCP Framework v0.4.0 authentication
  - All MCP server authentication now handled by framework components
  
### Removed
- **Custom Authentication Components**:
  - `AuthenticationManager` (custom implementation)
  - `UnifiedAuth` compatibility layer
  - `LoxoneAuthConfig` framework integration
  - `loxone-mcp-auth` CLI binary
  - `test_auth_framework` test binary
  - Custom validation and security middleware
  - Admin API endpoints for custom auth management
  - All custom auth test files

### Added
- **New Framework Backend**: `LoxoneFrameworkBackend` in `src/server/framework_backend.rs`
  - Simple, clean integration with PulseEngine MCP Framework
  - Replaces the removed `LoxoneBackend` from framework_integration
  
### Migration Guide
- Remove any references to `USE_CUSTOM_AUTH` environment variable
- Use PulseEngine MCP Framework v0.4.0 authentication directly
- Replace `LoxoneBackend::initialize()` with `LoxoneFrameworkBackend::initialize()`
- Authentication is now configured through framework's `AuthConfig`

### Note
- Loxone device authentication (in `src/client/auth.rs`) remains unchanged
- Only MCP server authentication has been migrated to framework

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