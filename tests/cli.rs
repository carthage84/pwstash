use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("pwstash").unwrap()
}

fn vault_file(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join("vault.stash")
}

fn init(dir: &TempDir) {
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["init", "-f"])
        .arg(vault_file(dir))
        .assert()
        .success()
        .stdout(predicate::str::contains("Created vault"));
}

fn add(dir: &TempDir, service: &str, username: &str, password: &str) {
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .env("PWSTASH_ENTRY_PASSWORD", password)
        .args(["add", "-f"])
        .arg(vault_file(dir))
        .args(["--service", service, "--username", username])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("Added {service}")));
}

#[test]
fn init_add_get_list_update_delete() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "github", "ada", "hunter2");

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["get", "-f"])
        .arg(vault_file(&dir))
        .args(["--service", "github"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Username: ada"))
        .stdout(predicate::str::contains("Password: hunter2"));

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["list", "-f"])
        .arg(vault_file(&dir))
        .assert()
        .success()
        .stdout(predicate::str::contains("github"))
        .stdout(predicate::str::contains("ada"))
        .stdout(predicate::str::contains("hunter2").not());

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .env("PWSTASH_ENTRY_PASSWORD", "newpass")
        .args(["update", "-f"])
        .arg(vault_file(&dir))
        .args(["--service", "github", "--username", "grace"])
        .assert()
        .success();

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["get", "-f"])
        .arg(vault_file(&dir))
        .args(["--service", "github"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Username: grace"))
        .stdout(predicate::str::contains("Password: newpass"));

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["delete", "-f"])
        .arg(vault_file(&dir))
        .args(["--service", "github"])
        .assert()
        .success();

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["list", "-f"])
        .arg(vault_file(&dir))
        .assert()
        .success()
        .stdout(predicate::str::contains("No entries."));
}

#[test]
fn init_existing_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["init", "-f"])
        .arg(vault_file(&dir))
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn wrong_master_password_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    bin()
        .env("PWSTASH_MASTER", "nope")
        .args(["list", "-f"])
        .arg(vault_file(&dir))
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid master password"));
}

#[test]
fn get_missing_service() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["get", "-f"])
        .arg(vault_file(&dir))
        .args(["--service", "missing"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No password found for missing"));
}

#[test]
fn copy_missing_service_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["copy", "-f"])
        .arg(vault_file(&dir))
        .args(["--service", "missing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no entry for missing"))
        .stdout(predicate::str::contains("hunter2").not());
}

#[test]
fn copy_does_not_print_password() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "github", "ada", "hunter2");

    let output = bin()
        .env("PWSTASH_MASTER", "master-secret")
        .env("PWSTASH_CLIPBOARD_TTL_MS", "1")
        .args(["copy", "-f"])
        .arg(vault_file(&dir))
        .args(["--service", "github"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        assert!(
            stderr.to_lowercase().contains("clipboard"),
            "copy failed for a reason other than clipboard: {stderr}"
        );
        return;
    }
    assert!(stdout.contains("Copied password for github"));
    assert!(!stdout.contains("hunter2"));
    assert!(!stderr.contains("hunter2"));
}

#[test]
fn default_file_from_env() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("from-env.stash");
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .env("PWSTASH_FILE", &file)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created vault"));
    assert!(file.exists());
}

#[test]
fn global_file_flag_before_subcommand() {
    let dir = TempDir::new().unwrap();
    let file = vault_file(&dir);
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["-f"])
        .arg(&file)
        .arg("init")
        .assert()
        .success();
    assert!(file.exists());
}

#[test]
fn no_args_missing_vault_hints_init() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("missing.stash");
    bin()
        .env("PWSTASH_FILE", &file)
        .env_remove("PWSTASH_MASTER")
        .assert()
        .failure()
        .stderr(predicate::str::contains("pwstash init"))
        .stderr(predicate::str::contains("missing.stash"));
}

#[test]
fn help_lists_commands_without_subcommand() {
    bin()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("gui"));
}

#[test]
fn existing_local_vault_used_without_flag() {
    let dir = TempDir::new().unwrap();
    let local = dir.path().join("pwstash.stash");
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["init", "-f"])
        .arg(&local)
        .assert()
        .success();
    bin()
        .current_dir(dir.path())
        .env_remove("PWSTASH_FILE")
        .env("PWSTASH_MASTER", "master-secret")
        .env("PWSTASH_ENTRY_PASSWORD", "hunter2")
        .args(["add", "--service", "github", "--username", "ada"])
        .assert()
        .success();
    bin()
        .current_dir(dir.path())
        .env_remove("PWSTASH_FILE")
        .env("PWSTASH_MASTER", "master-secret")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("github"));
}
