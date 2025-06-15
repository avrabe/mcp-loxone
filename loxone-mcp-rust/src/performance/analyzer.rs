//! Performance analysis and trending system

use crate::error::{LoxoneError, Result};
use crate::performance::{
    ErrorStatistics, PerformanceMeasurement, PerformanceStatistics, PerformanceTrends,
    RequestStatistics, ResourceStatistics,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::info;

/// Performance analyzer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzerConfig {
    /// Enable performance analysis
    pub enabled: bool,
    /// Analysis window duration
    pub analysis_window: Duration,
    /// Trend detection configuration
    pub trend_detection: TrendDetectionConfig,
    /// Anomaly detection configuration
    pub anomaly_detection: AnomalyDetectionConfig,
    /// Baseline calculation configuration
    pub baseline_calculation: BaselineConfig,
    /// Alert thresholds
    pub alert_thresholds: AlertThresholds,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            analysis_window: Duration::from_secs(300), // 5 minutes
            trend_detection: TrendDetectionConfig::default(),
            anomaly_detection: AnomalyDetectionConfig::default(),
            baseline_calculation: BaselineConfig::default(),
            alert_thresholds: AlertThresholds::default(),
        }
    }
}

impl AnalyzerConfig {
    /// Production configuration with conservative analysis
    pub fn production() -> Self {
        Self {
            enabled: true,
            analysis_window: Duration::from_secs(600), // 10 minutes
            trend_detection: TrendDetectionConfig::production(),
            anomaly_detection: AnomalyDetectionConfig::production(),
            baseline_calculation: BaselineConfig::production(),
            alert_thresholds: AlertThresholds::production(),
        }
    }

    /// Development configuration with sensitive analysis
    pub fn development() -> Self {
        Self {
            enabled: true,
            analysis_window: Duration::from_secs(60), // 1 minute
            trend_detection: TrendDetectionConfig::development(),
            anomaly_detection: AnomalyDetectionConfig::development(),
            baseline_calculation: BaselineConfig::development(),
            alert_thresholds: AlertThresholds::development(),
        }
    }

    /// Minimal configuration for testing
    pub fn minimal() -> Self {
        Self {
            enabled: false,
            analysis_window: Duration::from_secs(30),
            trend_detection: TrendDetectionConfig::disabled(),
            anomaly_detection: AnomalyDetectionConfig::disabled(),
            baseline_calculation: BaselineConfig::minimal(),
            alert_thresholds: AlertThresholds::minimal(),
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.analysis_window.is_zero() {
                return Err(LoxoneError::invalid_input("Analysis window cannot be zero"));
            }

            self.trend_detection.validate()?;
            self.anomaly_detection.validate()?;
            self.baseline_calculation.validate()?;
            self.alert_thresholds.validate()?;
        }
        Ok(())
    }
}

/// Trend detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendDetectionConfig {
    /// Enable trend detection
    pub enabled: bool,
    /// Minimum data points for trend analysis
    pub min_data_points: usize,
    /// Trend significance threshold
    pub significance_threshold: f64,
    /// Moving average window size
    pub moving_average_window: usize,
    /// Seasonal adjustment
    pub seasonal_adjustment: bool,
}

impl Default for TrendDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_data_points: 10,
            significance_threshold: 0.1,
            moving_average_window: 5,
            seasonal_adjustment: false,
        }
    }
}

impl TrendDetectionConfig {
    pub fn production() -> Self {
        Self {
            enabled: true,
            min_data_points: 20,
            significance_threshold: 0.05,
            moving_average_window: 10,
            seasonal_adjustment: true,
        }
    }

    pub fn development() -> Self {
        Self {
            enabled: true,
            min_data_points: 5,
            significance_threshold: 0.2,
            moving_average_window: 3,
            seasonal_adjustment: false,
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            min_data_points: 0,
            significance_threshold: 1.0,
            moving_average_window: 1,
            seasonal_adjustment: false,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.min_data_points == 0 {
                return Err(LoxoneError::invalid_input("Min data points cannot be zero"));
            }
            if self.moving_average_window == 0 {
                return Err(LoxoneError::invalid_input(
                    "Moving average window cannot be zero",
                ));
            }
            if self.significance_threshold <= 0.0 || self.significance_threshold >= 1.0 {
                return Err(LoxoneError::invalid_input(
                    "Significance threshold must be between 0 and 1",
                ));
            }
        }
        Ok(())
    }
}

/// Anomaly detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetectionConfig {
    /// Enable anomaly detection
    pub enabled: bool,
    /// Statistical method for anomaly detection
    pub method: AnomalyDetectionMethod,
    /// Sensitivity level (1-10, higher = more sensitive)
    pub sensitivity: u8,
    /// Minimum confidence for anomaly classification
    pub min_confidence: f64,
    /// Lookback window for baseline calculation
    pub lookback_window: Duration,
}

