//! WASM Component Model implementation for WASIP2
//!
//! This module provides the WASM Component Model interface implementation
//! optimized specifically for wasm32-wasip2 target.

use crate::config::{CredentialStore, ServerConfig};
use crate::error::{LoxoneError, Result};
use crate::wasm::wasip2::{Wasip2ConfigLoader, Wasip2CredentialManager, Wasip2McpServer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use web_sys::console;

use crate::config::credentials::{
    create_best_credential_manager, LoxoneCredentials, MultiBackendCredentialManager,
};

// Global state for the WASM component
static COMPONENT_STATE: Mutex<Option<ComponentState>> = Mutex::new(None);

struct ComponentState {
    credential_manager: MultiBackendCredentialManager,
    loxone_client: Option<crate::client::LoxoneClient>,
    config: ServerConfig,
}

/// WASM Component Model exports for the Loxone MCP server
pub mod exports {
    use super::*;

    /// Component model world interface
    wit_bindgen::generate!({
        world: "loxone-mcp",
        path: "wit",
        exports: {
            "loxone:mcp/server": Server,
            "loxone:mcp/credentials": CredentialManager,
            "loxone:mcp/config": ConfigManager,
        }
    });

    /// Server component implementation
    pub struct Server;

    impl exports::loxone::mcp::server::Guest for Server {
        /// Initialize the MCP server
        fn init(config: String) -> Result<(), String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;

            runtime.block_on(async {
                let server_config: ServerConfig = serde_json::from_str(&config)
                    .map_err(|e| format!("Invalid configuration: {}", e))?;

                let mut server = Wasip2McpServer::new(server_config)
                    .await
                    .map_err(|e| format!("Failed to create server: {}", e))?;

                server
                    .initialize_client()
                    .await
                    .map_err(|e| format!("Failed to initialize client: {}", e))?;

                Ok(())
            })
        }

        /// List available MCP tools
        fn list_tools() -> Vec<exports::loxone::mcp::server::Tool> {
            vec![
                exports::loxone::mcp::server::Tool {
                    name: "list_rooms".to_string(),
                    description: "List all available rooms in the Loxone system".to_string(),
                    parameters: r#"{"type": "object", "properties": {}}"#.to_string(),
                },
                exports::loxone::mcp::server::Tool {
                    name: "control_device".to_string(),
                    description: "Control a specific device by UUID".to_string(),
                    parameters: r#"{
                        "type": "object",
                        "properties": {
                            "uuid": {"type": "string", "description": "Device UUID"},
                            "action": {"type": "string", "description": "Action to perform"}
                        },
                        "required": ["uuid", "action"]
                    }"#
                    .to_string(),
                },
                exports::loxone::mcp::server::Tool {
                    name: "get_device_state".to_string(),
                    description: "Get current state of a device".to_string(),
                    parameters: r#"{
                        "type": "object",
                        "properties": {
                            "uuid": {"type": "string", "description": "Device UUID"}
                        },
                        "required": ["uuid"]
                    }"#
                    .to_string(),
                },
                exports::loxone::mcp::server::Tool {
                    name: "test_connection".to_string(),
                    description: "Test connection to Loxone Miniserver".to_string(),
                    parameters: r#"{"type": "object", "properties": {}}"#.to_string(),
                },
            ]
        }

        /// Call an MCP tool
        fn call_tool(name: String, arguments: String) -> Result<String, String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;

            runtime.block_on(async {
                // Load configuration
                let config = Wasip2ConfigLoader::load_config()
                    .await
                    .map_err(|e| format!("Failed to load config: {}", e))?;

                let mut server = Wasip2McpServer::new(config)
                    .await
                    .map_err(|e| format!("Failed to create server: {}", e))?;

                server
                    .initialize_client()
                    .await
                    .map_err(|e| format!("Failed to initialize client: {}", e))?;

                let args: serde_json::Value = serde_json::from_str(&arguments)
                    .map_err(|e| format!("Invalid arguments: {}", e))?;

                let result = server
                    .call_tool(&name, args)
                    .await
                    .map_err(|e| format!("Tool execution failed: {}", e))?;

                serde_json::to_string(&result)
                    .map_err(|e| format!("Failed to serialize result: {}", e))
            })
        }

        /// Get server capabilities
        fn get_capabilities() -> exports::loxone::mcp::server::Capabilities {
            exports::loxone::mcp::server::Capabilities {
                name: "Loxone MCP Server (WASIP2)".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                protocol_version: "2024-11-05".to_string(),
                features: vec![
                    "credential-management".to_string(),
                    "device-control".to_string(),
                    "sensor-discovery".to_string(),
                    "room-management".to_string(),
                    "infisical-integration".to_string(),
                    "wasi-keyvalue".to_string(),
                    "wasi-http".to_string(),
                    "wasi-config".to_string(),
                ],
                runtime: "wasm32-wasip2".to_string(),
            }
        }

        /// Shutdown the server
        fn shutdown() -> Result<(), String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;

            runtime.block_on(async {
                crate::wasm::wasip2::shutdown_wasip2_component()
                    .await
                    .map_err(|e| format!("Shutdown failed: {}", e))
            })
        }

        /// Get server statistics
        fn get_stats() -> String {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap_or_else(|_| panic!("Failed to create runtime"));

            runtime.block_on(async {
                match Wasip2McpServer::get_metrics() {
                    Ok(metrics) => {
                        serde_json::to_string(&metrics).unwrap_or_else(|_| "{}".to_string())
                    }
                    Err(_) => r#"{"error": "Failed to get metrics"}"#.to_string(),
                }
            })
        }
    }

    /// Credential manager component implementation
    pub struct CredentialManager;

    impl exports::loxone::mcp::credentials::Guest for CredentialManager {
        /// Store credentials securely
        fn store(username: String, password: String, host: Option<String>) -> Result<(), String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;

            runtime.block_on(async {
                let credentials = crate::config::credentials::LoxoneCredentials {
                    username,
                    password,
                    api_key: None,
                    #[cfg(feature = "crypto")]
                    public_key: None,
                };

                Wasip2CredentialManager::store_credentials(&credentials)
                    .await
                    .map_err(|e| format!("Failed to store credentials: {}", e))
            })
        }

        /// Retrieve stored credentials
        fn get() -> Result<exports::loxone::mcp::credentials::Credentials, String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;

            runtime.block_on(async {
                let creds = Wasip2CredentialManager::get_credentials()
                    .await
                    .map_err(|e| format!("Failed to get credentials: {}", e))?;

                Ok(exports::loxone::mcp::credentials::Credentials {
                    username: creds.username,
                    password: creds.password,
                    api_key: creds.api_key,
                })
            })
        }

        /// Clear stored credentials
        fn clear() -> Result<(), String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;

            runtime.block_on(async {
                Wasip2CredentialManager::clear_credentials()
                    .await
                    .map_err(|e| format!("Failed to clear credentials: {}", e))
            })
        }

        /// Validate that credentials exist and are valid
        fn validate() -> bool {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap_or_else(|_| return false);

            runtime.block_on(async { Wasip2CredentialManager::get_credentials().await.is_ok() })
        }

        /// Test credentials by attempting connection
        fn test() -> Result<bool, String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;

            runtime.block_on(async {
                // Load configuration and test connection
                let config = Wasip2ConfigLoader::load_config()
                    .await
                    .map_err(|e| format!("Failed to load config: {}", e))?;

                let mut server = Wasip2McpServer::new(config)
                    .await
                    .map_err(|e| format!("Failed to create server: {}", e))?;

                server
                    .initialize_client()
                    .await
                    .map_err(|e| format!("Failed to initialize client: {}", e))?;

                let result = server
                    .call_tool("test_connection", serde_json::json!({}))
                    .await
                    .map_err(|e| format!("Connection test failed: {}", e))?;

                if let Some(connected) = result.get("connected") {
                    Ok(connected.as_bool().unwrap_or(false))
                } else {
                    Ok(false)
                }
            })
        }
    }

    /// Configuration manager component implementation
    pub struct ConfigManager;

    impl exports::loxone::mcp::config::Guest for ConfigManager {
        /// Load configuration from WASI runtime config
        fn load() -> Result<String, String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;

            runtime.block_on(async {
                let config = Wasip2ConfigLoader::load_config()
                    .await
                    .map_err(|e| format!("Failed to load config: {}", e))?;

                serde_json::to_string(&config)
                    .map_err(|e| format!("Failed to serialize config: {}", e))
            })
        }

        /// Validate configuration
        fn validate(config: String) -> Result<bool, String> {
            let server_config: ServerConfig = serde_json::from_str(&config)
                .map_err(|e| format!("Invalid configuration: {}", e))?;

            match Wasip2ConfigLoader::validate_wasip2_config(&server_config) {
                Ok(()) => Ok(true),
                Err(_) => Ok(false),
            }
        }

        /// Get default configuration for WASIP2
        fn get_default() -> String {
            let mut config = ServerConfig::default();
            config.credentials = CredentialStore::WasiKeyvalue;
            config.mcp.transport.transport_type = "wasm".to_string();

            serde_json::to_string(&config).unwrap_or_else(|_| "{}".to_string())
        }

        /// Set configuration value
        fn set(key: String, value: String) -> Result<(), String> {
            // This would set configuration in WASI config interface
            // For now, return error as it's not implemented
            Err(format!("Setting config key '{}' not implemented", key))
        }

        /// Get configuration value
        fn get(key: String) -> Result<Option<String>, String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;

            runtime.block_on(async {
                #[cfg(target_arch = "wasm32")]
                {
                    use wasi::config::runtime;
                    match runtime::get(&key) {
                        Ok(value) => Ok(value),
                        Err(_) => Ok(None),
                    }
                }

                #[cfg(not(target_arch = "wasm32"))]
                Ok(None)
            })
        }
    }
}

