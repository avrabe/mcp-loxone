//! Unified API Key Management CLI
//!
//! This replaces the previous fragmented authentication tools with a single,
//! comprehensive CLI for managing API keys in the Loxone MCP server.

use clap::{Parser, Subcommand, ValueEnum};
use loxone_mcp_rust::auth::{
    manager::{AuthenticationManager, AuthManagerConfig},
    models::Role,
    storage::StorageBackendConfig,
};
use std::path::PathBuf;

#[derive(Debug, Clone, ValueEnum)]
enum RoleArg {
    Admin,
    Operator,
    Monitor,
    Device,
    Custom,
}

impl From<RoleArg> for Role {
    fn from(role: RoleArg) -> Self {
        match role {
            RoleArg::Admin => Role::Admin,
            RoleArg::Operator => Role::Operator,
            RoleArg::Monitor => Role::Monitor,
            RoleArg::Device => Role::Device {
                allowed_devices: Vec::new(), // Will be set via --devices parameter
            },
            RoleArg::Custom => Role::Custom {
                permissions: Vec::new(), // Will be set via --permissions parameter
            },
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum StorageType {
    File,
    Environment,
    Memory,
}

#[derive(Parser)]
#[command(
    name = "loxone-mcp-auth",
    about = "Unified API Key Management for Loxone MCP Server",
    version,
    author
)]
struct Cli {
    /// Storage type (file, environment, memory)
    #[arg(short, long, value_enum, default_value = "file")]
    storage: StorageType,

    /// Storage file path (for file storage)
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Environment variable name (for environment storage)
    #[arg(short, long, default_value = "LOXONE_API_KEYS")]
    env_var: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new API key
    Create {
        /// Human-readable name for the key
        #[arg(short, long)]
        name: String,

        /// Role for the API key
        #[arg(short, long, value_enum, default_value = "operator")]
        role: RoleArg,

        /// Expiration in days (0 = never expires)
        #[arg(short, long, default_value = "365")]
        expires: u32,

        /// Allowed device UUIDs (for device role, comma-separated)
        #[arg(short, long)]
        devices: Option<String>,

        /// Custom permissions (for custom role, comma-separated)
        #[arg(short, long)]
        permissions: Option<String>,

        /// IP whitelist (comma-separated, empty = all IPs allowed)
        #[arg(short, long)]
        ip_whitelist: Option<String>,
    },

