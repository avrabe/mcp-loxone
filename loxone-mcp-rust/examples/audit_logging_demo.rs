//! Audit Logging Demo
//!
//! This example demonstrates the comprehensive audit logging system for
//! security compliance, showing how to track all security-relevant operations,
//! access attempts, and system changes.

use loxone_mcp_rust::audit_log::{
    AuditConfig, AuditEntry, AuditEventType, AuditLoggerBuilder, AuditOutput, AuditOutputFormat,
    AuditSeverity,
};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔒 Audit Logging Demo");
    println!("====================\n");

    // Demo 1: Audit Configuration Options
    println!("1️⃣  Audit Configuration Options");

    // Default configuration
    let default_config = AuditConfig::default();
    println!("   🎯 Default Configuration:");
    println!("      Enabled: {}", default_config.enabled);
    println!("      Min severity: {:?}", default_config.min_severity);
    println!("      Format: {:?}", default_config.format);
    println!("      Buffer size: {}", default_config.buffer_size);
    println!("      Flush interval: {:?}", default_config.flush_interval);
    println!(
        "      Enable checksums: {}",
        default_config.enable_checksums
    );
    println!("      Retention days: {:?}", default_config.retention_days);

    // High-security configuration
    let high_security_config = AuditConfig {
        enabled: true,
        min_severity: AuditSeverity::Info,
        format: AuditOutputFormat::Json,
        buffer_size: 100,                       // Smaller buffer for faster writes
        flush_interval: Duration::from_secs(1), // Flush every second
        enable_checksums: true,
        retention_days: Some(365),             // 1 year retention
        max_file_size: Some(50 * 1024 * 1024), // 50MB files
        include_sensitive: false,
        exclude_events: vec![],
    };

    println!("\n   🛡️  High-Security Configuration:");
    println!(
        "      Retention: {} days",
        high_security_config.retention_days.unwrap()
    );
    println!(
        "      Flush interval: {:?}",
        high_security_config.flush_interval
    );
    println!(
        "      Max file size: {} MB",
        high_security_config.max_file_size.unwrap() / 1024 / 1024
    );

    // Demo 2: Creating and Starting Audit Logger
    println!("\n2️⃣  Creating and Starting Audit Logger");

    let audit_logger = AuditLoggerBuilder::new()
        .with_config(high_security_config)
        .with_output(AuditOutput::Stdout)
        .build()?;

    audit_logger.start().await?;
    println!("   ✅ Audit logger started");

    // Demo 3: Authentication Events
    println!("\n3️⃣  Authentication Events");

    // Successful authentication
    audit_logger
        .log_authentication("admin", true, "token", Some("192.168.1.100".parse()?))
        .await?;
    println!("   ✅ Logged successful authentication");

    // Failed authentication
    audit_logger
        .log_authentication("hacker", false, "password", Some("10.0.0.1".parse()?))
        .await?;
    println!("   ❌ Logged failed authentication");

    // Demo 4: Authorization Events
    println!("\n4️⃣  Authorization Events");

    // Granted authorization
    audit_logger
        .log_authorization("user1", "light.living_room", "control", true, None)
        .await?;
    println!("   ✅ Logged granted authorization");

    // Denied authorization
    audit_logger
        .log_authorization(
            "guest",
            "security.alarm",
            "disarm",
            false,
            Some("Insufficient privileges".to_string()),
        )
        .await?;
    println!("   ❌ Logged denied authorization");

    // Demo 5: Device Control Events
    println!("\n5️⃣  Device Control Events");

    // Successful device control
    audit_logger
        .log_device_control(
            "uuid-light-1",
            "Living Room Light",
            "on",
            "MCP API",
            true,
            None,
        )
        .await?;
    println!("   ✅ Logged successful device control");

    // Failed device control
    audit_logger
        .log_device_control(
            "uuid-door-1",
            "Front Door Lock",
            "unlock",
            "Remote API",
            false,
            Some("Device offline".to_string()),
        )
        .await?;
    println!("   ❌ Logged failed device control");

    // Demo 6: Security Alerts
    println!("\n6️⃣  Security Alerts");

    // Multiple failed login attempts
    audit_logger
        .log_security_alert(
            "brute_force_attempt",
            "Multiple failed login attempts from IP 10.0.0.1",
            "authentication_monitor",
            AuditSeverity::Critical,
        )
        .await?;
    println!("   🚨 Logged critical security alert");

    // Unusual access pattern
    audit_logger
        .log_security_alert(
            "unusual_access",
            "Accessing sensitive devices at unusual time",
            "behavior_monitor",
            AuditSeverity::Warning,
        )
        .await?;
    println!("   ⚠️  Logged warning security alert");

    // Demo 7: Complex Audit Events
    println!("\n7️⃣  Complex Audit Events");

    // Configuration change
    let config_change = AuditEntry::new(
        AuditSeverity::Warning,
        AuditEventType::ConfigurationChange {
            setting: "security.alarm.enabled".to_string(),
            old_value: Some("true".to_string()),
            new_value: "false".to_string(),
            changed_by: "admin".to_string(),
        },
    )
    .with_session(Uuid::new_v4())
    .with_context("reason".to_string(), "maintenance".to_string())
    .with_checksum();

    audit_logger.log(config_change).await?;
    println!("   🔧 Logged configuration change with context");

    // API access event
    let api_access = AuditEntry::new(
        AuditSeverity::Info,
        AuditEventType::ApiAccess {
            endpoint: "/api/v1/devices/control".to_string(),
            method: "POST".to_string(),
            client_id: Some("mobile-app-123".to_string()),
            status_code: 200,
            response_time: Duration::from_millis(150),
        },
    )
    .with_correlation(Uuid::new_v4());

    audit_logger.log(api_access).await?;
    println!("   📡 Logged API access with correlation ID");

    // Demo 8: Consent Management Events
    println!("\n8️⃣  Consent Management Events");

    let consent_event = AuditEntry::new(
        AuditSeverity::Info,
        AuditEventType::ConsentManagement {
            operation_id: Uuid::new_v4(),
            operation_type: "bulk_device_control".to_string(),
            decision: "approved".to_string(),
            user: Some("homeowner".to_string()),
            automated: false,
        },
    );

    audit_logger.log(consent_event).await?;
    println!("   ✅ Logged consent management event");

    // Demo 9: System Lifecycle Events
    println!("\n9️⃣  System Lifecycle Events");

    let mut startup_details = HashMap::new();
    startup_details.insert("version".to_string(), "1.0.0".to_string());
    startup_details.insert("config_source".to_string(), "environment".to_string());
    startup_details.insert("modules_loaded".to_string(), "15".to_string());

    let lifecycle_event = AuditEntry::new(
        AuditSeverity::Info,
        AuditEventType::SystemLifecycle {
            event: "system_startup".to_string(),
            details: startup_details,
        },
    );

    audit_logger.log(lifecycle_event).await?;
    println!("   🚀 Logged system lifecycle event");

    // Demo 10: Audit Statistics
    println!("\n🔟 Audit Statistics");

    let stats = audit_logger.get_statistics().await;
    println!("   📊 Current Statistics:");
    println!("      Total events: {}", stats.total_events);
    println!("      Failed logs: {}", stats.failed_logs);
    println!("      Buffer usage: {}", stats.buffer_usage);

    if !stats.events_by_severity.is_empty() {
        println!("   📈 Events by Severity:");
        for (severity, count) in &stats.events_by_severity {
            println!("      {}: {}", severity, count);
        }
    }

    if !stats.events_by_type.is_empty() {
        println!("   📈 Events by Type:");
        for (event_type, count) in &stats.events_by_type {
            println!("      {}: {}", event_type, count);
        }
    }

    // Demo 11: Checksum Verification
    println!("\n1️⃣1️⃣ Checksum Verification");

    let test_entry = AuditEntry::new(
        AuditSeverity::Info,
        AuditEventType::DataAccess {
            resource: "device_states".to_string(),
            operation: "read".to_string(),
            user: Some("api_user".to_string()),
            records_affected: Some(25),
        },
    )
    .with_checksum();

    println!(
        "   🔐 Entry checksum: {}",
        test_entry.checksum.as_ref().unwrap()
    );
    println!("   ✅ Checksum valid: {}", test_entry.verify_checksum());

    // Demo 12: Output Formats
    println!("\n1️⃣2️⃣ Output Format Examples");

    println!("   📄 JSON Format:");
    let _json_logger = AuditLoggerBuilder::new()
        .with_config(AuditConfig {
            format: AuditOutputFormat::Json,
            ..Default::default()
        })
        .with_output(AuditOutput::Stdout)
        .build()?;

    println!("      (Events would be logged as JSON objects)");

    println!("\n   📝 Syslog Format:");
    let _syslog_logger = AuditLoggerBuilder::new()
        .with_config(AuditConfig {
            format: AuditOutputFormat::Syslog,
            ..Default::default()
        })
        .with_output(AuditOutput::Stdout)
        .build()?;

    println!("      (Events would be logged in syslog format)");

    println!("\n   🔧 CEF Format:");
    let _cef_logger = AuditLoggerBuilder::new()
        .with_config(AuditConfig {
            format: AuditOutputFormat::Cef,
            ..Default::default()
        })
        .with_output(AuditOutput::Stdout)
        .build()?;

    println!("      (Events would be logged in Common Event Format)");

    // Demo 13: Compliance Features
    println!("\n1️⃣3️⃣ Compliance Features");

    println!("   📋 Regulatory Compliance:");
    println!("      • GDPR: Configurable retention, data minimization");
    println!("      • SOC2: Comprehensive audit trail, integrity checks");
    println!("      • ISO 27001: Security event tracking, access logs");
    println!("      • PCI DSS: Authentication tracking, data access logs");

    println!("\n   🔒 Security Features:");
    println!("      • Tamper-resistant checksums");
    println!("      • Immutable audit trail");
    println!("      • Automatic log rotation");
    println!("      • Secure remote logging");

    // Summary
    println!("\n✨ Audit Logging Benefits Summary:");
    println!("   • Complete audit trail for all security-relevant operations");
    println!("   • Configurable severity levels and event filtering");
    println!("   • Multiple output formats for integration");
    println!("   • Tamper-resistant logging with checksums");
    println!("   • Performance-optimized async implementation");
    println!("   • Compliance-ready with retention policies");
    println!("   • Rich contextual information for investigations");

    println!("\n🔧 Integration Examples:");
    println!("   // Create audit logger with file output");
    println!("   let logger = AuditLoggerBuilder::new()");
    println!("       .with_output(AuditOutput::File(\"/var/log/loxone-audit.log\".into()))");
    println!("       .retention_days(90)");
    println!("       .enable_checksums(true)");
    println!("       .build()?;");
    println!("   ");
    println!("   // Log device control with context");
    println!("   logger.log_device_control(");
    println!("       device_uuid, device_name, command,");
    println!("       source, success, error");
    println!("   ).await?;");
    println!("   ");
    println!("   // Create custom event with full context");
    println!("   let event = AuditEntry::new(severity, event_type)");
    println!("       .with_session(session_id)");
    println!("       .with_context(\"key\".to_string(), \"value\".to_string())");
    println!("       .with_checksum();");
    println!("   logger.log(event).await?;");

    Ok(())
}
