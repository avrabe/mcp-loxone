//! AES encryption utilities for secure Loxone WebSocket communication
//!
//! This module provides AES-CBC encryption/decryption functionality for securing
//! WebSocket message payloads in Loxone communication. It implements the encryption
//! protocol used by Loxone Miniservers for sensitive data transmission.

use aes::{
    cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit},
    Aes256,
};
use base64::{engine::general_purpose, Engine as _};
use rand::{thread_rng, RngCore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::debug;

// For CBC mode, we'll implement a simple version since the cbc crate version is old
// In production, consider upgrading to a newer cbc crate version

/// Encryption-related errors
#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("Invalid key length: expected 32 bytes, got {0}")]
    InvalidKeyLength(usize),

    #[error("Invalid IV length: expected 16 bytes, got {0}")]
    InvalidIvLength(usize),

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid base64 data: {0}")]
    Base64Error(#[from] base64::DecodeError),

    #[error("Key exchange failed: {0}")]
    KeyExchangeFailed(String),
}

/// Encryption session with key and metadata
#[derive(Debug, Clone)]
pub struct EncryptionSession {
    /// AES-256 encryption key (32 bytes)
    pub key: [u8; 32],
    /// Session identifier
    pub session_id: String,
    /// Key creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Key expiration timestamp
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// Encryption statistics
    pub stats: EncryptionStats,
}

/// Encryption statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct EncryptionStats {
    /// Messages encrypted
    pub messages_encrypted: u64,
    /// Messages decrypted
    pub messages_decrypted: u64,
    /// Bytes encrypted
    pub bytes_encrypted: u64,
    /// Bytes decrypted
    pub bytes_decrypted: u64,
    /// Encryption errors
    pub encryption_errors: u64,
    /// Decryption errors
    pub decryption_errors: u64,
}

/// Encrypted message container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedMessage {
    /// Base64-encoded encrypted payload
    pub payload: String,
    /// Base64-encoded initialization vector
    pub iv: String,
    /// Session identifier
    pub session_id: String,
    /// Message timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Optional message metadata
    pub metadata: Option<HashMap<String, String>>,
}

/// Key exchange request/response structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyExchangeRequest {
    /// Client public key (placeholder for future RSA implementation)
    pub client_key: String,
    /// Requested session duration in seconds
    pub session_duration: u64,
    /// Client capabilities
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyExchangeResponse {
    /// Server response status
    pub status: String,
    /// Encrypted session key
    pub encrypted_key: String,
    /// Session identifier
    pub session_id: String,
    /// Key expiration timestamp
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// Server capabilities
    pub server_capabilities: Vec<String>,
}

impl EncryptionSession {
    /// Create a new encryption session with random key
    pub fn new(session_duration_hours: u32) -> Self {
        let mut key = [0u8; 32];
        thread_rng().fill_bytes(&mut key);

        let now = chrono::Utc::now();
        let session_id = generate_session_id();

        Self {
            key,
            session_id,
            created_at: now,
            expires_at: now + chrono::Duration::hours(session_duration_hours as i64),
            stats: EncryptionStats::default(),
        }
    }

    /// Create session from existing key
    pub fn from_key(key: [u8; 32], session_duration_hours: u32) -> Self {
        let now = chrono::Utc::now();
        let session_id = generate_session_id();

        Self {
            key,
            session_id,
            created_at: now,
            expires_at: now + chrono::Duration::hours(session_duration_hours as i64),
            stats: EncryptionStats::default(),
        }
    }

    /// Check if the session is still valid
    pub fn is_valid(&self) -> bool {
        chrono::Utc::now() < self.expires_at
    }

    /// Get remaining session time in seconds
    pub fn remaining_seconds(&self) -> i64 {
        (self.expires_at - chrono::Utc::now()).num_seconds().max(0)
    }