impl Default for AnomalyDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            method: AnomalyDetectionMethod::StandardDeviation,
            sensitivity: 5,
            min_confidence: 0.8,
            lookback_window: Duration::from_secs(3600), // 1 hour
        }
    }
}

impl AnomalyDetectionConfig {
    pub fn production() -> Self {
        Self {
            enabled: true,
            method: AnomalyDetectionMethod::Percentile,
            sensitivity: 3,
            min_confidence: 0.9,
            lookback_window: Duration::from_secs(7200), // 2 hours
        }
    }

    pub fn development() -> Self {
        Self {
            enabled: true,
            method: AnomalyDetectionMethod::StandardDeviation,
            sensitivity: 7,
            min_confidence: 0.7,
            lookback_window: Duration::from_secs(1800), // 30 minutes
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            method: AnomalyDetectionMethod::StandardDeviation,
            sensitivity: 1,
            min_confidence: 1.0,
            lookback_window: Duration::from_secs(1),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.sensitivity == 0 || self.sensitivity > 10 {
                return Err(LoxoneError::invalid_input(
                    "Sensitivity must be between 1 and 10",
                ));
            }
            if self.min_confidence <= 0.0 || self.min_confidence > 1.0 {
                return Err(LoxoneError::invalid_input(
                    "Min confidence must be between 0 and 1",
                ));
            }
            if self.lookback_window.is_zero() {
                return Err(LoxoneError::invalid_input("Lookback window cannot be zero"));
            }
        }
        Ok(())
    }
}

/// Anomaly detection method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyDetectionMethod {
    /// Standard deviation based detection
    StandardDeviation,
    /// Percentile based detection
    Percentile,
    /// Isolation forest algorithm
    IsolationForest,
    /// Z-score based detection
    ZScore,
}

/// Baseline calculation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineConfig {
    /// Enable baseline calculation
    pub enabled: bool,
    /// Baseline calculation method
    pub method: BaselineMethod,
    /// Baseline window duration
    pub baseline_window: Duration,
    /// Update frequency
    pub update_frequency: Duration,
    /// Warmup period before baselines are considered stable
    pub warmup_period: Duration,
}

impl Default for BaselineConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            method: BaselineMethod::MovingAverage,
            baseline_window: Duration::from_secs(3600), // 1 hour
            update_frequency: Duration::from_secs(300), // 5 minutes
            warmup_period: Duration::from_secs(900),    // 15 minutes
        }
    }
}

impl BaselineConfig {
    pub fn production() -> Self {
        Self {
            enabled: true,
            method: BaselineMethod::ExponentialSmoothing,
            baseline_window: Duration::from_secs(7200), // 2 hours
            update_frequency: Duration::from_secs(600), // 10 minutes
            warmup_period: Duration::from_secs(1800),   // 30 minutes
        }
    }

    pub fn development() -> Self {
        Self {
            enabled: true,
            method: BaselineMethod::MovingAverage,
            baseline_window: Duration::from_secs(1800), // 30 minutes
            update_frequency: Duration::from_secs(60),  // 1 minute
            warmup_period: Duration::from_secs(300),    // 5 minutes
        }
    }

    pub fn minimal() -> Self {
        Self {
            enabled: false,
            method: BaselineMethod::Simple,
            baseline_window: Duration::from_secs(300), // 5 minutes
            update_frequency: Duration::from_secs(60), // 1 minute
            warmup_period: Duration::from_secs(60),    // 1 minute
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.baseline_window.is_zero() {
                return Err(LoxoneError::invalid_input("Baseline window cannot be zero"));
            }
            if self.update_frequency.is_zero() {
                return Err(LoxoneError::invalid_input(
                    "Update frequency cannot be zero",
                ));
            }
            if self.warmup_period.is_zero() {
                return Err(LoxoneError::invalid_input("Warmup period cannot be zero"));
            }
        }
        Ok(())
    }
}

/// Baseline calculation method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BaselineMethod {
    /// Simple average
    Simple,
    /// Moving average
    MovingAverage,
    /// Exponential smoothing
    ExponentialSmoothing,
    /// Weighted average
    WeightedAverage,
}

/// Alert thresholds configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlertThresholds {
    /// Response time thresholds
    pub response_time: ResponseTimeThresholds,
    /// Throughput thresholds
    pub throughput: ThroughputThresholds,
    /// Error rate thresholds
    pub error_rate: ErrorRateThresholds,
    /// Resource usage thresholds
    pub resource_usage: ResourceUsageThresholds,
}

