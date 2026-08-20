use std::io;
use std::panic;

use anyhow::Result;
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::Backend;

use crate::app::App;
use crate::event::EventHandler;
use crate::ui;

pub struct Tui<B: Backend> {
    terminal: Terminal<B>,
    pub events: EventHandler,
}

impl<B: Backend> Tui<B> {
    pub fn new(terminal: Terminal<B>, events: EventHandler) -> Self {
        Self { terminal, events }
    }

    pub fn init(&mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;

        let panic_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let _ = reset();
            panic_hook(info);
        }));

        self.terminal
            .hide_cursor()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        self.terminal.clear().map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    pub fn draw(&mut self, app: &mut App) -> Result<()> {
        self.terminal
            .draw(|frame| ui::render(app, frame))
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    pub fn exit(&mut self) -> Result<()> {
        reset()?;
        self.terminal
            .show_cursor()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }
}

fn reset() -> Result<()> {
    terminal::disable_raw_mode()?;
    execute!(
        io::stdout(),
        DisableBracketedPaste,
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    Ok(())
}
