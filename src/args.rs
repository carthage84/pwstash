use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Parser, Debug)]
#[command(version, about = "Encrypted password vault", long_about = None)]
pub struct CommandLineArgs {
    /// Path to the vault file
    #[arg(short, long, global = true, value_name = "PATH")]
    pub file: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a new vault
    Init,
    /// Add credentials
    Add {
        #[arg(short, long)]
        service: String,
        #[arg(short, long)]
        username: String,
        /// Generate a random password instead of prompting
        #[arg(long)]
        generate: bool,
        /// Length of a generated password (8–128)
        #[arg(long, default_value_t = crate::generate::DEFAULT_LENGTH)]
        length: usize,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Print credentials for a service
    Get {
        #[arg(short, long)]
        service: String,
    },
    /// Copy a service password to the clipboard (clears after 30s)
    Copy {
        #[arg(short, long)]
        service: String,
    },
    /// List services and usernames
    List,
    /// Find entries by service, username, URL, or notes
    Find { query: String },
    /// Rename a service
    Mv {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
    },
    /// Update credentials for a service
    Update {
        #[arg(short, long)]
        service: String,
        #[arg(short, long)]
        username: String,
        /// Generate a random password instead of prompting
        #[arg(long)]
        generate: bool,
        /// Length of a generated password (8–128)
        #[arg(long, default_value_t = crate::generate::DEFAULT_LENGTH)]
        length: usize,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Remove credentials for a service
    Delete {
        #[arg(short, long)]
        service: String,
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// Change the vault master password
    Passwd,
    /// Copy the encrypted vault file
    Backup {
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Copy or move entries into another vault
    Export {
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        service: Vec<String>,
        #[arg(long)]
        all: bool,
        /// Delete exported entries from this vault
        #[arg(long = "move")]
        move_entries: bool,
        #[arg(long, value_enum, default_value_t = OnConflict::Fail)]
        on_conflict: OnConflict,
    },
    /// Copy entries from another vault into this one
    Import {
        #[arg(long)]
        from: PathBuf,
        #[arg(long)]
        service: Vec<String>,
        #[arg(long)]
        all: bool,
        #[arg(long, value_enum, default_value_t = OnConflict::Fail)]
        on_conflict: OnConflict,
    },
    /// Open the terminal UI
    Gui,
    /// Generate shell completions
    Completions {
        /// Shell to emit completions for
        shell: Shell,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum OnConflict {
    Skip,
    Overwrite,
    Fail,
    Ask,
}

impl CommandLineArgs {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}
