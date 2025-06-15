# RMCP Migration Analysis and Strategy

This document provides a comprehensive inventory of rmcp dependencies and a migration strategy for replacing the rmcp crate with a custom MCP implementation.

## Executive Summary

The codebase has **moderate coupling** to rmcp with **22 distinct rmcp types** used across **4 core modules**. Most usage is concentrated in the server layer, making migration feasible but requiring careful planning. The main challenges are implementing the ServerHandler trait and MCP model types.

## RMCP Dependency Inventory

### 1. Core Dependencies

**Cargo.toml**:
```toml
rmcp = { version = "0.1.5", features = ["server", "macros"], optional = true }
```
- **Features used**: `server`, `macros`
- **Dependency type**: Optional (feature-gated for native builds)
- **Migration impact**: Can be cleanly removed

### 2. Import Analysis

**Active imports** (from `src/server/rmcp_impl.rs`):
```rust
use rmcp::{
    model::*,          // Extensive model types usage
    service::RequestContext,
    Error, RoleServer, ServerHandler,
};
```

**Infrastructure imports**:
```rust
use rmcp::ServiceExt;     // In server/mod.rs for .serve() method
use rmcp::ServerHandler;  // In http_transport.rs for trait usage
```

**Commented/disabled imports** (TODO items):
```rust
// use rmcp::tool;       // In multiple tool files (planned macro usage)
// pub use rmcp::{};     // In lib.rs (planned re-exports)
```

### 3. Types and Functionality Used

#### 3.1 Model Types (22 types)

**Request/Response Types**:
- `CallToolRequestParam` - Tool execution parameters
- `CallToolResult` - Tool execution results
- `Content` - Message content wrapper
- `PaginatedRequestParam` - Pagination parameters

**Server Capability Types**:
- `ServerInfo` - Server metadata
- `ServerCapabilities` - Feature flags (tools, resources, prompts)
- `ProtocolVersion` - MCP protocol version
- `Implementation` - Server implementation info

**Resource Management**:
- `ListResourcesResult` - Resource listing response
- `ReadResourceRequestParam` - Resource read parameters
- `ReadResourceResult` - Resource read response
- `Resource` - Resource metadata
- `RawResource` - Raw resource data
- `ResourceContents` - Resource content wrapper
- `Annotations` - Resource annotations

**Prompts System**:
- `ListPromptsResult` - Prompt listing response
- `GetPromptRequestParam` - Prompt request parameters
- `GetPromptResult` - Prompt response
- `Prompt` - Prompt definition
- `PromptArgument` - Prompt parameter definition
- `PromptMessage` - Prompt message
- `PromptMessageRole` - Message role (User/Assistant/System)
- `PromptMessageContent` - Message content

**Protocol Infrastructure**:
- `InitializeRequestParam` - Server initialization
- `InitializeResult` - Initialization response
- `CompleteRequestParam` - Completion request
- `CompleteResult` - Completion response
- `CompletionInfo` - Completion metadata
- `SetLevelRequestParam` - Logging level
- `ListResourceTemplatesResult` - Resource templates
- `SubscribeRequestParam` - Subscription management
- `UnsubscribeRequestParam` - Unsubscription

#### 3.2 Service Infrastructure

**Core Traits**:
- `ServerHandler` - Main MCP server trait (29 methods implemented)
- `ServiceExt` - Extension trait providing `.serve()` method

**Context Types**:
- `RequestContext<RoleServer>` - Request context for server role
- `RoleServer` - Type parameter for server role
- `Error` - rmcp error type

### 4. Implementation Analysis

#### 4.1 ServerHandler Implementation

**File**: `src/server/rmcp_impl.rs` (1,970 lines)

**Implemented Methods** (29 total):
1. `ping()` - Health check
2. `get_info()` - Server info
3. `list_tools()` - Tool enumeration
4. `call_tool()` - Tool execution (main functionality)
5. `list_resources()` - Resource enumeration
6. `read_resource()` - Resource reading
7. `list_prompts()` - Prompt enumeration
8. `get_prompt()` - Prompt generation
9. `initialize()` - Server initialization
10. `complete()` - Auto-completion
11. `set_level()` - Log level management
12. `list_resource_templates()` - Resource templates
13. `subscribe()` - Event subscription
14. `unsubscribe()` - Event unsubscription

**Plus 15 prompt-specific methods**:
- `get_cozy_prompt_messages()`
- `get_event_prompt_messages()`
- `get_energy_prompt_messages()`
- `get_morning_prompt_messages()`
- `get_night_prompt_messages()`
- `get_comfort_optimization_messages()`
- `get_seasonal_adjustment_messages()`
- `get_security_analysis_messages()`
- `get_troubleshooting_messages()`
- `get_custom_scene_messages()`

#### 4.2 Service Integration

**File**: `src/server/mod.rs`
```rust
let service = self
    .clone()
    .serve((stdin(), stdout()))  // ServiceExt::serve()
    .await?;

service.waiting().await?;  // Service lifecycle management
```

#### 4.3 HTTP Transport Integration

**File**: `src/http_transport.rs`
- Uses `ServerHandler` trait bound for MCP message handling
- Implements JSON-RPC over HTTP mapping to MCP protocol

## Migration Strategy

### Phase 1: Create Custom MCP Framework (Weeks 1-2)

**Goal**: Replace rmcp core with minimal custom implementation

**Tasks**:
1. **Create MCP model types** (`src/mcp/model.rs`):
   ```rust
   pub mod model {
       pub struct CallToolResult { /* ... */ }
       pub struct Content { /* ... */ }
       pub struct ServerInfo { /* ... */ }
       // ... 19 more types
   }
   ```

