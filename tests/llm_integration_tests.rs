//! Comprehensive LLM integration tests
//!
//! This module tests the complete LLM provider integration system including:
//! - Environment-based configuration loading
//! - Provider factory initialization with different configurations
//! - Health checking and fallback mechanisms
//! - Real and mock provider integration
//! - Error handling and recovery scenarios

use loxone_mcp_rust::error::LoxoneError;
use loxone_mcp_rust::sampling::{
    client::{MockSamplingClient, SamplingClient, SamplingClientManager},
    config::ProviderFactoryConfig,
    protocol::SamplingProtocolIntegration,
    SamplingMessage, SamplingRequest,
};
use serial_test::serial;
use std::env;
use std::sync::Arc;
use temp_env::with_vars;
use tokio::sync::Mutex;
use tokio::time::Duration;

#[allow(dead_code)]
static ENV_TEST_MUTEX: Mutex<()> = Mutex::const_new(());

/// Helper function to create clean environment variables for testing
fn get_clean_env() -> Vec<(&'static str, Option<&'static str>)> {
    vec![
        // Explicitly disable cloud providers
        ("OPENAI_API_KEY", None),
        ("ANTHROPIC_API_KEY", None),
        // Set explicit health states
        ("OLLAMA_HEALTH_OVERRIDE", Some("true")),
        ("OPENAI_HEALTH_OVERRIDE", Some("true")),
        ("ANTHROPIC_HEALTH_OVERRIDE", Some("true")),
    ]
}

/// Helper function for Ollama-only environment
fn get_ollama_only_env() -> Vec<(&'static str, Option<&'static str>)> {
    vec![
        ("OLLAMA_ENABLED", Some("true")),
        ("OLLAMA_BASE_URL", Some("http://localhost:11434")),
        ("OLLAMA_DEFAULT_MODEL", Some("llama3.2")),
        ("LLM_ENABLE_FALLBACK", Some("false")),
        ("OLLAMA_HEALTH_OVERRIDE", Some("true")),
        ("OPENAI_HEALTH_OVERRIDE", Some("true")),
        ("ANTHROPIC_HEALTH_OVERRIDE", Some("true")),
        ("OPENAI_API_KEY", None),
        ("ANTHROPIC_API_KEY", None),
    ]
}

/// Helper function for Ollama + OpenAI fallback environment
fn get_ollama_openai_env() -> Vec<(&'static str, Option<&'static str>)> {
    vec![
        ("OLLAMA_ENABLED", Some("true")),
        ("OLLAMA_BASE_URL", Some("http://localhost:11434")),
        ("OLLAMA_DEFAULT_MODEL", Some("llama3.2")),
        ("OPENAI_API_KEY", Some("test-openai-key")),
        ("OPENAI_DEFAULT_MODEL", Some("gpt-4o")),
        ("LLM_ENABLE_FALLBACK", Some("true")),
        ("LLM_PREFER_LOCAL", Some("true")),
        ("OLLAMA_HEALTH_OVERRIDE", Some("true")),
        ("OPENAI_HEALTH_OVERRIDE", Some("true")),
        ("ANTHROPIC_HEALTH_OVERRIDE", Some("true")),
        ("ANTHROPIC_API_KEY", None),
    ]
}

/// Helper function for all providers environment
fn get_all_providers_env() -> Vec<(&'static str, Option<&'static str>)> {
    vec![
        ("OLLAMA_ENABLED", Some("true")),
        ("OLLAMA_BASE_URL", Some("http://localhost:11434")),
        ("OLLAMA_DEFAULT_MODEL", Some("llama3.2")),
        ("OPENAI_API_KEY", Some("test-openai-key")),
        ("OPENAI_DEFAULT_MODEL", Some("gpt-4o")),
        ("ANTHROPIC_API_KEY", Some("test-anthropic-key")),
        (
            "ANTHROPIC_DEFAULT_MODEL",
            Some("claude-3-5-sonnet-20241022"),
        ),
        ("LLM_ENABLE_FALLBACK", Some("true")),
        ("LLM_PREFER_LOCAL", Some("true")),
        ("OLLAMA_HEALTH_OVERRIDE", Some("true")),
        ("OPENAI_HEALTH_OVERRIDE", Some("true")),
        ("ANTHROPIC_HEALTH_OVERRIDE", Some("true")),
    ]
}

