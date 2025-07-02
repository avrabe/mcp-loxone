//! Update credentials for Loxone MCP Rust server
//! Note: Host URL is now part of credential configuration, not stored separately

use loxone_mcp_rust::{config::credentials::create_best_credential_manager, Result};
use tracing::info;

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    println!("\n🔧 Credential Configuration Update");
    println!("========================================\n");

    // Use tokio runtime
    tokio::runtime::Runtime::new()?.block_on(async {
        info!("Creating credential manager with best available backend...");

        let multi_manager = create_best_credential_manager().await?;

        // Verify current credentials
        match multi_manager.get_credentials().await {
            Ok(creds) => {
                info!("✅ Current credentials found!");
                info!("   Username: {}", creds.username);
                info!("   Password: ***");
                if creds.api_key.is_some() {
                    info!("   API Key: ***");
                }
                info!("\n💡 Note: Host URLs are now configured via environment variables or Infisical.");
                info!("   Set LOXONE_HOST environment variable or use Infisical configuration.");
            }
            Err(e) => {
                eprintln!("❌ No credentials found: {e}");
                eprintln!("\n💡 Please run the setup utility first to configure credentials.");
                return Err(e);
            }
        }

        Ok(())
    })
}
