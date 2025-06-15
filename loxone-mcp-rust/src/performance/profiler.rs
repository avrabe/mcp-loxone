//! Performance profiling and bottleneck detection

use crate::error::{LoxoneError, Result};
use crate::performance::{PerformanceContext, PerformanceTiming};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Performance profiler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerConfig {
    /// Enable profiling
    pub enabled: bool,
    /// Profiling mode
    pub mode: ProfilingMode,
    /// Sampling rate (for sampling profiler)
    pub sampling_rate: Duration,
    /// Maximum number of profiles to keep in memory
    pub max_profiles: usize,
    /// Enable stack trace collection
    pub collect_stack_traces: bool,
    /// Minimum duration to consider for profiling
    pub min_duration_threshold: Duration,
    /// Bottleneck detection configuration
    pub bottleneck_detection: BottleneckDetectionConfig,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: ProfilingMode::Selective,
            sampling_rate: Duration::from_millis(100),
            max_profiles: 1000,
            collect_stack_traces: false,
            min_duration_threshold: Duration::from_millis(10),
            bottleneck_detection: BottleneckDetectionConfig::default(),
        }
    }
}

impl ProfilerConfig {
    /// Production configuration with minimal overhead
    pub fn production() -> Self {
        Self {
            enabled: true,
            mode: ProfilingMode::Selective,
            sampling_rate: Duration::from_millis(1000),
            max_profiles: 500,
            collect_stack_traces: false,
            min_duration_threshold: Duration::from_millis(100),
            bottleneck_detection: BottleneckDetectionConfig::production(),
        }
    }

    /// Development configuration with detailed profiling
    pub fn development() -> Self {
        Self {
            enabled: true,
            mode: ProfilingMode::Continuous,
            sampling_rate: Duration::from_millis(50),
            max_profiles: 2000,
            collect_stack_traces: true,
            min_duration_threshold: Duration::from_millis(1),
            bottleneck_detection: BottleneckDetectionConfig::development(),
        }
    }

    /// Disabled configuration for testing
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            mode: ProfilingMode::Disabled,
            sampling_rate: Duration::from_secs(1),
            max_profiles: 0,
            collect_stack_traces: false,
            min_duration_threshold: Duration::from_secs(1),
            bottleneck_detection: BottleneckDetectionConfig::disabled(),
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.sampling_rate.is_zero() {
                return Err(LoxoneError::invalid_input("Sampling rate cannot be zero"));
            }

            if self.max_profiles == 0 && self.mode != ProfilingMode::Disabled {
                return Err(LoxoneError::invalid_input(
                    "Max profiles cannot be zero when profiling is enabled",
                ));
            }

            self.bottleneck_detection.validate()?;
        }
        Ok(())
    }
}

/// Profiling mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProfilingMode {
    /// Profiling disabled
    Disabled,
    /// Profile only specific operations (low overhead)
    Selective,
    /// Continuous profiling (higher overhead)
    Continuous,
    /// Statistical sampling profiling
    Sampling,
}

/// Bottleneck detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleneckDetectionConfig {
    /// Enable bottleneck detection
    pub enabled: bool,
    /// Latency threshold for bottleneck detection
    pub latency_threshold: Duration,
    /// CPU usage threshold (percentage)
    pub cpu_threshold: f64,
    /// Memory usage threshold (bytes)
    pub memory_threshold: u64,
    /// Queue depth threshold
    pub queue_depth_threshold: u32,
    /// Detection window size
    pub detection_window: Duration,
    /// Minimum occurrences to consider a bottleneck
    pub min_occurrences: u32,
}

impl Default for BottleneckDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            latency_threshold: Duration::from_millis(1000),
            cpu_threshold: 80.0,
            memory_threshold: 1024 * 1024 * 512, // 512MB
            queue_depth_threshold: 10,
            detection_window: Duration::from_secs(60),
            min_occurrences: 3,
        }
    }
}

impl BottleneckDetectionConfig {
    /// Production configuration
    pub fn production() -> Self {
        Self {
            enabled: true,
            latency_threshold: Duration::from_millis(2000),
            cpu_threshold: 85.0,
            memory_threshold: 1024 * 1024 * 1024, // 1GB
            queue_depth_threshold: 20,
            detection_window: Duration::from_secs(300), // 5 minutes
            min_occurrences: 5,
        }
    }

    /// Development configuration
    pub fn development() -> Self {
        Self {
            enabled: true,
            latency_threshold: Duration::from_millis(500),
            cpu_threshold: 70.0,
            memory_threshold: 1024 * 1024 * 256, // 256MB
            queue_depth_threshold: 5,
            detection_window: Duration::from_secs(30),
            min_occurrences: 2,
        }
    }

