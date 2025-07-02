//! Authentication and encryption utilities for Loxone communication
//!
//! This module provides RSA and AES encryption capabilities for secure
//! communication with Loxone Miniservers using the token-based authentication.

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Loxone public key structure from server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxonePublicKey {
    /// Modulus (n) in base64
    pub n: String,
    /// Public exponent (e) in base64
    pub e: String,
}

/// Authentication token from Loxone server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    /// JWT token string
    pub token: String,
    /// Key for AES encryption
    pub key: String,
    /// Salt for key derivation
    pub salt: String,
    /// Expiration timestamp
    #[serde(rename = "validUntil")]
    pub valid_until: i64,
    /// Token rights/permissions
    #[serde(rename = "tokenRights")]
    pub token_rights: i32,
    /// Unsecure connection flag
    #[serde(rename = "unsecurePass")]
    pub unsecure_pass: bool,
}

// OpenSSL implementation (modern, battle-tested, Send + Sync)
#[cfg(feature = "crypto-openssl")]
use base64::{engine::general_purpose, Engine as _};
#[cfg(feature = "crypto-openssl")]
use openssl::pkey::PKey;
#[cfg(feature = "crypto-openssl")]
use openssl::rsa::{Padding, Rsa};
#[cfg(feature = "crypto-openssl")]
use x509_parser::{parse_x509_certificate, pem::parse_x509_pem};

/// Get RSA public key from PEM certificate (stub for non-crypto builds)
#[cfg(not(feature = "crypto-openssl"))]
pub fn get_public_key_from_certificate(_certificate_pem: &str) -> Result<LoxonePublicKey> {
    Err(LoxoneError::crypto(
        "Crypto features not enabled - cannot parse certificates".to_string(),
    ))
}

/// Encrypt credentials using RSA (stub for non-crypto builds)
#[cfg(not(feature = "crypto-openssl"))]
pub fn encrypt_credentials(_public_key: &LoxonePublicKey, _credentials: &str) -> Result<String> {
    Err(LoxoneError::crypto(
        "Crypto features not enabled - cannot encrypt credentials".to_string(),
    ))
}

/// Authentication manager for Loxone crypto operations (OpenSSL implementation)
#[cfg(feature = "crypto-openssl")]
pub struct LoxoneAuth {
    /// RSA public key from server
    public_key: Option<PKey<openssl::pkey::Public>>,

    /// Current authentication token
    token: Option<AuthToken>,

    /// AES session key
    session_key: Option<Vec<u8>>,
}

#[cfg(feature = "crypto-openssl")]
impl LoxoneAuth {
    /// Create a new authentication manager
    pub fn new() -> Self {
        Self {
            public_key: None,
            token: None,
            session_key: None,
        }
    }

