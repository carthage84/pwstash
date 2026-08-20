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

Default path: `pwstash.stash` in the current directory. Override with `-f` / `--file`.

Format (`PWS1`): 4-byte magic, 16-byte Argon2id salt, 12-byte AES-GCM nonce, then ciphertext. The salt is fixed when the vault is created; a new nonce is used on every save. Writes are atomic (temp file + rename). On Unix the file is created with mode `0600`.

## Secrets

Master and entry passwords are **never** taken from command-line arguments.

- Interactive: hidden prompt (`rpassword`)
- Non-interactive / tests:
  - `PWSTASH_MASTER` — vault password
  - `PWSTASH_ENTRY_PASSWORD` — password for `add` / `update`

`init` prompts twice unless `PWSTASH_MASTER` is set.

## Commands

```bash
pwstash init -f vault.stash
pwstash add  -f vault.stash --service github --username me
pwstash get  -f vault.stash --service github
pwstash copy -f vault.stash --service github
pwstash list -f vault.stash
pwstash update -f vault.stash --service github --username me
pwstash delete -f vault.stash --service github
pwstash gui  -f vault.stash
```

`list` prints service and username only. `get` prints username and password to the terminal (it will sit in scrollback). Prefer `copy` when you just need to paste the password: it places the secret on the clipboard, never prints it, waits 30 seconds, then overwrites the clipboard. Ctrl-C during that wait still clears the clipboard.

## Terminal UI

```bash
pwstash gui -f vault.stash
```

| Key | Action |
| --- | --- |
| `j` / `↓`, `k` / `↑` | Move selection |
| `/` | Search (filter as you type) |
| `Enter` or `p` | Toggle password reveal |
| `c` | Copy password (clipboard clears after 30s) |
| `a` | Add entry |
| `e` | Edit selected username/password |
| `d` | Delete with confirmation |
| `q` / `Ctrl-C` | Quit (`Esc` backs out of a form first) |

## Threat model

pwstash protects a vault at rest on disk against someone who does not know the master password. It does not protect against a compromised account, malware, or an unlocked terminal session. Passwords copied to the clipboard (`pwstash copy` or TUI `c`) live there until the 30-second clear, a Ctrl-C during the CLI wait (which still clears), or until you copy something else.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```
