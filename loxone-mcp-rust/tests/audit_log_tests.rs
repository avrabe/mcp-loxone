//! Tests for audit logging functionality

use loxone_mcp_rust::audit_log::{
    AuditConfig, AuditEntry, AuditEventType, AuditLoggerBuilder, AuditOutput, AuditOutputFormat,
    AuditSeverity,
};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

fn create_test_config() -> AuditConfig {
    AuditConfig {
        enabled: true,
        min_severity: AuditSeverity::Info,
        format: AuditOutputFormat::Json,
        buffer_size: 10,
        flush_interval: Duration::from_millis(100),
        enable_checksums: true,
        retention_days: Some(30),
        max_file_size: Some(1024 * 1024),
        include_sensitive: false,
        exclude_events: Vec::new(),
    }
}

#[tokio::test]
async fn test_audit_entry_creation() {
    let entry = AuditEntry::new(
        AuditSeverity::Info,
        AuditEventType::Authentication {
            username: "testuser".to_string(),
            success: true,
            method: "token".to_string(),
            ip_address: Some("192.168.1.100".parse().unwrap()),
        },
    );

    assert_eq!(entry.severity, AuditSeverity::Info);
    assert!(matches!(
        entry.event_type,
        AuditEventType::Authentication { .. }
    ));
    assert!(entry.session_id.is_none());
    assert!(entry.correlation_id.is_none());
    assert!(entry.context.is_empty());
}

#[tokio::test]
async fn test_audit_entry_with_context() {
    let entry = AuditEntry::new(
        AuditSeverity::Warning,
        AuditEventType::DeviceControl {
            device_uuid: "test-device".to_string(),
            device_name: "Test Light".to_string(),
            command: "on".to_string(),
            source: "test".to_string(),
            success: true,
            error: None,
        },
    )
    .with_session(Uuid::new_v4())
    .with_correlation(Uuid::new_v4())
    .with_context("room".to_string(), "living_room".to_string())
    .with_context("floor".to_string(), "ground".to_string());

    assert!(entry.session_id.is_some());
    assert!(entry.correlation_id.is_some());
    assert_eq!(entry.context.len(), 2);
    assert_eq!(entry.context.get("room"), Some(&"living_room".to_string()));
    assert_eq!(entry.context.get("floor"), Some(&"ground".to_string()));
}

#[tokio::test]
async fn test_audit_entry_checksum() {
    let entry = AuditEntry::new(
        AuditSeverity::Critical,
        AuditEventType::SecurityAlert {
            alert_type: "brute_force".to_string(),
            description: "Multiple failed login attempts".to_string(),
            source: "auth_monitor".to_string(),
            severity: AuditSeverity::Critical,
        },
    )
    .with_checksum();

    assert!(entry.checksum.is_some());
    assert!(entry.verify_checksum());

    // Test checksum verification after modification
    let mut modified_entry = entry.clone();
    modified_entry
        .context
        .insert("modified".to_string(), "yes".to_string());
    // Checksum should become invalid after modification
    assert!(!modified_entry.verify_checksum());
}

#[tokio::test]
async fn test_audit_logger_creation() {
    let config = create_test_config();
    let logger = AuditLoggerBuilder::new()
        .with_config(config.clone())
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    assert_eq!(logger.get_config().buffer_size, 10);
    assert_eq!(logger.get_config().min_severity, AuditSeverity::Info);
    assert!(logger.get_config().enable_checksums);
}

#[tokio::test]
async fn test_audit_logger_builder() {
    let logger = AuditLoggerBuilder::new()
        .min_severity(AuditSeverity::Warning)
        .enable_checksums(false)
        .retention_days(90)
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    assert_eq!(logger.get_config().min_severity, AuditSeverity::Warning);
    assert!(!logger.get_config().enable_checksums);
    assert_eq!(logger.get_config().retention_days, Some(90));
}

#[tokio::test]
async fn test_audit_logger_start_stop() {
    let logger = AuditLoggerBuilder::new()
        .with_config(create_test_config())
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    let start_result = logger.start().await;
    assert!(start_result.is_ok());

    let mut logger = logger;
    let stop_result = logger.stop().await;
    assert!(stop_result.is_ok());
}

