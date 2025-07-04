//! Circuit breaker pattern implementation for resilient error recovery
//!
//! This module provides a production-ready circuit breaker that prevents
//! cascading failures by temporarily disabling operations that are likely
//! to fail, allowing systems to recover gracefully.

use crate::error::LoxoneError;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Circuit is closed - normal operation
    Closed,
    /// Circuit is open - operations blocked
    Open,
    /// Circuit is half-open - testing if service recovered
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Failure threshold before opening circuit
    pub failure_threshold: u32,
    /// Success threshold to close circuit from half-open
    pub success_threshold: u32,
    /// Time window for failure counting
    pub failure_window: Duration,
    /// Timeout duration when circuit is open
    pub timeout_duration: Duration,
    /// Maximum timeout duration (exponential backoff)
    pub max_timeout_duration: Duration,
    /// Enable exponential backoff
    pub exponential_backoff: bool,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Error types that trigger the circuit breaker
    pub tracked_errors: Vec<String>,
    /// Enable detailed logging
    pub detailed_logging: bool,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            failure_window: Duration::minutes(1),
            timeout_duration: Duration::seconds(30),
            max_timeout_duration: Duration::minutes(5),
            exponential_backoff: true,
            backoff_multiplier: 2.0,
            tracked_errors: vec![
                "connection".to_string(),
                "timeout".to_string(),
                "service_unavailable".to_string(),
            ],
            detailed_logging: true,
        }
    }
}

impl CircuitBreakerConfig {
    /// Create configuration for critical services
    pub fn critical_service() -> Self {
        Self {
            failure_threshold: 3,
            success_threshold: 5,
            failure_window: Duration::seconds(30),
            timeout_duration: Duration::seconds(10),
            max_timeout_duration: Duration::minutes(2),
            exponential_backoff: false,
            backoff_multiplier: 1.5,
            tracked_errors: vec!["connection".to_string(), "timeout".to_string()],
            detailed_logging: true,
        }
    }

    /// Create configuration for non-critical services
    pub fn non_critical_service() -> Self {
        Self {
            failure_threshold: 10,
            success_threshold: 2,
            failure_window: Duration::minutes(5),
            timeout_duration: Duration::minutes(1),
            max_timeout_duration: Duration::minutes(10),
            exponential_backoff: true,
            backoff_multiplier: 3.0,
            tracked_errors: vec![
                "connection".to_string(),
                "timeout".to_string(),
                "service_unavailable".to_string(),
                "rate_limit".to_string(),
            ],
            detailed_logging: false,
        }
    }
}

/// Circuit breaker statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStats {
    /// Current state
    pub state: CircuitState,
    /// Total requests
    pub total_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Blocked requests
    pub blocked_requests: u64,
    /// Last failure time
    pub last_failure: Option<DateTime<Utc>>,
    /// Last success time
    pub last_success: Option<DateTime<Utc>>,
    /// Circuit open count
    pub circuit_open_count: u64,
    /// Current timeout duration
    pub current_timeout: Duration,
    /// Time until circuit can transition
    pub time_until_transition: Option<Duration>,
}

/// Circuit breaker event
#[derive(Debug, Clone, Serialize)]
pub struct CircuitBreakerEvent {
    /// Event type
    pub event_type: CircuitBreakerEventType,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Previous state
    pub previous_state: CircuitState,
    /// New state
    pub new_state: CircuitState,
    /// Additional context
    pub context: String,
}

/// Circuit breaker event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CircuitBreakerEventType {
    /// State changed
    StateChanged,
    /// Request allowed
    RequestAllowed,
    /// Request blocked
    RequestBlocked,
    /// Failure recorded
    FailureRecorded,
    /// Success recorded
    SuccessRecorded,
    /// Timeout adjusted
    TimeoutAdjusted,
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitBreakerState>>,
    event_listeners: Arc<RwLock<Vec<Arc<dyn CircuitBreakerListener + Send + Sync>>>>,
}

/// Internal circuit breaker state
struct CircuitBreakerState {
    current_state: CircuitState,
    failure_count: u32,
    success_count: u32,
    recent_failures: VecDeque<DateTime<Utc>>,
    last_state_change: DateTime<Utc>,
    current_timeout: Duration,
    consecutive_timeouts: u32,
    stats: CircuitBreakerStats,
}