    /// Disabled configuration
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            latency_threshold: Duration::from_secs(10),
            cpu_threshold: 100.0,
            memory_threshold: u64::MAX,
            queue_depth_threshold: u32::MAX,
            detection_window: Duration::from_secs(1),
            min_occurrences: u32::MAX,
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.latency_threshold.is_zero() {
                return Err(LoxoneError::invalid_input(
                    "Latency threshold cannot be zero",
                ));
            }

            if self.cpu_threshold <= 0.0 || self.cpu_threshold > 100.0 {
                return Err(LoxoneError::invalid_input(
                    "CPU threshold must be between 0 and 100",
                ));
            }

            if self.detection_window.is_zero() {
                return Err(LoxoneError::invalid_input(
                    "Detection window cannot be zero",
                ));
            }

            if self.min_occurrences == 0 {
                return Err(LoxoneError::invalid_input("Min occurrences cannot be zero"));
            }
        }
        Ok(())
    }
}

/// Performance profile data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceProfile {
    /// Profile context
    pub context: PerformanceContext,
    /// Profile timing
    pub timing: PerformanceTiming,
    /// Profiling samples
    pub samples: Vec<ProfileSample>,
    /// Stack traces (if enabled)
    pub stack_traces: Vec<StackTrace>,
    /// Resource usage during profiling
    pub resource_usage: ProfileResourceUsage,
    /// Detected bottlenecks
    pub bottlenecks: Vec<Bottleneck>,
}

/// Individual profiling sample
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSample {
    /// Sample timestamp (as system time nanos)
    pub timestamp: u64,
    /// Operation being profiled
    pub operation: String,
    /// Sample duration
    pub duration: Duration,
    /// Thread/task identifier
    pub thread_id: Option<String>,
    /// Custom data
    pub data: HashMap<String, String>,
}

/// Stack trace information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackTrace {
    /// Timestamp when captured (as system time nanos)
    pub timestamp: u64,
    /// Thread identifier
    pub thread_id: String,
    /// Stack frames
    pub frames: Vec<StackFrame>,
}

/// Stack frame information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFrame {
    /// Function name
    pub function: String,
    /// File name
    pub file: Option<String>,
    /// Line number
    pub line: Option<u32>,
    /// Module/crate name
    pub module: Option<String>,
}

/// Resource usage during profiling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileResourceUsage {
    /// Peak CPU usage
    pub peak_cpu: f64,
    /// Average CPU usage
    pub avg_cpu: f64,
    /// Peak memory usage
    pub peak_memory: u64,
    /// Average memory usage
    pub avg_memory: u64,
    /// Memory allocations
    pub allocations: u64,
    /// Memory deallocations
    pub deallocations: u64,
}

impl Default for ProfileResourceUsage {
    fn default() -> Self {
        Self {
            peak_cpu: 0.0,
            avg_cpu: 0.0,
            peak_memory: 0,
            avg_memory: 0,
            allocations: 0,
            deallocations: 0,
        }
    }
}

/// Detected bottleneck
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottleneck {
    /// Bottleneck type
    pub bottleneck_type: BottleneckType,
    /// Severity level
    pub severity: BottleneckSeverity,
    /// Description
    pub description: String,
    /// Location/operation
    pub location: String,
    /// Metric value that triggered detection
    pub trigger_value: f64,
    /// Threshold that was exceeded
    pub threshold: f64,
    /// Suggested optimizations
    pub suggestions: Vec<String>,
    /// First detected timestamp (as system time nanos)
    pub first_detected: u64,
    /// Occurrence count
    pub occurrences: u32,
}

/// Bottleneck type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BottleneckType {
    /// High latency operation
    HighLatency,
    /// High CPU usage
    CpuBound,
    /// High memory usage
    MemoryBound,
    /// I/O bound operation
    IoBound,
    /// Network latency
    NetworkBound,
    /// Database query bottleneck
    DatabaseBound,
    /// Contention/locking issues
    Contention,
    /// Memory leak
    MemoryLeak,
    /// Inefficient algorithm
    Algorithm,
}

/// Bottleneck severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BottleneckSeverity {
    /// Low impact
    Low,
    /// Medium impact
    Medium,
    /// High impact
    High,
    /// Critical impact
    Critical,
}

/// Performance profiler
pub struct PerformanceProfiler {
    config: ProfilerConfig,
    profiles: Arc<RwLock<Vec<PerformanceProfile>>>,
    bottlenecks: Arc<RwLock<HashMap<String, Bottleneck>>>,
    active_samples: Arc<RwLock<HashMap<String, Vec<ProfileSample>>>>,
}

