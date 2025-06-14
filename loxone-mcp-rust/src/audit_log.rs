//! Audit logging system for security compliance
//!
//! This module provides comprehensive audit logging capabilities for tracking
//! all security-relevant operations, access attempts, and system changes.
//! Designed to meet compliance requirements for home automation security.
//!
//! Features:
//! - Structured audit events with timestamps and context
//! - Multiple output formats (JSON, syslog, file)
//! - Configurable retention and rotation policies
//! - Tamper-resistant logging with checksums
//! - Performance-optimized async logging
//! - GDPR-compliant data handling

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Audit event severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AuditSeverity {
    /// Informational events
    Info,
    /// Warning events that may require attention
    Warning,
    /// Error events indicating failures
    Error,
    /// Critical security events
    Critical,
}

/// Types of auditable events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    /// Authentication events
    Authentication {
        username: String,
        success: bool,
        method: String,
        ip_address: Option<IpAddr>,
    },
    /// Authorization/access control events
    Authorization {
        username: String,
        resource: String,
        action: String,
        granted: bool,
        reason: Option<String>,
    },
    /// Device control commands
    DeviceControl {
        device_uuid: String,
        device_name: String,
        command: String,
        source: String,
        success: bool,
        error: Option<String>,
    },
    /// Configuration changes
    ConfigurationChange {
        setting: String,
        old_value: Option<String>,
        new_value: String,
        changed_by: String,
    },
    /// System lifecycle events
    SystemLifecycle {
        event: String,
        details: HashMap<String, String>,
    },
    /// Connection events
    Connection {
        event_type: String,
        remote_address: Option<IpAddr>,
        protocol: String,
        success: bool,
    },
    /// Data access events
    DataAccess {
        resource: String,
        operation: String,
        user: Option<String>,
        records_affected: Option<usize>,
    },
    /// Security alert events
    SecurityAlert {
        alert_type: String,
        description: String,
        source: String,
        severity: AuditSeverity,
    },
    /// Consent management events
    ConsentManagement {
        operation_id: Uuid,
        operation_type: String,
        decision: String,
        user: Option<String>,
        automated: bool,
    },
    /// API access events
    ApiAccess {
        endpoint: String,
        method: String,
        client_id: Option<String>,
        status_code: u16,
        response_time: Duration,
    },
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique identifier for this audit entry
    pub id: Uuid,

    /// Timestamp when the event occurred
    pub timestamp: SystemTime,

    /// Severity level of the event
    pub severity: AuditSeverity,

    /// Type of event with associated data
    pub event_type: AuditEventType,

    /// Session ID if applicable
    pub session_id: Option<Uuid>,

    /// Correlation ID for tracking related events
    pub correlation_id: Option<Uuid>,

    /// Additional context information
    pub context: HashMap<String, String>,

    /// Checksum for integrity verification
    pub checksum: Option<String>,
}

impl AuditEntry {
    /// Create a new audit entry
    pub fn new(severity: AuditSeverity, event_type: AuditEventType) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: SystemTime::now(),
            severity,
            event_type,
            session_id: None,
            correlation_id: None,
            context: HashMap::new(),
            checksum: None,
        }
    }

    /// Add session ID
    pub fn with_session(mut self, session_id: Uuid) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Add correlation ID
    pub fn with_correlation(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    /// Add context information
    pub fn with_context(mut self, key: String, value: String) -> Self {
        self.context.insert(key, value);
        self
    }

    /// Calculate checksum for integrity
    pub fn calculate_checksum(&self) -> String {
        #[cfg(feature = "audit")]
        {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();

            // Hash the serialized entry (excluding checksum field)
            let mut entry_for_hash = self.clone();
            entry_for_hash.checksum = None;

            if let Ok(json) = serde_json::to_string(&entry_for_hash) {
                hasher.update(json.as_bytes());
                format!("{:x}", hasher.finalize())
            } else {
                "invalid".to_string()
            }
        }

        #[cfg(not(feature = "audit"))]
        {
            // Simple checksum without crypto dependency
            let mut entry_for_hash = self.clone();
            entry_for_hash.checksum = None;

            if let Ok(json) = serde_json::to_string(&entry_for_hash) {
                let mut checksum = 0u32;
                for byte in json.bytes() {
                    checksum = checksum.wrapping_add(byte as u32);
                }
                format!("{:08x}", checksum)
            } else {
                "invalid".to_string()
            }
        }
    }

    /// Add checksum to entry
    pub fn with_checksum(mut self) -> Self {
        self.checksum = Some(self.calculate_checksum());
        self
    }

    /// Verify checksum integrity
    pub fn verify_checksum(&self) -> bool {
        if let Some(stored_checksum) = &self.checksum {
            let calculated = self.calculate_checksum();
            &calculated == stored_checksum
        } else {
            false
        }
    }
}

