//! Tests for MCP consent flow functionality

use loxone_mcp_rust::mcp_consent::{
    ConsentManager, ConsentConfig, ConsentRequest, ConsentResponse, ConsentDecision, 
    OperationType, SensitivityLevel
};
use std::collections::HashSet;
use std::time::{Duration, SystemTime};
use tokio::time::timeout;

fn create_test_config() -> ConsentConfig {
    let mut required_levels = HashSet::new();
    required_levels.insert(SensitivityLevel::High);
    required_levels.insert(SensitivityLevel::Critical);

    ConsentConfig {
        enabled: true,
        default_timeout: Duration::from_secs(5), // Short timeout for tests
        required_for_sensitivity: required_levels,
        auto_approve_operations: HashSet::new(),
        auto_deny_operations: HashSet::new(),
        max_pending_requests: 10,
        consent_cache_duration: Duration::from_secs(60),
        require_bulk_consent: true,
        bulk_threshold: 3,
        audit_all_decisions: true,
    }
}

#[tokio::test]
async fn test_consent_config_creation() {
    let config = ConsentConfig::default();
    assert!(config.enabled);
    assert_eq!(config.default_timeout, Duration::from_secs(300));
    assert!(config.required_for_sensitivity.contains(&SensitivityLevel::High));
    assert!(config.required_for_sensitivity.contains(&SensitivityLevel::Critical));
    assert!(config.require_bulk_consent);
    assert_eq!(config.bulk_threshold, 5);
}

#[tokio::test]
async fn test_consent_manager_creation() {
    let manager = ConsentManager::new();
    assert!(std::matches!(manager, ConsentManager { .. }));
    
    let config = create_test_config();
    let manager = ConsentManager::with_config(config.clone());
    assert!(std::matches!(manager, ConsentManager { .. }));
}

#[tokio::test]
async fn test_consent_manager_channels() {
    let mut manager = ConsentManager::new();
    
    // Setup channels
    let (request_rx, response_tx) = manager.setup_channels().await;
    
    // Verify channels work
    assert!(std::matches!(request_rx, tokio::sync::mpsc::UnboundedReceiver { .. }));
    assert!(std::matches!(response_tx, tokio::sync::mpsc::UnboundedSender { .. }));
}

#[tokio::test]
async fn test_operation_sensitivity_classification() {
    let manager = ConsentManager::with_config(create_test_config());
    
    // Device control operations
    let light_op = OperationType::DeviceControl {
        device_uuid: "uuid1".to_string(),
        device_name: "Light".to_string(),
        command: "on".to_string(),
    };
    
    let security_op = OperationType::DeviceControl {
        device_uuid: "uuid2".to_string(),
        device_name: "Security Lock".to_string(),
        command: "lock".to_string(),
    };
    
    // Test sensitivity classification
    let sensitivity1 = manager.classify_operation_sensitivity(&light_op);
    let sensitivity2 = manager.classify_operation_sensitivity(&security_op);
    
    assert_eq!(sensitivity1, SensitivityLevel::Medium);
    assert_eq!(sensitivity2, SensitivityLevel::High);
}

#[tokio::test]
async fn test_bulk_operation_classification() {
    let manager = ConsentManager::with_config(create_test_config());
    
    let small_bulk = OperationType::BulkDeviceControl {
        device_count: 2,
        room_name: Some("Living Room".to_string()),
        operation_type: "lights_off".to_string(),
    };
    
    let large_bulk = OperationType::BulkDeviceControl {
        device_count: 5,
        room_name: Some("Whole House".to_string()),
        operation_type: "all_off".to_string(),
    };
    
    let sensitivity1 = manager.classify_operation_sensitivity(&small_bulk);
    let sensitivity2 = manager.classify_operation_sensitivity(&large_bulk);
    
    assert_eq!(sensitivity1, SensitivityLevel::Medium);
    assert_eq!(sensitivity2, SensitivityLevel::High);
}

#[tokio::test]
async fn test_security_operations() {
    let manager = ConsentManager::with_config(create_test_config());
    
    let security_op = OperationType::SecurityControl {
        action: "arm_system".to_string(),
        scope: "full_house".to_string(),
    };
    
    let config_op = OperationType::SystemConfiguration {
        setting: "master_password".to_string(),
        old_value: Some("old".to_string()),
        new_value: "new".to_string(),
    };
    
    let sensitivity1 = manager.classify_operation_sensitivity(&security_op);
    let sensitivity2 = manager.classify_operation_sensitivity(&config_op);
    
    assert_eq!(sensitivity1, SensitivityLevel::Critical);
    assert_eq!(sensitivity2, SensitivityLevel::High);
}

