//! Authentication and encryption utilities for Loxone communication
//!
//! This module provides RSA and AES encryption capabilities for secure
//! communication with Loxone Miniservers using the token-based authentication.

#[cfg(all(feature = "crypto", feature = "rsa"))]
use crate::error::{LoxoneError, Result};
#[cfg(all(feature = "crypto", feature = "rsa"))]
use base64::{engine::general_purpose, Engine as _};
#[cfg(all(feature = "crypto", feature = "rsa"))]
use rand::rngs::OsRng;
#[cfg(all(feature = "crypto", feature = "rsa"))]
use rsa::{sha2::Sha256, Oaep, RsaPublicKey};
#[cfg(all(feature = "crypto", feature = "rsa"))]
use serde::{Deserialize, Serialize};
#[cfg(all(feature = "crypto", feature = "rsa"))]
use std::collections::HashMap;
#[cfg(all(feature = "crypto", feature = "rsa"))]
use x509_parser::{parse_x509_certificate, pem::parse_x509_pem};

/// Find the start of RSA public key data in DER-encoded certificate
#[cfg(all(feature = "crypto", feature = "rsa"))]
fn find_rsa_public_key_in_der(der_data: &[u8]) -> Option<usize> {
    // RSA OID: 06 09 2a 86 48 86 f7 0d 01 01 01 (just the OID part)
    let rsa_oid = [
        0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01,
    ];

    // Look for the RSA OID in the DER data
    for i in 0..der_data.len().saturating_sub(rsa_oid.len()) {
        if der_data[i..i + rsa_oid.len()] == rsa_oid {
            // Found RSA OID, now look for the BIT STRING containing the public key
            // From the hex dump, after the OID we have: 05 00 03 81 8d 00 30 81 89
            // We need to find the BIT STRING (0x03) and skip to the actual RSA key data
            let search_start = i + rsa_oid.len();

            // Look for the pattern: 05 00 03 (NULL + BIT STRING)
            for j in search_start..der_data.len().saturating_sub(6) {
                if der_data[j] == 0x05 && der_data[j + 1] == 0x00 && der_data[j + 2] == 0x03 {
                    // Found the pattern, now skip the BIT STRING header
                    // Structure: 03 [length] [unused_bits] [RSA_SEQUENCE]
                    let bit_string_length_pos = j + 3;
                    if bit_string_length_pos < der_data.len() {
                        // Skip BIT STRING tag (1) + length (1 or more) + unused bits (1)
                        let mut skip = 4; // 05 00 03 [length]

                        // Handle multi-byte length encoding if needed
                        if der_data[bit_string_length_pos] & 0x80 != 0 {
                            let length_bytes = (der_data[bit_string_length_pos] & 0x7f) as usize;
                            skip += length_bytes;
                        }

                        // Return position after: 05 00 03 [length] [unused_bits]
                        // This should point to the RSA SEQUENCE (0x30)
                        return Some(j + skip);
                    }
                }
            }
        }
    }

    None
}

/// Authentication token response from Loxone
#[cfg(all(feature = "crypto", feature = "rsa"))]
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
#[cfg(all(feature = "crypto", feature = "rsa"))]
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
#[cfg(all(feature = "crypto", feature = "rsa"))]
pub struct LoxoneAuth {
    /// RSA public key from server
    public_key: Option<RsaPublicKey>,

    /// Current authentication token
    token: Option<AuthToken>,

    /// AES session key
    session_key: Option<Vec<u8>>,
}

#[cfg(all(feature = "crypto", feature = "rsa"))]
impl LoxoneAuth {
    /// Create new authentication manager
    pub fn new() -> Self {
        Self {
            public_key: None,
            token: None,
            session_key: None,
        }
    }

