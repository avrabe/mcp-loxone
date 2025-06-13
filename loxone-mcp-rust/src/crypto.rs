#[cfg(feature = "crypto")]
use rand::rngs::OsRng;
#[cfg(feature = "crypto")]
use rsa::sha2::{Digest, Sha256};
#[cfg(feature = "crypto")]
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};

use anyhow::Result;

#[cfg(feature = "crypto")]
pub struct CryptoManager {
    rsa_private_key: Option<RsaPrivateKey>,
    rsa_public_key: Option<RsaPublicKey>,
}

#[cfg(feature = "crypto")]
impl Default for CryptoManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "crypto")]
impl CryptoManager {
    pub fn new() -> Self {
        Self {
            rsa_private_key: None,
            rsa_public_key: None,
        }
    }

    pub fn generate_rsa_keypair(&mut self) -> Result<()> {
        let mut rng = OsRng;
        let bits = 2048;
        let private_key = RsaPrivateKey::new(&mut rng, bits)?;
        let public_key = RsaPublicKey::from(&private_key);

        self.rsa_private_key = Some(private_key);
        self.rsa_public_key = Some(public_key);

        Ok(())
    }

    pub fn encrypt_with_rsa(&self, data: &[u8]) -> Result<Vec<u8>> {
        let public_key = self
            .rsa_public_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("RSA public key not available"))?;

        let mut rng = OsRng;
        let encrypted = public_key.encrypt(&mut rng, Pkcs1v15Encrypt, data)?;
        Ok(encrypted)
    }

    pub fn sha256_hash(&self, data: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    pub fn generate_session_key(&self) -> [u8; 32] {
        let mut key = [0u8; 32];
        use rand::RngCore;
        OsRng.fill_bytes(&mut key);
        key
    }
}

#[cfg(not(feature = "crypto"))]
pub struct CryptoManager;

#[cfg(not(feature = "crypto"))]
impl CryptoManager {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_rsa_keypair(&mut self) -> Result<()> {
        Err(anyhow::anyhow!("Crypto features not enabled"))
    }

    pub fn encrypt_with_rsa(&self, _data: &[u8]) -> Result<Vec<u8>> {
        Err(anyhow::anyhow!("Crypto features not enabled"))
    }

    pub fn sha256_hash(&self, _data: &[u8]) -> Vec<u8> {
        vec![]
    }

    pub fn generate_session_key(&self) -> [u8; 32] {
        [0u8; 32]
    }
}