    /// Set the RSA public key from server certificate or raw public key
    pub fn set_public_key(&mut self, certificate_pem: &str) -> Result<()> {
        // Try parsing as X.509 certificate first
        if let Ok((_, pem)) = parse_x509_pem(certificate_pem.as_bytes()) {
            if let Ok((_, cert)) = parse_x509_certificate(&pem.contents) {
                // Extract public key from certificate
                let public_key_info = cert.public_key();
                let public_key_der = &public_key_info.subject_public_key.data;

                // Parse DER-encoded public key
                let rsa_key = Rsa::public_key_from_der(public_key_der).map_err(|e| {
                    LoxoneError::crypto(format!("Failed to parse RSA key from certificate: {e}"))
                })?;

                self.public_key = Some(
                    PKey::from_rsa(rsa_key)
                        .map_err(|e| LoxoneError::crypto(format!("Failed to create PKey: {e}")))?,
                );

                return Ok(());
            }
        }

        // If certificate parsing fails, try parsing as raw RSA public key
        // Remove PEM markers and decode base64
        let pem_data = certificate_pem
            .replace("-----BEGIN CERTIFICATE-----", "")
            .replace("-----END CERTIFICATE-----", "")
            .replace("-----BEGIN PUBLIC KEY-----", "")
            .replace("-----END PUBLIC KEY-----", "")
            .replace("-----BEGIN RSA PUBLIC KEY-----", "")
            .replace("-----END RSA PUBLIC KEY-----", "")
            .replace(['\n', '\r', ' '], "");

        let key_bytes = general_purpose::STANDARD
            .decode(&pem_data)
            .map_err(|e| LoxoneError::crypto(format!("Failed to decode base64 key: {e}")))?;

        // Try different RSA key formats
        let rsa_key = if let Ok(key) = Rsa::public_key_from_der(&key_bytes) {
            key
        } else if let Ok(key) = Rsa::public_key_from_pem(certificate_pem.as_bytes()) {
            key
        } else {
            // Last resort: try as raw RSA public key in PKCS#1 format
            Rsa::public_key_from_der_pkcs1(&key_bytes).map_err(|e| {
                LoxoneError::crypto(format!("Failed to parse raw RSA public key: {e}"))
            })?
        };

        self.public_key = Some(
            PKey::from_rsa(rsa_key)
                .map_err(|e| LoxoneError::crypto(format!("Failed to create PKey: {e}")))?,
        );

        Ok(())
    }

    /// Encrypt credentials using RSA-OAEP
    pub fn encrypt_credentials(&self, credentials: &str) -> Result<String> {
        let public_key = self
            .public_key
            .as_ref()
            .ok_or_else(|| LoxoneError::crypto("No public key set".to_string()))?;

        let rsa_key = public_key
            .rsa()
            .map_err(|e| LoxoneError::crypto(format!("Failed to get RSA key: {e}")))?;

        let mut encrypted = vec![0u8; rsa_key.size() as usize];
        let encrypted_len = rsa_key
            .public_encrypt(credentials.as_bytes(), &mut encrypted, Padding::PKCS1)
            .map_err(|e| LoxoneError::crypto(format!("Encryption failed: {e}")))?;

        encrypted.truncate(encrypted_len);
        Ok(general_purpose::STANDARD.encode(&encrypted))
    }

    /// Set authentication token
    pub fn set_token(&mut self, token: AuthToken) {
        self.token = Some(token);
    }

    /// Get current token
    pub fn get_token(&self) -> Option<&AuthToken> {
        self.token.as_ref()
    }

    /// Check if current token is expired
    pub fn is_token_expired(&self) -> bool {
        match &self.token {
            Some(token) => {
                let now = chrono::Utc::now().timestamp();
                now >= token.valid_until
            }
            None => true,
        }
    }

    /// Get current token string
    pub fn get_token_string(&self) -> Option<String> {
        self.token.as_ref().map(|t| t.token.clone())
    }

    /// Set session key for AES encryption
    pub fn set_session_key(&mut self, key: Vec<u8>) {
        self.session_key = Some(key);
    }

    /// Get session key
    pub fn get_session_key(&self) -> Option<&[u8]> {
        self.session_key.as_deref()
    }

    /// Clear all authentication data
    pub fn clear(&mut self) {
        self.public_key = None;
        self.token = None;
        self.session_key = None;
    }
}

#[cfg(feature = "crypto-openssl")]
impl Default for LoxoneAuth {
    fn default() -> Self {
        Self::new()
    }
}

