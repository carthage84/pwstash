use std::path::PathBuf;

use clap::{Parser, Subcommand};

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
    /// Open the terminal UI
    Gui,
}

impl CommandLineArgs {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}