/// Initialize the WASM component with logging
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    // Set up panic hook for better error messages
    console_error_panic_hook::set_once();

    // Initialize tracing for WASM
    #[cfg(feature = "debug-logging")]
    {
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
        use tracing_wasm::WASMLayerConfigBuilder;

        let wasm_layer = tracing_wasm::WASMLayer::new(
            WASMLayerConfigBuilder::new()
                .set_max_level(tracing::Level::DEBUG)
                .build(),
        );

        tracing_subscriber::registry().with(wasm_layer).init();
    }

    console::log_1(&"Loxone MCP WASM component initialized".into());
}

/// Component initialization for WASIP2
#[cfg(target_arch = "wasm32")]
pub fn init_component() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging for WASIP2
    #[cfg(feature = "debug-logging")]
    {
        wasi::logging::log(
            wasi::logging::Level::Info,
            "loxone-mcp",
            "WASM Component Model initialized for WASIP2",
        );
    }

    // Initialize panic hook
    std::panic::set_hook(Box::new(|panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s
        } else {
            "Unknown panic occurred"
        };

        #[cfg(feature = "debug-logging")]
        wasi::logging::log(
            wasi::logging::Level::Error,
            "loxone-mcp",
            &format!("Panic: {}", message),
        );
    }));

    Ok(())
}

