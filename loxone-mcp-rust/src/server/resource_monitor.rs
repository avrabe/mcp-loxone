//! Resource monitoring and management for the MCP server
//!
//! This module provides monitoring of system resources and enforces limits
//! to prevent resource exhaustion.

use crate::error::{LoxoneError, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::System;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Resource limits configuration
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory usage in bytes
    pub max_memory_bytes: Option<u64>,

    /// Maximum CPU usage percentage
    pub max_cpu_percent: Option<f32>,

    /// Maximum number of concurrent requests
    pub max_concurrent_requests: usize,

    /// Maximum request duration
    pub max_request_duration: Duration,

    /// Memory warning threshold (percentage of max)
    pub memory_warning_threshold: f32,

    /// CPU warning threshold (percentage of max)
    pub cpu_warning_threshold: f32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: Some(512 * 1024 * 1024), // 512 MB
            max_cpu_percent: Some(80.0),
            max_concurrent_requests: 100,
            max_request_duration: Duration::from_secs(60),
            memory_warning_threshold: 0.8,
            cpu_warning_threshold: 0.8,
        }
    }
}

/// Resource usage statistics
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    /// Current memory usage in bytes
    pub memory_bytes: u64,

    /// Current CPU usage percentage
    pub cpu_percent: f32,

    /// Active request count
    pub active_requests: usize,

    /// Total requests processed
    pub total_requests: u64,

    /// Longest request duration
    pub max_request_duration: Duration,

    /// Average request duration
    pub avg_request_duration: Duration,

    /// Number of resource limit hits
    pub limit_hits: u64,

    /// Last update time
    pub last_update: Option<Instant>,
}

/// Resource monitor for tracking and limiting resource usage
pub struct ResourceMonitor {
    /// Resource limits
    limits: ResourceLimits,

    /// Current usage statistics
    usage: Arc<Mutex<ResourceUsage>>,

    /// System information
    system: Arc<Mutex<System>>,

    /// Process ID
    pid: sysinfo::Pid,

    /// Active request tracking
    active_requests: Arc<Mutex<Vec<ActiveRequest>>>,
}

/// Active request information
#[derive(Debug)]
struct ActiveRequest {
    /// Request ID
    id: String,

    /// Start time
    started_at: Instant,

    /// Tool name
    #[allow(dead_code)]
    tool_name: String,
}

impl ResourceMonitor {
    /// Create a new resource monitor
    pub fn new(limits: ResourceLimits) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        let pid = sysinfo::Pid::from(std::process::id() as usize);

        info!("Resource monitor initialized with limits: {:?}", limits);

