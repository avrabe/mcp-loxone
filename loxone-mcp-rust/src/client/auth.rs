//! Authentication and encryption utilities for Loxone communication
//!
//! This module provides RSA and AES encryption capabilities for secure
//! communication with Loxone Miniservers using the token-based authentication.

#[cfg(feature = "crypto")]
use crate::error::{LoxoneError, Result};
#[cfg(feature = "crypto")]
use base64::{Engine as _, engine::general_purpose};
#[cfg(feature = "crypto")]
use rand::rngs::OsRng;
#[cfg(feature = "crypto")]
use rsa::{RsaPublicKey, Oaep, sha2::Sha256};
#[cfg(feature = "crypto")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "crypto")]
use std::collections::HashMap;

/// Authentication token response from Loxone
#[cfg(feature = "crypto")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    /// JWT token for authentication
    pub token: String,
    
    /// Token expiration timestamp
    pub expires: u64,
    
    /// Refresh token for renewing authentication
    pub refresh_token: Option<String>,
    
    /// Session key for AES encryption
    pub session_key: Option<String>,
}

/// RSA public key information from Loxone
#[cfg(feature = "crypto")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxonePublicKey {
    /// RSA public key in PEM format
    pub key: String,
    
    /// Key modulus (n)
    pub n: String,
    
    /// Public exponent (e)
    pub e: String,
}

/// Authentication manager for Loxone crypto operations
#[cfg(feature = "crypto")]
pub struct LoxoneAuth {
    /// RSA public key from server
    public_key: Option<RsaPublicKey>,
    
    /// Current authentication token
    token: Option<AuthToken>,
    
    /// AES session key
    session_key: Option<Vec<u8>>,
}

#[cfg(feature = "crypto")]
impl LoxoneAuth {
    /// Create new authentication manager
    pub fn new() -> Self {
        Self {
            public_key: None,
            token: None,
            session_key: None,
        }
    }
    
    /// Set RSA public key from server
    pub fn set_public_key(&mut self, key_data: &LoxonePublicKey) -> Result<()> {
        // Parse RSA public key from modulus and exponent
        let n_bytes = general_purpose::STANDARD.decode(&key_data.n)
            .map_err(|e| LoxoneError::Crypto(format!("Invalid key modulus: {e}")))?;
        
        let e_bytes = general_purpose::STANDARD.decode(&key_data.e)
            .map_err(|e| LoxoneError::Crypto(format!("Invalid key exponent: {e}")))?;
        
        // Convert bytes to big integers
        let n = rsa::BigUint::from_bytes_be(&n_bytes);
        let e = rsa::BigUint::from_bytes_be(&e_bytes);
        
        // Create RSA public key
        let public_key = RsaPublicKey::new(n, e)
            .map_err(|e| LoxoneError::Crypto(format!("Invalid RSA key: {e}")))?;
        
        self.public_key = Some(public_key);
        Ok(())
    }
    
    /// Encrypt credentials using RSA public key
    pub fn encrypt_credentials(&self, username: &str, password: &str) -> Result<String> {
        let public_key = self.public_key.as_ref()
            .ok_or_else(|| LoxoneError::Crypto("No public key available".to_string()))?;
        
        // Combine username and password
        let credentials = format!("{username}:{password}");
        
        // Encrypt with RSA-OAEP
        let mut rng = OsRng;
        let padding = Oaep::new::<Sha256>();
        
        let encrypted = public_key.encrypt(&mut rng, padding, credentials.as_bytes())
            .map_err(|e| LoxoneError::Crypto(format!("RSA encryption failed: {e}")))?;
        
        // Encode as base64
        Ok(general_purpose::STANDARD.encode(encrypted))
    }
    
    /// Set authentication token
    pub fn set_token(&mut self, token: AuthToken) {
        self.token = Some(token);
    }
    
    /// Get current token
    pub fn get_token(&self) -> Option<&AuthToken> {
        self.token.as_ref()
    }
    
    /// Check if token is expired
    pub fn is_token_expired(&self) -> bool {
        match &self.token {
            Some(token) => {
                let now = chrono::Utc::now().timestamp() as u64;
                now >= token.expires
            }
            None => true,
        }
    }
    
    /// Generate AES session key
    pub fn generate_session_key(&mut self) -> Result<Vec<u8>> {
        use rand::RngCore;
        
        let mut key = vec![0u8; 32]; // 256-bit key
        OsRng.fill_bytes(&mut key);
        
        self.session_key = Some(key.clone());
        Ok(key)
    }
    
    /// Encrypt data with AES session key
    pub fn encrypt_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        let _session_key = self.session_key.as_ref()
            .ok_or_else(|| LoxoneError::Crypto("No session key available".to_string()))?;
        
        // AES encryption implementation would go here
        // For now, return placeholder
        Ok(data.to_vec())
    }
    
    /// Decrypt data with AES session key
    pub fn decrypt_data(&self, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        let _session_key = self.session_key.as_ref()
            .ok_or_else(|| LoxoneError::Crypto("No session key available".to_string()))?;
        
        // AES decryption implementation would go here
        // For now, return placeholder
        Ok(encrypted_data.to_vec())
    }
    
    /// Create authorization header for HTTP requests
    pub fn create_auth_header(&self) -> Result<String> {
        let token = self.token.as_ref()
            .ok_or_else(|| LoxoneError::authentication("No token available"))?;
        
        if self.is_token_expired() {
            return Err(LoxoneError::authentication("Token has expired"));
        }
        
        Ok(format!("Bearer {}", token.token))
    }
    
    /// Clear authentication data
    pub fn clear(&mut self) {
        self.public_key = None;
        self.token = None;
        self.session_key = None;
    }
}

