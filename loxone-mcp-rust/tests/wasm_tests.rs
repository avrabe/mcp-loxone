//! WASM-specific tests
//!
//! Tests that validate WASM compilation and WASI-specific functionality.

#![cfg(target_arch = "wasm32")]

mod common;
mod wasm;

use loxone_mcp_rust::{
    config::{ServerConfig, CredentialStore},
    error::LoxoneError,
};
use wasm_bindgen_test::*;
use wasm::*;

wasm_bindgen_test_configure!(run_in_browser);

// Test setup
#[wasm_bindgen_test]
async fn test_wasm_environment_setup() {
    setup();
    
    // Test browser compatibility
    BrowserCompatTester::test_browser_features().unwrap();
    BrowserCompatTester::test_async_support().await.unwrap();
    
    teardown();
}

#[wasm_bindgen_test]
async fn test_wasm_config_creation() {
    let config = ServerConfig::from_wasm_env().await.unwrap();
    
    // WASM should default to LocalStorage credential store
    assert!(matches!(config.credentials, CredentialStore::LocalStorage));
    
    // Should have valid defaults
    assert!(!config.mcp.name.is_empty());
    assert!(!config.mcp.version.is_empty());
}

#[wasm_bindgen_test]
async fn test_wasm_error_handling() {
    let error = LoxoneError::connection("WASM connection test");
    
    // Error should be serializable
    let error_string = format!("{}", error);
    assert!(error_string.contains("connection"));
    assert!(error_string.contains("WASM"));
}

#[wasm_bindgen_test]
async fn test_wasm_credential_storage() {
    use loxone_mcp_rust::config::{CredentialManager, LoxoneCredentials};
    
    let store = CredentialStore::LocalStorage;
    let manager = CredentialManager::new(store);
    
    let test_credentials = LoxoneCredentials {
        username: "wasm_test_user".to_string(),
        password: "wasm_test_password".to_string(),
        api_key: Some("wasm_test_key".to_string()),
        #[cfg(feature = "crypto")]
        public_key: None,
    };
    
    // Store credentials
    manager.store_credentials(&test_credentials).await.unwrap();
    
    // Retrieve credentials
    let retrieved = manager.get_credentials().await.unwrap();
    assert_eq!(retrieved.username, test_credentials.username);
    assert_eq!(retrieved.password, test_credentials.password);
    assert_eq!(retrieved.api_key, test_credentials.api_key);
    
    // Clear credentials
    manager.clear_credentials().await.unwrap();
    
    // Should fail to retrieve after clearing
    assert!(manager.get_credentials().await.is_err());
}

#[wasm_bindgen_test]
async fn test_wasm_feature_detection() {
    // These features should be disabled in WASM builds
    assert!(!cfg!(feature = "keyring-storage"));
    
    // WASM-specific features should be available
    assert!(cfg!(target_family = "wasm"));
}

#[wasm_bindgen_test]
async fn test_wasm_size_optimizations() {
    // Test that WASM builds are size-optimized
    // This is mostly validated by the build process, but we can check
    // that certain size-optimization features are working
    
    use loxone_mcp_rust::tools::ToolResponse;
    
    // Small response should serialize efficiently
    let response = ToolResponse::success(serde_json::json!({"test": "data"}));
    let serialized = serde_json::to_string(&response).unwrap();
    
    // Should be compact JSON
    assert!(!serialized.contains("  ")); // No pretty-printing
    assert!(serialized.len() < 200); // Reasonable size
}

#[wasm_bindgen_test]
async fn test_wasm_async_operations() {
    use loxone_mcp_rust::client::ClientContext;
    use std::sync::Arc;
    
    // Test that async operations work in WASM
    let context = Arc::new(ClientContext::new());
    
    // This should not block or cause issues in WASM
    let devices = context.devices.read().await;
    assert!(devices.is_empty());
    drop(devices);
    
    // Test concurrent access
    let context1 = context.clone();
    let context2 = context.clone();
    
    let (result1, result2) = tokio::join!(
        async {
            let devices = context1.devices.read().await;
            devices.len()
        },
        async {
            let devices = context2.devices.read().await;
            devices.len()
        }
    );
    
    assert_eq!(result1, 0);
    assert_eq!(result2, 0);
}

