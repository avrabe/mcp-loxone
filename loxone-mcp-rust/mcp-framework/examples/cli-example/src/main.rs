//! Example MCP server using the CLI framework
//!
//! This example demonstrates how to use the MCP CLI framework to create
//! a server with automatic CLI generation, configuration management,
//! and logging setup.

use clap::Parser;
use pulseengine_mcp_cli::{DefaultLoggingConfig, McpConfig, McpConfiguration};
use pulseengine_mcp_protocol::ServerInfo;

/// Example server configuration
#[derive(Debug, Clone, Parser, McpConfig)]
#[command(name = "example-server")]
#[command(about = "An example MCP server demonstrating the CLI framework")]
struct ExampleConfig {
    /// Server port
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// Database URL
    #[arg(short, long)]
    database_url: Option<String>,

    /// Enable debug mode
    #[arg(long)]
    debug: bool,

    /// Server information (auto-populated from Cargo.toml)
    #[mcp(auto_populate)]
    #[clap(skip)]
    server_info: Option<ServerInfo>,

    /// Logging configuration
    #[mcp(logging)]
    #[clap(skip)]
    logging: Option<DefaultLoggingConfig>,
}

impl Default for ExampleConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            database_url: None,
            debug: false,
            server_info: Some(pulseengine_mcp_cli::config::create_server_info(None, None)),
            logging: Some(DefaultLoggingConfig::default()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let config = ExampleConfig::parse();

    // Initialize logging
    config.initialize_logging()?;

    // Validate configuration
    config.validate()?;

    // Print configuration info
    tracing::info!("Starting example MCP server");
    tracing::info!("Server info: {:?}", config.get_server_info());
    tracing::info!("Port: {}", config.port);

    if let Some(db_url) = &config.database_url {
        tracing::info!("Database URL: {}", db_url);
    }

    if config.debug {
        tracing::info!("Debug mode enabled");
    }

    // For this example, we'll just simulate running the server
    tracing::info!("Server would start here in a real implementation");
    tracing::info!("Press Ctrl+C to stop");

    // In a real implementation, you would call:
    // run_server(config).await?;

    // For now, just wait indefinitely
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down gracefully");

    Ok(())
}
