use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Mode};

pub fn handle_key_events(key_event: KeyEvent, app: &mut App) -> Result<()> {
    if key_event.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('C'))
    {
        app.quit();
        return Ok(());
    }

    match app.mode {
        Mode::List => handle_list(key_event, app),
        Mode::Search => handle_search(key_event, app),
        Mode::Add | Mode::Edit | Mode::ChangeMaster | Mode::Rename => handle_form(key_event, app),
        Mode::ConfirmDelete => handle_confirm(key_event, app),
        Mode::Help => handle_help(key_event, app),
        Mode::Locked => handle_unlock(key_event, app),
    }
}

fn handle_list(key_event: KeyEvent, app: &mut App) -> Result<()> {
    match key_event.code {
        KeyCode::Esc | KeyCode::Char('q') => app.quit(),
        KeyCode::Down | KeyCode::Char('j') => app.select_next(),
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
        KeyCode::Char('/') => app.begin_search(),
        KeyCode::Char('P') => app.begin_change_master(),
        KeyCode::Char('p') if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
            app.begin_change_master();
        }
        KeyCode::Enter | KeyCode::Char('p') => app.toggle_reveal(),
        KeyCode::Char('c') => app.copy_password()?,
        KeyCode::Char('y') => app.copy_username()?,
        KeyCode::Char('g') => app.begin_add_generated(),
        KeyCode::Char('?') => app.begin_help(),
        KeyCode::Char('a') => app.begin_add(),
        KeyCode::Char('e') => app.begin_edit(),
        KeyCode::Char('r') => app.begin_rename(),
        KeyCode::Char('d') => app.begin_delete(),
        _ => {}
    }
    Ok(())
}

fn handle_search(key_event: KeyEvent, app: &mut App) -> Result<()> {
    match key_event.code {
        KeyCode::Esc => app.cancel_mode(),
        KeyCode::Enter => app.confirm_search(),
        KeyCode::Backspace => app.backspace(),
        KeyCode::Char(c) if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
            app.type_char(c);
        }
        _ => {}
    }
    Ok(())
}

fn handle_form(key_event: KeyEvent, app: &mut App) -> Result<()> {
    if !matches!(app.mode, Mode::ChangeMaster | Mode::Rename)
        && key_event.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key_event.code, KeyCode::Char('g') | KeyCode::Char('G'))
    {
        app.generate_form_password();
        return Ok(());
    }
    match key_event.code {
        KeyCode::Esc => app.cancel_mode(),
        KeyCode::Tab | KeyCode::Down => app.form.next_field(app.mode),
        KeyCode::BackTab | KeyCode::Up => app.form.prev_field(app.mode),
        KeyCode::Enter => {
            let last = app.form.field + 1 >= app.form.field_count(app.mode);
            if last {
                app.submit_form()?;
            } else {
                app.form.next_field(app.mode);
            }
        }
        KeyCode::Backspace => app.backspace(),
        KeyCode::Char(c) if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
            app.type_char(c);
        }
        _ => {}
    }
    Ok(())
}

fn handle_unlock(key_event: KeyEvent, app: &mut App) -> Result<()> {
    match key_event.code {
        KeyCode::Esc | KeyCode::Char('q') => app.quit(),
        KeyCode::Enter => app.submit_form()?,
        KeyCode::Backspace => app.backspace(),
        KeyCode::Char(c) if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
            app.type_char(c);
        }
        _ => {}
    }
    Ok(())
}

fn handle_help(key_event: KeyEvent, app: &mut App) -> Result<()> {
    match key_event.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => app.cancel_mode(),
        _ => {}
    }
    Ok(())
}

fn handle_confirm(key_event: KeyEvent, app: &mut App) -> Result<()> {
    match key_event.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => app.confirm_delete()?,
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc | KeyCode::Char('q') => {
            app.cancel_mode();
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;
    use crossterm::event::{KeyEventKind, KeyEventState};
    use tempfile::TempDir;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn app() -> (App, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("h.stash");
        let mut vault = Vault::create(&path, "master").unwrap();
        vault.add("github", "ada", "pw").unwrap();
        (App::new(vault), dir)
    }

    #[test]
    fn list_keys() {
        let (mut app, _dir) = app();
        handle_key_events(key(KeyCode::Char('j')), &mut app).unwrap();
        assert_eq!(app.selected_entry().unwrap().service, "github");
        handle_key_events(key(KeyCode::Char('a')), &mut app).unwrap();
        assert_eq!(app.mode, Mode::Add);
        handle_key_events(key(KeyCode::Esc), &mut app).unwrap();
        assert_eq!(app.mode, Mode::List);
        handle_key_events(key(KeyCode::Char('q')), &mut app).unwrap();
        assert!(!app.running);
    }

    #[test]
    fn help_and_generate_keys() {
        let (mut app, _dir) = app();
        handle_key_events(key(KeyCode::Char('?')), &mut app).unwrap();
        assert_eq!(app.mode, Mode::Help);
        handle_key_events(key(KeyCode::Esc), &mut app).unwrap();
        assert_eq!(app.mode, Mode::List);
        handle_key_events(key(KeyCode::Char('g')), &mut app).unwrap();
        assert_eq!(app.mode, Mode::Add);
        assert!(app.form.password.len() >= crate::generate::MIN_LENGTH);
        handle_key_events(key(KeyCode::Esc), &mut app).unwrap();
        handle_key_events(key(KeyCode::Char('r')), &mut app).unwrap();
        assert_eq!(app.mode, Mode::Rename);
        handle_key_events(key(KeyCode::Esc), &mut app).unwrap();
        handle_key_events(key(KeyCode::Char('P')), &mut app).unwrap();
        assert_eq!(app.mode, Mode::ChangeMaster);
        handle_key_events(key(KeyCode::Esc), &mut app).unwrap();
        app.lock();
        handle_key_events(key(KeyCode::Char('q')), &mut app).unwrap();
        assert!(!app.running);
    }
}
