use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about)]
pub struct CommandLineArgs {

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Vault
    Init {
        /// Path to the Vault file
        #[arg(short, long)]
        file: String,

        /// Password to the Vault
        #[arg(short, long)]
        masterpassword: Option<String>
    },
    /// Add new credentials to Vault
    Add {
        /// Path to the Vault file
        file: String,
        /// Name of the stashed service
        service: String,
        /// Username to stashed service
        username: String,
        /// Password to stashed service
        password: String,
        /// Password to the Vault
        masterpassword: Option<String>
    },
    /// Get credentials to selected service
    Get {
        /// Path to the Vault file
        file: String,
        /// Name of the stashed service
        service: String,
        /// Password to the Vault
        masterpassword: Option<String>
    },
    /// Run Graphical User Interface
    Gui {
        /// Path to the Vault file
        file: String,
        /// Password to the Vault
        masterpassword: Option<String>
    },
}

impl CommandLineArgs {
    pub fn parse() -> Self {
        CommandLineArgs::parse_from(std::env::args())
    }
}