/// Get RSA public key from PEM certificate (OpenSSL implementation)
#[cfg(feature = "crypto-openssl")]
pub fn get_public_key_from_certificate(certificate_pem: &str) -> Result<LoxonePublicKey> {
    // Try parsing as X.509 certificate first
    if let Ok((_, pem)) = parse_x509_pem(certificate_pem.as_bytes()) {
        if let Ok((_, cert)) = parse_x509_certificate(&pem.contents) {
            // Extract public key from certificate
            let public_key_info = cert.public_key();
            let public_key_der = &public_key_info.subject_public_key.data;

            // Parse DER-encoded RSA public key
            let rsa_key = Rsa::public_key_from_der(public_key_der).map_err(|e| {
                LoxoneError::crypto(format!("Failed to parse RSA key from certificate: {e}"))
            })?;

            // Extract n and e components
            let n = rsa_key.n();
            let e = rsa_key.e();

            // Convert to base64
            let n_base64 = general_purpose::STANDARD.encode(n.to_vec());
            let e_base64 = general_purpose::STANDARD.encode(e.to_vec());

            return Ok(LoxonePublicKey {
                n: n_base64,
                e: e_base64,
            });
        }
    }

    // If certificate parsing fails, try parsing as raw RSA public key
    // Remove PEM markers and decode base64
    let pem_data = certificate_pem
        .replace("-----BEGIN CERTIFICATE-----", "")
        .replace("-----END CERTIFICATE-----", "")
        .replace("-----BEGIN PUBLIC KEY-----", "")
        .replace("-----END PUBLIC KEY-----", "")
        .replace("-----BEGIN RSA PUBLIC KEY-----", "")
        .replace("-----END RSA PUBLIC KEY-----", "")
        .replace(['\n', '\r', ' '], "");

    let key_bytes = general_purpose::STANDARD
        .decode(&pem_data)
        .map_err(|e| LoxoneError::crypto(format!("Failed to decode base64 key: {e}")))?;

    // Try different RSA key formats
    let rsa_key = if let Ok(key) = Rsa::public_key_from_der(&key_bytes) {
        key
    } else if let Ok(key) = Rsa::public_key_from_pem(certificate_pem.as_bytes()) {
        key
    } else {
        // Last resort: try as raw RSA public key in PKCS#1 format
        Rsa::public_key_from_der_pkcs1(&key_bytes)
            .map_err(|e| LoxoneError::crypto(format!("Failed to parse raw RSA public key: {e}")))?
    };

    // Extract n and e components
    let n = rsa_key.n();
    let e = rsa_key.e();

    // Convert to base64
    let n_base64 = general_purpose::STANDARD.encode(n.to_vec());
    let e_base64 = general_purpose::STANDARD.encode(e.to_vec());

    Ok(LoxonePublicKey {
        n: n_base64,
        e: e_base64,
    })
}

/// Encrypt credentials using RSA-OAEP (OpenSSL implementation)
#[cfg(feature = "crypto-openssl")]
pub fn encrypt_credentials(public_key: &LoxonePublicKey, credentials: &str) -> Result<String> {
    // Decode n and e from base64
    let n_bytes = general_purpose::STANDARD
        .decode(&public_key.n)
        .map_err(|e| LoxoneError::crypto(format!("Failed to decode n: {e}")))?;
    let e_bytes = general_purpose::STANDARD
        .decode(&public_key.e)
        .map_err(|e| LoxoneError::crypto(format!("Failed to decode e: {e}")))?;

    // Create RSA public key from components
    let n = openssl::bn::BigNum::from_slice(&n_bytes)
        .map_err(|e| LoxoneError::crypto(format!("Failed to create BigNum for n: {e}")))?;
    let e = openssl::bn::BigNum::from_slice(&e_bytes)
        .map_err(|e| LoxoneError::crypto(format!("Failed to create BigNum for e: {e}")))?;

    let rsa_key = Rsa::from_public_components(n, e)
        .map_err(|e| LoxoneError::crypto(format!("Failed to create RSA key: {e}")))?;

    // Encrypt using OAEP padding
    let mut encrypted = vec![0u8; rsa_key.size() as usize];
    let encrypted_len = rsa_key
        .public_encrypt(credentials.as_bytes(), &mut encrypted, Padding::PKCS1)
        .map_err(|e| LoxoneError::crypto(format!("Encryption failed: {e}")))?;

    encrypted.truncate(encrypted_len);
    Ok(general_purpose::STANDARD.encode(&encrypted))
}

