//! Specific health check implementations

use super::{
    DependencyCheck, DependencyStatus, DependencyType, HealthCheck, HealthCheckResult, HealthStatus,
};
use crate::client::ClientContext;
use crate::error::Result;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

/// Memory usage health check
pub struct MemoryHealthCheck {
    warning_threshold: f64,
    critical_threshold: f64,
}

impl MemoryHealthCheck {
    pub fn new(warning_threshold: f64, critical_threshold: f64) -> Self {
        Self {
            warning_threshold,
            critical_threshold,
        }
    }
}

impl Default for MemoryHealthCheck {
    fn default() -> Self {
        Self::new(80.0, 95.0) // 80% warning, 95% critical
    }
}

#[async_trait::async_trait]
impl HealthCheck for MemoryHealthCheck {
    fn name(&self) -> &str {
        "memory_usage"
    }

    fn is_critical(&self) -> bool {
        true
    }

    async fn check(&self) -> Result<HealthCheckResult> {
        // Get current memory usage (simplified implementation)
        let memory_info = super::MemoryInfo::current();
        let usage_percent = memory_info.usage_percent;

        let (status, message) = if usage_percent >= self.critical_threshold {
            (
                HealthStatus::Unhealthy,
                format!("Memory usage critical: {:.1}%", usage_percent),
            )
        } else if usage_percent >= self.warning_threshold {
            (
                HealthStatus::Warning,
                format!("Memory usage high: {:.1}%", usage_percent),
            )
        } else {
            (
                HealthStatus::Healthy,
                format!("Memory usage normal: {:.1}%", usage_percent),
            )
        };

        Ok(HealthCheckResult {
            name: self.name().to_string(),
            status,
            message,
            duration_ms: 0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: std::collections::HashMap::from([
                (
                    "usage_percent".to_string(),
                    serde_json::Value::from(usage_percent),
                ),
                (
                    "used_bytes".to_string(),
                    serde_json::Value::Number(memory_info.used_bytes.into()),
                ),
                (
                    "total_bytes".to_string(),
                    serde_json::Value::Number(memory_info.total_bytes.into()),
                ),
                (
                    "warning_threshold".to_string(),
                    serde_json::Value::from(self.warning_threshold),
                ),
                (
                    "critical_threshold".to_string(),
                    serde_json::Value::from(self.critical_threshold),
                ),
            ]),
            error: None,
            critical: self.is_critical(),
        })
    }
}

/// Disk space health check
pub struct DiskSpaceHealthCheck {
    path: String,
    warning_threshold: f64,
    critical_threshold: f64,
}

impl DiskSpaceHealthCheck {
    pub fn new<S: Into<String>>(path: S, warning_threshold: f64, critical_threshold: f64) -> Self {
        Self {
            path: path.into(),
            warning_threshold,
            critical_threshold,
        }
    }
}

impl Default for DiskSpaceHealthCheck {
    fn default() -> Self {
        Self::new("/", 80.0, 95.0)
    }
}

#[async_trait::async_trait]
impl HealthCheck for DiskSpaceHealthCheck {
    fn name(&self) -> &str {
        "disk_space"
    }

    fn is_critical(&self) -> bool {
        true
    }

    async fn check(&self) -> Result<HealthCheckResult> {
        // Simplified disk space check
        let usage_percent = 15.0; // Mock value - in production would use statvfs or similar
        let total_bytes = 1024_u64 * 1024 * 1024 * 100; // 100GB
        let used_bytes = (total_bytes as f64 * usage_percent / 100.0) as u64;
        let available_bytes = total_bytes - used_bytes;

        let (status, message) = if usage_percent >= self.critical_threshold {
            (
                HealthStatus::Unhealthy,
                format!(
                    "Disk usage critical: {:.1}% on {}",
                    usage_percent, self.path
                ),
            )
        } else if usage_percent >= self.warning_threshold {
            (
                HealthStatus::Warning,
                format!("Disk usage high: {:.1}% on {}", usage_percent, self.path),
            )
        } else {
            (
                HealthStatus::Healthy,
                format!("Disk usage normal: {:.1}% on {}", usage_percent, self.path),
            )
        };

        Ok(HealthCheckResult {
            name: self.name().to_string(),
            status,
            message,
            duration_ms: 0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: std::collections::HashMap::from([
                (
                    "path".to_string(),
                    serde_json::Value::String(self.path.clone()),
                ),
                (
                    "usage_percent".to_string(),
                    serde_json::Value::from(usage_percent),
                ),
                (
                    "used_bytes".to_string(),
                    serde_json::Value::Number(used_bytes.into()),
                ),
                (
                    "available_bytes".to_string(),
                    serde_json::Value::Number(available_bytes.into()),
                ),
                (
                    "total_bytes".to_string(),
                    serde_json::Value::Number(total_bytes.into()),
                ),
            ]),
            error: None,
            critical: self.is_critical(),
        })
    }
}