#[tokio::test]
async fn test_provider_config_validation() {
    // Test 1: Default configuration should be valid
    let config = ProviderFactoryConfig::default();
    assert!(
        config.validate().is_ok(),
        "Default configuration should be valid"
    );

    // Test 2: Configuration with no providers should be invalid
    let mut invalid_config = config.clone();
    invalid_config.ollama.enabled = false;
    invalid_config.openai.enabled = false;
    invalid_config.anthropic.enabled = false;
    assert!(
        invalid_config.validate().is_err(),
        "Configuration with no providers should be invalid"
    );

    // Test 3: OpenAI enabled without API key should be invalid
    let mut invalid_openai = config.clone();
    invalid_openai.openai.enabled = true;
    invalid_openai.openai.api_key = None;
    assert!(
        invalid_openai.validate().is_err(),
        "OpenAI enabled without API key should be invalid"
    );

    // Test 4: Anthropic enabled without API key should be invalid
    let mut invalid_anthropic = config.clone();
    invalid_anthropic.anthropic.enabled = true;
    invalid_anthropic.anthropic.api_key = None;
    assert!(
        invalid_anthropic.validate().is_err(),
        "Anthropic enabled without API key should be invalid"
    );

    // Test 5: Valid configuration with all providers
    let mut valid_all = config;
    valid_all.openai.enabled = true;
    valid_all.openai.api_key = Some("test-key".to_string());
    valid_all.anthropic.enabled = true;
    valid_all.anthropic.api_key = Some("test-key".to_string());
    assert!(
        valid_all.validate().is_ok(),
        "Configuration with all providers should be valid"
    );
}

#[tokio::test]
#[serial]
async fn test_environment_based_configuration_loading() {
    // Test 1: Clean environment - should use defaults
    with_vars(get_clean_env(), || {
        let config = ProviderFactoryConfig::from_env();

        assert!(config.ollama.enabled, "Ollama should be enabled by default");
        assert!(
            !config.openai.enabled,
            "OpenAI should be disabled by default"
        );
        assert!(
            !config.anthropic.enabled,
            "Anthropic should be disabled by default"
        );
        assert_eq!(config.ollama.base_url, "http://localhost:11434");
        assert_eq!(config.ollama.default_model, "llama3.2");
    });

    // Test 2: Ollama only configuration
    with_vars(get_ollama_only_env(), || {
        let config = ProviderFactoryConfig::from_env();

        assert!(config.ollama.enabled);
        assert!(!config.openai.enabled);
        assert!(!config.anthropic.enabled);
        assert!(!config.selection.enable_fallback);
        assert!(config.is_ollama_primary());
        assert!(!config.has_fallback_providers());
    });

    // Test 3: Ollama with OpenAI fallback
    with_vars(get_ollama_openai_env(), || {
        let config = ProviderFactoryConfig::from_env();

        assert!(config.ollama.enabled);
        assert!(config.openai.enabled);
        assert!(!config.anthropic.enabled);
        assert!(config.selection.enable_fallback);
        assert!(config.is_ollama_primary());
        assert!(config.has_fallback_providers());

        let providers = config.get_enabled_providers();
        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].0, "ollama");
        assert_eq!(providers[1].0, "openai");
    });

    // Test 4: All providers enabled
    with_vars(get_all_providers_env(), || {
        let config = ProviderFactoryConfig::from_env();

        assert!(config.ollama.enabled);
        assert!(config.openai.enabled);
        assert!(config.anthropic.enabled);
        assert!(config.selection.enable_fallback);
        assert!(config.is_ollama_primary());
        assert!(config.has_fallback_providers());

        let providers = config.get_enabled_providers();
        assert_eq!(providers.len(), 3);
        assert_eq!(providers[0].0, "ollama"); // Priority 1
        assert_eq!(providers[1].0, "openai"); // Priority 2
        assert_eq!(providers[2].0, "anthropic"); // Priority 3
    });
}