/// Token-based HTTP client for authenticated Loxone communication
#[cfg(feature = "crypto-openssl")]
pub struct TokenAuthClient {
    /// Base URL of the Loxone server
    base_url: String,
    /// HTTP client
    client: reqwest::Client,
    /// Authentication manager
    auth: LoxoneAuth,
    /// Username for authentication
    username: String,
}

#[cfg(feature = "crypto-openssl")]
impl TokenAuthClient {
    /// Create a new token authentication client
    pub fn new(base_url: String, client: reqwest::Client) -> Self {
        Self {
            base_url,
            client,
            auth: LoxoneAuth::new(),
            username: String::new(),
        }
    }

    /// Authenticate with username and password using proper Loxone token flow
    pub async fn authenticate(&mut self, username: &str, password: &str) -> Result<()> {
        // Store username for later use
        self.username = username.to_string();
        use openssl::hash::{hash, MessageDigest};
        use openssl::pkey::PKey;
        use openssl::sign::Signer;

        // Step 1: Get server public key
        let cert_url = format!("{}/jdev/sys/getPublicKey", self.base_url);
        let cert_response = self
            .client
            .get(&cert_url)
            .timeout(std::time::Duration::from_secs(5)) // Short timeout for token auth check
            .send()
            .await?;
        let cert_text = cert_response.text().await?;

        // Log the public key response for debugging
        debug!("Public key response: {}", cert_text);

        let cert_data: serde_json::Value = serde_json::from_str(&cert_text).map_err(|e| {
            warn!("Failed to parse public key response as JSON: {}", e);
            warn!("Response was: {}", cert_text);
            LoxoneError::Json(e)
        })?;
        let certificate = cert_data["LL"]["value"]
            .as_str()
            .ok_or_else(|| LoxoneError::authentication("No certificate in response".to_string()))?;

        self.auth.set_public_key(certificate)?;

        // Step 2: Get salt from server
        let salt_url = format!("{}/jdev/sys/getkey2/{}", self.base_url, username);
        let salt_response = self
            .client
            .get(&salt_url)
            .timeout(std::time::Duration::from_secs(5)) // Short timeout for token auth check
            .send()
            .await?;
        let salt_text = salt_response.text().await?;

        let salt_data: serde_json::Value = serde_json::from_str(&salt_text)?;
        let salt_obj = salt_data["LL"]["value"].as_object().ok_or_else(|| {
            LoxoneError::authentication("No value object in salt response".to_string())
        })?;

        let salt = salt_obj["salt"]
            .as_str()
            .ok_or_else(|| LoxoneError::authentication("No salt in response".to_string()))?;
        let key = salt_obj["key"]
            .as_str()
            .ok_or_else(|| LoxoneError::authentication("No key in response".to_string()))?;
        let hash_alg = salt_obj["hashAlg"].as_str().unwrap_or("SHA1");

        // Step 3: Create password hash (using the algorithm specified by server)
        let pwd_salt = format!("{password}:{salt}");
        let pwd_hash = if hash_alg == "SHA256" {
            hash(MessageDigest::sha256(), pwd_salt.as_bytes())
        } else {
            hash(MessageDigest::sha1(), pwd_salt.as_bytes())
        }
        .map_err(|e| LoxoneError::crypto(format!("Failed to hash password: {e}")))?;
        let pwd_hash_hex = hex::encode(pwd_hash).to_uppercase();

        // Note: Unlike the original documentation, the Python implementation
        // shows that we don't need to generate and encrypt a session key for JWT.
        // The HMAC is sufficient for authentication.

        // Step 6: Create HMAC hash using server-specified algorithm

        // HMAC: key from server is the key, username:password_hash is the data
        let hmac_key_bytes = hex::decode(key)
            .map_err(|e| LoxoneError::crypto(format!("Failed to decode key: {e}")))?;
        let hmac_data = format!("{username}:{pwd_hash_hex}");

        let pkey = PKey::hmac(&hmac_key_bytes)
            .map_err(|e| LoxoneError::crypto(format!("Failed to create HMAC key: {e}")))?;
        let digest = if hash_alg == "SHA256" {
            MessageDigest::sha256()
        } else {
            MessageDigest::sha1()
        };
        let mut signer = Signer::new(digest, &pkey)
            .map_err(|e| LoxoneError::crypto(format!("Failed to create signer: {e}")))?;
        signer
            .update(hmac_data.as_bytes())
            .map_err(|e| LoxoneError::crypto(format!("Failed to update signer: {e}")))?;
        let hmac_result = signer
            .sign_to_vec()
            .map_err(|e| LoxoneError::crypto(format!("Failed to sign: {e}")))?;
        let hmac_hex = hex::encode(hmac_result).to_uppercase();

        // Step 7: Request JWT token (not gettoken!)
        let uuid = "loxone-mcp-rust"; // Client identifier
        let permission = "4"; // Standard permission level
        let client_info = "loxone-mcp"; // Client info string

        // Use getjwt endpoint, not gettoken
        let token_url = format!(
            "{}/jdev/sys/getjwt/{}/{}/{}/{}/{}",
            self.base_url,
            hmac_hex,
            urlencoding::encode(username),
            permission,
            uuid,
            urlencoding::encode(client_info)
        );

        let token_response = self
            .client
            .get(&token_url)
            .timeout(std::time::Duration::from_secs(5)) // Short timeout for token auth check
            .send()
            .await?;
        let token_text = token_response.text().await?;

        // Log the response for debugging
        debug!("Token response: {}", token_text);

        // Parse token response
        let token_data: serde_json::Value = serde_json::from_str(&token_text).map_err(|e| {
            warn!("Failed to parse token response as JSON: {}", e);
            warn!("Response was: {}", token_text);
            LoxoneError::Json(e)
        })?;

        // JWT response has the token info in an object
        let token_obj = token_data["LL"]["value"]
            .as_object()
            .ok_or_else(|| LoxoneError::authentication("Invalid token response format"))?;

        let auth_token = AuthToken {
            token: token_obj["token"]
                .as_str()
                .ok_or_else(|| LoxoneError::authentication("No token in response"))?
                .to_string(),
            key: token_obj
                .get("key")
                .and_then(|k| k.as_str())
                .unwrap_or("")
                .to_string(),
            salt: token_obj
                .get("salt")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
            valid_until: token_obj["validUntil"]
                .as_i64()
                .ok_or_else(|| LoxoneError::authentication("No validUntil in response"))?,
            token_rights: token_obj["tokenRights"]
                .as_i64()
                .ok_or_else(|| LoxoneError::authentication("No tokenRights in response"))?
                as i32,
            unsecure_pass: token_obj
                .get("unsecurePass")
                .and_then(|u| u.as_bool())
                .unwrap_or(false),
        };

        self.auth.set_token(auth_token);
        Ok(())
    }