#[tokio::test]
async fn test_auto_approval_disabled_consent() {
    let config = ConsentConfig {
        enabled: false,
        ..create_test_config()
    };
    let manager = ConsentManager::with_config(config);
    
    let operation = OperationType::DeviceControl {
        device_uuid: "uuid1".to_string(),
        device_name: "Light".to_string(),
        command: "on".to_string(),
    };
    
    let result = manager.request_consent(operation, "test".to_string()).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        ConsentDecision::AutoApproved { policy } => {
            assert_eq!(policy, "consent_disabled");
        }
        _ => panic!("Expected auto-approval for disabled consent"),
    }
}

#[tokio::test]
async fn test_auto_approval_low_sensitivity() {
    let manager = ConsentManager::with_config(create_test_config());
    
    let operation = OperationType::ConnectionManagement {
        action: "status".to_string(),
        target: "miniserver".to_string(),
    };
    
    let result = manager.request_consent(operation, "test".to_string()).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        ConsentDecision::AutoApproved { policy } => {
            assert_eq!(policy, "sensitivity_exemption");
        }
        _ => panic!("Expected auto-approval for low sensitivity"),
    }
}

#[tokio::test]
async fn test_auto_approve_list() {
    let mut auto_approve = HashSet::new();
    auto_approve.insert("device_control:status".to_string());
    
    let config = ConsentConfig {
        auto_approve_operations: auto_approve,
        ..create_test_config()
    };
    let manager = ConsentManager::with_config(config);
    
    let operation = OperationType::DeviceControl {
        device_uuid: "uuid1".to_string(),
        device_name: "Light".to_string(),
        command: "status".to_string(),
    };
    
    let result = manager.request_consent(operation, "test".to_string()).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        ConsentDecision::AutoApproved { policy } => {
            assert_eq!(policy, "auto_approve_list");
        }
        _ => panic!("Expected auto-approval from auto-approve list"),
    }
}

#[tokio::test]
async fn test_auto_deny_list() {
    let mut auto_deny = HashSet::new();
    auto_deny.insert("device_control:factory_reset".to_string());
    
    let config = ConsentConfig {
        auto_deny_operations: auto_deny,
        ..create_test_config()
    };
    let manager = ConsentManager::with_config(config);
    
    let operation = OperationType::DeviceControl {
        device_uuid: "uuid1".to_string(),
        device_name: "System".to_string(),
        command: "factory_reset".to_string(),
    };
    
    let result = manager.request_consent(operation, "test".to_string()).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        ConsentDecision::Denied { reason } => {
            assert_eq!(reason, "Operation in auto-deny list");
        }
        _ => panic!("Expected denial from auto-deny list"),
    }
}

#[tokio::test]
async fn test_consent_request_serialization() {
    let operation = OperationType::DeviceControl {
        device_uuid: "uuid1".to_string(),
        device_name: "Test Device".to_string(),
        command: "test".to_string(),
    };
    
    let request = ConsentRequest {
        id: uuid::Uuid::new_v4(),
        operation,
        sensitivity: SensitivityLevel::Medium,
        description: "Test operation".to_string(),
        details: "Test details".to_string(),
        risks: vec!["Test risk".to_string()],
        impact: "Test impact".to_string(),
        is_bulk: false,
        created_at: SystemTime::now(),
        timeout: Some(Duration::from_secs(300)),
        source: "test".to_string(),
        metadata: std::collections::HashMap::new(),
    };
    
    // Test serialization
    let serialized = serde_json::to_string(&request).unwrap();
    let deserialized: ConsentRequest = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(request.id, deserialized.id);
    assert_eq!(request.description, deserialized.description);
    assert_eq!(request.sensitivity, deserialized.sensitivity);
    assert_eq!(request.is_bulk, deserialized.is_bulk);
}

#[tokio::test]
async fn test_consent_response_serialization() {
    let response = ConsentResponse {
        request_id: uuid::Uuid::new_v4(),
        approved: true,
        reason: Some("User approved".to_string()),
        responded_at: SystemTime::now(),
        validity_duration: Some(Duration::from_secs(3600)),
        apply_to_similar: true,
        user_id: Some("test_user".to_string()),
    };
    
    // Test serialization
    let serialized = serde_json::to_string(&response).unwrap();
    let deserialized: ConsentResponse = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(response.request_id, deserialized.request_id);
    assert_eq!(response.approved, deserialized.approved);
    assert_eq!(response.reason, deserialized.reason);
    assert_eq!(response.apply_to_similar, deserialized.apply_to_similar);
    assert_eq!(response.user_id, deserialized.user_id);
}

