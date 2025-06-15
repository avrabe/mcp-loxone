//! API Key Management CLI Tool

use chrono::{Duration, Utc};
use clap::{Parser, Subcommand};
use loxone_mcp_rust::security::key_store::{
    default_key_store_path, ApiKey, ApiKeyRole, KeyStore, KeyStoreBackend, KeyStoreConfig,
};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(author, version, about = "Loxone MCP API Key Management")]
struct Cli {
    /// Key store file path
    #[arg(short, long, default_value_t = default_key_store_path().display().to_string())]
    store: String,

    /// Storage backend
    #[arg(short, long, default_value = "file")]
    backend: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new API key
    Generate {
        /// Role (admin, operator, monitor, device)
        #[arg(short, long, default_value = "operator")]
        role: String,

        /// Human-readable name
        #[arg(short, long)]
        name: String,

        /// Expiration in days (0 = never)
        #[arg(short, long, default_value = "365")]
        expires: u32,

        /// IP whitelist (comma-separated)
        #[arg(short, long)]
        ip: Option<String>,

        /// Allowed devices for device role (comma-separated)
        #[arg(short, long)]
        devices: Option<String>,
    },

    /// List all API keys
    List {
        /// Show only active keys
        #[arg(short, long)]
        active: bool,

        /// Output format (table, json, toml)
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// Show details of a specific key
    Show {
        /// Key ID
        key_id: String,
    },

    /// Revoke an API key
    Revoke {
        /// Key ID
        key_id: String,
    },

    /// Activate a revoked key
    Activate {
        /// Key ID
        key_id: String,
    },

    /// Update key properties
    Update {
        /// Key ID
        key_id: String,

        /// New name
        #[arg(short, long)]
        name: Option<String>,

        /// New expiration in days from now
        #[arg(short, long)]
        expires: Option<u32>,

        /// New IP whitelist (comma-separated)
        #[arg(short, long)]
        ip: Option<String>,
    },

    /// Export keys to different format
    Export {
        /// Output format (json, toml, env)
        #[arg(short, long, default_value = "json")]
        format: String,

        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Import keys from file
    Import {
        /// Input file
        file: String,

        /// Skip existing keys
        #[arg(short, long)]
        skip_existing: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    // Create key store
    let backend = match cli.backend.as_str() {
        "file" => KeyStoreBackend::File,
        "env" => KeyStoreBackend::Environment,
        "memory" => KeyStoreBackend::Memory,
        "sqlite" => KeyStoreBackend::Sqlite,
        _ => {
            eprintln!("Invalid backend: {}", cli.backend);
            std::process::exit(1);
        }
    };

    let config = KeyStoreConfig {
        backend,
        file_path: Some(PathBuf::from(&cli.store)),
        auto_save: true,
        encrypt_at_rest: false,
    };

    let store = KeyStore::new(config).await?;

    match cli.command {
        Commands::Generate {
            role,
            name,
            expires,
            ip,
            devices,
        } => {
            let role = parse_role(&role, devices)?;
            let key_id = generate_key_id(&role);

            let key = ApiKey {
                id: key_id.clone(),
                name,
                role,
                created_by: std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()),
                created_at: Utc::now(),
                expires_at: if expires > 0 {
                    Some(Utc::now() + Duration::days(expires as i64))
                } else {
                    None
                },
                ip_whitelist: ip
                    .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_default(),
                active: true,
                last_used: None,
                usage_count: 0,
                metadata: Default::default(),
            };

            store.add_key(key).await?;

            println!("✅ Generated new API key:");
            println!("   ID: {}", key_id);
            println!("   Add to your configuration:");
            println!("   export LOXONE_API_KEY={}", key_id);
        }

        Commands::List { active, format } => {
            let keys = store.list_keys().await;
            let keys: Vec<_> = if active {
                keys.into_iter().filter(|k| k.active).collect()
            } else {
                keys
            };

            match format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&keys)?),
                "toml" => println!("{}", toml::to_string_pretty(&keys)?),
                _ => {
                    // Table format
                    println!(
                        "{:<40} {:<15} {:<20} {:<10} {:<20}",
                        "ID", "Role", "Name", "Active", "Expires"
                    );
                    println!("{}", "-".repeat(105));

                    for key in keys {
                        let expires = key
                            .expires_at
                            .map(|e| e.format("%Y-%m-%d").to_string())
                            .unwrap_or_else(|| "Never".to_string());

                        println!(
                            "{:<40} {:<15} {:<20} {:<10} {:<20}",
                            key.id,
                            format!("{:?}", key.role).split(' ').next().unwrap(),
                            truncate(&key.name, 20),
                            if key.active { "Yes" } else { "No" },
                            expires
                        );
                    }
                }
            }
        }

