//! Cryptographic utilities for Loxone communication
//!
//! This module provides RSA and AES encryption capabilities using OpenSSL.

#[cfg(feature = "crypto-openssl")]
pub use crate::client::auth::{
    encrypt_credentials, get_public_key_from_certificate, AuthToken, LoxoneAuth, LoxonePublicKey,
    TokenAuthClient,
};

#[cfg(not(feature = "crypto-openssl"))]
pub mod stubs {
    //! Stub implementations for when crypto features are disabled

    use crate::error::{LoxoneError, Result};

    /// Stub for LoxonePublicKey
    pub struct LoxonePublicKey;

    /// Stub for AuthToken
    pub struct AuthToken;

    /// Stub for LoxoneAuth
    pub struct LoxoneAuth;

    impl LoxoneAuth {
        pub fn new() -> Self {
            Self
        }
        pub fn clear(&mut self) {}
    }

    impl Default for LoxoneAuth {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Stub for TokenAuthClient
    pub struct TokenAuthClient;

    impl TokenAuthClient {
        pub fn new(_base_url: String) -> Self {
            Self
        }
        pub fn clear(&mut self) {}
    }

    /// Stub function for getting public key from certificate
    pub fn get_public_key_from_certificate(_certificate_pem: &str) -> Result<LoxonePublicKey> {
        Err(LoxoneError::crypto(
            "Crypto features not enabled - cannot parse certificates".to_string(),
        ))
    }

    /// Stub function for encrypting credentials
    pub fn encrypt_credentials(
        _public_key: &LoxonePublicKey,
        _credentials: &str,
    ) -> Result<String> {
        Err(LoxoneError::crypto(
            "Crypto features not enabled - cannot encrypt credentials".to_string(),
        ))
    }
}

#[cfg(not(feature = "crypto-openssl"))]
pub use stubs::*;
