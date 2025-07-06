//! Tool context builder for efficient context creation
//!
//! This module provides a builder pattern for creating ToolContext instances
//! efficiently, reducing the need for repeated cloning of Arc references.

use crate::{
    client::{ClientContext, LoxoneClient},
    services::{StateManager, UnifiedValueResolver},
    tools::ToolContext,
};
use std::sync::Arc;

/// Builder for creating ToolContext instances efficiently
///
/// This builder holds Arc references to commonly used services and can create
/// multiple ToolContext instances without repeated cloning. This is particularly
/// useful in handler methods that create many tool contexts.
#[derive(Clone)]
pub struct ToolContextBuilder {
    /// Cached Loxone client reference
    client: Arc<dyn LoxoneClient>,

    /// Cached client context reference
    context: Arc<ClientContext>,

    /// Cached value resolver reference
    value_resolver: Arc<UnifiedValueResolver>,

    /// Cached state manager reference (optional)
    state_manager: Option<Arc<StateManager>>,
}

impl ToolContextBuilder {
    /// Create a new builder with all required services
    pub fn new(
        client: Arc<dyn LoxoneClient>,
        context: Arc<ClientContext>,
        value_resolver: Arc<UnifiedValueResolver>,
        state_manager: Option<Arc<StateManager>>,
    ) -> Self {
        Self {
            client,
            context,
            value_resolver,
            state_manager,
        }
    }

    /// Build a new ToolContext instance
    ///
    /// This method clones the Arc references, which is cheap since Arc uses
    /// reference counting. The builder can be reused to create multiple contexts.
    pub fn build(&self) -> ToolContext {
        ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.clone(),
        )
    }

    /// Build a new ToolContext instance without state manager
    ///
    /// Some tools don't need the state manager, so this provides a way to
    /// create a context without it, even if the builder has one configured.
    pub fn build_without_state_manager(&self) -> ToolContext {
        ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            None,
        )
    }

    /// Update the client reference
    ///
    /// Useful if the client needs to be swapped out (e.g., for testing)
    pub fn with_client(mut self, client: Arc<dyn LoxoneClient>) -> Self {
        self.client = client;
        self
    }

    /// Update the context reference
    pub fn with_context(mut self, context: Arc<ClientContext>) -> Self {
        self.context = context;
        self
    }

    /// Update the value resolver reference
    pub fn with_value_resolver(mut self, value_resolver: Arc<UnifiedValueResolver>) -> Self {
        self.value_resolver = value_resolver;
        self
    }

    /// Update the state manager reference
    pub fn with_state_manager(mut self, state_manager: Option<Arc<StateManager>>) -> Self {
        self.state_manager = state_manager;
        self
    }

    /// Create a lightweight reference that can be passed around
    ///
    /// This is useful when you need to pass the builder to multiple functions
    /// without moving ownership. The returned reference can create contexts
    /// just like the original builder.
    pub fn as_ref(&self) -> ToolContextBuilderRef<'_> {
        ToolContextBuilderRef {
            client: &self.client,
            context: &self.context,
            value_resolver: &self.value_resolver,
            state_manager: self.state_manager.as_ref(),
        }
    }
}

/// A borrowed reference to a ToolContextBuilder
///
/// This allows passing around a reference to the builder without cloning it,
/// useful in scenarios where you need to create contexts in nested function calls.
#[derive(Clone, Copy)]
pub struct ToolContextBuilderRef<'a> {
    client: &'a Arc<dyn LoxoneClient>,
    context: &'a Arc<ClientContext>,
    value_resolver: &'a Arc<UnifiedValueResolver>,
    state_manager: Option<&'a Arc<StateManager>>,
}

impl<'a> ToolContextBuilderRef<'a> {
    /// Build a new ToolContext instance from the reference
    pub fn build(&self) -> ToolContext {
        ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            self.state_manager.cloned(),
        )
    }

    /// Build without state manager
    pub fn build_without_state_manager(&self) -> ToolContext {
        ToolContext::with_services(
            self.client.clone(),
            self.context.clone(),
            self.value_resolver.clone(),
            None,
        )
    }
}

/// Extension trait for LoxoneMcpServer to easily create a context builder
pub trait ServerContextBuilderExt {
    /// Create a ToolContextBuilder from server components
    fn create_tool_context_builder(&self) -> ToolContextBuilder;
}

#[cfg(test)]
mod tests {
    // Mock implementations would go here for testing

    #[test]
    fn test_builder_creates_identical_contexts() {
        // This would test that multiple calls to build() create equivalent contexts
    }

    #[test]
    fn test_builder_ref_works_correctly() {
        // This would test that the reference builder creates the same contexts
    }
}
