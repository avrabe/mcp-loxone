//! Loxone MCP Authentication and Credential Management
//!
//! This tool manages Loxone Miniserver credentials with secure storage
//! and a credential ID system for easier management.

use clap::{Parser, Subcommand};
use loxone_mcp_rust::{
    client::create_client,
    config::{
        credentials::{create_best_credential_manager, LoxoneCredentials},
        AuthMethod, CredentialStore, LoxoneConfig,
    },
    Result,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, time::Duration};
use tracing::{error, info};
use url::Url;
use uuid::Uuid;

/// Stored credential metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredCredential {
    /// Unique credential ID
    id: String,
    /// Friendly name
    name: String,
    /// Host information
    host: String,
    /// Port
    port: u16,
    /// Creation timestamp
    created_at: chrono::DateTime<chrono::Utc>,
    /// Last used timestamp
    last_used: Option<chrono::DateTime<chrono::Utc>>,
}

/// Credential registry for managing multiple credentials
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CredentialRegistry {
    /// Map of credential ID to metadata
    credentials: HashMap<String, StoredCredential>,
}

impl CredentialRegistry {
    /// Registry file path
    fn registry_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".loxone-mcp")
            .join("credential-registry.json")
    }

    /// Load registry from disk
    fn load() -> Result<Self> {
        let path = Self::registry_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let data = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&data)?)
    }

    /// Save registry to disk
    fn save(&self) -> Result<()> {
        let path = Self::registry_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, data)?;
        Ok(())
    }

    /// Add a credential
    fn add_credential(&mut self, name: String, host: String, port: u16) -> String {
        let id = Uuid::new_v4().to_string();
        let credential = StoredCredential {
            id: id.clone(),
            name,
            host,
            port,
            created_at: chrono::Utc::now(),
            last_used: None,
        };
        self.credentials.insert(id.clone(), credential);
        id
    }

    /// Get credential by ID
    fn get_credential(&self, id: &str) -> Option<&StoredCredential> {
        self.credentials.get(id)
    }

    /// Update last used timestamp
    fn mark_used(&mut self, id: &str) {
        if let Some(cred) = self.credentials.get_mut(id) {
            cred.last_used = Some(chrono::Utc::now());
        }
    }

    /// Remove credential
    fn remove_credential(&mut self, id: &str) -> bool {
        self.credentials.remove(id).is_some()
    }
}

