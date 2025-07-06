//! Test framework v0.4.0 authentication integration

use pulseengine_mcp_auth::{AuthConfig, AuthenticationManager};

#[tokio::test]
async fn test_development_auth_config() {
    // Create a simple auth config for testing
    let auth_config = AuthConfig {
        enabled: true,
        ..Default::default()
    };
    
    // Create auth manager
    let auth_manager = AuthenticationManager::new(auth_config).await
        .expect("Failed to create auth manager");
    
    // Verify auth manager was created successfully
    // Just check that we can create it without panicking
}

#[tokio::test]
async fn test_disabled_auth_config() {
    // Create a disabled auth config
    let auth_config = AuthConfig {
        enabled: false,
        ..Default::default()
    };
    
    // Create auth manager
    let auth_manager = AuthenticationManager::new(auth_config).await
        .expect("Failed to create disabled auth manager");
    
    // Verify auth manager was created successfully
    // Just check that we can create it without panicking
}