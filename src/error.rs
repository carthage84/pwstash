use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("failed to generate random bytes")]
    Random,

    #[error("invalid encryption key length")]
    InvalidKeyLength,

    #[error("encryption failed")]
    Encrypt,

    #[error("decryption failed")]
    Decrypt,

    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("invalid Argon2 parameters")]
    InvalidKdfParams,
}

#[derive(Error, Debug)]
pub enum StashError {
    #[error("encryption error: {0}")]
    EncryptionError(#[from] EncryptionError),

    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("JSON error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("invalid master password or vault is corrupt")]
    InvalidMasterPassword,

    #[error("stash file not found")]
    FileNotFound,

    #[error("stash file {path} already exists")]
    FileAlreadyExists { path: String },

    #[error("vault file is corrupt")]
    CorruptVault,

    #[error("no entry for {service} found in stash")]
    ServiceNotFound { service: String },

    #[error("an entry for {service} already exists")]
    DuplicateService { service: String },

    #[error("{field} must not be empty")]
    EmptyField { field: &'static str },

    #[error("master passwords do not match")]
    PasswordMismatch,

    #[error("clipboard error: {0}")]
    Clipboard(String),

    #[error("generated password length must be between {min} and {max}")]
    InvalidGenerateLength { min: usize, max: usize },
}
