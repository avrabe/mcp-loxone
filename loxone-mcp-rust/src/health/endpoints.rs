//! HTTP endpoints for health monitoring

use super::{
    HealthChecker, HealthReport, HealthStatus, DiagnosticsCollector, DiagnosticSnapshot,
    DiagnosticTrends,
};
use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Health endpoints handler
pub struct HealthEndpoints {
    /// Health checker instance
    health_checker: Arc<HealthChecker>,
    /// Diagnostics collector
    diagnostics_collector: Arc<RwLock<DiagnosticsCollector>>,
    /// Configuration
    config: HealthEndpointsConfig,
}

impl HealthEndpoints {
    /// Create new health endpoints handler
    pub fn new(
        health_checker: Arc<HealthChecker>,
        diagnostics_collector: Arc<RwLock<DiagnosticsCollector>>,
    ) -> Self {
        Self {
            health_checker,
            diagnostics_collector,
            config: HealthEndpointsConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        health_checker: Arc<HealthChecker>,
        diagnostics_collector: Arc<RwLock<DiagnosticsCollector>>,
        config: HealthEndpointsConfig,
    ) -> Self {
        Self {
            health_checker,
            diagnostics_collector,
            config,
        }
    }

    /// Handle health check endpoint (GET /health)
    pub async fn handle_health(&self) -> Result<HealthResponse> {
        debug!("Processing health check request");
        
        let report = self.health_checker.check_health().await;
        let http_status = report.status.http_status_code();
        
        info!("Health check completed: {:?} (HTTP {})", report.status, http_status);
        
        Ok(HealthResponse {
            status: http_status,
            headers: self.create_health_headers(&report),
            body: serde_json::to_value(HealthCheckResult {
                status: format!("{:?}", report.status),
                timestamp: report.timestamp,
                checks: report.checks.into_iter().map(|c| CheckSummary {
                    name: c.name,
                    status: format!("{:?}", c.status),
                    message: c.message,
                    duration_ms: c.duration_ms,
                    critical: c.critical,
                }).collect(),
                summary: HealthSummaryResponse {
                    total_checks: report.summary.total_checks,
                    healthy: report.summary.healthy_checks,
                    warnings: report.summary.warning_checks,
                    failures: report.summary.unhealthy_checks,
                    critical_failures: report.summary.critical_failures,
                },
                system_info: if self.config.include_system_info {
                    Some(report.system_info)
                } else {
                    None
                },
            }).map_err(|e| LoxoneError::internal(format!("Failed to serialize health response: {}", e)))?,
        })
    }

    /// Handle liveness check endpoint (GET /health/live)
    pub async fn handle_liveness(&self) -> Result<HealthResponse> {
        debug!("Processing liveness check request");
        
        let status = self.health_checker.check_liveness().await;
        let http_status = if matches!(status, HealthStatus::Healthy | HealthStatus::Warning | HealthStatus::Degraded) {
            200
        } else {
            503
        };
        
        Ok(HealthResponse {
            status: http_status,
            headers: HashMap::from([
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Cache-Control".to_string(), "no-cache".to_string()),
            ]),
            body: serde_json::json!({
                "status": format!("{:?}", status),
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                "alive": !matches!(status, HealthStatus::Unhealthy | HealthStatus::Stopping)
            }),
        })
    }

    /// Handle readiness check endpoint (GET /health/ready)
    pub async fn handle_readiness(&self) -> Result<HealthResponse> {
        debug!("Processing readiness check request");
        
        let status = self.health_checker.check_readiness().await;
        let http_status = if matches!(status, HealthStatus::Healthy | HealthStatus::Warning) {
            200
        } else {
            503
        };
        
        Ok(HealthResponse {
            status: http_status,
            headers: HashMap::from([
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Cache-Control".to_string(), "no-cache".to_string()),
            ]),
            body: serde_json::json!({
                "status": format!("{:?}", status),
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                "ready": matches!(status, HealthStatus::Healthy | HealthStatus::Warning)
            }),
        })
    }

    /// Handle detailed diagnostics endpoint (GET /health/diagnostics)
    pub async fn handle_diagnostics(&self) -> Result<HealthResponse> {
        debug!("Processing diagnostics request");
        
        if !self.config.enable_diagnostics {
            return Ok(HealthResponse {
                status: 404,
                headers: HashMap::new(),
                body: serde_json::json!({
                    "error": "Diagnostics endpoint is disabled"
                }),
            });
        }

        let mut collector = self.diagnostics_collector.write().await;
        let snapshot = collector.collect_diagnostics().await?;
        let trends = collector.calculate_trends();
        
        drop(collector);
        
        Ok(HealthResponse {
            status: 200,
            headers: HashMap::from([
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Cache-Control".to_string(), format!("max-age={}", self.config.diagnostics_cache_seconds)),
            ]),
            body: serde_json::to_value(DiagnosticsResponse {
                snapshot,
                trends,
                collection_info: CollectionInfo {
                    interval_seconds: 60, // Default from collector
                    history_size: 100,    // Default from collector
                },
            }).map_err(|e| LoxoneError::internal(format!("Failed to serialize diagnostics: {}", e)))?,
        })
    }

    /// Handle metrics endpoint (GET /health/metrics)
    pub async fn handle_metrics(&self) -> Result<HealthResponse> {
        debug!("Processing metrics request");
        
        if !self.config.enable_metrics {
            return Ok(HealthResponse {
                status: 404,
                headers: HashMap::new(),
                body: serde_json::json!({
                    "error": "Metrics endpoint is disabled"
                }),
            });
        }

        let report = self.health_checker.check_health().await;
        let collector = self.diagnostics_collector.read().await;
        let latest_snapshot = collector.get_latest();
        
        let metrics = if self.config.prometheus_format {
            self.format_prometheus_metrics(&report, latest_snapshot.as_ref())?
        } else {
            self.format_json_metrics(&report, latest_snapshot.as_ref())?
        };
        
        let content_type = if self.config.prometheus_format {
            "text/plain; version=0.0.4"
        } else {
            "application/json"
        };
        
        Ok(HealthResponse {
            status: 200,
            headers: HashMap::from([
                ("Content-Type".to_string(), content_type.to_string()),
                ("Cache-Control".to_string(), format!("max-age={}", self.config.metrics_cache_seconds)),
            ]),
            body: metrics,
        })
    }

    /// Handle startup probe endpoint (GET /health/startup)
    pub async fn handle_startup(&self) -> Result<HealthResponse> {
        debug!("Processing startup probe request");
        
        let report = self.health_checker.check_health().await;
        let is_starting = matches!(report.status, HealthStatus::Starting);
        let is_ready = report.is_ready();
        
        let http_status = if is_ready {
            200
        } else if is_starting {
            503 // Still starting
        } else {
            500 // Failed to start
        };
        
        Ok(HealthResponse {
            status: http_status,
            headers: HashMap::from([
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Cache-Control".to_string(), "no-cache".to_string()),
            ]),
            body: serde_json::json!({
                "status": format!("{:?}", report.status),
                "ready": is_ready,
                "starting": is_starting,
                "timestamp": report.timestamp,
                "startup_time_seconds": report.system_info.uptime_seconds
            }),
        })
    }

    /// Create health check headers
    fn create_health_headers(&self, report: &HealthReport) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("X-Health-Status".to_string(), format!("{:?}", report.status));
        headers.insert("X-Health-Timestamp".to_string(), report.timestamp.to_string());
        headers.insert("X-Health-Checks".to_string(), report.checks.len().to_string());
        
        if !report.is_ready() {
            headers.insert("Retry-After".to_string(), "30".to_string());
        }
        
        // Add cache control based on health status
        let cache_time = match report.status {
            HealthStatus::Healthy => self.config.healthy_cache_seconds,
            HealthStatus::Warning => self.config.warning_cache_seconds,
            _ => 0, // No cache for unhealthy states
        };
        
        if cache_time > 0 {
            headers.insert("Cache-Control".to_string(), format!("max-age={}", cache_time));
        } else {
            headers.insert("Cache-Control".to_string(), "no-cache".to_string());
        }
        
        headers
    }