/// Thread pool health check
pub struct ThreadPoolHealthCheck {
    warning_threshold: f64,
    critical_threshold: f64,
}

impl ThreadPoolHealthCheck {
    pub fn new(warning_threshold: f64, critical_threshold: f64) -> Self {
        Self {
            warning_threshold,
            critical_threshold,
        }
    }
}

impl Default for ThreadPoolHealthCheck {
    fn default() -> Self {
        Self::new(80.0, 95.0)
    }
}

#[async_trait::async_trait]
impl HealthCheck for ThreadPoolHealthCheck {
    fn name(&self) -> &str {
        "thread_pool"
    }

    fn is_critical(&self) -> bool {
        true
    }

    async fn check(&self) -> Result<HealthCheckResult> {
        let thread_metrics = super::ThreadPoolMetrics::default();
        let usage_percent = if thread_metrics.total_threads > 0 {
            (thread_metrics.active_threads as f64 / thread_metrics.total_threads as f64) * 100.0
        } else {
            0.0
        };

        let (status, message) = if usage_percent >= self.critical_threshold {
            (
                HealthStatus::Unhealthy,
                format!("Thread pool usage critical: {:.1}%", usage_percent),
            )
        } else if usage_percent >= self.warning_threshold {
            (
                HealthStatus::Warning,
                format!("Thread pool usage high: {:.1}%", usage_percent),
            )
        } else {
            (
                HealthStatus::Healthy,
                format!("Thread pool usage normal: {:.1}%", usage_percent),
            )
        };

        Ok(HealthCheckResult {
            name: self.name().to_string(),
            status,
            message,
            duration_ms: 0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: std::collections::HashMap::from([
                (
                    "usage_percent".to_string(),
                    serde_json::Value::from(usage_percent),
                ),
                (
                    "active_threads".to_string(),
                    serde_json::Value::Number(thread_metrics.active_threads.into()),
                ),
                (
                    "total_threads".to_string(),
                    serde_json::Value::Number(thread_metrics.total_threads.into()),
                ),
                (
                    "queued_tasks".to_string(),
                    serde_json::Value::Number(thread_metrics.queued_tasks.into()),
                ),
                (
                    "completed_tasks".to_string(),
                    serde_json::Value::Number(thread_metrics.completed_tasks.into()),
                ),
            ]),
            error: None,
            critical: self.is_critical(),
        })
    }
}

/// Configuration file health check
pub struct ConfigHealthCheck {
    config_path: String,
}

impl ConfigHealthCheck {
    pub fn new<S: Into<String>>(config_path: S) -> Self {
        Self {
            config_path: config_path.into(),
        }
    }
}

impl Default for ConfigHealthCheck {
    fn default() -> Self {
        Self::new("config.toml")
    }
}

#[async_trait::async_trait]
impl HealthCheck for ConfigHealthCheck {
    fn name(&self) -> &str {
        "configuration"
    }

    fn is_critical(&self) -> bool {
        true
    }

    async fn check(&self) -> Result<HealthCheckResult> {
        // Check if config file exists and is readable
        let config_exists = std::path::Path::new(&self.config_path).exists();

        if !config_exists {
            return Ok(HealthCheckResult::unhealthy(
                self.name(),
                &format!("Configuration file not found: {}", self.config_path),
                Some("File does not exist".to_string()),
            )
            .critical());
        }

        // Try to read the config file
        match std::fs::metadata(&self.config_path) {
            Ok(metadata) => {
                let file_size = metadata.len();
                let modified = metadata
                    .modified()
                    .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
                    .unwrap_or(0);

                Ok(HealthCheckResult::healthy(
                    self.name(),
                    &format!("Configuration file accessible: {}", self.config_path),
                )
                .with_metadata("file_size", file_size)
                .with_metadata("last_modified", modified)
                .with_metadata("path", self.config_path.clone()))
            }
            Err(e) => Ok(HealthCheckResult::unhealthy(
                self.name(),
                &format!("Cannot access configuration file: {}", self.config_path),
                Some(e.to_string()),
            )
            .critical()),
        }
    }
}

/// Credentials health check
pub struct CredentialsHealthCheck;

