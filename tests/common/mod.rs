//! Common test utilities and mock infrastructure
//!
//! This module provides shared testing infrastructure including:
//! - Loxone API mocking with WireMock
//! - Test fixtures and utilities
//! - Environment isolation helpers
//! - Container-based testing support

pub mod loxone_mock;
pub mod test_fixtures;

pub use loxone_mock::*;
pub use test_fixtures::*;