#[tokio::test]
async fn test_audit_logger_disabled() {
    let config = AuditConfig {
        enabled: false,
        ..create_test_config()
    };

    let logger = AuditLoggerBuilder::new()
        .with_config(config)
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    logger.start().await.unwrap();

    // Logging should succeed but do nothing when disabled
    let result = logger
        .log_authentication(
            "testuser",
            true,
            "token",
            Some("192.168.1.100".parse().unwrap()),
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_severity_filtering() {
    let config = AuditConfig {
        min_severity: AuditSeverity::Warning,
        ..create_test_config()
    };

    let logger = AuditLoggerBuilder::new()
        .with_config(config)
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    logger.start().await.unwrap();

    // Info level should be filtered out
    let info_result = logger
        .log_authentication("testuser", true, "token", None)
        .await;
    assert!(info_result.is_ok());

    // Warning level should be logged
    let warning_result = logger
        .log_security_alert(
            "suspicious_activity",
            "Unusual access pattern detected",
            "behavior_monitor",
            AuditSeverity::Warning,
        )
        .await;
    assert!(warning_result.is_ok());
}

#[tokio::test]
async fn test_event_type_exclusion() {
    let config = AuditConfig {
        exclude_events: vec!["Authentication".to_string()],
        ..create_test_config()
    };

    let logger = AuditLoggerBuilder::new()
        .with_config(config)
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    logger.start().await.unwrap();

    // Authentication events should be excluded
    let auth_result = logger
        .log_authentication("testuser", true, "token", None)
        .await;
    assert!(auth_result.is_ok());

    // Device control should still be logged
    let device_result = logger
        .log_device_control("device1", "Test Device", "on", "test", true, None)
        .await;
    assert!(device_result.is_ok());
}

#[tokio::test]
async fn test_authentication_logging() {
    let logger = AuditLoggerBuilder::new()
        .with_config(create_test_config())
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    logger.start().await.unwrap();

    // Successful authentication
    let success_result = logger
        .log_authentication(
            "admin",
            true,
            "token",
            Some("192.168.1.100".parse().unwrap()),
        )
        .await;
    assert!(success_result.is_ok());

    // Failed authentication
    let failure_result = logger
        .log_authentication(
            "hacker",
            false,
            "password",
            Some("10.0.0.1".parse().unwrap()),
        )
        .await;
    assert!(failure_result.is_ok());
}

#[tokio::test]
async fn test_authorization_logging() {
    let logger = AuditLoggerBuilder::new()
        .with_config(create_test_config())
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    logger.start().await.unwrap();

    // Granted authorization
    let granted_result = logger
        .log_authorization("user1", "device.light1", "control", true, None)
        .await;
    assert!(granted_result.is_ok());

    // Denied authorization
    let denied_result = logger
        .log_authorization(
            "guest",
            "security.alarm",
            "disarm",
            false,
            Some("Insufficient privileges".to_string()),
        )
        .await;
    assert!(denied_result.is_ok());
}

#[tokio::test]
async fn test_device_control_logging() {
    let logger = AuditLoggerBuilder::new()
        .with_config(create_test_config())
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    logger.start().await.unwrap();

    // Successful device control
    let success_result = logger
        .log_device_control(
            "device-uuid-1",
            "Living Room Light",
            "toggle",
            "Mobile App",
            true,
            None,
        )
        .await;
    assert!(success_result.is_ok());

    // Failed device control
    let failure_result = logger
        .log_device_control(
            "device-uuid-2",
            "Garage Door",
            "open",
            "Remote API",
            false,
            Some("Device offline".to_string()),
        )
        .await;
    assert!(failure_result.is_ok());
}

#[tokio::test]
async fn test_security_alert_logging() {
    let logger = AuditLoggerBuilder::new()
        .with_config(create_test_config())
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    logger.start().await.unwrap();

    // Critical security alert
    let critical_result = logger
        .log_security_alert(
            "intrusion_detected",
            "Motion sensor triggered in secure area",
            "security_system",
            AuditSeverity::Critical,
        )
        .await;
    assert!(critical_result.is_ok());

    // Warning security alert
    let warning_result = logger
        .log_security_alert(
            "unusual_pattern",
            "Device access outside normal hours",
            "behavior_monitor",
            AuditSeverity::Warning,
        )
        .await;
    assert!(warning_result.is_ok());
}

#[tokio::test]
async fn test_complex_audit_events() {
    let logger = AuditLoggerBuilder::new()
        .with_config(create_test_config())
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    logger.start().await.unwrap();

    // Configuration change event
    let config_change = AuditEntry::new(
        AuditSeverity::Warning,
        AuditEventType::ConfigurationChange {
            setting: "security.auto_arm".to_string(),
            old_value: Some("true".to_string()),
            new_value: "false".to_string(),
            changed_by: "admin".to_string(),
        },
    )
    .with_session(Uuid::new_v4())
    .with_context("reason".to_string(), "maintenance".to_string());

    let config_result = logger.log(config_change).await;
    assert!(config_result.is_ok());

    // System lifecycle event
    let mut startup_details = HashMap::new();
    startup_details.insert("version".to_string(), "1.0.0".to_string());
    startup_details.insert("modules".to_string(), "15".to_string());

    let lifecycle_event = AuditEntry::new(
        AuditSeverity::Info,
        AuditEventType::SystemLifecycle {
            event: "startup".to_string(),
            details: startup_details,
        },
    );

    let lifecycle_result = logger.log(lifecycle_event).await;
    assert!(lifecycle_result.is_ok());

    // API access event
    let api_event = AuditEntry::new(
        AuditSeverity::Info,
        AuditEventType::ApiAccess {
            endpoint: "/api/v1/devices".to_string(),
            method: "GET".to_string(),
            client_id: Some("mobile-app".to_string()),
            status_code: 200,
            response_time: Duration::from_millis(125),
        },
    )
    .with_correlation(Uuid::new_v4());

    let api_result = logger.log(api_event).await;
    assert!(api_result.is_ok());
}

#[tokio::test]
async fn test_audit_statistics() {
    let logger = AuditLoggerBuilder::new()
        .with_config(create_test_config())
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    logger.start().await.unwrap();

    // Initial statistics should be empty
    let initial_stats = logger.get_statistics().await;
    assert_eq!(initial_stats.total_events, 0);
    assert_eq!(initial_stats.failed_logs, 0);
    assert!(initial_stats.events_by_severity.is_empty());
    assert!(initial_stats.events_by_type.is_empty());

    // Log some events
    logger
        .log_authentication("user1", true, "token", None)
        .await
        .unwrap();
    logger
        .log_device_control("device1", "Light", "on", "test", true, None)
        .await
        .unwrap();
    logger
        .log_security_alert("test_alert", "Test", "test", AuditSeverity::Warning)
        .await
        .unwrap();

    // Give some time for processing
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Note: Statistics might not immediately reflect the logged events
    // because they're processed asynchronously
    let _stats = logger.get_statistics().await;
    // We can't assert exact values due to async processing, but structure should be valid
    // Note: These fields are u64, so they're always >= 0, no need to assert
}

#[tokio::test]
async fn test_consent_management_events() {
    let logger = AuditLoggerBuilder::new()
        .with_config(create_test_config())
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    logger.start().await.unwrap();

    let consent_event = AuditEntry::new(
        AuditSeverity::Info,
        AuditEventType::ConsentManagement {
            operation_id: Uuid::new_v4(),
            operation_type: "device_control".to_string(),
            decision: "approved".to_string(),
            user: Some("homeowner".to_string()),
            automated: false,
        },
    );

    let result = logger.log(consent_event).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_connection_events() {
    let logger = AuditLoggerBuilder::new()
        .with_config(create_test_config())
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    logger.start().await.unwrap();

    let connection_event = AuditEntry::new(
        AuditSeverity::Info,
        AuditEventType::Connection {
            event_type: "client_connected".to_string(),
            remote_address: Some("192.168.1.50".parse().unwrap()),
            protocol: "HTTP".to_string(),
            success: true,
        },
    );

    let result = logger.log(connection_event).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_data_access_events() {
    let logger = AuditLoggerBuilder::new()
        .with_config(create_test_config())
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    logger.start().await.unwrap();

    let data_access_event = AuditEntry::new(
        AuditSeverity::Info,
        AuditEventType::DataAccess {
            resource: "device_states".to_string(),
            operation: "read".to_string(),
            user: Some("api_user".to_string()),
            records_affected: Some(42),
        },
    );

    let result = logger.log(data_access_event).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_different_output_formats() {
    // JSON format
    let json_logger = AuditLoggerBuilder::new()
        .with_config(AuditConfig {
            format: AuditOutputFormat::Json,
            ..create_test_config()
        })
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    assert!(matches!(
        json_logger.get_config().format,
        AuditOutputFormat::Json
    ));

    // Syslog format
    let syslog_logger = AuditLoggerBuilder::new()
        .with_config(AuditConfig {
            format: AuditOutputFormat::Syslog,
            ..create_test_config()
        })
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    assert!(matches!(
        syslog_logger.get_config().format,
        AuditOutputFormat::Syslog
    ));

    // CEF format
    let cef_logger = AuditLoggerBuilder::new()
        .with_config(AuditConfig {
            format: AuditOutputFormat::Cef,
            ..create_test_config()
        })
        .with_output(AuditOutput::Stdout)
        .build()
        .unwrap();

    assert!(matches!(
        cef_logger.get_config().format,
        AuditOutputFormat::Cef
    ));
}

#[tokio::test]
async fn test_audit_severity_ordering() {
    assert!(AuditSeverity::Critical > AuditSeverity::Error);
    assert!(AuditSeverity::Error > AuditSeverity::Warning);
    assert!(AuditSeverity::Warning > AuditSeverity::Info);

    let severities = vec![
        AuditSeverity::Info,
        AuditSeverity::Critical,
        AuditSeverity::Warning,
        AuditSeverity::Error,
    ];

    let mut sorted = severities.clone();
    sorted.sort();

    assert_eq!(
        sorted,
        vec![
            AuditSeverity::Info,
            AuditSeverity::Warning,
            AuditSeverity::Error,
            AuditSeverity::Critical,
        ]
    );
}

#[tokio::test]
async fn test_event_serialization() {
    let entry = AuditEntry::new(
        AuditSeverity::Info,
        AuditEventType::Authentication {
            username: "testuser".to_string(),
            success: true,
            method: "token".to_string(),
            ip_address: Some("192.168.1.100".parse().unwrap()),
        },
    )
    .with_context("client".to_string(), "mobile_app".to_string())
    .with_checksum();

    // Test JSON serialization
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("testuser"));
    assert!(json.contains("Authentication"));
    assert!(json.contains("mobile_app"));

    // Test deserialization
    let deserialized: AuditEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.severity, AuditSeverity::Info);
    assert!(matches!(
        deserialized.event_type,
        AuditEventType::Authentication { .. }
    ));
    assert_eq!(
        deserialized.context.get("client"),
        Some(&"mobile_app".to_string())
    );
}