    /// Make authenticated request to Loxone server
    pub async fn request(&self, endpoint: &str) -> Result<serde_json::Value> {
        let token = self
            .auth
            .get_token_string()
            .ok_or_else(|| LoxoneError::authentication("No token available".to_string()))?;

        // Token is sent as query parameters, not in the URL path
        let separator = if endpoint.contains('?') { "&" } else { "?" };
        let url = format!(
            "{}/jdev/{}{}autht={}&user={}",
            self.base_url,
            endpoint,
            separator,
            token,
            urlencoding::encode(&self.username)
        );

        let response = self.client.get(&url).send().await?;
        let text = response.text().await?;

        Ok(serde_json::from_str(&text)?)
    }

    /// Check if current token is expired
    pub fn is_token_expired(&self) -> bool {
        self.auth.is_token_expired()
    }

    /// Get current token
    pub fn get_token(&self) -> Option<&AuthToken> {
        self.auth.get_token()
    }

    /// Clear authentication data
    pub fn clear(&mut self) {
        self.auth.clear();
    }

    /// Check if client is authenticated (has valid token)
    pub fn is_authenticated(&self) -> bool {
        !self.auth.is_token_expired()
    }

    /// Get authentication query parameters for requests
    pub fn get_auth_params(&self) -> Result<String> {
        let token = self
            .auth
            .get_token_string()
            .ok_or_else(|| LoxoneError::authentication("No token available".to_string()))?;
        Ok(format!(
            "autht={}&user={}",
            token,
            urlencoding::encode(&self.username)
        ))
    }