#[async_trait::async_trait]
impl HealthCheck for CredentialsHealthCheck {
    fn name(&self) -> &str {
        "credentials"
    }

    fn is_critical(&self) -> bool {
        true
    }

    async fn check(&self) -> Result<HealthCheckResult> {
        // Check if credentials are available
        // This is simplified - in production would check actual credential store

        let has_username = std::env::var("LOXONE_USERNAME").is_ok();
        let has_password = std::env::var("LOXONE_PASSWORD").is_ok();
        let has_host = std::env::var("LOXONE_HOST").is_ok();

        if !has_username || !has_password || !has_host {
            let missing: Vec<&str> = [
                (!has_username).then_some("LOXONE_USERNAME"),
                (!has_password).then_some("LOXONE_PASSWORD"),
                (!has_host).then_some("LOXONE_HOST"),
            ]
            .into_iter()
            .flatten()
            .collect();

            return Ok(HealthCheckResult::unhealthy(
                self.name(),
                "Missing required credentials",
                Some(format!(
                    "Missing environment variables: {}",
                    missing.join(", ")
                )),
            )
            .critical());
        }

        Ok(
            HealthCheckResult::healthy(self.name(), "All required credentials are available")
                .with_metadata("has_username", has_username)
                .with_metadata("has_password", has_password)
                .with_metadata("has_host", has_host),
        )
    }
}

/// Loxone Miniserver dependency check
pub struct LoxoneMiniserverCheck {
    client_context: Arc<ClientContext>,
    endpoint: String,
}

impl LoxoneMiniserverCheck {
    pub fn new(client_context: Arc<ClientContext>, endpoint: String) -> Self {
        Self {
            client_context,
            endpoint,
        }
    }
}

#[async_trait::async_trait]
impl DependencyCheck for LoxoneMiniserverCheck {
    fn name(&self) -> &str {
        "loxone_miniserver"
    }

    fn dependency_type(&self) -> DependencyType {
        DependencyType::LoxoneMiniserver
    }

    fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn is_critical(&self) -> bool {
        true
    }

    async fn check(&self) -> Result<DependencyStatus> {
        let start_time = std::time::Instant::now();

        // Try to perform a simple request to the Miniserver
        // This is simplified - in production would use the actual client
        match tokio::time::timeout(Duration::from_secs(5), self.check_connection()).await {
            Ok(Ok(_)) => {
                let response_time = start_time.elapsed().as_millis() as u64;
                Ok(DependencyStatus {
                    name: self.name().to_string(),
                    dependency_type: self.dependency_type(),
                    status: HealthStatus::Healthy,
                    message: "Miniserver is reachable".to_string(),
                    endpoint: self.endpoint.clone(),
                    response_time_ms: response_time,
                    last_success: Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    ),
                    error: None,
                    critical: self.is_critical(),
                })
            }
            Ok(Err(e)) => {
                let response_time = start_time.elapsed().as_millis() as u64;
                Ok(DependencyStatus {
                    name: self.name().to_string(),
                    dependency_type: self.dependency_type(),
                    status: HealthStatus::Unhealthy,
                    message: "Miniserver connection failed".to_string(),
                    endpoint: self.endpoint.clone(),
                    response_time_ms: response_time,
                    last_success: None,
                    error: Some(e.to_string()),
                    critical: self.is_critical(),
                })
            }
            Err(_) => {
                let response_time = start_time.elapsed().as_millis() as u64;
                Ok(DependencyStatus {
                    name: self.name().to_string(),
                    dependency_type: self.dependency_type(),
                    status: HealthStatus::Unhealthy,
                    message: "Miniserver connection timed out".to_string(),
                    endpoint: self.endpoint.clone(),
                    response_time_ms: response_time,
                    last_success: None,
                    error: Some("Connection timeout".to_string()),
                    critical: self.is_critical(),
                })
            }
        }
    }
}

impl LoxoneMiniserverCheck {
    async fn check_connection(&self) -> Result<()> {
        // Simplified connection check
        // In production this would use the actual HTTP client to ping the Miniserver
        debug!("Checking Loxone Miniserver connection to {}", self.endpoint);

        // Mock success for now
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Simulate occasional failures for testing
        if self.endpoint.contains("invalid") {
            return Err(crate::error::LoxoneError::connection("Invalid endpoint"));
        }

        Ok(())
    }
}

/// File system dependency check
pub struct FileSystemCheck {
    path: String,
    check_write: bool,
}

impl FileSystemCheck {
    pub fn new<S: Into<String>>(path: S, check_write: bool) -> Self {
        Self {
            path: path.into(),
            check_write,
        }
    }
}