        Self {
            limits,
            usage: Arc::new(Mutex::new(ResourceUsage::default())),
            system: Arc::new(Mutex::new(system)),
            pid,
            active_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Check if resources are available for a new request
    pub async fn check_resources(&self, tool_name: &str) -> Result<ResourcePermit> {
        self.update_usage().await;

        let usage = self.usage.lock().await;

        // Check memory limit
        if let Some(max_memory) = self.limits.max_memory_bytes {
            if usage.memory_bytes > max_memory {
                return Err(LoxoneError::resource_exhausted(format!(
                    "Memory limit exceeded: {} MB > {} MB",
                    usage.memory_bytes / (1024 * 1024),
                    max_memory / (1024 * 1024)
                )));
            }

            // Warn if approaching limit
            let warning_threshold =
                (max_memory as f32 * self.limits.memory_warning_threshold) as u64;
            if usage.memory_bytes > warning_threshold {
                warn!(
                    "Memory usage approaching limit: {} MB / {} MB",
                    usage.memory_bytes / (1024 * 1024),
                    max_memory / (1024 * 1024)
                );
            }
        }

        // Check CPU limit
        if let Some(max_cpu) = self.limits.max_cpu_percent {
            if usage.cpu_percent > max_cpu {
                return Err(LoxoneError::resource_exhausted(format!(
                    "CPU limit exceeded: {:.1}% > {:.1}%",
                    usage.cpu_percent, max_cpu
                )));
            }

            // Warn if approaching limit
            let warning_threshold = max_cpu * self.limits.cpu_warning_threshold;
            if usage.cpu_percent > warning_threshold {
                warn!(
                    "CPU usage approaching limit: {:.1}% / {:.1}%",
                    usage.cpu_percent, max_cpu
                );
            }
        }

        // Check concurrent request limit
        if usage.active_requests >= self.limits.max_concurrent_requests {
            return Err(LoxoneError::resource_exhausted(format!(
                "Concurrent request limit exceeded: {} >= {}",
                usage.active_requests, self.limits.max_concurrent_requests
            )));
        }

        drop(usage);

        // Create request entry
        let request_id = uuid::Uuid::new_v4().to_string();
        let active_request = ActiveRequest {
            id: request_id.clone(),
            started_at: Instant::now(),
            tool_name: tool_name.to_string(),
        };

        // Add to active requests
        {
            let mut requests = self.active_requests.lock().await;
            requests.push(active_request);
        }

        // Update usage
        {
            let mut usage = self.usage.lock().await;
            usage.active_requests += 1;
            usage.total_requests += 1;
        }

        Ok(ResourcePermit {
            usage: self.usage.clone(),
            active_requests: self.active_requests.clone(),
            request_id,
            started_at: Instant::now(),
        })
    }

    /// Update resource usage statistics
    async fn update_usage(&self) {
        let mut system = self.system.lock().await;
        system.refresh_process(self.pid);

        if let Some(process) = system.process(self.pid) {
            let mut usage = self.usage.lock().await;
            usage.memory_bytes = process.memory();
            usage.cpu_percent = process.cpu_usage();
            usage.last_update = Some(Instant::now());

            debug!(
                "Resource usage: {} MB memory, {:.1}% CPU",
                usage.memory_bytes / (1024 * 1024),
                usage.cpu_percent
            );
        }
    }

    /// Release resources for a completed request
    async fn release(&self, request_id: &str, duration: Duration) {
        // Remove from active requests
        {
            let mut requests = self.active_requests.lock().await;
            requests.retain(|r| r.id != request_id);
        }

        // Update usage statistics
        {
            let mut usage = self.usage.lock().await;
            usage.active_requests = usage.active_requests.saturating_sub(1);

            // Update duration statistics
            if duration > usage.max_request_duration {
                usage.max_request_duration = duration;
            }

            // Update average (simple moving average)
            let total_duration_ms = usage.avg_request_duration.as_millis() as u64
                * (usage.total_requests - 1)
                + duration.as_millis() as u64;
            usage.avg_request_duration =
                Duration::from_millis(total_duration_ms / usage.total_requests);

            // Check if request exceeded limit
            if duration > self.limits.max_request_duration {
                warn!(
                    "Request {} exceeded duration limit: {:?} > {:?}",
                    request_id, duration, self.limits.max_request_duration
                );
                usage.limit_hits += 1;
            }
        }
    }

    /// Get current resource usage
    pub async fn get_usage(&self) -> ResourceUsage {
        self.update_usage().await;
        self.usage.lock().await.clone()
    }

    /// Clean up stale requests (safety mechanism)
    pub async fn cleanup_stale_requests(&self) {
        let now = Instant::now();
        let mut stale_requests = Vec::new();

        {
            let requests = self.active_requests.lock().await;
            for request in requests.iter() {
                if now.duration_since(request.started_at) > self.limits.max_request_duration * 2 {
                    stale_requests.push(request.id.clone());
                }
            }
        }

        for request_id in stale_requests {
            warn!("Cleaning up stale request: {}", request_id);
            self.release(&request_id, self.limits.max_request_duration)
                .await;
        }
    }
}

/// Resource permit that must be held during request processing
pub struct ResourcePermit {
    /// Usage tracking
    usage: Arc<Mutex<ResourceUsage>>,

