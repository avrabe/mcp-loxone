//! Test script to verify token authentication works

use loxone_mcp_rust::client::auth::TokenAuthClient;
use loxone_mcp_rust::error::Result;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,loxone_mcp_rust=debug")
        .init();

    println!("🔐 Testing Token Authentication with OpenSSL...\n");

    // Load environment from .env file if it exists
    if let Ok(env_file) = std::fs::read_to_string("dont-commit.env") {
        for line in env_file.lines() {
            if line.contains('=') && !line.starts_with('#') {
                let parts: Vec<&str> = line.splitn(2, '=').collect();
                if parts.len() == 2 {
                    env::set_var(parts[0].trim(), parts[1].trim());
                }
            }
        }
    }

    // Read credentials from environment
    let host = env::var("LOXONE_HOST").unwrap_or_else(|_| {
        println!("❌ LOXONE_HOST not set in environment or dont-commit.env file");
        std::process::exit(1);
    });
    let username = env::var("LOXONE_USERNAME")
        .or_else(|_| env::var("LOXONE_USER"))
        .unwrap_or_else(|_| {
            println!(
                "❌ LOXONE_USERNAME/LOXONE_USER not set in environment or dont-commit.env file"
            );
            std::process::exit(1);
        });
    let password = env::var("LOXONE_PASSWORD")
        .or_else(|_| env::var("LOXONE_PASS"))
        .unwrap_or_else(|_| {
            println!(
                "❌ LOXONE_PASSWORD/LOXONE_PASS not set in environment or dont-commit.env file"
            );
            std::process::exit(1);
        });

    println!("Connecting to: http://{}", host);
    println!("Username: {}", username);
    println!("Password: [hidden]\n");

    // Create HTTP client
    let client = reqwest::Client::new();

    // Create token auth client
    let mut auth_client = TokenAuthClient::new(format!("http://{}", host), client);

    println!("Step 1: Authenticating with token-based authentication...");
    match auth_client.authenticate(&username, &password).await {
        Ok(_) => {
            println!("✅ Authentication successful!");

            // Check if we have a valid token
            if auth_client.is_authenticated() {
                println!("✅ Token is valid and not expired");

                // Try to get auth params
                match auth_client.get_auth_params() {
                    Ok(params) => {
                        println!(
                            "✅ Auth params generated: {}",
                            params.chars().take(30).collect::<String>() + "..."
                        );
                    }
                    Err(e) => {
                        println!("❌ Failed to get auth params: {}", e);
                    }
                }

                // Try to make an authenticated request
                println!("\nStep 2: Testing authenticated request...");
                match auth_client.request("cfg/api").await {
                    Ok(response) => {
                        println!("✅ Authenticated request successful!");
                        println!(
                            "Response preview: {}",
                            serde_json::to_string_pretty(&response)
                                .unwrap_or_default()
                                .chars()
                                .take(200)
                                .collect::<String>()
                                + "..."
                        );
                    }
                    Err(e) => {
                        println!("❌ Authenticated request failed: {}", e);
                    }
                }
            } else {
                println!("❌ Token is invalid or expired");
            }
        }
        Err(e) => {
            println!("❌ Authentication failed: {}", e);
            println!("\nThis could be due to:");
            println!("- Incorrect credentials");
            println!("- Network connectivity issues");
            println!("- Server not supporting token authentication");
            println!("- Certificate/SSL issues");
        }
    }

    println!("\n🏁 Token authentication test completed");
    Ok(())
}
