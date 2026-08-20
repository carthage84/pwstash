use std::path::PathBuf;

use clap::{Parser, Subcommand};

pub const DEFAULT_VAULT: &str = "pwstash.stash";

#[derive(Parser, Debug)]
#[command(version, about = "Encrypted password vault", long_about = None)]
pub struct CommandLineArgs {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a new vault
    Init {
        #[arg(short, long, default_value = DEFAULT_VAULT)]
        file: PathBuf,
    },
    /// Add credentials
    Add {
        #[arg(short, long, default_value = DEFAULT_VAULT)]
        file: PathBuf,
        #[arg(short, long)]
        service: String,
        #[arg(short, long)]
        username: String,
    },
    /// Print credentials for a service
    Get {
        #[arg(short, long, default_value = DEFAULT_VAULT)]
        file: PathBuf,
        #[arg(short, long)]
        service: String,
    },
    /// List services and usernames
    List {
        #[arg(short, long, default_value = DEFAULT_VAULT)]
        file: PathBuf,
    },
    /// Update credentials for a service
    Update {
        #[arg(short, long, default_value = DEFAULT_VAULT)]
        file: PathBuf,
        #[arg(short, long)]
        service: String,
        #[arg(short, long)]
        username: String,
    },
    /// Remove credentials for a service
    Delete {
        #[arg(short, long, default_value = DEFAULT_VAULT)]
        file: PathBuf,
        #[arg(short, long)]
        service: String,
    },
    /// Open the terminal UI
    Gui {
        #[arg(short, long, default_value = DEFAULT_VAULT)]
        file: PathBuf,
    },
}

impl CommandLineArgs {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}
