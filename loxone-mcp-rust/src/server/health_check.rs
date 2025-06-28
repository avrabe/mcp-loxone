//! Comprehensive health check system for Loxone MCP server
//!
//! This module provides detailed health monitoring beyond basic connectivity,
//! including system metrics, performance monitoring, and service health assessment.

use crate::client::LoxoneClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

/// Overall health status levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    /// All systems operational
    Healthy,
    /// Some issues but system is functional
    Degraded,
    /// Major issues affecting functionality
    Unhealthy,
    /// System is not responding
    Critical,
}

impl HealthStatus {
    /// Check if status indicates system is operational
    pub fn is_operational(&self) -> bool {
        matches!(self, HealthStatus::Healthy | HealthStatus::Degraded)
    }

    /// Get numeric score for status (higher is better)
    pub fn score(&self) -> u8 {
        match self {
            HealthStatus::Healthy => 100,
            HealthStatus::Degraded => 60,
            HealthStatus::Unhealthy => 30,
            HealthStatus::Critical => 0,
        }
    }
}

/// Individual health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// Name of the health check
    pub name: String,
    /// Overall status
    pub status: HealthStatus,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Detailed message
    pub message: String,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    /// Timestamp of the check
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl HealthCheckResult {
    pub fn new(name: String, status: HealthStatus, response_time_ms: u64, message: String) -> Self {
        Self {
            name,
            status,
            response_time_ms,
            message,
            metadata: std::collections::HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Add metadata to the health check result
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Comprehensive health check report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Overall system status
    pub overall_status: HealthStatus,
    /// Overall response time
    pub overall_response_time_ms: u64,
    /// Individual check results
    pub checks: Vec<HealthCheckResult>,
    /// System summary
    pub summary: HealthSummary,
    /// Timestamp of the report
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Health summary with key metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSummary {
    /// Total checks performed
    pub total_checks: usize,
    /// Number of healthy checks
    pub healthy_checks: usize,
    /// Number of degraded checks
    pub degraded_checks: usize,
    /// Number of unhealthy checks
    pub unhealthy_checks: usize,
    /// Number of critical checks
    pub critical_checks: usize,
    /// Average response time
    pub avg_response_time_ms: u64,
    /// Slowest check
    pub slowest_check: Option<String>,
    /// System uptime (if available)
    pub uptime_seconds: Option<u64>,
}

/// Configuration for health checks
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Timeout for individual checks
    pub check_timeout: Duration,
    /// Warning threshold for response times (ms)
    pub slow_response_threshold_ms: u64,
    /// Critical threshold for response times (ms)
    pub critical_response_threshold_ms: u64,
    /// Enable performance monitoring
    pub enable_performance_monitoring: bool,
    /// Enable device sampling for health checks
    pub enable_device_sampling: bool,
    /// Maximum number of devices to sample
    pub max_device_samples: usize,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_timeout: Duration::from_secs(10),
            slow_response_threshold_ms: 1000,
            critical_response_threshold_ms: 5000,
            enable_performance_monitoring: true,
            enable_device_sampling: true,
            max_device_samples: 5,
        }
    }
}

/// Enhanced health checker for Loxone systems
pub struct HealthChecker {
    client: Arc<dyn LoxoneClient>,
    config: HealthCheckConfig,
    start_time: Instant,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new(client: Arc<dyn LoxoneClient>, config: HealthCheckConfig) -> Self {
        Self {
            client,
            config,
            start_time: Instant::now(),
        }
    }

    /// Create with default configuration
    pub fn with_defaults(client: Arc<dyn LoxoneClient>) -> Self {
        Self::new(client, HealthCheckConfig::default())
    }