    /// Set RSA public key from server (supports both structured and PEM format)
    pub fn set_public_key(&mut self, key_data: &LoxonePublicKey) -> Result<()> {
        // Parse RSA public key from modulus and exponent
        let n_bytes = general_purpose::STANDARD
            .decode(&key_data.n)
            .map_err(|e| LoxoneError::Crypto(format!("Invalid key modulus: {e}")))?;

        let e_bytes = general_purpose::STANDARD
            .decode(&key_data.e)
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

    /// Set RSA public key from PEM certificate
    pub fn set_public_key_from_pem(&mut self, pem_cert: &str) -> Result<()> {
        tracing::debug!("Parsing PEM certificate of length: {}", pem_cert.len());

        // Ensure the PEM certificate has proper line endings and formatting
        let normalized_pem = if pem_cert.contains('\n') {
            // Already has line breaks, just normalize
            pem_cert
                .replace('\r', "")
                .lines()
                .map(|line| line.trim())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            // Single line PEM - need to add proper line breaks
            // Split by the markers first to handle the content properly
            if let Some(start_pos) = pem_cert.find("-----BEGIN CERTIFICATE-----") {
                if let Some(end_pos) = pem_cert.find("-----END CERTIFICATE-----") {
                    let header = "-----BEGIN CERTIFICATE-----";
                    let footer = "-----END CERTIFICATE-----";

                    // Extract the base64 content between markers
                    let content_start = start_pos + header.len();
                    let base64_content = &pem_cert[content_start..end_pos];

                    // Format the base64 content with proper line breaks (64 chars per line)
                    let mut formatted_content = String::new();
                    for chunk in base64_content.as_bytes().chunks(64) {
                        if let Ok(chunk_str) = std::str::from_utf8(chunk) {
                            formatted_content.push_str(chunk_str);
                            formatted_content.push('\n');
                        }
                    }

                    // Construct the final PEM
                    format!("{}\n{}{}", header, formatted_content, footer)
                } else {
                    pem_cert.to_string()
                }
            } else {
                pem_cert.to_string()
            }
        };

        tracing::debug!("Normalized PEM certificate:\n{}", normalized_pem);

        // Parse PEM certificate
        let (_, pem) = parse_x509_pem(normalized_pem.as_bytes()).map_err(|e| {
            tracing::error!("PEM parsing failed: {}", e);
            tracing::debug!("PEM content: {}", normalized_pem);
            LoxoneError::Crypto(format!("Invalid PEM format: {e}"))
        })?;

        // Try to parse as X509 certificate first
        let cert_result = parse_x509_certificate(&pem.contents);
        let public_key_der_vec = match cert_result {
            Ok((_, cert)) => {
                tracing::debug!("Successfully parsed X509 certificate");
                cert.public_key().subject_public_key.as_ref().to_vec()
            }
            Err(e) => {
                tracing::warn!(
                    "X509 certificate parsing failed: {}, trying to extract public key manually",
                    e
                );

                // For certificates with invalid serial numbers, we can try to parse manually
                // Look for the RSA public key structure in the certificate
                // X509 certificates have this structure, and we need to find the SubjectPublicKeyInfo

                // Try to find the RSA public key sequence in the DER data
                // This is a simplified approach that looks for the RSA OID and public key data
                if let Some(rsa_key_start) = find_rsa_public_key_in_der(&pem.contents) {
                    tracing::debug!("Found RSA public key at offset {}", rsa_key_start);
                    let end_idx = (rsa_key_start + 10).min(pem.contents.len());
                    if rsa_key_start < pem.contents.len() {
                        tracing::debug!(
                            "First {} bytes at offset {}: {:02x?}",
                            end_idx - rsa_key_start,
                            rsa_key_start,
                            &pem.contents[rsa_key_start..end_idx]
                        );
                    } else {
                        tracing::error!(
                            "RSA key offset {} is beyond certificate length {}",
                            rsa_key_start,
                            pem.contents.len()
                        );
                    }
                    // Skip the unused bits byte (0x00) to get to the RSA SEQUENCE
                    if rsa_key_start < pem.contents.len() && pem.contents[rsa_key_start] == 0x00 {
                        pem.contents[rsa_key_start + 1..].to_vec()
                    } else {
                        pem.contents[rsa_key_start..].to_vec()
                    }
                } else {
                    tracing::error!("Could not find RSA public key in certificate");
                    return Err(LoxoneError::Crypto(
                        "Could not extract RSA public key from certificate".to_string(),
                    ));
                }
            }
        };
        let public_key_der = &public_key_der_vec;

        // For RSA public keys, we need to parse the DER structure to extract n and e
        // This is a simplified parser for RSA public key DER format:
        // SEQUENCE {
        //   modulus INTEGER,
        //   publicExponent INTEGER
        // }

        if public_key_der.len() < 10 {
            return Err(LoxoneError::Crypto("Public key too short".to_string()));
        }

        // Skip the initial SEQUENCE tag and length
        let mut offset = 0;
        if public_key_der[offset] != 0x30 {
            return Err(LoxoneError::Crypto("Invalid DER sequence tag".to_string()));
        }
        offset += 1;

        // Skip length encoding (simplified - assumes short form)
        if public_key_der[offset] & 0x80 == 0 {
            offset += 1; // Short form
        } else {
            let length_octets = (public_key_der[offset] & 0x7f) as usize;
            offset += 1 + length_octets; // Long form
        }

        // Parse modulus (n)
        if offset >= public_key_der.len() || public_key_der[offset] != 0x02 {
            return Err(LoxoneError::Crypto(
                "Invalid modulus INTEGER tag".to_string(),
            ));
        }
        offset += 1;

        let n_length = public_key_der[offset] as usize;
        offset += 1;

        if offset + n_length > public_key_der.len() {
            return Err(LoxoneError::Crypto(
                "Modulus length exceeds data".to_string(),
            ));
        }

        let mut n_bytes = &public_key_der[offset..offset + n_length];
        // Skip leading zero if present
        if !n_bytes.is_empty() && n_bytes[0] == 0 {
            n_bytes = &n_bytes[1..];
        }
        offset += n_length;

        // Parse exponent (e)
        if offset >= public_key_der.len() || public_key_der[offset] != 0x02 {
            return Err(LoxoneError::Crypto(
                "Invalid exponent INTEGER tag".to_string(),
            ));
        }
        offset += 1;

        let e_length = public_key_der[offset] as usize;
        offset += 1;

        if offset + e_length > public_key_der.len() {
            return Err(LoxoneError::Crypto(
                "Exponent length exceeds data".to_string(),
            ));
        }

        let mut e_bytes = &public_key_der[offset..offset + e_length];
        // Skip leading zero if present
        if !e_bytes.is_empty() && e_bytes[0] == 0 {
            e_bytes = &e_bytes[1..];
        }

        // Convert bytes to big integers
        let n = rsa::BigUint::from_bytes_be(n_bytes);
        let e = rsa::BigUint::from_bytes_be(e_bytes);

        // Create RSA public key
        let public_key = RsaPublicKey::new(n, e)
            .map_err(|e| LoxoneError::Crypto(format!("Invalid RSA key: {e}")))?;

        self.public_key = Some(public_key);
        Ok(())
    }

    /// Encrypt credentials using RSA public key
    pub fn encrypt_credentials(&self, username: &str, password: &str) -> Result<String> {
        let public_key = self
            .public_key
            .as_ref()
            .ok_or_else(|| LoxoneError::Crypto("No public key available".to_string()))?;

        // Combine username and password
        let credentials = format!("{username}:{password}");

        // Encrypt with RSA-OAEP
        let mut rng = OsRng;
        let padding = Oaep::new::<Sha256>();

        let encrypted = public_key
            .encrypt(&mut rng, padding, credentials.as_bytes())
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
        let _session_key = self
            .session_key
            .as_ref()
            .ok_or_else(|| LoxoneError::Crypto("No session key available".to_string()))?;

        // AES encryption implementation would go here
        // For now, return placeholder
        Ok(data.to_vec())
    }

    /// Decrypt data with AES session key
    pub fn decrypt_data(&self, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        let _session_key = self
            .session_key
            .as_ref()
            .ok_or_else(|| LoxoneError::Crypto("No session key available".to_string()))?;

        // AES decryption implementation would go here
        // For now, return placeholder
        Ok(encrypted_data.to_vec())
    }

    /// Create authorization header for HTTP requests
    pub fn create_auth_header(&self) -> Result<String> {
        let token = self
            .token
            .as_ref()
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

#[cfg(all(feature = "crypto", feature = "rsa"))]
impl Default for LoxoneAuth {
    fn default() -> Self {
        Self::new()
    }
}

/// Token-based authentication client
#[cfg(all(feature = "crypto", feature = "rsa"))]
pub struct TokenAuthClient {
    /// Base URL for Loxone Miniserver
    base_url: url::Url,

    /// HTTP client for API calls
    client: reqwest::Client,

    /// Authentication manager
    auth: LoxoneAuth,
}

#[cfg(all(feature = "crypto", feature = "rsa"))]
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
        let url = self
            .base_url
            .join("jdev/sys/getPublicKey")
            .map_err(|e| LoxoneError::connection(format!("Invalid URL: {e}")))?;

        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(LoxoneError::authentication(format!(
                "Failed to get public key: {}",
                response.status()
            )));
        }

        let text = response.text().await?;

        // Parse Loxone response format
        let json: serde_json::Value = serde_json::from_str(&text)?;

        // Debug: log the actual response to understand the format
        tracing::info!("Public key response received, parsing...");
        tracing::debug!(
            "Public key response: {}",
            serde_json::to_string_pretty(&json).unwrap_or_default()
        );

        // Loxone responses are wrapped in "LL" object
        if let Some(ll) = json.get("LL") {
            if let Some(value) = ll.get("value") {
                // Check if the value is a PEM certificate string (Gen 1 format)
                if let Some(cert_pem) = value.as_str() {
                    if cert_pem.starts_with("-----BEGIN CERTIFICATE-----") {
                        tracing::info!("Received PEM certificate format - parsing RSA public key");
                        tracing::info!("PEM certificate content length: {} bytes", cert_pem.len());
                        tracing::info!("PEM certificate full content: {}", cert_pem);
                        self.auth.set_public_key_from_pem(cert_pem)?;
                    } else {
                        // Loxone Gen 1 might return the certificate without PEM headers
                        // Try to add PEM headers and parse
                        tracing::info!("Trying to parse certificate without PEM headers");
                        tracing::info!(
                            "Raw certificate string (first 200 chars): {}",
                            &cert_pem[..cert_pem.len().min(200)]
                        );

                        // Check if it's base64 encoded certificate data
                        let pem_formatted = if cert_pem.contains('\n') || cert_pem.contains(' ') {
                            // Already has some formatting, try as-is first
                            format!(
                                "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----",
                                cert_pem.trim()
                            )
                        } else {
                            // Single line base64, add proper formatting
                            let mut formatted = String::from("-----BEGIN CERTIFICATE-----\n");
                            for chunk in cert_pem.as_bytes().chunks(64) {
                                formatted.push_str(std::str::from_utf8(chunk).unwrap_or(""));
                                formatted.push('\n');
                            }
                            formatted.push_str("-----END CERTIFICATE-----");
                            formatted
                        };

                        tracing::debug!(
                            "Formatted PEM certificate length: {} bytes",
                            pem_formatted.len()
                        );
                        match self.auth.set_public_key_from_pem(&pem_formatted) {
                            Ok(()) => {
                                tracing::info!("Successfully parsed certificate with formatting");
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to parse as certificate: {}, received string: {}",
                                    e,
                                    cert_pem
                                );
                                return Err(LoxoneError::authentication(
                                    "Invalid public key format",
                                ));
                            }
                        }
                    }
                } else {
                    // Try to parse as structured key data (Gen 2+ format)
                    let key_data: LoxonePublicKey = serde_json::from_value(value.clone())?;
                    self.auth.set_public_key(&key_data)?;
                }
            } else {
                tracing::error!(
                    "Missing 'value' field in LL response. Full response: {}",
                    text
                );
                return Err(LoxoneError::authentication("Invalid public key response"));
            }
        } else {
            tracing::error!(
                "Missing 'LL' field in public key response. Full response: {}",
                text
            );
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
        let url = self
            .base_url
            .join("jdev/sys/getjwt")
            .map_err(|e| LoxoneError::connection(format!("Invalid URL: {e}")))?;

        let mut params = HashMap::new();
        params.insert("user", encrypted_credentials);

        let response = self.client.post(url).form(&params).send().await?;

        if !response.status().is_success() {
            return Err(LoxoneError::authentication(format!(
                "Authentication failed: {}",
                response.status()
            )));
        }

        let text = response.text().await?;
        let json: serde_json::Value = serde_json::from_str(&text)?;

        if let Some(value) = json.get("value") {
            let token: AuthToken = serde_json::from_value(value.clone())?;
            self.auth.set_token(token);
        } else {
            return Err(LoxoneError::authentication(
                "Invalid authentication response",
            ));
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

    /// Clear authentication data
    pub fn clear(&mut self) {
        self.auth.clear();
    }

    /// Refresh authentication token
    pub async fn refresh_token(&mut self) -> Result<()> {
        let refresh_token = self
            .auth
            .get_token()
            .and_then(|t| t.refresh_token.as_ref())
            .ok_or_else(|| LoxoneError::authentication("No refresh token available"))?;

        let url = self
            .base_url
            .join("jdev/sys/refreshjwt")
            .map_err(|e| LoxoneError::connection(format!("Invalid URL: {e}")))?;

        let mut params = HashMap::new();
        params.insert("refreshToken", refresh_token.clone());

        let response = self.client.post(url).form(&params).send().await?;

        if !response.status().is_success() {
            return Err(LoxoneError::authentication(format!(
                "Token refresh failed: {}",
                response.status()
            )));
        }

        let text = response.text().await?;
        let json: serde_json::Value = serde_json::from_str(&text)?;

        if let Some(value) = json.get("value") {
            let token: AuthToken = serde_json::from_value(value.clone())?;
            self.auth.set_token(token);
        } else {
            return Err(LoxoneError::authentication(
                "Invalid token refresh response",
            ));
        }

        Ok(())
    }
}

// Placeholder implementations when rsa feature is disabled
#[cfg(not(all(feature = "crypto", feature = "rsa")))]
use crate::error::{LoxoneError, Result};

#[cfg(not(all(feature = "crypto", feature = "rsa")))]
pub struct LoxoneAuth;

#[cfg(not(all(feature = "crypto", feature = "rsa")))]
impl LoxoneAuth {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(all(feature = "crypto", feature = "rsa")))]
impl Default for LoxoneAuth {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(all(feature = "crypto", feature = "rsa")))]
pub struct TokenAuthClient;

#[cfg(not(all(feature = "crypto", feature = "rsa")))]
impl TokenAuthClient {
    pub fn new(_base_url: url::Url, _client: reqwest::Client) -> Self {
        Self
    }

    pub fn is_authenticated(&self) -> bool {
        false
    }

    pub async fn authenticate(&mut self, _username: &str, _password: &str) -> Result<()> {
        Err(LoxoneError::Crypto(
            "RSA functionality is disabled due to security vulnerabilities".to_string(),
        ))
    }

    pub async fn refresh_token(&mut self) -> Result<()> {
        Err(LoxoneError::Crypto(
            "RSA functionality is disabled due to security vulnerabilities".to_string(),
        ))
    }

    pub fn get_auth_header(&self) -> Result<String> {
        Err(LoxoneError::Crypto(
            "RSA functionality is disabled due to security vulnerabilities".to_string(),
        ))
    }

    pub fn clear(&mut self) {
        // No-op
    }
}
