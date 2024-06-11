use std::io;

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use pwstash::args::{CommandLineArgs, Commands};
use pwstash::app::{App, AppResult};
use pwstash::event::{Event, EventHandler};
use pwstash::handler::handle_key_events;
use pwstash::tui::Tui;
//use clap::Parser;

fn main() -> AppResult<()> {
    let cli = CommandLineArgs::parse();

    match &cli.command {
        Commands::Init {
            file,
            masterpassword } => {

        }
        Commands::Add {
            file,
            masterpassword,
            service,
            username,
            password} => {

        }
        Commands::Get {
            file,
            masterpassword,
            service
        } => {

        }
        Commands::Gui {
            file,
            masterpassword
        } => {
            run_gui(file, masterpassword).unwrap();
        }
        _ => {
        }
    }
    Ok(())
}

fn run_gui(file: &String, masterpassword: &Option<String>) -> AppResult<()> {
// Create an application.
let mut app = App::new();

// Initialize the terminal user interface.
let backend = CrosstermBackend::new(io::stderr());
let terminal = Terminal::new(backend)?;
let events = EventHandler::new(250);
let mut tui = Tui::new(terminal, events);
tui.init()?;

// Start the main loop.
while app.running {
// Render the user interface.
tui.draw(&mut app)?;
    // Handle events.
    match tui.events.next()? {
        Event::Tick => app.tick(),
        Event::Key(key_event) => handle_key_events(key_event, &mut app)?,
        Event::Mouse(_) => {}
        Event::Resize(_, _) => {}
    }
}

// Exit the user interface.
tui.exit()?;
Ok(())
}
