//! Real-time health monitoring and alerting

use super::{
    HealthChecker, HealthStatus, HealthReport, DiagnosticsCollector, DiagnosticSnapshot,
    DiagnosticTrends, TrendDirection,
};
use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, RwLock, Mutex};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Real-time health monitoring service
pub struct HealthMonitor {
    /// Health checker
    health_checker: Arc<HealthChecker>,
    /// Diagnostics collector
    diagnostics_collector: Arc<RwLock<DiagnosticsCollector>>,
    /// Monitoring configuration
    config: MonitoringConfig,
    /// Event broadcaster
    event_broadcaster: broadcast::Sender<HealthEvent>,
    /// Alert manager
    alert_manager: Arc<Mutex<AlertManager>>,
    /// Monitoring state
    state: Arc<RwLock<MonitoringState>>,
}

impl HealthMonitor {
    /// Create new health monitor
    pub fn new(
        health_checker: Arc<HealthChecker>,
        diagnostics_collector: Arc<RwLock<DiagnosticsCollector>>,
        config: MonitoringConfig,
    ) -> Self {
        let (event_broadcaster, _) = broadcast::channel(config.event_buffer_size);
        let alert_manager = Arc::new(Mutex::new(AlertManager::new(config.alert_config.clone())));
        let state = Arc::new(RwLock::new(MonitoringState::new()));

        Self {
            health_checker,
            diagnostics_collector,
            config,
            event_broadcaster,
            alert_manager,
            state,
        }
    }

    /// Start monitoring service
    pub async fn start(&self) -> Result<()> {
        info!("Starting health monitoring service");
        
        // Update monitoring state
        {
            let mut state = self.state.write().await;
            state.start_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            state.is_running = true;
        }

        // Start health check monitoring
        let health_monitor_task = self.start_health_monitoring();
        
        // Start diagnostics monitoring
        let diagnostics_monitor_task = self.start_diagnostics_monitoring();
        
        // Start alert processing
        let alert_processor_task = self.start_alert_processing();

        // Run all monitoring tasks concurrently
        tokio::select! {
            result = health_monitor_task => {
                error!("Health monitoring task ended: {:?}", result);
                result
            }
            result = diagnostics_monitor_task => {
                error!("Diagnostics monitoring task ended: {:?}", result);
                result
            }
            result = alert_processor_task => {
                error!("Alert processing task ended: {:?}", result);
                result
            }
        }
    }

    /// Stop monitoring service
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping health monitoring service");
        
        let mut state = self.state.write().await;
        state.is_running = false;
        state.stop_time = Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
        
