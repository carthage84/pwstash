//! Vault cryptography.
//!
//! v1 uses Argon2id to derive a 32-byte key from the master password and
//! AES-256-GCM to encrypt the JSON payload.
//!
//! Argon2id parameters (OWASP 2023 recommendation for backend/interactive):
//! - memory: 19456 KiB (19 MiB)
//! - iterations: 2
//! - parallelism: 1
//! - output: 32 bytes
//!
//! These constants are fixed for the `PWS1` file format. A future version
//! would persist parameters in the header.

use aes_gcm::aead::{Aead, KeyInit, generic_array::GenericArray};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use zeroize::Zeroize;

use crate::error::{EncryptionError, StashError};

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;
pub const KEY_LEN: usize = 32;

const ARGON2_M_KIB: u32 = 19_456;
const ARGON2_T_COST: u32 = 2;
const ARGON2_P_COST: u32 = 1;

fn argon2() -> Result<Argon2<'static>, StashError> {
    let params = Params::new(ARGON2_M_KIB, ARGON2_T_COST, ARGON2_P_COST, Some(KEY_LEN))
        .map_err(|_| StashError::EncryptionError(EncryptionError::InvalidKdfParams))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

pub fn generate_salt() -> Result<[u8; SALT_LEN], StashError> {
    fill_random()
}

pub fn generate_nonce() -> Result<[u8; NONCE_LEN], StashError> {
    fill_random()
}

fn fill_random<const N: usize>() -> Result<[u8; N], StashError> {
    let mut buf = [0u8; N];
    rand::rngs::OsRng
        .try_fill_bytes(&mut buf)
        .map_err(|_| StashError::EncryptionError(EncryptionError::Random))?;
    Ok(buf)
}

pub fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], StashError> {
    if salt.len() != SALT_LEN {
        return Err(StashError::EncryptionError(
            EncryptionError::InvalidKeyLength,
        ));
    }
    let kdf = argon2()?;
    let mut key = [0u8; KEY_LEN];
    kdf.hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| StashError::EncryptionError(EncryptionError::KeyDerivation(e.to_string())))?;
    Ok(key)
}

pub fn encrypt(
    plaintext: &[u8],
    key: &[u8; KEY_LEN],
) -> Result<([u8; NONCE_LEN], Vec<u8>), StashError> {
    let nonce_bytes = generate_nonce()?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| StashError::EncryptionError(EncryptionError::Encrypt))?;
    Ok((nonce_bytes, ciphertext))
}

pub fn decrypt(
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
    key: &[u8; KEY_LEN],
) -> Result<Vec<u8>, StashError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = GenericArray::from_slice(nonce);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| StashError::EncryptionError(EncryptionError::Decrypt))
}

/// Best-effort wipe of a derived key on the stack after use by callers who
/// do not wrap it in `Zeroizing`.
pub fn zeroize_key(key: &mut [u8; KEY_LEN]) {
    key.zeroize();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_salt() -> [u8; SALT_LEN] {
        *b"0123456789abcdef"
    }

    #[test]
    fn derive_key_is_deterministic() {
        let salt = test_salt();
        let a = derive_key("correct horse battery staple", &salt).unwrap();
        let b = derive_key("correct horse battery staple", &salt).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn derive_key_differs_for_different_passwords() {
        let salt = test_salt();
        let a = derive_key("alpha", &salt).unwrap();
        let b = derive_key("bravo", &salt).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = derive_key("master", &test_salt()).unwrap();
        let (nonce, ct) = encrypt(b"hello vault", &key).unwrap();
        let pt = decrypt(&nonce, &ct, &key).unwrap();
        assert_eq!(pt, b"hello vault");
    }

    #[test]
    fn wrong_key_fails_closed() {
        let key = derive_key("master", &test_salt()).unwrap();
        let other = derive_key("other", &test_salt()).unwrap();
        let (nonce, ct) = encrypt(b"secret", &key).unwrap();
        let err = decrypt(&nonce, &ct, &other).unwrap_err();
        assert!(matches!(
            err,
            StashError::EncryptionError(EncryptionError::Decrypt)
        ));
    }

    #[test]
    fn mutated_ciphertext_fails() {
        let key = derive_key("master", &test_salt()).unwrap();
        let (nonce, mut ct) = encrypt(b"secret", &key).unwrap();
        let last = ct.len() - 1;
        ct[last] ^= 0xff;
        assert!(decrypt(&nonce, &ct, &key).is_err());
    }

    #[test]
    fn truncated_ciphertext_fails() {
        let key = derive_key("master", &test_salt()).unwrap();
        let (nonce, ct) = encrypt(b"secret", &key).unwrap();
        assert!(decrypt(&nonce, &ct[..3], &key).is_err());
    }
}
