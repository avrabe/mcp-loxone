//! Enhanced setup utility for Loxone MCP Rust server
//! 
//! This utility helps configure credentials for the Rust server with:
//! - Interactive and non-interactive modes
//! - Multi-backend credential storage (Infisical, keychain, environment)
//! - CLI arguments matching the Python implementation

use clap::{Parser, ValueEnum};
use loxone_mcp_rust::{
    config::credentials::{LoxoneCredentials, create_best_credential_manager, CredentialManager},
    config::CredentialStore,
    Result,
};
use std::{io::{self, Write}, time::Duration, process::Command};
use tracing::{info, error, warn};

/// Available credential storage backends
#[derive(Debug, Clone, ValueEnum)]
enum CredentialBackend {
    /// Automatic selection (Infisical → Environment → Keychain)
    Auto,
    /// Infisical secret management
    Infisical,
    /// Environment variables
    Environment,
    /// System keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service)
    Keychain,
    /// WASI Key-Value store (WASM only)
    #[cfg(target_arch = "wasm32")]
    WasiKeyValue,
    /// Browser Local Storage (WASM only)
    #[cfg(target_arch = "wasm32")]
    LocalStorage,
}

/// Setup utility for Loxone MCP Rust server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Miniserver IP address (e.g., 192.168.1.100)
    #[arg(long)]
    host: Option<String>,

    /// Username for Miniserver
    #[arg(long)]
    username: Option<String>,

    /// Password for Miniserver  
    #[arg(long)]
    password: Option<String>,

    /// SSE API key (optional)
    #[arg(long, alias = "api-key")]
    api_key: Option<String>,

    /// Disable automatic server discovery
    #[arg(long, alias = "no-discovery")]
    no_discovery: bool,

    /// Discovery timeout in seconds
    #[arg(long, default_value = "5.0")]
    discovery_timeout: f64,

    /// Run in non-interactive mode (requires --host, --username, --password)
    #[arg(long)]
    non_interactive: bool,

    /// Choose credential storage backend
    #[arg(long, value_enum)]
    backend: Option<CredentialBackend>,

    /// Export environment variables for the selected backend
    #[arg(long)]
    export_env: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let args = Args::parse();

    println!("\n🔐 Loxone MCP Rust Server Setup");
    println!("========================================");

    // Quick start for local development
    println!("\n🚀 Quick Start (Lokale Entwicklung):");
    println!("────────────────────────────────────");
    println!("Für einen schnellen Test, kopiere und führe aus:\n");
    println!("```bash");
    println!("# Option 1: Direkte Umgebungsvariablen");
    println!("export LOXONE_USER=\"admin\"");
    println!("export LOXONE_PASS=\"password\"");
    println!("export LOXONE_HOST=\"192.168.1.100\"");
    println!("cargo run --bin loxone-mcp-server");
    println!("```\n");
    println!("```bash");
    println!("# Option 2: Keychain Setup (einmalig)");
    println!("cargo run --bin loxone-mcp-setup");
    println!("# Folge den Anweisungen...");
    println!("```\n");
    
    // Determine which credential backend to use
    let selected_backend = if let Some(backend_choice) = args.backend {
        backend_choice
    } else if args.non_interactive {
        // Auto-detect in non-interactive mode
        CredentialBackend::Auto
    } else {
        // Interactive backend selection
        select_credential_backend_interactive()?
    };

    println!("\n💡 Gewählter Credential Backend: {:?}", selected_backend);

    // Handle server discovery/host selection
    let host = if let Some(host) = args.host {
        println!("📍 Using provided host: {}", host);
        host
    } else if args.no_discovery {
        println!("🚫 Server discovery disabled");
        if args.non_interactive {
            error!("❌ Error: --host required in non-interactive mode when discovery is disabled");
            std::process::exit(1);
        } else {
            get_manual_input("Miniserver IP address (e.g., 192.168.1.100): ")?
        }
    } else {
        // Try network discovery
        println!("🔍 Discovering Loxone Miniservers on your network...");
        
        #[cfg(feature = "discovery")]
        {
            use loxone_mcp_rust::network_discovery::NetworkDiscovery;
            
            let discovery = NetworkDiscovery::new(Duration::from_secs_f64(args.discovery_timeout));
            match discovery.discover_servers().await {
                Ok(servers) if !servers.is_empty() => {
                    println!("\n✅ Found {} Loxone Miniserver(s):", servers.len());
                    for (i, server) in servers.iter().enumerate() {
                        println!("  {}. {} at {} (discovered via {})", 
                            i + 1, server.name, server.ip, server.method);
                    }
                    
                    if args.non_interactive {
                        // Use first discovered server in non-interactive mode
                        println!("\n📍 Using first discovered server: {}", servers[0].ip);
                        servers[0].ip.clone()
                    } else if servers.len() == 1 {
                        // Single server found - confirm with user
                        let confirm = get_manual_input(&format!(
                            "\nUse {} at {}? [Y/n]: ", 
                            servers[0].name, servers[0].ip
                        ))?;
                        
                        if confirm.to_lowercase() != "n" {
                            servers[0].ip.clone()
                        } else {
                            get_manual_input("Miniserver IP address (e.g., 192.168.1.100): ")?
                        }
                    } else {
                        // Multiple servers - let user choose
                        loop {
                            let choice = get_manual_input("\nSelect server number or enter IP manually: ")?;
                            
                            if let Ok(num) = choice.parse::<usize>() {
                                if num > 0 && num <= servers.len() {
                                    break servers[num - 1].ip.clone();
                                } else {
                                    println!("❌ Invalid selection. Please choose 1-{}", servers.len());
                                }
                            } else if !choice.is_empty() {
                                // Assume it's an IP address
                                break choice;
                            }
                        }
                    }
                }
                Ok(_) => {
                    println!("❌ No Loxone Miniservers found on your network");
                    if args.non_interactive {
                        error!("❌ Error: --host required when no servers found");
                        std::process::exit(1);
                    } else {
                        get_manual_input("Miniserver IP address (e.g., 192.168.1.100): ")?
                    }
                }
                Err(e) => {
                    warn!("Discovery failed: {}", e);
                    if args.non_interactive {
                        error!("❌ Error: --host required when discovery fails");
                        std::process::exit(1);
                    } else {
                        get_manual_input("Miniserver IP address (e.g., 192.168.1.100): ")?
                    }
                }
            }
        }
        
        #[cfg(not(feature = "discovery"))]
        {
            println!("ℹ️  Discovery feature not enabled. Build with --features discovery");
            if args.non_interactive {
                error!("❌ Error: --host required in non-interactive mode");
                std::process::exit(1);
            } else {
                get_manual_input("Miniserver IP address (e.g., 192.168.1.100): ")?
            }
        }
    };

    // Check if localhost/127.0.0.1 is configured and offer mock server
    let mock_server_handle = if host.starts_with("127.0.0.1") || host.starts_with("localhost") {
        println!("\n🧪 Localhost konfiguriert! Möchtest du den Mock Server verwenden?");
        
        // Default mock server runs on port 8080
        let mock_host = if host.contains(':') {
            host.clone()
        } else {
            format!("{}:8080", host)
        };
        
        // Check if mock server is already running
        let test_url = format!("http://{}/", mock_host);
        let is_running = reqwest::Client::new()
            .get(&test_url)
            .timeout(Duration::from_millis(500))
            .send()
            .await
            .is_ok();
            
        if is_running {
            println!("✅ Mock Server läuft bereits auf {}", host);
            None
        } else if !args.non_interactive {
            let use_mock = get_manual_input("Mock Server automatisch starten? [Y/n]: ")?;
            if use_mock.to_lowercase() != "n" {
                println!("🚀 Starte Mock Server auf {}...", mock_host);
                
                // Start mock server in background
                let child = Command::new("cargo")
                    .args(["run", "--bin", "loxone-mcp-mock-server"])
                    .spawn()
                    .map_err(loxone_mcp_rust::error::LoxoneError::Io)?;
                    
                // Wait a bit for server to start
                tokio::time::sleep(Duration::from_secs(2)).await;
                
                // Update host to include port if needed
                if !host.contains(':') {
                    println!("📝 Mock Server läuft auf Port 8080");
                    println!("   Verwende: export LOXONE_HOST=\"127.0.0.1:8080\"");
                }
                
                // Set mock server credentials
                println!("📝 Verwende Mock Server Credentials:");
                println!("   Username: admin");
                println!("   Password: test");
                
                Some(child)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Override credentials if mock server is being used
    let (username, password) = if mock_server_handle.is_some() {
        ("admin".to_string(), "test".to_string())
    } else {
        // Collect credentials normally
        let username = if let Some(username) = args.username {
            username
        } else if !args.non_interactive {
            get_manual_input("Username: ")?
        } else {
            error!("❌ Error: Username not available from CLI arguments");
            std::process::exit(1);
        };

        let password = if let Some(password) = args.password {
            password
        } else if !args.non_interactive {
            get_password_input()?
        } else {
            error!("❌ Error: Password not available from CLI arguments");
            std::process::exit(1);
        };
        
        (username, password)
    };


    // Test connection before saving
    println!("\n🔌 Testing connection...");
    match test_connection(&host, &username, &password).await {
        Ok(info) => {
            println!("\n✅ Successfully connected to Loxone Miniserver!");
            if let Some(name) = info.get("name") {
                println!("   Miniserver: {}", name);
            }
            if let Some(version) = info.get("version") {
                println!("   Version: {}", version);
            }
        }
        Err(e) => {
            error!("\n❌ Connection failed: {}", e);
            if !args.non_interactive {
                let retry = get_manual_input("\nWould you like to try again? [Y/n]: ")?;
                if retry.to_lowercase() != "n" {
                    error!("Please restart setup with correct credentials");
                }
            }
            std::process::exit(1);
        }
    }

    // Handle SSE API key
    let api_key = if let Some(api_key) = args.api_key {
        Some(api_key)
    } else if args.non_interactive {
        // Auto-generate API key in non-interactive mode
        let generated_key = generate_api_key();
        println!("🔑 Auto-generated SSE API key: {}", generated_key);
        println!("📋 Use this for web integrations:");
        println!("   Authorization: Bearer {}", generated_key);
        Some(generated_key)
    } else {
        // Interactive SSE setup
        setup_sse_api_key_interactive()?
    };

    // Create credentials
    let credentials = LoxoneCredentials {
        username: username.clone(),
        password,
        api_key,
        #[cfg(feature = "crypto")]
        public_key: None,
    };

    // Create credential manager based on selected backend
    let credential_manager = create_credential_manager_for_backend(&selected_backend).await?;
    
    // Store credentials
    info!("💾 Storing credentials using {:?} backend...", selected_backend);
    credential_manager.store_credentials(&credentials).await?;

    println!("\n✅ Credentials stored successfully in {:?}!", selected_backend);
    println!("   Host: {}", host);
    println!("   User: {}", username);
    println!("   Pass: {}", "*".repeat(8));
    if credentials.api_key.is_some() {
        println!("   API Key: {}", "*".repeat(8));
    }

    // Verify by reading back
    info!("🔍 Verifying stored credentials...");
    match credential_manager.get_credentials().await {
        Ok(_) => {
            info!("✅ Credentials verified successfully!");
        }
        Err(e) => {
            error!("❌ Failed to verify credentials: {}", e);
            std::process::exit(1);
        }
    }

    // Summary and next steps
    println!("\n📝 Next steps:");
    println!("1. Test Rust server: cargo run --bin loxone-mcp-server");
    println!("2. Verify credentials: cargo run --bin loxone-mcp-verify");
    println!("3. Test connection: cargo run --bin loxone-mcp-test-connection");

    if matches!(selected_backend, CredentialBackend::Infisical) {
        println!("\n🔐 Infisical Setup Complete!");
        println!("   ✅ Credentials are now stored in your Infisical project");
        println!("   ✅ Team members can access the same credentials");
        println!("   💡 To share with team: provide them with the same environment variables:");
        println!("      INFISICAL_PROJECT_ID=<project-id>");
        println!("      INFISICAL_ENVIRONMENT=<environment>");
        println!("      INFISICAL_CLIENT_ID=<their-client-id>");
        println!("      INFISICAL_CLIENT_SECRET=<their-client-secret>");
    } else {
        println!("\n💡 To upgrade to team-friendly Infisical storage:");
        println!("   1. Sign up at https://app.infisical.com");
        println!("   2. Create a project and set up Universal Auth");
        println!("   3. Set environment variables and run setup again");
    }

    println!("\n🎉 Setup complete!");

    // Show environment variables for server usage
    show_environment_variables(&selected_backend, &host, &username, &credentials, args.export_env);

    // Show backend-specific configuration advice
    show_backend_configuration_advice(&selected_backend);

    // Cleanup: Stop mock server if we started it
    if let Some(mut handle) = mock_server_handle {
        println!("\n🛑 Stopping mock server...");
        let _ = handle.kill();
        println!("   Mock server stopped. To run it manually:");
        println!("   cargo run --bin loxone-mcp-mock-server");
    }

    Ok(())
}

/// Interactive SSE API key setup
fn setup_sse_api_key_interactive() -> Result<Option<String>> {
    println!("\n🌐 SSE Server Setup (for web integrations like n8n, Home Assistant)");
    println!("{}", "=".repeat(60));

    println!("\nChoose SSE API key setup:");
    println!("  1. Generate secure API key automatically (recommended)");
    println!("  2. Enter custom API key");
    println!("  3. Skip SSE setup (can be configured later)");

    loop {
        let choice = get_manual_input("\nSelect option [1-3]: ")?;

        match choice.as_str() {
            "1" => {
                // Generate API key
                let api_key = generate_api_key();
                println!("\n🔑 Generated SSE API key!");
                println!("   API Key: {}", api_key);
                println!("\n📋 Use this for web integrations:");
                println!("   Authorization: Bearer {}", api_key);
                println!("   OR X-API-Key: {}", api_key);
                return Ok(Some(api_key));
            }
            "2" => {
                // Custom API key
                let api_key = get_manual_input("Enter your custom API key: ")?;
                if api_key.is_empty() {
                    println!("❌ API key cannot be empty");
                    continue;
                }
                if api_key.len() < 16 {
                    println!("⚠️  Warning: API key should be at least 16 characters for security");
                    let confirm = get_manual_input("Continue anyway? [y/N]: ")?;
                    if confirm.to_lowercase() != "y" {
                        continue;
                    }
                }
                println!("\n✅ Custom API key accepted!");
                println!("   API Key: {}", api_key);
                return Ok(Some(api_key));
            }
            "3" => {
                // Skip SSE setup
                println!("⏭️  SSE setup skipped");
                println!("   You can generate an API key later by:");
                println!("   1. Running setup again, or");
                println!("   2. Setting LOXONE_SSE_API_KEY environment variable");
                return Ok(None);
            }
            _ => {
                println!("❌ Invalid choice. Please enter 1, 2, or 3.");
            }
        }
    }
}

/// Get manual input from user
fn get_manual_input(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

/// Get password input (hidden)
fn get_password_input() -> Result<String> {
    print!("Password: ");
    io::stdout().flush()?;
    let password = rpassword::read_password()?;
    if password.is_empty() {
        error!("Password cannot be empty");
        std::process::exit(1);
    }
    Ok(password)
}

/// Generate a secure API key
fn generate_api_key() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut rng = rand::thread_rng();
    
    (0..43) // URL-safe base64 length for 32 bytes
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Test connection to Loxone Miniserver
async fn test_connection(host: &str, username: &str, password: &str) -> Result<std::collections::HashMap<String, String>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let url = format!("http://{}/data/LoxAPP3.json", host);
    let response = client
        .get(&url)
        .basic_auth(username, Some(password))
        .send()
        .await?;

    if response.status().is_success() {
        let data: serde_json::Value = response.json().await?;
        let mut info = std::collections::HashMap::new();

        if let Some(ms_info) = data.get("msInfo") {
            info.insert(
                "name".to_string(),
                ms_info.get("projectName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
            );
            info.insert(
                "version".to_string(),
                ms_info.get("swVersion")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
            );
        }

        Ok(info)
    } else if response.status() == 401 {
        Err(loxone_mcp_rust::error::LoxoneError::credentials("Invalid username or password".to_string()))
    } else {
        Err(loxone_mcp_rust::error::LoxoneError::credentials(format!("HTTP {}", response.status())))
    }
}

/// Interactive credential backend selection
fn select_credential_backend_interactive() -> Result<CredentialBackend> {
    println!("\n🔧 Credential Storage Backend Auswahl:");
    println!("────────────────────────────────────────");
    
    // Check what's available
    let infisical_available = std::env::var("INFISICAL_PROJECT_ID").is_ok() 
        && std::env::var("INFISICAL_CLIENT_ID").is_ok()
        && std::env::var("INFISICAL_CLIENT_SECRET").is_ok();

    println!("Verfügbare Backends:");
    println!("  1. Auto (empfohlen) - Automatische Auswahl");
    
    if infisical_available {
        println!("  2. Infisical ✅ - Team Secret Management (konfiguriert)");
    } else {
        println!("  2. Infisical ❌ - Team Secret Management (nicht konfiguriert)");
        println!("       Quick Setup: export INFISICAL_PROJECT_ID=\"proj_abc123\"");
        println!("                    export INFISICAL_CLIENT_ID=\"st.client123\"");
        println!("                    export INFISICAL_CLIENT_SECRET=\"st.secret456\"");
        println!("                    # Für lokale Instanz: export INFISICAL_HOST=\"http://localhost:8080\"");
    }
    
    println!("  3. Keychain - System Keychain (macOS/Windows/Linux)");
    println!("  4. Environment - Umgebungsvariablen (temporär)");

    loop {
        let choice = get_manual_input("\nWähle Backend [1-4]: ")?;
        
        match choice.as_str() {
            "1" | "" => return Ok(CredentialBackend::Auto),
            "2" => {
                if infisical_available {
                    return Ok(CredentialBackend::Infisical);
                } else {
                    println!("\n❌ Infisical nicht konfiguriert!");
                    println!();
                    println!("🚀 Schnell-Setup für Infisical:");
                    println!("   1. Öffne: https://app.infisical.com/signup");
                    println!("   2. Erstelle ein Projekt (z.B. 'loxone-home')");
                    println!("   3. Gehe zu Settings → Service Tokens → Create Token");
                    println!("   4. Setze die Umgebungsvariablen:");
                    println!();
                    println!("   export INFISICAL_PROJECT_ID=\"proj_abc123...\"    # Von der Projekt-URL");
                    println!("   export INFISICAL_CLIENT_ID=\"st.client123...\"   # Machine Identity ID");
                    println!("   export INFISICAL_CLIENT_SECRET=\"st.secret456...\" # Service Token");
                    println!("   export INFISICAL_ENVIRONMENT=\"dev\"             # Optional");
                    println!();
                    println!("   🏠 Für lokale/selbst-gehostete Instanz zusätzlich:");
                    println!("   export INFISICAL_HOST=\"http://localhost:8080\"  # Lokale Docker-Instanz");
                    println!("   # oder: export INFISICAL_HOST=\"https://your-infisical.domain.com\"");
                    println!();
                    println!("📖 Detailierte Anleitung: siehe INFISICAL_SETUP.md");
                    
                    let setup_now = get_manual_input("\nJetzt Umgebungsvariablen setzen? [y/N]: ")?;
                    if setup_now.to_lowercase() == "y" {
                        println!("\n💡 Öffne ein neues Terminal und führe aus:");
                        println!("   export INFISICAL_PROJECT_ID=\"deine-projekt-id\"");
                        println!("   export INFISICAL_CLIENT_ID=\"deine-client-id\"");
                        println!("   export INFISICAL_CLIENT_SECRET=\"dein-service-token\"");
                        println!("   cargo run --bin loxone-mcp-setup --backend infisical");
                        std::process::exit(0);
                    }
                    continue;
                }
            },
            "3" => return Ok(CredentialBackend::Keychain),
            "4" => {
                println!("⚠️  Environment Variables sind nur temporär und gehen beim Neustart verloren!");
                let confirm = get_manual_input("Trotzdem verwenden? [y/N]: ")?;
                if confirm.to_lowercase() == "y" {
                    return Ok(CredentialBackend::Environment);
                }
                continue;
            },
            _ => {
                println!("❌ Ungültige Auswahl. Bitte wähle 1-4.");
                continue;
            }
        }
    }
}

/// Create credential manager for specific backend
async fn create_credential_manager_for_backend(backend: &CredentialBackend) -> Result<CredentialManager> {
    match backend {
        CredentialBackend::Auto => {
            // Use the existing multi-backend logic
            let _multi_manager = create_best_credential_manager().await?;
            // Convert to single CredentialManager - we'll need to pick the first working backend
            let stores = vec![
                #[cfg(feature = "infisical")]
                {
                    if std::env::var("INFISICAL_PROJECT_ID").is_ok() {
                        Some(CredentialStore::Infisical {
                            project_id: std::env::var("INFISICAL_PROJECT_ID").unwrap(),
                            environment: std::env::var("INFISICAL_ENVIRONMENT").unwrap_or_else(|_| "dev".to_string()),
                            client_id: std::env::var("INFISICAL_CLIENT_ID").unwrap(),
                            client_secret: std::env::var("INFISICAL_CLIENT_SECRET").unwrap(),
                            host: std::env::var("INFISICAL_HOST").ok(),
                        })
                    } else {
                        None
                    }
                },
                Some(CredentialStore::Environment),
                #[cfg(feature = "keyring-storage")]
                Some(CredentialStore::Keyring),
            ];
            
            for store in stores.into_iter().flatten() {
                if let Ok(manager) = CredentialManager::new_async(store).await {
                    return Ok(manager);
                }
            }
            
            Err(loxone_mcp_rust::error::LoxoneError::credentials("No working credential backend found".to_string()))
        },
        CredentialBackend::Infisical => {
            #[cfg(feature = "infisical")]
            {
                let store = CredentialStore::Infisical {
                    project_id: std::env::var("INFISICAL_PROJECT_ID")
                        .map_err(|_| loxone_mcp_rust::error::LoxoneError::credentials("INFISICAL_PROJECT_ID not set".to_string()))?,
                    environment: std::env::var("INFISICAL_ENVIRONMENT").unwrap_or_else(|_| "dev".to_string()),
                    client_id: std::env::var("INFISICAL_CLIENT_ID")
                        .map_err(|_| loxone_mcp_rust::error::LoxoneError::credentials("INFISICAL_CLIENT_ID not set".to_string()))?,
                    client_secret: std::env::var("INFISICAL_CLIENT_SECRET")
                        .map_err(|_| loxone_mcp_rust::error::LoxoneError::credentials("INFISICAL_CLIENT_SECRET not set".to_string()))?,
                    host: std::env::var("INFISICAL_HOST").ok(),
                };
                CredentialManager::new_async(store).await
            }
            #[cfg(not(feature = "infisical"))]
            Err(loxone_mcp_rust::error::LoxoneError::credentials("Infisical feature not enabled".to_string()))
        },
        CredentialBackend::Environment => {
            CredentialManager::new_async(CredentialStore::Environment).await
        },
        CredentialBackend::Keychain => {
            #[cfg(feature = "keyring-storage")]
            {
                CredentialManager::new_async(CredentialStore::Keyring).await
            }
            #[cfg(not(feature = "keyring-storage"))]
            Err(loxone_mcp_rust::error::LoxoneError::credentials("Keyring feature not enabled".to_string()))
        },
        #[cfg(target_arch = "wasm32")]
        CredentialBackend::WasiKeyValue => {
            CredentialManager::new_async(CredentialStore::WasiKeyValue { store_name: None }).await
        },
        #[cfg(target_arch = "wasm32")]
        CredentialBackend::LocalStorage => {
            CredentialManager::new_async(CredentialStore::LocalStorage).await
        },
    }
}

/// Show environment variables for server usage
fn show_environment_variables(
    backend: &CredentialBackend,
    host: &str,
    username: &str,
    credentials: &LoxoneCredentials,
    export_format: bool,
) {
    println!("\n📋 Server Konfiguration:");
    println!("═══════════════════════════");

    match backend {
        CredentialBackend::Environment => {
            println!("\n💡 Da du Environment Variables gewählt hast, setze diese für den Server:");
            
            if export_format {
                println!("\n# Kopiere diese Befehle:");
                println!("export LOXONE_USER=\"{}\"", username);
                println!("export LOXONE_PASS=\"{}\"", credentials.password);
                println!("export LOXONE_HOST=\"{}\"", host);
                if let Some(ref api_key) = credentials.api_key {
                    println!("export LOXONE_API_KEY=\"{}\"", api_key);
                }
            } else {
                println!("\n```bash");
                println!("export LOXONE_USER=\"{}\"", username);
                println!("export LOXONE_PASS=\"{}\"", credentials.password);
                println!("export LOXONE_HOST=\"{}\"", host);
                if let Some(ref api_key) = credentials.api_key {
                    println!("export LOXONE_API_KEY=\"{}\"", api_key);
                }
                println!("```");
            }
        },
        CredentialBackend::Infisical => {
            let infisical_host = std::env::var("INFISICAL_HOST").unwrap_or_else(|_| "https://app.infisical.com".to_string());
            let is_custom_host = std::env::var("INFISICAL_HOST").is_ok();
            
            println!("\n💡 Für Infisical stelle sicher, dass diese Umgebungsvariablen gesetzt sind:");
            
            if export_format {
                println!("\n# Infisical Konfiguration:");
                if let Ok(project_id) = std::env::var("INFISICAL_PROJECT_ID") {
                    println!("export INFISICAL_PROJECT_ID=\"{}\"", project_id);
                }
                if let Ok(client_id) = std::env::var("INFISICAL_CLIENT_ID") {
                    println!("export INFISICAL_CLIENT_ID=\"{}\"", client_id);
                }
                if let Ok(client_secret) = std::env::var("INFISICAL_CLIENT_SECRET") {
                    println!("export INFISICAL_CLIENT_SECRET=\"{}\"", client_secret);
                }
                if let Ok(environment) = std::env::var("INFISICAL_ENVIRONMENT") {
                    println!("export INFISICAL_ENVIRONMENT=\"{}\"", environment);
                } else {
                    println!("export INFISICAL_ENVIRONMENT=\"dev\"");
                }
                if is_custom_host {
                    println!("export INFISICAL_HOST=\"{}\"", infisical_host);
                }
                println!();
                println!("# Infisical URL: {}", infisical_host);
                if is_custom_host {
                    println!("# (Custom/Self-hosted Instanz)");
                } else {
                    println!("# (Offizielle Cloud-Instanz)");
                }
            } else {
                println!("\n```bash");
                println!("# Diese sollten bereits gesetzt sein:");
                if let Ok(project_id) = std::env::var("INFISICAL_PROJECT_ID") {
                    println!("export INFISICAL_PROJECT_ID=\"{}\"", project_id);
                }
                if let Ok(client_id) = std::env::var("INFISICAL_CLIENT_ID") {
                    println!("export INFISICAL_CLIENT_ID=\"{}\"", client_id);
                }
                if let Ok(client_secret) = std::env::var("INFISICAL_CLIENT_SECRET") {
                    println!("export INFISICAL_CLIENT_SECRET=\"{}***\"", &client_secret[..8.min(client_secret.len())]);
                }
                if let Ok(environment) = std::env::var("INFISICAL_ENVIRONMENT") {
                    println!("export INFISICAL_ENVIRONMENT=\"{}\"", environment);
                } else {
                    println!("export INFISICAL_ENVIRONMENT=\"dev\"");
                }
                if is_custom_host {
                    println!("export INFISICAL_HOST=\"{}\"", infisical_host);
                }
                println!("```");
                println!();
                println!("🌐 Infisical URL: {}", infisical_host);
                if is_custom_host {
                    println!("   (Custom/Self-hosted Instanz)");
                } else {
                    println!("   (Offizielle Cloud-Instanz)");
                    println!("   Dashboard: https://app.infisical.com/project/{}/overview", 
                        std::env::var("INFISICAL_PROJECT_ID").unwrap_or_else(|_| "YOUR_PROJECT_ID".to_string()));
                }
            }
        },
        CredentialBackend::Keychain | CredentialBackend::Auto => {
            println!("\n✅ Credentials sind im Keychain gespeichert - keine Umgebungsvariablen nötig!");
            println!("   Der Server lädt sie automatisch aus dem sicheren Keychain.");
            
            println!("\n📌 Optional: Du kannst diese Umgebungsvariablen setzen um das Keychain zu überschreiben:");
            if export_format {
                println!("\n# Optional (überschreibt Keychain):");
                println!("# export LOXONE_USER=\"{}\"", username);
                println!("# export LOXONE_PASS=\"{}\"", credentials.password);
                println!("# export LOXONE_HOST=\"{}\"", host);
                if let Some(ref api_key) = credentials.api_key {
                    println!("# export LOXONE_API_KEY=\"{}\"", api_key);
                }
            } else {
                println!("\n```bash");
                println!("# Optional (überschreibt Keychain):");
                println!("# export LOXONE_USER=\"{}\"", username);
                println!("# export LOXONE_PASS=\"{}\"", credentials.password);
                println!("# export LOXONE_HOST=\"{}\"", host);
                if let Some(ref api_key) = credentials.api_key {
                    println!("# export LOXONE_API_KEY=\"{}\"", api_key);
                }
                println!("```");
            }
        },
        #[cfg(target_arch = "wasm32")]
        _ => {
            println!("\n💡 WASM Umgebung - Credentials sind im Browser Storage gespeichert.");
        },
    }

    // Generate export script if requested
    if export_format && matches!(backend, CredentialBackend::Environment | CredentialBackend::Infisical) {
        generate_export_script(backend, host, username, credentials);
    }

    println!("\n🚀 Server starten:");
    println!("```bash");
    match backend {
        CredentialBackend::Environment => {
            println!("# Option 1: Mit den export Befehlen oben");
            println!("cargo run --bin loxone-mcp-server stdio");
            println!();
            println!("# Option 2: Mit dem generierten Script");
            println!("source export_env.sh && cargo run --bin loxone-mcp-server stdio");
        },
        _ => {
            println!("cargo run --bin loxone-mcp-server stdio    # Für Claude Desktop");
            println!("cargo run --bin loxone-mcp-server http     # Für n8n/Web");
        }
    }
    println!("```");
}

/// Show backend-specific configuration advice
fn show_backend_configuration_advice(backend: &CredentialBackend) {
    println!("\n🔧 Backend-spezifische Konfiguration:");
    println!("──────────────────────────────────────");

    match backend {
        CredentialBackend::Auto => {
            println!("\n✨ Auto-Modus gewählt - der Server wird automatisch das beste verfügbare Backend verwenden:");
            println!("   1. Infisical (wenn konfiguriert)");
            println!("   2. Umgebungsvariablen");
            println!("   3. System Keychain");
        },
        CredentialBackend::Infisical => {
            let infisical_host = std::env::var("INFISICAL_HOST").unwrap_or_else(|_| "https://app.infisical.com".to_string());
            let is_custom_host = std::env::var("INFISICAL_HOST").is_ok();
            let project_id = std::env::var("INFISICAL_PROJECT_ID").unwrap_or_else(|_| "YOUR_PROJECT_ID".to_string());
            
            println!("\n🔐 Infisical Konfiguration:");
            println!("   • Credentials sind in Infisical gespeichert");
            println!("   • Team-Mitglieder können dieselben Credentials verwenden");
            println!("   • Audit-Log verfügbar für Zugriffskontrolle");
            println!("   • Rotiere regelmäßig deine Service Tokens");
            println!();
            println!("🌐 Infisical Instance:");
            println!("   URL: {}", infisical_host);
            if is_custom_host {
                println!("   Type: Self-hosted/Custom Instance");
                println!("   Project Dashboard: {}/project/{}/overview", infisical_host, project_id);
                println!("   Settings: {}/project/{}/settings", infisical_host, project_id);
            } else {
                println!("   Type: Official Cloud Instance");
                println!("   Project Dashboard: https://app.infisical.com/project/{}/overview", project_id);
                println!("   Settings: https://app.infisical.com/project/{}/settings", project_id);
                println!("   Service Tokens: https://app.infisical.com/project/{}/settings/service-tokens", project_id);
            }
        },
        CredentialBackend::Environment => {
            println!("\n⚠️  Environment Variables Konfiguration:");
            println!("   • Credentials sind nur temporär (verschwinden beim Neustart)");
            println!("   • Gut für CI/CD und temporäre Tests");
            println!("   • Für persistente Speicherung verwende Keychain oder Infisical");
            println!("   • Stelle sicher, dass die Variablen in deiner Shell gesetzt sind");
        },
        CredentialBackend::Keychain => {
            println!("\n🔒 Keychain Konfiguration:");
            println!("   • Credentials sind sicher im System Keychain gespeichert");
            println!("   • Automatisches Laden beim Server-Start");
            println!("   • Plattform-spezifisch:");
            println!("     - macOS: Keychain Access App");
            println!("     - Windows: Credential Manager");
            println!("     - Linux: GNOME Keyring / KDE Wallet");
            
            #[cfg(target_os = "macos")]
            println!("\n   💡 macOS: Öffne 'Keychain Access' um Credentials zu verwalten");
            
            #[cfg(target_os = "windows")]
            println!("\n   💡 Windows: Öffne 'Credential Manager' um Credentials zu verwalten");
            
            #[cfg(target_os = "linux")]
            println!("\n   💡 Linux: Verwende 'seahorse' oder 'kwalletmanager' um Credentials zu verwalten");
        },
        #[cfg(target_arch = "wasm32")]
        CredentialBackend::WasiKeyValue => {
            println!("\n🌐 WASI Key-Value Konfiguration:");
            println!("   • Credentials sind im WASI Key-Value Store gespeichert");
            println!("   • Verfügbar in WASM Component Model Umgebungen");
        },
        #[cfg(target_arch = "wasm32")]
        CredentialBackend::LocalStorage => {
            println!("\n🌐 Browser Local Storage Konfiguration:");
            println!("   • Credentials sind im Browser Local Storage gespeichert");
            println!("   • Nur für Browser-basierte WASM Anwendungen");
        },
    }

    println!("\n📚 Weitere Hilfe:");
    println!("   • Setup erneut ausführen: cargo run --bin loxone-mcp-setup");
    println!("   • Credentials prüfen: cargo run --bin loxone-mcp-verify");
    println!("   • Verbindung testen: cargo run --bin loxone-mcp-test-connection");
}

/// Generate export script for environment variables
fn generate_export_script(
    backend: &CredentialBackend,
    host: &str,
    username: &str,
    credentials: &LoxoneCredentials,
) {
    let script_content = match backend {
        CredentialBackend::Environment => {
            format!(
                r#"#!/bin/bash
# Generated by Loxone MCP Setup - Environment Variables
# Source this file to set environment variables for the Loxone MCP server
#
# Usage: source export_env.sh

echo "🔧 Loading Loxone MCP environment variables..."

export LOXONE_USER="{}"
export LOXONE_PASS="{}"
export LOXONE_HOST="{}"{}

echo "✅ Environment configured for Loxone MCP server"
echo "   User: $LOXONE_USER"
echo "   Host: $LOXONE_HOST"
"#,
                username,
                credentials.password,
                host,
                credentials.api_key.as_ref().map(|key| format!("\nexport LOXONE_API_KEY=\"{}\"", key)).unwrap_or_default()
            )
        },
        CredentialBackend::Infisical => {
            let project_id = std::env::var("INFISICAL_PROJECT_ID").unwrap_or_default();
            let client_id = std::env::var("INFISICAL_CLIENT_ID").unwrap_or_default();
            let client_secret = std::env::var("INFISICAL_CLIENT_SECRET").unwrap_or_default();
            let environment = std::env::var("INFISICAL_ENVIRONMENT").unwrap_or_else(|_| "dev".to_string());
            let infisical_host = std::env::var("INFISICAL_HOST").unwrap_or_default();

            format!(
                r#"#!/bin/bash
# Generated by Loxone MCP Setup - Infisical Configuration
# Source this file to set Infisical environment variables
#
# Usage: source export_env.sh

echo "🔧 Loading Infisical configuration for Loxone MCP..."

export INFISICAL_PROJECT_ID="{}"
export INFISICAL_CLIENT_ID="{}"
export INFISICAL_CLIENT_SECRET="{}"
export INFISICAL_ENVIRONMENT="{}"{}

echo "✅ Infisical configuration loaded"
echo "   Project: $INFISICAL_PROJECT_ID"
echo "   Environment: $INFISICAL_ENVIRONMENT"
"#,
                project_id,
                client_id,
                client_secret,
                environment,
                if !infisical_host.is_empty() { format!("\nexport INFISICAL_HOST=\"{}\"", infisical_host) } else { "".to_string() }
            )
        },
        _ => return, // Only generate for Environment and Infisical
    };

    match std::fs::write("export_env.sh", script_content) {
        Ok(_) => {
            println!("\n📄 Script generiert: export_env.sh");
            println!("   Verwende: source export_env.sh");
            
            // Make executable on Unix systems
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = std::fs::metadata("export_env.sh") {
                    let mut permissions = metadata.permissions();
                    permissions.set_mode(0o755);
                    let _ = std::fs::set_permissions("export_env.sh", permissions);
                }
            }
        },
        Err(e) => {
            println!("\n⚠️  Konnte export_env.sh nicht erstellen: {}", e);
        }
    }
}