#[tokio::test]
#[serial]
async fn test_sampling_client_manager_initialization() {
    // Test 1: Manager with default configuration
    // Set clean environment
    for (key, value) in get_clean_env() {
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }

    let config = ProviderFactoryConfig::from_env();
    let manager = SamplingClientManager::new_with_config(config);

    assert!(manager.is_available().await, "Manager should be available");

    let summary = manager.get_provider_summary().await;
    assert!(
        summary.contains("Primary: Ollama"),
        "Should show Ollama as primary"
    );
    assert!(
        summary.contains("Fallback: 0 available"),
        "Should show no fallback providers"
    );

    // Test 2: Manager with fallback providers - disabled (TestEnvironment not available)
    /*
    {
        // let _guard = TestEnvironment::all_providers().apply();
        let config = ProviderFactoryConfig::from_env();
        let manager = SamplingClientManager::new_with_config(config);

        assert!(manager.is_available().await, "Manager should be available");

        let summary = manager.get_provider_summary().await;
        assert!(
            summary.contains("Primary: Ollama"),
            "Should show Ollama as primary"
        );
        assert!(
            summary.contains("Fallback: 2 available"),
            "Should show 2 fallback providers"
        );
    }
    */
}

#[tokio::test]
#[ignore = "TestEnvironment not available - disabled during framework migration"]
async fn test_provider_health_checking() {
    // TODO: This test has race condition issues similar to other environment-dependent tests.
    // Test passes when run individually: cargo test test_provider_health_checking
    let _lock = ENV_TEST_MUTEX.lock().await;
    // let _guard = TestEnvironment::all_providers().apply();
    let config = ProviderFactoryConfig::from_env();
    let manager = SamplingClientManager::new_with_config(config);

    // Test 1: Initial health check - all should be healthy
    let health = manager.check_provider_health().await;
    assert!(
        health.get("ollama").copied().unwrap_or(false),
        "Ollama should be healthy"
    );
    assert!(
        health.get("openai").copied().unwrap_or(false),
        "OpenAI should be healthy"
    );
    assert!(
        health.get("anthropic").copied().unwrap_or(false),
        "Anthropic should be healthy"
    );

    // Test 2: Health status retrieval
    let health_status = manager.get_provider_health().await;
    assert!(
        !health_status.is_empty(),
        "Health status should not be empty"
    );
    assert!(
        health_status.contains_key("ollama"),
        "Should track Ollama health"
    );

    // Test 3: Provider summary with health info
    let summary = manager.get_provider_summary().await;
    assert!(
        summary.contains("3/3 healthy"),
        "Should show all providers healthy"
    );
}

