use std::env;

use crate::error::StashError;

const MASTER_ENV: &str = "PWSTASH_MASTER";
const ENTRY_ENV: &str = "PWSTASH_ENTRY_PASSWORD";

pub fn read_master_password() -> Result<String, StashError> {
    from_env(MASTER_ENV).map_or_else(|| prompt("Master password: "), Ok)
}

pub fn read_new_master_password() -> Result<String, StashError> {
    if let Some(from_env) = from_env(MASTER_ENV) {
        return Ok(from_env);
    }
    let first = prompt("New master password: ")?;
    let second = prompt("Confirm master password: ")?;
    if first != second {
        return Err(StashError::PasswordMismatch);
    }
    if first.is_empty() {
        return Err(StashError::EmptyField {
            field: "master password",
        });
    }
    Ok(first)
}

pub fn read_entry_password() -> Result<String, StashError> {
    from_env(ENTRY_ENV).map_or_else(|| prompt("Entry password: "), Ok)
}

fn from_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.is_empty())
}

fn prompt(message: &str) -> Result<String, StashError> {
    rpassword::prompt_password(message).map_err(StashError::from)
}
