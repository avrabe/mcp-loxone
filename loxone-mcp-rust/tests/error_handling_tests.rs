//! Tests for enhanced structured error handling

use loxone_mcp_rust::error::{ErrorCode, ErrorContext, ErrorReporter, ErrorSeverity, LoxoneError};
use loxone_mcp_rust::log_structured_error;

#[tokio::test]
async fn test_structured_error_creation() {
    let error = LoxoneError::connection("Failed to connect to Miniserver");
    let structured = error.to_structured_error(None);

    assert_eq!(structured.code, ErrorCode::ConnectionLost);
    assert_eq!(structured.code_number, 1003);
    assert_eq!(structured.category, "connection");
    assert!(structured.is_retryable);
    assert!(!structured.is_auth_error);
    assert_eq!(structured.severity, ErrorSeverity::Warning);
}

#[tokio::test]
async fn test_error_context_with_metadata() {
    let context = ErrorContext::new(ErrorCode::DeviceNotFound, "device_service", "find_device")
        .with_metadata("device_id", "12345")
        .with_metadata("room", "living_room")
        .with_correlation_id("req_abc123");

    assert_eq!(context.component, "device_service");
    assert_eq!(context.operation, "find_device");
    assert_eq!(context.correlation_id, Some("req_abc123".to_string()));
    assert_eq!(context.metadata.get("device_id").unwrap(), "12345");
}

#[tokio::test]
async fn test_recovery_suggestions() {
    let error = LoxoneError::authentication("Invalid credentials");
    let suggestions = error.generate_recovery_suggestions();

    assert!(!suggestions.is_empty());
    assert!(suggestions
        .iter()
        .any(|s| s.description.contains("credentials")));
    assert!(suggestions.iter().any(|s| s.action_code.is_some()));
}

#[tokio::test]
async fn test_error_reporting() {
    let error = LoxoneError::device_control("Failed to turn on light");
    let context = ErrorContext::new(
        ErrorCode::DeviceControlFailed,
        "automation",
        "execute_command",
    )
    .with_correlation_id("test_123");

    // This will log the error - in a real test we'd capture the log output
    ErrorReporter::log_error(&error, Some(context));

    // Test API error formatting
    let api_response = ErrorReporter::format_api_error(&error, true);
    let error_obj = api_response["error"].as_object().unwrap();

    assert_eq!(error_obj["code"].as_u64().unwrap(), 1303);
    assert_eq!(error_obj["category"].as_str().unwrap(), "device");
    assert!(!error_obj["retryable"].as_bool().unwrap()); // device_control errors are not retryable
}

#[tokio::test]
async fn test_error_metrics() {
    let error = LoxoneError::rate_limit_error("Too many requests");
    let metrics = ErrorReporter::generate_metrics(&error);

    assert_eq!(metrics["error_code"].as_u64().unwrap(), 1502);
    assert_eq!(metrics["category"].as_str().unwrap(), "resource");
    assert_eq!(metrics["severity"].as_str().unwrap(), "Error");
    assert!(!metrics["retryable"].as_bool().unwrap_or(true)); // rate_limit errors are not retryable
}

#[tokio::test]
async fn test_structured_error_macro() {
    let error = LoxoneError::config("Missing configuration file");

    // Test the macro (this would normally log)
    log_structured_error!(error, "config_manager", "load_config", "test_correlation");

    // If we get here without panicking, the macro works
    // No assertion needed - getting here means success
}

#[test]
fn test_error_code_mapping() {
    // Test that all error types map to appropriate codes
    let test_cases = vec![
        (LoxoneError::connection("test"), ErrorCode::ConnectionLost),
        (
            LoxoneError::authentication("test"),
            ErrorCode::InvalidCredentials,
        ),
        (LoxoneError::config("test"), ErrorCode::ConfigurationInvalid),
        (
            LoxoneError::device_control("test"),
            ErrorCode::DeviceControlFailed,
        ),
        (LoxoneError::timeout("test"), ErrorCode::ConnectionTimeout),
        (LoxoneError::invalid_input("test"), ErrorCode::InvalidInput),
        (LoxoneError::not_found("test"), ErrorCode::DeviceNotFound),
        (
            LoxoneError::rate_limit_error("test"),
            ErrorCode::RateLimitExceeded,
        ),
    ];

    for (error, expected_code) in test_cases {
        assert_eq!(error.to_error_code(), expected_code);
    }
}

#[test]
fn test_error_severity_classification() {
    // Test severity levels
    assert_eq!(
        LoxoneError::authentication("test").severity(),
        ErrorSeverity::Critical
    );
    assert_eq!(LoxoneError::config("test").severity(), ErrorSeverity::Error);
    assert_eq!(
        LoxoneError::connection("test").severity(),
        ErrorSeverity::Warning
    );
    assert_eq!(
        LoxoneError::timeout("test").severity(),
        ErrorSeverity::Warning
    );
}

#[test]
fn test_error_code_numbers() {
    // Test that error codes have the right numeric values
    assert_eq!(ErrorCode::ConnectionTimeout.as_number(), 1001);
    assert_eq!(ErrorCode::InvalidCredentials.as_number(), 1101);
    assert_eq!(ErrorCode::ConfigurationInvalid.as_number(), 1202);
    assert_eq!(ErrorCode::DeviceNotFound.as_number(), 1301);
    assert_eq!(ErrorCode::ParsingFailed.as_number(), 1401);
    assert_eq!(ErrorCode::ResourceExhausted.as_number(), 1501);
}

#[test]
fn test_error_categories() {
    assert_eq!(ErrorCode::ConnectionTimeout.category(), "connection");
    assert_eq!(ErrorCode::InvalidCredentials.category(), "authentication");
    assert_eq!(ErrorCode::ConfigurationInvalid.category(), "configuration");
    assert_eq!(ErrorCode::DeviceNotFound.category(), "device");
    assert_eq!(ErrorCode::ParsingFailed.category(), "data");
    assert_eq!(ErrorCode::ResourceExhausted.category(), "resource");
}
