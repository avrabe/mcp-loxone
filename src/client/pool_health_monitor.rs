//! Connection pool health monitoring and metrics collection
//!
//! This module provides comprehensive health monitoring, metrics collection,
//! and alerting capabilities for connection pools.

use crate::client::adaptive_pool::{AdaptiveConnectionPool, PoolStatistics};
use crate::client::load_balancer::LoadBalancingStatistics;
use crate::error::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration as TokioDuration};
use tracing::{debug, error, info, warn};

/// Health monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMonitorConfig {
    /// Monitoring interval
    pub check_interval: Duration,
    /// Metrics retention period
    pub metrics_retention: Duration,
    /// Health check timeout
    pub health_check_timeout: Duration,
    /// Alert thresholds
    pub alert_thresholds: AlertThresholds,
    /// Enable detailed metrics collection
    pub detailed_metrics: bool,
    /// Maximum metrics history size
    pub max_history_size: usize,
}

/// Alert threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// Connection utilization warning threshold (0.0-1.0)
    pub connection_utilization_warning: f64,
    /// Connection utilization critical threshold (0.0-1.0)
    pub connection_utilization_critical: f64,
    /// Average response time warning threshold (ms)
    pub response_time_warning_ms: u64,
    /// Average response time critical threshold (ms)
    pub response_time_critical_ms: u64,
    /// Error rate warning threshold (0.0-1.0)
    pub error_rate_warning: f64,
    /// Error rate critical threshold (0.0-1.0)
    pub error_rate_critical: f64,
    /// Queue depth warning threshold
    pub queue_depth_warning: usize,
    /// Queue depth critical threshold
    pub queue_depth_critical: usize,
}

/// Pool health status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// All systems operating normally
    Healthy,
    /// Performance degraded but functional
    Warning,
    /// Critical issues detected
    Critical,
    /// System unavailable
    Unavailable,
}

/// Health alert information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthAlert {
    /// Alert severity
    pub severity: AlertSeverity,
    /// Alert type
    pub alert_type: AlertType,
    /// Human-readable message
    pub message: String,
    /// Metric value that triggered the alert
    pub value: f64,
    /// Threshold that was breached
    pub threshold: f64,
    /// Alert timestamp
    pub timestamp: DateTime<Utc>,
    /// Additional context
    pub context: HashMap<String, String>,
}

/// Alert severity levels
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Warning,
    Critical,
}

/// Types of health alerts
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AlertType {
    HighConnectionUtilization,
    HighResponseTime,
    HighErrorRate,
    HighQueueDepth,
    ConnectionFailure,
    LoadBalancerIssue,
    CircuitBreakerTripped,
    MemoryUsage,
    PerformanceDegradation,
}

/// Comprehensive health metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    /// Timestamp of this snapshot
    pub timestamp: DateTime<Utc>,
    /// Overall health status
    pub health_status: HealthStatus,
    /// Pool statistics
    pub pool_stats: PoolStatistics,
    /// Load balancing statistics
    pub load_balancing_stats: LoadBalancingStatistics,
    /// Connection-level metrics
    pub connection_metrics: HashMap<String, ConnectionHealthMetrics>,
    /// System resource metrics
    pub system_metrics: SystemMetrics,
    /// Performance trends
    pub performance_trends: PerformanceTrends,
    /// Active alerts
    pub active_alerts: Vec<HealthAlert>,
}

/// Connection-level health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionHealthMetrics {
    /// Connection ID
    pub connection_id: String,
    /// Authentication method used
    pub auth_method: String,
    /// Current status
    pub is_healthy: bool,
    /// Active requests count
    pub active_requests: usize,
    /// Total requests served
    pub total_requests: u64,
    /// Failed requests count
    pub failed_requests: u64,
    /// Success rate
    pub success_rate: f64,
    /// Average response time (ms)
    pub avg_response_time_ms: f64,
    /// Last health check time
    pub last_health_check: DateTime<Utc>,
    /// Connection age
    pub connection_age: Duration,
    /// Circuit breaker status
    pub circuit_breaker_status: Option<String>,
}

/// System resource metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Memory usage of the pool (bytes)
    pub memory_usage_bytes: u64,
    /// CPU usage percentage (if available)
    pub cpu_usage_percent: Option<f64>,
    /// Network connections count
    pub network_connections: usize,
    /// Thread pool utilization
    pub thread_pool_utilization: Option<f64>,
}

/// Performance trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTrends {
    /// Response time trend over time
    pub response_time_trend: Vec<TrendPoint>,
    /// Throughput trend (requests per second)
    pub throughput_trend: Vec<TrendPoint>,
    /// Error rate trend
    pub error_rate_trend: Vec<TrendPoint>,
    /// Connection utilization trend
    pub utilization_trend: Vec<TrendPoint>,
}