impl AlertThresholds {
    pub fn production() -> Self {
        Self {
            response_time: ResponseTimeThresholds::production(),
            throughput: ThroughputThresholds::production(),
            error_rate: ErrorRateThresholds::production(),
            resource_usage: ResourceUsageThresholds::production(),
        }
    }

    pub fn development() -> Self {
        Self {
            response_time: ResponseTimeThresholds::development(),
            throughput: ThroughputThresholds::development(),
            error_rate: ErrorRateThresholds::development(),
            resource_usage: ResourceUsageThresholds::development(),
        }
    }

    pub fn minimal() -> Self {
        Self {
            response_time: ResponseTimeThresholds::minimal(),
            throughput: ThroughputThresholds::minimal(),
            error_rate: ErrorRateThresholds::minimal(),
            resource_usage: ResourceUsageThresholds::minimal(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        self.response_time.validate()?;
        self.throughput.validate()?;
        self.error_rate.validate()?;
        self.resource_usage.validate()?;
        Ok(())
    }
}

/// Response time alert thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseTimeThresholds {
    pub warning: Duration,
    pub critical: Duration,
}

impl Default for ResponseTimeThresholds {
    fn default() -> Self {
        Self {
            warning: Duration::from_millis(1000),
            critical: Duration::from_millis(5000),
        }
    }
}

impl ResponseTimeThresholds {
    pub fn production() -> Self {
        Self {
            warning: Duration::from_millis(2000),
            critical: Duration::from_millis(10000),
        }
    }

    pub fn development() -> Self {
        Self {
            warning: Duration::from_millis(500),
            critical: Duration::from_millis(2000),
        }
    }

    pub fn minimal() -> Self {
        Self {
            warning: Duration::from_secs(10),
            critical: Duration::from_secs(30),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.warning >= self.critical {
            return Err(LoxoneError::invalid_input(
                "Warning threshold must be less than critical threshold",
            ));
        }
        Ok(())
    }
}

/// Throughput alert thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputThresholds {
    pub min_rps_warning: f64,
    pub min_rps_critical: f64,
}

impl Default for ThroughputThresholds {
    fn default() -> Self {
        Self {
            min_rps_warning: 1.0,
            min_rps_critical: 0.1,
        }
    }
}

impl ThroughputThresholds {
    pub fn production() -> Self {
        Self {
            min_rps_warning: 10.0,
            min_rps_critical: 1.0,
        }
    }

    pub fn development() -> Self {
        Self {
            min_rps_warning: 0.5,
            min_rps_critical: 0.05,
        }
    }

    pub fn minimal() -> Self {
        Self {
            min_rps_warning: 0.01,
            min_rps_critical: 0.001,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.min_rps_warning <= self.min_rps_critical {
            return Err(LoxoneError::invalid_input(
                "Warning threshold must be greater than critical threshold for throughput",
            ));
        }
        Ok(())
    }
}

/// Error rate alert thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRateThresholds {
    pub warning_percent: f64,
    pub critical_percent: f64,
}

impl Default for ErrorRateThresholds {
    fn default() -> Self {
        Self {
            warning_percent: 1.0,
            critical_percent: 5.0,
        }
    }
}

impl ErrorRateThresholds {
    pub fn production() -> Self {
        Self {
            warning_percent: 0.5,
            critical_percent: 2.0,
        }
    }

    pub fn development() -> Self {
        Self {
            warning_percent: 2.0,
            critical_percent: 10.0,
        }
    }

    pub fn minimal() -> Self {
        Self {
            warning_percent: 10.0,
            critical_percent: 50.0,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.warning_percent >= self.critical_percent {
            return Err(LoxoneError::invalid_input(
                "Warning threshold must be less than critical threshold",
            ));
        }
        Ok(())
    }
}

/// Resource usage alert thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageThresholds {
    pub cpu_warning_percent: f64,
    pub cpu_critical_percent: f64,
    pub memory_warning_bytes: u64,
    pub memory_critical_bytes: u64,
}

impl Default for ResourceUsageThresholds {
    fn default() -> Self {
        Self {
            cpu_warning_percent: 70.0,
            cpu_critical_percent: 90.0,
            memory_warning_bytes: 512 * 1024 * 1024, // 512MB
            memory_critical_bytes: 1024 * 1024 * 1024, // 1GB
        }
    }
}

impl ResourceUsageThresholds {
    pub fn production() -> Self {
        Self {
            cpu_warning_percent: 80.0,
            cpu_critical_percent: 95.0,
            memory_warning_bytes: 1024 * 1024 * 1024,  // 1GB
            memory_critical_bytes: 2048 * 1024 * 1024, // 2GB
        }
    }