    /// Refresh the authentication token
    pub async fn refresh_token(&mut self) -> Result<()> {
        let token = self
            .auth
            .get_token_string()
            .ok_or_else(|| LoxoneError::authentication("No token to refresh"))?;

        // Call refresh endpoint
        let refresh_url = format!(
            "{}/jdev/sys/refreshjwt/{}/{}",
            self.base_url,
            token,
            urlencoding::encode(&self.username)
        );

        let response = self.client.get(&refresh_url).send().await?;
        let text = response.text().await?;

        // Parse refresh response
        let data: serde_json::Value = serde_json::from_str(&text)?;

        // Check if refresh was successful
        if let Some(value) = data["LL"]["value"].as_object() {
            // Update token info
            let auth_token = AuthToken {
                token: self.auth.get_token().unwrap().token.clone(), // Keep same token
                key: value
                    .get("key")
                    .and_then(|k| k.as_str())
                    .unwrap_or("")
                    .to_string(),
                salt: value
                    .get("salt")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string(),
                valid_until: value["validUntil"].as_i64().ok_or_else(|| {
                    LoxoneError::authentication("No validUntil in refresh response")
                })?,
                token_rights: value
                    .get("tokenRights")
                    .and_then(|t| t.as_i64())
                    .unwrap_or(self.auth.get_token().unwrap().token_rights as i64)
                    as i32,
                unsecure_pass: value
                    .get("unsecurePass")
                    .and_then(|u| u.as_bool())
                    .unwrap_or(false),
            };

            self.auth.set_token(auth_token);
            Ok(())
        } else {
            Err(LoxoneError::authentication("Failed to refresh token"))
        }
    }
}

/// Fallback authentication manager for non-crypto builds
#[cfg(not(feature = "crypto-openssl"))]
pub struct LoxoneAuth;

#[cfg(not(feature = "crypto-openssl"))]
impl LoxoneAuth {
    pub fn new() -> Self {
        Self
    }

    pub fn clear(&mut self) {
        // No-op for fallback
    }
}

#[cfg(not(feature = "crypto-openssl"))]
impl Default for LoxoneAuth {
    fn default() -> Self {
        Self::new()
    }
}

/// Fallback token client for non-crypto builds
#[cfg(not(feature = "crypto-openssl"))]
pub struct TokenAuthClient;

#[cfg(not(feature = "crypto-openssl"))]
impl TokenAuthClient {
    pub fn new(_base_url: String, _client: reqwest::Client) -> Self {
        Self
    }

    pub fn clear(&mut self) {
        // No-op for fallback
    }

    pub fn is_authenticated(&self) -> bool {
        false
    }

    pub fn get_auth_header(&self) -> Result<String> {
        Err(LoxoneError::crypto(
            "Crypto features not enabled - cannot get auth header".to_string(),
        ))
    }

    pub async fn refresh_token(&mut self) -> Result<()> {
        Err(LoxoneError::crypto(
            "Crypto features not enabled - cannot refresh token".to_string(),
        ))
    }

    pub async fn authenticate(&mut self, _username: &str, _password: &str) -> Result<()> {
        Err(LoxoneError::crypto(
            "Crypto features not enabled - cannot authenticate".to_string(),
        ))
    }

    pub fn get_token(&self) -> Option<&()> {
        None
    }

    pub fn is_token_expired(&self) -> bool {
        true
    }
}
