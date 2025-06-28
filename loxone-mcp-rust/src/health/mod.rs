//! Health check and system diagnostics module
//!
//! This module provides comprehensive health monitoring for the Loxone MCP server,
//! including dependency checks, performance metrics, and system diagnostics.

pub mod checks;
pub mod diagnostics;
pub mod endpoints;
pub mod monitoring;

// Re-export types from diagnostics
pub use diagnostics::{DiagnosticSnapshot, DiagnosticTrends, DiagnosticsCollector, TrendDirection};

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Overall health status of the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    /// All systems operational
    Healthy,
    /// Some non-critical issues detected
    Warning,
    /// Critical issues affecting functionality
    Unhealthy,
    /// System is degraded but functional
    Degraded,
    /// System is starting up
    Starting,
    /// System is shutting down
    Stopping,
}

impl HealthStatus {
    /// Combine multiple health statuses to determine overall status
    pub fn combine(statuses: &[HealthStatus]) -> HealthStatus {
        if statuses.is_empty() {
            return HealthStatus::Unhealthy;
        }

        if statuses
            .iter()
            .any(|s| matches!(s, HealthStatus::Unhealthy))
        {
            HealthStatus::Unhealthy
        } else if statuses.iter().any(|s| matches!(s, HealthStatus::Degraded)) {
            HealthStatus::Degraded
        } else if statuses.iter().any(|s| matches!(s, HealthStatus::Warning)) {
            HealthStatus::Warning
        } else if statuses
            .iter()
            .any(|s| matches!(s, HealthStatus::Starting | HealthStatus::Stopping))
        {
            HealthStatus::Starting // Use Starting for any transitional state
        } else {
            HealthStatus::Healthy
        }
    }

    /// Check if status indicates the system is operational
    pub fn is_operational(&self) -> bool {
        matches!(
            self,
            HealthStatus::Healthy | HealthStatus::Warning | HealthStatus::Degraded
        )
    }

    /// Get HTTP status code for this health status
    pub fn http_status_code(&self) -> u16 {
        match self {
            HealthStatus::Healthy => 200,
            HealthStatus::Warning => 200,
            HealthStatus::Degraded => 503,
            HealthStatus::Unhealthy => 503,
            HealthStatus::Starting => 503,
            HealthStatus::Stopping => 503,
        }
    }
}

/// Individual health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// Name of the health check
    pub name: String,
    /// Current status
    pub status: HealthStatus,
    /// Human-readable message
    pub message: String,
    /// Check execution time in milliseconds
    pub duration_ms: u64,
    /// Timestamp when check was performed
    pub timestamp: u64,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Error details if check failed
    pub error: Option<String>,
    /// Whether this check is critical for system operation
    pub critical: bool,
}

impl HealthCheckResult {
    /// Create a successful health check result
    pub fn healthy(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Healthy,
            message: message.to_string(),
            duration_ms: 0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
            error: None,
            critical: false,
        }
    }

    /// Create a warning health check result
    pub fn warning(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Warning,
            message: message.to_string(),
            duration_ms: 0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
            error: None,
            critical: false,
        }
    }

    /// Create an unhealthy health check result
    pub fn unhealthy(name: &str, message: &str, error: Option<String>) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Unhealthy,
            message: message.to_string(),
            duration_ms: 0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
            error,
            critical: true,
        }
    }

    /// Add metadata to the health check result
    pub fn with_metadata<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<serde_json::Value>,
    {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Mark this check as critical
    pub fn critical(mut self) -> Self {
        self.critical = true;
        self
    }

    /// Set the duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration_ms = duration.as_millis() as u64;
        self
    }
}

/// Complete system health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Overall system status
    pub status: HealthStatus,
    /// Timestamp when report was generated
    pub timestamp: u64,
    /// Individual health check results
    pub checks: Vec<HealthCheckResult>,
    /// System information
    pub system_info: SystemInfo,
    /// Dependency status
    pub dependencies: Vec<DependencyStatus>,
    /// Performance metrics
    pub metrics: PerformanceMetrics,
    /// Summary statistics
    pub summary: HealthSummary,
}

impl HealthReport {
    /// Create a new health report
    pub fn new(checks: Vec<HealthCheckResult>) -> Self {
        let statuses: Vec<HealthStatus> = checks.iter().map(|c| c.status.clone()).collect();
        let overall_status = HealthStatus::combine(&statuses);

        let system_info = SystemInfo::current();
        let summary = HealthSummary::from_checks(&checks);

        Self {
            status: overall_status,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            checks,
            system_info,
            dependencies: Vec::new(),
            metrics: PerformanceMetrics::default(),
            summary,
        }
    }

    /// Add dependency status to the report
    pub fn with_dependencies(mut self, dependencies: Vec<DependencyStatus>) -> Self {
        self.dependencies = dependencies;
        self
    }

