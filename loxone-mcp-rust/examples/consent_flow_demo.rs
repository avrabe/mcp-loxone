//! MCP Consent Flow Demo
//! 
//! This example demonstrates the consent management system for sensitive operations
//! in the Loxone MCP server, showing how to configure consent policies, handle
//! consent requests, and integrate with device control operations.

use loxone_mcp_rust::config::{AuthMethod, LoxoneConfig};
use loxone_mcp_rust::config::credentials::LoxoneCredentials;
use loxone_mcp_rust::client::token_http_client::TokenHttpClient;
use loxone_mcp_rust::mcp_consent::{
    ConsentManager, ConsentConfig, ConsentRequest, ConsentResponse, OperationType, SensitivityLevel
};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🛡️  MCP Consent Flow Demo");
    println!("=======================\n");

    // Demo 1: Consent Configuration Options
    println!("1️⃣  Consent Configuration Options");
    
    // Default configuration
    let default_config = ConsentConfig::default();
    println!("   🎯 Default Configuration:");
    println!("      Enabled: {}", default_config.enabled);
    println!("      Default timeout: {:?}", default_config.default_timeout);
    println!("      Required for sensitivity levels: {:?}", default_config.required_for_sensitivity);
    println!("      Bulk consent required: {}", default_config.require_bulk_consent);
    println!("      Bulk threshold: {} devices", default_config.bulk_threshold);
    println!("      Audit all decisions: {}", default_config.audit_all_decisions);

    // Strict security configuration
    let mut required_levels = HashSet::new();
    required_levels.insert(SensitivityLevel::Medium);
    required_levels.insert(SensitivityLevel::High);
    required_levels.insert(SensitivityLevel::Critical);

    let strict_config = ConsentConfig {
        enabled: true,
        default_timeout: Duration::from_secs(60), // 1 minute timeout
        required_for_sensitivity: required_levels,
        auto_approve_operations: HashSet::new(),
        auto_deny_operations: HashSet::new(),
        max_pending_requests: 5,
        consent_cache_duration: Duration::from_secs(1800), // 30 minutes
        require_bulk_consent: true,
        bulk_threshold: 2, // Lower threshold
        audit_all_decisions: true,
    };

    println!("\n   🔒 Strict Security Configuration:");
    println!("      Timeout: {:?}", strict_config.default_timeout);
    println!("      Bulk threshold: {} devices", strict_config.bulk_threshold);
    println!("      Cache duration: {:?}", strict_config.consent_cache_duration);

    // Permissive configuration
    let mut permissive_levels = HashSet::new();
    permissive_levels.insert(SensitivityLevel::Critical);

    let permissive_config = ConsentConfig {
        enabled: true,
        default_timeout: Duration::from_secs(600), // 10 minutes
        required_for_sensitivity: permissive_levels,
        auto_approve_operations: HashSet::new(),
        auto_deny_operations: HashSet::new(),
        max_pending_requests: 20,
        consent_cache_duration: Duration::from_secs(7200), // 2 hours
        require_bulk_consent: false,
        bulk_threshold: 10,
        audit_all_decisions: false,
    };

    println!("\n   🔓 Permissive Configuration:");
    println!("      Only requires consent for: {:?}", permissive_config.required_for_sensitivity);
    println!("      Bulk consent: {}", permissive_config.require_bulk_consent);
    println!("      Audit decisions: {}", permissive_config.audit_all_decisions);

    // Demo 2: Operation Types and Sensitivity Levels
    println!("\n2️⃣  Operation Types and Sensitivity Levels");

    let operations = vec![
        (
            OperationType::DeviceControl {
                device_uuid: "uuid1".to_string(),
                device_name: "Living Room Light".to_string(),
                command: "on".to_string(),
            },
            "Individual device control"
        ),
        (
            OperationType::BulkDeviceControl {
                device_count: 8,
                room_name: Some("Living Room".to_string()),
                operation_type: "lights_off".to_string(),
            },
            "Bulk device control"
        ),
        (
            OperationType::SecurityControl {
                action: "arm_system".to_string(),
                scope: "full_house".to_string(),
            },
            "Security system control"
        ),
        (
            OperationType::SystemConfiguration {
                setting: "master_password".to_string(),
                old_value: Some("hidden".to_string()),
                new_value: "hidden".to_string(),
            },
            "System configuration change"
        ),
        (
            OperationType::DataExport {
                data_type: "user_data".to_string(),
                scope: "all_rooms".to_string(),
            },
            "Data export operation"
        ),
        (
            OperationType::ConnectionManagement {
                action: "disconnect".to_string(),
                target: "miniserver".to_string(),
            },
            "Connection management"
        ),
    ];

    for (operation, description) in operations {
        let consent_manager = ConsentManager::with_config(default_config.clone());
        let sensitivity = match &operation {
            OperationType::DeviceControl { command, .. } => {
                if command.contains("security") || command.contains("alarm") || command.contains("lock") {
                    SensitivityLevel::High
                } else {
                    SensitivityLevel::Medium
                }
            }
            OperationType::BulkDeviceControl { device_count, .. } => {
                if *device_count >= default_config.bulk_threshold {
                    SensitivityLevel::High
                } else {
                    SensitivityLevel::Medium
                }
            }
            OperationType::SecurityControl { .. } => SensitivityLevel::Critical,
            OperationType::SystemConfiguration { .. } => SensitivityLevel::High,
            OperationType::DataExport { .. } => SensitivityLevel::Medium,
            OperationType::ConnectionManagement { .. } => SensitivityLevel::Low,
        };

        println!("   📋 {}: {:?}", description, sensitivity);
    }

    // Demo 3: Consent Manager Creation and Setup
    println!("\n3️⃣  Consent Manager Setup");

    let mut consent_manager = ConsentManager::with_config(default_config);
    
    // Setup channels for consent communication
    let (request_rx, response_tx) = consent_manager.setup_channels().await;
    
    println!("   ✅ Consent manager created with default configuration");
    println!("   ✅ Communication channels established");
    println!("   📡 Ready to handle consent requests and responses");

    // Demo 4: Mock Consent Request Simulation
    println!("\n4️⃣  Mock Consent Request Simulation");

    // Simulate a high-sensitivity device control operation
    let mock_operation = OperationType::DeviceControl {
        device_uuid: "security-lock-uuid".to_string(),
        device_name: "Front Door Lock".to_string(),
        command: "unlock".to_string(),
    };

    println!("   🔐 Simulating security-sensitive operation:");
    println!("      Device: Front Door Lock");
    println!("      Command: unlock");
    println!("      Expected sensitivity: High");

    // This would normally request consent from user
    println!("   📋 Consent request would be generated with:");
    println!("      • Human-readable description");
    println!("      • Detailed explanation of action");
    println!("      • Potential risks and consequences");
    println!("      • Expected impact assessment");

    // Demo 5: Integration with HTTP Client
    println!("\n5️⃣  HTTP Client Integration");

    let config = LoxoneConfig {
        url: Url::parse("http://192.168.1.100")?,
        username: "demo_user".to_string(),
        verify_ssl: false,
        timeout: Duration::from_secs(30),
        max_retries: 3,
        max_connections: Some(10),
        #[cfg(feature = "websocket")]
        websocket: Default::default(),
        auth_method: AuthMethod::Token,
    };

    let credentials = LoxoneCredentials {
        username: "demo_user".to_string(),
        password: "demo_password".to_string(),
        api_key: None,
        #[cfg(feature = "crypto")]
        public_key: None,
    };

    match TokenHttpClient::new(config, credentials).await {
        Ok(mut client) => {
            println!("   ✅ HTTP client created successfully");
            
            // Enable consent management
            let consent_manager = Arc::new(ConsentManager::new());
            client.enable_consent_management(consent_manager);
            
            println!("   ✅ Consent management enabled for HTTP client");
            println!("   🛡️  All device commands will now require appropriate consent");
            
            if client.has_consent_management() {
                println!("   ✅ Consent management is active");
            }
        }
        Err(e) => println!("   ❌ Error creating client: {}", e),
    }

    // Demo 6: Consent Flow Scenarios
    println!("\n6️⃣  Consent Flow Scenarios");

    println!("   📖 Scenario 1: Auto-approved operation");
    println!("      • Low sensitivity operation");
    println!("      • Operation type not in consent requirements");
    println!      • Result: Immediate approval");

    println!("\n   📖 Scenario 2: User consent required");
    println!("      • High sensitivity operation");
    println!("      • No cached consent available");
    println!("      • User prompted for approval");
    println!("      • Result: Pending user response");

    println!("\n   📖 Scenario 3: Cached consent");
    println!("      • Similar operation performed recently");
    println!("      • Valid cached consent exists");
    println!("      • Result: Immediate approval from cache");

    println!("\n   📖 Scenario 4: Auto-denied operation");
    println!("      • Operation in auto-deny list");
    println!("      • Security policy violation");
    println!("      • Result: Immediate denial");

    println!("\n   📖 Scenario 5: Timeout");
    println!("      • User doesn't respond within timeout");
    println!("      • Operation cannot proceed");
    println!("      • Result: Timeout error");

    // Demo 7: Statistics and Audit Trail
    println!("\n7️⃣  Statistics and Audit Trail");

    // Mock some statistics
    println!("   📊 Example Consent Statistics:");
    println!("      Total requests: 156");
    println!("      Pending requests: 2");
    println!("      Approved: 142 (91.0%)");
    println!("      Denied: 8 (5.1%)");
    println!("      Auto-approved: 134 (85.9%)");
    println!("      Timed out: 6 (3.8%)");
    println!("      Cache size: 45 entries");

    println!("\n   📋 Audit Trail Benefits:");
    println!("      • Complete record of all consent decisions");
    println!("      • Timestamp and user information");
    println!("      • Operation details and outcomes");
    println!("      • Compliance and security monitoring");
    println!("      • Performance analysis and optimization");

    // Demo 8: Best Practices
    println!("\n8️⃣  Best Practices");

    println!("   ✅ Configuration:");
    println!("      • Set appropriate sensitivity thresholds");
    println!("      • Configure reasonable timeouts");
    println!("      • Enable audit logging for compliance");
    println!("      • Use auto-approve lists for routine operations");
    println!("      • Set bulk operation thresholds carefully");

    println!("\n   ✅ Implementation:");
    println!("      • Integrate consent checks at the right granularity");
    println!("      • Provide clear, understandable consent messages");
    println!("      • Handle consent failures gracefully");
    println!("      • Cache consent decisions when appropriate");
    println!("      • Monitor consent patterns and adjust policies");

    println!("\n   ✅ Security:");
    println!("      • Require consent for sensitive operations");
    println!("      • Use higher sensitivity for security-related commands");
    println!("      • Implement proper timeout handling");
    println!("      • Maintain detailed audit trails");
    println!("      • Regular review of consent policies");

    println!("\n✨ MCP Consent Flow Summary:");
    println!("   • Configurable consent policies for operations");
    println!("   • Automatic sensitivity classification");
    println!("   • User consent request/response workflow");
    println!("   • Consent caching for similar operations");
    println!("   • Comprehensive audit trail and statistics");
    println!("   • Integration with HTTP and WebSocket clients");
    println!("   • Bulk operation consent handling");
    println!("   • Auto-approve/deny policy support");

    println!("\n🔧 Integration Examples:");
    println!("   // Enable consent management");
    println!("   let consent_manager = Arc::new(ConsentManager::new());");
    println!("   client.enable_consent_management(consent_manager);");
    println!("   ");
    println!("   // Send command with automatic consent checking");
    println!("   client.send_command(\"device-uuid\", \"sensitive-command\").await?;");
    println!("   ");
    println!("   // Bulk operations also check consent");
    println!("   client.control_devices_parallel(commands).await?;");

    Ok(())
}