    /// Format metrics in Prometheus format
    fn format_prometheus_metrics(&self, report: &HealthReport, snapshot: Option<&DiagnosticSnapshot>) -> Result<Value> {
        let mut metrics = Vec::new();
        
        // Health status metric
        metrics.push(format!(
            "# HELP loxone_health_status Current health status (0=unhealthy, 1=warning, 2=degraded, 3=healthy)\n\
             # TYPE loxone_health_status gauge\n\
             loxone_health_status {}\n",
            match report.status {
                HealthStatus::Unhealthy => 0,
                HealthStatus::Warning => 1,
                HealthStatus::Degraded => 2,
                HealthStatus::Healthy => 3,
                HealthStatus::Starting => 1,
                HealthStatus::Stopping => 0,
            }
        ));
        
        // Check metrics
        metrics.push("# HELP loxone_health_checks_total Total number of health checks\n# TYPE loxone_health_checks_total gauge\n".to_string());
        metrics.push(format!("loxone_health_checks_total {}\n", report.checks.len()));
        
        metrics.push("# HELP loxone_health_check_failures_total Total number of failed health checks\n# TYPE loxone_health_check_failures_total gauge\n".to_string());
        metrics.push(format!("loxone_health_check_failures_total {}\n", report.summary.unhealthy_checks));
        
        // System metrics from snapshot
        if let Some(snapshot) = snapshot {
            metrics.push(format!(
                "# HELP loxone_memory_usage_percent Memory usage percentage\n\
                 # TYPE loxone_memory_usage_percent gauge\n\
                 loxone_memory_usage_percent {}\n",
                snapshot.system_info.memory.usage_percent
            ));
            
            metrics.push(format!(
                "# HELP loxone_cpu_usage_percent CPU usage percentage\n\
                 # TYPE loxone_cpu_usage_percent gauge\n\
                 loxone_cpu_usage_percent {}\n",
                snapshot.system_info.cpu.usage_percent
            ));
        }
        
        Ok(Value::String(metrics.join("")))
    }