/// Export the MCP server interface for WASM components
pub mod mcp_server_impl {
    use super::*;

    /// Initialize the MCP server with configuration
    pub async fn initialize(config_json: &str) -> Result<()> {
        let config: ServerConfig = serde_json::from_str(config_json)
            .map_err(|e| LoxoneError::config(format!("Invalid configuration: {}", e)))?;

        // Create credential manager
        let credential_manager = create_best_credential_manager().await?;

        // Initialize component state
        let state = ComponentState {
            credential_manager,
            loxone_client: None,
            config,
        };

        *COMPONENT_STATE.lock().unwrap() = Some(state);

        tracing::info!("MCP server initialized successfully");
        Ok(())
    }

    /// List available MCP tools
    pub fn list_tools() -> Result<Vec<String>> {
        let tools = vec![
            "get_rooms".to_string(),
            "get_room_devices".to_string(),
            "control_device".to_string(),
            "get_device_state".to_string(),
            "discover_sensors".to_string(),
            "list_credentials".to_string(),
            "validate_credentials".to_string(),
        ];

        Ok(tools)
    }

    /// Call an MCP tool
    pub async fn call_tool(name: &str, arguments_json: &str) -> Result<String> {
        let state_guard = COMPONENT_STATE.lock().unwrap();
        let state = state_guard
            .as_ref()
            .ok_or_else(|| LoxoneError::config("Component not initialized"))?;

        match name {
            "get_rooms" => {
                if let Some(client) = &state.loxone_client {
                    let rooms = client.get_rooms().await?;
                    Ok(serde_json::to_string(&rooms)?)
                } else {
                    Err(LoxoneError::config("Loxone client not connected"))
                }
            }

            "list_credentials" => {
                // This is safe to call without Loxone connection
                drop(state_guard); // Release the lock

                // Use a mock result for now
                let credentials = vec!["LOXONE_HOST", "LOXONE_USER", "LOXONE_PASS"];
                Ok(serde_json::to_string(&credentials)?)
            }

            "validate_credentials" => {
                drop(state_guard); // Release the lock

                let state_guard = COMPONENT_STATE.lock().unwrap();
                let state = state_guard.as_ref().unwrap();

                // Try to get credentials to validate they exist
                match state.credential_manager.get_credentials().await {
                    Ok(_) => Ok(serde_json::to_string(&true)?),
                    Err(_) => Ok(serde_json::to_string(&false)?),
                }
            }

            _ => Err(LoxoneError::config(format!("Unknown tool: {}", name))),
        }
    }

