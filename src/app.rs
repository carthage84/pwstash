use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::widgets::ListState;

use crate::clipboard::{self, CLIPBOARD_TTL};
use crate::vault::{PasswordEntry, Vault};

const STATUS_TTL: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    List,
    Search,
    Add,
    Edit,
    ConfirmDelete,
    ChangeMaster,
    Help,
}

#[derive(Debug, Clone, Default)]
pub struct FormState {
    pub service: String,
    pub username: String,
    pub password: String,
    pub confirm: String,
    pub field: usize,
}

impl FormState {
    pub fn field_count(&self, mode: Mode) -> usize {
        match mode {
            Mode::Add => 3,
            Mode::Edit | Mode::ChangeMaster => 2,
            _ => 0,
        }
    }

    pub fn active_mut(&mut self, mode: Mode) -> &mut String {
        match (mode, self.field) {
            (Mode::Add, 0) => &mut self.service,
            (Mode::Add, 1) | (Mode::Edit, 0) => &mut self.username,
            (Mode::Add, 2) | (Mode::Edit, 1) | (Mode::ChangeMaster, 0) => &mut self.password,
            (Mode::ChangeMaster, 1) => &mut self.confirm,
            _ => &mut self.service,
        }
    }

    pub fn next_field(&mut self, mode: Mode) {
        let n = self.field_count(mode);
        if n > 0 {
            self.field = (self.field + 1) % n;
        }
    }

    pub fn prev_field(&mut self, mode: Mode) {
        let n = self.field_count(mode);
        if n > 0 {
            self.field = (self.field + n - 1) % n;
        }
    }
}

pub struct App {
    pub running: bool,
    pub vault: Vault,
    pub mode: Mode,
    pub selected: usize,
    pub filter: String,
    pub revealed: bool,
    pub form: FormState,
    pub list_state: ListState,
    pub status: Option<(String, Instant)>,
    clipboard_clear_at: Option<Instant>,
}

impl App {
    pub fn new(vault: Vault) -> Self {
        let mut app = Self {
            running: true,
            vault,
            mode: Mode::List,
            selected: 0,
            filter: String::new(),
            revealed: false,
            form: FormState::default(),
            list_state: ListState::default(),
            status: None,
            clipboard_clear_at: None,
        };
        app.sync_list_state();
        app
    }

    pub fn tick(&mut self) {
        if let Some(deadline) = self.clipboard_clear_at
            && Instant::now() >= deadline
        {
            let _ = clipboard::clear();
            self.clipboard_clear_at = None;
            self.flash("Clipboard cleared");
        }
        if let Some((_, until)) = &self.status
            && Instant::now() >= *until
        {
            self.status = None;
        }
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn flash(&mut self, message: impl Into<String>) {
        self.status = Some((message.into(), Instant::now() + STATUS_TTL));
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        let q = self.filter.to_lowercase();
        self.vault
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                q.is_empty()
                    || entry.service.to_lowercase().contains(&q)
                    || entry.username.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn selected_entry(&self) -> Option<&PasswordEntry> {
        let indices = self.filtered_indices();
        indices
            .get(self.selected)
            .and_then(|&i| self.vault.entries().get(i))
    }

    pub fn selected_service(&self) -> Option<String> {
        self.selected_entry().map(|e| e.service.clone())
    }

    pub fn sync_list_state(&mut self) {
        let n = self.filtered_indices().len();
        if n == 0 {
            self.selected = 0;
            self.list_state.select(None);
            return;
        }
        if self.selected >= n {
            self.selected = n - 1;
        }
        self.list_state.select(Some(self.selected));
    }

    pub fn select_next(&mut self) {
        let n = self.filtered_indices().len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + 1) % n;
        self.revealed = false;
        self.sync_list_state();
    }

    pub fn select_prev(&mut self) {
        let n = self.filtered_indices().len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + n - 1) % n;
        self.revealed = false;
        self.sync_list_state();
    }

    pub fn toggle_reveal(&mut self) {
        if self.selected_entry().is_some() {
            self.revealed = !self.revealed;
        }
    }

    pub fn copy_password(&mut self) -> Result<()> {
        self.copy_selected("Password", |entry| entry.password.clone())
    }

    pub fn copy_username(&mut self) -> Result<()> {
        self.copy_selected("Username", |entry| entry.username.clone())
    }

    fn copy_selected(
        &mut self,
        label: &str,
        value: impl FnOnce(&PasswordEntry) -> String,
    ) -> Result<()> {
        let Some(entry) = self.selected_entry() else {
            self.flash("No entry selected");
            return Ok(());
        };
        let secret = value(entry);
        match clipboard::copy_text(&secret) {
            Ok(()) => {
                self.clipboard_clear_at = Some(Instant::now() + CLIPBOARD_TTL);
                self.flash(format!("{label} copied (clears in 30s)"));
            }
            Err(err) => self.flash(err.to_string()),
        }
        Ok(())
    }