/// Loxone MCP Authentication and Credential Management
#[derive(Parser, Debug)]
#[command(name = "loxone-mcp-auth")]
#[command(about = "Manage Loxone Miniserver credentials securely")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Store new Loxone Miniserver credentials
    Store {
        /// Friendly name for the credentials
        #[arg(short, long)]
        name: String,

        /// Miniserver host (IP or hostname)
        #[arg(short = 'H', long)]
        host: String,

        /// Miniserver port
        #[arg(short, long, default_value = "80")]
        port: u16,

        /// Miniserver username
        #[arg(short, long)]
        username: String,

        /// Miniserver password
        #[arg(short, long)]
        password: String,

        /// Test connection before storing
        #[arg(short, long, default_value = "true")]
        test: bool,

        /// Storage backend to use
        #[arg(long, value_enum)]
        backend: Option<StorageBackend>,
    },

    /// List all stored Loxone credentials
    List {
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },

    /// Show specific credential details
    Show {
        /// Credential ID
        credential_id: String,

        /// Include sensitive information
        #[arg(short, long)]
        include_sensitive: bool,
    },

    /// Update existing credentials
    Update {
        /// Credential ID to update
        credential_id: String,

        /// New name
        #[arg(short, long)]
        name: Option<String>,

        /// New username
        #[arg(short, long)]
        username: Option<String>,

        /// New password
        #[arg(short, long)]
        password: Option<String>,

        /// Test connection after update
        #[arg(short, long, default_value = "true")]
        test: bool,
    },

    /// Delete stored credentials
    Delete {
        /// Credential ID to delete
        credential_id: String,

        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Test connection with stored credentials
    Test {
        /// Credential ID to test
        credential_id: String,

        /// Show detailed connection info
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum StorageBackend {
    /// System keychain (default)
    Keychain,
    /// Environment variables
    Environment,
    /// Infisical secret management
    #[cfg(feature = "infisical")]
    Infisical,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "loxone_mcp_auth=info".to_string()),
        )
        .init();

    let cli = Cli::parse();

    // Load credential registry
    let mut registry = CredentialRegistry::load()?;

    match cli.command {
        Commands::Store {
            name,
            host,
            port,
            username,
            password,
            test,
            backend,
        } => {
            info!("📝 Storing Loxone credentials for: {}", name);

            // Test connection if requested
            if test {
                info!("🔌 Testing connection to {}:{}", host, port);
                test_connection(&host, port, &username, &password).await?;
                info!("✅ Connection test successful!");
            }

            // Determine storage backend
            let store = match backend {
                Some(StorageBackend::Environment) => CredentialStore::Environment,
                #[cfg(feature = "infisical")]
                Some(StorageBackend::Infisical) => {
                    // Check if Infisical is configured
                    match (
                        std::env::var("INFISICAL_PROJECT_ID"),
                        std::env::var("INFISICAL_CLIENT_ID"),
                        std::env::var("INFISICAL_CLIENT_SECRET"),
                    ) {
                        (Ok(project_id), Ok(client_id), Ok(client_secret)) => {
                            CredentialStore::Infisical {
                                project_id,
                                environment: std::env::var("INFISICAL_ENVIRONMENT")
                                    .unwrap_or_else(|_| "dev".to_string()),
                                client_id,
                                client_secret,
                                host: std::env::var("INFISICAL_HOST").ok(),
                            }
                        }
                        _ => {
                            error!("❌ Infisical not configured. Set INFISICAL_PROJECT_ID, INFISICAL_CLIENT_ID, and INFISICAL_CLIENT_SECRET");
                            return Ok(());
                        }
                    }
                }
                _ => {
                    // Default to keychain or best available
                    #[cfg(feature = "keyring-storage")]
                    {
                        CredentialStore::Keyring
                    }
                    #[cfg(not(feature = "keyring-storage"))]
                    {
                        CredentialStore::Environment
                    }
                }
            };

            // Create credential manager
            let manager =
                loxone_mcp_rust::config::credentials::CredentialManager::new_async(store.clone())
                    .await?;

            // Store host in registry
            std::env::set_var("LOXONE_HOST", format!("{host}:{port}"));

            // Store credentials
            let credentials = LoxoneCredentials {
                username: username.clone(),
                password,
                api_key: None,
                #[cfg(feature = "crypto-openssl")]
                public_key: None,
            };

            manager.store_credentials(&credentials).await?;

            // Add to registry
            let credential_id = registry.add_credential(name, host.clone(), port);
            registry.save()?;

            info!("✅ Credentials stored successfully!");
            info!("📋 Credential ID: {}", credential_id);
            info!("\n🚀 To use these credentials:");
            info!(
                "   cargo run --bin loxone-mcp-server stdio --credential-id {}",
                credential_id
            );

            if matches!(store, CredentialStore::Environment) {
                info!("\n⚠️  Environment variables need to be set:");
                info!("   export LOXONE_USER=\"{}\"", username);
                info!("   export LOXONE_PASS=\"****\"");
                info!("   export LOXONE_HOST=\"{}:{}\"", host, port);
            }
        }

        Commands::List { detailed } => {
            info!("📋 Listing stored Loxone credentials...\n");

            if registry.credentials.is_empty() {
                info!("No credentials found. Use 'store' command to add credentials.");
                return Ok(());
            }

            let mut credentials: Vec<_> = registry.credentials.values().collect();
            credentials.sort_by(|a, b| a.created_at.cmp(&b.created_at));

            for (idx, cred) in credentials.iter().enumerate() {
                if detailed {
                    println!("{}. {} (ID: {})", idx + 1, cred.name, cred.id);
                    println!("   Host: {}:{}", cred.host, cred.port);
                    println!(
                        "   Created: {}",
                        cred.created_at.format("%Y-%m-%d %H:%M:%S")
                    );
                    if let Some(last_used) = &cred.last_used {
                        println!("   Last Used: {}", last_used.format("%Y-%m-%d %H:%M:%S"));
                    }
                    println!();
                } else {
                    println!(
                        "{}. {} - {}:{} ({})",
                        idx + 1,
                        cred.name,
                        cred.host,
                        cred.port,
                        &cred.id[..8]
                    );
                }
            }
        }

        Commands::Show {
            credential_id,
            include_sensitive,
        } => {
            info!("🔍 Showing credential: {}", credential_id);

            let stored = registry.get_credential(&credential_id).ok_or_else(|| {
                loxone_mcp_rust::error::LoxoneError::config("Credential not found")
            })?;

            // Load actual credentials
            let manager = create_best_credential_manager().await?;

            // Set host for retrieval
            std::env::set_var("LOXONE_HOST", format!("{}:{}", stored.host, stored.port));

            let credentials = manager.get_credentials().await?;

            println!("\n📋 Credential Details:");
            println!("   ID: {}", stored.id);
            println!("   Name: {}", stored.name);
            println!("   Host: {}:{}", stored.host, stored.port);
            println!("   Username: {}", credentials.username);
            if include_sensitive {
                println!("   Password: {}", credentials.password);
            } else {
                println!("   Password: {}", "*".repeat(8));
            }
            println!(
                "   Created: {}",
                stored.created_at.format("%Y-%m-%d %H:%M:%S")
            );
            if let Some(last_used) = &stored.last_used {
                println!("   Last Used: {}", last_used.format("%Y-%m-%d %H:%M:%S"));
            }
        }

        Commands::Update {
            credential_id,
            name,
            username,
            password,
            test,
        } => {
            info!("🔄 Updating credential: {}", credential_id);

            let stored = registry
                .get_credential(&credential_id)
                .ok_or_else(|| loxone_mcp_rust::error::LoxoneError::config("Credential not found"))?
                .clone();

            // Load existing credentials
            let manager = create_best_credential_manager().await?;
            std::env::set_var("LOXONE_HOST", format!("{}:{}", stored.host, stored.port));
            let mut credentials = manager.get_credentials().await?;

            // Update fields if provided
            if let Some(new_name) = &name {
                if let Some(cred) = registry.credentials.get_mut(&credential_id) {
                    cred.name = new_name.clone();
                }
            }
            if let Some(new_username) = username {
                credentials.username = new_username;
            }
            if let Some(new_password) = password {
                credentials.password = new_password;
            }

            // Test connection if requested
            if test {
                info!("🔌 Testing updated credentials...");
                test_connection(
                    &stored.host,
                    stored.port,
                    &credentials.username,
                    &credentials.password,
                )
                .await?;
                info!("✅ Connection test successful!");
            }

            // Store updated credentials
            manager.store_credentials(&credentials).await?;
            registry.save()?;

            info!("✅ Credential updated successfully!");
        }

        Commands::Delete {
            credential_id,
            force,
        } => {
            if !force {
                println!("⚠️  Are you sure you want to delete credential '{credential_id}'?");
                println!("   This action cannot be undone.");
                print!("   Type 'yes' to confirm: ");
                use std::io::{self, Write};
                io::stdout().flush().unwrap();

                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();

                if input.trim().to_lowercase() != "yes" {
                    info!("❌ Deletion cancelled");
                    return Ok(());
                }
            }

            info!("🗑️  Deleting credential: {}", credential_id);

            if registry.remove_credential(&credential_id) {
                registry.save()?;
                info!("✅ Credential deleted successfully!");
                info!("   Note: The actual credentials may still be stored in the backend");
                info!("   (keychain/environment/infisical) and need to be manually removed.");
            } else {
                error!("❌ Credential not found");
            }
        }

        Commands::Test {
            credential_id,
            verbose,
        } => {
            info!("🔌 Testing credential: {}", credential_id);

            let stored = registry.get_credential(&credential_id).ok_or_else(|| {
                loxone_mcp_rust::error::LoxoneError::config("Credential not found")
            })?;

            // Load credentials
            let manager = create_best_credential_manager().await?;
            std::env::set_var("LOXONE_HOST", format!("{}:{}", stored.host, stored.port));
            let credentials = manager.get_credentials().await?;

            // Test connection
            test_connection_verbose(
                &stored.host,
                stored.port,
                &credentials.username,
                &credentials.password,
                verbose,
            )
            .await?;

            // Update last used
            registry.mark_used(&credential_id);
            registry.save()?;

            info!("✅ Connection test successful!");
        }
    }

    Ok(())
}

