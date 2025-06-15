//! Performance reporting and alerting system

use crate::error::{LoxoneError, Result};
use crate::performance::{PerformanceMeasurement, PerformanceStatistics};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Performance reporter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReporterConfig {
    /// Enable performance reporting
    pub enabled: bool,
    /// Reporting destinations
    pub destinations: Vec<ReportDestination>,
    /// Report generation configuration
    pub report_generation: ReportGenerationConfig,
    /// Alert configuration
    pub alerting: AlertingConfig,
    /// Export configuration
    pub export: ExportConfig,
}

impl Default for ReporterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            destinations: vec![ReportDestination::Log],
            report_generation: ReportGenerationConfig::default(),
            alerting: AlertingConfig::default(),
            export: ExportConfig::default(),
        }
    }
}

impl ReporterConfig {
    /// Production configuration with comprehensive reporting
    pub fn production() -> Self {
        Self {
            enabled: true,
            destinations: vec![
                ReportDestination::Log,
                ReportDestination::File {
                    path: "/var/log/loxone-mcp/performance.log".to_string(),
                },
                ReportDestination::Metrics {
                    endpoint: "http://localhost:9090/api/v1/write".to_string(),
                },
            ],
            report_generation: ReportGenerationConfig::production(),
            alerting: AlertingConfig::production(),
            export: ExportConfig::production(),
        }
    }

    /// Development configuration with detailed reporting
    pub fn development() -> Self {
        Self {
            enabled: true,
            destinations: vec![ReportDestination::Log, ReportDestination::Console],
            report_generation: ReportGenerationConfig::development(),
            alerting: AlertingConfig::development(),
            export: ExportConfig::development(),
        }
    }

    /// Disabled configuration for testing
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            destinations: Vec::new(),
            report_generation: ReportGenerationConfig::minimal(),
            alerting: AlertingConfig::disabled(),
            export: ExportConfig::disabled(),
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.destinations.is_empty() {
                return Err(LoxoneError::invalid_input(
                    "At least one reporting destination must be configured",
                ));
            }

            for destination in &self.destinations {
                destination.validate()?;
            }

            self.report_generation.validate()?;
            self.alerting.validate()?;
            self.export.validate()?;
        }
        Ok(())
    }
}

/// Report destination configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportDestination {
    /// Log to application logs
    Log,
    /// Output to console
    Console,
    /// Write to file
    File { path: String },
    /// Send to metrics endpoint
    Metrics { endpoint: String },
    /// Send to webhook
    Webhook {
        url: String,
        headers: HashMap<String, String>,
    },
    /// Send email alerts
    Email { smtp_config: SmtpConfig },
    /// Send to Slack
    Slack { webhook_url: String },
}

impl ReportDestination {
    fn validate(&self) -> Result<()> {
        match self {
            ReportDestination::File { path } => {
                if path.is_empty() {
                    return Err(LoxoneError::invalid_input("File path cannot be empty"));
                }
            }
            ReportDestination::Metrics { endpoint } => {
                if endpoint.is_empty() {
                    return Err(LoxoneError::invalid_input(
                        "Metrics endpoint cannot be empty",
                    ));
                }
            }
            ReportDestination::Webhook { url, .. } => {
                if url.is_empty() {
                    return Err(LoxoneError::invalid_input("Webhook URL cannot be empty"));
                }
            }
            ReportDestination::Email { smtp_config } => {
                smtp_config.validate()?;
            }
            ReportDestination::Slack { webhook_url } => {
                if webhook_url.is_empty() {
                    return Err(LoxoneError::invalid_input(
                        "Slack webhook URL cannot be empty",
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// SMTP configuration for email alerts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub to_emails: Vec<String>,
    pub use_tls: bool,
}

impl SmtpConfig {
    fn validate(&self) -> Result<()> {
        if self.server.is_empty() {
            return Err(LoxoneError::invalid_input("SMTP server cannot be empty"));
        }
        if self.from_email.is_empty() {
            return Err(LoxoneError::invalid_input("From email cannot be empty"));
        }
        if self.to_emails.is_empty() {
            return Err(LoxoneError::invalid_input(
                "At least one recipient email must be configured",
            ));
        }
        Ok(())
    }
}

/// Report generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportGenerationConfig {
    /// Enable automatic report generation
    pub enabled: bool,
    /// Report generation interval
    pub interval: Duration,
    /// Report formats to generate
    pub formats: Vec<ReportFormat>,
    /// Include detailed breakdown
    pub include_breakdown: bool,
    /// Include historical trends
    pub include_trends: bool,
    /// Include recommendations
    pub include_recommendations: bool,
}

impl Default for ReportGenerationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(3600), // 1 hour
            formats: vec![ReportFormat::Json],
            include_breakdown: true,
            include_trends: false,
            include_recommendations: true,
        }
    }
}

impl ReportGenerationConfig {
    pub fn production() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(7200), // 2 hours
            formats: vec![ReportFormat::Json, ReportFormat::Prometheus],
            include_breakdown: true,
            include_trends: true,
            include_recommendations: true,
        }
    }