2. **Implement ServerHandler trait** (`src/mcp/server.rs`):
   ```rust
   #[async_trait]
   pub trait ServerHandler {
       async fn ping(&self, context: RequestContext) -> Result<(), Error>;
       async fn call_tool(&self, request: CallToolRequestParam, context: RequestContext) -> Result<CallToolResult, Error>;
       // ... 27 more methods
   }
   ```

3. **Create service infrastructure** (`src/mcp/service.rs`):
   ```rust
   pub trait ServiceExt {
       async fn serve<IO>(self, io: IO) -> Result<Service, Error>;
   }
   ```

**Migration effort**: **Medium** - Well-defined interfaces, mostly data structures

### Phase 2: Update Server Implementation (Week 3)

**Goal**: Migrate ServerHandler implementation to use custom types

**Files to update**:
- `src/server/rmcp_impl.rs` → `src/server/mcp_impl.rs`
- Update imports: `use rmcp::` → `use crate::mcp::`
- Update method signatures to use custom types

**Migration effort**: **Low** - Mostly find-and-replace imports

### Phase 3: Update Service Integration (Week 4)

**Goal**: Replace rmcp service integration

**Tasks**:
1. **Update server startup** (`src/server/mod.rs`):
   ```rust
   // Before:
   use rmcp::ServiceExt;
   let service = self.serve((stdin(), stdout())).await?;
   
   // After:
   use crate::mcp::ServiceExt;
   let service = self.serve((stdin(), stdout())).await?;
   ```

2. **Update HTTP transport** (`src/http_transport.rs`):
   ```rust
   // Before:
   use rmcp::ServerHandler;
   
   // After:
   use crate::mcp::ServerHandler;
   ```

**Migration effort**: **Low** - Minimal interface changes

### Phase 4: Enable Advanced Features (Week 5)

**Goal**: Re-enable commented rmcp features with custom implementation

**Tasks**:
1. **Implement tool macros** (currently commented as `// use rmcp::tool;`):
   ```rust
   // Create custom #[tool] attribute macro
   #[tool]
   pub async fn discover_all_devices(...) -> ToolResponse { ... }
   ```

2. **Add re-exports** (`src/lib.rs`):
   ```rust
   pub use crate::mcp::{ServerHandler, ServiceExt, model::*};
   ```

**Migration effort**: **Medium** - Macro implementation requires procedural macro development

## Complexity Assessment

### Easy to Replace (Effort: Low)

1. **Model types** - Pure data structures with straightforward serialization
2. **Server trait methods** - Well-defined interfaces with clear contracts
3. **Import updates** - Mechanical find-and-replace operations

### Moderate Complexity (Effort: Medium)

1. **Service infrastructure** - JSON-RPC over stdio, WebSocket support
2. **Error handling** - Custom error types and conversion
3. **Context management** - Request tracking and lifecycle

### Challenging to Replace (Effort: High)

1. **Tool macros** - Procedural macro for tool definition (currently disabled)
2. **Protocol compliance** - Full MCP specification adherence
3. **Transport layer** - stdio, HTTP, WebSocket transport implementations

## Migration Timeline

| Phase | Duration | Effort | Risk | Dependencies |
|-------|----------|--------|------|--------------|
| 1: Custom Framework | 2 weeks | Medium | Low | None |
| 2: Server Migration | 1 week | Low | Low | Phase 1 |
| 3: Service Integration | 1 week | Low | Low | Phase 2 |
| 4: Advanced Features | 1 week | Medium | Medium | Phase 3 |
| **Total** | **5 weeks** | **Medium** | **Low-Medium** | Sequential |

## Risk Analysis

### Low Risk Items
- **Model types**: Simple data structures
- **Handler implementation**: Clear interface contracts
- **Basic service**: Well-understood stdio transport

### Medium Risk Items
- **Tool macros**: Complex procedural macro development
- **Protocol compliance**: Ensuring full MCP spec adherence
- **HTTP transport**: Complex JSON-RPC mapping

### Mitigation Strategies

1. **Incremental migration**: Keep rmcp as fallback during transition
2. **Test coverage**: Comprehensive testing of custom implementations
3. **Protocol validation**: Use MCP Inspector for compliance testing
4. **Gradual rollout**: Feature-flag new implementations

## Recommended Approach

### Option 1: Full Custom Implementation (Recommended)
- **Pros**: Complete control, no external dependencies, optimized for use case
- **Cons**: Higher initial development effort
- **Timeline**: 5 weeks
- **Best for**: Long-term maintainability and customization

### Option 2: Hybrid Approach
- **Pros**: Lower initial effort, gradual migration
- **Cons**: Temporary complexity, dual implementations
- **Timeline**: 3 weeks minimum + ongoing maintenance
- **Best for**: Quick wins with future migration path

### Option 3: Fork rmcp
- **Pros**: Minimal initial changes, leverages existing code
- **Cons**: Long-term maintenance burden, external dependency
- **Timeline**: 1 week
- **Best for**: Short-term solution only

## Conclusion

The migration is **feasible and recommended**. The codebase has moderate but manageable coupling to rmcp, with most usage concentrated in well-defined server interfaces. A custom implementation would provide:

1. **Full control** over MCP protocol implementation
2. **Optimized performance** for Loxone-specific use cases
3. **Simplified dependencies** and reduced external risks
4. **Enhanced features** tailored to home automation needs

The **5-week timeline** is realistic and would result in a more maintainable, performant, and feature-rich implementation.