//! System diagnostics collection for health monitoring

use super::{CpuInfo, MemoryInfo, PerformanceMetrics, SystemInfo, ThreadPoolMetrics};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// System diagnostics collector
pub struct DiagnosticsCollector {
    /// Metrics collection interval
    collection_interval: Duration,
    /// Historical data points to keep
    history_size: usize,
    /// Collected metrics history
    metrics_history: Vec<DiagnosticSnapshot>,
}

impl DiagnosticsCollector {
    /// Create new diagnostics collector
    pub fn new(collection_interval: Duration, history_size: usize) -> Self {
        Self {
            collection_interval,
            history_size,
            metrics_history: Vec::with_capacity(history_size),
        }
    }

    /// Collect current system diagnostics
    pub async fn collect_diagnostics(&mut self) -> Result<DiagnosticSnapshot> {
        debug!("Collecting system diagnostics");

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let snapshot = DiagnosticSnapshot {
            timestamp,
            system_info: SystemInfo::current(),
            process_info: self.collect_process_info().await?,
            network_info: self.collect_network_info().await?,
            disk_info: self.collect_disk_info().await?,
            environment_info: self.collect_environment_info(),
            runtime_metrics: self.collect_runtime_metrics().await?,
        };

        // Store in history
        self.add_to_history(snapshot.clone());

        Ok(snapshot)
    }

    /// Get diagnostics history
    pub fn get_history(&self) -> &[DiagnosticSnapshot] {
        &self.metrics_history
    }

    /// Get latest diagnostics
    pub fn get_latest(&self) -> Option<&DiagnosticSnapshot> {
        self.metrics_history.last()
    }

    /// Calculate system trends from history
    pub fn calculate_trends(&self) -> Option<DiagnosticTrends> {
        if self.metrics_history.len() < 2 {
            return None;
        }

        let latest = self.metrics_history.last()?;
        let previous = &self.metrics_history[self.metrics_history.len() - 2];

        Some(DiagnosticTrends {
            memory_usage_trend: self.calculate_memory_trend(previous, latest),
            cpu_usage_trend: self.calculate_cpu_trend(previous, latest),
            disk_usage_trend: self.calculate_disk_trend(previous, latest),
            performance_trend: self.calculate_performance_trend(previous, latest),
            timestamp: latest.timestamp,
        })
    }

    /// Add snapshot to history
    fn add_to_history(&mut self, snapshot: DiagnosticSnapshot) {
        self.metrics_history.push(snapshot);

        // Keep only the specified number of history items
        if self.metrics_history.len() > self.history_size {
            self.metrics_history.remove(0);
        }
    }

    /// Collect process-specific information
    async fn collect_process_info(&self) -> Result<ProcessInfo> {
        // In production, this would use system APIs to get actual process info
        Ok(ProcessInfo {
            pid: std::process::id(),
            parent_pid: 1, // Simplified
            command_line: std::env::args().collect::<Vec<_>>().join(" "),
            working_directory: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            start_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(), // Simplified
            cpu_time_user: 0.5, // Mock values
            cpu_time_system: 0.2,
            memory_rss_bytes: 1024 * 1024 * 50,  // 50MB
            memory_vms_bytes: 1024 * 1024 * 100, // 100MB
            open_files: 25,
            thread_count: std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(1),
        })
    }

    /// Collect network information
    async fn collect_network_info(&self) -> Result<NetworkInfo> {
        // In production, this would collect actual network statistics
        Ok(NetworkInfo {
            interfaces: vec![NetworkInterface {
                name: "eth0".to_string(),
                bytes_sent: 1024 * 1024 * 100,     // 100MB
                bytes_received: 1024 * 1024 * 200, // 200MB
                packets_sent: 50000,
                packets_received: 75000,
                errors_sent: 0,
                errors_received: 0,
                is_up: true,
            }],
            connections: ConnectionInfo {
                tcp_established: 5,
                tcp_listen: 3,
                tcp_time_wait: 2,
                udp_sockets: 1,
            },
        })
    }

    /// Collect disk information
    async fn collect_disk_info(&self) -> Result<DiskInfo> {
        // In production, this would use system APIs for actual disk info
        Ok(DiskInfo {
            filesystems: vec![FilesystemInfo {
                mount_point: "/".to_string(),
                filesystem_type: "ext4".to_string(),
                total_bytes: 1024_u64 * 1024 * 1024 * 100, // 100GB
                available_bytes: 1024_u64 * 1024 * 1024 * 75, // 75GB available
                used_bytes: 1024_u64 * 1024 * 1024 * 25,   // 25GB used
                usage_percent: 25.0,
                inodes_total: 1000000,
                inodes_used: 250000,
                inodes_available: 750000,
            }],
            io_stats: DiskIOStats {
                reads_completed: 10000,
                writes_completed: 5000,
                bytes_read: 1024 * 1024 * 500,    // 500MB
                bytes_written: 1024 * 1024 * 200, // 200MB
                io_time_ms: 5000,
            },
        })
    }