#[cfg(feature = "crypto")]
impl Default for LoxoneAuth {
    fn default() -> Self {
        Self::new()
    }
}

/// Token-based authentication client
#[cfg(feature = "crypto")]
pub struct TokenAuthClient {
    /// Base URL for Loxone Miniserver
    base_url: url::Url,
    
    /// HTTP client for API calls
    client: reqwest::Client,
    
    /// Authentication manager
    auth: LoxoneAuth,
}

#[cfg(feature = "crypto")]
impl TokenAuthClient {
    /// Create new token authentication client
    pub fn new(base_url: url::Url, client: reqwest::Client) -> Self {
        Self {
            base_url,
            client,
            auth: LoxoneAuth::new(),
        }
    }
    
    /// Get RSA public key from server
    pub async fn get_public_key(&mut self) -> Result<()> {
        let url = self.base_url.join("jdev/sys/getPublicKey")
            .map_err(|e| LoxoneError::connection(format!("Invalid URL: {e}")))?;
        
        let response = self.client.get(url).send().await?;
        
        if !response.status().is_success() {
            return Err(LoxoneError::authentication(format!(
                "Failed to get public key: {}", response.status()
            )));
        }
        
        let text = response.text().await?;
        
        // Parse Loxone response format
        let json: serde_json::Value = serde_json::from_str(&text)?;
        
        if let Some(value) = json.get("value") {
            let key_data: LoxonePublicKey = serde_json::from_value(value.clone())?;
            self.auth.set_public_key(&key_data)?;
        } else {
            return Err(LoxoneError::authentication("Invalid public key response"));
        }
        
        Ok(())
    }
    
    /// Authenticate with username and password
    pub async fn authenticate(&mut self, username: &str, password: &str) -> Result<()> {
        // Get public key if not already available
        if self.auth.public_key.is_none() {
            self.get_public_key().await?;
        }
        
        // Encrypt credentials
        let encrypted_credentials = self.auth.encrypt_credentials(username, password)?;
        
        // Request authentication token
        let url = self.base_url.join("jdev/sys/getjwt")
            .map_err(|e| LoxoneError::connection(format!("Invalid URL: {e}")))?;
        
        let mut params = HashMap::new();
        params.insert("user", encrypted_credentials);
        
        let response = self.client.post(url).form(&params).send().await?;
        
        if !response.status().is_success() {
            return Err(LoxoneError::authentication(format!(
                "Authentication failed: {}", response.status()
            )));
        }
        
        let text = response.text().await?;
        let json: serde_json::Value = serde_json::from_str(&text)?;
        
        if let Some(value) = json.get("value") {
            let token: AuthToken = serde_json::from_value(value.clone())?;
            self.auth.set_token(token);
        } else {
            return Err(LoxoneError::authentication("Invalid authentication response"));
        }
        
        Ok(())
    }
    
    /// Get current authentication header
    pub fn get_auth_header(&self) -> Result<String> {
        self.auth.create_auth_header()
    }
    
    /// Check if authentication is valid
    pub fn is_authenticated(&self) -> bool {
        self.auth.get_token().is_some() && !self.auth.is_token_expired()
    }
    
    /// Refresh authentication token
    pub async fn refresh_token(&mut self) -> Result<()> {
        let refresh_token = self.auth.get_token()
            .and_then(|t| t.refresh_token.as_ref())
            .ok_or_else(|| LoxoneError::authentication("No refresh token available"))?;
        
        let url = self.base_url.join("jdev/sys/refreshjwt")
            .map_err(|e| LoxoneError::connection(format!("Invalid URL: {e}")))?;
        
        let mut params = HashMap::new();
        params.insert("refreshToken", refresh_token.clone());
        
        let response = self.client.post(url).form(&params).send().await?;
        
        if !response.status().is_success() {
            return Err(LoxoneError::authentication(format!(
                "Token refresh failed: {}", response.status()
            )));
        }
        
        let text = response.text().await?;
        let json: serde_json::Value = serde_json::from_str(&text)?;
        
        if let Some(value) = json.get("value") {
            let token: AuthToken = serde_json::from_value(value.clone())?;
            self.auth.set_token(token);
        } else {
            return Err(LoxoneError::authentication("Invalid token refresh response"));
        }
        
        Ok(())
    }
}

// Placeholder implementations when crypto feature is disabled
#[cfg(not(feature = "crypto"))]
pub struct LoxoneAuth;

#[cfg(not(feature = "crypto"))]
impl LoxoneAuth {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(feature = "crypto"))]
pub struct TokenAuthClient;

#[cfg(not(feature = "crypto"))]
impl TokenAuthClient {
    pub fn new(_base_url: url::Url, _client: reqwest::Client) -> Self {
        Self
    }
}