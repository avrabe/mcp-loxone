//! Advanced error recovery patterns for production resilience
//!
//! This module provides sophisticated error recovery mechanisms including
//! circuit breakers, retry policies, and failover strategies.

pub mod circuit_breaker;
pub mod resilience_manager;
pub mod retry_policy;

// Re-export commonly used types
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerManager, CircuitBreakerStats, CircuitState,
};
pub use resilience_manager::{
    FallbackStrategy, ResilienceBuilder, ResilienceConfig, ResilienceManager, ResilienceStats,
};
pub use retry_policy::{
    BackoffStrategy, JitterType, RetryBuilder, RetryExecutor, RetryPolicy, RetryStats,
};
