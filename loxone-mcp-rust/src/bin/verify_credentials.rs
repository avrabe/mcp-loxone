//! Verify credentials for Loxone MCP Rust server
//! 
//! This utility checks if credentials are properly stored and accessible.

use loxone_mcp_rust::{
    config::credentials::create_best_credential_manager,
    Result,
};
use tracing::{info, error};

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("\n🔍 Loxone MCP Credential Verification");
    println!("========================================\n");

    // Use tokio runtime
    tokio::runtime::Runtime::new()?.block_on(async {
        println!("Checking credential backends...\n");
        
        // Create multi-backend credential manager
        let multi_manager = create_best_credential_manager().await?;
        
        // Try to get credentials
        match multi_manager.get_credentials().await {
            Ok(creds) => {
                info!("✅ Credentials found!");
                info!("   Username: {}", creds.username);
                info!("   Password: ***");
                if creds.api_key.is_some() {
                    info!("   API Key: ***");
                }
            }
            Err(e) => {
                error!("❌ Failed to get credentials: {}", e);
                println!("\n⚠️  If you see 'Unable to obtain authorization', please:");
                println!("   1. Check your macOS Keychain Access app");
                println!("   2. Look for 'LoxoneMCP' entries");
                println!("   3. Grant access to 'loxone-mcp-rust' when prompted");
                println!("\nAlternatively, you can use environment variables:");
                println!("   export LOXONE_USERNAME=your_username");
                println!("   export LOXONE_PASSWORD=your_password");
                return Err(e);
            }
        }
        
        println!("\nCredential verification complete using best available backend.");
        
        println!("\n✅ Verification complete!");
        println!("\nYour credentials are accessible to the Rust server.");
        
        Ok(())
    })
}