    /// Encrypt a message using AES-256-CBC
    pub fn encrypt_message(
        &mut self,
        plaintext: &[u8],
    ) -> Result<EncryptedMessage, EncryptionError> {
        if !self.is_valid() {
            return Err(EncryptionError::EncryptionFailed(
                "Session expired".to_string(),
            ));
        }

        // Generate random IV
        let mut iv = [0u8; 16];
        thread_rng().fill_bytes(&mut iv);

        // Encrypt the message
        let encrypted_data = encrypt_aes_cbc(&self.key, &iv, plaintext)?;

        // Update statistics
        self.stats.messages_encrypted += 1;
        self.stats.bytes_encrypted += plaintext.len() as u64;

        debug!(
            "Encrypted {} bytes with session {}",
            plaintext.len(),
            &self.session_id[..8]
        );

        Ok(EncryptedMessage {
            payload: general_purpose::STANDARD.encode(&encrypted_data),
            iv: general_purpose::STANDARD.encode(iv),
            session_id: self.session_id.clone(),
            timestamp: chrono::Utc::now(),
            metadata: None,
        })
    }

    /// Decrypt a message using AES-256-CBC
    pub fn decrypt_message(
        &mut self,
        encrypted_msg: &EncryptedMessage,
    ) -> Result<Vec<u8>, EncryptionError> {
        if !self.is_valid() {
            return Err(EncryptionError::DecryptionFailed(
                "Session expired".to_string(),
            ));
        }

        if encrypted_msg.session_id != self.session_id {
            return Err(EncryptionError::DecryptionFailed(
                "Session ID mismatch".to_string(),
            ));
        }

        // Decode base64 data
        let encrypted_data = general_purpose::STANDARD.decode(&encrypted_msg.payload)?;
        let iv = general_purpose::STANDARD.decode(&encrypted_msg.iv)?;

        if iv.len() != 16 {
            return Err(EncryptionError::InvalidIvLength(iv.len()));
        }

        let iv_len = iv.len();
        let iv_array: [u8; 16] = iv
            .try_into()
            .map_err(|_| EncryptionError::InvalidIvLength(iv_len))?;

        // Decrypt the message
        let plaintext = decrypt_aes_cbc(&self.key, &iv_array, &encrypted_data)?;

        // Update statistics
        self.stats.messages_decrypted += 1;
        self.stats.bytes_decrypted += plaintext.len() as u64;

        debug!(
            "Decrypted {} bytes with session {}",
            plaintext.len(),
            &self.session_id[..8]
        );

        Ok(plaintext)
    }

    /// Get encryption statistics
    pub fn get_stats(&self) -> &EncryptionStats {
        &self.stats
    }
}

/// Encrypt data using AES-256-CBC with PKCS7 padding
pub fn encrypt_aes_cbc(
    key: &[u8; 32],
    iv: &[u8; 16],
    plaintext: &[u8],
) -> Result<Vec<u8>, EncryptionError> {
    // Add PKCS7 padding
    let block_size = 16;
    let padding_len = block_size - (plaintext.len() % block_size);
    let mut padded_data = plaintext.to_vec();
    padded_data.extend(vec![padding_len as u8; padding_len]);

    // Initialize AES-256 cipher
    let cipher = Aes256::new(GenericArray::from_slice(key));

    // Perform CBC encryption
    let mut ciphertext = Vec::with_capacity(padded_data.len());
    let mut previous_block = *iv;

    for chunk in padded_data.chunks(block_size) {
        // XOR with previous ciphertext block (or IV for first block)
        let mut block = [0u8; 16];
        for i in 0..16 {
            block[i] = chunk[i] ^ previous_block[i];
        }

        // Encrypt the block
        let mut encrypted_block = *GenericArray::from_slice(&block);
        cipher.encrypt_block(&mut encrypted_block);

        // Store the encrypted block
        ciphertext.extend_from_slice(&encrypted_block);
        previous_block.copy_from_slice(&encrypted_block);
    }

    Ok(ciphertext)
}

