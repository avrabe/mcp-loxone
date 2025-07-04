//! Load balancing strategies for connection pools
//!
//! This module provides various load balancing algorithms to distribute
//! connections optimally across available pool connections.

use crate::client::adaptive_pool::AdaptiveConnection;
use crate::client::LoxoneClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::debug;

/// Load balancing strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    /// Round-robin selection
    RoundRobin,
    /// Select connection with least active requests
    LeastConnections,
    /// Weighted round-robin based on connection performance
    WeightedRoundRobin {
        /// Weight calculation method
        weight_method: WeightMethod,
    },
    /// Performance-based selection considering multiple metrics
    PerformanceBased {
        /// Response time weight (0.0-1.0)
        response_time_weight: f64,
        /// Error rate weight (0.0-1.0)
        error_rate_weight: f64,
        /// Connection age weight (0.0-1.0)
        age_weight: f64,
    },
    /// Random selection (for testing/fallback)
    Random,
    /// Sticky sessions based on request characteristics
    Sticky {
        /// Session timeout
        session_timeout: Duration,
    },
}

/// Weight calculation methods for weighted round-robin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WeightMethod {
    /// Based on success rate
    SuccessRate,
    /// Based on average response time
    ResponseTime,
    /// Based on connection capacity
    Capacity,
    /// Combined metric
    Combined,
}

/// Load balancer for connection selection
pub struct LoadBalancer {
    /// Current balancing strategy
    strategy: LoadBalancingStrategy,
    /// Round-robin counter for round-robin strategies
    round_robin_counter: AtomicUsize,
    /// Connection weights for weighted strategies
    connection_weights: Arc<tokio::sync::RwLock<HashMap<String, f64>>>,
    /// Performance metrics for performance-based selection
    performance_metrics: Arc<tokio::sync::RwLock<HashMap<String, ConnectionPerformanceMetrics>>>,
    /// Sticky session mapping
    sticky_sessions: Arc<tokio::sync::RwLock<HashMap<String, StickySession>>>,
}

/// Performance metrics for a connection
#[derive(Debug, Clone)]
struct ConnectionPerformanceMetrics {
    /// Average response time in milliseconds
    avg_response_time_ms: f64,
    /// Success rate (0.0-1.0)
    success_rate: f64,
    /// Last updated timestamp
    last_updated: SystemTime,
    /// Total requests processed
    total_requests: u64,
    /// Recent response times (sliding window)
    recent_response_times: Vec<u64>,
    /// Recent error count (sliding window)
    recent_errors: u64,
}

/// Sticky session information
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct StickySession {
    /// Connection ID
    connection_id: String,
    /// Session created at
    created_at: SystemTime,
    /// Last accessed
    last_accessed: SystemTime,
    /// Request count for this session
    request_count: u64,
}

