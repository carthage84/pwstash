use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::encryption::{self, KEY_LEN, SALT_LEN};
use crate::error::StashError;
use crate::persistence::{self, VaultFile};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct PasswordEntry {
    pub service: String,
    pub username: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictDecision {
    Skip,
    Overwrite,
    Abort,
    Rename(String),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TransferReport {
    pub copied: Vec<String>,
    pub overwritten: Vec<String>,
    pub skipped: Vec<String>,
    /// Original source name and the free name it was stored under.
    pub renamed: Vec<(String, String)>,
}

impl PasswordEntry {
    pub fn matches(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        self.service.to_lowercase().contains(&q)
            || self.username.to_lowercase().contains(&q)
            || self.url.to_lowercase().contains(&q)
            || self.notes.to_lowercase().contains(&q)
    }

    pub fn equivalent(&self, other: &Self) -> bool {
        self.username == other.username
            && self.password == other.password
            && self.url == other.url
            && self.notes == other.notes
    }

    pub fn diff_lines(&self, incoming: &Self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.username != incoming.username {
            lines.push(format!(
                "username: {} -> {}",
                self.username, incoming.username
            ));
        }
        if self.password != incoming.password {
            lines.push("password: differs".to_string());
        }
        if self.url != incoming.url {
            lines.push(format!(
                "url: {} -> {}",
                display_blank(&self.url),
                display_blank(&incoming.url)
            ));
        }
        if self.notes != incoming.notes {
            lines.push(format!(
                "notes: {} -> {}",
                display_blank(&self.notes),
                display_blank(&incoming.notes)
            ));
        }
        lines
    }
}

fn display_blank(value: &str) -> &str {
    if value.is_empty() { "(empty)" } else { value }
}

impl std::fmt::Debug for PasswordEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PasswordEntry")
            .field("service", &self.service)
            .field("username", &self.username)
            .field("url", &self.url)
            .field("password", &"***")
            .finish()
    }
}

pub struct Vault {
    path: PathBuf,
    salt: [u8; SALT_LEN],
    key: Zeroizing<[u8; KEY_LEN]>,
    entries: Vec<PasswordEntry>,
    locked: bool,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault")
            .field("path", &self.path)
            .field("len", &self.entries.len())
            .finish_non_exhaustive()
    }
}

impl Vault {
    pub fn create(path: impl AsRef<Path>, master_password: &str) -> Result<Self, StashError> {
        let path = path.as_ref().to_path_buf();
        if path.exists() {
            return Err(StashError::FileAlreadyExists {
                path: path.display().to_string(),
            });
        }
        if master_password.is_empty() {
            return Err(StashError::EmptyField {
                field: "master password",
            });
        }
        let salt = encryption::generate_salt()?;
        let key = Zeroizing::new(encryption::derive_key(master_password, &salt)?);
        let vault = Self {
            path,
            salt,
            key,
            entries: Vec::new(),
            locked: false,
        };
        vault.save()?;
        Ok(vault)
    }