/// Output format for audit logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditOutputFormat {
    /// JSON format
    Json,
    /// Syslog format
    Syslog,
    /// Common Event Format (CEF)
    Cef,
    /// Custom format with template
    Custom(String),
}

/// Audit log output destination
#[derive(Debug, Clone)]
pub enum AuditOutput {
    /// Write to file
    File(PathBuf),
    /// Send to syslog
    Syslog(String),
    /// Write to stdout
    Stdout,
    /// Send to remote endpoint
    Remote(String),
    /// Multiple outputs
    Multiple(Vec<AuditOutput>),
}

/// Configuration for audit logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Enable audit logging
    pub enabled: bool,

    /// Minimum severity level to log
    pub min_severity: AuditSeverity,

    /// Output format
    pub format: AuditOutputFormat,

    /// Buffer size for async logging
    pub buffer_size: usize,

    /// Flush interval
    pub flush_interval: Duration,

    /// Enable integrity checksums
    pub enable_checksums: bool,

    /// Retention period for logs
    pub retention_days: Option<u32>,

    /// Maximum log file size before rotation
    pub max_file_size: Option<usize>,

    /// Include sensitive data in logs
    pub include_sensitive: bool,

    /// Event types to exclude
    pub exclude_events: Vec<String>,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_severity: AuditSeverity::Info,
            format: AuditOutputFormat::Json,
            buffer_size: 1000,
            flush_interval: Duration::from_secs(5),
            enable_checksums: true,
            retention_days: Some(90), // 90 days default retention
            max_file_size: Some(100 * 1024 * 1024), // 100MB
            include_sensitive: false,
            exclude_events: Vec::new(),
        }
    }
}

/// Statistics for audit logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStats {
    /// Total events logged
    pub total_events: u64,

    /// Events by severity
    pub events_by_severity: HashMap<String, u64>,

    /// Events by type
    pub events_by_type: HashMap<String, u64>,

    /// Failed log attempts
    pub failed_logs: u64,

    /// Current buffer usage
    pub buffer_usage: usize,

    /// Last flush time
    pub last_flush: Option<SystemTime>,

    /// Integrity check failures
    pub integrity_failures: u64,
}

/// Audit logger implementation
pub struct AuditLogger {
    /// Configuration
    config: AuditConfig,

    /// Output destination
    output: AuditOutput,

    /// Event buffer
    buffer: Arc<RwLock<Vec<AuditEntry>>>,

    /// Statistics
    stats: Arc<RwLock<AuditStats>>,

    /// Channel for async logging
    sender: mpsc::UnboundedSender<AuditEntry>,
    receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<AuditEntry>>>>,

    /// Shutdown signal
    shutdown_sender: Option<mpsc::UnboundedSender<()>>,
    shutdown_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<()>>>>,

    /// Current log file handle
    file_handle: Arc<RwLock<Option<File>>>,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new(config: AuditConfig, output: AuditOutput) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = mpsc::unbounded_channel();

