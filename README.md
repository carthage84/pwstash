# pwstash

Simple encrypted password vault with a CLI and a terminal UI.

The vault is a single local file. A master password derives an Argon2id key; entries are stored as AES-256-GCM encrypted JSON. Nothing is sent over the network.

## Install

Requires current stable Rust (1.97+).

```bash
cargo install --path .
```

Or run from the repo:

```bash
cargo run --release -- init
```

## Vault file

Path is chosen in this order:

1. `-f` / `--file` (global; can sit before or after the subcommand)
2. `PWSTASH_FILE`
3. `./pwstash.stash` if that file already exists
4. Per-user default: `%APPDATA%\pwstash\vault.stash` on Windows, `~/.local/share/pwstash/vault.stash` on Unix

`init` creates missing parent directories.

Format (`PWS1`): 4-byte magic, 16-byte Argon2id salt, 12-byte AES-GCM nonce, then ciphertext. The salt is fixed when the vault is created; a new nonce is used on every save. Writes are atomic (temp file + rename). On Unix the file is created with mode `0600`.

## Secrets

Master and entry passwords are **never** taken from command-line arguments.

- Interactive: hidden prompt (`rpassword`)
- Non-interactive / tests:
  - `PWSTASH_MASTER` — vault password
  - `PWSTASH_NEW_MASTER` — new vault password for `passwd`
  - `PWSTASH_ENTRY_PASSWORD` — password for `add` / `update`

`init` prompts twice unless `PWSTASH_MASTER` is set.

## Commands

```bash
pwstash              # opens the TUI (same as `pwstash gui`)
pwstash init
pwstash add --service github --username me
pwstash add --service github --username me --generate
pwstash add --service github --username me --url https://github.com --notes 2fa
pwstash get --service github
pwstash copy --service github
pwstash list
pwstash find git
pwstash mv --from github --to gh
pwstash update --service github --username me --generate --length 24
pwstash delete --service github --yes
pwstash passwd
pwstash gui

# or an explicit file
pwstash -f vault.stash init
pwstash add -f vault.stash --service github --username me
```

`list` prints service, username, and URL if set, sorted alphabetically by service. `find <query>` filters the same fields as TUI search (service, username, URL, notes). `mv --from a --to b` renames a service. `get` prints username, password, and any URL/notes. Prefer `copy` when you just need to paste the password: it places the secret on the clipboard, never prints it, waits 30 seconds, then overwrites the clipboard. Ctrl-C during that wait still clears the clipboard.

`--url` and `--notes` are optional on `add` / `update`. Omitted flags on `update` leave the existing values. Older vaults without those fields still load; they default to empty.

`--generate` on `add` / `update` creates a random password (letters, digits, symbols; default length 20, `--length` 8–128) and does not print it. `delete` asks you to type `y` and press Enter unless you pass `--yes`. Non-interactive runs (no TTY) require `--yes`.

`passwd` unlocks with the current master password, then prompts twice for a new one (or `PWSTASH_NEW_MASTER`). The vault is rewritten with a fresh salt.

If the vault file does not exist, commands that need it fail with a hint to run `pwstash init`. Nothing is created automatically.

## Terminal UI

`pwstash` with no subcommand opens the TUI on the default vault. `pwstash gui` does the same.

After 2 minutes with no keys, the TUI locks: entries and the derived key are wiped from memory, the clipboard is cleared, and you must type the master password to unlock. `q` on the lock screen quits.

```bash
pwstash
pwstash gui -f vault.stash
```

| Key | Action |
| --- | --- |
| `j` / `↓`, `k` / `↑` | Move selection |
| `/` | Search (filter as you type) |
| `Enter` or `p` | Toggle password reveal |
| `c` | Copy password (clipboard clears after 30s) |
| `y` | Copy username (clipboard clears after 30s) |
| `g` | Add entry with a generated password |
| `Ctrl-G` | Generate a password in the add/edit form |
| `?` | Key help |
| `a` | Add entry |
| `e` | Edit selected entry (blank password keeps the current one) |
| `r` | Rename selected service |
| `d` | Delete with confirmation |
| `P` | Change master password |
| `q` / `Ctrl-C` | Quit (`Esc` backs out of a form first) |

## Threat model

pwstash protects a vault at rest on disk against someone who does not know the master password. It does not protect against a compromised account, malware, or an unlocked terminal session. Passwords copied to the clipboard (`pwstash copy` or TUI `c`) live there until the 30-second clear, a Ctrl-C during the CLI wait (which still clears), or until you copy something else.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```