    /// Collect environment information
    fn collect_environment_info(&self) -> EnvironmentInfo {
        let mut env_vars = HashMap::new();

        // Collect relevant environment variables (excluding sensitive ones)
        for (key, value) in std::env::vars() {
            if self.is_safe_env_var(&key) {
                env_vars.insert(key, value);
            }
        }

        EnvironmentInfo {
            hostname: std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string()),
            user: std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()),
            shell: std::env::var("SHELL").unwrap_or_else(|_| "unknown".to_string()),
            path: std::env::var("PATH").unwrap_or_else(|_| "unknown".to_string()),
            timezone: std::env::var("TZ").unwrap_or_else(|_| "UTC".to_string()),
            environment_variables: env_vars,
        }
    }

    /// Collect runtime-specific metrics
    async fn collect_runtime_metrics(&self) -> Result<RuntimeMetrics> {
        Ok(RuntimeMetrics {
            gc_collections: 0, // Rust doesn't have GC, but we might track allocator stats
            gc_time_ms: 0,
            allocated_bytes: 1024 * 1024 * 45, // Mock value
            heap_size_bytes: 1024 * 1024 * 50,
            stack_size_bytes: 1024 * 8, // 8KB stack
            thread_pool: ThreadPoolMetrics::default(),
            async_tasks_active: 10,
            async_tasks_completed: 1000,
        })
    }

    /// Check if environment variable is safe to collect
    fn is_safe_env_var(&self, key: &str) -> bool {
        !key.to_uppercase().contains("PASSWORD")
            && !key.to_uppercase().contains("SECRET")
            && !key.to_uppercase().contains("TOKEN")
            && !key.to_uppercase().contains("KEY")
            && !key.to_uppercase().contains("PRIVATE")
    }

    /// Calculate memory usage trend
    fn calculate_memory_trend(
        &self,
        previous: &DiagnosticSnapshot,
        current: &DiagnosticSnapshot,
    ) -> TrendDirection {
        let prev_usage = previous.system_info.memory.usage_percent;
        let curr_usage = current.system_info.memory.usage_percent;

        if curr_usage > prev_usage + 1.0 {
            TrendDirection::Increasing
        } else if curr_usage < prev_usage - 1.0 {
            TrendDirection::Decreasing
        } else {
            TrendDirection::Stable
        }
    }

    /// Calculate CPU usage trend
    fn calculate_cpu_trend(
        &self,
        previous: &DiagnosticSnapshot,
        current: &DiagnosticSnapshot,
    ) -> TrendDirection {
        let prev_usage = previous.system_info.cpu.usage_percent;
        let curr_usage = current.system_info.cpu.usage_percent;

        if curr_usage > prev_usage + 2.0 {
            TrendDirection::Increasing
        } else if curr_usage < prev_usage - 2.0 {
            TrendDirection::Decreasing
        } else {
            TrendDirection::Stable
        }
    }

    /// Calculate disk usage trend
    fn calculate_disk_trend(
        &self,
        previous: &DiagnosticSnapshot,
        current: &DiagnosticSnapshot,
    ) -> TrendDirection {
        if let (Some(prev_fs), Some(curr_fs)) = (
            previous.disk_info.filesystems.first(),
            current.disk_info.filesystems.first(),
        ) {
            if curr_fs.usage_percent > prev_fs.usage_percent + 0.5 {
                TrendDirection::Increasing
            } else if curr_fs.usage_percent < prev_fs.usage_percent - 0.5 {
                TrendDirection::Decreasing
            } else {
                TrendDirection::Stable
            }
        } else {
            TrendDirection::Stable
        }
    }

    /// Calculate performance trend
    fn calculate_performance_trend(
        &self,
        _previous: &DiagnosticSnapshot,
        _current: &DiagnosticSnapshot,
    ) -> TrendDirection {
        // This would compare performance metrics like response times, throughput, etc.
        TrendDirection::Stable
    }
}

/// Complete diagnostic snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSnapshot {
    /// Timestamp when snapshot was taken
    pub timestamp: u64,
    /// System information
    pub system_info: SystemInfo,
    /// Process-specific information
    pub process_info: ProcessInfo,
    /// Network information
    pub network_info: NetworkInfo,
    /// Disk information
    pub disk_info: DiskInfo,
    /// Environment information
    pub environment_info: EnvironmentInfo,
    /// Runtime metrics
    pub runtime_metrics: RuntimeMetrics,
}

/// Process information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Parent process ID
    pub parent_pid: u32,
    /// Command line used to start process
    pub command_line: String,
    /// Current working directory
    pub working_directory: String,
    /// Process start time (Unix timestamp)
    pub start_time: u64,
    /// CPU time spent in user mode (seconds)
    pub cpu_time_user: f64,
    /// CPU time spent in system mode (seconds)
    pub cpu_time_system: f64,
    /// Resident set size (physical memory currently used)
    pub memory_rss_bytes: u64,
    /// Virtual memory size
    pub memory_vms_bytes: u64,
    /// Number of open file descriptors
    pub open_files: u32,
    /// Number of threads
    pub thread_count: usize,
}