    pub fn development() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(300), // 5 minutes
            formats: vec![ReportFormat::Json, ReportFormat::Html],
            include_breakdown: true,
            include_trends: true,
            include_recommendations: true,
        }
    }

    pub fn minimal() -> Self {
        Self {
            enabled: false,
            interval: Duration::from_secs(3600),
            formats: vec![ReportFormat::Json],
            include_breakdown: false,
            include_trends: false,
            include_recommendations: false,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.interval.is_zero() {
                return Err(LoxoneError::invalid_input(
                    "Report generation interval cannot be zero",
                ));
            }
            if self.formats.is_empty() {
                return Err(LoxoneError::invalid_input(
                    "At least one report format must be specified",
                ));
            }
        }
        Ok(())
    }
}

/// Report format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportFormat {
    /// JSON format
    Json,
    /// HTML format
    Html,
    /// Prometheus metrics format
    Prometheus,
    /// CSV format
    Csv,
    /// Plain text format
    Text,
}

/// Alerting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingConfig {
    /// Enable alerting
    pub enabled: bool,
    /// Alert thresholds
    pub thresholds: AlertThresholds,
    /// Alert cooldown period
    pub cooldown_period: Duration,
    /// Maximum alerts per period
    pub max_alerts_per_period: u32,
    /// Alert suppression rules
    pub suppression_rules: Vec<SuppressionRule>,
}

impl Default for AlertingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            thresholds: AlertThresholds::default(),
            cooldown_period: Duration::from_secs(300), // 5 minutes
            max_alerts_per_period: 10,
            suppression_rules: Vec::new(),
        }
    }
}

impl AlertingConfig {
    pub fn production() -> Self {
        Self {
            enabled: true,
            thresholds: AlertThresholds::production(),
            cooldown_period: Duration::from_secs(900), // 15 minutes
            max_alerts_per_period: 5,
            suppression_rules: vec![SuppressionRule {
                name: "Suppress during maintenance".to_string(),
                condition: "maintenance_mode".to_string(),
                duration: Duration::from_secs(3600),
            }],
        }
    }

    pub fn development() -> Self {
        Self {
            enabled: true,
            thresholds: AlertThresholds::development(),
            cooldown_period: Duration::from_secs(60), // 1 minute
            max_alerts_per_period: 20,
            suppression_rules: Vec::new(),
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            thresholds: AlertThresholds::minimal(),
            cooldown_period: Duration::from_secs(1),
            max_alerts_per_period: 0,
            suppression_rules: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.cooldown_period.is_zero() {
                return Err(LoxoneError::invalid_input("Cooldown period cannot be zero"));
            }
            self.thresholds.validate()?;
        }
        Ok(())
    }
}

/// Alert thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// Response time threshold (ms)
    pub response_time_ms: u64,
    /// CPU usage threshold (percentage)
    pub cpu_usage_percent: f64,
    /// Memory usage threshold (bytes)
    pub memory_usage_bytes: u64,
    /// Error rate threshold (percentage)
    pub error_rate_percent: f64,
    /// Throughput threshold (requests per second)
    pub min_throughput_rps: f64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            response_time_ms: 5000,
            cpu_usage_percent: 85.0,
            memory_usage_bytes: 1024 * 1024 * 1024, // 1GB
            error_rate_percent: 5.0,
            min_throughput_rps: 0.1,
        }
    }
}