        Ok(())
    }

    /// Subscribe to health events
    pub fn subscribe(&self) -> broadcast::Receiver<HealthEvent> {
        self.event_broadcaster.subscribe()
    }

    /// Get current monitoring status
    pub async fn get_status(&self) -> MonitoringStatus {
        let state = self.state.read().await;
        let alert_manager = self.alert_manager.lock().await;
        
        MonitoringStatus {
            is_running: state.is_running,
            uptime_seconds: if state.is_running {
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - state.start_time
            } else {
                state.stop_time.unwrap_or(0) - state.start_time
            },
            total_events: state.total_events,
            last_health_check: state.last_health_check,
            last_diagnostics_check: state.last_diagnostics_check,
            active_alerts: alert_manager.get_active_alerts().len(),
            config: self.config.clone(),
        }
    }

    /// Force a health check
    pub async fn trigger_health_check(&self) -> Result<HealthReport> {
        debug!("Triggering manual health check");
        let report = self.health_checker.check_health().await;
        
        // Process the report
        self.process_health_report(report.clone()).await?;
        
        Ok(report)
    }

    /// Start health check monitoring task
    async fn start_health_monitoring(&self) -> Result<()> {
        let mut interval = interval(self.config.health_check_interval);
        
        loop {
            interval.tick().await;
            
            // Check if monitoring is still running
            {
                let state = self.state.read().await;
                if !state.is_running {
                    break;
                }
            }
            
            debug!("Performing scheduled health check");
            
            let report = self.health_checker.check_health().await;
            if let Err(e) = self.process_health_report(report).await {
                warn!("Failed to process health report: {}", e);
                
                let event = HealthEvent {
                    event_type: HealthEventType::HealthCheckFailed,
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    data: serde_json::json!({
                        "error": e.to_string()
                    }),
                    severity: EventSeverity::Critical,
                };
                
                if let Err(_) = self.event_broadcaster.send(event) {
                    warn!("No subscribers for health events");
                }
            }
        }
        
        Ok(())
    }

    /// Start diagnostics monitoring task
    async fn start_diagnostics_monitoring(&self) -> Result<()> {
        let mut interval = interval(self.config.diagnostics_interval);
        
        loop {
            interval.tick().await;
            
            // Check if monitoring is still running
            {
                let state = self.state.read().await;
                if !state.is_running {
                    break;
                }
            }
            
            debug!("Collecting diagnostics");
            
            let mut collector = self.diagnostics_collector.write().await;
            match collector.collect_diagnostics().await {
                Ok(snapshot) => {
                    let trends = collector.calculate_trends();
                    drop(collector);
                    
                    if let Err(e) = self.process_diagnostics(snapshot, trends).await {
                        warn!("Failed to process diagnostics: {}", e);
                    }
                }
                Err(e) => {
                    error!("Diagnostics collection failed: {}", e);
                }
            }
        }
        
        Ok(())
    }

    /// Start alert processing task
    async fn start_alert_processing(&self) -> Result<()> {
        let mut interval = interval(self.config.alert_check_interval);
        
        loop {
            interval.tick().await;
            
            // Check if monitoring is still running
            {
                let state = self.state.read().await;
                if !state.is_running {
                    break;
                }
            }
            
            // Process alerts
            let mut alert_manager = self.alert_manager.lock().await;
            if let Err(e) = alert_manager.process_alerts().await {
                warn!("Alert processing failed: {}", e);
            }
        }
        
        Ok(())
    }

    /// Process health report and emit events
    async fn process_health_report(&self, report: HealthReport) -> Result<()> {
        // Update monitoring state
        {
            let mut state = self.state.write().await;
            state.last_health_check = Some(report.timestamp);
            state.total_events += 1;
        }

        // Emit health status event
        let event = HealthEvent {
            event_type: HealthEventType::HealthStatusChanged,
            timestamp: report.timestamp,
            data: serde_json::to_value(&report)
                .map_err(|e| LoxoneError::internal(format!("Failed to serialize health report: {}", e)))?,
            severity: match report.status {
                HealthStatus::Healthy => EventSeverity::Info,
                HealthStatus::Warning => EventSeverity::Warning,
                HealthStatus::Degraded => EventSeverity::Warning,
                HealthStatus::Unhealthy => EventSeverity::Critical,
                HealthStatus::Starting => EventSeverity::Info,
                HealthStatus::Stopping => EventSeverity::Warning,
            },
        };

        if let Err(_) = self.event_broadcaster.send(event) {
            debug!("No subscribers for health events");
        }

        // Check for alerts
        let mut alert_manager = self.alert_manager.lock().await;
        alert_manager.evaluate_health_alerts(&report).await?;

        Ok(())
    }

    /// Process diagnostics and emit events
    async fn process_diagnostics(&self, snapshot: DiagnosticSnapshot, trends: Option<DiagnosticTrends>) -> Result<()> {
        // Update monitoring state
        {
            let mut state = self.state.write().await;
            state.last_diagnostics_check = Some(snapshot.timestamp);
        }

        // Emit diagnostics event
        let event = HealthEvent {
            event_type: HealthEventType::DiagnosticsUpdated,
            timestamp: snapshot.timestamp,
            data: serde_json::json!({
                "snapshot": snapshot,
                "trends": trends
            }),
            severity: EventSeverity::Info,
        };

        if let Err(_) = self.event_broadcaster.send(event) {
            debug!("No subscribers for health events");
        }

        // Check for diagnostic alerts
        let mut alert_manager = self.alert_manager.lock().await;
        alert_manager.evaluate_diagnostic_alerts(&snapshot, trends.as_ref()).await?;

        Ok(())
    }
}

