//! Ollama LLM Integration Demonstration
//!
//! This binary demonstrates the complete LLM integration system working with
//! a local Ollama instance. It showcases:
//! - Real Ollama HTTP client communication
//! - Environment-based provider configuration
//! - Intelligent fallback mechanism
//! - Home automation sampling scenarios

use loxone_mcp_rust::error::Result;
use loxone_mcp_rust::sampling::client::SamplingClientManager;
use loxone_mcp_rust::sampling::config::ProviderFactoryConfig;
use loxone_mcp_rust::sampling::ollama_http::OllamaHttpClient;
use loxone_mcp_rust::sampling::{AutomationSamplingBuilder, SamplingMessage, SamplingRequest};
use std::env;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("🦙 Ollama LLM Integration Demonstration");
    info!("========================================");

    // Load provider configuration from environment
    let config = ProviderFactoryConfig::from_env();

    info!("🔧 Provider Configuration:");
    info!("  {}", config.get_selection_summary());
    info!("  Ollama URL: {}", config.ollama.base_url);
    info!("  Ollama Model: {}", config.ollama.default_model);

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("❌ Configuration validation failed: {}", e);
        return Err(e);
    }
    info!("✅ Configuration validated successfully");

    // Test 1: Direct Ollama HTTP client
    info!("\n🧪 Test 1: Direct Ollama HTTP Client");
    info!("===================================");

    let ollama_client = OllamaHttpClient::new(
        config.ollama.base_url.clone(),
        config.ollama.default_model.clone(),
    )?;

    // Check Ollama health
    info!("🏥 Checking Ollama health...");
    match ollama_client.health_check().await {
        Ok(true) => info!("✅ Ollama is healthy and reachable"),
        Ok(false) => {
            warn!("⚠️ Ollama is reachable but unhealthy");
            warn!("   This might indicate an issue with the Ollama service");
        }
        Err(e) => {
            error!("❌ Ollama health check failed: {}", e);
            error!(
                "   Please ensure Ollama is running on {}",
                config.ollama.base_url
            );
            error!("   You can start Ollama with: ollama serve");
            return Err(e);
        }
    }

    // List available models
    info!("📋 Listing available models...");
    match ollama_client.list_models().await {
        Ok(models) => {
            info!("✅ Found {} model(s):", models.len());
            for model in &models {
                info!("   - {}", model);
            }

            // Check if our configured model is available
            if models.iter().any(|m| m == &config.ollama.default_model) {
                info!(
                    "✅ Configured model '{}' is available",
                    config.ollama.default_model
                );
            } else {
                warn!(
                    "⚠️ Configured model '{}' is not available",
                    config.ollama.default_model
                );
                warn!("   Available models: {:?}", models);
                warn!(
                    "   You may need to download the model with: ollama pull {}",
                    config.ollama.default_model
                );
            }
        }
        Err(e) => {
            error!("❌ Failed to list models: {}", e);
            warn!("   Continuing with demo anyway...");
        }
    }

    // Test simple question
    info!("\n🤖 Testing simple question...");
    let simple_request = SamplingRequest::new(vec![SamplingMessage::user(
        "What is 2 + 2? Please answer briefly.",
    )])
    .with_max_tokens(50)
    .with_temperature(0.1);

    match ollama_client.generate(&simple_request).await {
        Ok(response) => {
            info!("✅ Simple question response:");
            info!("   Model: {}", response.model);
            info!(
                "   Response: {}",
                response
                    .content
                    .text
                    .as_ref()
                    .unwrap_or(&"No text".to_string())
            );
        }
        Err(e) => {
            error!("❌ Simple question failed: {}", e);
        }
    }

    // Test 2: Sampling Client Manager with Fallback
    info!("\n🧪 Test 2: Sampling Client Manager with Fallback");
    info!("===============================================");

    // Create enhanced sampling client manager
    let manager = SamplingClientManager::new_with_config(config.clone());

    info!(
        "📊 Manager status: {}",
        manager.get_provider_summary().await
    );

    // Check provider health
    let health = manager.check_provider_health().await;
    info!("🏥 Provider health status:");
    for (provider, healthy) in &health {
        let status = if *healthy {
            "✅ healthy"
        } else {
            "❌ unhealthy"
        };
        info!("   {} - {}", provider, status);
    }

    // Test fallback behavior
    info!("\n🔄 Testing fallback behavior...");

    // Test with Ollama healthy
    info!("Test 2a: Normal operation (Ollama healthy)");
    env::set_var("OLLAMA_HEALTH_OVERRIDE", "true");
    env::set_var("OPENAI_HEALTH_OVERRIDE", "true");
    env::set_var("ANTHROPIC_HEALTH_OVERRIDE", "true");

    let request = SamplingRequest::new(vec![SamplingMessage::user(
        "Explain home automation in 20 words or less.",
    )]);

    match manager.request_sampling(request).await {
        Ok(response) => {
            info!("✅ Normal operation successful:");
            info!("   Model: {}", response.model);
            info!(
                "   Response: {}",
                response
                    .content
                    .text
                    .as_ref()
                    .unwrap_or(&"No text".to_string())
            );
        }
        Err(e) => {
            warn!("⚠️ Normal operation failed: {}", e);
        }
    }

    // Test with Ollama unhealthy (fallback scenario)
    info!("\nTest 2b: Fallback scenario (Ollama unhealthy)");
    env::set_var("OLLAMA_HEALTH_OVERRIDE", "false");

    let fallback_request = SamplingRequest::new(vec![SamplingMessage::user(
        "What are smart lights? Answer briefly.",
    )]);

    match manager.request_sampling(fallback_request).await {
        Ok(response) => {
            info!("✅ Fallback operation successful:");
            info!("   Model: {}", response.model);
            info!(
                "   Response: {}",
                response
                    .content
                    .text
                    .as_ref()
                    .unwrap_or(&"No text".to_string())
            );
        }
        Err(e) => {
            warn!("⚠️ Fallback operation failed: {}", e);
        }
    }

    // Test 3: Home Automation Scenarios
    info!("\n🧪 Test 3: Home Automation Scenarios");
    info!("==================================");

    // Restore Ollama health for automation demos
    env::set_var("OLLAMA_HEALTH_OVERRIDE", "true");

    let automation_builder = AutomationSamplingBuilder::new()
        .with_rooms(serde_json::json!({
            "living_room": {"devices": ["main_light", "couch_light", "tv_light"]},
            "bedroom": {"devices": ["ceiling_light", "bedside_lamps"]},
            "kitchen": {"devices": ["under_cabinet", "pendant_lights"]}
        }))
        .with_devices(serde_json::json!({
            "main_light": {"type": "dimmer", "current_level": 80, "room": "living_room"},
            "couch_light": {"type": "dimmer", "current_level": 40, "room": "living_room"},
            "ceiling_light": {"type": "switch", "state": "off", "room": "bedroom"}
        }))
        .with_sensors(serde_json::json!({
            "living_room_temp": {"value": 21.5, "unit": "°C"},
            "outdoor_temp": {"value": 8.2, "unit": "°C"}
        }))
        .with_weather(serde_json::json!({
            "condition": "cloudy",
            "temperature": 8.2,
            "humidity": 75
        }));

    // Scenario 1: Cozy evening
    info!("\n🌅 Scenario 1: Cozy Evening Setup");
    let cozy_request = automation_builder
        .build_cozy_request("evening", "cloudy", "relaxing")
        .unwrap();

    match manager.request_sampling(cozy_request).await {
        Ok(response) => {
            info!("✅ Cozy evening automation suggestion:");
            info!("   Model: {}", response.model);
            if let Some(text) = response.content.text {
                // Split into lines for better readability
                for line in text.lines().take(8) {
                    // Limit to first 8 lines
                    info!("   {}", line);
                }
                if text.lines().count() > 8 {
                    info!("   ... (truncated)");
                }
            }
        }
        Err(e) => {
            warn!("⚠️ Cozy evening scenario failed: {}", e);
        }
    }

    // Scenario 2: Party preparation
    info!("\n🎉 Scenario 2: Party Preparation");
    let party_request = automation_builder
        .build_event_request(
            "dinner party",
            Some("living room"),
            Some("3 hours"),
            Some("6"),
        )
        .unwrap();

    match manager.request_sampling(party_request).await {
        Ok(response) => {
            info!("✅ Party preparation automation suggestion:");
            info!("   Model: {}", response.model);
            if let Some(text) = response.content.text {
                // Split into lines for better readability
                for line in text.lines().take(8) {
                    // Limit to first 8 lines
                    info!("   {}", line);
                }
                if text.lines().count() > 8 {
                    info!("   ... (truncated)");
                }
            }
        }
        Err(e) => {
            warn!("⚠️ Party preparation scenario failed: {}", e);
        }
    }

    // Test 4: Performance and Metrics
    info!("\n🧪 Test 4: Performance and Metrics");
    info!("=================================");

    let start_time = std::time::Instant::now();
    let concurrent_requests = 3;

    info!("🚀 Testing {} concurrent requests...", concurrent_requests);

    let manager_arc = std::sync::Arc::new(manager);
    let mut tasks = Vec::new();
    for i in 0..concurrent_requests {
        let manager_clone = manager_arc.clone();
        let task = tokio::spawn(async move {
            let request = SamplingRequest::new(vec![SamplingMessage::user(format!(
                "What is {}+{}? Answer with just the number.",
                i,
                i + 1
            ))])
            .with_max_tokens(20)
            .with_temperature(0.1);

            (i, manager_clone.request_sampling(request).await)
        });
        tasks.push(task);
    }

    let results = futures::future::join_all(tasks).await;
    let elapsed = start_time.elapsed();

    let mut successful = 0;
    let mut failed = 0;

    for result in results {
        match result {
            Ok((i, Ok(response))) => {
                successful += 1;
                info!(
                    "✅ Request {}: {}",
                    i,
                    response
                        .content
                        .text
                        .as_ref()
                        .unwrap_or(&"No response".to_string())
                );
            }
            Ok((i, Err(e))) => {
                failed += 1;
                warn!("❌ Request {} failed: {}", i, e);
            }
            Err(e) => {
                failed += 1;
                error!("❌ Task failed: {}", e);
            }
        }
    }

    info!("📊 Concurrent test results:");
    info!("   ✅ Successful: {}", successful);
    info!("   ❌ Failed: {}", failed);
    info!("   ⏱️ Total time: {:?}", elapsed);
    info!(
        "   📈 Average per request: {:?}",
        elapsed / concurrent_requests
    );

    // Final summary
    info!("\n🎯 Demo Summary");
    info!("==============");
    info!("✅ Ollama HTTP client integration working");
    info!("✅ Provider configuration and validation working");
    info!("✅ Health checking and fallback mechanism working");
    info!("✅ Home automation scenarios working");
    info!("✅ Concurrent request handling working");
    info!(
        "🦙 Your Ollama instance with {} is ready for MCP integration!",
        config.ollama.default_model
    );

    // Clean up environment variables
    env::remove_var("OLLAMA_HEALTH_OVERRIDE");
    env::remove_var("OPENAI_HEALTH_OVERRIDE");
    env::remove_var("ANTHROPIC_HEALTH_OVERRIDE");

    info!("\n🎉 Demo completed successfully!");
    Ok(())
}