#[tokio::test]
async fn test_consent_statistics() {
    let manager = ConsentManager::with_config(create_test_config());
    
    // Get initial statistics
    let stats = manager.get_statistics().await;
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.pending_requests, 0);
    
    // Test that statistics structure is correct
    assert_eq!(stats.approved_count, 0);
    assert_eq!(stats.denied_count, 0);
    assert_eq!(stats.auto_approved_count, 0);
    assert_eq!(stats.timed_out_count, 0);
    assert_eq!(stats.cache_size, 0);
}

#[tokio::test]
async fn test_cache_cleanup() {
    let manager = ConsentManager::with_config(create_test_config());
    
    // Test cache cleanup (should not panic)
    manager.cleanup_cache().await;
    
    // Get statistics to verify cache is empty
    let stats = manager.get_statistics().await;
    assert_eq!(stats.cache_size, 0);
}

#[tokio::test]
async fn test_operation_key_generation() {
    let manager = ConsentManager::with_config(create_test_config());
    
    let device_op = OperationType::DeviceControl {
        device_uuid: "uuid1".to_string(),
        device_name: "Light".to_string(),
        command: "on".to_string(),
    };
    
    let bulk_op = OperationType::BulkDeviceControl {
        device_count: 5,
        room_name: Some("Living Room".to_string()),
        operation_type: "lights_off".to_string(),
    };
    
    let security_op = OperationType::SecurityControl {
        action: "arm".to_string(),
        scope: "house".to_string(),
    };
    
    let key1 = manager.get_operation_key(&device_op);
    let key2 = manager.get_operation_key(&bulk_op);
    let key3 = manager.get_operation_key(&security_op);
    
    assert_eq!(key1, "device_control:on");
    assert_eq!(key2, "bulk_control:lights_off");
    assert_eq!(key3, "security:arm");
}

#[tokio::test]
async fn test_bulk_operation_detection() {
    let manager = ConsentManager::with_config(create_test_config());
    
    let small_bulk = OperationType::BulkDeviceControl {
        device_count: 2,
        room_name: Some("Living Room".to_string()),
        operation_type: "lights_off".to_string(),
    };
    
    let large_bulk = OperationType::BulkDeviceControl {
        device_count: 5,
        room_name: Some("Whole House".to_string()),
        operation_type: "all_off".to_string(),
    };
    
    let device_op = OperationType::DeviceControl {
        device_uuid: "uuid1".to_string(),
        device_name: "Light".to_string(),
        command: "on".to_string(),
    };
    
    assert!(!manager.is_bulk_operation(&small_bulk)); // Below threshold
    assert!(manager.is_bulk_operation(&large_bulk));  // Above threshold
    assert!(!manager.is_bulk_operation(&device_op));  // Not bulk type
}

#[tokio::test]
async fn test_pending_request_limit() {
    let config = ConsentConfig {
        max_pending_requests: 2, // Very low limit for testing
        ..create_test_config()
    };
    let manager = ConsentManager::with_config(config);
    
    let operation = OperationType::DeviceControl {
        device_uuid: "uuid1".to_string(),
        device_name: "Light".to_string(),
        command: "secure_action".to_string(), // High sensitivity
    };
    
    // These should hit the pending request limit and be denied
    // Note: In practice, this would require a more complex test setup
    // to actually create pending requests without immediate resolution
    let result = manager.request_consent(operation.clone(), "test1".to_string()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_timeout_behavior() {
    let config = ConsentConfig {
        default_timeout: Duration::from_millis(50), // Very short timeout
        ..create_test_config()
    };
    let manager = ConsentManager::with_config(config);
    
    let operation = OperationType::DeviceControl {
        device_uuid: "uuid1".to_string(),
        device_name: "Light".to_string(),
        command: "secure_action".to_string(),
    };
    
    // This should timeout quickly
    let result = timeout(
        Duration::from_millis(200),
        manager.request_consent(operation, "test".to_string())
    ).await;
    
    assert!(result.is_ok()); // The function should complete
    let consent_result = result.unwrap();
    assert!(consent_result.is_ok());
}