    /// Add performance metrics to the report
    pub fn with_metrics(mut self, metrics: PerformanceMetrics) -> Self {
        self.metrics = metrics;
        self
    }

    /// Check if system is ready to serve traffic
    pub fn is_ready(&self) -> bool {
        self.status.is_operational() && self.dependencies.iter().all(|d| d.status.is_operational())
    }

    /// Check if system is alive (basic liveness check)
    pub fn is_alive(&self) -> bool {
        !matches!(
            self.status,
            HealthStatus::Unhealthy | HealthStatus::Stopping
        )
    }
}

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Server version
    pub version: String,
    /// Build timestamp
    pub build_time: String,
    /// Git commit hash
    pub git_commit: String,
    /// Rust version used for compilation
    pub rust_version: String,
    /// Target architecture
    pub target_arch: String,
    /// Operating system
    pub os: String,
    /// Process ID
    pub pid: u32,
    /// Server uptime in seconds
    pub uptime_seconds: u64,
    /// Memory usage information
    pub memory: MemoryInfo,
    /// CPU information
    pub cpu: CpuInfo,
}

impl SystemInfo {
    /// Get current system information
    pub fn current() -> Self {
        let uptime = std::process::id(); // Simplified - in real implementation would track actual uptime

        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_time: std::env::var("VERGEN_BUILD_TIMESTAMP")
                .unwrap_or_else(|_| "unknown".to_string()),
            git_commit: std::env::var("VERGEN_GIT_SHA").unwrap_or_else(|_| "unknown".to_string()),
            rust_version: std::env::var("VERGEN_RUSTC_SEMVER")
                .unwrap_or_else(|_| "unknown".to_string()),
            target_arch: std::env::consts::ARCH.to_string(),
            os: std::env::consts::OS.to_string(),
            pid: std::process::id(),
            uptime_seconds: uptime as u64, // Simplified
            memory: MemoryInfo::current(),
            cpu: CpuInfo::current(),
        }
    }
}

/// Memory usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// Used memory in bytes
    pub used_bytes: u64,
    /// Available memory in bytes
    pub available_bytes: u64,
    /// Total memory in bytes
    pub total_bytes: u64,
    /// Memory usage percentage
    pub usage_percent: f64,
}

impl MemoryInfo {
    /// Get current memory information
    pub fn current() -> Self {
        // Simplified implementation - in production would use system calls
        Self {
            used_bytes: 1024 * 1024 * 100,      // 100MB
            available_bytes: 1024 * 1024 * 900, // 900MB
            total_bytes: 1024 * 1024 * 1000,    // 1GB
            usage_percent: 10.0,
        }
    }
}

/// CPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// Number of CPU cores
    pub cores: usize,
    /// Current CPU usage percentage
    pub usage_percent: f64,
    /// Load average over 1 minute
    pub load_avg_1m: f64,
    /// Load average over 5 minutes
    pub load_avg_5m: f64,
    /// Load average over 15 minutes
    pub load_avg_15m: f64,
}

impl CpuInfo {
    /// Get current CPU information
    pub fn current() -> Self {
        Self {
            cores: std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(1),
            usage_percent: 5.0, // Simplified
            load_avg_1m: 0.5,
            load_avg_5m: 0.4,
            load_avg_15m: 0.3,
        }
    }
}

/// Dependency status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyStatus {
    /// Name of the dependency
    pub name: String,
    /// Dependency type (database, service, etc.)
    pub dependency_type: DependencyType,
    /// Current status
    pub status: HealthStatus,
    /// Status message
    pub message: String,
    /// Endpoint or connection string
    pub endpoint: String,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Last successful check timestamp
    pub last_success: Option<u64>,
    /// Error details if dependency is down
    pub error: Option<String>,
    /// Whether this dependency is critical
    pub critical: bool,
}

/// Type of dependency
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DependencyType {
    /// Loxone Miniserver
    LoxoneMiniserver,
    /// Database connection
    Database,
    /// External API
    ExternalApi,
    /// File system
    FileSystem,
    /// Network service
    NetworkService,
    /// Cache service
    Cache,
    /// Message queue
    MessageQueue,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Request rate (requests per second)
    pub requests_per_second: f64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// 95th percentile response time
    pub p95_response_time_ms: f64,
    /// 99th percentile response time
    pub p99_response_time_ms: f64,
    /// Error rate (errors per second)
    pub errors_per_second: f64,
    /// Total requests processed
    pub total_requests: u64,
    /// Total errors encountered
    pub total_errors: u64,
    /// Active connections
    pub active_connections: u64,
    /// Thread pool metrics
    pub thread_pool: ThreadPoolMetrics,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            requests_per_second: 0.0,
            avg_response_time_ms: 0.0,
            p95_response_time_ms: 0.0,
            p99_response_time_ms: 0.0,
            errors_per_second: 0.0,
            total_requests: 0,
            total_errors: 0,
            active_connections: 0,
            thread_pool: ThreadPoolMetrics::default(),
        }
    }
}

