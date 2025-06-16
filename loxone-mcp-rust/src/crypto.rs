//! Cryptographic utilities for Loxone communication
//!
//! This module provides RSA and AES encryption capabilities, but is currently
//! disabled due to security vulnerabilities in the rsa crate dependencies.

// The RSA functionality is disabled due to RUSTSEC-2023-0071 (Marvin Attack vulnerability)
// When a secure RSA implementation is available, this code can be restored from git history.

#[cfg(all(feature = "crypto", feature = "rsa"))]
mod rsa_crypto {
    use anyhow::Result;
    use rand::rngs::OsRng;
    use rsa::sha2::{Digest, Sha256};
    use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};

    pub struct CryptoManager {
        rsa_private_key: Option<RsaPrivateKey>,
        rsa_public_key: Option<RsaPublicKey>,
    }

    impl Default for CryptoManager {
        fn default() -> Self {
            Self::new()
        }
    }

    impl CryptoManager {
        pub fn new() -> Self {
            Self {
                rsa_private_key: None,
                rsa_public_key: None,
            }
        }

        pub fn generate_keypair(&mut self, bits: usize) -> Result<()> {
            let mut rng = OsRng;
            let private_key = RsaPrivateKey::new(&mut rng, bits)?;
            let public_key = RsaPublicKey::from(&private_key);

            self.rsa_private_key = Some(private_key);
            self.rsa_public_key = Some(public_key);

            Ok(())
        }

        pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
            let public_key = self
                .rsa_public_key
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("No public key available"))?;

            let mut rng = OsRng;
            let encrypted = public_key.encrypt(&mut rng, Pkcs1v15Encrypt, data)?;
            Ok(encrypted)
        }

        pub fn decrypt(&self, encrypted_data: &[u8]) -> Result<Vec<u8>> {
            let private_key = self
                .rsa_private_key
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("No private key available"))?;

            let decrypted = private_key.decrypt(Pkcs1v15Encrypt, encrypted_data)?;
            Ok(decrypted)
        }

        pub fn hash_sha256(data: &[u8]) -> Vec<u8> {
            let mut hasher = Sha256::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
    }
}

// Stub implementation when RSA is disabled
#[cfg(not(all(feature = "crypto", feature = "rsa")))]
mod stub_crypto {
    use anyhow::Result;

    pub struct CryptoManager;

    impl Default for CryptoManager {
        fn default() -> Self {
            Self::new()
        }
    }

    impl CryptoManager {
        pub fn new() -> Self {
            Self
        }

        pub fn generate_keypair(&mut self, _bits: usize) -> Result<()> {
            Err(anyhow::anyhow!(
                "RSA functionality is disabled due to security vulnerabilities"
            ))
        }

        pub fn encrypt(&self, _data: &[u8]) -> Result<Vec<u8>> {
            Err(anyhow::anyhow!(
                "RSA functionality is disabled due to security vulnerabilities"
            ))
        }

        pub fn decrypt(&self, _encrypted_data: &[u8]) -> Result<Vec<u8>> {
            Err(anyhow::anyhow!(
                "RSA functionality is disabled due to security vulnerabilities"
            ))
        }

        pub fn hash_sha256(_data: &[u8]) -> Vec<u8> {
            Vec::new()
        }
    }
}

// Re-export the appropriate implementation
#[cfg(all(feature = "crypto", feature = "rsa"))]
pub use rsa_crypto::CryptoManager;

#[cfg(not(all(feature = "crypto", feature = "rsa")))]
pub use stub_crypto::CryptoManager;
