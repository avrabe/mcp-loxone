//! Common test utilities and mock infrastructure
//!
//! This module provides shared testing infrastructure including:
//! - Loxone API mocking with WireMock
//! - Test fixtures and utilities
//! - Environment isolation helpers
//! - Container-based testing support

pub mod containers;
pub mod loxone_mock;
pub mod test_fixtures;

// Re-export key types that are actually used
pub use loxone_mock::MockLoxoneServer;
pub use test_fixtures::{test_server_config, TestDeviceUuids};

// Export container types (testcontainers is now a dependency)
pub use containers::ContainerTestEnvironment;