    /// List all API keys
    List {
        /// Show only active keys
        #[arg(short, long)]
        active: bool,

        /// Output format (table, json)
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// Show details of a specific API key
    Show {
        /// API key ID
        key_id: String,
    },

    /// Update an existing API key
    Update {
        /// API key ID
        key_id: String,

        /// New name
        #[arg(short, long)]
        name: Option<String>,

        /// Activate or deactivate the key
        #[arg(short, long)]
        active: Option<bool>,

        /// New expiration in days from now (0 = never expires)
        #[arg(short, long)]
        expires: Option<u32>,

        /// New IP whitelist (comma-separated)
        #[arg(short, long)]
        ip_whitelist: Option<String>,
    },

    /// Delete an API key
    Delete {
        /// API key ID
        key_id: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },

    /// Test an API key
    Test {
        /// API key secret to test
        secret: String,

        /// Client IP to test from
        #[arg(short, long, default_value = "127.0.0.1")]
        ip: String,
    },

    /// Show authentication statistics
    Stats,

    /// Show recent audit events
    Audit {
        /// Number of events to show
        #[arg(short, long, default_value = "50")]
        limit: usize,
    },


    /// Initialize authentication system
    Init {
        /// Create an initial admin key
        #[arg(short, long)]
        admin_key: bool,
    },

    /// Validate SSH-style security for credential files
    Security {
        /// Check security and exit
        #[arg(short, long)]
        check_only: bool,

        /// Automatically fix insecure permissions
        #[arg(short, long)]
        auto_fix: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(log_level.parse().unwrap())
        )
        .init();

    // Create storage configuration
    let storage_config = match cli.storage {
        StorageType::File => {
            let path = cli.file.unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".loxone-mcp")
                    .join("credentials.json")
            });
            StorageBackendConfig::File { path }
        }
        StorageType::Environment => {
            StorageBackendConfig::Environment {
                var_name: cli.env_var,
            }
        }
        StorageType::Memory => StorageBackendConfig::Memory,
    };

    // Create authentication manager
    let config = AuthManagerConfig {
        storage_config,
        validation_config: Default::default(),
        cache_refresh_interval_minutes: 60,
        enable_cache_warming: false, // Don't need background tasks for CLI
    };

    let auth_manager = AuthenticationManager::with_config(config).await?;

    // Execute command
    match cli.command {
        Commands::Create {
            name,
            role,
            expires,
            devices,
            permissions,
            ip_whitelist,
        } => {
            let mut api_role: Role = role.into();

            // Handle device role
            if let Role::Device { ref mut allowed_devices } = api_role {
                if let Some(devices_str) = devices {
                    *allowed_devices = devices_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect();
                } else {
                    eprintln!("Error: Device role requires --devices parameter");
                    std::process::exit(1);
                }
            }

            // Handle custom role
            if let Role::Custom { permissions: ref mut role_permissions } = api_role {
                if let Some(perms_str) = permissions {
                    *role_permissions = perms_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect();
                } else {
                    eprintln!("Error: Custom role requires --permissions parameter");
                    std::process::exit(1);
                }
            }

            let expires_days = if expires == 0 { None } else { Some(expires) };

            let mut key = auth_manager
                .create_key(name, api_role, "cli".to_string(), expires_days)
                .await?;

            // Set IP whitelist if provided
            if let Some(ip_list) = ip_whitelist {
                key.ip_whitelist = ip_list
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                auth_manager.update_key(key.clone()).await?;
            }

            println!("✅ API Key created successfully!");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("ID:     {}", key.id);
            println!("Secret: {}", key.secret);
            println!("Role:   {:?}", key.role);
            println!("Name:   {}", key.name);
            if let Some(expires_at) = key.expires_at {
                println!("Expires: {}", expires_at.format("%Y-%m-%d %H:%M:%S UTC"));
            } else {
                println!("Expires: Never");
            }
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("\n⚠️  SAVE THE SECRET - IT CANNOT BE RETRIEVED LATER!");
            println!("\n📝 Usage examples:");
            println!("  curl -H \"Authorization: Bearer {}\" http://localhost:3001/health", key.secret);
            println!("  curl \"http://localhost:3001/dashboard?api_key={}\"", key.secret);
            println!("\n🔧 Environment variable:");
            println!("  export LOXONE_API_KEY={}", key.secret);
        }

        Commands::List { active, format } => {
            let keys = auth_manager.list_keys().await;
            let filtered_keys: Vec<_> = if active {
                keys.into_iter().filter(|k| k.is_valid()).collect()
            } else {
                keys
            };

            match format.as_str() {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&filtered_keys)?);
                }
                _ => {
                    // Table format
                    if filtered_keys.is_empty() {
                        println!("No API keys found.");
                        return Ok(());
                    }

                    println!(
                        "{:<40} {:<20} {:<10} {:<8} {:<20} {:<12}",
                        "ID", "Name", "Role", "Active", "Expires", "Usage"
                    );
                    println!("{}", "─".repeat(110));

                    for key in filtered_keys {
                        let expires = key
                            .expires_at
                            .map(|e| e.format("%Y-%m-%d").to_string())
                            .unwrap_or_else(|| "Never".to_string());

                        let role_str = match key.role {
                            Role::Admin => "Admin".to_string(),
                            Role::Operator => "Operator".to_string(),
                            Role::Monitor => "Monitor".to_string(),
                            Role::Device { ref allowed_devices } => {
                                format!("Device({})", allowed_devices.len())
                            }
                            Role::Custom { ref permissions } => {
                                format!("Custom({})", permissions.len())
                            }
                        };

                        println!(
                            "{:<40} {:<20} {:<10} {:<8} {:<20} {:<12}",
                            truncate(&key.id, 40),
                            truncate(&key.name, 20),
                            role_str,
                            if key.active { "Yes" } else { "No" },
                            expires,
                            key.usage_count
                        );
                    }
                }
            }
        }

        Commands::Show { key_id } => {
            if let Some(key) = auth_manager.get_key(&key_id).await {
                println!("API Key Details:");
                println!("  ID: {}", key.id);
                println!("  Name: {}", key.name);
                println!("  Role: {:?}", key.role);
                println!("  Created by: {}", key.created_by);
                println!("  Created at: {}", key.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
                if let Some(expires_at) = key.expires_at {
                    println!("  Expires at: {}", expires_at.format("%Y-%m-%d %H:%M:%S UTC"));
                } else {
                    println!("  Expires: Never");
                }
                println!("  Active: {}", key.active);
                println!("  Usage count: {}", key.usage_count);
                if let Some(last_used) = key.last_used {
                    println!("  Last used: {}", last_used.format("%Y-%m-%d %H:%M:%S UTC"));
                } else {
                    println!("  Last used: Never");
                }
                if key.ip_whitelist.is_empty() {
                    println!("  IP whitelist: All IPs allowed");
                } else {
                    println!("  IP whitelist: {}", key.ip_whitelist.join(", "));
                }
                if !key.metadata.is_empty() {
                    println!("  Metadata:");
                    for (k, v) in &key.metadata {
                        println!("    {}: {}", k, v);
                    }
                }
            } else {
                eprintln!("❌ API key not found: {}", key_id);
                std::process::exit(1);
            }
        }

        Commands::Update {
            key_id,
            name,
            active,
            expires,
            ip_whitelist,
        } => {
            if let Some(mut key) = auth_manager.get_key(&key_id).await {
                let mut updated = false;

                if let Some(new_name) = name {
                    key.name = new_name;
                    updated = true;
                }

                if let Some(is_active) = active {
                    key.active = is_active;
                    updated = true;
                }

                if let Some(expires_days) = expires {
                    key.expires_at = if expires_days == 0 {
                        None
                    } else {
                        Some(chrono::Utc::now() + chrono::Duration::days(expires_days as i64))
                    };
                    updated = true;
                }

                if let Some(ip_list) = ip_whitelist {
                    key.ip_whitelist = if ip_list.is_empty() {
                        Vec::new()
                    } else {
                        ip_list
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    };
                    updated = true;
                }

                if updated {
                    auth_manager.update_key(key).await?;
                    println!("✅ API key updated successfully: {}", key_id);
                } else {
                    println!("ℹ️  No changes specified for key: {}", key_id);
                }
            } else {
                eprintln!("❌ API key not found: {}", key_id);
                std::process::exit(1);
            }
        }

        Commands::Delete { key_id, yes } => {
            if !yes {
                print!("Are you sure you want to delete API key '{}'? [y/N]: ", key_id);
                use std::io::{self, Write};
                io::stdout().flush().unwrap();

                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("❌ Deletion cancelled");
                    return Ok(());
                }
            }

            if auth_manager.delete_key(&key_id).await? {
                println!("✅ API key deleted successfully: {}", key_id);
            } else {
                eprintln!("❌ API key not found: {}", key_id);
                std::process::exit(1);
            }
        }

        Commands::Test { secret, ip } => {
            let result = auth_manager.authenticate(&secret, &ip).await;
            match result {
                loxone_mcp_rust::auth::models::AuthResult::Success(auth_success) => {
                    println!("✅ Authentication successful!");
                    println!("  Key ID: {}", auth_success.key.id);
                    println!("  Key Name: {}", auth_success.key.name);
                    println!("  Role: {:?}", auth_success.key.role);
                    println!("  Session ID: {}", auth_success.context.session_id);
                    println!("  Client IP: {}", auth_success.context.client_ip);
                }
                loxone_mcp_rust::auth::models::AuthResult::Unauthorized { reason } => {
                    eprintln!("❌ Authentication failed: {}", reason);
                    std::process::exit(1);
                }
                loxone_mcp_rust::auth::models::AuthResult::Forbidden { reason } => {
                    eprintln!("❌ Authentication forbidden: {}", reason);
                    std::process::exit(1);
                }
                loxone_mcp_rust::auth::models::AuthResult::RateLimited { retry_after_seconds } => {
                    eprintln!("❌ Rate limited. Retry after {} seconds", retry_after_seconds);
                    std::process::exit(1);
                }
            }
        }

        Commands::Stats => {
            let stats = auth_manager.get_auth_stats().await;
            println!("Authentication Statistics:");
            println!("  Total API keys: {}", stats.total_keys);
            println!("  Active keys: {}", stats.active_keys);
            println!("  Expired keys: {}", stats.expired_keys);
            println!("  Blocked IPs: {}", stats.currently_blocked_ips);
            println!("  Failed attempts: {}", stats.total_failed_attempts);
        }

        Commands::Audit { limit } => {
            match auth_manager.get_audit_events(limit).await {
                Ok(events) => {
                    if events.is_empty() {
                        println!("No audit events found.");
                        return Ok(());
                    }

                    println!("Recent Audit Events:");
                    println!("{:<20} {:<15} {:<25} {:<15} {:<8}", "Timestamp", "Event", "Key ID", "Client IP", "Success");
                    println!("{}", "─".repeat(85));

                    for event in events {
                        let timestamp = event.timestamp.format("%Y-%m-%d %H:%M:%S").to_string();
                        let key_id = event.key_id.as_deref().unwrap_or("-");
                        
                        println!(
                            "{:<20} {:<15} {:<25} {:<15} {:<8}",
                            timestamp,
                            truncate(&event.event_type, 15),
                            truncate(key_id, 25),
                            truncate(&event.client_ip, 15),
                            if event.success { "Yes" } else { "No" }
                        );
                    }
                }
                Err(e) => {
                    eprintln!("❌ Failed to retrieve audit events: {}", e);
                    std::process::exit(1);
                }
            }
        }


        Commands::Init { admin_key } => {
            println!("🚀 Initializing Loxone MCP authentication system...");

            if admin_key {
                let key = auth_manager
                    .create_key(
                        "Initial Admin Key".to_string(),
                        Role::Admin,
                        "initialization".to_string(),
                        None, // No expiration
                    )
                    .await?;

                println!("✅ Authentication system initialized!");
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("🔑 Initial Admin Key Created:");
                println!("   ID:     {}", key.id);
                println!("   Secret: {}", key.secret);
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("\n⚠️  SAVE THE SECRET - IT CANNOT BE RETRIEVED LATER!");
                println!("\n🔧 Use this API key in your requests:");
                println!("   Authorization: Bearer {}", key.secret);
                println!("   Or: X-API-Key: {}", key.secret);
                println!("   Or: ?api_key={}", key.secret);
            } else {
                println!("✅ Authentication system initialized (no admin key created)");
            }

            println!("\n📚 Next steps:");
            println!("   1. Create API keys: loxone-mcp-auth create --name \"My Key\" --role operator");
            println!("   2. List keys: loxone-mcp-auth list");
            println!("   3. Test a key: loxone-mcp-auth test <secret>");
        }

        Commands::Security { check_only, auto_fix } => {
            use loxone_mcp_rust::auth::security::{self, SecurityCheck};
            
            println!("🔐 Validating SSH-style security for credential files...");
            
            // Get the default credential directory
            let cred_dir = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".loxone-mcp");
            
            let credentials_file = cred_dir.join("credentials.json");
            let audit_file = cred_dir.join("credentials.audit.jsonl");
            
            // Check if files exist
            if !cred_dir.exists() {
                println!("📁 Credential directory does not exist yet: {}", cred_dir.display());
                println!("   This is normal for a fresh installation.");
                return Ok(());
            }
            
            // Validate security
            let files_to_check = [&credentials_file, &audit_file];
            let existing_files: Vec<_> = files_to_check.iter()
                .filter(|f| f.exists())
                .map(|f| f.as_path())
                .collect();
            
            if existing_files.is_empty() {
                println!("📁 No credential files found in: {}", cred_dir.display());
                println!("   This is normal for a fresh installation.");
                return Ok(());
            }
            
            match security::validate_credential_security(&cred_dir, &existing_files) {
                Ok(checks) => {
                    let mut has_issues = false;
                    let mut secure_count = 0;
                    
                    for check in &checks {
                        match check {
                            SecurityCheck::Secure => {
                                secure_count += 1;
                            }
                            SecurityCheck::Insecure { current, required, path, fix_command } => {
                                has_issues = true;
                                println!("⚠️  SECURITY WARNING:");
                                println!("   Permissions {:o} for '{}' are too open.", current, path);
                                println!("   Required: {:o} (SSH-style secure permissions)", required);
                                println!("   Fix: {}", fix_command);
                                println!();
                            }
                            SecurityCheck::Unchecked { reason } => {
                                println!("ℹ️  Security check skipped: {}", reason);
                            }
                        }
                    }
                    
                    if !has_issues {
                        println!("✅ All credential files have secure permissions ({} files checked)", secure_count);
                        println!("   Directory: {} (700)", cred_dir.display());
                        for file in &existing_files {
                            println!("   File: {} (600)", file.display());
                        }
                    } else if check_only {
                        println!("❌ Security issues found. Use --auto-fix to correct them.");
                        std::process::exit(1);
                    } else if auto_fix {
                        println!("🔧 Auto-fixing insecure permissions...");
                        match security::auto_fix_permissions(&checks, true) {
                            Ok(()) => {
                                println!("✅ All permissions have been fixed");
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to fix permissions: {}", e);
                                std::process::exit(1);
                            }
                        }
                    } else {
                        println!("💡 Run with --auto-fix to automatically correct these issues");
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("❌ Security validation failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}

/// Truncate a string to fit in a column
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}