    /// Perform comprehensive health check
    pub async fn check_health(&self) -> Result<HealthReport> {
        let start_time = Instant::now();
        info!("🔍 Starting comprehensive health check");

        let mut checks = Vec::new();

        // 1. Basic connectivity check
        checks.push(self.check_connectivity().await);

        // 2. System information check
        checks.push(self.check_system_info().await);

        // 3. Structure availability check
        checks.push(self.check_structure().await);

        // 4. Performance monitoring (if enabled)
        if self.config.enable_performance_monitoring {
            checks.push(self.check_performance().await);
        }

        // 5. Device sampling (if enabled)
        if self.config.enable_device_sampling {
            checks.push(self.check_device_sampling().await);
        }

        // 6. Memory and resource usage
        checks.push(self.check_resources().await);

        // Calculate overall status and metrics
        let overall_response_time_ms = start_time.elapsed().as_millis() as u64;
        let overall_status = self.calculate_overall_status(&checks);
        let summary = self.generate_summary(&checks, overall_response_time_ms);

        let report = HealthReport {
            overall_status,
            overall_response_time_ms,
            checks,
            summary,
            timestamp: chrono::Utc::now(),
        };

        info!(
            "✅ Health check completed in {}ms - Status: {:?}",
            overall_response_time_ms, report.overall_status
        );

        Ok(report)
    }

    /// Check basic connectivity to Loxone system
    async fn check_connectivity(&self) -> HealthCheckResult {
        let start = Instant::now();
        let name = "connectivity".to_string();

        match tokio::time::timeout(self.config.check_timeout, self.client.health_check()).await {
            Ok(Ok(true)) => {
                let response_time = start.elapsed().as_millis() as u64;
                HealthCheckResult::new(
                    name,
                    if response_time > self.config.slow_response_threshold_ms {
                        HealthStatus::Degraded
                    } else {
                        HealthStatus::Healthy
                    },
                    response_time,
                    "Loxone system is reachable".to_string(),
                )
                .with_metadata("reachable".to_string(), serde_json::json!(true))
            }
            Ok(Ok(false)) => HealthCheckResult::new(
                name,
                HealthStatus::Unhealthy,
                start.elapsed().as_millis() as u64,
                "Loxone system is reachable but not fully functional".to_string(),
            ),
            Ok(Err(e)) => HealthCheckResult::new(
                name,
                HealthStatus::Critical,
                start.elapsed().as_millis() as u64,
                format!("Connection failed: {e}"),
            ),
            Err(_) => HealthCheckResult::new(
                name,
                HealthStatus::Critical,
                self.config.check_timeout.as_millis() as u64,
                "Connection timeout".to_string(),
            ),
        }
    }

    /// Check system information availability
    async fn check_system_info(&self) -> HealthCheckResult {
        let start = Instant::now();
        let name = "system_info".to_string();

        match tokio::time::timeout(self.config.check_timeout, self.client.get_system_info()).await {
            Ok(Ok(info)) => {
                let response_time = start.elapsed().as_millis() as u64;
                let mut result = HealthCheckResult::new(
                    name,
                    if response_time > self.config.slow_response_threshold_ms {
                        HealthStatus::Degraded
                    } else {
                        HealthStatus::Healthy
                    },
                    response_time,
                    "System information available".to_string(),
                );

                // Add system info metadata
                if let Some(version) = info.get("swVersion").and_then(|v| v.as_str()) {
                    result =
                        result.with_metadata("version".to_string(), serde_json::json!(version));
                }
                if let Some(serial) = info.get("serialNr").and_then(|v| v.as_str()) {
                    result = result.with_metadata("serial".to_string(), serde_json::json!(serial));
                }

                result
            }
            Ok(Err(e)) => HealthCheckResult::new(
                name,
                HealthStatus::Unhealthy,
                start.elapsed().as_millis() as u64,
                format!("Failed to get system info: {e}"),
            ),
            Err(_) => HealthCheckResult::new(
                name,
                HealthStatus::Critical,
                self.config.check_timeout.as_millis() as u64,
                "System info request timeout".to_string(),
            ),
        }
    }