    pub fn development() -> Self {
        Self {
            cpu_warning_percent: 60.0,
            cpu_critical_percent: 80.0,
            memory_warning_bytes: 256 * 1024 * 1024,  // 256MB
            memory_critical_bytes: 512 * 1024 * 1024, // 512MB
        }
    }

    pub fn minimal() -> Self {
        Self {
            cpu_warning_percent: 90.0,
            cpu_critical_percent: 99.0,
            memory_warning_bytes: 2048 * 1024 * 1024,  // 2GB
            memory_critical_bytes: 4096 * 1024 * 1024, // 4GB
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.cpu_warning_percent >= self.cpu_critical_percent {
            return Err(LoxoneError::invalid_input(
                "CPU warning threshold must be less than critical threshold",
            ));
        }
        if self.memory_warning_bytes >= self.memory_critical_bytes {
            return Err(LoxoneError::invalid_input(
                "Memory warning threshold must be less than critical threshold",
            ));
        }
        Ok(())
    }
}

/// Performance analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Analysis timestamp (as system time nanos)
    pub timestamp: u64,
    /// Overall performance score (0-100)
    pub performance_score: f64,
    /// Detected trends
    pub trends: PerformanceTrends,
    /// Detected anomalies
    pub anomalies: Vec<PerformanceAnomaly>,
    /// Alert conditions
    pub alerts: Vec<PerformanceAlert>,
    /// Baseline comparisons
    pub baseline_comparison: BaselineComparison,
    /// Recommendations
    pub recommendations: Vec<PerformanceRecommendation>,
}

/// Performance anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnomaly {
    /// Anomaly type
    pub anomaly_type: AnomalyType,
    /// Metric that triggered the anomaly
    pub metric: String,
    /// Anomaly value
    pub value: f64,
    /// Expected baseline value
    pub baseline: f64,
    /// Confidence score (0-1)
    pub confidence: f64,
    /// Anomaly severity
    pub severity: AnomalySeverity,
    /// Description
    pub description: String,
    /// Timestamp when detected (as system time nanos)
    pub detected_at: u64,
}

/// Anomaly type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyType {
    /// Value is significantly higher than expected
    Spike,
    /// Value is significantly lower than expected
    Drop,
    /// Unusual pattern detected
    Pattern,
    /// Sudden change in trend
    TrendChange,
}

/// Anomaly severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Performance alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlert {
    /// Alert type
    pub alert_type: AlertType,
    /// Alert level
    pub level: AlertLevel,
    /// Metric that triggered the alert
    pub metric: String,
    /// Current value
    pub current_value: f64,
    /// Threshold that was crossed
    pub threshold: f64,
    /// Alert message
    pub message: String,
    /// Timestamp when triggered (as system time nanos)
    pub triggered_at: u64,
}

/// Alert type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    ResponseTime,
    Throughput,
    ErrorRate,
    CpuUsage,
    MemoryUsage,
    Custom(String),
}

/// Alert level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

/// Baseline comparison result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineComparison {
    /// Response time comparison
    pub response_time: BaselineMetricComparison,
    /// Throughput comparison
    pub throughput: BaselineMetricComparison,
    /// Error rate comparison
    pub error_rate: BaselineMetricComparison,
    /// Resource usage comparison
    pub resource_usage: BaselineMetricComparison,
}

/// Baseline metric comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineMetricComparison {
    /// Current value
    pub current: f64,
    /// Baseline value
    pub baseline: f64,
    /// Percentage change from baseline
    pub change_percent: f64,
    /// Whether change is significant
    pub significant_change: bool,
}

/// Performance recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRecommendation {
    /// Recommendation category
    pub category: RecommendationCategory,
    /// Priority level
    pub priority: RecommendationPriority,
    /// Recommendation title
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Expected impact
    pub expected_impact: String,
    /// Implementation effort
    pub effort: ImplementationEffort,
}

/// Recommendation category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationCategory {
    Algorithm,
    Caching,
    Database,
    Infrastructure,
    Configuration,
    Monitoring,
}

/// Recommendation priority
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

/// Implementation effort estimate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImplementationEffort {
    Low,
    Medium,
    High,
}

/// Performance analyzer
pub struct PerformanceAnalyzer {
    config: AnalyzerConfig,
    measurements: Arc<RwLock<Vec<PerformanceMeasurement>>>,
    #[allow(dead_code)]
    baselines: Arc<RwLock<HashMap<String, f64>>>,
    last_analysis: Arc<RwLock<Option<AnalysisResult>>>,
}