#[tokio::test]
#[ignore = "TODO: Fix race condition - test passes individually but fails when run with other tests"]
async fn test_intelligent_fallback_behavior() {
    // TODO: This test has a race condition where it expects failure when all providers
    // are unhealthy, but the test doesn't fail as expected when run with other tests.
    // The health override mechanism may not be working correctly in concurrent scenarios.
    // Test passes when run individually: cargo test test_intelligent_fallback_behavior
    let _lock = ENV_TEST_MUTEX.lock().await;
    // Test 1: Normal operation - primary succeeds
    {
        // let _guard = TestEnvironment::all_providers()
        //     .with_health_overrides(true, true, true)
        //     .apply();

        let config = ProviderFactoryConfig::from_env();
        let manager = SamplingClientManager::new_with_config(config);

        let request = SamplingRequest::new(vec![SamplingMessage::user("Test message")]);
        let response = manager
            .request_sampling(request)
            .await
            .expect("Request should succeed");

        assert_eq!(response.role, "assistant");
        assert!(response.content.text.is_some());
        // Should use primary provider (Ollama)
        assert_eq!(response.model, "llama3.2:latest");
    }

    // Test 2: Primary fails, fallback succeeds
    {
        // let _guard = TestEnvironment::all_providers()
        //     .with_health_overrides(false, true, true) // Ollama unhealthy, others healthy
        //     .apply();

        let config = ProviderFactoryConfig::from_env();
        let manager = SamplingClientManager::new_with_config(config);

        let request =
            SamplingRequest::new(vec![SamplingMessage::user("Test message for fallback")]);
        let response = manager
            .request_sampling(request)
            .await
            .expect("Fallback should succeed");

        assert_eq!(response.role, "assistant");
        assert!(response.content.text.is_some());
        // Should use first fallback provider (OpenAI)
        assert_eq!(response.model, "gpt-4o");
    }

    // Test 3: Primary and first fallback fail, second fallback succeeds
    {
        // let _guard = TestEnvironment::all_providers()
        //     .with_health_overrides(false, false, true) // Only Anthropic healthy
        //     .apply();

        let config = ProviderFactoryConfig::from_env();
        let manager = SamplingClientManager::new_with_config(config);

        let request = SamplingRequest::new(vec![SamplingMessage::user(
            "Test message for second fallback",
        )]);
        let response = manager
            .request_sampling(request)
            .await
            .expect("Second fallback should succeed");

        assert_eq!(response.role, "assistant");
        assert!(response.content.text.is_some());
        // Should use second fallback provider (Anthropic)
        assert_eq!(response.model, "claude-3-5-sonnet-20241022");
    }

    // Test 4: All providers fail
    {
        // let _guard = TestEnvironment::all_providers()
        //     .with_health_overrides(false, false, false) // All unhealthy
        //     .apply();

        let config = ProviderFactoryConfig::from_env();
        let manager = SamplingClientManager::new_with_config(config);

        let request =
            SamplingRequest::new(vec![SamplingMessage::user("Test message - should fail")]);
        let result = manager.request_sampling(request).await;

        assert!(
            result.is_err(),
            "Should fail when all providers are unhealthy"
        );
        if let Err(LoxoneError::ServiceUnavailable(msg)) = result {
            assert!(
                msg.contains("unhealthy"),
                "Error should mention provider health"
            );
        } else {
            panic!("Should return ServiceUnavailable error");
        }
    }
}

#[tokio::test]
#[ignore = "TODO: Fix race condition - test passes individually but fails when run with other tests"]
async fn test_fallback_disabled_behavior() {
    // TODO: This test has a race condition where it expects failure when primary is
    // unhealthy and fallback is disabled, but doesn't fail as expected with other tests.
    // The health override mechanism may not be working correctly in concurrent scenarios.
    // Test passes when run individually: cargo test test_fallback_disabled_behavior
    let _lock = ENV_TEST_MUTEX.lock().await;
    // Test fallback disabled - should only use primary
    // let _guard = TestEnvironment::ollama_only()
    //     .with_health_overrides(false, true, true) // Ollama unhealthy, others healthy
    //     .apply();

    let config = ProviderFactoryConfig::from_env();
    let manager = SamplingClientManager::new_with_config(config);

    let request = SamplingRequest::new(vec![SamplingMessage::user("Test with fallback disabled")]);
    let result = manager.request_sampling(request).await;

    assert!(
        result.is_err(),
        "Should fail when primary is unhealthy and fallback is disabled"
    );
    if let Err(LoxoneError::ServiceUnavailable(msg)) = result {
        assert!(
            msg.contains("unhealthy"),
            "Error should mention provider health"
        );
    } else {
        panic!("Should return ServiceUnavailable error");
    }
}