    /// Check structure file availability and parsing
    async fn check_structure(&self) -> HealthCheckResult {
        let start = Instant::now();
        let name = "structure".to_string();

        match tokio::time::timeout(self.config.check_timeout, self.client.get_structure()).await {
            Ok(Ok(structure)) => {
                let response_time = start.elapsed().as_millis() as u64;
                HealthCheckResult::new(
                    name,
                    if response_time > self.config.slow_response_threshold_ms {
                        HealthStatus::Degraded
                    } else {
                        HealthStatus::Healthy
                    },
                    response_time,
                    "Structure file loaded successfully".to_string(),
                )
                .with_metadata(
                    "rooms_count".to_string(),
                    serde_json::json!(structure.rooms.len()),
                )
                .with_metadata(
                    "controls_count".to_string(),
                    serde_json::json!(structure.controls.len()),
                )
                .with_metadata(
                    "last_modified".to_string(),
                    serde_json::json!(structure.last_modified),
                )
            }
            Ok(Err(e)) => HealthCheckResult::new(
                name,
                HealthStatus::Unhealthy,
                start.elapsed().as_millis() as u64,
                format!("Failed to load structure: {e}"),
            ),
            Err(_) => HealthCheckResult::new(
                name,
                HealthStatus::Critical,
                self.config.check_timeout.as_millis() as u64,
                "Structure request timeout".to_string(),
            ),
        }
    }

    /// Check system performance metrics
    async fn check_performance(&self) -> HealthCheckResult {
        let start = Instant::now();
        let name = "performance".to_string();

        // Perform multiple small requests to test responsiveness
        let mut total_time = 0u64;
        let mut successful_requests = 0;
        let test_count = 3;

        for _ in 0..test_count {
            let request_start = Instant::now();
            match tokio::time::timeout(Duration::from_secs(2), self.client.get_system_info()).await
            {
                Ok(Ok(_)) => {
                    total_time += request_start.elapsed().as_millis() as u64;
                    successful_requests += 1;
                }
                _ => break,
            }
        }

        let response_time = start.elapsed().as_millis() as u64;
        let avg_request_time = if successful_requests > 0 {
            total_time / successful_requests as u64
        } else {
            response_time
        };

        let status = if successful_requests == test_count {
            if avg_request_time > self.config.critical_response_threshold_ms {
                HealthStatus::Unhealthy
            } else if avg_request_time > self.config.slow_response_threshold_ms {
                HealthStatus::Degraded
            } else {
                HealthStatus::Healthy
            }
        } else if successful_requests > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Critical
        };

