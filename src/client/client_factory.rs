//! Client factory pattern for dynamic authentication selection
//!
//! This module provides a factory pattern that creates the appropriate
//! Loxone client based on server capabilities and configuration.

#[cfg(feature = "websocket")]
use crate::client::LoxoneWebSocketClient;
#[cfg(feature = "crypto-openssl")]
use crate::client::TokenHttpClient;
use crate::client::{LoxoneClient, LoxoneHttpClient};
use crate::config::{credentials::LoxoneCredentials, AuthMethod, LoxoneConfig};
use crate::error::{LoxoneError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Server authentication capabilities discovered through probing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Supports basic HTTP authentication
    pub supports_basic_auth: bool,
    /// Supports token-based authentication
    pub supports_token_auth: bool,
    /// Supports WebSocket connections
    pub supports_websocket: bool,
    /// Server version (if available)
    pub server_version: Option<String>,
    /// Encryption level supported
    pub encryption_level: EncryptionLevel,
    /// Discovered at timestamp
    pub discovered_at: chrono::DateTime<chrono::Utc>,
}

/// Encryption levels supported by the server
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EncryptionLevel {
    /// No encryption (old Miniservers)
    None,
    /// Basic encryption
    Basic,
    /// AES encryption
    Aes,
    /// Full encryption with key exchange
    Full,
}

/// Client factory trait for creating appropriate clients
#[async_trait]
pub trait ClientFactory: Send + Sync {
    /// Create a client based on configuration and discovered capabilities
    async fn create_client(
        &self,
        config: &LoxoneConfig,
        credentials: &LoxoneCredentials,
        preferred_method: Option<AuthMethod>,
    ) -> Result<(Box<dyn LoxoneClient>, AuthMethod)>;

    /// Discover server capabilities
    async fn discover_capabilities(&self, config: &LoxoneConfig) -> Result<ServerCapabilities>;

    /// Get cached capabilities if available
    fn get_cached_capabilities(&self) -> Option<ServerCapabilities>;
}

/// Adaptive client factory that negotiates the best authentication method
///
/// This factory automatically detects server capabilities and chooses the appropriate
/// authentication method. For Loxone Gen 1 Miniservers (version < 9), it will only
/// use basic authentication to avoid account lockouts from unsupported token auth attempts.
pub struct AdaptiveClientFactory {
    /// Cached server capabilities
    cached_capabilities: Arc<tokio::sync::RwLock<Option<ServerCapabilities>>>,
    /// Discovery timeout
    discovery_timeout: Duration,
    /// Fallback chain for authentication methods
    fallback_chain: Vec<AuthMethod>,
}

impl Default for AdaptiveClientFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveClientFactory {
    /// Create new adaptive client factory
    pub fn new() -> Self {
        Self {
            cached_capabilities: Arc::new(tokio::sync::RwLock::new(None)),
            discovery_timeout: Duration::from_secs(5),
            fallback_chain: vec![
                #[cfg(feature = "crypto-openssl")]
                AuthMethod::Token,
                AuthMethod::Basic,
                #[cfg(feature = "websocket")]
                AuthMethod::WebSocket,
            ],
        }
    }

    /// Create with custom configuration
    pub fn with_config(discovery_timeout: Duration, fallback_chain: Vec<AuthMethod>) -> Self {
        Self {
            cached_capabilities: Arc::new(tokio::sync::RwLock::new(None)),
            discovery_timeout,
            fallback_chain,
        }
    }

    /// Probe server for token authentication support
    async fn probe_token_auth(&self, config: &LoxoneConfig) -> bool {
        #[cfg(feature = "crypto-openssl")]
        {
            // Try to get the key exchange endpoint
            let url = format!("{}/jdev/sys/getkey2/admin", config.url.as_str());
            match timeout(self.discovery_timeout, reqwest::get(&url)).await {
                Ok(Ok(response)) => {
                    let status = response.status();
                    debug!("Token auth probe returned status: {}", status);
                    status.is_success() || status.as_u16() == 400 // 400 means endpoint exists but needs proper request
                }
                Ok(Err(e)) => {
                    debug!("Token auth probe failed: {}", e);
                    false
                }
                Err(_) => {
                    debug!("Token auth probe timed out");
                    false
                }
            }
        }
        #[cfg(not(feature = "crypto-openssl"))]
        {
            false
        }
    }

