//! Test network discovery for Loxone Miniservers
//!
//! Run with: cargo run --example test_discovery --features discovery

use loxone_mcp_rust::network_discovery::NetworkDiscovery;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,loxone_mcp_rust=debug")
        .init();
    
    println!("\n🔍 Testing Loxone network discovery...\n");
    
    // Create discovery instance with 10 second timeout
    let discovery = NetworkDiscovery::new(Duration::from_secs(10));
    
    // Discover servers
    match discovery.discover_servers().await {
        Ok(servers) => {
            if servers.is_empty() {
                println!("❌ No Loxone Miniservers found on your network\n");
                println!("Troubleshooting tips:");
                println!("1. Make sure you're on the same network as the Miniserver");
                println!("2. Check that mDNS/Bonjour is not blocked by firewall");
                println!("3. Ensure the Miniserver is powered on and accessible");
                println!("4. Try accessing it directly if you know the IP");
            } else {
                println!("✅ Found {} Loxone Miniserver(s):\n", servers.len());
                
                for (i, server) in servers.iter().enumerate() {
                    println!("{}. {} ", i + 1, "=".repeat(60));
                    println!("   Name: {}", server.name);
                    println!("   IP: {}", server.ip);
                    println!("   Port: {}", server.port);
                    println!("   Discovery Method: {}", server.method);
                    
                    if let Some(service_type) = &server.service_type {
                        println!("   Service Type: {}", service_type);
                    }
                    
                    if let Some(service_name) = &server.service_name {
                        println!("   Service Name: {}", service_name);
                    }
                    
                    println!("   URL: http://{}:{}", server.ip, server.port);
                }
                
                println!("\n💡 To configure credentials for a server, run:");
                println!("   cargo run --bin loxone-mcp-setup -- --host {}", servers[0].ip);
            }
        }
        Err(e) => {
            eprintln!("❌ Discovery error: {}", e);
        }
    }
    
    Ok(())
}