/// Decrypt data using AES-256-CBC with PKCS7 padding
pub fn decrypt_aes_cbc(
    key: &[u8; 32],
    iv: &[u8; 16],
    ciphertext: &[u8],
) -> Result<Vec<u8>, EncryptionError> {
    if !ciphertext.len().is_multiple_of(16) {
        return Err(EncryptionError::DecryptionFailed(
            "Ciphertext length must be multiple of 16".to_string(),
        ));
    }

    // Initialize AES-256 cipher
    let cipher = Aes256::new(GenericArray::from_slice(key));

    // Perform CBC decryption
    let mut plaintext = Vec::with_capacity(ciphertext.len());
    let mut previous_block = *iv;

    for chunk in ciphertext.chunks(16) {
        // Decrypt the block
        let mut decrypted_block = *GenericArray::from_slice(chunk);
        cipher.decrypt_block(&mut decrypted_block);

        // XOR with previous ciphertext block (or IV for first block)
        let mut block = [0u8; 16];
        for i in 0..16 {
            block[i] = decrypted_block[i] ^ previous_block[i];
        }

        plaintext.extend_from_slice(&block);
        previous_block.copy_from_slice(chunk);
    }

    // Remove PKCS7 padding
    if let Some(&padding_len) = plaintext.last() {
        let padding_len = padding_len as usize;
        if padding_len > 0 && padding_len <= 16 {
            // Verify padding
            let padding_start = plaintext.len() - padding_len;
            if plaintext[padding_start..]
                .iter()
                .all(|&b| b == padding_len as u8)
            {
                plaintext.truncate(padding_start);
            } else {
                return Err(EncryptionError::DecryptionFailed(
                    "Invalid PKCS7 padding".to_string(),
                ));
            }
        } else {
            return Err(EncryptionError::DecryptionFailed(
                "Invalid padding length".to_string(),
            ));
        }
    }

    Ok(plaintext)
}