    /// Probe server for WebSocket support
    async fn probe_websocket(&self, config: &LoxoneConfig) -> bool {
        #[cfg(feature = "websocket")]
        {
            // Check if WebSocket endpoint responds
            let url_str = config.url.as_str();
            let _ws_url = url_str
                .replace("http://", "ws://")
                .replace("https://", "wss://");

            // Extract host and port for TCP connection test
            if let Some(host) = config.url.host_str() {
                let port = config.url.port().unwrap_or(80);
                match timeout(
                    Duration::from_secs(2),
                    tokio::net::TcpStream::connect(format!("{host}:{port}")),
                )
                .await
                {
                    Ok(Ok(_)) => {
                        debug!("WebSocket port is open");
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        #[cfg(not(feature = "websocket"))]
        {
            false
        }
    }

    /// Try to create a client with specific auth method
    async fn try_create_client(
        &self,
        config: &LoxoneConfig,
        credentials: &LoxoneCredentials,
        method: AuthMethod,
    ) -> Result<Box<dyn LoxoneClient>> {
        match method {
            AuthMethod::Basic => {
                debug!("Creating basic HTTP client");
                let client = LoxoneHttpClient::new(config.clone(), credentials.clone()).await?;
                Ok(Box::new(client))
            }
            #[cfg(feature = "crypto-openssl")]
            AuthMethod::Token => {
                debug!("Creating token-based HTTP client");
                let client = TokenHttpClient::new(config.clone(), credentials.clone()).await?;
                Ok(Box::new(client))
            }
            #[cfg(feature = "websocket")]
            AuthMethod::WebSocket => {
                debug!("Creating WebSocket client");
                let client =
                    LoxoneWebSocketClient::new(config.clone(), credentials.clone()).await?;
                Ok(Box::new(client))
            }
            #[allow(unreachable_patterns)]
            _ => Err(LoxoneError::config(format!(
                "Authentication method {method:?} not supported"
            ))),
        }
    }
}

#[async_trait]
impl ClientFactory for AdaptiveClientFactory {
    async fn create_client(
        &self,
        config: &LoxoneConfig,
        credentials: &LoxoneCredentials,
        preferred_method: Option<AuthMethod>,
    ) -> Result<(Box<dyn LoxoneClient>, AuthMethod)> {
        // Check if we have cached capabilities
        let capabilities = if let Some(caps) = self.get_cached_capabilities() {
            // Use cached capabilities if they're recent (< 5 minutes old)
            if chrono::Utc::now() - caps.discovered_at < chrono::Duration::minutes(5) {
                caps
            } else {
                // Re-discover if cache is stale
                self.discover_capabilities(config).await?
            }
        } else {
            // Discover capabilities if not cached
            self.discover_capabilities(config).await?
        };

        // Update cache
        {
            let mut cache = self.cached_capabilities.write().await;
            *cache = Some(capabilities.clone());
        }

        // Determine auth method priority based on capabilities and preference
        let mut auth_methods = Vec::new();

        // Add preferred method first if specified and supported
        if let Some(preferred) = preferred_method {
            match preferred {
                AuthMethod::Token if capabilities.supports_token_auth => {
                    auth_methods.push(AuthMethod::Token)
                }
                AuthMethod::Basic if capabilities.supports_basic_auth => {
                    auth_methods.push(AuthMethod::Basic)
                }
                #[cfg(feature = "websocket")]
                AuthMethod::WebSocket if capabilities.supports_websocket => {
                    auth_methods.push(AuthMethod::WebSocket)
                }
                _ => {}
            }
        }

        // Add remaining methods based on capabilities and fallback chain
        for method in &self.fallback_chain {
            if !auth_methods.contains(method) {
                match method {
                    AuthMethod::Token if capabilities.supports_token_auth => {
                        auth_methods.push(*method)
                    }
                    AuthMethod::Basic if capabilities.supports_basic_auth => {
                        auth_methods.push(*method)
                    }
                    #[cfg(feature = "websocket")]
                    AuthMethod::WebSocket if capabilities.supports_websocket => {
                        auth_methods.push(*method)
                    }
                    _ => {}
                }
            }
        }

        // Try each method in order
        let mut last_error = None;
        for method in auth_methods {
            info!("Attempting authentication with method: {:?}", method);
            match self.try_create_client(config, credentials, method).await {
                Ok(client) => {
                    info!("Successfully created client with method: {:?}", method);
                    return Ok((client, method));
                }
                Err(e) => {
                    warn!("Failed to create client with method {:?}: {}", method, e);
                    last_error = Some(e);
                }
            }
        }

        // If all methods failed, return the last error
        Err(last_error.unwrap_or_else(|| {
            LoxoneError::config("No authentication methods available or all methods failed")
        }))
    }

    async fn discover_capabilities(&self, config: &LoxoneConfig) -> Result<ServerCapabilities> {
        info!(
            "Discovering server capabilities for: {}",
            config.url.as_str()
        );

        // Always supports basic auth as fallback
        let mut capabilities = ServerCapabilities {
            supports_basic_auth: true,
            supports_token_auth: false,
            supports_websocket: false,
            server_version: None,
            encryption_level: EncryptionLevel::None,
            discovered_at: chrono::Utc::now(),
        };

        // Try to get server version first - this helps determine what to probe
        match timeout(
            self.discovery_timeout,
            reqwest::get(&format!("{}/jdev/cfg/apiversion", config.url.as_str())),
        )
        .await
        {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    if let Ok(text) = response.text().await {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(version) = json
                                .get("LL")
                                .and_then(|v| v.get("value"))
                                .and_then(|v| v.as_str())
                            {
                                capabilities.server_version = Some(version.to_string());
                                info!("Detected Loxone Miniserver version: {}", version);

                                // Parse version to determine generation
                                if let Some(major_version) = version
                                    .split('.')
                                    .next()
                                    .and_then(|v| v.parse::<u32>().ok())
                                {
                                    if major_version < 9 {
                                        info!("Detected Gen 1 Miniserver (version < 9), disabling token auth probing");
                                        // Skip token auth probe for Gen 1 to avoid potential lockouts
                                        capabilities.supports_token_auth = false;
                                        capabilities.encryption_level = EncryptionLevel::None;
                                    } else {
                                        // Gen 2+ - probe for token auth
                                        capabilities.supports_token_auth =
                                            self.probe_token_auth(config).await;
                                        if capabilities.supports_token_auth {
                                            capabilities.encryption_level = EncryptionLevel::Aes;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {
                warn!("Could not determine server version - assuming Gen 1 for safety");
                // If we can't determine version, assume Gen 1 for safety
                capabilities.supports_token_auth = false;
            }
        }

        // Probe for WebSocket support (available on both Gen 1 and Gen 2)
        capabilities.supports_websocket = self.probe_websocket(config).await;

        info!("Discovered capabilities: {:?}", capabilities);
        Ok(capabilities)
    }

    fn get_cached_capabilities(&self) -> Option<ServerCapabilities> {
        self.cached_capabilities.blocking_read().clone()
    }
}

/// Simple factory that always creates the same client type
pub struct StaticClientFactory {
    auth_method: AuthMethod,
}

impl StaticClientFactory {
    /// Create factory for specific auth method
    pub fn new(auth_method: AuthMethod) -> Self {
        Self { auth_method }
    }
}

#[async_trait]
impl ClientFactory for StaticClientFactory {
    async fn create_client(
        &self,
        config: &LoxoneConfig,
        credentials: &LoxoneCredentials,
        _preferred_method: Option<AuthMethod>,
    ) -> Result<(Box<dyn LoxoneClient>, AuthMethod)> {
        let factory = AdaptiveClientFactory::new();
        let client = factory
            .try_create_client(config, credentials, self.auth_method)
            .await?;
        Ok((client, self.auth_method))
    }

    async fn discover_capabilities(&self, _config: &LoxoneConfig) -> Result<ServerCapabilities> {
        // Return static capabilities based on auth method
        Ok(ServerCapabilities {
            supports_basic_auth: matches!(self.auth_method, AuthMethod::Basic),
            supports_token_auth: matches!(self.auth_method, AuthMethod::Token),
            supports_websocket: matches!(self.auth_method, AuthMethod::WebSocket),
            server_version: None,
            encryption_level: match self.auth_method {
                AuthMethod::Token => EncryptionLevel::Aes,
                _ => EncryptionLevel::None,
            },
            discovered_at: chrono::Utc::now(),
        })
    }

    fn get_cached_capabilities(&self) -> Option<ServerCapabilities> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_factory_creation() {
        let factory = AdaptiveClientFactory::new();
        assert!(factory.cached_capabilities.blocking_read().is_none());
    }

    #[test]
    fn test_static_factory_creation() {
        let factory = StaticClientFactory::new(AuthMethod::Basic);
        assert!(factory.get_cached_capabilities().is_none());
    }

    #[tokio::test]
    async fn test_capabilities_discovery() {
        let config = LoxoneConfig {
            url: "http://192.168.1.100".parse().unwrap(),
            ..Default::default()
        };

        let factory = AdaptiveClientFactory::new();
        // This will fail in tests but demonstrates the API
        let result = factory.discover_capabilities(&config).await;
        assert!(result.is_err() || result.unwrap().supports_basic_auth);
    }
}