impl PerformanceProfiler {
    /// Create new performance profiler
    pub fn new(config: ProfilerConfig) -> Result<Self> {
        config.validate()?;

        Ok(Self {
            config,
            profiles: Arc::new(RwLock::new(Vec::new())),
            bottlenecks: Arc::new(RwLock::new(HashMap::new())),
            active_samples: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Start profiling an operation
    pub async fn start_profiling(&self, context: PerformanceContext) -> Result<ProfileSession> {
        if !self.config.enabled || self.config.mode == ProfilingMode::Disabled {
            return Ok(ProfileSession::disabled());
        }

        let session = ProfileSession::new(context.clone(), self.config.clone());

        // Initialize active samples for this operation
        let mut active_samples = self.active_samples.write().await;
        active_samples.insert(context.operation_id.clone(), Vec::new());

        debug!(
            "Started profiling session for operation: {}",
            context.operation_id
        );
        Ok(session)
    }

    /// Record a sample during profiling
    pub async fn record_sample(&self, operation_id: &str, sample: ProfileSample) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut active_samples = self.active_samples.write().await;
        if let Some(samples) = active_samples.get_mut(operation_id) {
            samples.push(sample);
        }

        Ok(())
    }

    /// Finish profiling and generate profile
    pub async fn finish_profiling(&self, session: ProfileSession) -> Result<PerformanceProfile> {
        if !self.config.enabled || session.is_disabled() {
            return Ok(PerformanceProfile {
                context: session.context,
                timing: session.timing,
                samples: Vec::new(),
                stack_traces: Vec::new(),
                resource_usage: ProfileResourceUsage::default(),
                bottlenecks: Vec::new(),
            });
        }

        let mut timing = session.timing;
        timing.finish();

        // Get samples for this operation
        let mut active_samples = self.active_samples.write().await;
        let samples = active_samples
            .remove(&session.context.operation_id)
            .unwrap_or_default();

        // Collect stack traces if enabled
        let stack_traces = if self.config.collect_stack_traces {
            self.collect_stack_traces(&session.context).await
        } else {
            Vec::new()
        };

        // Collect resource usage
        let resource_usage = self.collect_resource_usage(&session.context).await;

        // Detect bottlenecks
        let bottlenecks = self
            .detect_bottlenecks(&session.context, &timing, &samples, &resource_usage)
            .await?;

        let profile = PerformanceProfile {
            context: session.context,
            timing,
            samples,
            stack_traces,
            resource_usage,
            bottlenecks: bottlenecks.clone(),
        };

        // Store profile
        let mut profiles = self.profiles.write().await;
        profiles.push(profile.clone());

        // Cleanup old profiles
        if profiles.len() > self.config.max_profiles {
            let excess = profiles.len() - self.config.max_profiles;
            profiles.drain(0..excess);
        }

        // Update bottleneck tracking
        self.update_bottleneck_tracking(bottlenecks).await?;

        info!(
            "Finished profiling session for operation: {}",
            profile.context.operation_id
        );
        Ok(profile)
    }

    /// Get all detected bottlenecks
    pub async fn get_bottlenecks(&self) -> Result<Vec<Bottleneck>> {
        let bottlenecks = self.bottlenecks.read().await;
        Ok(bottlenecks.values().cloned().collect())
    }

    /// Get profiles for a time range
    pub async fn get_profiles(&self, _time_range: Duration) -> Result<Vec<PerformanceProfile>> {
        let profiles = self.profiles.read().await;
        // let _cutoff = Instant::now() - time_range;

        // Note: This comparison will work as long as PerformanceTiming uses Instant internally
        // If PerformanceTiming is changed to use timestamps, this logic needs updating
        Ok(profiles
            .iter()
            .filter(|_profile| {
                // Check if the profile's start time is within the time range
                // For now, we'll include all profiles since proper time comparison
                // would require consistent timestamp formats
                true
            })
            .cloned()
            .collect())
    }

    /// Get profiling statistics
    pub async fn get_statistics(&self) -> Result<ProfilingStatistics> {
        let profiles = self.profiles.read().await;
        let bottlenecks = self.bottlenecks.read().await;

        let total_profiles = profiles.len();
        let total_bottlenecks = bottlenecks.len();

        let avg_duration = if !profiles.is_empty() {
            let total_duration: Duration = profiles
                .iter()
                .filter_map(|p| p.timing.get_duration())
                .sum();
            total_duration / total_profiles as u32
        } else {
            Duration::from_millis(0)
        };

        let bottleneck_breakdown =
            bottlenecks
                .values()
                .fold(HashMap::new(), |mut acc, bottleneck| {
                    let key = format!("{:?}", bottleneck.bottleneck_type);
                    *acc.entry(key).or_insert(0) += 1;
                    acc
                });

        Ok(ProfilingStatistics {
            total_profiles,
            total_bottlenecks,
            avg_duration,
            bottleneck_breakdown,
        })
    }

    // Private helper methods

    async fn collect_stack_traces(&self, _context: &PerformanceContext) -> Vec<StackTrace> {
        // In a real implementation, this would collect actual stack traces
        // For now, return empty vector
        Vec::new()
    }

    async fn collect_resource_usage(&self, _context: &PerformanceContext) -> ProfileResourceUsage {
        // In a real implementation, this would collect actual resource usage
        // For now, return default values
        ProfileResourceUsage::default()
    }

    async fn detect_bottlenecks(
        &self,
        context: &PerformanceContext,
        timing: &PerformanceTiming,
        samples: &[ProfileSample],
        resource_usage: &ProfileResourceUsage,
    ) -> Result<Vec<Bottleneck>> {
        if !self.config.bottleneck_detection.enabled {
            return Ok(Vec::new());
        }

        let mut bottlenecks = Vec::new();

        // Check latency bottleneck
        if let Some(duration) = timing.get_duration() {
            if duration > self.config.bottleneck_detection.latency_threshold {
                bottlenecks.push(Bottleneck {
                    bottleneck_type: BottleneckType::HighLatency,
                    severity: self.get_latency_severity(duration),
                    description: format!("High latency detected: {:?}", duration),
                    location: context.operation_type.clone(),
                    trigger_value: duration.as_millis() as f64,
                    threshold: self
                        .config
                        .bottleneck_detection
                        .latency_threshold
                        .as_millis() as f64,
                    suggestions: vec![
                        "Consider optimizing the algorithm".to_string(),
                        "Check for blocking operations".to_string(),
                        "Review database queries".to_string(),
                    ],
                    first_detected: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u64,
                    occurrences: 1,
                });
            }
        }

        // Check CPU bottleneck
        if resource_usage.peak_cpu > self.config.bottleneck_detection.cpu_threshold {
            bottlenecks.push(Bottleneck {
                bottleneck_type: BottleneckType::CpuBound,
                severity: self.get_cpu_severity(resource_usage.peak_cpu),
                description: format!("High CPU usage detected: {:.1}%", resource_usage.peak_cpu),
                location: context.operation_type.clone(),
                trigger_value: resource_usage.peak_cpu,
                threshold: self.config.bottleneck_detection.cpu_threshold,
                suggestions: vec![
                    "Optimize computational algorithms".to_string(),
                    "Consider parallel processing".to_string(),
                    "Profile CPU-intensive functions".to_string(),
                ],
                first_detected: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64,
                occurrences: 1,
            });
        }

        // Check memory bottleneck
        if resource_usage.peak_memory > self.config.bottleneck_detection.memory_threshold {
            bottlenecks.push(Bottleneck {
                bottleneck_type: BottleneckType::MemoryBound,
                severity: self.get_memory_severity(resource_usage.peak_memory),
                description: format!(
                    "High memory usage detected: {} bytes",
                    resource_usage.peak_memory
                ),
                location: context.operation_type.clone(),
                trigger_value: resource_usage.peak_memory as f64,
                threshold: self.config.bottleneck_detection.memory_threshold as f64,
                suggestions: vec![
                    "Optimize memory allocation patterns".to_string(),
                    "Implement memory pooling".to_string(),
                    "Check for memory leaks".to_string(),
                ],
                first_detected: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64,
                occurrences: 1,
            });
        }

        // Analyze samples for patterns
        if !samples.is_empty() {
            let slow_samples: Vec<_> = samples
                .iter()
                .filter(|sample| sample.duration > Duration::from_millis(100))
                .collect();

            if slow_samples.len() > samples.len() / 2 {
                bottlenecks.push(Bottleneck {
                    bottleneck_type: BottleneckType::Algorithm,
                    severity: BottleneckSeverity::Medium,
                    description: "Multiple slow operations detected".to_string(),
                    location: context.operation_type.clone(),
                    trigger_value: slow_samples.len() as f64,
                    threshold: (samples.len() / 2) as f64,
                    suggestions: vec![
                        "Review algorithm efficiency".to_string(),
                        "Consider caching strategies".to_string(),
                        "Optimize data structures".to_string(),
                    ],
                    first_detected: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u64,
                    occurrences: 1,
                });
            }
        }

        Ok(bottlenecks)
    }

    fn get_latency_severity(&self, duration: Duration) -> BottleneckSeverity {
        let threshold = self.config.bottleneck_detection.latency_threshold;
        if duration > threshold * 5 {
            BottleneckSeverity::Critical
        } else if duration > threshold * 3 {
            BottleneckSeverity::High
        } else if duration > threshold * 2 {
            BottleneckSeverity::Medium
        } else {
            BottleneckSeverity::Low
        }
    }

    fn get_cpu_severity(&self, cpu_usage: f64) -> BottleneckSeverity {
        if cpu_usage > 95.0 {
            BottleneckSeverity::Critical
        } else if cpu_usage > 90.0 {
            BottleneckSeverity::High
        } else if cpu_usage > 85.0 {
            BottleneckSeverity::Medium
        } else {
            BottleneckSeverity::Low
        }
    }

    fn get_memory_severity(&self, memory_usage: u64) -> BottleneckSeverity {
        let threshold = self.config.bottleneck_detection.memory_threshold;
        if memory_usage > threshold * 3 {
            BottleneckSeverity::Critical
        } else if memory_usage > threshold * 2 {
            BottleneckSeverity::High
        } else if memory_usage > threshold + threshold / 2 {
            BottleneckSeverity::Medium
        } else {
            BottleneckSeverity::Low
        }
    }

    async fn update_bottleneck_tracking(&self, new_bottlenecks: Vec<Bottleneck>) -> Result<()> {
        let mut bottlenecks = self.bottlenecks.write().await;

        for bottleneck in new_bottlenecks {
            let key = format!("{}:{:?}", bottleneck.location, bottleneck.bottleneck_type);

            if let Some(existing) = bottlenecks.get_mut(&key) {
                existing.occurrences += 1;
            } else {
                bottlenecks.insert(key, bottleneck);
            }
        }

        Ok(())
    }
}

/// Profile session handle
pub struct ProfileSession {
    context: PerformanceContext,
    timing: PerformanceTiming,
    disabled: bool,
}

impl ProfileSession {
    fn new(context: PerformanceContext, _config: ProfilerConfig) -> Self {
        Self {
            context,
            timing: PerformanceTiming::new(),
            disabled: false,
        }
    }

    fn disabled() -> Self {
        Self {
            context: PerformanceContext::new("disabled".to_string(), "disabled".to_string()),
            timing: PerformanceTiming::new(),
            disabled: true,
        }
    }

    fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Record a timing phase
    pub fn record_phase(&mut self, phase: String, duration: Duration) {
        self.timing.record_phase(phase, duration);
    }

    /// Add a tag
    pub fn tag(&mut self, key: String, value: String) {
        self.timing.tag(key, value);
    }
}

/// Profiling statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilingStatistics {
    /// Total number of profiles
    pub total_profiles: usize,
    /// Total number of detected bottlenecks
    pub total_bottlenecks: usize,
    /// Average operation duration
    pub avg_duration: Duration,
    /// Bottleneck breakdown by type
    pub bottleneck_breakdown: HashMap<String, u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_config_validation() {
        let config = ProfilerConfig::production();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_bottleneck_detection_config() {
        let config = BottleneckDetectionConfig::development();
        assert!(config.validate().is_ok());
        assert!(config.enabled);
    }

    #[tokio::test]
    async fn test_profiler_creation() {
        let config = ProfilerConfig::development();
        let profiler = PerformanceProfiler::new(config);
        assert!(profiler.is_ok());
    }

    #[tokio::test]
    async fn test_profiling_session() {
        let config = ProfilerConfig::development();
        let profiler = PerformanceProfiler::new(config).unwrap();

        let context = PerformanceContext::new("test_op".to_string(), "test".to_string());
        let session = profiler.start_profiling(context).await.unwrap();

        assert!(!session.is_disabled());

        let profile = profiler.finish_profiling(session).await.unwrap();
        assert_eq!(profile.context.operation_id, "test_op");
    }

    #[test]
    fn test_bottleneck_severity() {
        let config = BottleneckDetectionConfig::development();
        let profiler = PerformanceProfiler::new(ProfilerConfig {
            bottleneck_detection: config,
            ..ProfilerConfig::development()
        })
        .unwrap();

        assert!(matches!(
            profiler.get_cpu_severity(50.0),
            BottleneckSeverity::Low
        ));
        assert!(matches!(
            profiler.get_cpu_severity(87.0),
            BottleneckSeverity::Medium
        ));
        assert!(matches!(
            profiler.get_cpu_severity(99.0),
            BottleneckSeverity::Critical
        ));
    }
}