/// Network information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    /// Network interfaces
    pub interfaces: Vec<NetworkInterface>,
    /// Connection information
    pub connections: ConnectionInfo,
}

/// Network interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// Interface name
    pub name: String,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Packets sent
    pub packets_sent: u64,
    /// Packets received
    pub packets_received: u64,
    /// Send errors
    pub errors_sent: u64,
    /// Receive errors
    pub errors_received: u64,
    /// Whether interface is up
    pub is_up: bool,
}

/// Network connection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    /// Established TCP connections
    pub tcp_established: u32,
    /// Listening TCP sockets
    pub tcp_listen: u32,
    /// TCP connections in TIME_WAIT state
    pub tcp_time_wait: u32,
    /// UDP sockets
    pub udp_sockets: u32,
}

/// Disk information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    /// Filesystem information
    pub filesystems: Vec<FilesystemInfo>,
    /// Disk I/O statistics
    pub io_stats: DiskIOStats,
}

/// Filesystem information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemInfo {
    /// Mount point
    pub mount_point: String,
    /// Filesystem type
    pub filesystem_type: String,
    /// Total bytes
    pub total_bytes: u64,
    /// Available bytes
    pub available_bytes: u64,
    /// Used bytes
    pub used_bytes: u64,
    /// Usage percentage
    pub usage_percent: f64,
    /// Total inodes
    pub inodes_total: u64,
    /// Used inodes
    pub inodes_used: u64,
    /// Available inodes
    pub inodes_available: u64,
}

/// Disk I/O statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIOStats {
    /// Number of read operations completed
    pub reads_completed: u64,
    /// Number of write operations completed
    pub writes_completed: u64,
    /// Bytes read
    pub bytes_read: u64,
    /// Bytes written
    pub bytes_written: u64,
    /// Time spent on I/O operations (milliseconds)
    pub io_time_ms: u64,
}

/// Environment information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    /// Hostname
    pub hostname: String,
    /// Current user
    pub user: String,
    /// Shell
    pub shell: String,
    /// PATH environment variable
    pub path: String,
    /// Timezone
    pub timezone: String,
    /// Safe environment variables
    pub environment_variables: HashMap<String, String>,
}

/// Runtime-specific metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMetrics {
    /// Number of garbage collections (0 for Rust)
    pub gc_collections: u64,
    /// Time spent in garbage collection
    pub gc_time_ms: u64,
    /// Currently allocated bytes
    pub allocated_bytes: u64,
    /// Heap size in bytes
    pub heap_size_bytes: u64,
    /// Stack size in bytes
    pub stack_size_bytes: u64,
    /// Thread pool metrics
    pub thread_pool: ThreadPoolMetrics,
    /// Active async tasks
    pub async_tasks_active: u32,
    /// Completed async tasks
    pub async_tasks_completed: u64,
}

/// Diagnostic trends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticTrends {
    /// Memory usage trend
    pub memory_usage_trend: TrendDirection,
    /// CPU usage trend
    pub cpu_usage_trend: TrendDirection,
    /// Disk usage trend
    pub disk_usage_trend: TrendDirection,
    /// Performance trend
    pub performance_trend: TrendDirection,
    /// Timestamp of trend calculation
    pub timestamp: u64,
}

/// Trend direction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    /// Metric is increasing
    Increasing,
    /// Metric is decreasing
    Decreasing,
    /// Metric is stable
    Stable,
}

impl Default for DiagnosticsCollector {
    fn default() -> Self {
        Self::new(Duration::from_secs(60), 100) // Collect every minute, keep 100 samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_diagnostics_collection() {
        let mut collector = DiagnosticsCollector::default();
        let snapshot = collector.collect_diagnostics().await.unwrap();

        assert!(snapshot.process_info.pid > 0);
        assert!(!snapshot.environment_info.hostname.is_empty());
        assert!(!snapshot.disk_info.filesystems.is_empty());
    }

    #[tokio::test]
    async fn test_trend_calculation() {
        let mut collector = DiagnosticsCollector::default();

        // Collect first snapshot
        collector.collect_diagnostics().await.unwrap();

        // Wait a bit and collect second snapshot
        tokio::time::sleep(Duration::from_millis(100)).await;
        collector.collect_diagnostics().await.unwrap();

        let trends = collector.calculate_trends();
        assert!(trends.is_some());

        let trends = trends.unwrap();
        assert!(matches!(trends.memory_usage_trend, TrendDirection::Stable));
    }

    #[test]
    fn test_safe_env_var_filtering() {
        let collector = DiagnosticsCollector::default();

        assert!(collector.is_safe_env_var("PATH"));
        assert!(collector.is_safe_env_var("HOME"));
        assert!(!collector.is_safe_env_var("PASSWORD"));
        assert!(!collector.is_safe_env_var("SECRET_KEY"));
        assert!(!collector.is_safe_env_var("API_TOKEN"));
    }
}
