//! Test framework v0.4.0 authentication integration

use pulseengine_mcp_auth::{AuthConfig, AuthenticationManager};

#[tokio::test]
async fn test_development_auth_config() {
    // Create a memory-based auth config for testing (avoids file system encryption issues)
    let auth_config = AuthConfig::memory();

    // Create auth manager
    let _auth_manager = AuthenticationManager::new(auth_config)
        .await
        .expect("Failed to create auth manager");

    // Verify auth manager was created successfully
    // Just check that we can create it without panicking
}

#[tokio::test]
async fn test_disabled_auth_config() {
    // Create a disabled auth config with memory storage to avoid file system issues
    let mut auth_config = AuthConfig::memory();
    auth_config.enabled = false;

    // Create auth manager
    let _auth_manager = AuthenticationManager::new(auth_config)
        .await
        .expect("Failed to create disabled auth manager");

    // Verify auth manager was created successfully
    // Just check that we can create it without panicking
}