    pub fn generate_form_password(&mut self) {
        match crate::generate::generate(crate::generate::DEFAULT_LENGTH) {
            Ok(password) => {
                self.form.password = password;
                self.flash("Generated a password");
            }
            Err(err) => self.flash(err.to_string()),
        }
    }

    pub fn begin_add_generated(&mut self) {
        self.begin_add();
        self.generate_form_password();
    }

    pub fn begin_help(&mut self) {
        self.mode = Mode::Help;
    }

    pub fn begin_search(&mut self) {
        self.mode = Mode::Search;
    }

    pub fn begin_add(&mut self) {
        self.form = FormState::default();
        self.mode = Mode::Add;
    }

    pub fn begin_edit(&mut self) {
        let Some(entry) = self.selected_entry() else {
            self.flash("No entry selected");
            return;
        };
        self.form = FormState {
            service: entry.service.clone(),
            username: entry.username.clone(),
            password: String::new(),
            confirm: String::new(),
            field: 0,
        };
        self.mode = Mode::Edit;
    }

    pub fn begin_change_master(&mut self) {
        self.form = FormState::default();
        self.mode = Mode::ChangeMaster;
    }

    pub fn begin_delete(&mut self) {
        if self.selected_entry().is_none() {
            self.flash("No entry selected");
            return;
        }
        self.mode = Mode::ConfirmDelete;
    }

    pub fn cancel_mode(&mut self) {
        match self.mode {
            Mode::Search => {
                self.filter.clear();
                self.selected = 0;
                self.sync_list_state();
                self.mode = Mode::List;
            }
            Mode::Add | Mode::Edit | Mode::ConfirmDelete | Mode::ChangeMaster | Mode::Help => {
                self.form = FormState::default();
                self.mode = Mode::List;
            }
            Mode::List => {}
        }
    }

    pub fn confirm_search(&mut self) {
        self.mode = Mode::List;
        self.sync_list_state();
    }

    pub fn handle_paste(&mut self, text: &str) {
        match self.mode {
            Mode::Search => {
                self.filter.push_str(text);
                self.selected = 0;
                self.sync_list_state();
            }
            Mode::Add | Mode::Edit | Mode::ChangeMaster => {
                let mode = self.mode;
                self.form.active_mut(mode).push_str(text);
            }
            Mode::List | Mode::ConfirmDelete | Mode::Help => {}
        }
    }

    pub fn type_char(&mut self, c: char) {
        match self.mode {
            Mode::Search => {
                self.filter.push(c);
                self.selected = 0;
                self.revealed = false;
                self.sync_list_state();
            }
            Mode::Add | Mode::Edit | Mode::ChangeMaster => {
                let mode = self.mode;
                self.form.active_mut(mode).push(c);
            }
            Mode::List | Mode::ConfirmDelete | Mode::Help => {}
        }
    }

    pub fn backspace(&mut self) {
        match self.mode {
            Mode::Search => {
                self.filter.pop();
                self.selected = 0;
                self.sync_list_state();
            }
            Mode::Add | Mode::Edit | Mode::ChangeMaster => {
                let mode = self.mode;
                self.form.active_mut(mode).pop();
            }
            Mode::List | Mode::ConfirmDelete | Mode::Help => {}
        }
    }