/// Alert manager for handling health alerts
pub struct AlertManager {
    /// Alert configuration
    config: AlertConfig,
    /// Active alerts
    active_alerts: HashMap<String, Alert>,
    /// Alert history
    alert_history: Vec<Alert>,
}

impl AlertManager {
    /// Create new alert manager
    pub fn new(config: AlertConfig) -> Self {
        Self {
            config,
            active_alerts: HashMap::new(),
            alert_history: Vec::new(),
        }
    }

    /// Evaluate health-based alerts
    pub async fn evaluate_health_alerts(&mut self, report: &HealthReport) -> Result<()> {
        // Check for critical health status
        if matches!(report.status, HealthStatus::Unhealthy) {
            self.trigger_alert(
                "critical_health_status".to_string(),
                AlertSeverity::Critical,
                format!("System health is critical: {:?}", report.status),
                serde_json::to_value(report).unwrap_or_default(),
            ).await?;
        } else {
            self.resolve_alert("critical_health_status").await?;
        }

        // Check for too many failures
        if report.summary.critical_failures > self.config.max_critical_failures {
            self.trigger_alert(
                "too_many_failures".to_string(),
                AlertSeverity::Warning,
                format!("Too many critical failures: {}", report.summary.critical_failures),
                serde_json::json!({
                    "critical_failures": report.summary.critical_failures,
                    "max_allowed": self.config.max_critical_failures
                }),
            ).await?;
        } else {
            self.resolve_alert("too_many_failures").await?;
        }

        Ok(())
    }

    /// Evaluate diagnostic-based alerts
    pub async fn evaluate_diagnostic_alerts(
        &mut self,
        snapshot: &DiagnosticSnapshot,
        trends: Option<&DiagnosticTrends>,
    ) -> Result<()> {
        // Check memory usage
        if snapshot.system_info.memory.usage_percent > self.config.memory_threshold {
            self.trigger_alert(
                "high_memory_usage".to_string(),
                AlertSeverity::Warning,
                format!("High memory usage: {:.1}%", snapshot.system_info.memory.usage_percent),
                serde_json::json!({
                    "usage_percent": snapshot.system_info.memory.usage_percent,
                    "threshold": self.config.memory_threshold
                }),
            ).await?;
        } else {
            self.resolve_alert("high_memory_usage").await?;
        }

        // Check CPU usage
        if snapshot.system_info.cpu.usage_percent > self.config.cpu_threshold {
            self.trigger_alert(
                "high_cpu_usage".to_string(),
                AlertSeverity::Warning,
                format!("High CPU usage: {:.1}%", snapshot.system_info.cpu.usage_percent),
                serde_json::json!({
                    "usage_percent": snapshot.system_info.cpu.usage_percent,
                    "threshold": self.config.cpu_threshold
                }),
            ).await?;
        } else {
            self.resolve_alert("high_cpu_usage").await?;
        }

        // Check disk usage
        if let Some(fs) = snapshot.disk_info.filesystems.first() {
            if fs.usage_percent > self.config.disk_threshold {
                self.trigger_alert(
                    "high_disk_usage".to_string(),
                    AlertSeverity::Warning,
                    format!("High disk usage: {:.1}% on {}", fs.usage_percent, fs.mount_point),
                    serde_json::json!({
                        "usage_percent": fs.usage_percent,
                        "threshold": self.config.disk_threshold,
                        "mount_point": fs.mount_point
                    }),
                ).await?;
            } else {
                self.resolve_alert("high_disk_usage").await?;
            }
        }

        // Check trends for degrading performance
        if let Some(trends) = trends {
            if matches!(trends.memory_usage_trend, TrendDirection::Increasing) &&
               matches!(trends.cpu_usage_trend, TrendDirection::Increasing) {
                self.trigger_alert(
                    "performance_degradation".to_string(),
                    AlertSeverity::Info,
                    "System performance is degrading".to_string(),
                    serde_json::to_value(trends).unwrap_or_default(),
                ).await?;
            } else {
                self.resolve_alert("performance_degradation").await?;
            }
        }

        Ok(())
    }

