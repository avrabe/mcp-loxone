//! Test direct connection to Loxone using stored credentials

use loxone_mcp_rust::{
    client::{http_client::LoxoneHttpClient, LoxoneClient},
    config::{
        credentials::{create_best_credential_manager, create_credentials},
        LoxoneConfig,
    },
    Result,
};
use std::env;
use tracing::{debug, error, info};

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("debug").init();

    println!("\n🧪 Testing Loxone Connection");
    println!("========================================\n");

    // Use tokio runtime
    tokio::runtime::Runtime::new()?.block_on(async {
        // Try to get credentials using best available backend
        let multi_manager = create_best_credential_manager().await?;

        let (username, password, host) = match multi_manager.get_credentials().await {
            Ok(creds) => {
                info!("✅ Using credentials from configured backend");
                let host =
                    env::var("LOXONE_HOST").unwrap_or_else(|_| "http://192.168.178.10".to_string());
                (creds.username, creds.password, host)
            }
            Err(e) => {
                error!("❌ No credentials found in any backend: {}", e);
                error!("   Please run: cargo run --bin loxone-mcp-setup");
                error!("   Or set: LOXONE_USER, LOXONE_PASS, LOXONE_HOST");
                return Err(e);
            }
        };

        println!("🔗 Testing connection to:");
        println!("   Host: {host}");
        println!("   User: {username}");
        println!("   Pass: ***");
        println!();

        // Parse host URL
        let url = host.parse().map_err(|e| {
            loxone_mcp_rust::error::LoxoneError::config(format!("Invalid URL: {e}"))
        })?;

        // Create config
        let config = LoxoneConfig {
            url,
            username: username.clone(),
            timeout: std::time::Duration::from_secs(10),
            max_retries: 1,
            verify_ssl: false,
            max_connections: Some(10),
            #[cfg(feature = "websocket")]
            websocket: Default::default(),
            auth_method: Default::default(),
        };

        // Create credentials
        let credentials = create_credentials(username, password);

        info!("🔌 Creating HTTP client...");
        let mut client = LoxoneHttpClient::new(config, credentials).await?;

        info!("🚀 Testing connection...");
        match client.connect().await {
            Ok(_) => {
                info!("✅ Successfully connected to Loxone!");

                // Test health check
                info!("❤️  Testing health check...");
                match client.health_check().await {
                    Ok(healthy) => {
                        if healthy {
                            info!("✅ System is healthy!");
                        } else {
                            info!("⚠️  System may have issues");
                        }
                    }
                    Err(e) => {
                        error!("❌ Health check failed: {}", e);
                    }
                }

                // Test system info
                info!("ℹ️  Getting system info...");
                match client.get_system_info().await {
                    Ok(info) => {
                        info!(
                            "✅ System info: {}",
                            serde_json::to_string_pretty(&info).unwrap_or_default()
                        );
                    }
                    Err(e) => {
                        error!("❌ Failed to get system info: {}", e);
                    }
                }

                // Test structure
                info!("📊 Getting structure data...");
                match client.get_structure().await {
                    Ok(structure) => {
                        info!("✅ Structure data retrieved successfully!");
                        info!("   Controls: {}", structure.controls.len());
                        info!("   Rooms: {}", structure.rooms.len());

                        // Show first few rooms
                        for (i, (uuid, room)) in structure.rooms.iter().enumerate() {
                            if i >= 3 {
                                break;
                            }
                            let room_name = room
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("Unknown");
                            info!("   Room: {} ({})", room_name, uuid);
                        }
                    }
                    Err(e) => {
                        error!("❌ Failed to get structure: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("❌ Connection failed: {}", e);

                // Additional debugging
                debug!("Debug: Checking base URL accessibility...");
                let test_url = format!("{host}/favicon.ico");
                match reqwest::get(&test_url).await {
                    Ok(response) => {
                        debug!("Test GET {}: {}", test_url, response.status());
                    }
                    Err(e) => {
                        debug!("Test GET failed: {}", e);
                    }
                }

                error!("💡 Check that:");
                error!("   - The Loxone Miniserver is accessible at {}", host);
                error!("   - Your credentials are correct");
                error!("   - The server is not in maintenance mode");
                error!("   - Port 80 is open and reachable");
            }
        }

        Ok(())
    })
}
