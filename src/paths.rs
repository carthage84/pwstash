use std::env;
use std::path::{Path, PathBuf};

pub const LOCAL_VAULT_NAME: &str = "pwstash.stash";
pub const FILE_ENV: &str = "PWSTASH_FILE";

/// Resolve which vault file to use.
///
/// Order: explicit CLI `--file`, then `PWSTASH_FILE`, then `./pwstash.stash`
/// if that file already exists, then the per-user default path.
pub fn resolve_vault_path(cli_file: Option<&Path>) -> PathBuf {
    if let Some(path) = cli_file {
        return path.to_path_buf();
    }
    let env_file = env::var_os(FILE_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    resolve(
        env_file.as_deref(),
        Path::new(LOCAL_VAULT_NAME).exists(),
        &platform_vault_path(),
    )
}

pub fn resolve(env_file: Option<&Path>, local_exists: bool, platform_default: &Path) -> PathBuf {
    if let Some(path) = env_file {
        return path.to_path_buf();
    }
    if local_exists {
        return PathBuf::from(LOCAL_VAULT_NAME);
    }
    platform_default.to_path_buf()
}

pub fn platform_vault_path() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(appdata) = env::var_os("APPDATA").filter(|v| !v.is_empty()) {
            return PathBuf::from(appdata).join("pwstash").join("vault.stash");
        }
    }
    #[cfg(unix)]
    {
        if let Some(home) = env::var_os("HOME").filter(|v| !v.is_empty()) {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("pwstash")
                .join("vault.stash");
        }
    }
    PathBuf::from(LOCAL_VAULT_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_file_wins_over_local_and_platform() {
        let path = resolve(
            Some(Path::new("/tmp/from-env.stash")),
            true,
            Path::new("/home/user/vault.stash"),
        );
        assert_eq!(path, PathBuf::from("/tmp/from-env.stash"));
    }

    #[test]
    fn existing_local_vault_used_when_no_env() {
        let path = resolve(None, true, Path::new("/home/user/vault.stash"));
        assert_eq!(path, PathBuf::from(LOCAL_VAULT_NAME));
    }

    #[test]
    fn platform_default_when_nothing_else() {
        let home = Path::new("/home/user/.local/share/pwstash/vault.stash");
        let path = resolve(None, false, home);
        assert_eq!(path, home);
    }
}