impl AlertThresholds {
    pub fn production() -> Self {
        Self {
            response_time_ms: 10000,
            cpu_usage_percent: 90.0,
            memory_usage_bytes: 2048 * 1024 * 1024, // 2GB
            error_rate_percent: 2.0,
            min_throughput_rps: 1.0,
        }
    }

    pub fn development() -> Self {
        Self {
            response_time_ms: 2000,
            cpu_usage_percent: 70.0,
            memory_usage_bytes: 512 * 1024 * 1024, // 512MB
            error_rate_percent: 10.0,
            min_throughput_rps: 0.05,
        }
    }

    pub fn minimal() -> Self {
        Self {
            response_time_ms: u64::MAX,
            cpu_usage_percent: 100.0,
            memory_usage_bytes: u64::MAX,
            error_rate_percent: 100.0,
            min_throughput_rps: 0.0,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.cpu_usage_percent <= 0.0 || self.cpu_usage_percent > 100.0 {
            return Err(LoxoneError::invalid_input(
                "CPU usage threshold must be between 0 and 100",
            ));
        }
        if self.error_rate_percent < 0.0 || self.error_rate_percent > 100.0 {
            return Err(LoxoneError::invalid_input(
                "Error rate threshold must be between 0 and 100",
            ));
        }
        Ok(())
    }
}

/// Alert suppression rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuppressionRule {
    /// Rule name
    pub name: String,
    /// Condition for suppression
    pub condition: String,
    /// Suppression duration
    pub duration: Duration,
}

/// Export configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    /// Enable data export
    pub enabled: bool,
    /// Export formats
    pub formats: Vec<ExportFormat>,
    /// Export destinations
    pub destinations: Vec<ExportDestination>,
    /// Data retention period
    pub retention_period: Duration,
    /// Export batch size
    pub batch_size: usize,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            formats: vec![ExportFormat::Json],
            destinations: Vec::new(),
            retention_period: Duration::from_secs(86400 * 7), // 7 days
            batch_size: 1000,
        }
    }
}

impl ExportConfig {
    pub fn production() -> Self {
        Self {
            enabled: true,
            formats: vec![ExportFormat::Json, ExportFormat::Parquet],
            destinations: vec![ExportDestination::S3 {
                bucket: "performance-data".to_string(),
                prefix: "loxone-mcp/".to_string(),
            }],
            retention_period: Duration::from_secs(86400 * 30), // 30 days
            batch_size: 5000,
        }
    }

    pub fn development() -> Self {
        Self {
            enabled: true,
            formats: vec![ExportFormat::Json],
            destinations: vec![ExportDestination::LocalFile {
                directory: "./performance-exports".to_string(),
            }],
            retention_period: Duration::from_secs(86400 * 3), // 3 days
            batch_size: 100,
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            formats: Vec::new(),
            destinations: Vec::new(),
            retention_period: Duration::from_secs(1),
            batch_size: 1,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.formats.is_empty() {
                return Err(LoxoneError::invalid_input(
                    "At least one export format must be specified",
                ));
            }
            if self.destinations.is_empty() {
                return Err(LoxoneError::invalid_input(
                    "At least one export destination must be specified",
                ));
            }
            if self.batch_size == 0 {
                return Err(LoxoneError::invalid_input(
                    "Export batch size cannot be zero",
                ));
            }
        }
        Ok(())
    }
}

/// Export format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Csv,
    Parquet,
    Avro,
}

/// Export destination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportDestination {
    LocalFile {
        directory: String,
    },
    S3 {
        bucket: String,
        prefix: String,
    },
    Database {
        connection_string: String,
    },
    HttpEndpoint {
        url: String,
        headers: HashMap<String, String>,
    },
}

/// Performance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    /// Report metadata
    pub metadata: ReportMetadata,
    /// Performance statistics
    pub statistics: PerformanceStatistics,
    /// Performance breakdown by operation
    pub operation_breakdown: HashMap<String, OperationStats>,
    /// Time series data
    pub time_series: Option<TimeSeriesData>,
    /// Recommendations
    pub recommendations: Vec<PerformanceRecommendation>,
    /// Alerts generated
    pub alerts: Vec<PerformanceAlert>,
}