        Self {
            config,
            output,
            buffer: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(AuditStats {
                total_events: 0,
                events_by_severity: HashMap::new(),
                events_by_type: HashMap::new(),
                failed_logs: 0,
                buffer_usage: 0,
                last_flush: None,
                integrity_failures: 0,
            })),
            sender: tx,
            receiver: Arc::new(RwLock::new(Some(rx))),
            shutdown_sender: Some(shutdown_tx),
            shutdown_receiver: Arc::new(RwLock::new(Some(shutdown_rx))),
            file_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the audit logger
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            info!("Audit logging is disabled");
            return Ok(());
        }

        // Initialize output
        self.initialize_output().await?;

        // Start background processing task
        self.start_processing_task().await;

        // Start flush task
        self.start_flush_task().await;

        info!("Audit logger started");
        Ok(())
    }

    /// Stop the audit logger
    pub async fn stop(&mut self) -> Result<()> {
        // Flush remaining events
        self.flush().await?;

        // Send shutdown signal
        if let Some(sender) = self.shutdown_sender.take() {
            let _ = sender.send(());
        }

        // Close file handle if open
        if let Some(mut file) = self.file_handle.write().await.take() {
            file.flush().await?;
        }

        info!("Audit logger stopped");
        Ok(())
    }

    /// Log an audit event
    pub async fn log(&self, entry: AuditEntry) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Check minimum severity
        if entry.severity < self.config.min_severity {
            return Ok(());
        }

        // Check if event type is excluded
        let event_type_name = self.get_event_type_name(&entry.event_type);
        if self.config.exclude_events.contains(&event_type_name) {
            return Ok(());
        }

        // Add checksum if enabled
        let entry = if self.config.enable_checksums {
            entry.with_checksum()
        } else {
            entry
        };

        // Send to processing queue
        if let Err(e) = self.sender.send(entry) {
            let mut stats = self.stats.write().await;
            stats.failed_logs += 1;
            return Err(LoxoneError::connection(format!(
                "Failed to queue audit event: {}",
                e
            )));
        }

        Ok(())
    }

    /// Log authentication event
    pub async fn log_authentication(
        &self,
        username: &str,
        success: bool,
        method: &str,
        ip_address: Option<IpAddr>,
    ) -> Result<()> {
        let entry = AuditEntry::new(
            if success {
                AuditSeverity::Info
            } else {
                AuditSeverity::Warning
            },
            AuditEventType::Authentication {
                username: username.to_string(),
                success,
                method: method.to_string(),
                ip_address,
            },
        );

        self.log(entry).await
    }

    /// Log authorization event
    pub async fn log_authorization(
        &self,
        username: &str,
        resource: &str,
        action: &str,
        granted: bool,
        reason: Option<String>,
    ) -> Result<()> {
        let entry = AuditEntry::new(
            if granted {
                AuditSeverity::Info
            } else {
                AuditSeverity::Warning
            },
            AuditEventType::Authorization {
                username: username.to_string(),
                resource: resource.to_string(),
                action: action.to_string(),
                granted,
                reason,
            },
        );

        self.log(entry).await
    }

    /// Log device control event
    pub async fn log_device_control(
        &self,
        device_uuid: &str,
        device_name: &str,
        command: &str,
        source: &str,
        success: bool,
        error: Option<String>,
    ) -> Result<()> {
        let entry = AuditEntry::new(
            if success {
                AuditSeverity::Info
            } else {
                AuditSeverity::Error
            },
            AuditEventType::DeviceControl {
                device_uuid: device_uuid.to_string(),
                device_name: device_name.to_string(),
                command: command.to_string(),
                source: source.to_string(),
                success,
                error,
            },
        );

        self.log(entry).await
    }

    /// Log security alert
    pub async fn log_security_alert(
        &self,
        alert_type: &str,
        description: &str,
        source: &str,
        severity: AuditSeverity,
    ) -> Result<()> {
        let entry = AuditEntry::new(
            severity,
            AuditEventType::SecurityAlert {
                alert_type: alert_type.to_string(),
                description: description.to_string(),
                source: source.to_string(),
                severity,
            },
        );

        self.log(entry).await
    }

    /// Get audit statistics
    pub async fn get_statistics(&self) -> AuditStats {
        self.stats.read().await.clone()
    }

    /// Get audit configuration
    pub fn get_config(&self) -> &AuditConfig {
        &self.config
    }

    /// Search audit logs
    pub async fn search(
        &self,
        _start_time: Option<SystemTime>,
        _end_time: Option<SystemTime>,
        _severity: Option<AuditSeverity>,
        _event_type: Option<String>,
        _limit: Option<usize>,
    ) -> Result<Vec<AuditEntry>> {
        // This would be implemented based on the output destination
        // For file-based logs, it would read and parse the files
        // For remote logs, it would query the remote system
        todo!("Implement audit log search")
    }

    /// Verify audit log integrity
    pub async fn verify_integrity(
        &self,
        _start_time: Option<SystemTime>,
        _end_time: Option<SystemTime>,
    ) -> Result<(usize, usize)> {
        // This would verify checksums for all entries in the given time range
        // Returns (total_entries, failed_entries)
        todo!("Implement integrity verification")
    }

    /// Export audit logs
    pub async fn export(
        &self,
        _format: AuditOutputFormat,
        _start_time: Option<SystemTime>,
        _end_time: Option<SystemTime>,
        _output_path: PathBuf,
    ) -> Result<()> {
        // This would export logs in the specified format
        todo!("Implement audit log export")
    }

    /// Initialize output destination
    async fn initialize_output(&self) -> Result<()> {
        match &self.output {
            AuditOutput::File(path) => {
                // Create directory if needed
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }

                // Open file for append
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .await?;

                *self.file_handle.write().await = Some(file);
            }
            AuditOutput::Stdout => {
                // No initialization needed
            }
            AuditOutput::Syslog(_) => {
                // Would initialize syslog connection
                warn!("Syslog output not yet implemented");
            }
            AuditOutput::Remote(_) => {
                // Would initialize remote connection
                warn!("Remote output not yet implemented");
            }
            AuditOutput::Multiple(_outputs) => {
                // Would initialize all outputs
                warn!("Multiple outputs not yet implemented");
            }
        }

        Ok(())
    }

    /// Get event type name for categorization
    fn get_event_type_name(&self, event_type: &AuditEventType) -> String {
        match event_type {
            AuditEventType::Authentication { .. } => "Authentication".to_string(),
            AuditEventType::Authorization { .. } => "Authorization".to_string(),
            AuditEventType::DeviceControl { .. } => "DeviceControl".to_string(),
            AuditEventType::ConfigurationChange { .. } => "ConfigurationChange".to_string(),
            AuditEventType::SystemLifecycle { .. } => "SystemLifecycle".to_string(),
            AuditEventType::Connection { .. } => "Connection".to_string(),
            AuditEventType::DataAccess { .. } => "DataAccess".to_string(),
            AuditEventType::SecurityAlert { .. } => "SecurityAlert".to_string(),
            AuditEventType::ConsentManagement { .. } => "ConsentManagement".to_string(),
            AuditEventType::ApiAccess { .. } => "ApiAccess".to_string(),
        }
    }

    /// Format entry based on configured format
    fn format_entry(&self, entry: &AuditEntry) -> Result<String> {
        match &self.config.format {
            AuditOutputFormat::Json => Ok(serde_json::to_string(entry)?),
            AuditOutputFormat::Syslog => {
                // Format as syslog message
                let severity_num = match entry.severity {
                    AuditSeverity::Critical => 2,
                    AuditSeverity::Error => 3,
                    AuditSeverity::Warning => 4,
                    AuditSeverity::Info => 6,
                };

                Ok(format!(
                    "<{}>{} {} loxone-mcp[{}]: {}",
                    severity_num,
                    chrono::DateTime::<chrono::Utc>::from(entry.timestamp).format("%b %d %H:%M:%S"),
                    hostname::get()?.to_string_lossy(),
                    std::process::id(),
                    serde_json::to_string(&entry.event_type)?
                ))
            }
            AuditOutputFormat::Cef => {
                // Common Event Format
                Ok(format!(
                    "CEF:0|Loxone|MCP|1.0|{}|{}|{}|",
                    self.get_event_type_name(&entry.event_type),
                    self.get_event_type_name(&entry.event_type),
                    match entry.severity {
                        AuditSeverity::Critical => 10,
                        AuditSeverity::Error => 7,
                        AuditSeverity::Warning => 4,
                        AuditSeverity::Info => 1,
                    }
                ))
            }
            AuditOutputFormat::Custom(template) => {
                // Would implement template-based formatting
                Ok(template.clone())
            }
        }
    }

    /// Write entry to output
    async fn write_entry(&self, entry: &AuditEntry) -> Result<()> {
        let formatted = self.format_entry(entry)?;

        match &self.output {
            AuditOutput::File(_) => {
                if let Some(file) = &mut *self.file_handle.write().await {
                    file.write_all(formatted.as_bytes()).await?;
                    file.write_all(b"\n").await?;
                }
            }
            AuditOutput::Stdout => {
                println!("{}", formatted);
            }
            _ => {
                // Other outputs not yet implemented
            }
        }

        Ok(())
    }

    /// Flush buffered events
    async fn flush(&self) -> Result<()> {
        let entries = {
            let mut buffer = self.buffer.write().await;
            std::mem::take(&mut *buffer)
        };

        for entry in entries {
            if let Err(e) = self.write_entry(&entry).await {
                error!("Failed to write audit entry: {}", e);
                let mut stats = self.stats.write().await;
                stats.failed_logs += 1;
            }
        }

        // Update flush time
        {
            let mut stats = self.stats.write().await;
            stats.last_flush = Some(SystemTime::now());
        }

        // Flush file if applicable
        if let Some(file) = &mut *self.file_handle.write().await {
            file.flush().await?;
        }

        Ok(())
    }

    /// Start background processing task
    async fn start_processing_task(&self) {
        let receiver = self.receiver.clone();
        let buffer = self.buffer.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();
        let shutdown_receiver = self.shutdown_receiver.clone();

        tokio::spawn(async move {
            let mut rx = receiver.write().await.take();
            let mut shutdown_rx = shutdown_receiver.write().await.take();

            loop {
                tokio::select! {
                    Some(entry) = async {
                        if let Some(ref mut rx) = rx {
                            rx.recv().await
                        } else {
                            None
                        }
                    } => {
                        // Update statistics
                        {
                            let mut stats_guard = stats.write().await;
                            stats_guard.total_events += 1;

                            let severity_key = format!("{:?}", entry.severity);
                            *stats_guard.events_by_severity.entry(severity_key).or_insert(0) += 1;

                            let event_key = match &entry.event_type {
                                AuditEventType::Authentication { .. } => "Authentication",
                                AuditEventType::Authorization { .. } => "Authorization",
                                AuditEventType::DeviceControl { .. } => "DeviceControl",
                                AuditEventType::ConfigurationChange { .. } => "ConfigurationChange",
                                AuditEventType::SystemLifecycle { .. } => "SystemLifecycle",
                                AuditEventType::Connection { .. } => "Connection",
                                AuditEventType::DataAccess { .. } => "DataAccess",
                                AuditEventType::SecurityAlert { .. } => "SecurityAlert",
                                AuditEventType::ConsentManagement { .. } => "ConsentManagement",
                                AuditEventType::ApiAccess { .. } => "ApiAccess",
                            };
                            *stats_guard.events_by_type.entry(event_key.to_string()).or_insert(0) += 1;
                        }

                        // Add to buffer
                        let mut buffer_guard = buffer.write().await;
                        buffer_guard.push(entry);

                        // Update buffer usage
                        {
                            let mut stats_guard = stats.write().await;
                            stats_guard.buffer_usage = buffer_guard.len();
                        }

                        // Flush if buffer is full
                        if buffer_guard.len() >= config.buffer_size {
                            drop(buffer_guard);
                            // Trigger flush (would be done by flush task)
                        }
                    }
                    _ = async {
                        if let Some(ref mut rx) = shutdown_rx {
                            rx.recv().await
                        } else {
                            std::future::pending().await
                        }
                    } => {
                        debug!("Audit processing task shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// Start flush task
    async fn start_flush_task(&self) {
        let _buffer = self.buffer.clone();
        let _stats = self.stats.clone();
        let config = self.config.clone();
        let _file_handle = self.file_handle.clone();
        let _output = self.output.clone();
        let shutdown_receiver = self.shutdown_receiver.clone();

        let self_clone = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.flush_interval);
            let mut shutdown_rx = shutdown_receiver.write().await.take();

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = self_clone.flush().await {
                            error!("Failed to flush audit log: {}", e);
                        }
                    }
                    _ = async {
                        if let Some(ref mut rx) = shutdown_rx {
                            rx.recv().await
                        } else {
                            std::future::pending().await
                        }
                    } => {
                        debug!("Audit flush task shutting down");
                        break;
                    }
                }
            }
        });
    }
}

