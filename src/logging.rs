//! Enhanced logging configuration with file rotation
//!
//! This module provides comprehensive logging setup with:
//! - File-based logging with rotation
//! - Structured logging with context
//! - Request/response logging
//! - Performance metrics

pub mod metrics;
pub mod sanitization;
pub mod structured;

use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    EnvFilter,
};

/// Logging configuration
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Log level
    pub level: Level,

    /// Log to file
    pub file_path: Option<PathBuf>,

    /// Log to stderr
    pub stderr: bool,

    /// Include timestamps
    pub timestamps: bool,

    /// Include thread IDs
    pub thread_ids: bool,

    /// Include spans
    pub spans: FmtSpan,

    /// Pretty print JSON
    pub pretty_json: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: Level::INFO,
            file_path: None,
            stderr: true,
            timestamps: true,
            thread_ids: false,
            spans: FmtSpan::NONE,
            pretty_json: false,
        }
    }
}

impl LogConfig {
    /// Create config from environment
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Set log level from RUST_LOG
        if let Ok(rust_log) = std::env::var("RUST_LOG") {
            // Parse the env filter to extract the level
            if rust_log.contains("trace") {
                config.level = Level::TRACE;
            } else if rust_log.contains("debug") {
                config.level = Level::DEBUG;
            } else if rust_log.contains("info") {
                config.level = Level::INFO;
            } else if rust_log.contains("warn") {
                config.level = Level::WARN;
            } else if rust_log.contains("error") {
                config.level = Level::ERROR;
            }
        }

        // Set file path from LOXONE_LOG_FILE
        if let Ok(log_file) = std::env::var("LOXONE_LOG_FILE") {
            config.file_path = Some(PathBuf::from(log_file));
        }

        // Set stderr logging
        if let Ok(log_stderr) = std::env::var("LOXONE_LOG_STDERR") {
            config.stderr = log_stderr.to_lowercase() != "false";
        }

        config
    }
}

/// Initialize logging with the given configuration
pub fn init_logging(config: LogConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Create env filter
    let env_filter = EnvFilter::builder()
        .with_default_directive(config.level.into())
        .from_env_lossy();

    // Create formatter
    let format = fmt::format()
        .with_level(true)
        .with_target(true)
        .with_thread_ids(config.thread_ids);

    // Store spans to avoid move issues
    let span_events = config.spans;

    // Build subscriber based on configuration
    match (config.stderr, config.file_path) {
        (true, Some(file_path)) => {
            // Both stderr and file logging
            // Ensure parent directory exists
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Create file appender with rotation
            let file_appender = tracing_appender::rolling::daily(
                file_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(".")),
                file_path
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("loxone-mcp.log")),
            );

            let stderr_layer = fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .event_format(format.clone())
                .with_span_events(span_events.clone());

            let file_layer = fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .event_format(format)
                .with_span_events(span_events);

            let subscriber = tracing_subscriber::registry()
                .with(env_filter)
                .with(stderr_layer)
                .with(file_layer);

            tracing::subscriber::set_global_default(subscriber)?;
        }
        (true, None) => {
            // Only stderr logging
            let stderr_layer = fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .event_format(format)
                .with_span_events(span_events);

            let subscriber = tracing_subscriber::registry()
                .with(env_filter)
                .with(stderr_layer);

            tracing::subscriber::set_global_default(subscriber)?;
        }
        (false, Some(file_path)) => {
            // Only file logging
            // Ensure parent directory exists
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Create file appender with rotation
            let file_appender = tracing_appender::rolling::daily(
                file_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(".")),
                file_path
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("loxone-mcp.log")),
            );

            let file_layer = fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .event_format(format)
                .with_span_events(span_events);

            let subscriber = tracing_subscriber::registry()
                .with(env_filter)
                .with(file_layer);

            tracing::subscriber::set_global_default(subscriber)?;
        }
        (false, None) => {
            // No output logging configured, just filter
            let subscriber = tracing_subscriber::registry().with(env_filter);

            tracing::subscriber::set_global_default(subscriber)?;
        }
    }

    Ok(())
}

/// Logging middleware for MCP tools
pub struct LoggingMiddleware;

impl LoggingMiddleware {
    /// Log tool invocation
    pub fn log_tool_call(tool_name: &str, params: &serde_json::Value) {
        // Sanitize parameters to avoid logging sensitive data
        let sanitized_params = Self::sanitize_params(params);

        tracing::info!(
            tool = tool_name,
            params = ?sanitized_params,
            "MCP tool called"
        );
    }

