//! On-disk `PWS1` vault format and atomic file writes.
//!
//! ```text
//! magic        4 bytes  b"PWS1"
//! salt        16 bytes  Argon2id salt (fixed at init)
//! nonce       12 bytes  AES-GCM nonce (new on every save)
//! ciphertext   N bytes  AES-256-GCM payload including tag
//! ```

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use crate::encryption::{NONCE_LEN, SALT_LEN};
use crate::error::StashError;

pub const MAGIC: &[u8; 4] = b"PWS1";
pub const HEADER_LEN: usize = MAGIC.len() + SALT_LEN + NONCE_LEN;
const GCM_TAG_LEN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultFile {
    pub salt: [u8; SALT_LEN],
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
}

impl VaultFile {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.ciphertext.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&self.ciphertext);
        out
    }

    pub fn decode(data: &[u8]) -> Result<Self, StashError> {
        if data.len() < HEADER_LEN + GCM_TAG_LEN {
            return Err(StashError::CorruptVault);
        }
        if &data[..MAGIC.len()] != MAGIC {
            return Err(StashError::CorruptVault);
        }
        let salt_start = MAGIC.len();
        let nonce_start = salt_start + SALT_LEN;
        let ct_start = nonce_start + NONCE_LEN;
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&data[salt_start..nonce_start]);
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&data[nonce_start..ct_start]);
        Ok(Self {
            salt,
            nonce,
            ciphertext: data[ct_start..].to_vec(),
        })
    }
}

pub fn read_vault_file(path: &Path) -> Result<VaultFile, StashError> {
    if !path.exists() {
        return Err(StashError::FileNotFound);
    }
    let mut file = File::open(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    VaultFile::decode(&data)
}

pub fn write_vault_file(path: &Path, vault_file: &VaultFile) -> Result<(), StashError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = tmp_path_for(path);
    {
        let mut tmp = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)?;
        tmp.write_all(&vault_file.encode())?;
        tmp.flush()?;
        tmp.sync_all()?;
    }
    set_owner_readwrite(&tmp_path);

    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&tmp_path, path)?;
    set_owner_readwrite(path);
    Ok(())
}

fn tmp_path_for(path: &Path) -> std::path::PathBuf {
    match path.file_name() {
        Some(name) => {
            let mut tmp_name = name.to_os_string();
            tmp_name.push(".tmp");
            path.with_file_name(tmp_name)
        }
        None => path.with_extension("tmp"),
    }
}

fn set_owner_readwrite(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample() -> VaultFile {
        VaultFile {
            salt: [7u8; SALT_LEN],
            nonce: [9u8; NONCE_LEN],
            ciphertext: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17],
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let file = sample();
        let decoded = VaultFile::decode(&file.encode()).unwrap();
        assert_eq!(file, decoded);
    }

    #[test]
    fn truncated_file_is_corrupt() {
        assert!(matches!(
            VaultFile::decode(b"PWS"),
            Err(StashError::CorruptVault)
        ));
    }

    #[test]
    fn bad_magic_is_corrupt() {
        let mut bytes = sample().encode();
        bytes[0] = b'X';
        assert!(matches!(
            VaultFile::decode(&bytes),
            Err(StashError::CorruptVault)
        ));
    }

    #[test]
    fn atomic_write_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.stash");
        let file = sample();
        write_vault_file(&path, &file).unwrap();
        let loaded = read_vault_file(&path).unwrap();
        assert_eq!(file, loaded);
    }

    #[test]
    fn read_missing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("missing.stash");
        assert!(matches!(
            read_vault_file(&path),
            Err(StashError::FileNotFound)
        ));
    }

    #[test]
    fn overwrite_existing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.stash");
        write_vault_file(&path, &sample()).unwrap();
        let mut updated = sample();
        updated.ciphertext[0] = 99;
        write_vault_file(&path, &updated).unwrap();
        assert_eq!(read_vault_file(&path).unwrap(), updated);
    }
}