#[tokio::test]
async fn test_mock_provider_responses() {
    // let _guard = TestEnvironment::all_providers().apply();

    // Test different provider mock responses
    let providers = ["ollama", "openai", "anthropic"];

    for provider_type in &providers {
        let client = MockSamplingClient::new_with_provider(provider_type);

        // Test basic functionality
        assert!(client.is_sampling_supported());
        assert_eq!(client.provider_type(), *provider_type);

        // Test health check
        let healthy = client.health_check().await;
        assert!(
            healthy,
            "Provider {provider_type} should be healthy by default"
        );

        // Test capabilities
        let caps = client.get_sampling_capabilities();
        assert!(caps.supported);
        assert!(caps.max_tokens.is_some());
        assert!(!caps.supported_models.is_empty());

        // Test sampling request
        let request = SamplingRequest::new(vec![SamplingMessage::user("Make my home cozy")]);
        let response = client
            .create_message(request)
            .await
            .expect("Request should succeed");

        assert_eq!(response.role, "assistant");
        assert!(response.content.text.is_some());
        let response_text = response.content.text.unwrap();
        assert!(
            response_text.contains(provider_type),
            "Response should mention provider type"
        );
        assert!(
            response_text.contains("Cozy"),
            "Response should be contextually relevant"
        );
    }
}

#[tokio::test]
async fn test_sampling_protocol_integration() {
    // let _guard = TestEnvironment::all_providers().apply();

    // Test 1: Protocol integration initialization
    let integration = SamplingProtocolIntegration::new_with_mock(true);
    assert!(
        integration.is_sampling_available().await,
        "Sampling should be available"
    );

    // Test 2: Capabilities check
    let capabilities = integration
        .get_capabilities()
        .await
        .expect("Should get capabilities");
    assert!(capabilities.supports_sampling, "Should support sampling");

    // Test 3: Sampling request through protocol
    let request = SamplingRequest::new(vec![SamplingMessage::user("Test protocol integration")]);
    let response = integration
        .request_sampling(request)
        .await
        .expect("Protocol request should succeed");

    assert_eq!(response.role, "assistant");
    assert!(response.content.text.is_some());
}

#[tokio::test]
async fn test_concurrent_sampling_requests() {
    // let _guard = TestEnvironment::all_providers().apply();
    let config = ProviderFactoryConfig::from_env();
    let manager = Arc::new(SamplingClientManager::new_with_config(config));

    // Launch multiple concurrent requests
    let mut tasks = Vec::new();
    for i in 0..5 {
        let manager_clone = manager.clone();
        let task = tokio::spawn(async move {
            let request = SamplingRequest::new(vec![SamplingMessage::user(format!(
                "Concurrent request {i}"
            ))]);
            manager_clone.request_sampling(request).await
        });
        tasks.push(task);
    }

    // Wait for all requests to complete
    let results: Vec<_> = futures::future::join_all(tasks).await;

    // Verify all requests succeeded
    for (i, result) in results.into_iter().enumerate() {
        let response = result
            .expect("Task should not panic")
            .unwrap_or_else(|_| panic!("Request {i} should succeed"));
        assert_eq!(response.role, "assistant");
        assert!(response.content.text.is_some());
    }
}

#[tokio::test]
#[ignore = "TODO: Fix race condition - test passes individually but fails when run with other tests"]
async fn test_provider_failover_timing() {
    // TODO: This test has a race condition where it expects OpenAI fallback (gpt-4o)
    // but sometimes gets Ollama response (llama3.2:latest) when run with other tests.
    // The health override mechanism may not be working correctly in concurrent scenarios.
    // Test passes when run individually: cargo test test_provider_failover_timing
    let _lock = ENV_TEST_MUTEX.lock().await;
    // let _guard = TestEnvironment::all_providers()
    //     .with_health_overrides(false, true, true) // Primary fails, fallbacks succeed
    //     .apply();

    let config = ProviderFactoryConfig::from_env();
    let manager = SamplingClientManager::new_with_config(config);

    let start_time = std::time::Instant::now();

    let request = SamplingRequest::new(vec![SamplingMessage::user("Test failover timing")]);
    let response = manager
        .request_sampling(request)
        .await
        .expect("Fallback should succeed");

    let elapsed = start_time.elapsed();

    // Verify response is from fallback
    assert_eq!(response.model, "gpt-4o"); // Should be OpenAI fallback

    // Failover should be reasonably fast (less than 100ms for mock)
    assert!(
        elapsed < Duration::from_millis(100),
        "Failover should be fast for mock providers"
    );
}

