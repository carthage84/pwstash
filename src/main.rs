use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;

use anyhow::Context;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use pwstash::app::App;
use pwstash::args::{CommandLineArgs, Commands};
use pwstash::clipboard;
use pwstash::event::Event;
use pwstash::generate;
use pwstash::handler::handle_key_events;
use pwstash::master_password::{
    read_entry_password, read_master_password, read_new_master_password,
};
use pwstash::paths;
use pwstash::tui::Tui;
use pwstash::vault::Vault;

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("Error: {err:#}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run() -> anyhow::Result<()> {
    let cli = CommandLineArgs::parse();
    let file = paths::resolve_vault_path(cli.file.as_deref());
    match cli.command.unwrap_or(Commands::Gui) {
        Commands::Init => {
            let master = read_new_master_password()?;
            Vault::create(&file, &master)?;
            println!("Created vault {}", file.display());
        }
        Commands::Add {
            service,
            username,
            generate: generate_flag,
            length,
        } => {
            let mut vault = open_vault(&file)?;
            let password = entry_password(generate_flag, length)?;
            vault.add(&service, &username, &password)?;
            if generate_flag {
                println!("Added {service} with a generated password");
            } else {
                println!("Added {service}");
            }
        }
        Commands::Get { service } => {
            let vault = open_vault(&file)?;
            match vault.get(&service) {
                Some(entry) => {
                    println!("Username: {}", entry.username);
                    println!("Password: {}", entry.password);
                }
                None => println!("No password found for {service}"),
            }
        }
        Commands::Copy { service } => {
            let vault = open_vault(&file)?;
            let password = vault
                .get(&service)
                .ok_or_else(|| pwstash::error::StashError::ServiceNotFound {
                    service: service.clone(),
                })?
                .password
                .clone();
            clipboard::copy_text(&password)?;
            let secs = clipboard::clipboard_ttl().as_secs().max(1);
            println!("Copied password for {service}. Clearing clipboard in {secs}s...");
            clipboard::wait_then_clear()?;
            println!("Clipboard cleared.");
        }
        Commands::List => {
            let vault = open_vault(&file)?;
            if vault.entries().is_empty() {
                println!("No entries.");
            } else {
                for entry in vault.entries() {
                    println!("{}\t{}", entry.service, entry.username);
                }
            }
        }
        Commands::Update {
            service,
            username,
            generate: generate_flag,
            length,
        } => {
            let mut vault = open_vault(&file)?;
            let password = entry_password(generate_flag, length)?;
            vault.update(&service, &username, &password)?;
            if generate_flag {
                println!("Updated {service} with a generated password");
            } else {
                println!("Updated {service}");
            }
        }
        Commands::Delete { service, yes } => {
            let mut vault = open_vault(&file)?;
            if vault.get(&service).is_none() {
                return Err(pwstash::error::StashError::ServiceNotFound {
                    service: service.clone(),
                }
                .into());
            }
            if !confirm_delete(&service, yes)? {
                println!("Aborted.");
                return Ok(());
            }
            vault.delete(&service)?;
            println!("Deleted {service}");
        }
        Commands::Gui => {
            let vault = open_vault(&file)?;
            run_gui(vault)?;
        }
    }
    Ok(())
}

fn entry_password(generate_flag: bool, length: usize) -> anyhow::Result<String> {
    if generate_flag {
        Ok(generate::generate(length)?)
    } else {
        Ok(read_entry_password()?)
    }
}

fn confirm_delete(service: &str, yes: bool) -> anyhow::Result<bool> {
    if yes {
        return Ok(true);
    }
    if !io::stdin().is_terminal() {
        anyhow::bail!("refusing to delete {service} without confirmation. Re-run with --yes.");
    }
    eprintln!("Delete {service}? Type y and press Enter, or n to cancel.");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(matches!(line.trim(), "y" | "Y" | "yes"))
}

fn open_vault(path: &Path) -> anyhow::Result<Vault> {
    if !path.exists() {
        anyhow::bail!(
            "no vault found at {}. Run `pwstash init` to create one.",
            path.display()
        );
    }
    let master = read_master_password()?;
    Vault::open(path, &master).with_context(|| format!("opening {}", path.display()))
}

fn run_gui(vault: Vault) -> anyhow::Result<()> {
    let mut app = App::new(vault);
    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    let events = pwstash::event::EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.init()?;

    while app.running {
        tui.draw(&mut app)?;
        match tui.events.next()? {
            Event::Tick => app.tick(),
            Event::Key(key_event) => handle_key_events(key_event, &mut app)?,
            Event::Paste(text) => app.handle_paste(&text),
            Event::Mouse(_) | Event::Resize(_, _) => {}
        }
    }

    tui.exit()?;
    Ok(())
}
