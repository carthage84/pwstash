use std::io;
use std::path::Path;
use std::process::ExitCode;

use anyhow::Context;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use pwstash::app::App;
use pwstash::args::{CommandLineArgs, Commands};
use pwstash::event::Event;
use pwstash::handler::handle_key_events;
use pwstash::master_password::{
    read_entry_password, read_master_password, read_new_master_password,
};
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
    match cli.command {
        Commands::Init { file } => {
            let master = read_new_master_password()?;
            Vault::create(&file, &master)?;
            println!("Created vault {}", file.display());
        }
        Commands::Add {
            file,
            service,
            username,
        } => {
            let mut vault = open_vault(&file)?;
            let password = read_entry_password()?;
            vault.add(&service, &username, &password)?;
            println!("Added {service}");
        }
        Commands::Get { file, service } => {
            let vault = open_vault(&file)?;
            match vault.get(&service) {
                Some(entry) => {
                    println!("Username: {}", entry.username);
                    println!("Password: {}", entry.password);
                }
                None => println!("No password found for {service}"),
            }
        }
        Commands::List { file } => {
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
            file,
            service,
            username,
        } => {
            let mut vault = open_vault(&file)?;
            let password = read_entry_password()?;
            vault.update(&service, &username, &password)?;
            println!("Updated {service}");
        }
        Commands::Delete { file, service } => {
            let mut vault = open_vault(&file)?;
            vault.delete(&service)?;
            println!("Deleted {service}");
        }
        Commands::Gui { file } => {
            let vault = open_vault(&file)?;
            run_gui(vault)?;
        }
    }
    Ok(())
}

fn open_vault(path: &Path) -> anyhow::Result<Vault> {
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