/// A single trend data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendPoint {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Metric value
    pub value: f64,
}

/// Health monitor for connection pools
pub struct PoolHealthMonitor {
    /// Configuration
    config: HealthMonitorConfig,
    /// Pool reference
    pool: Arc<AdaptiveConnectionPool>,
    /// Historical metrics
    metrics_history: Arc<RwLock<VecDeque<HealthMetrics>>>,
    /// Alert broadcaster
    alert_sender: broadcast::Sender<HealthAlert>,
    /// Current alerts
    active_alerts: Arc<RwLock<HashMap<String, HealthAlert>>>,
    /// Monitoring task handle
    monitor_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::seconds(30),
            metrics_retention: Duration::hours(24),
            health_check_timeout: Duration::seconds(5),
            alert_thresholds: AlertThresholds::default(),
            detailed_metrics: true,
            max_history_size: 2880, // 24 hours at 30-second intervals
        }
    }
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            connection_utilization_warning: 0.8,
            connection_utilization_critical: 0.95,
            response_time_warning_ms: 1000,
            response_time_critical_ms: 5000,
            error_rate_warning: 0.05,
            error_rate_critical: 0.15,
            queue_depth_warning: 10,
            queue_depth_critical: 50,
        }
    }
}

impl PoolHealthMonitor {
    /// Create new health monitor
    pub fn new(pool: Arc<AdaptiveConnectionPool>, config: HealthMonitorConfig) -> Self {
        let (alert_sender, _) = broadcast::channel(1000);

        Self {
            config,
            pool,
            metrics_history: Arc::new(RwLock::new(VecDeque::new())),
            alert_sender,
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            monitor_task: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Start health monitoring
    pub async fn start(&self) -> Result<()> {
        info!("Starting pool health monitoring");

        let mut task_handle = self.monitor_task.write().await;
        if task_handle.is_some() {
            warn!("Health monitor already running");
            return Ok(());
        }

        let pool = self.pool.clone();
        let config = self.config.clone();
        let metrics_history = self.metrics_history.clone();
        let alert_sender = self.alert_sender.clone();
        let active_alerts = self.active_alerts.clone();
        let shutdown = self.shutdown.clone();

        let handle = tokio::spawn(async move {
            Self::monitoring_loop(
                pool,
                config,
                metrics_history,
                alert_sender,
                active_alerts,
                shutdown,
            )
            .await;
        });

        *task_handle = Some(handle);
        Ok(())
    }

    /// Stop health monitoring
    pub async fn stop(&self) {
        info!("Stopping pool health monitoring");
        *self.shutdown.write().await = true;

        if let Some(handle) = self.monitor_task.write().await.take() {
            let _ = handle.await;
        }
    }

    /// Get current health metrics
    pub async fn get_current_metrics(&self) -> Result<HealthMetrics> {
        self.collect_metrics().await
    }

    /// Get metrics history
    pub async fn get_metrics_history(&self) -> Vec<HealthMetrics> {
        self.metrics_history.read().await.iter().cloned().collect()
    }

    /// Get metrics for a specific time range
    pub async fn get_metrics_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<HealthMetrics> {
        let history = self.metrics_history.read().await;
        history
            .iter()
            .filter(|m| m.timestamp >= start && m.timestamp <= end)
            .cloned()
            .collect()
    }

    /// Subscribe to health alerts
    pub fn subscribe_to_alerts(&self) -> broadcast::Receiver<HealthAlert> {
        self.alert_sender.subscribe()
    }

    /// Get active alerts
    pub async fn get_active_alerts(&self) -> Vec<HealthAlert> {
        self.active_alerts.read().await.values().cloned().collect()
    }

    /// Main monitoring loop
    async fn monitoring_loop(
        pool: Arc<AdaptiveConnectionPool>,
        config: HealthMonitorConfig,
        metrics_history: Arc<RwLock<VecDeque<HealthMetrics>>>,
        alert_sender: broadcast::Sender<HealthAlert>,
        active_alerts: Arc<RwLock<HashMap<String, HealthAlert>>>,
        shutdown: Arc<RwLock<bool>>,
    ) {
        let mut interval = interval(TokioDuration::from_millis(
            config.check_interval.num_milliseconds() as u64,
        ));

        while !*shutdown.read().await {
            interval.tick().await;

            let monitor = PoolHealthMonitor {
                config: config.clone(),
                pool: pool.clone(),
                metrics_history: metrics_history.clone(),
                alert_sender: alert_sender.clone(),
                active_alerts: active_alerts.clone(),
                monitor_task: Arc::new(RwLock::new(None)),
                shutdown: shutdown.clone(),
            };

            if let Err(e) = monitor.perform_health_check().await {
                error!("Health check failed: {}", e);
            }
        }

        info!("Health monitoring loop stopped");
    }

    /// Perform a single health check cycle
    async fn perform_health_check(&self) -> Result<()> {
        let metrics = self.collect_metrics().await?;

        // Store metrics in history
        {
            let mut history = self.metrics_history.write().await;
            history.push_back(metrics.clone());

            // Trim history to max size
            while history.len() > self.config.max_history_size {
                history.pop_front();
            }
        }

        // Analyze metrics and generate alerts
        self.analyze_metrics_and_alert(&metrics).await?;

        debug!(
            "Health check completed - Status: {:?}",
            metrics.health_status
        );
        Ok(())
    }

    /// Collect comprehensive health metrics
    async fn collect_metrics(&self) -> Result<HealthMetrics> {
        let timestamp = Utc::now();

        // Collect pool statistics
        let pool_stats = self.pool.get_stats().await;
        let load_balancing_stats = self.pool.get_load_balancing_stats().await;

        // Collect connection-level metrics
        let connection_metrics = self.collect_connection_metrics().await?;

        // Collect system metrics
        let system_metrics = self.collect_system_metrics().await;

        // Calculate performance trends
        let performance_trends = self.calculate_performance_trends().await;

        // Determine overall health status
        let health_status = self
            .determine_health_status(&pool_stats, &connection_metrics)
            .await;

        // Get current active alerts
        let active_alerts = self.active_alerts.read().await.values().cloned().collect();

        Ok(HealthMetrics {
            timestamp,
            health_status,
            pool_stats,
            load_balancing_stats,
            connection_metrics,
            system_metrics,
            performance_trends,
            active_alerts,
        })
    }

    /// Collect metrics for individual connections
    async fn collect_connection_metrics(&self) -> Result<HashMap<String, ConnectionHealthMetrics>> {
        // This is a simplified implementation - in a real scenario,
        // we would need access to individual connection details
        let metrics = HashMap::new();

        // For now, return empty metrics as we'd need to refactor
        // the adaptive pool to expose individual connection details

        Ok(metrics)
    }

    /// Collect system resource metrics
    async fn collect_system_metrics(&self) -> SystemMetrics {
        SystemMetrics {
            memory_usage_bytes: 0, // Would use system metrics library
            cpu_usage_percent: None,
            network_connections: 0,
            thread_pool_utilization: None,
        }
    }

    /// Calculate performance trends from historical data
    async fn calculate_performance_trends(&self) -> PerformanceTrends {
        let history = self.metrics_history.read().await;
        let recent_history: Vec<_> = history
            .iter()
            .rev()
            .take(60) // Last 60 data points (30 minutes at 30-second intervals)
            .collect();

        let response_time_trend = recent_history
            .iter()
            .map(|m| TrendPoint {
                timestamp: m.timestamp,
                value: m.pool_stats.success_rate, // Placeholder - would be response time
            })
            .collect();

        let throughput_trend = recent_history
            .iter()
            .map(|m| TrendPoint {
                timestamp: m.timestamp,
                value: m.pool_stats.total_requests as f64,
            })
            .collect();

        let error_rate_trend = recent_history
            .iter()
            .map(|m| TrendPoint {
                timestamp: m.timestamp,
                value: 1.0 - m.pool_stats.success_rate,
            })
            .collect();

        let utilization_trend = recent_history
            .iter()
            .map(|m| TrendPoint {
                timestamp: m.timestamp,
                value: m.pool_stats.active_connections as f64,
            })
            .collect();

        PerformanceTrends {
            response_time_trend,
            throughput_trend,
            error_rate_trend,
            utilization_trend,
        }
    }

    /// Determine overall health status
    async fn determine_health_status(
        &self,
        pool_stats: &PoolStatistics,
        _connection_metrics: &HashMap<String, ConnectionHealthMetrics>,
    ) -> HealthStatus {
        let utilization = pool_stats.active_connections as f64 / 10.0; // Assuming max 10 connections
        let error_rate = 1.0 - pool_stats.success_rate;

        if utilization >= self.config.alert_thresholds.connection_utilization_critical
            || error_rate >= self.config.alert_thresholds.error_rate_critical
        {
            HealthStatus::Critical
        } else if utilization >= self.config.alert_thresholds.connection_utilization_warning
            || error_rate >= self.config.alert_thresholds.error_rate_warning
        {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        }
    }

    /// Analyze metrics and generate alerts
    async fn analyze_metrics_and_alert(&self, metrics: &HealthMetrics) -> Result<()> {
        let mut new_alerts = Vec::new();
        let mut resolved_alerts = Vec::new();

        // Check connection utilization
        let utilization = metrics.pool_stats.active_connections as f64 / 10.0; // Assuming max 10
        if utilization >= self.config.alert_thresholds.connection_utilization_critical {
            new_alerts.push(HealthAlert {
                severity: AlertSeverity::Critical,
                alert_type: AlertType::HighConnectionUtilization,
                message: format!(
                    "Connection utilization is critically high: {:.1}%",
                    utilization * 100.0
                ),
                value: utilization,
                threshold: self.config.alert_thresholds.connection_utilization_critical,
                timestamp: Utc::now(),
                context: HashMap::new(),
            });
        } else if utilization >= self.config.alert_thresholds.connection_utilization_warning {
            new_alerts.push(HealthAlert {
                severity: AlertSeverity::Warning,
                alert_type: AlertType::HighConnectionUtilization,
                message: format!(
                    "Connection utilization is high: {:.1}%",
                    utilization * 100.0
                ),
                value: utilization,
                threshold: self.config.alert_thresholds.connection_utilization_warning,
                timestamp: Utc::now(),
                context: HashMap::new(),
            });
        } else {
            resolved_alerts.push("HighConnectionUtilization".to_string());
        }

        // Check error rate
        let error_rate = 1.0 - metrics.pool_stats.success_rate;
        if error_rate >= self.config.alert_thresholds.error_rate_critical {
            new_alerts.push(HealthAlert {
                severity: AlertSeverity::Critical,
                alert_type: AlertType::HighErrorRate,
                message: format!("Error rate is critically high: {:.1}%", error_rate * 100.0),
                value: error_rate,
                threshold: self.config.alert_thresholds.error_rate_critical,
                timestamp: Utc::now(),
                context: HashMap::new(),
            });
        } else if error_rate >= self.config.alert_thresholds.error_rate_warning {
            new_alerts.push(HealthAlert {
                severity: AlertSeverity::Warning,
                alert_type: AlertType::HighErrorRate,
                message: format!("Error rate is high: {:.1}%", error_rate * 100.0),
                value: error_rate,
                threshold: self.config.alert_thresholds.error_rate_warning,
                timestamp: Utc::now(),
                context: HashMap::new(),
            });
        } else {
            resolved_alerts.push("HighErrorRate".to_string());
        }

        // Update active alerts
        {
            let mut active_alerts = self.active_alerts.write().await;

            // Remove resolved alerts
            for alert_key in resolved_alerts {
                if active_alerts.remove(&alert_key).is_some() {
                    info!("Alert resolved: {}", alert_key);
                }
            }

            // Add new alerts
            for alert in new_alerts {
                let alert_key = format!("{:?}", alert.alert_type);
                let is_new = !active_alerts.contains_key(&alert_key);

                active_alerts.insert(alert_key.clone(), alert.clone());

                if is_new {
                    warn!("New alert: {} - {}", alert_key, alert.message);
                    let _ = self.alert_sender.send(alert);
                }
            }
        }

        Ok(())
    }

    /// Generate health report
    pub async fn generate_health_report(&self) -> Result<String> {
        let metrics = self.get_current_metrics().await?;
        let active_alerts = self.get_active_alerts().await;

        let mut report = String::new();
        report.push_str("=== Pool Health Report ===\n");
        report.push_str(&format!(
            "Timestamp: {}\n",
            metrics.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        report.push_str(&format!("Health Status: {:?}\n", metrics.health_status));
        report.push_str(&format!(
            "Active Connections: {}\n",
            metrics.pool_stats.active_connections
        ));
        report.push_str(&format!(
            "Success Rate: {:.2}%\n",
            metrics.pool_stats.success_rate * 100.0
        ));
        report.push_str(&format!(
            "Total Requests: {}\n",
            metrics.pool_stats.total_requests
        ));
        report.push_str(&format!(
            "Failed Requests: {}\n",
            metrics.pool_stats.failed_requests
        ));

        if !active_alerts.is_empty() {
            report.push_str("\n=== Active Alerts ===\n");
            for alert in active_alerts {
                report.push_str(&format!(
                    "[{:?}] {:?}: {}\n",
                    alert.severity, alert.alert_type, alert.message
                ));
            }
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_monitor_config_default() {
        let config = HealthMonitorConfig::default();
        assert_eq!(config.check_interval, Duration::seconds(30));
        assert_eq!(config.max_history_size, 2880);
        assert!(config.detailed_metrics);
    }

    #[test]
    fn test_alert_thresholds_default() {
        let thresholds = AlertThresholds::default();
        assert_eq!(thresholds.connection_utilization_warning, 0.8);
        assert_eq!(thresholds.connection_utilization_critical, 0.95);
        assert_eq!(thresholds.error_rate_warning, 0.05);
        assert_eq!(thresholds.error_rate_critical, 0.15);
    }

    #[test]
    fn test_health_status_ordering() {
        assert_ne!(HealthStatus::Healthy, HealthStatus::Warning);
        assert_ne!(HealthStatus::Warning, HealthStatus::Critical);
        assert_ne!(HealthStatus::Critical, HealthStatus::Unavailable);
    }
}