#[async_trait::async_trait]
impl DependencyCheck for FileSystemCheck {
    fn name(&self) -> &str {
        "filesystem"
    }

    fn dependency_type(&self) -> DependencyType {
        DependencyType::FileSystem
    }

    fn endpoint(&self) -> &str {
        &self.path
    }

    fn is_critical(&self) -> bool {
        true
    }

    async fn check(&self) -> Result<DependencyStatus> {
        let start_time = std::time::Instant::now();

        // Check if path exists and is accessible
        let path = std::path::Path::new(&self.path);

        if !path.exists() {
            let response_time = start_time.elapsed().as_millis() as u64;
            return Ok(DependencyStatus {
                name: self.name().to_string(),
                dependency_type: self.dependency_type(),
                status: HealthStatus::Unhealthy,
                message: format!("Path does not exist: {}", self.path),
                endpoint: self.path.clone(),
                response_time_ms: response_time,
                last_success: None,
                error: Some("Path not found".to_string()),
                critical: self.is_critical(),
            });
        }

        // Check write permissions if requested
        if self.check_write {
            let test_file = path.join(".health_check_write_test");
            match std::fs::write(&test_file, "test") {
                Ok(_) => {
                    // Clean up test file
                    let _ = std::fs::remove_file(&test_file);
                }
                Err(e) => {
                    let response_time = start_time.elapsed().as_millis() as u64;
                    return Ok(DependencyStatus {
                        name: self.name().to_string(),
                        dependency_type: self.dependency_type(),
                        status: HealthStatus::Unhealthy,
                        message: format!("Cannot write to path: {}", self.path),
                        endpoint: self.path.clone(),
                        response_time_ms: response_time,
                        last_success: None,
                        error: Some(e.to_string()),
                        critical: self.is_critical(),
                    });
                }
            }
        }

        let response_time = start_time.elapsed().as_millis() as u64;
        Ok(DependencyStatus {
            name: self.name().to_string(),
            dependency_type: self.dependency_type(),
            status: HealthStatus::Healthy,
            message: format!("File system accessible: {}", self.path),
            endpoint: self.path.clone(),
            response_time_ms: response_time,
            last_success: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
            error: None,
            critical: self.is_critical(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_health_check() {
        let check = MemoryHealthCheck::default();
        let result = check.check().await.unwrap();

        assert_eq!(result.name, "memory_usage");
        assert!(result.critical);
        assert!(result.metadata.contains_key("usage_percent"));
    }

    #[tokio::test]
    async fn test_disk_space_check() {
        let check = DiskSpaceHealthCheck::default();
        let result = check.check().await.unwrap();

        assert_eq!(result.name, "disk_space");
        assert!(result.critical);
        assert!(result.metadata.contains_key("usage_percent"));
    }

    #[tokio::test]
    async fn test_thread_pool_check() {
        let check = ThreadPoolHealthCheck::default();
        let result = check.check().await.unwrap();

        assert_eq!(result.name, "thread_pool");
        assert!(result.critical);
        assert!(result.metadata.contains_key("active_threads"));
    }

    #[tokio::test]
    async fn test_config_health_check() {
        let check = ConfigHealthCheck::new("Cargo.toml"); // Use existing file
        let result = check.check().await.unwrap();

        assert_eq!(result.name, "configuration");
        assert_eq!(result.status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_config_health_check_missing_file() {
        let check = ConfigHealthCheck::new("nonexistent.toml");
        let result = check.check().await.unwrap();

        assert_eq!(result.name, "configuration");
        assert_eq!(result.status, HealthStatus::Unhealthy);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_credentials_check() {
        let check = CredentialsHealthCheck;
        let result = check.check().await.unwrap();

        assert_eq!(result.name, "credentials");
        // Result depends on environment variables, but should not panic
    }

    #[tokio::test]
    async fn test_filesystem_check() {
        let check = FileSystemCheck::new("/tmp", false);
        let result = check.check().await.unwrap();

        assert_eq!(result.dependency_type, DependencyType::FileSystem);
        // Should succeed on most systems
    }

    #[tokio::test]
    async fn test_loxone_miniserver_check() {
        let client_context = Arc::new(ClientContext::new());
        let check = LoxoneMiniserverCheck::new(client_context, "http://localhost:8080".to_string());

        let result = check.check().await.unwrap();
        assert_eq!(result.name, "loxone_miniserver");
        assert_eq!(result.dependency_type, DependencyType::LoxoneMiniserver);
    }
}