    pub fn submit_form(&mut self) -> Result<()> {
        match self.mode {
            Mode::Add => {
                let service = self.form.service.clone();
                let username = self.form.username.clone();
                let password = self.form.password.clone();
                match self.vault.add(&service, &username, &password) {
                    Ok(()) => {
                        self.form = FormState::default();
                        self.mode = Mode::List;
                        if let Some(idx) = self
                            .filtered_indices()
                            .into_iter()
                            .find(|&i| self.vault.entries()[i].service == service)
                        {
                            self.selected = idx;
                        }
                        self.sync_list_state();
                        self.flash(format!("Added {service}"));
                    }
                    Err(err) => self.flash(err.to_string()),
                }
            }
            Mode::Edit => {
                let service = self.form.service.clone();
                let username = self.form.username.clone();
                let password = self.form.password.clone();
                match self.vault.update(&service, &username, &password) {
                    Ok(()) => {
                        self.form = FormState::default();
                        self.mode = Mode::List;
                        self.revealed = false;
                        self.flash(format!("Updated {service}"));
                    }
                    Err(err) => self.flash(err.to_string()),
                }
            }
            Mode::ChangeMaster => {
                if self.form.password != self.form.confirm {
                    self.flash("Master passwords do not match");
                    return Ok(());
                }
                match self.vault.change_master(&self.form.password) {
                    Ok(()) => {
                        self.form = FormState::default();
                        self.mode = Mode::List;
                        self.flash("Master password updated");
                    }
                    Err(err) => self.flash(err.to_string()),
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn confirm_delete(&mut self) -> Result<()> {
        if let Some(service) = self.selected_service() {
            match self.vault.delete(&service) {
                Ok(()) => {
                    self.flash(format!("Deleted {service}"));
                    self.revealed = false;
                }
                Err(err) => self.flash(err.to_string()),
            }
        }
        self.mode = Mode::List;
        self.sync_list_state();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;
    use tempfile::TempDir;

    fn app_with_entries() -> (App, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("t.stash");
        let mut vault = Vault::create(&path, "master").unwrap();
        vault.add("github", "ada", "gh-pass").unwrap();
        vault.add("mail", "ada", "mail-pass").unwrap();
        (App::new(vault), dir)
    }

    #[test]
    fn navigation_wraps() {
        let (mut app, _dir) = app_with_entries();
        assert_eq!(app.selected_entry().unwrap().service, "github");
        app.select_next();
        assert_eq!(app.selected_entry().unwrap().service, "mail");
        app.select_next();
        assert_eq!(app.selected_entry().unwrap().service, "github");
        app.select_prev();
        assert_eq!(app.selected_entry().unwrap().service, "mail");
    }

    #[test]
    fn filter_narrows_list() {
        let (mut app, _dir) = app_with_entries();
        app.filter = "mail".into();
        app.selected = 0;
        app.sync_list_state();
        assert_eq!(app.filtered_indices().len(), 1);
        assert_eq!(app.selected_entry().unwrap().service, "mail");
    }

    #[test]
    fn add_via_form() {
        let (mut app, _dir) = app_with_entries();
        app.begin_add();
        app.form.service = "work".into();
        app.form.username = "dev".into();
        app.form.password = "secret".into();
        app.submit_form().unwrap();
        assert_eq!(app.mode, Mode::List);
        assert!(app.vault.get("work").is_some());
    }

    #[test]
    fn edit_via_form() {
        let (mut app, _dir) = app_with_entries();
        app.begin_edit();
        assert_eq!(app.mode, Mode::Edit);
        app.form.username = "new".into();
        app.form.password = "newpass".into();
        app.submit_form().unwrap();
        let entry = app.vault.get("github").unwrap();
        assert_eq!(entry.username, "new");
        assert_eq!(entry.password, "newpass");
    }

    #[test]
    fn delete_via_confirm() {
        let (mut app, _dir) = app_with_entries();
        app.begin_delete();
        assert_eq!(app.mode, Mode::ConfirmDelete);
        app.confirm_delete().unwrap();
        assert!(app.vault.get("github").is_none());
        assert_eq!(app.mode, Mode::List);
    }

    #[test]
    fn reveal_toggles() {
        let (mut app, _dir) = app_with_entries();
        assert!(!app.revealed);
        app.toggle_reveal();
        assert!(app.revealed);
        app.toggle_reveal();
        assert!(!app.revealed);
    }

    #[test]
    fn cancel_search_clears_filter() {
        let (mut app, _dir) = app_with_entries();
        app.begin_search();
        app.type_char('g');
        assert_eq!(app.filter, "g");
        app.cancel_mode();
        assert!(app.filter.is_empty());
        assert_eq!(app.mode, Mode::List);
    }

    #[test]
    fn paste_into_form() {
        let (mut app, _dir) = app_with_entries();
        app.begin_add();
        app.handle_paste("github-work");
        assert_eq!(app.form.service, "github-work");
    }

    #[test]
    fn quit() {
        let (mut app, _dir) = app_with_entries();
        app.quit();
        assert!(!app.running);
    }

    #[test]
    fn generate_fills_password_field() {
        let (mut app, _dir) = app_with_entries();
        app.begin_add_generated();
        assert_eq!(app.mode, Mode::Add);
        assert_eq!(app.form.password.len(), crate::generate::DEFAULT_LENGTH);
    }

    #[test]
    fn change_master_via_form() {
        let (mut app, _dir) = app_with_entries();
        let path = app.vault.path().to_path_buf();
        app.begin_change_master();
        app.form.password = "fresh-master".into();
        app.form.confirm = "fresh-master".into();
        app.submit_form().unwrap();
        assert_eq!(app.mode, Mode::List);
        assert!(Vault::open(&path, "master").is_err());
        let opened = Vault::open(&path, "fresh-master").unwrap();
        assert!(opened.get("github").is_some());
    }

    #[test]
    fn change_master_mismatch_stays_in_form() {
        let (mut app, _dir) = app_with_entries();
        app.begin_change_master();
        app.form.password = "a".into();
        app.form.confirm = "b".into();
        app.submit_form().unwrap();
        assert_eq!(app.mode, Mode::ChangeMaster);
    }

    #[test]
    fn help_mode_toggles() {
        let (mut app, _dir) = app_with_entries();
        app.begin_help();
        assert_eq!(app.mode, Mode::Help);
        app.cancel_mode();
        assert_eq!(app.mode, Mode::List);
    }
}
