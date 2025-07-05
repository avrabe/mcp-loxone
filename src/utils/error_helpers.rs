//! Error handling helper functions
//!
//! Provides safe alternatives to unwrap() for common patterns

use crate::error::{LoxoneError, Result};
use axum::http::header;
use std::fmt::Display;
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};
use tracing::warn;

/// Safely acquire a mutex lock, recovering from poisoned state if necessary
pub fn safe_mutex_lock<'a, T>(mutex: &'a Mutex<T>, context: &str) -> Result<MutexGuard<'a, T>> {
    match mutex.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            warn!(
                "Mutex poisoned in {}, recovering with potentially inconsistent state",
                context
            );
            // Extract the guard from the poisoned error
            // This is safe because we're acknowledging the inconsistent state
            Ok(poisoned.into_inner())
        }
    }
}

/// Parse a string with context information for better error messages
pub fn parse_with_context<T, S>(value: S, context: &str) -> Result<T>
where
    T: FromStr,
    T::Err: Display,
    S: AsRef<str>,
{
    value.as_ref().parse().map_err(|e: T::Err| {
        LoxoneError::invalid_input(format!(
            "Failed to parse {} - {}: {}",
            context,
            value.as_ref(),
            e
        ))
    })
}

/// Parse a URL with validation and context
pub fn parse_url_safe(url: &str, context: &str) -> Result<url::Url> {
    // Basic validation before parsing
    if url.is_empty() {
        return Err(LoxoneError::invalid_input(format!(
            "Empty URL provided for {context}"
        )));
    }

    if !url.starts_with("http://")
        && !url.starts_with("https://")
        && !url.starts_with("ws://")
        && !url.starts_with("wss://")
    {
        return Err(LoxoneError::invalid_input(format!(
            "Invalid URL scheme for {context}: {url}"
        )));
    }

    url::Url::parse(url).map_err(|e| {
        LoxoneError::invalid_input(format!("Failed to parse URL for {context} - {url}: {e}"))
    })
}

/// Parse a socket address with validation
pub fn parse_socket_addr_safe(addr: &str, context: &str) -> Result<std::net::SocketAddr> {
    use std::net::SocketAddr;

    if addr.is_empty() {
        return Err(LoxoneError::invalid_input(format!(
            "Empty socket address provided for {context}"
        )));
    }

    addr.parse::<SocketAddr>().map_err(|e| {
        LoxoneError::invalid_input(format!(
            "Failed to parse socket address for {context} - {addr}: {e}"
        ))
    })
}

/// Extract a value from JSON with type checking
pub fn extract_json_value<T, F>(value: &serde_json::Value, field: &str, extractor: F) -> Result<T>
where
    F: FnOnce(&serde_json::Value) -> Option<T>,
{
    value
        .get(field)
        .ok_or_else(|| LoxoneError::invalid_input(format!("Missing field: {field}")))
        .and_then(|v| {
            extractor(v).ok_or_else(|| {
                LoxoneError::invalid_input(format!(
                    "Invalid type for field '{}': expected {}, got {}",
                    field,
                    std::any::type_name::<T>(),
                    match v {
                        serde_json::Value::Null => "null",
                        serde_json::Value::Bool(_) => "boolean",
                        serde_json::Value::Number(_) => "number",
                        serde_json::Value::String(_) => "string",
                        serde_json::Value::Array(_) => "array",
                        serde_json::Value::Object(_) => "object",
                    }
                ))
            })
        })
}

/// Convert Option to Result with context
pub fn require_some<T>(option: Option<T>, context: &str) -> Result<T> {
    option.ok_or_else(|| LoxoneError::config(format!("Missing required value: {context}")))
}

/// Safely create HTTP headers with error handling
pub fn safe_header_pair(
    name: &str,
    value: &str,
) -> Result<(header::HeaderName, header::HeaderValue)> {
    let header_name = header::HeaderName::from_bytes(name.as_bytes())
        .map_err(|e| LoxoneError::invalid_input(format!("Invalid header name '{name}': {e}")))?;

    let header_value = header::HeaderValue::from_str(value).map_err(|e| {
        LoxoneError::invalid_input(format!("Invalid header value for '{name}': {e}"))
    })?;

    Ok((header_name, header_value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_mutex_lock() {
        let mutex = Mutex::new(42);

        // Normal lock should work
        {
            let guard = safe_mutex_lock(&mutex, "test").unwrap();
            assert_eq!(*guard, 42);
        }

        // Simulate poisoned mutex
        let mutex = Mutex::new(100);
        let _ = std::panic::catch_unwind(|| {
            let _guard = mutex.lock().unwrap();
            panic!("Simulated panic");
        });

        // Should still be able to lock
        let guard = safe_mutex_lock(&mutex, "poisoned test").unwrap();
        assert_eq!(*guard, 100);
    }

    #[test]
    fn test_parse_with_context() {
        // Valid parse
        let result: Result<i32> = parse_with_context("42", "test number");
        assert_eq!(result.unwrap(), 42);

        // Invalid parse
        let result: Result<i32> = parse_with_context("not a number", "test number");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("test number"));
    }

    #[test]
    fn test_parse_url_safe() {
        // Valid URLs
        assert!(parse_url_safe("http://example.com", "test").is_ok());
        assert!(parse_url_safe("https://example.com:8080/path", "test").is_ok());
        assert!(parse_url_safe("ws://localhost:9000", "test").is_ok());

        // Invalid URLs
        assert!(parse_url_safe("", "test").is_err());
        assert!(parse_url_safe("not a url", "test").is_err());
        assert!(parse_url_safe("ftp://example.com", "test").is_err());
    }

    #[test]
    fn test_extract_json_value() {
        let json = serde_json::json!({
            "name": "test",
            "value": 42,
            "enabled": true
        });

        // Valid extractions
        let name: Result<String> =
            extract_json_value(&json, "name", |v| v.as_str().map(|s| s.to_string()));
        assert_eq!(name.unwrap(), "test");

        let value: Result<i64> = extract_json_value(&json, "value", |v| v.as_i64());
        assert_eq!(value.unwrap(), 42);

        // Missing field
        let missing: Result<String> =
            extract_json_value(&json, "missing", |v| v.as_str().map(|s| s.to_string()));
        assert!(missing.is_err());

        // Wrong type
        let wrong_type: Result<String> =
            extract_json_value(&json, "value", |v| v.as_str().map(|s| s.to_string()));
        assert!(wrong_type.is_err());
    }
}