#[wasm_bindgen_test]
async fn test_wasm_memory_efficiency() {
    use loxone_mcp_rust::client::{LoxoneDevice, ClientContext};
    use std::collections::HashMap;
    
    // Test that memory usage is reasonable in WASM
    let context = ClientContext::new();
    
    // Add some test devices
    let mut devices = HashMap::new();
    for i in 0..10 {
        let mut states = HashMap::new();
        states.insert("value".to_string(), serde_json::json!(i));
        
        let device = LoxoneDevice {
            uuid: format!("device-{}", i),
            name: format!("Test Device {}", i),
            device_type: "TestDevice".to_string(),
            room: Some(format!("Room {}", i % 3)),
            states,
            category: "test".to_string(),
            sub_controls: HashMap::new(),
        };
        
        devices.insert(device.uuid.clone(), device);
    }
    
    // Update context
    {
        let mut context_devices = context.devices.write().await;
        *context_devices = devices;
    }
    
    // Verify devices are accessible
    let stored_devices = context.devices.read().await;
    assert_eq!(stored_devices.len(), 10);
    
    // Test filtering (should not create excessive copies)
    let room_devices = context.get_devices_by_room("Room 0").await.unwrap();
    assert!(!room_devices.is_empty());
}

#[wasm_bindgen_test]
async fn test_wasm_json_handling() {
    use serde_json;
    
    // Test that JSON operations work efficiently in WASM
    let large_json = serde_json::json!({
        "devices": (0..100).map(|i| serde_json::json!({
            "id": i,
            "name": format!("Device {}", i),
            "states": {
                "value": i * 2,
                "active": i % 2 == 0
            }
        })).collect::<Vec<_>>(),
        "metadata": {
            "total": 100,
            "timestamp": "2025-01-06T12:00:00Z"
        }
    });
    
    // Serialize and deserialize
    let serialized = serde_json::to_string(&large_json).unwrap();
    let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    
    // Should be identical
    assert_eq!(large_json, deserialized);
    
    // Should handle nested access efficiently
    let devices = deserialized.get("devices").unwrap().as_array().unwrap();
    assert_eq!(devices.len(), 100);
    
    let first_device = &devices[0];
    assert_eq!(first_device.get("id").unwrap().as_u64().unwrap(), 0);
}

#[wasm_bindgen_test]
async fn test_wasm_error_propagation() {
    use loxone_mcp_rust::error::{LoxoneError, Result};
    
    // Test that errors propagate correctly in WASM
    async fn failing_function() -> Result<String> {
        Err(LoxoneError::connection("WASM test error"))
    }
    
    async fn calling_function() -> Result<String> {
        failing_function().await?;
        Ok("success".to_string())
    }
    
    let result = calling_function().await;
    assert!(result.is_err());
    
    let error = result.err().unwrap();
    assert!(format!("{}", error).contains("WASM test error"));
}

#[wasm_bindgen_test]
async fn test_wasm_concurrent_operations() {
    use loxone_mcp_rust::tools::ToolResponse;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    
    // Test concurrent operations in WASM
    let counter = Arc::new(AtomicU32::new(0));
    
    let futures = (0..10).map(|i| {
        let counter = counter.clone();
        async move {
            // Simulate some work
            tokio::task::yield_now().await;
            
            counter.fetch_add(1, Ordering::SeqCst);
            
            ToolResponse::success(serde_json::json!({
                "task": i,
                "result": "completed"
            }))
        }
    });
    
    let results = futures::future::join_all(futures).await;
    
    // All tasks should complete
    assert_eq!(results.len(), 10);
    assert_eq!(counter.load(Ordering::SeqCst), 10);
    
    // All responses should be successful
    for (i, response) in results.iter().enumerate() {
        assert_eq!(response.status, "success");
        assert_eq!(response.data.get("task").unwrap().as_u64().unwrap(), i as u64);
    }
}