impl PerformanceAnalyzer {
    /// Create new performance analyzer
    pub fn new(config: AnalyzerConfig) -> Result<Self> {
        config.validate()?;

        Ok(Self {
            config,
            measurements: Arc::new(RwLock::new(Vec::new())),
            baselines: Arc::new(RwLock::new(HashMap::new())),
            last_analysis: Arc::new(RwLock::new(None)),
        })
    }

    /// Analyze performance measurement for issues
    pub async fn analyze_performance(
        &self,
        measurement: &PerformanceMeasurement,
    ) -> Result<Vec<crate::performance::PerformanceIssue>> {
        if !self.config.enabled {
            return Ok(Vec::new());
        }

        let mut issues = Vec::new();

        // Store measurement for trend analysis
        {
            let mut measurements = self.measurements.write().await;
            measurements.push(measurement.clone());
        }

        // Check for immediate issues
        if let Some(duration) = measurement.timing.get_duration() {
            if duration > self.config.alert_thresholds.response_time.critical {
                issues.push(crate::performance::PerformanceIssue {
                    severity: crate::performance::PerformanceIssueSeverity::Critical,
                    issue_type: crate::performance::PerformanceIssueType::HighLatency,
                    description: format!("Critical response time: {:?}", duration),
                    recommendation: Some("Immediate investigation required".to_string()),
                    metric_value: Some(duration.as_millis() as f64),
                    threshold: Some(
                        self.config
                            .alert_thresholds
                            .response_time
                            .critical
                            .as_millis() as f64,
                    ),
                });
            } else if duration > self.config.alert_thresholds.response_time.warning {
                issues.push(crate::performance::PerformanceIssue {
                    severity: crate::performance::PerformanceIssueSeverity::Warning,
                    issue_type: crate::performance::PerformanceIssueType::HighLatency,
                    description: format!("High response time: {:?}", duration),
                    recommendation: Some("Monitor closely and optimize if persistent".to_string()),
                    metric_value: Some(duration.as_millis() as f64),
                    threshold: Some(
                        self.config
                            .alert_thresholds
                            .response_time
                            .warning
                            .as_millis() as f64,
                    ),
                });
            }
        }

        // Check resource usage
        if let Some(cpu) = measurement.resource_usage.cpu_usage {
            if cpu
                > self
                    .config
                    .alert_thresholds
                    .resource_usage
                    .cpu_critical_percent
            {
                issues.push(crate::performance::PerformanceIssue {
                    severity: crate::performance::PerformanceIssueSeverity::Critical,
                    issue_type: crate::performance::PerformanceIssueType::HighCpuUsage,
                    description: format!("Critical CPU usage: {:.1}%", cpu),
                    recommendation: Some(
                        "Scale resources or optimize CPU-intensive operations".to_string(),
                    ),
                    metric_value: Some(cpu),
                    threshold: Some(
                        self.config
                            .alert_thresholds
                            .resource_usage
                            .cpu_critical_percent,
                    ),
                });
            }
        }

        if let Some(memory) = measurement.resource_usage.memory_usage {
            if memory
                > self
                    .config
                    .alert_thresholds
                    .resource_usage
                    .memory_critical_bytes
            {
                issues.push(crate::performance::PerformanceIssue {
                    severity: crate::performance::PerformanceIssueSeverity::Critical,
                    issue_type: crate::performance::PerformanceIssueType::HighMemoryUsage,
                    description: format!("Critical memory usage: {} bytes", memory),
                    recommendation: Some(
                        "Investigate memory leaks or scale memory resources".to_string(),
                    ),
                    metric_value: Some(memory as f64),
                    threshold: Some(
                        self.config
                            .alert_thresholds
                            .resource_usage
                            .memory_critical_bytes as f64,
                    ),
                });
            }
        }

        Ok(issues)
    }

    /// Get performance statistics
    pub async fn get_statistics(&self) -> Result<PerformanceStatistics> {
        if !self.config.enabled {
            return Ok(PerformanceStatistics::default());
        }

        let measurements = self.measurements.read().await;

        if measurements.is_empty() {
            return Ok(PerformanceStatistics::default());
        }

        // Calculate request statistics
        let request_stats = self.calculate_request_statistics(&measurements);
        let resource_stats = self.calculate_resource_statistics(&measurements);
        let error_stats = self.calculate_error_statistics(&measurements);
        let trends = self.calculate_trends(&measurements).await?;

        Ok(PerformanceStatistics {
            request_stats,
            resource_stats,
            error_stats,
            trends,
        })
    }