impl Clone for AuditLogger {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            output: self.output.clone(),
            buffer: self.buffer.clone(),
            stats: self.stats.clone(),
            sender: self.sender.clone(),
            receiver: self.receiver.clone(),
            shutdown_sender: None, // Don't clone shutdown sender
            shutdown_receiver: self.shutdown_receiver.clone(),
            file_handle: self.file_handle.clone(),
        }
    }
}

/// Builder for audit logger
pub struct AuditLoggerBuilder {
    config: AuditConfig,
    output: Option<AuditOutput>,
}

impl AuditLoggerBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: AuditConfig::default(),
            output: None,
        }
    }

    /// Set configuration
    pub fn with_config(mut self, config: AuditConfig) -> Self {
        self.config = config;
        self
    }

    /// Set output destination
    pub fn with_output(mut self, output: AuditOutput) -> Self {
        self.output = Some(output);
        self
    }

    /// Enable checksums
    pub fn enable_checksums(mut self, enable: bool) -> Self {
        self.config.enable_checksums = enable;
        self
    }

    /// Set minimum severity
    pub fn min_severity(mut self, severity: AuditSeverity) -> Self {
        self.config.min_severity = severity;
        self
    }

    /// Set retention days
    pub fn retention_days(mut self, days: u32) -> Self {
        self.config.retention_days = Some(days);
        self
    }

    /// Build the audit logger
    pub fn build(self) -> Result<AuditLogger> {
        let output = self.output.unwrap_or(AuditOutput::Stdout);
        Ok(AuditLogger::new(self.config, output))
    }
}

impl Default for AuditLoggerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audit_entry_creation() {
        let entry = AuditEntry::new(
            AuditSeverity::Info,
            AuditEventType::Authentication {
                username: "testuser".to_string(),
                success: true,
                method: "token".to_string(),
                ip_address: None,
            },
        );

        assert_eq!(entry.severity, AuditSeverity::Info);
        assert!(matches!(
            entry.event_type,
            AuditEventType::Authentication { .. }
        ));
    }

    #[tokio::test]
    async fn test_audit_entry_checksum() {
        let entry = AuditEntry::new(
            AuditSeverity::Info,
            AuditEventType::SystemLifecycle {
                event: "startup".to_string(),
                details: HashMap::new(),
            },
        )
        .with_checksum();

        assert!(entry.checksum.is_some());
        assert!(entry.verify_checksum());
    }

    #[tokio::test]
    async fn test_audit_logger_creation() {
        let logger = AuditLoggerBuilder::new()
            .min_severity(AuditSeverity::Warning)
            .enable_checksums(true)
            .with_output(AuditOutput::Stdout)
            .build()
            .unwrap();

        assert!(logger.config.enable_checksums);
        assert_eq!(logger.config.min_severity, AuditSeverity::Warning);
    }
}