/// Report metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    /// Report generation timestamp
    pub generated_at: u64,
    /// Report period start
    pub period_start: u64,
    /// Report period end
    pub period_end: u64,
    /// Report format
    pub format: ReportFormat,
    /// Data source
    pub data_source: String,
    /// Report version
    pub version: String,
}

/// Operation-specific statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStats {
    /// Operation name
    pub operation: String,
    /// Request count
    pub request_count: u64,
    /// Average response time
    pub avg_response_time_ms: f64,
    /// 95th percentile response time
    pub p95_response_time_ms: f64,
    /// Error count
    pub error_count: u64,
    /// Success rate
    pub success_rate: f64,
}

/// Time series data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesData {
    /// Data points
    pub data_points: Vec<TimeSeriesPoint>,
    /// Sampling interval
    pub interval_seconds: u64,
}

/// Time series data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    /// Timestamp
    pub timestamp: u64,
    /// Response time
    pub response_time_ms: f64,
    /// Request rate
    pub request_rate: f64,
    /// Error rate
    pub error_rate: f64,
    /// CPU usage
    pub cpu_usage: f64,
    /// Memory usage
    pub memory_usage: u64,
}

/// Performance recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRecommendation {
    /// Recommendation ID
    pub id: String,
    /// Recommendation title
    pub title: String,
    /// Description
    pub description: String,
    /// Priority level
    pub priority: RecommendationPriority,
    /// Expected impact
    pub expected_impact: String,
    /// Implementation effort
    pub effort: ImplementationEffort,
}

/// Recommendation priority
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

/// Implementation effort
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImplementationEffort {
    Low,
    Medium,
    High,
}

/// Performance alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlert {
    /// Alert ID
    pub id: String,
    /// Alert type
    pub alert_type: AlertType,
    /// Severity level
    pub severity: AlertSeverity,
    /// Alert message
    pub message: String,
    /// Metric value that triggered the alert
    pub metric_value: f64,
    /// Threshold that was exceeded
    pub threshold: f64,
    /// Timestamp when triggered
    pub triggered_at: u64,
}

/// Alert type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    ResponseTime,
    CpuUsage,
    MemoryUsage,
    ErrorRate,
    Throughput,
}

/// Alert severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Performance reporter
pub struct PerformanceReporter {
    config: ReporterConfig,
    alert_history: RwLock<Vec<PerformanceAlert>>,
    report_history: RwLock<Vec<PerformanceReport>>,
    last_report_time: RwLock<Option<Instant>>,
}

impl PerformanceReporter {
    /// Create new performance reporter
    pub fn new(config: ReporterConfig) -> Result<Self> {
        config.validate()?;

        Ok(Self {
            config,
            alert_history: RwLock::new(Vec::new()),
            report_history: RwLock::new(Vec::new()),
            last_report_time: RwLock::new(None),
        })
    }

    /// Report a performance measurement
    pub async fn report_measurement(&self, measurement: &PerformanceMeasurement) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Check for alerts
        if self.config.alerting.enabled {
            self.check_and_send_alerts(measurement).await?;
        }

        // Log measurement
        self.log_measurement(measurement).await?;

        // Check if it's time to generate a report
        if self.should_generate_report().await? {
            self.generate_and_send_report(measurement).await?;
        }