        HealthCheckResult::new(
            name,
            status,
            response_time,
            format!(
                "Performance test: {successful_requests}/{test_count} requests successful, avg {avg_request_time}ms"
            ),
        )
        .with_metadata(
            "successful_requests".to_string(),
            serde_json::json!(successful_requests),
        )
        .with_metadata("total_requests".to_string(), serde_json::json!(test_count))
        .with_metadata(
            "avg_request_time_ms".to_string(),
            serde_json::json!(avg_request_time),
        )
    }

    /// Sample device states to check system responsiveness
    async fn check_device_sampling(&self) -> HealthCheckResult {
        let start = Instant::now();
        let name = "device_sampling".to_string();

        // Get structure first to find devices to sample
        match self.client.get_structure().await {
            Ok(structure) => {
                let devices: Vec<_> = structure
                    .controls
                    .keys()
                    .take(self.config.max_device_samples)
                    .cloned()
                    .collect();

                if devices.is_empty() {
                    return HealthCheckResult::new(
                        name,
                        HealthStatus::Degraded,
                        start.elapsed().as_millis() as u64,
                        "No devices found for sampling".to_string(),
                    );
                }

                match tokio::time::timeout(
                    self.config.check_timeout,
                    self.client.get_device_states(&devices),
                )
                .await
                {
                    Ok(Ok(states)) => {
                        let response_time = start.elapsed().as_millis() as u64;
                        HealthCheckResult::new(
                            name,
                            if response_time > self.config.slow_response_threshold_ms {
                                HealthStatus::Degraded
                            } else {
                                HealthStatus::Healthy
                            },
                            response_time,
                            format!("Successfully sampled {} device states", states.len()),
                        )
                        .with_metadata(
                            "devices_sampled".to_string(),
                            serde_json::json!(devices.len()),
                        )
                        .with_metadata(
                            "states_received".to_string(),
                            serde_json::json!(states.len()),
                        )
                    }
                    Ok(Err(e)) => HealthCheckResult::new(
                        name,
                        HealthStatus::Unhealthy,
                        start.elapsed().as_millis() as u64,
                        format!("Failed to sample device states: {e}"),
                    ),
                    Err(_) => HealthCheckResult::new(
                        name,
                        HealthStatus::Critical,
                        self.config.check_timeout.as_millis() as u64,
                        "Device sampling timeout".to_string(),
                    ),
                }
            }
            Err(e) => HealthCheckResult::new(
                name,
                HealthStatus::Unhealthy,
                start.elapsed().as_millis() as u64,
                format!("Failed to get structure for device sampling: {e}"),
            ),
        }
    }

    /// Check resource usage and memory
    async fn check_resources(&self) -> HealthCheckResult {
        let start = Instant::now();
        let name = "resources".to_string();

        // Basic resource monitoring
        let uptime_seconds = self.start_time.elapsed().as_secs();

        // Check if we can allocate and process data (basic memory check)
        let test_data: Vec<u8> = vec![0; 1024 * 1024]; // 1MB test allocation
        let _ = test_data.len(); // Use the data

        let response_time = start.elapsed().as_millis() as u64;

        HealthCheckResult::new(
            name,
            HealthStatus::Healthy,
            response_time,
            "Resource check completed".to_string(),
        )
        .with_metadata(
            "uptime_seconds".to_string(),
            serde_json::json!(uptime_seconds),
        )
        .with_metadata("memory_test_passed".to_string(), serde_json::json!(true))
    }

    /// Calculate overall status from individual checks
    fn calculate_overall_status(&self, checks: &[HealthCheckResult]) -> HealthStatus {
        if checks.is_empty() {
            return HealthStatus::Critical;
        }

        let mut critical_count = 0;
        let mut unhealthy_count = 0;
        let mut degraded_count = 0;
        let mut _healthy_count = 0;

        for check in checks {
            match check.status {
                HealthStatus::Critical => critical_count += 1,
                HealthStatus::Unhealthy => unhealthy_count += 1,
                HealthStatus::Degraded => degraded_count += 1,
                HealthStatus::Healthy => _healthy_count += 1,
            }
        }

        // Determine overall status based on check results
        if critical_count > 0 {
            HealthStatus::Critical
        } else if unhealthy_count > checks.len() / 2 {
            HealthStatus::Unhealthy
        } else if unhealthy_count > 0 || degraded_count > checks.len() / 2 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    /// Generate summary statistics
    fn generate_summary(
        &self,
        checks: &[HealthCheckResult],
        _overall_response_time_ms: u64,
    ) -> HealthSummary {
        let total_checks = checks.len();
        let mut healthy_checks = 0;
        let mut degraded_checks = 0;
        let mut unhealthy_checks = 0;
        let mut critical_checks = 0;
        let mut total_response_time = 0;
        let mut slowest_check = None;
        let mut slowest_time = 0;

        for check in checks {
            match check.status {
                HealthStatus::Healthy => healthy_checks += 1,
                HealthStatus::Degraded => degraded_checks += 1,
                HealthStatus::Unhealthy => unhealthy_checks += 1,
                HealthStatus::Critical => critical_checks += 1,
            }

            total_response_time += check.response_time_ms;

            if check.response_time_ms > slowest_time {
                slowest_time = check.response_time_ms;
                slowest_check = Some(check.name.clone());
            }
        }

        let avg_response_time_ms = if total_checks > 0 {
            total_response_time / total_checks as u64
        } else {
            0
        };

        let uptime_seconds = Some(self.start_time.elapsed().as_secs());

        HealthSummary {
            total_checks,
            healthy_checks,
            degraded_checks,
            unhealthy_checks,
            critical_checks,
            avg_response_time_ms,
            slowest_check,
            uptime_seconds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::LoxoneClient;
    use crate::error::LoxoneError;
    use async_trait::async_trait;

    struct MockClient {
        should_fail: bool,
    }

    #[async_trait]
    impl LoxoneClient for MockClient {
        async fn connect(&mut self) -> Result<()> {
            if self.should_fail {
                Err(LoxoneError::connection("Mock connection failure"))
            } else {
                Ok(())
            }
        }

        async fn is_connected(&self) -> Result<bool> {
            Ok(!self.should_fail)
        }

        async fn disconnect(&mut self) -> Result<()> {
            Ok(())
        }

        async fn send_command(
            &self,
            _uuid: &str,
            _action: &str,
        ) -> Result<crate::client::LoxoneResponse> {
            if self.should_fail {
                Err(LoxoneError::connection("Mock command failure"))
            } else {
                Ok(crate::client::LoxoneResponse {
                    code: 200,
                    value: serde_json::json!({"status": "ok"}),
                })
            }
        }

        async fn get_structure(&self) -> Result<crate::client::LoxoneStructure> {
            if self.should_fail {
                Err(LoxoneError::connection("Mock failure"))
            } else {
                Ok(crate::client::LoxoneStructure {
                    last_modified: chrono::Utc::now().to_string(),
                    rooms: std::collections::HashMap::new(),
                    controls: std::collections::HashMap::new(),
                    cats: std::collections::HashMap::new(),
                    global_states: std::collections::HashMap::new(),
                })
            }
        }

        async fn get_device_states(
            &self,
            _uuids: &[String],
        ) -> Result<std::collections::HashMap<String, serde_json::Value>> {
            if self.should_fail {
                Err(LoxoneError::connection("Mock failure"))
            } else {
                Ok(std::collections::HashMap::new())
            }
        }

        async fn get_system_info(&self) -> Result<serde_json::Value> {
            if self.should_fail {
                Err(LoxoneError::connection("Mock failure"))
            } else {
                Ok(serde_json::json!({
                    "swVersion": "12.3.4.5",
                    "serialNr": "12345"
                }))
            }
        }

        async fn get_state_values(
            &self,
            _state_uuids: &[String],
        ) -> Result<std::collections::HashMap<String, serde_json::Value>> {
            if self.should_fail {
                Err(LoxoneError::connection("Mock failure"))
            } else {
                Ok(std::collections::HashMap::new())
            }
        }

        async fn health_check(&self) -> Result<bool> {
            Ok(!self.should_fail)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_health_check_success() {
        let client = Arc::new(MockClient { should_fail: false });
        let health_checker = HealthChecker::with_defaults(client);

        let report = health_checker.check_health().await.unwrap();
        assert_eq!(report.overall_status, HealthStatus::Healthy);
        assert!(!report.checks.is_empty());
    }

    #[tokio::test]
    async fn test_health_check_failure() {
        let client = Arc::new(MockClient { should_fail: true });
        let health_checker = HealthChecker::with_defaults(client);

        let report = health_checker.check_health().await.unwrap();
        assert_ne!(report.overall_status, HealthStatus::Healthy);
        assert!(!report.checks.is_empty());
    }

    #[tokio::test]
    async fn test_health_status_score() {
        assert_eq!(HealthStatus::Healthy.score(), 100);
        assert_eq!(HealthStatus::Degraded.score(), 60);
        assert_eq!(HealthStatus::Unhealthy.score(), 30);
        assert_eq!(HealthStatus::Critical.score(), 0);
    }

    #[tokio::test]
    async fn test_health_status_operational() {
        assert!(HealthStatus::Healthy.is_operational());
        assert!(HealthStatus::Degraded.is_operational());
        assert!(!HealthStatus::Unhealthy.is_operational());
        assert!(!HealthStatus::Critical.is_operational());
    }
}