/// Test connection to Loxone Miniserver
async fn test_connection(host: &str, port: u16, username: &str, password: &str) -> Result<()> {
    let url = format!("http://{host}:{port}");
    let config = LoxoneConfig {
        url: Url::parse(&url).map_err(|e| {
            loxone_mcp_rust::error::LoxoneError::config(format!("Invalid URL: {e}"))
        })?,
        username: username.to_string(),
        timeout: Duration::from_secs(10),
        max_retries: 1,
        verify_ssl: false,
        max_connections: Some(1),
        #[cfg(feature = "websocket")]
        websocket: Default::default(),
        auth_method: AuthMethod::Basic,
    };

    let credentials = LoxoneCredentials {
        username: username.to_string(),
        password: password.to_string(),
        api_key: None,
        #[cfg(feature = "crypto-openssl")]
        public_key: None,
    };

    // Try to create client and get structure
    let mut client = create_client(&config, &credentials).await?;
    client.connect().await?;
    let _structure = client.get_structure().await?;

    Ok(())
}

/// Test connection with verbose output
async fn test_connection_verbose(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    verbose: bool,
) -> Result<()> {
    if verbose {
        info!("🔗 Connection Details:");
        info!("   Host: {}:{}", host, port);
        info!("   Username: {}", username);
        info!("   Auth Method: Basic");
    }

    let start = std::time::Instant::now();
    test_connection(host, port, username, password).await?;
    let duration = start.elapsed();

    if verbose {
        info!("⏱️  Connection established in {:?}", duration);

        // Get additional info
        let url = format!("http://{host}:{port}");
        let config = LoxoneConfig {
            url: Url::parse(&url).unwrap(),
            username: username.to_string(),
            timeout: Duration::from_secs(10),
            max_retries: 1,
            verify_ssl: false,
            max_connections: Some(1),
            #[cfg(feature = "websocket")]
            websocket: Default::default(),
            auth_method: AuthMethod::Basic,
        };

        let credentials = LoxoneCredentials {
            username: username.to_string(),
            password: password.to_string(),
            api_key: None,
            #[cfg(feature = "crypto-openssl")]
            public_key: None,
        };

        let mut client = create_client(&config, &credentials).await?;
        client.connect().await?;
        let structure = client.get_structure().await?;

        info!("📊 Miniserver Info:");
        info!("   Last Modified: {}", structure.last_modified);
        info!("   Devices: {}", structure.controls.len());
        info!("   Rooms: {}", structure.rooms.len());
        info!("   Categories: {}", structure.cats.len());
    }

    Ok(())
}