        Ok(())
    }

    /// Generate a performance report
    pub async fn generate_report(
        &self,
        statistics: &PerformanceStatistics,
    ) -> Result<PerformanceReport> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let period_end = now;
        let period_start = period_end - self.config.report_generation.interval.as_secs();

        let metadata = ReportMetadata {
            generated_at: now,
            period_start,
            period_end,
            format: ReportFormat::Json, // Default format
            data_source: "loxone-mcp-rust".to_string(),
            version: "1.0.0".to_string(),
        };

        // Generate operation breakdown (simplified)
        let mut operation_breakdown = HashMap::new();
        operation_breakdown.insert(
            "http_get".to_string(),
            OperationStats {
                operation: "http_get".to_string(),
                request_count: statistics.request_stats.total_requests / 3,
                avg_response_time_ms: statistics.request_stats.avg_response_time.as_millis() as f64,
                p95_response_time_ms: statistics.request_stats.p95_response_time.as_millis() as f64,
                error_count: statistics.error_stats.total_errors / 3,
                success_rate: statistics.request_stats.success_rate,
            },
        );

        operation_breakdown.insert(
            "http_post".to_string(),
            OperationStats {
                operation: "http_post".to_string(),
                request_count: statistics.request_stats.total_requests / 3,
                avg_response_time_ms: statistics.request_stats.avg_response_time.as_millis() as f64
                    * 1.2,
                p95_response_time_ms: statistics.request_stats.p95_response_time.as_millis() as f64
                    * 1.2,
                error_count: statistics.error_stats.total_errors / 3,
                success_rate: statistics.request_stats.success_rate - 1.0,
            },
        );

        // Generate recommendations
        let recommendations = self.generate_recommendations(statistics).await?;

        // Get recent alerts
        let alert_history = self.alert_history.read().await;
        let recent_alerts = alert_history
            .iter()
            .filter(|alert| alert.triggered_at >= period_start)
            .cloned()
            .collect();

        let report = PerformanceReport {
            metadata,
            statistics: statistics.clone(),
            operation_breakdown,
            time_series: None, // Would include if enabled
            recommendations,
            alerts: recent_alerts,
        };

        // Store report
        {
            let mut report_history = self.report_history.write().await;
            report_history.push(report.clone());

            // Keep only recent reports
            let max_reports = 100;
            if report_history.len() > max_reports {
                let excess = report_history.len() - max_reports;
                report_history.drain(0..excess);
            }
        }

        info!(
            "Generated performance report for period {}-{}",
            period_start, period_end
        );
        Ok(report)
    }

    /// Send report to configured destinations
    pub async fn send_report(&self, report: &PerformanceReport) -> Result<()> {
        for destination in &self.config.destinations {
            match destination {
                ReportDestination::Log => {
                    info!("Performance report: {:?}", report.metadata);
                }
                ReportDestination::Console => {
                    println!("=== Performance Report ===");
                    println!("Generated at: {}", report.metadata.generated_at);
                    println!(
                        "Period: {} - {}",
                        report.metadata.period_start, report.metadata.period_end
                    );
                    println!(
                        "Total requests: {}",
                        report.statistics.request_stats.total_requests
                    );
                    println!(
                        "Average response time: {:?}",
                        report.statistics.request_stats.avg_response_time
                    );
                    println!(
                        "Success rate: {:.1}%",
                        report.statistics.request_stats.success_rate
                    );
                    println!("Alerts: {}", report.alerts.len());
                    println!("Recommendations: {}", report.recommendations.len());
                }
                ReportDestination::File { path } => {
                    debug!("Would write report to file: {}", path);
                    // In a real implementation, would write to file
                }
                ReportDestination::Metrics { endpoint } => {
                    debug!("Would send metrics to endpoint: {}", endpoint);
                    // In a real implementation, would send to metrics endpoint
                }
                ReportDestination::Webhook { url, .. } => {
                    debug!("Would send report to webhook: {}", url);
                    // In a real implementation, would send HTTP request
                }
                ReportDestination::Email { .. } => {
                    debug!("Would send email report");
                    // In a real implementation, would send email
                }
                ReportDestination::Slack { webhook_url } => {
                    debug!("Would send Slack notification to: {}", webhook_url);
                    // In a real implementation, would send to Slack
                }
            }
        }

        Ok(())
    }

    // Private helper methods

    async fn check_and_send_alerts(&self, measurement: &PerformanceMeasurement) -> Result<()> {
        let mut alerts = Vec::new();

        // Check response time alert
        if let Some(duration) = measurement.timing.get_duration() {
            if duration.as_millis() as u64 > self.config.alerting.thresholds.response_time_ms {
                alerts.push(PerformanceAlert {
                    id: uuid::Uuid::new_v4().to_string(),
                    alert_type: AlertType::ResponseTime,
                    severity: AlertSeverity::Warning,
                    message: format!("High response time: {}ms", duration.as_millis()),
                    metric_value: duration.as_millis() as f64,
                    threshold: self.config.alerting.thresholds.response_time_ms as f64,
                    triggered_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                });
            }
        }

        // Check CPU usage alert
        if let Some(cpu) = measurement.resource_usage.cpu_usage {
            if cpu > self.config.alerting.thresholds.cpu_usage_percent {
                alerts.push(PerformanceAlert {
                    id: uuid::Uuid::new_v4().to_string(),
                    alert_type: AlertType::CpuUsage,
                    severity: AlertSeverity::Critical,
                    message: format!("High CPU usage: {:.1}%", cpu),
                    metric_value: cpu,
                    threshold: self.config.alerting.thresholds.cpu_usage_percent,
                    triggered_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                });
            }
        }

        // Send alerts
        for alert in alerts {
            self.send_alert(&alert).await?;

            // Store alert
            let mut alert_history = self.alert_history.write().await;
            alert_history.push(alert);

            // Keep only recent alerts
            let max_alerts = 1000;
            if alert_history.len() > max_alerts {
                let excess = alert_history.len() - max_alerts;
                alert_history.drain(0..excess);
            }
        }

        Ok(())
    }

    async fn send_alert(&self, alert: &PerformanceAlert) -> Result<()> {
        for destination in &self.config.destinations {
            match destination {
                ReportDestination::Log => {
                    warn!(
                        "Performance alert: {} - {}",
                        alert.message, alert.metric_value
                    );
                }
                ReportDestination::Console => {
                    eprintln!("ALERT: {} - {}", alert.message, alert.metric_value);
                }
                _ => {
                    debug!("Would send alert to destination: {:?}", destination);
                }
            }
        }

        Ok(())
    }

    async fn log_measurement(&self, measurement: &PerformanceMeasurement) -> Result<()> {
        let duration = measurement
            .timing
            .get_duration()
            .map(|d| d.as_millis())
            .unwrap_or(0);

        debug!(
            "Performance measurement: {} - {}ms - {} issues",
            measurement.context.operation_type,
            duration,
            measurement.issues.len()
        );

        Ok(())
    }

    async fn should_generate_report(&self) -> Result<bool> {
        if !self.config.report_generation.enabled {
            return Ok(false);
        }

        let last_report_time = self.last_report_time.read().await;

        match *last_report_time {
            Some(last_time) => Ok(last_time.elapsed() >= self.config.report_generation.interval),
            None => Ok(true), // First report
        }
    }

    async fn generate_and_send_report(&self, _measurement: &PerformanceMeasurement) -> Result<()> {
        // Generate a simple statistics object for the report
        let statistics = PerformanceStatistics::default(); // Would calculate actual stats

        let report = self.generate_report(&statistics).await?;
        self.send_report(&report).await?;

        // Update last report time
        {
            let mut last_report_time = self.last_report_time.write().await;
            *last_report_time = Some(Instant::now());
        }

        Ok(())
    }

    async fn generate_recommendations(
        &self,
        _statistics: &PerformanceStatistics,
    ) -> Result<Vec<PerformanceRecommendation>> {
        let mut recommendations = Vec::new();

        recommendations.push(PerformanceRecommendation {
            id: uuid::Uuid::new_v4().to_string(),
            title: "Implement response caching".to_string(),
            description: "Cache frequently requested data to reduce response times".to_string(),
            priority: RecommendationPriority::Medium,
            expected_impact: "30-70% reduction in response time for cached requests".to_string(),
            effort: ImplementationEffort::Medium,
        });

        recommendations.push(PerformanceRecommendation {
            id: uuid::Uuid::new_v4().to_string(),
            title: "Optimize database queries".to_string(),
            description: "Review and optimize slow database queries".to_string(),
            priority: RecommendationPriority::High,
            expected_impact: "20-50% reduction in query response time".to_string(),
            effort: ImplementationEffort::High,
        });

        Ok(recommendations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reporter_config_validation() {
        let config = ReporterConfig::production();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_alert_thresholds_validation() {
        let thresholds = AlertThresholds::production();
        assert!(thresholds.validate().is_ok());
    }

    #[tokio::test]
    async fn test_reporter_creation() {
        let config = ReporterConfig::development();
        let reporter = PerformanceReporter::new(config);
        assert!(reporter.is_ok());
    }

    #[tokio::test]
    async fn test_report_generation() {
        let config = ReporterConfig::development();
        let reporter = PerformanceReporter::new(config).unwrap();

        let statistics = PerformanceStatistics::default();
        let report = reporter.generate_report(&statistics).await.unwrap();

        assert!(!report.metadata.data_source.is_empty());
        assert!(report.metadata.generated_at > 0);
    }
}