    /// Active requests tracking
    active_requests: Arc<Mutex<Vec<ActiveRequest>>>,

    /// Request ID
    request_id: String,

    /// Start time
    started_at: Instant,
}

impl Drop for ResourcePermit {
    fn drop(&mut self) {
        let usage = self.usage.clone();
        let active_requests = self.active_requests.clone();
        let request_id = self.request_id.clone();
        let duration = self.started_at.elapsed();

        // Release resources asynchronously
        tokio::spawn(async move {
            // Remove from active requests
            {
                let mut requests = active_requests.lock().await;
                requests.retain(|r| r.id != request_id);
            }

            // Update usage statistics
            {
                let mut usage = usage.lock().await;
                usage.active_requests = usage.active_requests.saturating_sub(1);

                // Update duration statistics
                if duration > usage.max_request_duration {
                    usage.max_request_duration = duration;
                }

                // Update average (simple moving average)
                if usage.total_requests > 0 {
                    let total_duration_ms = usage.avg_request_duration.as_millis() as u64
                        * (usage.total_requests.saturating_sub(1))
                        + duration.as_millis() as u64;
                    usage.avg_request_duration =
                        Duration::from_millis(total_duration_ms / usage.total_requests);
                }
            }
        });
    }
}

/// Resource health information
#[derive(Debug, Clone)]
pub struct ResourceHealth {
    /// Overall health status
    pub healthy: bool,

    /// Memory usage percentage
    pub memory_percent: f32,

    /// CPU usage percentage  
    pub cpu_percent: f32,

    /// Request queue utilization
    pub request_utilization: f32,

    /// Any warnings
    pub warnings: Vec<String>,
}

impl ResourceMonitor {
    /// Get resource health status
    pub async fn health_check(&self) -> ResourceHealth {
        let usage = self.get_usage().await;
        let mut warnings = Vec::new();

        let memory_percent = if let Some(max_memory) = self.limits.max_memory_bytes {
            let percent = (usage.memory_bytes as f32 / max_memory as f32) * 100.0;
            if percent > self.limits.memory_warning_threshold * 100.0 {
                warnings.push(format!("Memory usage high: {:.1}%", percent));
            }
            percent
        } else {
            0.0
        };

        let cpu_percent = usage.cpu_percent;
        if let Some(max_cpu) = self.limits.max_cpu_percent {
            if cpu_percent > max_cpu * self.limits.cpu_warning_threshold {
                warnings.push(format!("CPU usage high: {:.1}%", cpu_percent));
            }
        }

        let request_utilization =
            (usage.active_requests as f32 / self.limits.max_concurrent_requests as f32) * 100.0;
        if request_utilization > 80.0 {
            warnings.push(format!("Request queue high: {:.1}%", request_utilization));
        }

        ResourceHealth {
            healthy: warnings.is_empty() && usage.limit_hits == 0,
            memory_percent,
            cpu_percent,
            request_utilization,
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resource_limits() {
        let limits = ResourceLimits {
            max_concurrent_requests: 2,
            ..Default::default()
        };

        let monitor = ResourceMonitor::new(limits);

        // Should allow first request
        let permit1 = monitor.check_resources("test1").await.unwrap();
        let usage = monitor.get_usage().await;
        assert_eq!(usage.active_requests, 1);

        // Should allow second request
        let _permit2 = monitor.check_resources("test2").await.unwrap();
        let usage = monitor.get_usage().await;
        assert_eq!(usage.active_requests, 2);

        // Should reject third request
        let result = monitor.check_resources("test3").await;
        assert!(result.is_err());

        // Drop a permit
        drop(permit1);
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Should now allow another request
        let _permit3 = monitor.check_resources("test3").await.unwrap();
    }

    #[tokio::test]
    async fn test_resource_health() {
        let monitor = ResourceMonitor::new(ResourceLimits::default());

        let health = monitor.health_check().await;
        assert!(health.healthy);
        assert!(health.warnings.is_empty());
    }
}