/// Generate a secure session ID
fn generate_session_id() -> String {
    let mut bytes = [0u8; 16];
    thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Key derivation for Loxone-specific key exchange
pub fn derive_loxone_key(
    username: &str,
    password: &str,
    salt: &[u8],
) -> Result<[u8; 32], EncryptionError> {
    use sha2::{Digest, Sha256};

    let mut key = [0u8; 32];
    let credential_data = format!("{username}:{password}");

    // Simplified key derivation - in production, use proper PBKDF2
    let mut hasher = Sha256::new();
    hasher.update(credential_data.as_bytes());
    hasher.update(salt);

    // Hash multiple times to simulate PBKDF2 iterations
    let mut hash = hasher.finalize();
    for _ in 0..1000 {
        let mut hasher = Sha256::new();
        hasher.update(hash);
        hash = hasher.finalize();
    }

    key.copy_from_slice(&hash);
    Ok(key)
}

/// Simple key exchange implementation (placeholder for future RSA)
pub async fn perform_key_exchange(
    request: KeyExchangeRequest,
    server_capabilities: Vec<String>,
) -> Result<KeyExchangeResponse, EncryptionError> {
    // In a real implementation, this would use RSA key exchange
    // For now, we'll simulate a successful exchange

    debug!(
        "Performing key exchange for client with capabilities: {:?}",
        request.capabilities
    );

    // Generate a new session
    let session = EncryptionSession::new((request.session_duration / 3600) as u32);

    // In real implementation, encrypt the session key with client's public key
    let encrypted_key = general_purpose::STANDARD.encode(session.key);

    Ok(KeyExchangeResponse {
        status: "success".to_string(),
        encrypted_key,
        session_id: session.session_id,
        expires_at: session.expires_at,
        server_capabilities,
    })
}

/// Encryption session manager for multiple concurrent sessions
#[derive(Debug)]
pub struct EncryptionManager {
    sessions: HashMap<String, EncryptionSession>,
    max_sessions: usize,
}

impl EncryptionManager {
    /// Create a new encryption manager
    pub fn new(max_sessions: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            max_sessions,
        }
    }

    /// Add a new session
    pub fn add_session(&mut self, session: EncryptionSession) -> Result<(), EncryptionError> {
        if self.sessions.len() >= self.max_sessions {
            self.cleanup_expired_sessions();

            if self.sessions.len() >= self.max_sessions {
                return Err(EncryptionError::KeyExchangeFailed(
                    "Maximum sessions reached".to_string(),
                ));
            }
        }

        let session_id = session.session_id.clone();
        self.sessions.insert(session_id, session);
        Ok(())
    }

    /// Get a session by ID
    pub fn get_session(&self, session_id: &str) -> Option<&EncryptionSession> {
        self.sessions.get(session_id)
    }

    /// Get a mutable session by ID
    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut EncryptionSession> {
        self.sessions.get_mut(session_id)
    }

    /// Remove a session
    pub fn remove_session(&mut self, session_id: &str) -> Option<EncryptionSession> {
        self.sessions.remove(session_id)
    }

    /// Clean up expired sessions
    pub fn cleanup_expired_sessions(&mut self) -> usize {
        let before_count = self.sessions.len();
        self.sessions.retain(|_, session| session.is_valid());
        let removed_count = before_count - self.sessions.len();

        if removed_count > 0 {
            debug!("Cleaned up {} expired encryption sessions", removed_count);
        }

        removed_count
    }

    /// Get total session statistics
    pub fn get_total_stats(&self) -> EncryptionStats {
        let mut total = EncryptionStats::default();

        for session in self.sessions.values() {
            let stats = &session.stats;
            total.messages_encrypted += stats.messages_encrypted;
            total.messages_decrypted += stats.messages_decrypted;
            total.bytes_encrypted += stats.bytes_encrypted;
            total.bytes_decrypted += stats.bytes_decrypted;
            total.encryption_errors += stats.encryption_errors;
            total.decryption_errors += stats.decryption_errors;
        }

        total
    }

    /// Get session count and health info
    pub fn get_session_info(&self) -> (usize, usize, usize) {
        let total = self.sessions.len();
        let valid = self.sessions.values().filter(|s| s.is_valid()).count();
        let expired = total - valid;

        (total, valid, expired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_session_creation() {
        let session = EncryptionSession::new(24);
        assert!(session.is_valid());
        assert!(session.remaining_seconds() > 0);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let mut session = EncryptionSession::new(1);
        let plaintext = b"Hello, Loxone!";

        let encrypted = session.encrypt_message(plaintext).unwrap();
        let decrypted = session.decrypt_message(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
        assert_eq!(session.stats.messages_encrypted, 1);
        assert_eq!(session.stats.messages_decrypted, 1);
    }

    #[test]
    fn test_aes_cbc_functions() {
        let key = [0u8; 32];
        let iv = [0u8; 16];
        let plaintext = b"Test message for AES encryption";

        let encrypted = encrypt_aes_cbc(&key, &iv, plaintext).unwrap();
        let decrypted = decrypt_aes_cbc(&key, &iv, &encrypted).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
        // Ensure we're not using the placeholder implementation
        assert!(!encrypted.starts_with(b"AES_PLACEHOLDER_"));
        // Ensure the encrypted data is different from plaintext (actual encryption happened)
        assert_ne!(plaintext, encrypted.as_slice());
    }

    #[tokio::test]
    async fn test_key_exchange() {
        let request = KeyExchangeRequest {
            client_key: "client_public_key".to_string(),
            session_duration: 3600,
            capabilities: vec!["aes256".to_string()],
        };

        let server_caps = vec!["aes256".to_string(), "secure_websocket".to_string()];
        let response = perform_key_exchange(request, server_caps).await.unwrap();

        assert_eq!(response.status, "success");
        assert!(!response.encrypted_key.is_empty());
        assert!(!response.session_id.is_empty());
    }

    #[test]
    fn test_encryption_manager() {
        let mut manager = EncryptionManager::new(2);

        let session1 = EncryptionSession::new(1);
        let session2 = EncryptionSession::new(1);

        let id1 = session1.session_id.clone();
        let id2 = session2.session_id.clone();

        manager.add_session(session1).unwrap();
        manager.add_session(session2).unwrap();

        assert!(manager.get_session(&id1).is_some());
        assert!(manager.get_session(&id2).is_some());

        let (total, valid, _expired) = manager.get_session_info();
        assert_eq!(total, 2);
        assert_eq!(valid, 2);
    }
}
