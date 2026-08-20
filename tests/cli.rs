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
        .args(["--service", "github", "--yes"])
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
fn add_get_shows_url_and_notes() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .env("PWSTASH_ENTRY_PASSWORD", "hunter2")
        .args(["add", "-f"])
        .arg(vault_file(&dir))
        .args([
            "--service",
            "mail",
            "--username",
            "ada",
            "--url",
            "https://mail.example",
            "--notes",
            "work",
        ])
        .assert()
        .success();

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["get", "-f"])
        .arg(vault_file(&dir))
        .args(["--service", "mail"])
        .assert()
        .success()
        .stdout(predicate::str::contains("URL: https://mail.example"))
        .stdout(predicate::str::contains("Notes: work"));

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["list", "-f"])
        .arg(vault_file(&dir))
        .assert()
        .success()
        .stdout(predicate::str::contains("https://mail.example"));
}

#[test]
fn add_generate_does_not_print_password() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["add", "-f"])
        .arg(vault_file(&dir))
        .args([
            "--service",
            "generated",
            "--username",
            "ada",
            "--generate",
            "--length",
            "16",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("generated password"));

    let output = bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["get", "-f"])
        .arg(vault_file(&dir))
        .args(["--service", "generated"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let password = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Password: "))
        .expect("password line");
    assert_eq!(password.len(), 16);
}

#[test]
fn delete_without_yes_fails_when_stdin_is_not_a_tty() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "github", "ada", "hunter2");
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["delete", "-f"])
        .arg(vault_file(&dir))
        .args(["--service", "github"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));
}

#[test]
fn backup_export_import() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "github", "ada", "hunter2");
    add(&dir, "mail", "bob", "secret");

    let backup = dir.path().join("vault.bak");
    bin()
        .args(["backup", "-f"])
        .arg(vault_file(&dir))
        .args(["-o"])
        .arg(&backup)
        .assert()
        .success();
    assert!(backup.exists());

    let dest = dir.path().join("other.stash");
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .env("PWSTASH_DEST_MASTER", "dest-secret")
        .args(["export", "-f"])
        .arg(vault_file(&dir))
        .args(["-o"])
        .arg(&dest)
        .args(["--service", "github"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Copied 1"));

    bin()
        .env("PWSTASH_MASTER", "dest-secret")
        .args(["get", "-f"])
        .arg(&dest)
        .args(["--service", "github"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Password: hunter2"));

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["list", "-f"])
        .arg(vault_file(&dir))
        .assert()
        .success()
        .stdout(predicate::str::contains("github"));

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .env("PWSTASH_DEST_MASTER", "dest-secret")
        .args(["export", "-f"])
        .arg(vault_file(&dir))
        .args(["-o"])
        .arg(&dest)
        .args(["--service", "mail", "--move", "--on-conflict", "skip"])
        .assert()
        .success();

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["list", "-f"])
        .arg(vault_file(&dir))
        .assert()
        .success()
        .stdout(predicate::str::contains("mail").not());

    let third = dir.path().join("third.stash");
    bin()
        .env("PWSTASH_MASTER", "third-secret")
        .args(["init", "-f"])
        .arg(&third)
        .assert()
        .success();
    bin()
        .env("PWSTASH_MASTER", "third-secret")
        .env("PWSTASH_ENTRY_PASSWORD", "other")
        .args(["add", "-f"])
        .arg(&third)
        .args(["--service", "github", "--username", "eve"])
        .assert()
        .success();
    bin()
        .env("PWSTASH_MASTER", "third-secret")
        .env("PWSTASH_SOURCE_MASTER", "dest-secret")
        .args(["import", "-f"])
        .arg(&third)
        .args(["--from"])
        .arg(&dest)
        .args(["--all", "--on-conflict", "skip"])
        .assert()
        .success()
        .stdout(predicate::str::contains("skipped"));
}

#[test]
fn find_and_rename() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "GitHub", "ada", "hunter2");
    add(&dir, "mail", "bob", "secret");

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["find", "-f"])
        .arg(vault_file(&dir))
        .arg("git")
        .assert()
        .success()
        .stdout(predicate::str::contains("GitHub"))
        .stdout(predicate::str::contains("mail").not());

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["find", "-f"])
        .arg(vault_file(&dir))
        .arg("nope")
        .assert()
        .success()
        .stdout(predicate::str::contains("No matching entries."));

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["mv", "-f"])
        .arg(vault_file(&dir))
        .args(["--from", "github", "--to", "gh"])
        .assert()
        .success();

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["list", "-f"])
        .arg(vault_file(&dir))
        .assert()
        .success()
        .stdout(predicate::str::contains("gh"))
        .stdout(predicate::str::contains("GitHub").not());
}

#[test]
fn passwd_changes_master_and_keeps_entries() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "github", "ada", "hunter2");
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .env("PWSTASH_NEW_MASTER", "fresh-secret")
        .args(["passwd", "-f"])
        .arg(vault_file(&dir))
        .assert()
        .success()
        .stdout(predicate::str::contains("Master password updated"));

    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["list", "-f"])
        .arg(vault_file(&dir))
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid master password"));

    bin()
        .env("PWSTASH_MASTER", "fresh-secret")
        .args(["get", "-f"])
        .arg(vault_file(&dir))
        .args(["--service", "github"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Password: hunter2"));
}

#[test]
fn delete_missing_service_skips_prompt() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    bin()
        .env("PWSTASH_MASTER", "master-secret")
        .args(["delete", "-f"])
        .arg(vault_file(&dir))
        .args(["--service", "missing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no entry for missing"));
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