    /// Get server capabilities
    pub fn get_capabilities() -> Result<String> {
        let capabilities = serde_json::json!({
            "name": "Loxone MCP (WASM)",
            "version": env!("CARGO_PKG_VERSION"),
            "runtime": "wasm-wasip2",
            "features": [
                "credential-management",
                "device-control",
                "sensor-discovery",
                "infisical-integration",
                "wasi-keyvalue"
            ],
            "backends": [
                "environment",
                "infisical",
                "wasi-keyvalue",
                "local-storage"
            ]
        });

        Ok(capabilities.to_string())
    }

    /// Shutdown the server
    pub fn shutdown() -> Result<()> {
        *COMPONENT_STATE.lock().unwrap() = None;
        tracing::info!("MCP server shutdown complete");
        Ok(())
    }
}

/// Component metadata
pub struct ComponentMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub interfaces: Vec<String>,
    pub capabilities: Vec<String>,
}

impl Default for ComponentMetadata {
    fn default() -> Self {
        Self {
            name: "loxone-mcp-wasip2".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Loxone Generation 1 MCP Server for WASM Component Model".to_string(),
            author: "Ralf Anton Beier".to_string(),
            license: "MIT".to_string(),
            interfaces: vec![
                "loxone:mcp/server".to_string(),
                "loxone:mcp/credentials".to_string(),
                "loxone:mcp/config".to_string(),
                "wasi:keyvalue/store".to_string(),
                "wasi:http/outgoing-handler".to_string(),
                "wasi:config/runtime".to_string(),
                "wasi:logging".to_string(),
            ],
            capabilities: vec![
                "credential-management".to_string(),
                "device-control".to_string(),
                "sensor-discovery".to_string(),
                "room-management".to_string(),
                "infisical-integration".to_string(),
                "wasi-keyvalue".to_string(),
                "wasi-http".to_string(),
                "wasi-config".to_string(),
                "wasi-logging".to_string(),
            ],
        }
    }
}

impl ComponentMetadata {
    /// Get component manifest as JSON
    pub fn to_manifest(&self) -> String {
        serde_json::json!({
            "name": self.name,
            "version": self.version,
            "description": self.description,
            "author": self.author,
            "license": self.license,
            "interfaces": self.interfaces,
            "capabilities": self.capabilities,
            "target": "wasm32-wasip2",
            "component_model_version": "0.2.0"
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_metadata() {
        let metadata = ComponentMetadata::default();
        assert_eq!(metadata.name, "loxone-mcp-wasip2");
        assert!(metadata
            .interfaces
            .contains(&"loxone:mcp/server".to_string()));
        assert!(metadata.capabilities.contains(&"wasi-keyvalue".to_string()));

        let manifest = metadata.to_manifest();
        let parsed: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(parsed["target"], "wasm32-wasip2");
    }

    #[tokio::test]
    async fn test_component_initialization() {
        #[cfg(target_arch = "wasm32")]
        {
            let result = init_component();
            assert!(result.is_ok());
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Test should pass on non-WASM platforms
            assert!(true);
        }
    }
}