    /// Trigger an alert
    async fn trigger_alert(
        &mut self,
        alert_id: String,
        severity: AlertSeverity,
        message: String,
        data: serde_json::Value,
    ) -> Result<()> {
        let alert = Alert {
            id: alert_id.clone(),
            severity,
            message,
            data,
            triggered_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            resolved_at: None,
            count: self.active_alerts.get(&alert_id)
                .map(|a| a.count + 1)
                .unwrap_or(1),
        };

        info!("Alert triggered: {} - {}", alert_id, alert.message);
        
        self.active_alerts.insert(alert_id, alert.clone());
        self.alert_history.push(alert);

        // Keep alert history size manageable
        if self.alert_history.len() > self.config.max_alert_history {
            self.alert_history.remove(0);
        }

        Ok(())
    }

    /// Resolve an alert
    async fn resolve_alert(&mut self, alert_id: &str) -> Result<()> {
        if let Some(mut alert) = self.active_alerts.remove(alert_id) {
            alert.resolved_at = Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
            info!("Alert resolved: {} - {}", alert_id, alert.message);
        }
        Ok(())
    }

    /// Process alerts (cleanup, notifications, etc.)
    pub async fn process_alerts(&mut self) -> Result<()> {
        // In a real implementation, this would send notifications,
        // clean up old alerts, etc.
        Ok(())
    }

    /// Get active alerts
    pub fn get_active_alerts(&self) -> Vec<&Alert> {
        self.active_alerts.values().collect()
    }

    /// Get alert history
    pub fn get_alert_history(&self) -> &[Alert] {
        &self.alert_history
    }
}

/// Monitoring configuration
#[derive(Debug, Clone, serde::Serialize)]
pub struct MonitoringConfig {
    /// Health check interval
    pub health_check_interval: Duration,
    /// Diagnostics collection interval
    pub diagnostics_interval: Duration,
    /// Alert checking interval
    pub alert_check_interval: Duration,
    /// Event buffer size
    pub event_buffer_size: usize,
    /// Alert configuration
    pub alert_config: AlertConfig,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            health_check_interval: Duration::from_secs(30),
            diagnostics_interval: Duration::from_secs(60),
            alert_check_interval: Duration::from_secs(10),
            event_buffer_size: 1000,
            alert_config: AlertConfig::default(),
        }
    }
}

/// Alert configuration
#[derive(Debug, Clone, serde::Serialize)]
pub struct AlertConfig {
    /// Memory usage threshold for alerts (percentage)
    pub memory_threshold: f64,
    /// CPU usage threshold for alerts (percentage)
    pub cpu_threshold: f64,
    /// Disk usage threshold for alerts (percentage)
    pub disk_threshold: f64,
    /// Maximum allowed critical failures
    pub max_critical_failures: usize,
    /// Maximum alert history to keep
    pub max_alert_history: usize,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            memory_threshold: 85.0,
            cpu_threshold: 80.0,
            disk_threshold: 90.0,
            max_critical_failures: 3,
            max_alert_history: 1000,
        }
    }
}

/// Health event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthEvent {
    /// Type of event
    pub event_type: HealthEventType,
    /// Event timestamp
    pub timestamp: u64,
    /// Event data
    pub data: serde_json::Value,
    /// Event severity
    pub severity: EventSeverity,
}

/// Health event type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthEventType {
    /// Health status changed
    HealthStatusChanged,
    /// Health check failed
    HealthCheckFailed,
    /// Diagnostics updated
    DiagnosticsUpdated,
    /// Alert triggered
    AlertTriggered,
    /// Alert resolved
    AlertResolved,
}