    /// Format metrics in JSON format
    fn format_json_metrics(&self, report: &HealthReport, snapshot: Option<&DiagnosticSnapshot>) -> Result<Value> {
        let mut metrics = serde_json::Map::new();
        
        // Health metrics
        metrics.insert("health_status".to_string(), serde_json::json!({
            "value": format!("{:?}", report.status),
            "timestamp": report.timestamp
        }));
        
        metrics.insert("health_checks_total".to_string(), serde_json::json!({
            "value": report.checks.len(),
            "timestamp": report.timestamp
        }));
        
        metrics.insert("health_check_failures".to_string(), serde_json::json!({
            "value": report.summary.unhealthy_checks,
            "timestamp": report.timestamp
        }));
        
        // System metrics
        if let Some(snapshot) = snapshot {
            metrics.insert("memory_usage_percent".to_string(), serde_json::json!({
                "value": snapshot.system_info.memory.usage_percent,
                "timestamp": snapshot.timestamp
            }));
            
            metrics.insert("cpu_usage_percent".to_string(), serde_json::json!({
                "value": snapshot.system_info.cpu.usage_percent,
                "timestamp": snapshot.timestamp
            }));
            
            metrics.insert("disk_usage_percent".to_string(), serde_json::json!({
                "value": snapshot.disk_info.filesystems.get(0)
                    .map(|fs| fs.usage_percent)
                    .unwrap_or(0.0),
                "timestamp": snapshot.timestamp
            }));
        }
        
        Ok(Value::Object(metrics))
    }
}

/// Health endpoints configuration
#[derive(Debug, Clone)]
pub struct HealthEndpointsConfig {
    /// Include detailed system information in health response
    pub include_system_info: bool,
    /// Enable diagnostics endpoint
    pub enable_diagnostics: bool,
    /// Enable metrics endpoint
    pub enable_metrics: bool,
    /// Use Prometheus format for metrics
    pub prometheus_format: bool,
    /// Cache time for healthy responses (seconds)
    pub healthy_cache_seconds: u32,
    /// Cache time for warning responses (seconds)
    pub warning_cache_seconds: u32,
    /// Cache time for diagnostics responses (seconds)
    pub diagnostics_cache_seconds: u32,
    /// Cache time for metrics responses (seconds)
    pub metrics_cache_seconds: u32,
}

impl Default for HealthEndpointsConfig {
    fn default() -> Self {
        Self {
            include_system_info: true,
            enable_diagnostics: true,
            enable_metrics: true,
            prometheus_format: false,
            healthy_cache_seconds: 30,
            warning_cache_seconds: 10,
            diagnostics_cache_seconds: 60,
            metrics_cache_seconds: 15,
        }
    }
}

/// HTTP response structure
#[derive(Debug)]
pub struct HealthResponse {
    /// HTTP status code
    pub status: u16,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response body
    pub body: Value,
}

/// Health check result for JSON response
#[derive(Debug, Serialize)]
struct HealthCheckResult {
    status: String,
    timestamp: u64,
    checks: Vec<CheckSummary>,
    summary: HealthSummaryResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_info: Option<super::SystemInfo>,
}

/// Check summary for response
#[derive(Debug, Serialize)]
struct CheckSummary {
    name: String,
    status: String,
    message: String,
    duration_ms: u64,
    critical: bool,
}