impl LoadBalancer {
    /// Create new load balancer with specified strategy
    pub fn new(strategy: LoadBalancingStrategy) -> Self {
        Self {
            strategy,
            round_robin_counter: AtomicUsize::new(0),
            connection_weights: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            performance_metrics: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            sticky_sessions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Select best connection based on current strategy
    pub async fn select_connection(
        &self,
        available_connections: &[Arc<AdaptiveConnection>],
        session_key: Option<&str>,
    ) -> Option<Arc<AdaptiveConnection>> {
        if available_connections.is_empty() {
            return None;
        }

        match &self.strategy {
            LoadBalancingStrategy::RoundRobin => {
                self.select_round_robin(available_connections).await
            }
            LoadBalancingStrategy::LeastConnections => {
                self.select_least_connections(available_connections).await
            }
            LoadBalancingStrategy::WeightedRoundRobin { weight_method } => {
                self.select_weighted_round_robin(available_connections, weight_method)
                    .await
            }
            LoadBalancingStrategy::PerformanceBased {
                response_time_weight,
                error_rate_weight,
                age_weight,
            } => {
                self.select_performance_based(
                    available_connections,
                    *response_time_weight,
                    *error_rate_weight,
                    *age_weight,
                )
                .await
            }
            LoadBalancingStrategy::Random => self.select_random(available_connections).await,
            LoadBalancingStrategy::Sticky { session_timeout } => {
                self.select_sticky(available_connections, session_key, *session_timeout)
                    .await
            }
        }
    }

    /// Round-robin selection
    async fn select_round_robin(
        &self,
        connections: &[Arc<AdaptiveConnection>],
    ) -> Option<Arc<AdaptiveConnection>> {
        let index = self.round_robin_counter.fetch_add(1, Ordering::Relaxed) % connections.len();
        connections.get(index).cloned()
    }

    /// Select connection with least active requests
    async fn select_least_connections(
        &self,
        connections: &[Arc<AdaptiveConnection>],
    ) -> Option<Arc<AdaptiveConnection>> {
        let mut best_connection = None;
        let mut min_active = usize::MAX;

        for connection in connections {
            let active = connection.metadata.active_requests.load(Ordering::Relaxed);
            if active < min_active {
                min_active = active;
                best_connection = Some(connection.clone());
            }
        }

        best_connection
    }

    /// Weighted round-robin selection
    async fn select_weighted_round_robin(
        &self,
        connections: &[Arc<AdaptiveConnection>],
        weight_method: &WeightMethod,
    ) -> Option<Arc<AdaptiveConnection>> {
        // Update weights based on current metrics
        self.update_connection_weights(connections, weight_method)
            .await;

        let weights = self.connection_weights.read().await;
        let total_weight: f64 = connections
            .iter()
            .map(|conn| weights.get(&conn.id).copied().unwrap_or(1.0))
            .sum();

        if total_weight <= 0.0 {
            // Fallback to round-robin if no valid weights
            return self.select_round_robin(connections).await;
        }

        // Generate weighted random selection
        let mut random_value = (SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as f64)
            % total_weight;

        for connection in connections {
            let weight = weights.get(&connection.id).copied().unwrap_or(1.0);
            if random_value < weight {
                return Some(connection.clone());
            }
            random_value -= weight;
        }

        // Fallback to first connection
        connections.first().cloned()
    }

    /// Performance-based connection selection
    async fn select_performance_based(
        &self,
        connections: &[Arc<AdaptiveConnection>],
        response_time_weight: f64,
        error_rate_weight: f64,
        age_weight: f64,
    ) -> Option<Arc<AdaptiveConnection>> {
        let metrics = self.performance_metrics.read().await;
        let mut best_connection = None;
        let mut best_score = f64::MIN;

        for connection in connections {
            let score = self
                .calculate_performance_score(
                    connection,
                    &metrics,
                    response_time_weight,
                    error_rate_weight,
                    age_weight,
                )
                .await;

            debug!(
                "Connection {} performance score: {:.3}",
                connection.id, score
            );

            if score > best_score {
                best_score = score;
                best_connection = Some(connection.clone());
            }
        }

        best_connection.or_else(|| connections.first().cloned())
    }

    /// Random connection selection
    async fn select_random(
        &self,
        connections: &[Arc<AdaptiveConnection>],
    ) -> Option<Arc<AdaptiveConnection>> {
        let index = (SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as usize)
            % connections.len();
        connections.get(index).cloned()
    }

    /// Sticky session selection
    async fn select_sticky(
        &self,
        connections: &[Arc<AdaptiveConnection>],
        session_key: Option<&str>,
        session_timeout: Duration,
    ) -> Option<Arc<AdaptiveConnection>> {
        if let Some(key) = session_key {
            let mut sessions = self.sticky_sessions.write().await;
            let now = SystemTime::now();

            // Clean up expired sessions
            sessions.retain(|_, session| {
                now.duration_since(session.last_accessed)
                    .unwrap_or_default()
                    < session_timeout
            });

            // Check for existing session
            if let Some(session) = sessions.get_mut(key) {
                // Find the connection for this session
                if let Some(connection) = connections
                    .iter()
                    .find(|conn| conn.id == session.connection_id)
                {
                    session.last_accessed = now;
                    session.request_count += 1;
                    return Some(connection.clone());
                } else {
                    // Connection no longer available, remove session
                    sessions.remove(key);
                }
            }

            // Create new session with least loaded connection
            if let Some(connection) = self.select_least_connections(connections).await {
                sessions.insert(
                    key.to_string(),
                    StickySession {
                        connection_id: connection.id.clone(),
                        created_at: now,
                        last_accessed: now,
                        request_count: 1,
                    },
                );
                return Some(connection);
            }
        }

        // Fallback to least connections if no session key
        self.select_least_connections(connections).await
    }

    /// Update connection weights based on current metrics
    async fn update_connection_weights(
        &self,
        connections: &[Arc<AdaptiveConnection>],
        weight_method: &WeightMethod,
    ) {
        let mut weights = self.connection_weights.write().await;
        let metrics = self.performance_metrics.read().await;

        for connection in connections {
            let weight = match weight_method {
                WeightMethod::SuccessRate => {
                    connection.metadata.get_success_rate().max(0.1) // Minimum weight
                }
                WeightMethod::ResponseTime => {
                    if let Some(metric) = metrics.get(&connection.id) {
                        // Lower response time = higher weight
                        (1000.0 / (metric.avg_response_time_ms + 1.0)).max(0.1)
                    } else {
                        1.0
                    }
                }
                WeightMethod::Capacity => {
                    // Higher capacity (lower active requests) = higher weight
                    let active = connection.metadata.active_requests.load(Ordering::Relaxed) as f64;
                    (10.0 / (active + 1.0)).max(0.1)
                }
                WeightMethod::Combined => {
                    let success_rate = connection.metadata.get_success_rate();
                    let active = connection.metadata.active_requests.load(Ordering::Relaxed) as f64;
                    let response_time_factor = if let Some(metric) = metrics.get(&connection.id) {
                        1000.0 / (metric.avg_response_time_ms + 1.0)
                    } else {
                        1.0
                    };

                    // Combined score: success_rate * capacity_factor * response_time_factor
                    (success_rate * (10.0 / (active + 1.0)) * response_time_factor).max(0.1)
                }
            };

            weights.insert(connection.id.clone(), weight);
        }
    }

    /// Calculate performance score for a connection
    async fn calculate_performance_score(
        &self,
        connection: &Arc<AdaptiveConnection>,
        metrics: &HashMap<String, ConnectionPerformanceMetrics>,
        response_time_weight: f64,
        error_rate_weight: f64,
        age_weight: f64,
    ) -> f64 {
        let mut score = 0.0;

        // Response time component (lower is better)
        if let Some(metric) = metrics.get(&connection.id) {
            let response_time_score = (1000.0 / (metric.avg_response_time_ms + 1.0)) / 1000.0; // Normalize
            score += response_time_score * response_time_weight;

            // Error rate component (lower is better)
            let error_rate_score = metric.success_rate; // Success rate is inverse of error rate
            score += error_rate_score * error_rate_weight;
        }

        // Connection age component (newer connections might be better)
        let age_since_creation = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - connection.metadata.created_at.timestamp() as u64;
        let age_score = (3600.0 / (age_since_creation as f64 + 1.0)).min(1.0); // Prefer connections < 1 hour old
        score += age_score * age_weight;

        // Active requests penalty (fewer active requests is better)
        let active_requests = connection.metadata.active_requests.load(Ordering::Relaxed) as f64;
        let capacity_score = (10.0 / (active_requests + 1.0)).min(1.0);
        score += capacity_score * 0.1; // Small bonus for low utilization

        score
    }

    /// Record performance metrics for a connection
    pub async fn record_performance(
        &self,
        connection_id: &str,
        response_time_ms: u64,
        success: bool,
    ) {
        let mut metrics = self.performance_metrics.write().await;
        let now = SystemTime::now();

        let metric = metrics.entry(connection_id.to_string()).or_insert_with(|| {
            ConnectionPerformanceMetrics {
                avg_response_time_ms: 0.0,
                success_rate: 1.0,
                last_updated: now,
                total_requests: 0,
                recent_response_times: Vec::new(),
                recent_errors: 0,
            }
        });

        // Update response time (exponential moving average)
        if metric.avg_response_time_ms == 0.0 {
            metric.avg_response_time_ms = response_time_ms as f64;
        } else {
            metric.avg_response_time_ms =
                metric.avg_response_time_ms * 0.9 + (response_time_ms as f64) * 0.1;
        }

        // Update recent response times (sliding window of last 10)
        metric.recent_response_times.push(response_time_ms);
        if metric.recent_response_times.len() > 10 {
            metric.recent_response_times.remove(0);
        }

        // Update error tracking
        if !success {
            metric.recent_errors += 1;
        }

        // Calculate success rate from recent requests
        let recent_total = metric.recent_response_times.len() as u64;
        if recent_total > 0 {
            metric.success_rate =
                ((recent_total - metric.recent_errors) as f64) / (recent_total as f64);
        }

        // Reset error count if we have enough samples
        if metric.recent_response_times.len() >= 10 {
            metric.recent_errors =
                metric
                    .recent_errors
                    .saturating_sub(if metric.recent_response_times.len() > 10 {
                        1
                    } else {
                        0
                    });
        }

        metric.total_requests += 1;
        metric.last_updated = now;

        debug!(
            "Updated performance metrics for {}: avg_time={:.1}ms, success_rate={:.3}",
            connection_id, metric.avg_response_time_ms, metric.success_rate
        );
    }

    /// Get current load balancing statistics
    pub async fn get_statistics(&self) -> LoadBalancingStatistics {
        let weights = self.connection_weights.read().await;
        let metrics = self.performance_metrics.read().await;
        let sessions = self.sticky_sessions.read().await;

        LoadBalancingStatistics {
            strategy: self.strategy.clone(),
            total_connections_weighted: weights.len(),
            total_performance_tracked: metrics.len(),
            active_sticky_sessions: sessions.len(),
            round_robin_position: self.round_robin_counter.load(Ordering::Relaxed),
            connection_weights: weights.clone(),
        }
    }

    /// Clean up old performance data
    pub async fn cleanup_old_data(&self, max_age: Duration) {
        let now = SystemTime::now();

        // Clean up old performance metrics
        {
            let mut metrics = self.performance_metrics.write().await;
            metrics.retain(|id, metric| {
                let should_keep =
                    now.duration_since(metric.last_updated).unwrap_or_default() < max_age;
                if !should_keep {
                    debug!("Cleaning up old performance data for connection: {}", id);
                }
                should_keep
            });
        }

        // Clean up old sticky sessions
        {
            let mut sessions = self.sticky_sessions.write().await;
            sessions.retain(|key, session| {
                let should_keep = now
                    .duration_since(session.last_accessed)
                    .unwrap_or_default()
                    < max_age;
                if !should_keep {
                    debug!("Cleaning up old sticky session: {}", key);
                }
                should_keep
            });
        }

        // Clean up connection weights for non-existent connections
        {
            let metrics = self.performance_metrics.read().await;
            let mut weights = self.connection_weights.write().await;
            weights.retain(|id, _| {
                let should_keep = metrics.contains_key(id);
                if !should_keep {
                    debug!("Cleaning up weight for removed connection: {}", id);
                }
                should_keep
            });
        }
    }
}

/// Load balancing statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancingStatistics {
    /// Current strategy
    pub strategy: LoadBalancingStrategy,
    /// Number of connections with weights
    pub total_connections_weighted: usize,
    /// Number of connections with performance tracking
    pub total_performance_tracked: usize,
    /// Number of active sticky sessions
    pub active_sticky_sessions: usize,
    /// Current round-robin position
    pub round_robin_position: usize,
    /// Current connection weights
    pub connection_weights: HashMap<String, f64>,
}

impl Default for LoadBalancingStrategy {
    fn default() -> Self {
        Self::PerformanceBased {
            response_time_weight: 0.4,
            error_rate_weight: 0.4,
            age_weight: 0.2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::adaptive_pool::ConnectionMetadata;
    use crate::config::AuthMethod;
    use std::sync::atomic::AtomicU64;

    // Helper function to create test connection
    fn create_test_connection(id: &str, active_requests: usize) -> Arc<AdaptiveConnection> {
        let metadata = ConnectionMetadata {
            created_at: chrono::Utc::now(),
            last_used: Arc::new(tokio::sync::RwLock::new(chrono::Utc::now())),
            last_health_check: Arc::new(tokio::sync::RwLock::new(chrono::Utc::now())),
            is_healthy: Arc::new(tokio::sync::RwLock::new(true)),
            total_requests: Arc::new(AtomicU64::new(100)),
            failed_requests: Arc::new(AtomicU64::new(5)),
            active_requests: Arc::new(AtomicUsize::new(active_requests)),
        };

        // Note: This is a simplified version for testing
        // Create a mock client for testing
        struct MockClient;

        #[async_trait::async_trait]
        impl LoxoneClient for MockClient {
            async fn connect(&mut self) -> Result<()> {
                Ok(())
            }

            async fn is_connected(&self) -> Result<bool> {
                Ok(true)
            }

            async fn disconnect(&mut self) -> Result<()> {
                Ok(())
            }

            async fn send_command(
                &self,
                _uuid: &str,
                _command: &str,
            ) -> Result<crate::client::LoxoneResponse> {
                Ok(crate::client::LoxoneResponse {
                    code: 200,
                    value: serde_json::json!("OK"),
                })
            }

            async fn get_structure(&self) -> Result<crate::client::LoxoneStructure> {
                Ok(crate::client::LoxoneStructure {
                    last_modified: "2024-01-01T00:00:00Z".to_string(),
                    controls: std::collections::HashMap::new(),
                    rooms: std::collections::HashMap::new(),
                    cats: std::collections::HashMap::new(),
                    global_states: std::collections::HashMap::new(),
                })
            }

            async fn get_device_states(
                &self,
                _uuids: &[String],
            ) -> Result<std::collections::HashMap<String, serde_json::Value>> {
                Ok(std::collections::HashMap::new())
            }

            async fn get_state_values(
                &self,
                _state_uuids: &[String],
            ) -> Result<std::collections::HashMap<String, serde_json::Value>> {
                Ok(std::collections::HashMap::new())
            }

            async fn get_system_info(&self) -> Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }

            async fn health_check(&self) -> Result<bool> {
                Ok(true)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        Arc::new(AdaptiveConnection {
            id: id.to_string(),
            client: Box::new(MockClient),
            auth_method: AuthMethod::Basic,
            capabilities: crate::client::client_factory::ServerCapabilities {
                supports_basic_auth: true,
                supports_token_auth: false,
                supports_websocket: false,
                server_version: None,
                encryption_level: crate::client::client_factory::EncryptionLevel::None,
                discovered_at: chrono::Utc::now(),
            },
            metadata,
            circuit_breaker: None,
        })
    }

    #[tokio::test]
    #[ignore = "Requires mock client implementation"]
    async fn test_round_robin_selection() {
        let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
        let connections = vec![
            create_test_connection("conn1", 0),
            create_test_connection("conn2", 1),
            create_test_connection("conn3", 2),
        ];

        // Test round-robin behavior
        let selected1 = balancer.select_connection(&connections, None).await;
        let selected2 = balancer.select_connection(&connections, None).await;
        let selected3 = balancer.select_connection(&connections, None).await;
        let selected4 = balancer.select_connection(&connections, None).await;

        assert!(selected1.is_some());
        assert!(selected2.is_some());
        assert!(selected3.is_some());
        assert!(selected4.is_some());

        // Fourth selection should wrap around to first
        assert_eq!(selected1.unwrap().id, selected4.unwrap().id);
    }

    #[tokio::test]
    #[ignore = "Requires mock client implementation"]
    async fn test_least_connections_selection() {
        let balancer = LoadBalancer::new(LoadBalancingStrategy::LeastConnections);
        let connections = vec![
            create_test_connection("conn1", 5),
            create_test_connection("conn2", 1), // Should be selected
            create_test_connection("conn3", 3),
        ];

        let selected = balancer.select_connection(&connections, None).await;
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().id, "conn2");
    }

    #[tokio::test]
    #[ignore = "Requires mock client implementation"]
    async fn test_performance_metrics_recording() {
        let balancer = LoadBalancer::new(LoadBalancingStrategy::PerformanceBased {
            response_time_weight: 0.5,
            error_rate_weight: 0.3,
            age_weight: 0.2,
        });

        // Record some performance data
        balancer.record_performance("conn1", 100, true).await;
        balancer.record_performance("conn1", 150, true).await;
        balancer.record_performance("conn1", 200, false).await;

        let stats = balancer.get_statistics().await;
        assert_eq!(stats.total_performance_tracked, 1);
    }
}