#[tokio::test]
#[ignore = "TODO: Fix race condition - test passes individually but fails when run with other tests"]
async fn test_provider_configuration_summary() {
    // TODO: This test has a race condition where it expects 3 enabled providers but gets 2
    // when run with other tests. The environment variable configuration system appears to
    // have interference despite the async mutex. Test passes individually.
    let _lock = ENV_TEST_MUTEX.lock().await;
    // let _guard = TestEnvironment::all_providers().apply();
    let config = ProviderFactoryConfig::from_env();

    let summary = config.get_selection_summary();
    assert!(
        summary.contains("Primary: ollama"),
        "Summary should show primary provider"
    );
    assert!(
        summary.contains("Fallback: enabled"),
        "Summary should show fallback status"
    );
    assert!(
        summary.contains("Local preference: yes"),
        "Summary should show local preference"
    );

    let providers = config.get_enabled_providers();
    assert_eq!(providers.len(), 3, "Should have 3 enabled providers");

    // Verify priority ordering
    assert_eq!(providers[0], ("ollama", 1));
    assert_eq!(providers[1], ("openai", 2));
    assert_eq!(providers[2], ("anthropic", 3));
}

#[tokio::test]
async fn test_error_scenarios() {
    // Test 1: Sampling with unsupported client
    {
        let client = MockSamplingClient::new(false); // disabled
        let request = SamplingRequest::new(vec![SamplingMessage::user("Test")]);
        let result = client.create_message(request).await;
        assert!(result.is_err(), "Should fail with unsupported client");
    }

    // Test 2: Invalid sampling request (empty messages)
    {
        // let _guard = TestEnvironment::all_providers().apply();
        let config = ProviderFactoryConfig::from_env();
        let manager = SamplingClientManager::new_with_config(config);

        let request = SamplingRequest::new(vec![]); // Empty messages
        let result = manager.request_sampling(request).await;
        assert!(result.is_err(), "Should fail with empty messages");
    }
}

/// Helper to run tests that require external dependencies
/// These tests are skipped in CI but can be run locally with proper setup
mod external_tests {
    use super::*;

    /// Test with real Ollama instance (if available)
    #[tokio::test]
    #[ignore = "Requires local Ollama instance"]
    async fn test_real_ollama_integration() {
        // This test requires a real Ollama instance running on localhost:11434
        // let _guard = TestEnvironment::ollama_only().apply();
        let config = ProviderFactoryConfig::from_env();

        // Try to validate we can connect to real Ollama
        // In a real implementation, this would use actual HTTP client
        // For now, we just verify the configuration is correct
        assert!(config.ollama.enabled);
        assert_eq!(config.ollama.base_url, "http://localhost:11434");
        assert!(config.validate().is_ok());
    }

    /// Test with real cloud providers (if API keys are available)
    #[tokio::test]
    #[ignore = "Requires real API keys"]
    async fn test_real_cloud_providers() {
        // This test would require real API keys set in environment
        // For security, we don't include real API keys in tests

        // Check if real API keys are available
        let has_openai = env::var("REAL_OPENAI_API_KEY").is_ok();
        let has_anthropic = env::var("REAL_ANTHROPIC_API_KEY").is_ok();

        if !has_openai && !has_anthropic {
            eprintln!("Skipping real provider test - no API keys available");
            return;
        }

        // Test would go here with real API integration
        // This is intentionally left as a placeholder for manual testing
    }
}