/// Circuit breaker listener trait
#[async_trait::async_trait]
pub trait CircuitBreakerListener: Send + Sync {
    /// Called when circuit breaker event occurs
    async fn on_event(&self, event: &CircuitBreakerEvent);
}

impl CircuitBreaker {
    /// Create new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        let initial_state = CircuitBreakerState {
            current_state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            recent_failures: VecDeque::new(),
            last_state_change: Utc::now(),
            current_timeout: config.timeout_duration,
            consecutive_timeouts: 0,
            stats: CircuitBreakerStats {
                state: CircuitState::Closed,
                total_requests: 0,
                failed_requests: 0,
                successful_requests: 0,
                blocked_requests: 0,
                last_failure: None,
                last_success: None,
                circuit_open_count: 0,
                current_timeout: config.timeout_duration,
                time_until_transition: None,
            },
        };

        Self {
            config,
            state: Arc::new(RwLock::new(initial_state)),
            event_listeners: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Check if request should be allowed
    pub async fn should_allow_request(&self) -> bool {
        let mut state = self.state.write().await;
        state.stats.total_requests += 1;

        match state.current_state {
            CircuitState::Closed => {
                if self.config.detailed_logging {
                    debug!("Circuit breaker closed, allowing request");
                }
                self.emit_event(
                    CircuitBreakerEventType::RequestAllowed,
                    state.current_state,
                    state.current_state,
                    "Request allowed in closed state".to_string(),
                )
                .await;
                true
            }
            CircuitState::Open => {
                let elapsed = Utc::now() - state.last_state_change;
                if elapsed >= state.current_timeout {
                    // Transition to half-open
                    self.transition_state(&mut state, CircuitState::HalfOpen)
                        .await;
                    if self.config.detailed_logging {
                        info!("Circuit breaker transitioning to half-open");
                    }
                    true
                } else {
                    state.stats.blocked_requests += 1;
                    if self.config.detailed_logging {
                        debug!("Circuit breaker open, blocking request");
                    }
                    self.emit_event(
                        CircuitBreakerEventType::RequestBlocked,
                        state.current_state,
                        state.current_state,
                        format!(
                            "Request blocked, circuit open for {:?}",
                            state.current_timeout - elapsed
                        ),
                    )
                    .await;
                    false
                }
            }
            CircuitState::HalfOpen => {
                if self.config.detailed_logging {
                    debug!("Circuit breaker half-open, allowing test request");
                }
                self.emit_event(
                    CircuitBreakerEventType::RequestAllowed,
                    state.current_state,
                    state.current_state,
                    "Test request allowed in half-open state".to_string(),
                )
                .await;
                true
            }
        }
    }

    /// Record successful operation
    pub async fn record_success(&self) {
        let mut state = self.state.write().await;
        state.stats.successful_requests += 1;
        state.stats.last_success = Some(Utc::now());

        match state.current_state {
            CircuitState::Closed => {
                // Reset failure count on success
                state.failure_count = 0;
                state.recent_failures.clear();
            }
            CircuitState::HalfOpen => {
                state.success_count += 1;
                if state.success_count >= self.config.success_threshold {
                    self.transition_state(&mut state, CircuitState::Closed)
                        .await;
                    state.consecutive_timeouts = 0;
                    state.current_timeout = self.config.timeout_duration;
                    info!("Circuit breaker closed after successful recovery");
                }
            }
            CircuitState::Open => {
                warn!("Success recorded while circuit is open - this shouldn't happen");
            }
        }

        self.emit_event(
            CircuitBreakerEventType::SuccessRecorded,
            state.current_state,
            state.current_state,
            "Operation succeeded".to_string(),
        )
        .await;
    }

    /// Record failed operation
    pub async fn record_failure(&self, error: &LoxoneError) {
        let mut state = self.state.write().await;

        // Check if this error type should trigger the circuit breaker
        let error_type = self.get_error_type(error);
        if !self.config.tracked_errors.contains(&error_type) {
            debug!("Error type '{}' not tracked by circuit breaker", error_type);
            return;
        }

        state.stats.failed_requests += 1;
        state.stats.last_failure = Some(Utc::now());

        match state.current_state {
            CircuitState::Closed => {
                state.recent_failures.push_back(Utc::now());

                // Remove old failures outside the window
                let cutoff = Utc::now() - self.config.failure_window;
                while let Some(failure_time) = state.recent_failures.front() {
                    if *failure_time < cutoff {
                        state.recent_failures.pop_front();
                    } else {
                        break;
                    }
                }

                state.failure_count = state.recent_failures.len() as u32;

                if state.failure_count >= self.config.failure_threshold {
                    self.transition_state(&mut state, CircuitState::Open).await;
                    state.stats.circuit_open_count += 1;
                    error!(
                        "Circuit breaker opened after {} failures",
                        state.failure_count
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Single failure in half-open state reopens the circuit
                self.transition_state(&mut state, CircuitState::Open).await;

                // Apply exponential backoff if enabled
                if self.config.exponential_backoff {
                    state.consecutive_timeouts += 1;
                    let new_timeout_ms = state.current_timeout.num_milliseconds() as f64
                        * self
                            .config
                            .backoff_multiplier
                            .powi(state.consecutive_timeouts as i32);
                    let new_timeout = Duration::milliseconds(new_timeout_ms as i64);

                    state.current_timeout = if new_timeout > self.config.max_timeout_duration {
                        self.config.max_timeout_duration
                    } else {
                        new_timeout
                    };

                    // Update stats with new timeout
                    state.stats.current_timeout = state.current_timeout;

                    warn!(
                        "Circuit breaker reopened with timeout {:?} (backoff x{})",
                        state.current_timeout, state.consecutive_timeouts
                    );

                    self.emit_event(
                        CircuitBreakerEventType::TimeoutAdjusted,
                        CircuitState::HalfOpen,
                        CircuitState::Open,
                        format!("Timeout adjusted to {:?}", state.current_timeout),
                    )
                    .await;
                } else {
                    warn!("Circuit breaker reopened");
                }
            }
            CircuitState::Open => {
                // Already open, just count the failure
                debug!("Failure recorded while circuit is open");
            }
        }

        self.emit_event(
            CircuitBreakerEventType::FailureRecorded,
            state.current_state,
            state.current_state,
            format!("Operation failed: {error_type}"),
        )
        .await;
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> CircuitBreakerStats {
        let state = self.state.read().await;
        let mut stats = state.stats.clone();

        // Update current timeout from state (important for exponential backoff)
        stats.current_timeout = state.current_timeout;

        // Calculate time until transition for open state
        if state.current_state == CircuitState::Open {
            let elapsed = Utc::now() - state.last_state_change;
            if elapsed < state.current_timeout {
                stats.time_until_transition = Some(state.current_timeout - elapsed);
            }
        }

        stats
    }

    /// Reset circuit breaker
    pub async fn reset(&self) {
        let mut state = self.state.write().await;

        state.current_state = CircuitState::Closed;
        state.failure_count = 0;
        state.success_count = 0;
        state.recent_failures.clear();
        state.last_state_change = Utc::now();
        state.current_timeout = self.config.timeout_duration;
        state.consecutive_timeouts = 0;

        info!("Circuit breaker reset to closed state");

        self.emit_event(
            CircuitBreakerEventType::StateChanged,
            state.current_state,
            CircuitState::Closed,
            "Circuit breaker manually reset".to_string(),
        )
        .await;
    }

    /// Add event listener
    pub async fn add_listener(&self, listener: Arc<dyn CircuitBreakerListener + Send + Sync>) {
        let mut listeners = self.event_listeners.write().await;
        listeners.push(listener);
    }

    /// Transition to new state
    async fn transition_state(&self, state: &mut CircuitBreakerState, new_state: CircuitState) {
        let old_state = state.current_state;
        state.current_state = new_state;
        state.last_state_change = Utc::now();
        state.success_count = 0;
        state.failure_count = 0;
        state.stats.state = new_state;

        self.emit_event(
            CircuitBreakerEventType::StateChanged,
            old_state,
            new_state,
            format!("State transition: {old_state:?} -> {new_state:?}"),
        )
        .await;
    }

    /// Emit circuit breaker event
    async fn emit_event(
        &self,
        event_type: CircuitBreakerEventType,
        previous_state: CircuitState,
        new_state: CircuitState,
        context: String,
    ) {
        let event = CircuitBreakerEvent {
            event_type,
            timestamp: Utc::now(),
            previous_state,
            new_state,
            context,
        };

        let listeners = self.event_listeners.read().await;
        for listener in listeners.iter() {
            listener.on_event(&event).await;
        }
    }

    /// Get error type for tracking
    fn get_error_type(&self, error: &LoxoneError) -> String {
        // Extract error type from LoxoneError
        let error_str = error.to_string().to_lowercase();
        match true {
            _ if error_str.contains("connection") => "connection".to_string(),
            _ if error_str.contains("timeout") => "timeout".to_string(),
            _ if error_str.contains("unavailable") => "service_unavailable".to_string(),
            _ if error_str.contains("rate") => "rate_limit".to_string(),
            _ => "unknown".to_string(),
        }
    }
}

/// Circuit breaker manager for multiple services
pub struct CircuitBreakerManager {
    breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
    default_config: CircuitBreakerConfig,
}

impl CircuitBreakerManager {
    /// Create new circuit breaker manager
    pub fn new(default_config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: Arc::new(RwLock::new(HashMap::new())),
            default_config,
        }
    }

    /// Get or create circuit breaker for service
    pub async fn get_breaker(&self, service_name: &str) -> Arc<CircuitBreaker> {
        let mut breakers = self.breakers.write().await;

        if let Some(breaker) = breakers.get(service_name) {
            return breaker.clone();
        }

        let breaker = Arc::new(CircuitBreaker::new(self.default_config.clone()));
        breakers.insert(service_name.to_string(), breaker.clone());

        info!("Created new circuit breaker for service: {}", service_name);
        breaker
    }

    /// Get all circuit breaker statistics
    pub async fn get_all_stats(&self) -> HashMap<String, CircuitBreakerStats> {
        let breakers = self.breakers.read().await;
        let mut stats = HashMap::new();

        for (name, breaker) in breakers.iter() {
            stats.insert(name.clone(), breaker.get_stats().await);
        }

        stats
    }

    /// Reset all circuit breakers
    pub async fn reset_all(&self) {
        let breakers = self.breakers.read().await;

        for (name, breaker) in breakers.iter() {
            breaker.reset().await;
            info!("Reset circuit breaker for service: {}", name);
        }
    }
}

/// Helper macro for circuit breaker protected operations
#[macro_export]
macro_rules! with_circuit_breaker {
    ($breaker:expr, $operation:expr) => {{
        if !$breaker.should_allow_request().await {
            return Err(LoxoneError::service_unavailable(
                "Circuit breaker is open - service temporarily unavailable",
            ));
        }

        match $operation.await {
            Ok(result) => {
                $breaker.record_success().await;
                Ok(result)
            }
            Err(error) => {
                $breaker.record_failure(&error).await;
                Err(error)
            }
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_basic() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            timeout_duration: Duration::milliseconds(100),
            failure_window: Duration::seconds(60), // Make sure window is long enough
            ..Default::default()
        };

        let breaker = CircuitBreaker::new(config);

        // Should start closed
        assert!(breaker.should_allow_request().await);

        // Record failures
        for _ in 0..3 {
            breaker
                .record_failure(&LoxoneError::connection("test error"))
                .await;
        }

        // Should now be open
        let stats = breaker.get_stats().await;
        assert_eq!(stats.state, CircuitState::Open);

        // Should now be open
        assert!(!breaker.should_allow_request().await);

        // Wait for timeout
        sleep(tokio::time::Duration::from_millis(150)).await;

        // Should be half-open
        assert!(breaker.should_allow_request().await);

        // Record success
        breaker.record_success().await;
        breaker.record_success().await;
        breaker.record_success().await;

        // Should be closed again
        let stats = breaker.get_stats().await;
        assert_eq!(stats.state, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_exponential_backoff() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout_duration: Duration::milliseconds(100),
            exponential_backoff: true,
            backoff_multiplier: 2.0,
            ..Default::default()
        };

        let breaker = CircuitBreaker::new(config);

        // Open the breaker
        breaker
            .record_failure(&LoxoneError::connection("test"))
            .await;
        breaker
            .record_failure(&LoxoneError::connection("test"))
            .await;

        // Wait and transition to half-open
        sleep(tokio::time::Duration::from_millis(150)).await;
        assert!(breaker.should_allow_request().await);

        // Fail again
        breaker
            .record_failure(&LoxoneError::connection("test"))
            .await;

        // Check that timeout has increased (should be 100ms * 2.0 = 200ms)
        let stats = breaker.get_stats().await;
        assert_eq!(
            stats.current_timeout,
            Duration::milliseconds(200),
            "Timeout should double from 100ms to 200ms with backoff multiplier 2.0"
        );
    }
}