        Commands::Show { key_id } => match store.get_key(&key_id).await {
            Some(key) => {
                println!("API Key Details:");
                println!("  ID: {}", key.id);
                println!("  Name: {}", key.name);
                println!("  Role: {:?}", key.role);
                println!("  Created by: {}", key.created_by);
                println!(
                    "  Created at: {}",
                    key.created_at.format("%Y-%m-%d %H:%M:%S")
                );
                println!(
                    "  Expires: {}",
                    key.expires_at
                        .map(|e| e.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "Never".to_string())
                );
                println!("  Active: {}", key.active);
                println!(
                    "  IP Whitelist: {}",
                    if key.ip_whitelist.is_empty() {
                        "None (all allowed)".to_string()
                    } else {
                        key.ip_whitelist.join(", ")
                    }
                );
                println!(
                    "  Last used: {}",
                    key.last_used
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "Never".to_string())
                );
                println!("  Usage count: {}", key.usage_count);
            }
            None => {
                eprintln!("❌ Key not found: {}", key_id);
                std::process::exit(1);
            }
        },

        Commands::Revoke { key_id } => match store.get_key(&key_id).await {
            Some(mut key) => {
                key.active = false;
                store.update_key(key).await?;
                println!("✅ Revoked key: {}", key_id);
            }
            None => {
                eprintln!("❌ Key not found: {}", key_id);
                std::process::exit(1);
            }
        },

        Commands::Activate { key_id } => match store.get_key(&key_id).await {
            Some(mut key) => {
                key.active = true;
                store.update_key(key).await?;
                println!("✅ Activated key: {}", key_id);
            }
            None => {
                eprintln!("❌ Key not found: {}", key_id);
                std::process::exit(1);
            }
        },

        Commands::Update {
            key_id,
            name,
            expires,
            ip,
        } => match store.get_key(&key_id).await {
            Some(mut key) => {
                if let Some(name) = name {
                    key.name = name;
                }
                if let Some(expires) = expires {
                    key.expires_at = if expires > 0 {
                        Some(Utc::now() + Duration::days(expires as i64))
                    } else {
                        None
                    };
                }
                if let Some(ip) = ip {
                    key.ip_whitelist = ip.split(',').map(|s| s.trim().to_string()).collect();
                }

                store.update_key(key).await?;
                println!("✅ Updated key: {}", key_id);
            }
            None => {
                eprintln!("❌ Key not found: {}", key_id);
                std::process::exit(1);
            }
        },

        Commands::Export { format, output } => {
            let keys = store.list_keys().await;

            let content = match format.as_str() {
                "json" => serde_json::to_string_pretty(&keys)?,
                "toml" => toml::to_string_pretty(&keys)?,
                "env" => {
                    // Export as environment variable
                    serde_json::to_string(&keys)?
                }
                _ => {
                    eprintln!("❌ Unknown format: {}", format);
                    std::process::exit(1);
                }
            };

            if let Some(output) = output {
                std::fs::write(&output, content)?;
                println!("✅ Exported {} keys to {}", keys.len(), output);
            } else if format == "env" {
                println!("export LOXONE_API_KEYS='{}'", content);
            } else {
                println!("{}", content);
            }
        }

        Commands::Import {
            file,
            skip_existing,
        } => {
            let content = std::fs::read_to_string(&file)?;
            let keys: Vec<ApiKey> = if file.ends_with(".toml") {
                toml::from_str(&content)?
            } else {
                serde_json::from_str(&content)?
            };

            let mut imported = 0;
            let mut skipped = 0;

            for key in keys {
                if skip_existing && store.get_key(&key.id).await.is_some() {
                    skipped += 1;
                    continue;
                }

                match store.add_key(key).await {
                    Ok(_) => imported += 1,
                    Err(e) => eprintln!("⚠️  Failed to import key: {}", e),
                }
            }

            println!("✅ Imported {} keys, skipped {}", imported, skipped);
        }
    }

    Ok(())
}

fn parse_role(role_str: &str, devices: Option<String>) -> Result<ApiKeyRole, String> {
    match role_str.to_lowercase().as_str() {
        "admin" => Ok(ApiKeyRole::Admin),
        "operator" => Ok(ApiKeyRole::Operator),
        "monitor" => Ok(ApiKeyRole::Monitor),
        "device" => {
            let allowed_devices = devices
                .ok_or("Device role requires --devices parameter")?
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            Ok(ApiKeyRole::Device { allowed_devices })
        }
        _ => Err(format!("Unknown role: {}", role_str)),
    }
}

fn generate_key_id(role: &ApiKeyRole) -> String {
    let role_prefix = match role {
        ApiKeyRole::Admin => "admin",
        ApiKeyRole::Operator => "operator",
        ApiKeyRole::Monitor => "monitor",
        ApiKeyRole::Device { .. } => "device",
        ApiKeyRole::Custom { .. } => "custom",
    };

    let seq = chrono::Utc::now().timestamp_millis() % 1000;
    let random = &Uuid::new_v4().to_string()[..12];

    format!("lmcp_{}_{:03}_{}", role_prefix, seq, random)
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