    pub fn open(path: impl AsRef<Path>, master_password: &str) -> Result<Self, StashError> {
        let path = path.as_ref().to_path_buf();
        let file = persistence::read_vault_file(&path)?;
        let key = Zeroizing::new(encryption::derive_key(master_password, &file.salt)?);
        let plaintext = match encryption::decrypt(&file.nonce, &file.ciphertext, &key) {
            Ok(bytes) => bytes,
            Err(_) => return Err(StashError::InvalidMasterPassword),
        };
        let mut entries: Vec<PasswordEntry> = serde_json::from_slice(&plaintext)?;
        sort_entries(&mut entries);
        Ok(Self {
            path,
            salt: file.salt,
            key,
            entries,
            locked: false,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn entries(&self) -> &[PasswordEntry] {
        &self.entries
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn lock(&mut self) {
        for entry in &mut self.entries {
            entry.zeroize();
        }
        self.entries.clear();
        self.key = Zeroizing::new([0u8; KEY_LEN]);
        self.locked = true;
    }

    pub fn unlock(&mut self, master_password: &str) -> Result<(), StashError> {
        *self = Self::open(&self.path, master_password)?;
        Ok(())
    }

    pub fn save(&self) -> Result<(), StashError> {
        self.ensure_unlocked()?;
        let plaintext = serde_json::to_vec(&self.entries)?;
        let (nonce, ciphertext) = encryption::encrypt(&plaintext, &self.key)?;
        persistence::write_vault_file(
            &self.path,
            &VaultFile {
                salt: self.salt,
                nonce,
                ciphertext,
            },
        )
    }

    pub fn add(&mut self, service: &str, username: &str, password: &str) -> Result<(), StashError> {
        self.add_full(service, username, password, "", "")
    }

    pub fn add_full(
        &mut self,
        service: &str,
        username: &str,
        password: &str,
        url: &str,
        notes: &str,
    ) -> Result<(), StashError> {
        self.ensure_unlocked()?;
        let service = require_trimmed(service, "service")?;
        let username = require_trimmed(username, "username")?;
        require_password(password)?;
        if self
            .entries
            .iter()
            .any(|e| e.service.eq_ignore_ascii_case(&service))
        {
            return Err(StashError::DuplicateService {
                service: service.clone(),
            });
        }
        self.entries.push(PasswordEntry {
            service,
            username,
            password: password.to_string(),
            url: optional_text(url),
            notes: optional_text(notes),
        });
        sort_entries(&mut self.entries);
        self.save()
    }

    pub fn selected_entries(&self, names: &[String]) -> Result<Vec<PasswordEntry>, StashError> {
        self.ensure_unlocked()?;
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            match self.get(name) {
                Some(entry) => out.push(entry.clone()),
                None => {
                    return Err(StashError::ServiceNotFound {
                        service: name.clone(),
                    });
                }
            }
        }
        Ok(out)
    }

    pub fn unused_name(&self, base: &str) -> String {
        let base = base.trim();
        (1u32..)
            .map(|n| format!("{base}-{n}"))
            .find(|candidate| self.get(candidate).is_none())
            .expect("exhausted numeric suffixes")
    }

    pub fn ingest(
        &mut self,
        incoming: Vec<PasswordEntry>,
        mut on_conflict: impl FnMut(
            &PasswordEntry,
            &PasswordEntry,
            &str,
        ) -> Result<ConflictDecision, StashError>,
    ) -> Result<TransferReport, StashError> {
        self.ensure_unlocked()?;
        let mut report = TransferReport::default();
        let mut ready = Vec::new();
        let mut conflicts = Vec::new();
        for entry in incoming {
            match self.get(&entry.service) {
                Some(existing) if existing.equivalent(&entry) => {
                    report.skipped.push(entry.service.clone());
                }
                Some(existing) => conflicts.push((existing.clone(), entry)),
                None => ready.push(entry),
            }
        }
        for entry in ready {
            report.copied.push(entry.service.clone());
            self.entries.push(entry);
        }
        if !report.copied.is_empty() {
            sort_entries(&mut self.entries);
            self.save()?;
        }
        for (existing, incoming) in conflicts {
            let service = incoming.service.clone();
            let proposed = self.unused_name(&service);
            match on_conflict(&existing, &incoming, &proposed)? {
                ConflictDecision::Skip => report.skipped.push(service),
                ConflictDecision::Overwrite => {
                    self.replace_entry(incoming);
                    report.overwritten.push(service);
                    self.save()?;
                }
                ConflictDecision::Rename(new_name) => {
                    report
                        .renamed
                        .push(self.absorb_renamed(incoming, &new_name)?);
                }
                ConflictDecision::Abort => return Err(StashError::TransferAborted),
            }
        }
        Ok(report)
    }

    pub(crate) fn absorb_new(&mut self, entry: PasswordEntry) {
        self.entries.push(entry);
        sort_entries(&mut self.entries);
    }

    pub(crate) fn replace_entry(&mut self, entry: PasswordEntry) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|e| e.service.eq_ignore_ascii_case(&entry.service))
        {
            self.entries[index] = entry;
        } else {
            self.entries.push(entry);
        }
        sort_entries(&mut self.entries);
    }

    pub(crate) fn absorb_renamed(
        &mut self,
        mut entry: PasswordEntry,
        new_name: &str,
    ) -> Result<(String, String), StashError> {
        self.ensure_unlocked()?;
        let new_name = require_trimmed(new_name, "service")?;
        if self.get(&new_name).is_some() {
            return Err(StashError::DuplicateService { service: new_name });
        }
        let original = entry.service.clone();
        entry.service = new_name.clone();
        self.entries.push(entry);
        sort_entries(&mut self.entries);
        self.save()?;
        Ok((original, new_name))
    }

    pub fn find(&self, query: &str) -> Vec<&PasswordEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.matches(query))
            .collect()
    }

    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), StashError> {
        self.ensure_unlocked()?;
        let to = require_trimmed(to, "service")?;
        let index = self
            .entries
            .iter()
            .position(|e| e.service.eq_ignore_ascii_case(from))
            .ok_or_else(|| StashError::ServiceNotFound {
                service: from.to_string(),
            })?;
        if self
            .entries
            .iter()
            .enumerate()
            .any(|(i, entry)| i != index && entry.service.eq_ignore_ascii_case(&to))
        {
            return Err(StashError::DuplicateService {
                service: to.clone(),
            });
        }
        self.entries[index].service = to;
        sort_entries(&mut self.entries);
        self.save()
    }

    pub fn get(&self, service: &str) -> Option<&PasswordEntry> {
        self.entries
            .iter()
            .find(|e| e.service.eq_ignore_ascii_case(service))
    }

    pub fn update(
        &mut self,
        service: &str,
        username: &str,
        password: &str,
    ) -> Result<(), StashError> {
        self.update_full(service, username, Some(password), None, None)
    }

    pub fn update_full(
        &mut self,
        service: &str,
        username: &str,
        password: Option<&str>,
        url: Option<&str>,
        notes: Option<&str>,
    ) -> Result<(), StashError> {
        self.ensure_unlocked()?;
        let username = require_trimmed(username, "username")?;
        if let Some(password) = password {
            require_password(password)?;
        }
        let index = self
            .entries
            .iter()
            .position(|e| e.service.eq_ignore_ascii_case(service))
            .ok_or_else(|| StashError::ServiceNotFound {
                service: service.to_string(),
            })?;
        self.entries[index].username = username;
        if let Some(password) = password {
            self.entries[index].password = password.to_string();
        }
        if let Some(url) = url {
            self.entries[index].url = optional_text(url);
        }
        if let Some(notes) = notes {
            self.entries[index].notes = optional_text(notes);
        }
        self.save()
    }

    pub fn change_master(&mut self, new_password: &str) -> Result<(), StashError> {
        self.ensure_unlocked()?;
        if new_password.is_empty() {
            return Err(StashError::EmptyField {
                field: "master password",
            });
        }
        let salt = encryption::generate_salt()?;
        let key = Zeroizing::new(encryption::derive_key(new_password, &salt)?);
        self.salt = salt;
        self.key = key;
        self.save()
    }

    pub fn delete(&mut self, service: &str) -> Result<(), StashError> {
        self.ensure_unlocked()?;
        let original = self.entries.len();
        self.entries
            .retain(|e| !e.service.eq_ignore_ascii_case(service));
        if self.entries.len() == original {
            return Err(StashError::ServiceNotFound {
                service: service.to_string(),
            });
        }
        sort_entries(&mut self.entries);
        self.save()
    }

    fn ensure_unlocked(&self) -> Result<(), StashError> {
        if self.locked {
            Err(StashError::VaultLocked)
        } else {
            Ok(())
        }
    }
}