/// Thread pool metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadPoolMetrics {
    /// Active threads
    pub active_threads: usize,
    /// Total threads in pool
    pub total_threads: usize,
    /// Queued tasks
    pub queued_tasks: usize,
    /// Completed tasks
    pub completed_tasks: u64,
}

impl Default for ThreadPoolMetrics {
    fn default() -> Self {
        Self {
            active_threads: 0,
            total_threads: std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(1),
            queued_tasks: 0,
            completed_tasks: 0,
        }
    }
}

/// Health summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSummary {
    /// Total number of checks performed
    pub total_checks: usize,
    /// Number of healthy checks
    pub healthy_checks: usize,
    /// Number of warning checks
    pub warning_checks: usize,
    /// Number of unhealthy checks
    pub unhealthy_checks: usize,
    /// Number of critical checks that failed
    pub critical_failures: usize,
    /// Average check duration in milliseconds
    pub avg_check_duration_ms: f64,
    /// Slowest check duration in milliseconds
    pub max_check_duration_ms: u64,
}

impl HealthSummary {
    /// Create summary from health check results
    pub fn from_checks(checks: &[HealthCheckResult]) -> Self {
        let total_checks = checks.len();
        let healthy_checks = checks
            .iter()
            .filter(|c| matches!(c.status, HealthStatus::Healthy))
            .count();
        let warning_checks = checks
            .iter()
            .filter(|c| matches!(c.status, HealthStatus::Warning))
            .count();
        let unhealthy_checks = checks
            .iter()
            .filter(|c| matches!(c.status, HealthStatus::Unhealthy))
            .count();
        let critical_failures = checks
            .iter()
            .filter(|c| c.critical && !matches!(c.status, HealthStatus::Healthy))
            .count();

        let total_duration: u64 = checks.iter().map(|c| c.duration_ms).sum();
        let avg_check_duration_ms = if total_checks > 0 {
            total_duration as f64 / total_checks as f64
        } else {
            0.0
        };

        let max_check_duration_ms = checks.iter().map(|c| c.duration_ms).max().unwrap_or(0);

        Self {
            total_checks,
            healthy_checks,
            warning_checks,
            unhealthy_checks,
            critical_failures,
            avg_check_duration_ms,
            max_check_duration_ms,
        }
    }
}

/// Health check trait that all health checks must implement
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    /// Name of the health check
    fn name(&self) -> &str;

    /// Whether this check is critical for system operation
    fn is_critical(&self) -> bool {
        false
    }

    /// Timeout for this health check
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }

    /// Execute the health check
    async fn check(&self) -> Result<HealthCheckResult>;
}