    /// Log tool response
    pub fn log_tool_response(
        tool_name: &str,
        duration_ms: u64,
        success: bool,
        response: &serde_json::Value,
    ) {
        // Sanitize response
        let sanitized_response = Self::sanitize_response(response);

        if success {
            tracing::info!(
                tool = tool_name,
                duration_ms = duration_ms,
                response = ?sanitized_response,
                "MCP tool completed successfully"
            );
        } else {
            tracing::error!(
                tool = tool_name,
                duration_ms = duration_ms,
                response = ?sanitized_response,
                "MCP tool failed"
            );
        }
    }

    /// Sanitize parameters to remove sensitive data
    fn sanitize_params(params: &serde_json::Value) -> serde_json::Value {
        match params {
            serde_json::Value::Object(map) => {
                let mut sanitized = serde_json::Map::new();
                for (key, value) in map {
                    if Self::is_sensitive_field(key) {
                        sanitized.insert(key.clone(), serde_json::Value::String("***".to_string()));
                    } else {
                        sanitized.insert(key.clone(), Self::sanitize_params(value));
                    }
                }
                serde_json::Value::Object(sanitized)
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(Self::sanitize_params).collect())
            }
            _ => params.clone(),
        }
    }

    /// Sanitize response data
    fn sanitize_response(response: &serde_json::Value) -> serde_json::Value {
        match response {
            serde_json::Value::Object(map) => {
                let mut sanitized = serde_json::Map::new();
                for (key, value) in map {
                    if Self::is_sensitive_field(key) {
                        sanitized.insert(key.clone(), serde_json::Value::String("***".to_string()));
                    } else if key == "data" && value.is_object() {
                        // Limit data size in logs
                        let data_str = value.to_string();
                        if data_str.len() > 1000 {
                            sanitized.insert(
                                key.clone(),
                                serde_json::Value::String(format!(
                                    "{}... (truncated, {} bytes total)",
                                    &data_str[..1000],
                                    data_str.len()
                                )),
                            );
                        } else {
                            sanitized.insert(key.clone(), value.clone());
                        }
                    } else {
                        sanitized.insert(key.clone(), Self::sanitize_response(value));
                    }
                }
                serde_json::Value::Object(sanitized)
            }
            serde_json::Value::Array(arr) => {
                if arr.len() > 10 {
                    // Limit array size in logs
                    let mut truncated: Vec<_> =
                        arr.iter().take(10).map(Self::sanitize_response).collect();
                    truncated.push(serde_json::Value::String(format!(
                        "... ({} more items)",
                        arr.len() - 10
                    )));
                    serde_json::Value::Array(truncated)
                } else {
                    serde_json::Value::Array(arr.iter().map(Self::sanitize_response).collect())
                }
            }
            _ => response.clone(),
        }
    }

    /// Check if a field name indicates sensitive data
    fn is_sensitive_field(field: &str) -> bool {
        let field_lower = field.to_lowercase();
        field_lower.contains("password")
            || field_lower.contains("secret")
            || field_lower.contains("token")
            || field_lower.contains("api_key")
            || field_lower.contains("apikey")
            || field_lower.contains("auth")
            || field_lower.contains("credential")
    }
}

/// Performance logging utilities
pub struct PerfLogger;

impl PerfLogger {
    /// Create a new performance span
    pub fn span(operation: &str) -> tracing::Span {
        tracing::info_span!("perf", operation = operation)
    }

    /// Log slow operations
    pub fn log_if_slow(operation: &str, duration_ms: u64, threshold_ms: u64) {
        if duration_ms > threshold_ms {
            tracing::warn!(
                operation = operation,
                duration_ms = duration_ms,
                threshold_ms = threshold_ms,
                "Slow operation detected"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_sensitive_fields() {
        let params = serde_json::json!({
            "username": "test_user",
            "password": "secret123",
            "api_key": "key123",
            "device": "Living Room Light"
        });

        let sanitized = LoggingMiddleware::sanitize_params(&params);

        assert_eq!(sanitized["username"], "test_user");
        assert_eq!(sanitized["password"], "***");
        assert_eq!(sanitized["api_key"], "***");
        assert_eq!(sanitized["device"], "Living Room Light");
    }
}
