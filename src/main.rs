use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;

use anyhow::Context;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use pwstash::app::App;
use pwstash::args::{CommandLineArgs, Commands, OnConflict};
use pwstash::clipboard;
use pwstash::error::StashError;
use pwstash::event::Event;
use pwstash::generate;
use pwstash::handler::handle_key_events;
use pwstash::master_password::{
    read_changed_master_password, read_dest_master_password, read_entry_password,
    read_master_password, read_new_master_password, read_source_master_password,
};
use pwstash::paths;
use pwstash::persistence;
use pwstash::tui::Tui;
use pwstash::vault::Vault;
use pwstash::vault::{ConflictDecision, PasswordEntry};

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
            url,
            notes,
        } => {
            let mut vault = open_vault(&file)?;
            let password = entry_password(generate_flag, length)?;
            vault.add_full(
                &service,
                &username,
                &password,
                url.as_deref().unwrap_or(""),
                notes.as_deref().unwrap_or(""),
            )?;
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
                    if !entry.url.is_empty() {
                        println!("URL: {}", entry.url);
                    }
                    if !entry.notes.is_empty() {
                        println!("Notes: {}", entry.notes);
                    }
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
            print_listing(vault.entries().iter());
        }
        Commands::Find { query } => {
            let vault = open_vault(&file)?;
            let found = vault.find(&query);
            if found.is_empty() {
                println!("No matching entries.");
            } else {
                print_listing(found);
            }
        }
        Commands::Mv { from, to } => {
            let mut vault = open_vault(&file)?;
            vault.rename(&from, &to)?;
            println!("Renamed {from} to {to}");
        }
        Commands::Update {
            service,
            username,
            generate: generate_flag,
            length,
            url,
            notes,
        } => {
            let mut vault = open_vault(&file)?;
            let password = entry_password(generate_flag, length)?;
            vault.update_full(
                &service,
                &username,
                Some(&password),
                url.as_deref(),
                notes.as_deref(),
            )?;
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
        Commands::Passwd => {
            let mut vault = open_vault(&file)?;
            let new_master = read_changed_master_password()?;
            vault.change_master(&new_master)?;
            println!("Master password updated");
        }
        Commands::Backup { output } => {
            persistence::copy_vault_file(&file, &output)?;
            println!("Copied vault to {}", output.display());
        }
        Commands::Export {
            output,
            service,
            all,
            move_entries,
            on_conflict,
        } => {
            let mut source = open_vault(&file)?;
            let incoming = collect_entries(&source, all, &service)?;
            let mut dest = open_or_create_dest(&output)?;
            let report = dest.ingest(incoming, conflict_resolver(on_conflict))?;
            if move_entries {
                for name in report
                    .copied
                    .iter()
                    .chain(report.overwritten.iter())
                    .chain(report.renamed.iter().map(|(from, _)| from))
                {
                    source.delete(name)?;
                }
            }
            print_transfer_report(&report, move_entries);
        }
        Commands::Import {
            from,
            service,
            all,
            on_conflict,
        } => {
            let mut dest = open_vault(&file)?;
            let source = open_source(&from)?;
            let incoming = collect_entries(&source, all, &service)?;
            let report = dest.ingest(incoming, conflict_resolver(on_conflict))?;
            print_transfer_report(&report, false);
        }
        Commands::Gui => {
            let vault = open_vault(&file)?;
            run_gui(vault)?;
        }
    }
    Ok(())
}

fn collect_entries(
    vault: &Vault,
    all: bool,
    services: &[String],
) -> anyhow::Result<Vec<PasswordEntry>> {
    if all {
        Ok(vault.entries().to_vec())
    } else if services.is_empty() {
        Err(StashError::NoServicesSelected.into())
    } else {
        Ok(vault.selected_entries(services)?)
    }
}

fn open_or_create_dest(path: &Path) -> anyhow::Result<Vault> {
    if path.exists() {
        let master = read_dest_master_password(false)?;
        Ok(Vault::open(path, &master)?)
    } else {
        let master = read_dest_master_password(true)?;
        Ok(Vault::create(path, &master)?)
    }
}

fn open_source(path: &Path) -> anyhow::Result<Vault> {
    if !path.exists() {
        anyhow::bail!(
            "no vault found at {}. Run `pwstash init` to create one.",
            path.display()
        );
    }
    let master = read_source_master_password()?;
    Ok(Vault::open(path, &master)?)
}

fn conflict_resolver(
    policy: OnConflict,
) -> impl FnMut(&PasswordEntry, &PasswordEntry, &str) -> Result<ConflictDecision, StashError> {
    move |existing, incoming, proposed| match policy {
        OnConflict::Skip => Ok(ConflictDecision::Skip),
        OnConflict::Overwrite => Ok(ConflictDecision::Overwrite),
        OnConflict::Fail => Err(StashError::DuplicateService {
            service: incoming.service.clone(),
        }),
        OnConflict::Ask => ask_conflict(existing, incoming, proposed),
    }
}

fn ask_conflict(
    existing: &PasswordEntry,
    incoming: &PasswordEntry,
    proposed: &str,
) -> Result<ConflictDecision, StashError> {
    if !io::stdin().is_terminal() {
        return Err(StashError::DuplicateService {
            service: incoming.service.clone(),
        });
    }
    eprintln!("Conflict for {}:", incoming.service);
    for line in existing.diff_lines(incoming) {
        eprintln!("  {line}");
    }
    eprintln!(
        "Type s to skip, o to overwrite, r to rename (default {proposed}), a to abort, then press Enter."
    );
    io::stderr().flush().ok();
    let mut line = String::new();
    io::stdin().read_line(&mut line).map_err(StashError::from)?;
    let mut parts = line.split_whitespace();
    match parts.next().unwrap_or("") {
        "o" | "O" | "overwrite" => Ok(ConflictDecision::Overwrite),
        "a" | "A" | "abort" => Ok(ConflictDecision::Abort),
        "r" | "R" | "rename" => {
            let name = match parts.next() {
                Some(name) => name.to_string(),
                None => {
                    eprintln!("New name [{proposed}]:");
                    io::stderr().flush().ok();
                    line.clear();
                    io::stdin().read_line(&mut line).map_err(StashError::from)?;
                    let typed = line.trim();
                    if typed.is_empty() {
                        proposed.to_string()
                    } else {
                        typed.to_string()
                    }
                }
            };
            Ok(ConflictDecision::Rename(name))
        }
        _ => Ok(ConflictDecision::Skip),
    }
}

fn print_transfer_report(report: &pwstash::vault::TransferReport, moved: bool) {
    let verb = if moved { "Moved" } else { "Copied" };
    println!(
        "{verb} {}, overwrote {}, renamed {}, skipped {}",
        report.copied.len(),
        report.overwritten.len(),
        report.renamed.len(),
        report.skipped.len()
    );
}

fn print_listing<'a>(entries: impl IntoIterator<Item = &'a pwstash::vault::PasswordEntry>) {
    let entries: Vec<_> = entries.into_iter().collect();
    if entries.is_empty() {
        println!("No entries.");
        return;
    }
    for entry in entries {
        if entry.url.is_empty() {
            println!("{}\t{}", entry.service, entry.username);
        } else {
            println!("{}\t{}\t{}", entry.service, entry.username, entry.url);
        }
    }
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
            Event::Key(key_event) => {
                app.note_activity();
                handle_key_events(key_event, &mut app)?;
            }
            Event::Paste(text) => {
                app.note_activity();
                app.handle_paste(&text);
            }
            Event::Mouse(_) | Event::Resize(_, _) => {}
        }
    }

    tui.exit()?;
    Ok(())
}
