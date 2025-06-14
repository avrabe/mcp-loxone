//! Demonstration of authentication method selection
//!
//! This example shows how the client factory function chooses between
//! basic HTTP authentication and token-based authentication based on
//! the configuration.

use loxone_mcp_rust::client::create_client;
use loxone_mcp_rust::config::credentials::LoxoneCredentials;
use loxone_mcp_rust::config::{AuthMethod, LoxoneConfig};
use std::time::Duration;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 Authentication Method Selection Demo");
    println!("=====================================\n");

    let credentials = LoxoneCredentials {
        username: "demo_user".to_string(),
        password: "demo_password".to_string(),
        api_key: None,
        #[cfg(feature = "crypto")]
        public_key: None,
    };

    // Demo 1: Token Authentication (default for new installations)
    println!("1️⃣  Creating client with Token Authentication (recommended for Loxone V9+)");
    let config_token = LoxoneConfig {
        url: Url::parse("http://192.168.1.100")?,
        username: "demo_user".to_string(),
        verify_ssl: false,
        timeout: Duration::from_secs(30),
        max_retries: 3,
        max_connections: Some(10),
        #[cfg(feature = "websocket")]
        websocket: Default::default(),
        auth_method: AuthMethod::Token, // Uses RSA + JWT token authentication
    };

    match create_client(&config_token, &credentials).await {
        Ok(_client) => {
            println!("   ✅ Token-based HTTP client created successfully");
            #[cfg(feature = "crypto")]
            println!("   🔒 Uses RSA encryption + JWT tokens for secure authentication");
            #[cfg(not(feature = "crypto"))]
            println!("   ⚠️  Crypto feature disabled - falling back to basic auth");
        }
        Err(e) => println!("   ❌ Error: {}", e),
    }

    // Demo 2: Basic Authentication (legacy for Loxone V8 and older)
    println!("\n2️⃣  Creating client with Basic Authentication (legacy mode)");
    let config_basic = LoxoneConfig {
        auth_method: AuthMethod::Basic, // Uses HTTP Basic Auth
        ..config_token
    };

    match create_client(&config_basic, &credentials).await {
        Ok(_client) => {
            println!("   ✅ Basic authentication HTTP client created successfully");
            println!("   📝 Uses HTTP Basic Auth headers (less secure)");
        }
        Err(e) => println!("   ❌ Error: {}", e),
    }

    // Demo 3: Show default behavior
    println!("\n3️⃣  Default authentication method for new configurations");
    let default_method = AuthMethod::default();
    println!("   🎯 Default: {:?}", default_method);
    println!("   💡 New installations automatically use Token auth for better security");

    println!("\n✨ Features of Token Authentication:");
    println!("   • RSA-2048 public key encryption for credential exchange");
    println!("   • JWT tokens with expiration and refresh capability");
    println!("   • Automatic token refresh before expiration");
    println!("   • Retry logic with re-authentication on 401 errors");
    println!("   • Session key management for AES encryption (future use)");

    println!("\n📚 When to use each method:");
    println!("   • Token Auth: Loxone V9+ (recommended for security)");
    println!("   • Basic Auth: Loxone V8 and older (legacy compatibility)");

    Ok(())
}