/// Event severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSeverity {
    /// Informational
    Info,
    /// Warning
    Warning,
    /// Critical
    Critical,
}

/// Alert information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Unique alert ID
    pub id: String,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Alert message
    pub message: String,
    /// Alert data
    pub data: serde_json::Value,
    /// When alert was triggered
    pub triggered_at: u64,
    /// When alert was resolved (if resolved)
    pub resolved_at: Option<u64>,
    /// Number of times this alert has been triggered
    pub count: u32,
}

/// Alert severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Informational alert
    Info,
    /// Warning alert
    Warning,
    /// Critical alert
    Critical,
}

/// Monitoring state
#[derive(Debug)]
struct MonitoringState {
    /// Whether monitoring is running
    is_running: bool,
    /// Start time
    start_time: u64,
    /// Stop time
    stop_time: Option<u64>,
    /// Total events processed
    total_events: u64,
    /// Last health check timestamp
    last_health_check: Option<u64>,
    /// Last diagnostics check timestamp
    last_diagnostics_check: Option<u64>,
}

impl MonitoringState {
    fn new() -> Self {
        Self {
            is_running: false,
            start_time: 0,
            stop_time: None,
            total_events: 0,
            last_health_check: None,
            last_diagnostics_check: None,
        }
    }
}

/// Monitoring status
#[derive(Debug, Serialize)]
pub struct MonitoringStatus {
    /// Whether monitoring is running
    pub is_running: bool,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Total events processed
    pub total_events: u64,
    /// Last health check timestamp
    pub last_health_check: Option<u64>,
    /// Last diagnostics check timestamp
    pub last_diagnostics_check: Option<u64>,
    /// Number of active alerts
    pub active_alerts: usize,
    /// Configuration
    pub config: MonitoringConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::checks::MemoryHealthCheck;

    #[tokio::test]
    async fn test_health_monitor_creation() {
        let health_checker = Arc::new(HealthChecker::new());
        let diagnostics_collector = Arc::new(RwLock::new(DiagnosticsCollector::default()));
        let config = MonitoringConfig::default();
        
        let monitor = HealthMonitor::new(health_checker, diagnostics_collector, config);
        let status = monitor.get_status().await;
        
        assert!(!status.is_running);
        assert_eq!(status.total_events, 0);
    }

    #[tokio::test]
    async fn test_alert_manager() {
        let mut alert_manager = AlertManager::new(AlertConfig::default());
        
        // Trigger an alert
        alert_manager.trigger_alert(
            "test_alert".to_string(),
            AlertSeverity::Warning,
            "Test alert message".to_string(),
            serde_json::json!({"test": "data"}),
        ).await.unwrap();
        
        let active_alerts = alert_manager.get_active_alerts();
        assert_eq!(active_alerts.len(), 1);
        assert_eq!(active_alerts[0].id, "test_alert");
        
        // Resolve the alert
        alert_manager.resolve_alert("test_alert").await.unwrap();
        let active_alerts = alert_manager.get_active_alerts();
        assert_eq!(active_alerts.len(), 0);
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let health_checker = Arc::new(HealthChecker::new());
        let diagnostics_collector = Arc::new(RwLock::new(DiagnosticsCollector::default()));
        let config = MonitoringConfig::default();
        
        let monitor = HealthMonitor::new(health_checker, diagnostics_collector, config);
        let mut receiver = monitor.subscribe();
        
        // This would normally be done by the monitoring loop
        let event = HealthEvent {
            event_type: HealthEventType::HealthStatusChanged,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            data: serde_json::json!({"test": "event"}),
            severity: EventSeverity::Info,
        };
        
        monitor.event_broadcaster.send(event).unwrap();
        
        let received_event = receiver.recv().await.unwrap();
        assert!(matches!(received_event.event_type, HealthEventType::HealthStatusChanged));
    }
}