    /// Perform comprehensive performance analysis
    pub async fn perform_analysis(&self) -> Result<AnalysisResult> {
        if !self.config.enabled {
            return Ok(AnalysisResult {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64,
                performance_score: 100.0,
                trends: PerformanceTrends::default(),
                anomalies: Vec::new(),
                alerts: Vec::new(),
                baseline_comparison: BaselineComparison {
                    response_time: BaselineMetricComparison {
                        current: 0.0,
                        baseline: 0.0,
                        change_percent: 0.0,
                        significant_change: false,
                    },
                    throughput: BaselineMetricComparison {
                        current: 0.0,
                        baseline: 0.0,
                        change_percent: 0.0,
                        significant_change: false,
                    },
                    error_rate: BaselineMetricComparison {
                        current: 0.0,
                        baseline: 0.0,
                        change_percent: 0.0,
                        significant_change: false,
                    },
                    resource_usage: BaselineMetricComparison {
                        current: 0.0,
                        baseline: 0.0,
                        change_percent: 0.0,
                        significant_change: false,
                    },
                },
                recommendations: Vec::new(),
            });
        }

        let measurements = self.measurements.read().await;

        let performance_score = self.calculate_performance_score(&measurements);
        let trends = self.calculate_trends(&measurements).await?;
        let anomalies = self.detect_anomalies(&measurements).await?;
        let alerts = self.check_alert_conditions(&measurements).await?;
        let baseline_comparison = self.compare_with_baselines(&measurements).await?;
        let recommendations = self
            .generate_recommendations(&measurements, &anomalies, &alerts)
            .await?;

        let result = AnalysisResult {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            performance_score,
            trends,
            anomalies,
            alerts,
            baseline_comparison,
            recommendations,
        };

        // Store analysis result
        {
            let mut last_analysis = self.last_analysis.write().await;
            *last_analysis = Some(result.clone());
        }

        info!(
            "Performance analysis completed with score: {:.1}",
            performance_score
        );
        Ok(result)
    }

    // Private helper methods