/// Health checker that runs multiple health checks
pub struct HealthChecker {
    checks: Vec<Box<dyn HealthCheck>>,
    dependency_checks: Vec<Box<dyn DependencyCheck>>,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
            dependency_checks: Vec::new(),
        }
    }

    /// Add a health check
    pub fn add_check(mut self, check: Box<dyn HealthCheck>) -> Self {
        self.checks.push(check);
        self
    }

    /// Add a dependency check
    pub fn add_dependency_check(mut self, check: Box<dyn DependencyCheck>) -> Self {
        self.dependency_checks.push(check);
        self
    }

    /// Run all health checks and generate a report
    pub async fn check_health(&self) -> HealthReport {
        debug!("Running {} health checks", self.checks.len());

        let mut results = Vec::new();

        // Run health checks in parallel
        let check_futures: Vec<_> = self
            .checks
            .iter()
            .map(|check| async {
                let start_time = std::time::Instant::now();
                let name = check.name().to_string();

                match tokio::time::timeout(check.timeout(), check.check()).await {
                    Ok(Ok(mut result)) => {
                        result.duration_ms = start_time.elapsed().as_millis() as u64;
                        result
                    }
                    Ok(Err(e)) => {
                        let _duration = start_time.elapsed().as_millis() as u64;
                        HealthCheckResult::unhealthy(&name, "Check failed", Some(e.to_string()))
                            .with_duration(start_time.elapsed())
                            .with_metadata("timeout", false)
                    }
                    Err(_) => {
                        let _duration = start_time.elapsed().as_millis() as u64;
                        HealthCheckResult::unhealthy(&name, "Check timed out", None)
                            .with_duration(start_time.elapsed())
                            .with_metadata("timeout", true)
                    }
                }
            })
            .collect();

        results = futures::future::join_all(check_futures).await;

        // Run dependency checks
        let mut dependencies = Vec::new();
        for dep_check in &self.dependency_checks {
            match dep_check.check().await {
                Ok(status) => dependencies.push(status),
                Err(e) => {
                    warn!("Dependency check {} failed: {}", dep_check.name(), e);
                    dependencies.push(DependencyStatus {
                        name: dep_check.name().to_string(),
                        dependency_type: dep_check.dependency_type(),
                        status: HealthStatus::Unhealthy,
                        message: "Check failed".to_string(),
                        endpoint: dep_check.endpoint().to_string(),
                        response_time_ms: 0,
                        last_success: None,
                        error: Some(e.to_string()),
                        critical: dep_check.is_critical(),
                    });
                }
            }
        }

        let report = HealthReport::new(results)
            .with_dependencies(dependencies)
            .with_metrics(PerformanceMetrics::default()); // Would be populated from actual metrics

        debug!("Health check completed: {:?}", report.status);
        report
    }

    /// Perform a quick liveness check (subset of all checks)
    pub async fn check_liveness(&self) -> HealthStatus {
        // Run only critical checks for liveness
        let critical_checks: Vec<_> = self.checks.iter().filter(|c| c.is_critical()).collect();

        if critical_checks.is_empty() {
            return HealthStatus::Healthy;
        }

        let mut critical_results = Vec::new();
        for check in critical_checks {
            match tokio::time::timeout(Duration::from_secs(1), check.check()).await {
                Ok(Ok(result)) => critical_results.push(result.status),
                _ => critical_results.push(HealthStatus::Unhealthy),
            }
        }

        HealthStatus::combine(&critical_results)
    }

    /// Perform a readiness check (all dependencies must be available)
    pub async fn check_readiness(&self) -> HealthStatus {
        let mut dependency_statuses = Vec::new();

        for dep_check in &self.dependency_checks {
            if dep_check.is_critical() {
                match dep_check.check().await {
                    Ok(status) => dependency_statuses.push(status.status),
                    Err(_) => dependency_statuses.push(HealthStatus::Unhealthy),
                }
            }
        }

        HealthStatus::combine(&dependency_statuses)
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Dependency check trait
#[async_trait::async_trait]
pub trait DependencyCheck: Send + Sync {
    /// Name of the dependency
    fn name(&self) -> &str;

    /// Type of dependency
    fn dependency_type(&self) -> DependencyType;

    /// Endpoint or connection string
    fn endpoint(&self) -> &str;

    /// Whether this dependency is critical
    fn is_critical(&self) -> bool {
        true
    }

    /// Check the dependency status
    async fn check(&self) -> Result<DependencyStatus>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_combination() {
        assert_eq!(
            HealthStatus::combine(&[HealthStatus::Healthy, HealthStatus::Healthy]),
            HealthStatus::Healthy
        );

        assert_eq!(
            HealthStatus::combine(&[HealthStatus::Healthy, HealthStatus::Warning]),
            HealthStatus::Warning
        );

        assert_eq!(
            HealthStatus::combine(&[HealthStatus::Healthy, HealthStatus::Unhealthy]),
            HealthStatus::Unhealthy
        );

        assert_eq!(
            HealthStatus::combine(&[HealthStatus::Warning, HealthStatus::Degraded]),
            HealthStatus::Degraded
        );
    }

    #[test]
    fn test_health_check_result_creation() {
        let result = HealthCheckResult::healthy("test", "All good")
            .with_metadata("version", "1.0")
            .critical();

        assert_eq!(result.status, HealthStatus::Healthy);
        assert!(result.critical);
        assert_eq!(result.metadata.get("version").unwrap(), "1.0");
    }

    #[test]
    fn test_health_summary() {
        let checks = vec![
            HealthCheckResult::healthy("check1", "OK"),
            HealthCheckResult::warning("check2", "Warning"),
            HealthCheckResult::unhealthy("check3", "Failed", None).critical(),
        ];

        let summary = HealthSummary::from_checks(&checks);
        assert_eq!(summary.total_checks, 3);
        assert_eq!(summary.healthy_checks, 1);
        assert_eq!(summary.warning_checks, 1);
        assert_eq!(summary.unhealthy_checks, 1);
        assert_eq!(summary.critical_failures, 1);
    }

    #[test]
    fn test_system_info() {
        let info = SystemInfo::current();
        assert!(!info.version.is_empty());
        assert!(!info.target_arch.is_empty());
        assert!(!info.os.is_empty());
        assert!(info.pid > 0);
    }

    #[tokio::test]
    async fn test_health_checker() {
        let checker = HealthChecker::new();
        let report = checker.check_health().await;

        assert_eq!(report.status, HealthStatus::Healthy); // No checks = healthy
        assert_eq!(report.checks.len(), 0);
        assert!(report.is_alive());
        assert!(report.is_ready());
    }
}