fn require_trimmed(value: &str, field: &'static str) -> Result<String, StashError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(StashError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn sort_entries(entries: &mut [PasswordEntry]) {
    entries.sort_by(|a, b| {
        a.service
            .to_lowercase()
            .cmp(&b.service.to_lowercase())
            .then_with(|| a.username.to_lowercase().cmp(&b.username.to_lowercase()))
    });
}

fn optional_text(value: &str) -> String {
    value.trim().to_string()
}

fn require_password(password: &str) -> Result<(), StashError> {
    if password.is_empty() {
        Err(StashError::EmptyField { field: "password" })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn vault() -> (Vault, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.stash");
        let v = Vault::create(&path, "master-secret").unwrap();
        (v, dir)
    }

    #[test]
    fn create_open_empty() {
        let (v, _dir) = vault();
        let opened = Vault::open(v.path(), "master-secret").unwrap();
        assert!(opened.entries().is_empty());
    }

    #[test]
    fn create_existing_fails() {
        let (v, _dir) = vault();
        let err = Vault::create(v.path(), "master-secret").unwrap_err();
        assert!(matches!(err, StashError::FileAlreadyExists { .. }));
    }

    #[test]
    fn open_missing_fails() {
        let dir = TempDir::new().unwrap();
        let err = Vault::open(dir.path().join("nope.stash"), "x").unwrap_err();
        assert!(matches!(err, StashError::FileNotFound));
    }

    #[test]
    fn wrong_password_fails_closed() {
        let (v, _dir) = vault();
        let err = Vault::open(v.path(), "wrong").unwrap_err();
        assert!(matches!(err, StashError::InvalidMasterPassword));
    }

    #[test]
    fn add_get_update_delete() {
        let (mut v, _dir) = vault();
        v.add("GitHub", "me", "hunter2").unwrap();
        let got = v.get("github").unwrap();
        assert_eq!(got.username, "me");
        assert_eq!(got.password, "hunter2");

        v.update("github", "other", "newpass").unwrap();
        let got = v.get("GitHub").unwrap();
        assert_eq!(got.username, "other");
        assert_eq!(got.password, "newpass");

        v.delete("GITHUB").unwrap();
        assert!(v.get("github").is_none());
    }

    #[test]
    fn duplicate_service_rejected() {
        let (mut v, _dir) = vault();
        v.add("github", "me", "a").unwrap();
        let err = v.add("GitHub", "you", "b").unwrap_err();
        assert!(matches!(err, StashError::DuplicateService { .. }));
    }

    #[test]
    fn update_missing_service() {
        let (mut v, _dir) = vault();
        let err = v.update("nope", "u", "p").unwrap_err();
        assert!(matches!(err, StashError::ServiceNotFound { .. }));
    }

    #[test]
    fn delete_missing_service() {
        let (mut v, _dir) = vault();
        let err = v.delete("nope").unwrap_err();
        assert!(matches!(err, StashError::ServiceNotFound { .. }));
    }

    #[test]
    fn empty_fields_rejected() {
        let (mut v, _dir) = vault();
        assert!(matches!(
            v.add("  ", "u", "p"),
            Err(StashError::EmptyField { field: "service" })
        ));
        assert!(matches!(
            v.add("s", "  ", "p"),
            Err(StashError::EmptyField { field: "username" })
        ));
        assert!(matches!(
            v.add("s", "u", ""),
            Err(StashError::EmptyField { field: "password" })
        ));
    }

    #[test]
    fn url_and_notes_roundtrip() {
        let (mut v, _dir) = vault();
        v.add_full("mail", "ada", "pw", "https://mail.example", "work account")
            .unwrap();
        let opened = Vault::open(v.path(), "master-secret").unwrap();
        let entry = opened.get("mail").unwrap();
        assert_eq!(entry.url, "https://mail.example");
        assert_eq!(entry.notes, "work account");
    }

    #[test]
    fn find_matches_service_and_notes() {
        let (mut v, _dir) = vault();
        v.add_full("GitHub", "ada", "pw", "", "2fa work").unwrap();
        v.add("mail", "bob", "pw").unwrap();
        assert_eq!(v.find("git").len(), 1);
        assert_eq!(v.find("2fa").len(), 1);
        assert_eq!(v.find("nope").len(), 0);
        assert_eq!(v.find("").len(), 2);
    }

    #[test]
    fn rename_service_and_reject_duplicate() {
        let (mut v, _dir) = vault();
        v.add("GitHub", "ada", "pw").unwrap();
        v.add("mail", "bob", "pw").unwrap();
        v.rename("github", "gh").unwrap();
        assert!(v.get("github").is_none());
        assert_eq!(v.get("gh").unwrap().username, "ada");
        let err = v.rename("gh", "mail").unwrap_err();
        assert!(matches!(err, StashError::DuplicateService { .. }));
        v.rename("gh", "GH").unwrap();
        assert_eq!(v.get("GH").unwrap().service, "GH");
    }

    #[test]
    fn ingest_copy_skip_overwrite() {
        let (mut src, dir) = vault();
        src.add_full("mail", "ada", "old", "https://a", "n1")
            .unwrap();
        src.add("extra", "bob", "pw").unwrap();
        let dest_path = dir.path().join("dest.stash");
        let mut dest = Vault::create(&dest_path, "dest-secret").unwrap();
        dest.add_full("mail", "ada", "old", "https://a", "n1")
            .unwrap();
        dest.add_full("clash", "x", "pw", "", "old note").unwrap();
        src.add_full("clash", "y", "pw", "", "new note").unwrap();

        let incoming = src.entries().to_vec();
        let report = dest
            .ingest(incoming, |_, _, _| Ok(ConflictDecision::Overwrite))
            .unwrap();
        assert!(
            report
                .skipped
                .iter()
                .any(|s| s.eq_ignore_ascii_case("mail"))
        );
        assert!(report.copied.iter().any(|s| s == "extra"));
        assert!(report.overwritten.iter().any(|s| s == "clash"));
        assert_eq!(dest.get("clash").unwrap().username, "y");
        assert_eq!(dest.get("extra").unwrap().username, "bob");
    }

    #[test]
    fn unused_name_skips_taken_numbers() {
        let (mut v, _dir) = vault();
        v.add("github", "ada", "pw").unwrap();
        v.add("github-1", "ada", "pw").unwrap();
        v.add("github-3", "ada", "pw").unwrap();
        assert_eq!(v.unused_name("github"), "github-2");
        assert_eq!(v.unused_name("GitHub"), "GitHub-2");
        assert_eq!(v.unused_name("mail"), "mail-1");
    }

    #[test]
    fn ingest_rename_uses_free_name() {
        let (mut src, dir) = vault();
        src.add_full("github", "eve", "pw2", "https://gh", "incoming")
            .unwrap();
        let dest_path = dir.path().join("dest.stash");
        let mut dest = Vault::create(&dest_path, "dest-secret").unwrap();
        dest.add_full("github", "ada", "pw", "", "keep me").unwrap();
        dest.add("github-1", "bob", "pw").unwrap();

        let incoming = src.entries().to_vec();
        let report = dest
            .ingest(incoming, |_, _, proposed| {
                assert_eq!(proposed, "github-2");
                Ok(ConflictDecision::Rename(proposed.to_string()))
            })
            .unwrap();
        assert_eq!(
            report.renamed,
            vec![("github".to_string(), "github-2".to_string())]
        );
        assert_eq!(dest.get("github").unwrap().username, "ada");
        assert_eq!(dest.get("github-2").unwrap().username, "eve");
        assert_eq!(dest.get("github-2").unwrap().url, "https://gh");
    }

    #[test]
    fn list_order_is_alphabetical() {
        let (mut v, _dir) = vault();
        v.add("zeta", "a", "p").unwrap();
        v.add("Alpha", "a", "p").unwrap();
        let names: Vec<_> = v.entries().iter().map(|e| e.service.as_str()).collect();
        assert_eq!(names, ["Alpha", "zeta"]);
    }

    #[test]
    fn missing_url_notes_default_empty() {
        let json = r#"[{"service":"s","username":"u","password":"p"}]"#;
        let entries: Vec<PasswordEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries[0].url, "");
        assert_eq!(entries[0].notes, "");
    }

    #[test]
    fn update_preserves_url_unless_passed() {
        let (mut v, _dir) = vault();
        v.add_full("mail", "ada", "pw", "https://a.example", "note")
            .unwrap();
        v.update("mail", "ada", "pw2").unwrap();
        let entry = v.get("mail").unwrap();
        assert_eq!(entry.url, "https://a.example");
        assert_eq!(entry.notes, "note");
        v.update_full("mail", "ada", Some("pw3"), None, Some("new note"))
            .unwrap();
        let entry = v.get("mail").unwrap();
        assert_eq!(entry.url, "https://a.example");
        assert_eq!(entry.notes, "new note");
    }

    #[test]
    fn update_keeps_password_when_omitted() {
        let (mut v, _dir) = vault();
        v.add_full("mail", "ada", "secret", "https://a.example", "note")
            .unwrap();
        v.update_full("mail", "grace", None, Some("https://b.example"), None)
            .unwrap();
        let entry = v.get("mail").unwrap();
        assert_eq!(entry.username, "grace");
        assert_eq!(entry.password, "secret");
        assert_eq!(entry.url, "https://b.example");
        assert_eq!(entry.notes, "note");
    }

    #[test]
    fn persist_across_open() {
        let (mut v, _dir) = vault();
        v.add("mail", "ada", "pw").unwrap();
        let opened = Vault::open(v.path(), "master-secret").unwrap();
        assert_eq!(opened.entries().len(), 1);
        assert_eq!(opened.entries()[0].service, "mail");
        assert_eq!(opened.entries()[0].password, "pw");
    }

    #[test]
    fn change_master_reencrypts() {
        let (mut v, _dir) = vault();
        v.add("mail", "ada", "pw").unwrap();
        v.change_master("new-master-secret").unwrap();
        let err = Vault::open(v.path(), "master-secret").unwrap_err();
        assert!(matches!(err, StashError::InvalidMasterPassword));
        let opened = Vault::open(v.path(), "new-master-secret").unwrap();
        assert_eq!(opened.get("mail").unwrap().password, "pw");
    }

    #[test]
    fn lock_wipes_then_unlock_restores() {
        let (mut v, _dir) = vault();
        v.add("mail", "ada", "pw").unwrap();
        v.lock();
        assert!(v.is_locked());
        assert!(v.entries().is_empty());
        assert!(matches!(v.add("x", "y", "z"), Err(StashError::VaultLocked)));
        v.unlock("master-secret").unwrap();
        assert!(!v.is_locked());
        assert_eq!(v.get("mail").unwrap().password, "pw");
    }

    #[test]
    fn lock_wrong_password_stays_locked() {
        let (mut v, _dir) = vault();
        v.lock();
        assert!(matches!(
            v.unlock("nope"),
            Err(StashError::InvalidMasterPassword)
        ));
        assert!(v.is_locked());
    }

    #[test]
    fn change_master_rejects_empty() {
        let (mut v, _dir) = vault();
        assert!(matches!(
            v.change_master(""),
            Err(StashError::EmptyField {
                field: "master password"
            })
        ));
    }

    #[test]
    fn tampered_file_fails() {
        let (v, _dir) = vault();
        let mut bytes = std::fs::read(v.path()).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xaa;
        std::fs::write(v.path(), bytes).unwrap();
        let err = Vault::open(v.path(), "master-secret").unwrap_err();
        assert!(matches!(err, StashError::InvalidMasterPassword));
    }
}