    fn calculate_request_statistics(
        &self,
        measurements: &[PerformanceMeasurement],
    ) -> RequestStatistics {
        if measurements.is_empty() {
            return RequestStatistics::default();
        }

        let durations: Vec<Duration> = measurements
            .iter()
            .filter_map(|m| m.timing.get_duration())
            .collect();

        if durations.is_empty() {
            return RequestStatistics::default();
        }

        let total_requests = measurements.len() as u64;
        let total_duration: Duration = durations.iter().sum();
        let avg_response_time = total_duration / durations.len() as u32;

        let mut sorted_durations = durations.clone();
        sorted_durations.sort();

        let p50_response_time = sorted_durations[sorted_durations.len() * 50 / 100];
        let p95_response_time = sorted_durations[sorted_durations.len() * 95 / 100];
        let p99_response_time = sorted_durations[sorted_durations.len() * 99 / 100];

        // Calculate requests per second based on time window
        let requests_per_second = if !measurements.is_empty() {
            let time_span = measurements.last().unwrap().timing.start_time
                - measurements.first().unwrap().timing.start_time;
            if time_span.as_secs() > 0 {
                total_requests as f64 / time_span.as_secs() as f64
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Calculate success rate (simplified - would need error tracking)
        let success_rate = 95.0; // Placeholder

        RequestStatistics {
            total_requests,
            avg_response_time,
            p50_response_time,
            p95_response_time,
            p99_response_time,
            requests_per_second,
            success_rate,
        }
    }

    fn calculate_resource_statistics(
        &self,
        measurements: &[PerformanceMeasurement],
    ) -> ResourceStatistics {
        if measurements.is_empty() {
            return ResourceStatistics::default();
        }

        let cpu_values: Vec<f64> = measurements
            .iter()
            .filter_map(|m| m.resource_usage.cpu_usage)
            .collect();

        let memory_values: Vec<u64> = measurements
            .iter()
            .filter_map(|m| m.resource_usage.memory_usage)
            .collect();

        let avg_cpu_usage = if !cpu_values.is_empty() {
            cpu_values.iter().sum::<f64>() / cpu_values.len() as f64
        } else {
            0.0
        };

        let peak_cpu_usage = cpu_values.iter().fold(0.0f64, |a, &b| a.max(b));

        let avg_memory_usage = if !memory_values.is_empty() {
            memory_values.iter().sum::<u64>() / memory_values.len() as u64
        } else {
            0
        };

        let peak_memory_usage = memory_values.iter().fold(0, |a, &b| a.max(b));

        ResourceStatistics {
            avg_cpu_usage,
            peak_cpu_usage,
            avg_memory_usage,
            peak_memory_usage,
            total_network_traffic: 0, // Would need to aggregate network metrics
            total_disk_io: 0,         // Would need to aggregate disk metrics
        }
    }

    fn calculate_error_statistics(
        &self,
        _measurements: &[PerformanceMeasurement],
    ) -> ErrorStatistics {
        // Simplified error statistics - would need actual error tracking
        ErrorStatistics::default()
    }

    async fn calculate_trends(
        &self,
        measurements: &[PerformanceMeasurement],
    ) -> Result<PerformanceTrends> {
        if !self.config.trend_detection.enabled
            || measurements.len() < self.config.trend_detection.min_data_points
        {
            return Ok(PerformanceTrends::default());
        }

        // Calculate trends using linear regression (simplified)
        let response_time_trend = self.calculate_linear_trend(
            &measurements
                .iter()
                .filter_map(|m| m.timing.get_duration().map(|d| d.as_millis() as f64))
                .collect::<Vec<_>>(),
        );

        let throughput_trend = 0.0; // Would calculate based on request rate changes
        let error_rate_trend = 0.0; // Would calculate based on error rate changes
        let resource_usage_trend = 0.0; // Would calculate based on resource usage changes

        Ok(PerformanceTrends {
            response_time_trend,
            throughput_trend,
            error_rate_trend,
            resource_usage_trend,
        })
    }

    fn calculate_linear_trend(&self, values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }

        let n = values.len() as f64;
        let x_values: Vec<f64> = (0..values.len()).map(|i| i as f64).collect();

        let sum_x: f64 = x_values.iter().sum();
        let sum_y: f64 = values.iter().sum();
        let sum_xy: f64 = x_values.iter().zip(values.iter()).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = x_values.iter().map(|x| x * x).sum();

        // Calculate slope (trend)
        let numerator = n * sum_xy - sum_x * sum_y;
        let denominator = n * sum_x2 - sum_x * sum_x;

        if denominator != 0.0 {
            numerator / denominator
        } else {
            0.0
        }
    }

    async fn detect_anomalies(
        &self,
        _measurements: &[PerformanceMeasurement],
    ) -> Result<Vec<PerformanceAnomaly>> {
        if !self.config.anomaly_detection.enabled {
            return Ok(Vec::new());
        }

        // Simplified anomaly detection - would implement proper statistical methods
        Ok(Vec::new())
    }

    async fn check_alert_conditions(
        &self,
        measurements: &[PerformanceMeasurement],
    ) -> Result<Vec<PerformanceAlert>> {
        let mut alerts = Vec::new();

        for measurement in measurements.iter().rev().take(10) {
            // Check recent measurements
            // Check response time alerts
            if let Some(duration) = measurement.timing.get_duration() {
                if duration > self.config.alert_thresholds.response_time.critical {
                    alerts.push(PerformanceAlert {
                        alert_type: AlertType::ResponseTime,
                        level: AlertLevel::Critical,
                        metric: "response_time".to_string(),
                        current_value: duration.as_millis() as f64,
                        threshold: self
                            .config
                            .alert_thresholds
                            .response_time
                            .critical
                            .as_millis() as f64,
                        message: format!(
                            "Response time {} exceeds critical threshold",
                            duration.as_millis()
                        ),
                        triggered_at: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_nanos() as u64,
                    });
                }
            }

            // Check resource usage alerts
            if let Some(cpu) = measurement.resource_usage.cpu_usage {
                if cpu
                    > self
                        .config
                        .alert_thresholds
                        .resource_usage
                        .cpu_critical_percent
                {
                    alerts.push(PerformanceAlert {
                        alert_type: AlertType::CpuUsage,
                        level: AlertLevel::Critical,
                        metric: "cpu_usage".to_string(),
                        current_value: cpu,
                        threshold: self
                            .config
                            .alert_thresholds
                            .resource_usage
                            .cpu_critical_percent,
                        message: format!("CPU usage {:.1}% exceeds critical threshold", cpu),
                        triggered_at: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_nanos() as u64,
                    });
                }
            }
        }

        Ok(alerts)
    }

    async fn compare_with_baselines(
        &self,
        _measurements: &[PerformanceMeasurement],
    ) -> Result<BaselineComparison> {
        // Simplified baseline comparison - would implement proper baseline tracking
        Ok(BaselineComparison {
            response_time: BaselineMetricComparison {
                current: 100.0,
                baseline: 95.0,
                change_percent: 5.3,
                significant_change: false,
            },
            throughput: BaselineMetricComparison {
                current: 10.0,
                baseline: 12.0,
                change_percent: -16.7,
                significant_change: true,
            },
            error_rate: BaselineMetricComparison {
                current: 0.5,
                baseline: 0.2,
                change_percent: 150.0,
                significant_change: true,
            },
            resource_usage: BaselineMetricComparison {
                current: 45.0,
                baseline: 40.0,
                change_percent: 12.5,
                significant_change: false,
            },
        })
    }

    async fn generate_recommendations(
        &self,
        _measurements: &[PerformanceMeasurement],
        anomalies: &[PerformanceAnomaly],
        alerts: &[PerformanceAlert],
    ) -> Result<Vec<PerformanceRecommendation>> {
        let mut recommendations = Vec::new();

        // Generate recommendations based on alerts
        for alert in alerts {
            match alert.alert_type {
                AlertType::ResponseTime => {
                    recommendations.push(PerformanceRecommendation {
                        category: RecommendationCategory::Algorithm,
                        priority: RecommendationPriority::High,
                        title: "Optimize response time".to_string(),
                        description: "Response times are exceeding acceptable thresholds"
                            .to_string(),
                        expected_impact: "Reduce response time by 20-50%".to_string(),
                        effort: ImplementationEffort::Medium,
                    });
                }
                AlertType::CpuUsage => {
                    recommendations.push(PerformanceRecommendation {
                        category: RecommendationCategory::Infrastructure,
                        priority: RecommendationPriority::High,
                        title: "Scale CPU resources".to_string(),
                        description: "CPU usage is consistently high".to_string(),
                        expected_impact: "Reduce CPU contention and improve responsiveness"
                            .to_string(),
                        effort: ImplementationEffort::Low,
                    });
                }
                _ => {}
            }
        }

        // Generate recommendations based on anomalies
        for _anomaly in anomalies {
            recommendations.push(PerformanceRecommendation {
                category: RecommendationCategory::Monitoring,
                priority: RecommendationPriority::Medium,
                title: "Investigate performance anomaly".to_string(),
                description: "Unusual performance patterns detected".to_string(),
                expected_impact: "Identify and resolve performance issues".to_string(),
                effort: ImplementationEffort::Medium,
            });
        }

        // Add general recommendations
        recommendations.push(PerformanceRecommendation {
            category: RecommendationCategory::Caching,
            priority: RecommendationPriority::Medium,
            title: "Implement response caching".to_string(),
            description: "Cache frequently requested data to reduce response times".to_string(),
            expected_impact: "Reduce response time by 30-70% for cached requests".to_string(),
            effort: ImplementationEffort::Medium,
        });

        Ok(recommendations)
    }

    fn calculate_performance_score(&self, measurements: &[PerformanceMeasurement]) -> f64 {
        if measurements.is_empty() {
            return 100.0;
        }

        let mut score: f64 = 100.0;

        // Factor in response times
        let avg_response_time = if !measurements.is_empty() {
            let durations: Vec<Duration> = measurements
                .iter()
                .filter_map(|m| m.timing.get_duration())
                .collect();

            if !durations.is_empty() {
                durations.iter().sum::<Duration>() / durations.len() as u32
            } else {
                Duration::from_millis(0)
            }
        } else {
            Duration::from_millis(0)
        };

        if avg_response_time > self.config.alert_thresholds.response_time.critical {
            score -= 40.0;
        } else if avg_response_time > self.config.alert_thresholds.response_time.warning {
            score -= 20.0;
        }

        // Factor in resource usage
        let avg_cpu = measurements
            .iter()
            .filter_map(|m| m.resource_usage.cpu_usage)
            .fold(0.0, |acc, cpu| acc + cpu)
            / measurements.len() as f64;

        if avg_cpu
            > self
                .config
                .alert_thresholds
                .resource_usage
                .cpu_critical_percent
        {
            score -= 30.0;
        } else if avg_cpu
            > self
                .config
                .alert_thresholds
                .resource_usage
                .cpu_warning_percent
        {
            score -= 15.0;
        }

        score.max(0.0f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_config_validation() {
        let config = AnalyzerConfig::production();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_alert_thresholds_validation() {
        let thresholds = AlertThresholds::production();
        assert!(thresholds.validate().is_ok());
    }

    #[tokio::test]
    async fn test_analyzer_creation() {
        let config = AnalyzerConfig::development();
        let analyzer = PerformanceAnalyzer::new(config);
        assert!(analyzer.is_ok());
    }

    #[test]
    fn test_linear_trend_calculation() {
        let config = AnalyzerConfig::development();
        let analyzer = PerformanceAnalyzer::new(config).unwrap();

        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let trend = analyzer.calculate_linear_trend(&values);
        assert!(trend > 0.0); // Should be positive trend
    }

    #[tokio::test]
    async fn test_performance_analysis() {
        let config = AnalyzerConfig::minimal();
        let analyzer = PerformanceAnalyzer::new(config).unwrap();

        let analysis = analyzer.perform_analysis().await.unwrap();
        assert!(analysis.performance_score >= 0.0);
        assert!(analysis.performance_score <= 100.0);
    }
}