/// Health summary for response
#[derive(Debug, Serialize)]
struct HealthSummaryResponse {
    total_checks: usize,
    healthy: usize,
    warnings: usize,
    failures: usize,
    critical_failures: usize,
}

/// Diagnostics response
#[derive(Debug, Serialize)]
struct DiagnosticsResponse {
    snapshot: DiagnosticSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    trends: Option<DiagnosticTrends>,
    collection_info: CollectionInfo,
}

/// Collection information
#[derive(Debug, Serialize)]
struct CollectionInfo {
    interval_seconds: u64,
    history_size: usize,
}

/// Helper function to create a basic health endpoints setup
pub fn create_health_endpoints(
    health_checker: Arc<HealthChecker>,
) -> (Arc<HealthEndpoints>, Arc<RwLock<DiagnosticsCollector>>) {
    let diagnostics_collector = Arc::new(RwLock::new(DiagnosticsCollector::default()));
    let endpoints = Arc::new(HealthEndpoints::new(
        health_checker,
        diagnostics_collector.clone(),
    ));
    
    (endpoints, diagnostics_collector)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::checks::MemoryHealthCheck;

    #[tokio::test]
    async fn test_health_endpoint() {
        let health_checker = Arc::new(
            HealthChecker::new()
                .add_check(Box::new(MemoryHealthCheck::default()))
        );
        let diagnostics_collector = Arc::new(RwLock::new(DiagnosticsCollector::default()));
        let endpoints = HealthEndpoints::new(health_checker, diagnostics_collector);
        
        let response = endpoints.handle_health().await.unwrap();
        assert_eq!(response.status, 200);
        assert!(response.headers.contains_key("Content-Type"));
        assert!(response.body.is_object());
    }

    #[tokio::test]
    async fn test_liveness_endpoint() {
        let health_checker = Arc::new(HealthChecker::new());
        let diagnostics_collector = Arc::new(RwLock::new(DiagnosticsCollector::default()));
        let endpoints = HealthEndpoints::new(health_checker, diagnostics_collector);
        
        let response = endpoints.handle_liveness().await.unwrap();
        assert_eq!(response.status, 200);
        
        let body = response.body.as_object().unwrap();
        assert!(body.contains_key("alive"));
        assert!(body.contains_key("status"));
    }

    #[tokio::test]
    async fn test_readiness_endpoint() {
        let health_checker = Arc::new(HealthChecker::new());
        let diagnostics_collector = Arc::new(RwLock::new(DiagnosticsCollector::default()));
        let endpoints = HealthEndpoints::new(health_checker, diagnostics_collector);
        
        let response = endpoints.handle_readiness().await.unwrap();
        assert_eq!(response.status, 200);
        
        let body = response.body.as_object().unwrap();
        assert!(body.contains_key("ready"));
        assert!(body.contains_key("status"));
    }

    #[tokio::test]
    async fn test_diagnostics_endpoint() {
        let health_checker = Arc::new(HealthChecker::new());
        let diagnostics_collector = Arc::new(RwLock::new(DiagnosticsCollector::default()));
        let endpoints = HealthEndpoints::new(health_checker, diagnostics_collector);
        
        let response = endpoints.handle_diagnostics().await.unwrap();
        assert_eq!(response.status, 200);
        
        let body = response.body.as_object().unwrap();
        assert!(body.contains_key("snapshot"));
        assert!(body.contains_key("collection_info"));
    }

    #[tokio::test]
    async fn test_metrics_endpoint_json() {
        let health_checker = Arc::new(HealthChecker::new());
        let diagnostics_collector = Arc::new(RwLock::new(DiagnosticsCollector::default()));
        let mut config = HealthEndpointsConfig::default();
        config.prometheus_format = false;
        let endpoints = HealthEndpoints::with_config(health_checker, diagnostics_collector, config);
        
        let response = endpoints.handle_metrics().await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.headers.get("Content-Type").unwrap(), "application/json");
        assert!(response.body.is_object());
    }

    #[tokio::test]
    async fn test_metrics_endpoint_prometheus() {
        let health_checker = Arc::new(HealthChecker::new());
        let diagnostics_collector = Arc::new(RwLock::new(DiagnosticsCollector::default()));
        let mut config = HealthEndpointsConfig::default();
        config.prometheus_format = true;
        let endpoints = HealthEndpoints::with_config(health_checker, diagnostics_collector, config);
        
        let response = endpoints.handle_metrics().await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.headers.get("Content-Type").unwrap(), "text/plain; version=0.0.4");
        assert!(response.body.is_string());
    }
}