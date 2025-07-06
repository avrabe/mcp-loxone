# ToolContextBuilder Usage Guide

## Overview

The `ToolContextBuilder` is a builder pattern implementation designed to reduce excessive cloning of `Arc` references when creating multiple `ToolContext` instances in the Loxone MCP server handlers.

## Problem It Solves

Previously, every handler method that needed a `ToolContext` would manually clone all four Arc references:

```rust
// OLD WAY - Excessive cloning in every handler
let tool_context = ToolContext::with_services(
    self.client.clone(),         // Clone Arc
    self.context.clone(),        // Clone Arc
    self.value_resolver.clone(), // Clone Arc
    self.state_manager.clone(),  // Clone Arc
);
```

With 30+ handler methods, this resulted in 120+ Arc clones being created repeatedly.

## Solution: ToolContextBuilder

The `ToolContextBuilder` holds the Arc references and can efficiently create multiple `ToolContext` instances:

```rust
// NEW WAY - Create builder once
let builder = ToolContextBuilder::new(
    client,
    context,
    value_resolver,
    state_manager,
);

// Create contexts efficiently
let context1 = builder.build();
let context2 = builder.build();
let context3 = builder.build();
```

## Integration Patterns

### 1. Server-Level Builder (Recommended)

Add the builder as a field in your server struct:

```rust
pub struct LoxoneMcpServer {
    // ... other fields ...
    tool_context_builder: ToolContextBuilder,
}

impl LoxoneMcpServer {
    pub fn new(/* params */) -> Self {
        let builder = ToolContextBuilder::new(
            client.clone(),
            context.clone(),
            value_resolver.clone(),
            state_manager.clone(),
        );
        
        Self {
            // ... other fields ...
            tool_context_builder: builder,
        }
    }
    
    // Handler methods can now use the builder
    pub async fn control_lights(&self, /* params */) -> Result<Response> {
        let tool_context = self.tool_context_builder.build();
        // Use tool_context...
    }
}
```

### 2. Using ServerContextBuilderExt Trait

For types that implement the trait (like `LoxoneBackend`):

```rust
use loxone_mcp::tools::ServerContextBuilderExt;

let backend = LoxoneBackend::new(/* params */);
let builder = backend.create_tool_context_builder();

// Now use the builder
let context = builder.build();
```

### 3. Builder References for Nested Calls

When passing the builder to nested functions, use `as_ref()`:

```rust
async fn complex_operation(server: &LoxoneMcpServer) {
    let builder_ref = server.tool_context_builder.as_ref();
    
    process_devices(builder_ref).await;
    update_states(builder_ref).await;
}

async fn process_devices(builder: ToolContextBuilderRef<'_>) {
    let context = builder.build();
    // Process devices...
}
```

### 4. Conditional Context Creation

Build contexts with or without state manager:

```rust
// Full context with state manager
let full_context = builder.build();

// Lightweight context without state manager
let light_context = builder.build_without_state_manager();
```

## Performance Benefits

1. **Reduced Cloning**: Instead of cloning 4 Arcs per handler call, clone once when creating the builder
2. **Cache Locality**: The builder keeps Arc references together, improving cache performance
3. **Cleaner Code**: Less boilerplate in handler methods
4. **Flexibility**: Easy to modify what gets passed to contexts in one place

## Migration Guide

To migrate existing code:

1. **Add builder to server struct**:
   ```rust
   pub struct LoxoneMcpServer {
       // existing fields...
       tool_context_builder: ToolContextBuilder,
   }
   ```

2. **Initialize builder in constructor**:
   ```rust
   impl LoxoneMcpServer {
       pub fn new(/* params */) -> Self {
           let builder = ToolContextBuilder::new(/* services */);
           Self {
               // existing fields...
               tool_context_builder: builder,
           }
       }
   }
   ```

3. **Update handler methods**:
   ```rust
   // Before
   let tool_context = ToolContext::with_services(
       self.client.clone(),
       self.context.clone(),
       self.value_resolver.clone(),
       self.state_manager.clone(),
   );
   
   // After
   let tool_context = self.tool_context_builder.build();
   ```

## Advanced Usage

### Dynamic Builder Updates

The builder can be modified if services need to be swapped:

```rust
// Update specific service
let new_builder = builder
    .with_client(new_client)
    .with_state_manager(new_state_manager);
```

### Testing with Mock Services

Create test builders with mock services:

```rust
#[cfg(test)]
fn create_test_builder() -> ToolContextBuilder {
    ToolContextBuilder::new(
        Arc::new(MockLoxoneClient::new()),
        Arc::new(ClientContext::new()),
        Arc::new(UnifiedValueResolver::new()),
        None, // No state manager for tests
    )
}
```

## Best Practices

1. **Create builder once**: Initialize at server startup, not per request
2. **Use references**: Pass `ToolContextBuilderRef` to avoid cloning the builder itself
3. **Consider lifetime**: Builder should live as long as the server
4. **Thread safety**: The builder is `Clone` and thread-safe via Arc

## Example

See `examples/tool_context_builder_demo.rs` for a complete working example demonstrating various usage patterns.