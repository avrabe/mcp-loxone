//! Test different Loxone endpoints to find what works

use reqwest::Client;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("\n🔍 Testing Loxone Endpoints");
    println!("========================================\n");

    let host = env::var("LOXONE_HOST").unwrap_or_else(|_| "http://192.168.178.10".to_string());
    let _username = "Ralf"; // From keychain
    let _password = "test"; // Will get 401 but that's OK, we want to see if endpoint exists

    println!("🔗 Testing endpoints on: {}", host);
    println!();

    // Create HTTP client with basic auth
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    // List of endpoints to test
    let endpoints = vec![
        "/",
        "/data/LoxAPP3.json",
        "/jdev/sys/getversion",
        "/jdev/sys/getkey", 
        "/jdev/sys/getkey2",
        "/jdev/cfg/api",
        "/dev/sys/getversion",
        "/sys/getversion",
        "/api/version",
        "/version",
        "/status",
        "/info",
        "/dev/cfg/api",
    ];

    for endpoint in endpoints {
        let url = format!("{}{}", host, endpoint);
        
        print!("Testing {:<25} ... ", endpoint);
        
        match client.get(&url).send().await {
            Ok(response) => {
                let status = response.status();
                let content_type = response.headers()
                    .get("content-type")
                    .and_then(|ct| ct.to_str().ok())
                    .unwrap_or("unknown")
                    .to_string();
                
                let status_code = status.as_u16();
                let success = status.is_success();
                
                let body_preview = if success {
                    match response.text().await {
                        Ok(body) => {
                            if body.len() > 100 {
                                format!("{}...", &body[..100])
                            } else {
                                body
                            }
                        }
                        Err(_) => "failed to read".to_string()
                    }
                } else {
                    String::new()
                };
                
                match status_code {
                    200 => println!("✅ {} ({})", status, content_type),
                    401 => println!("🔐 {} - Auth required (endpoint exists)", status),
                    404 => println!("❌ {} - Not found", status),
                    _ => println!("⚠️  {} - {}", status, status.canonical_reason().unwrap_or("Unknown")),
                }
                
                // Show body preview for successful responses
                if success && !body_preview.is_empty() {
                    println!("     Preview: {}", body_preview.replace('\n', " "));
                }
            }
            Err(e) => {
                println!("❌ Connection error: {}", e);
            }
        }
    }

    println!("\n📋 Summary:");
    println!("  ✅ = Working endpoint");
    println!("  🔐 = Requires authentication (but endpoint exists)");
    println!("  ❌ = Not found");
    println!("  ⚠️  = Other HTTP status");

    Ok(())
}