use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::widgets::ListState;

use crate::clipboard::{self, CLIPBOARD_TTL};
use crate::vault::{ConflictDecision, PasswordEntry, TransferReport, Vault};

const STATUS_TTL: Duration = Duration::from_secs(4);
pub const IDLE_LOCK: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    List,
    Search,
    Add,
    Edit,
    ConfirmDelete,
    ChangeMaster,
    Rename,
    Help,
    Locked,
    TransferSetup,
    TransferConflict,
    TransferRename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferKind {
    ExportCopy,
    ExportMove,
    Import,
}

#[derive(Debug, Clone, Default)]
pub struct FormState {
    pub service: String,
    pub username: String,
    pub password: String,
    pub confirm: String,
    pub url: String,
    pub notes: String,
    pub field: usize,
}

impl FormState {
    pub fn field_count(&self, mode: Mode, transfer_kind: Option<TransferKind>) -> usize {
        match mode {
            Mode::Add => 5,
            Mode::Edit => 4,
            Mode::Rename | Mode::TransferRename => 1,
            Mode::ChangeMaster => 2,
            Mode::TransferSetup => match transfer_kind {
                Some(TransferKind::Import) => 2,
                _ => 3,
            },
            Mode::Locked => 1,
            _ => 0,
        }
    }

    pub fn active_mut(&mut self, mode: Mode) -> &mut String {
        match (mode, self.field) {
            (Mode::Add, 0) => &mut self.service,
            (Mode::Add, 1) | (Mode::Edit, 0) => &mut self.username,
            (Mode::Add, 2) | (Mode::Edit, 1) | (Mode::ChangeMaster, 0) | (Mode::Locked, _) => {
                &mut self.password
            }
            (Mode::Add, 3) | (Mode::Edit, 2) => &mut self.url,
            (Mode::Add, 4) | (Mode::Edit, 3) => &mut self.notes,
            (Mode::Rename, _) | (Mode::TransferRename, _) | (Mode::TransferSetup, 0) => {
                &mut self.service
            }
            (Mode::TransferSetup, 1) => &mut self.password,
            (Mode::TransferSetup, 2) => &mut self.confirm,
            (Mode::ChangeMaster, 1) => &mut self.confirm,
            _ => &mut self.service,
        }
    }

    pub fn next_field(&mut self, mode: Mode, transfer_kind: Option<TransferKind>) {
        let n = self.field_count(mode, transfer_kind);
        if n > 0 {
            self.field = (self.field + 1) % n;
        }
    }

    pub fn prev_field(&mut self, mode: Mode, transfer_kind: Option<TransferKind>) {
        let n = self.field_count(mode, transfer_kind);
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
    last_input: Instant,
    idle_timeout: Duration,
    pub marked: HashSet<String>,
    pub transfer_kind: Option<TransferKind>,
    transfer_dest: Option<Vault>,
    transfer_queue: Vec<PasswordEntry>,
    pub conflict_existing: Option<PasswordEntry>,
    pub conflict_incoming: Option<PasswordEntry>,
    transfer_report: TransferReport,
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
            last_input: Instant::now(),
            idle_timeout: IDLE_LOCK,
            marked: HashSet::new(),
            transfer_kind: None,
            transfer_dest: None,
            transfer_queue: Vec::new(),
            conflict_existing: None,
            conflict_incoming: None,
            transfer_report: TransferReport::default(),
        };
        app.sync_list_state();
        app
    }

    pub fn tick(&mut self) {
        if self.mode != Mode::Locked && self.last_input.elapsed() >= self.idle_timeout {
            self.lock();
            return;
        }
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

    pub fn note_activity(&mut self) {
        if self.mode != Mode::Locked {
            self.last_input = Instant::now();
        }
    }

    pub fn lock(&mut self) {
        let _ = clipboard::clear();
        self.clipboard_clear_at = None;
        self.revealed = false;
        self.filter.clear();
        self.form = FormState::default();
        self.status = None;
        self.marked.clear();
        self.clear_transfer();
        self.vault.lock();
        self.sync_list_state();
        self.mode = Mode::Locked;
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn flash(&mut self, message: impl Into<String>) {
        self.status = Some((message.into(), Instant::now() + STATUS_TTL));
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        self.vault
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.matches(&self.filter))
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
            url: entry.url.clone(),
            notes: entry.notes.clone(),
            field: 0,
        };
        self.mode = Mode::Edit;
    }

    pub fn begin_change_master(&mut self) {
        self.form = FormState::default();
        self.mode = Mode::ChangeMaster;
    }

    pub fn begin_rename(&mut self) {
        let Some(service) = self.selected_service() else {
            self.flash("No entry selected");
            return;
        };
        self.form = FormState {
            service,
            ..FormState::default()
        };
        self.mode = Mode::Rename;
    }

    pub fn is_marked(&self, service: &str) -> bool {
        self.marked
            .iter()
            .any(|name| name.eq_ignore_ascii_case(service))
    }

    pub fn toggle_mark(&mut self) {
        let Some(service) = self.selected_service() else {
            self.flash("No entry selected");
            return;
        };
        if let Some(existing) = self
            .marked
            .iter()
            .find(|name| name.eq_ignore_ascii_case(&service))
            .cloned()
        {
            self.marked.remove(&existing);
        } else {
            self.marked.insert(service);
        }
    }

    fn services_to_transfer(&self) -> Vec<String> {
        if self.marked.is_empty() {
            self.selected_service().into_iter().collect()
        } else {
            self.marked.iter().cloned().collect()
        }
    }

    fn clear_transfer(&mut self) {
        self.transfer_kind = None;
        self.transfer_dest = None;
        self.transfer_queue.clear();
        self.conflict_existing = None;
        self.conflict_incoming = None;
        self.transfer_report = TransferReport::default();
    }

    fn dest_vault(&self) -> Option<&Vault> {
        dest_ref(self.transfer_kind, &self.vault, &self.transfer_dest)
    }

    pub fn begin_export(&mut self, move_entries: bool) {
        if self.services_to_transfer().is_empty() {
            self.flash("No entry selected");
            return;
        }
        self.form = FormState::default();
        self.transfer_kind = Some(if move_entries {
            TransferKind::ExportMove
        } else {
            TransferKind::ExportCopy
        });
        self.mode = Mode::TransferSetup;
    }

    pub fn begin_import(&mut self) {
        self.form = FormState::default();
        self.transfer_kind = Some(TransferKind::Import);
        self.mode = Mode::TransferSetup;
    }

    fn submit_transfer_setup(&mut self) -> Result<()> {
        let path = PathBuf::from(self.form.service.trim());
        if path.as_os_str().is_empty() {
            self.flash("Path must not be empty");
            return Ok(());
        }
        let master = self.form.password.clone();
        let kind = self.transfer_kind;
        match kind {
            Some(TransferKind::Import) => match Vault::open(&path, &master) {
                Ok(source) => {
                    let incoming = source.entries().to_vec();
                    self.start_ingest(incoming, false);
                }
                Err(err) => self.flash(err.to_string()),
            },
            Some(TransferKind::ExportCopy | TransferKind::ExportMove) => {
                let dest = if path.exists() {
                    Vault::open(&path, &master)
                } else {
                    if master != self.form.confirm {
                        self.flash("Master passwords do not match");
                        return Ok(());
                    }
                    Vault::create(&path, &master)
                };
                match dest {
                    Ok(dest) => {
                        let names = self.services_to_transfer();
                        match self.vault.selected_entries(&names) {
                            Ok(incoming) => {
                                self.transfer_dest = Some(dest);
                                self.start_ingest(
                                    incoming,
                                    matches!(kind, Some(TransferKind::ExportMove)),
                                );
                            }
                            Err(err) => self.flash(err.to_string()),
                        }
                    }
                    Err(err) => self.flash(err.to_string()),
                }
            }
            None => self.flash("No transfer in progress"),
        }
        Ok(())
    }

    pub(crate) fn start_ingest(&mut self, incoming: Vec<PasswordEntry>, move_after: bool) {
        let dest = match dest_mut(self.transfer_kind, &mut self.vault, &mut self.transfer_dest) {
            Some(dest) => dest,
            None => {
                self.flash("No destination vault");
                return;
            }
        };
        let mut ready = Vec::new();
        let mut conflicts = Vec::new();
        for entry in incoming {
            match dest.get(&entry.service) {
                Some(existing) => conflicts.push((existing.clone(), entry)),
                None => ready.push(entry),
            }
        }
        for entry in ready {
            self.transfer_report.copied.push(entry.service.clone());
            dest.absorb_new(entry);
        }
        if let Err(err) = dest.save() {
            self.flash(err.to_string());
            self.clear_transfer();
            self.mode = Mode::List;
            return;
        }
        self.transfer_queue = conflicts
            .into_iter()
            .map(|(_, incoming)| incoming)
            .collect();
        // store existing at conflict time from dest
        if move_after {
            self.transfer_kind = Some(TransferKind::ExportMove);
        }
        self.form = FormState::default();
        if self.transfer_queue.is_empty() {
            self.finish_transfer();
        } else {
            self.show_next_conflict();
        }
    }

    fn show_next_conflict(&mut self) {
        let Some(incoming) = self.transfer_queue.first().cloned() else {
            self.finish_transfer();
            return;
        };
        let Some(dest) = self.dest_vault() else {
            self.finish_transfer();
            return;
        };
        let existing = dest.get(&incoming.service).cloned();
        self.conflict_incoming = Some(incoming);
        self.conflict_existing = existing;
        self.form = FormState::default();
        self.mode = Mode::TransferConflict;
    }

    pub fn begin_conflict_rename(&mut self) {
        let Some(service) = self.conflict_incoming.as_ref().map(|e| e.service.clone()) else {
            return;
        };
        let proposed = match self.dest_vault() {
            Some(dest) => dest.unused_name(&service),
            None => format!("{service}-1"),
        };
        self.form = FormState {
            service: proposed,
            ..FormState::default()
        };
        self.mode = Mode::TransferRename;
    }

    pub fn resolve_conflict(&mut self, decision: ConflictDecision) -> Result<()> {
        let Some(incoming) = self.transfer_queue.first().cloned() else {
            self.finish_transfer();
            return Ok(());
        };
        match decision {
            ConflictDecision::Abort => {
                self.flash("Transfer aborted");
                self.finish_transfer();
                return Ok(());
            }
            ConflictDecision::Skip => {
                self.transfer_report.skipped.push(incoming.service.clone());
                self.transfer_queue.remove(0);
            }
            ConflictDecision::Overwrite => {
                let dest = dest_mut(self.transfer_kind, &mut self.vault, &mut self.transfer_dest);
                let Some(dest) = dest else {
                    self.finish_transfer();
                    return Ok(());
                };
                dest.replace_entry(incoming.clone());
                if let Err(err) = dest.save() {
                    self.flash(err.to_string());
                    self.finish_transfer();
                    return Ok(());
                }
                self.transfer_report
                    .overwritten
                    .push(incoming.service.clone());
                self.transfer_queue.remove(0);
            }
            ConflictDecision::Rename(new_name) => {
                let dest = dest_mut(self.transfer_kind, &mut self.vault, &mut self.transfer_dest);
                let Some(dest) = dest else {
                    self.finish_transfer();
                    return Ok(());
                };
                match dest.absorb_renamed(incoming, &new_name) {
                    Ok(pair) => {
                        self.transfer_report.renamed.push(pair);
                        self.transfer_queue.remove(0);
                    }
                    Err(err) => {
                        self.flash(err.to_string());
                        self.mode = Mode::TransferRename;
                        return Ok(());
                    }
                }
            }
        }
        if self.transfer_queue.is_empty() {
            self.finish_transfer();
        } else {
            self.show_next_conflict();
        }
        Ok(())
    }

    fn finish_transfer(&mut self) {
        let moved = matches!(self.transfer_kind, Some(TransferKind::ExportMove));
        if moved {
            let names: Vec<_> = self
                .transfer_report
                .copied
                .iter()
                .chain(self.transfer_report.overwritten.iter())
                .chain(self.transfer_report.renamed.iter().map(|(from, _)| from))
                .cloned()
                .collect();
            for name in names {
                let _ = self.vault.delete(&name);
                self.marked.remove(&name);
            }
        }
        let report = format!(
            "Copied {}, overwrote {}, renamed {}, skipped {}",
            self.transfer_report.copied.len(),
            self.transfer_report.overwritten.len(),
            self.transfer_report.renamed.len(),
            self.transfer_report.skipped.len()
        );
        self.clear_transfer();
        self.form = FormState::default();
        self.mode = Mode::List;
        self.sync_list_state();
        self.flash(report);
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
            Mode::TransferRename => {
                self.form = FormState::default();
                self.mode = Mode::TransferConflict;
            }
            Mode::Add
            | Mode::Edit
            | Mode::ConfirmDelete
            | Mode::ChangeMaster
            | Mode::Rename
            | Mode::TransferSetup
            | Mode::TransferConflict
            | Mode::Help => {
                self.form = FormState::default();
                self.clear_transfer();
                self.mode = Mode::List;
            }
            Mode::List | Mode::Locked => {}
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
            Mode::Add
            | Mode::Edit
            | Mode::ChangeMaster
            | Mode::Rename
            | Mode::TransferSetup
            | Mode::TransferRename
            | Mode::Locked => {
                let mode = self.mode;
                self.form.active_mut(mode).push_str(text);
            }
            Mode::List | Mode::ConfirmDelete | Mode::Help | Mode::TransferConflict => {}
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
            Mode::Add
            | Mode::Edit
            | Mode::ChangeMaster
            | Mode::Rename
            | Mode::TransferSetup
            | Mode::TransferRename
            | Mode::Locked => {
                let mode = self.mode;
                self.form.active_mut(mode).push(c);
            }
            Mode::List | Mode::ConfirmDelete | Mode::Help | Mode::TransferConflict => {}
        }
    }

    pub fn backspace(&mut self) {
        match self.mode {
            Mode::Search => {
                self.filter.pop();
                self.selected = 0;
                self.sync_list_state();
            }
            Mode::Add
            | Mode::Edit
            | Mode::ChangeMaster
            | Mode::Rename
            | Mode::TransferSetup
            | Mode::TransferRename
            | Mode::Locked => {
                let mode = self.mode;
                self.form.active_mut(mode).pop();
            }
            Mode::List | Mode::ConfirmDelete | Mode::Help | Mode::TransferConflict => {}
        }
    }

    pub fn submit_form(&mut self) -> Result<()> {
        match self.mode {
            Mode::Add => {
                let service = self.form.service.clone();
                let username = self.form.username.clone();
                let password = self.form.password.clone();
                let url = self.form.url.clone();
                let notes = self.form.notes.clone();
                match self
                    .vault
                    .add_full(&service, &username, &password, &url, &notes)
                {
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
                let url = self.form.url.clone();
                let notes = self.form.notes.clone();
                let password = if password.is_empty() {
                    None
                } else {
                    Some(password.as_str())
                };
                match self.vault.update_full(
                    &service,
                    &username,
                    password,
                    Some(&url),
                    Some(&notes),
                ) {
                    Ok(()) => {
                        self.form = FormState::default();
                        self.mode = Mode::List;
                        self.revealed = false;
                        self.flash(format!("Updated {service}"));
                    }
                    Err(err) => self.flash(err.to_string()),
                }
            }
            Mode::Rename => {
                let Some(from) = self.selected_service() else {
                    self.flash("No entry selected");
                    self.mode = Mode::List;
                    return Ok(());
                };
                let to = self.form.service.clone();
                match self.vault.rename(&from, &to) {
                    Ok(()) => {
                        self.form = FormState::default();
                        self.mode = Mode::List;
                        if let Some(idx) = self
                            .filtered_indices()
                            .into_iter()
                            .find(|&i| self.vault.entries()[i].service == to)
                        {
                            self.selected = idx;
                        }
                        self.sync_list_state();
                        self.flash(format!("Renamed to {to}"));
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
            Mode::TransferSetup => self.submit_transfer_setup()?,
            Mode::TransferRename => {
                let name = self.form.service.clone();
                self.resolve_conflict(ConflictDecision::Rename(name))?;
            }
            Mode::Locked => {
                let password = self.form.password.clone();
                match self.vault.unlock(&password) {
                    Ok(()) => {
                        self.form = FormState::default();
                        self.mode = Mode::List;
                        self.last_input = Instant::now();
                        self.sync_list_state();
                        self.flash("Unlocked");
                    }
                    Err(err) => {
                        self.form.password.clear();
                        self.flash(err.to_string());
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn expire_idle_for_test(&mut self) {
        self.last_input = Instant::now() - self.idle_timeout - Duration::from_secs(1);
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

fn dest_ref<'a>(
    kind: Option<TransferKind>,
    vault: &'a Vault,
    dest: &'a Option<Vault>,
) -> Option<&'a Vault> {
    match kind {
        Some(TransferKind::Import) => Some(vault),
        Some(TransferKind::ExportCopy | TransferKind::ExportMove) => dest.as_ref(),
        None => None,
    }
}

fn dest_mut<'a>(
    kind: Option<TransferKind>,
    vault: &'a mut Vault,
    dest: &'a mut Option<Vault>,
) -> Option<&'a mut Vault> {
    match kind {
        Some(TransferKind::Import) => Some(vault),
        Some(TransferKind::ExportCopy | TransferKind::ExportMove) => dest.as_mut(),
        None => None,
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
        app.form.url = "https://work.example".into();
        app.form.notes = "vpn".into();
        app.submit_form().unwrap();
        assert_eq!(app.mode, Mode::List);
        let entry = app.vault.get("work").unwrap();
        assert_eq!(entry.url, "https://work.example");
        assert_eq!(entry.notes, "vpn");
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
    fn mark_toggles_selection() {
        let (mut app, _dir) = app_with_entries();
        assert!(!app.is_marked("github"));
        app.toggle_mark();
        assert!(app.is_marked("github"));
        app.toggle_mark();
        assert!(!app.is_marked("github"));
    }

    #[test]
    fn rename_via_form() {
        let (mut app, _dir) = app_with_entries();
        app.begin_rename();
        assert_eq!(app.mode, Mode::Rename);
        app.form.service = "gh".into();
        app.submit_form().unwrap();
        assert_eq!(app.mode, Mode::List);
        assert!(app.vault.get("github").is_none());
        assert!(app.vault.get("gh").is_some());
    }

    #[test]
    fn edit_without_password_keeps_secret() {
        let (mut app, _dir) = app_with_entries();
        app.begin_edit();
        app.form.username = "ada2".into();
        app.form.url = "https://github.com".into();
        app.form.notes = "2fa".into();
        app.submit_form().unwrap();
        let entry = app.vault.get("github").unwrap();
        assert_eq!(entry.username, "ada2");
        assert_eq!(entry.password, "gh-pass");
        assert_eq!(entry.url, "https://github.com");
        assert_eq!(entry.notes, "2fa");
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
    fn import_setup_has_two_fields() {
        let (mut app, _dir) = app_with_entries();
        app.begin_import();
        assert_eq!(app.form.field_count(app.mode, app.transfer_kind), 2);
        app.begin_export(false);
        assert_eq!(app.form.field_count(app.mode, app.transfer_kind), 3);
    }

    #[test]
    fn import_name_clash_prompts_even_when_identical() {
        let (mut app, dir) = app_with_entries();
        let source_path = dir.path().join("src.stash");
        let mut source = Vault::create(&source_path, "src-secret").unwrap();
        source.add("github", "ada", "gh-pass").unwrap();
        source.add("mail", "ada", "mail-pass").unwrap();
        app.transfer_kind = Some(TransferKind::Import);
        app.start_ingest(source.entries().to_vec(), false);
        assert_eq!(app.mode, Mode::TransferConflict);
        assert_eq!(
            app.conflict_incoming.as_ref().map(|e| e.service.as_str()),
            Some("github")
        );
    }

    #[test]
    fn conflict_rename_form_prefills_free_name() {
        let (mut app, dir) = app_with_entries();
        let source_path = dir.path().join("src.stash");
        let mut source = Vault::create(&source_path, "src-secret").unwrap();
        source.add("github", "eve", "other").unwrap();
        source.add("github-1", "taken", "pw").unwrap();
        app.transfer_kind = Some(TransferKind::Import);
        app.start_ingest(source.entries().to_vec(), false);
        assert_eq!(app.mode, Mode::TransferConflict);
        app.begin_conflict_rename();
        assert_eq!(app.mode, Mode::TransferRename);
        assert_eq!(app.form.service, "github-2");
        app.submit_form().unwrap();
        assert_eq!(app.mode, Mode::List);
        assert_eq!(app.vault.get("github").unwrap().username, "ada");
        assert_eq!(app.vault.get("github-2").unwrap().username, "eve");
        assert_eq!(app.vault.get("github-1").unwrap().username, "taken");
    }

    #[test]
    fn conflict_rename_rejects_taken_name() {
        let (mut app, dir) = app_with_entries();
        let source_path = dir.path().join("src.stash");
        let mut source = Vault::create(&source_path, "src-secret").unwrap();
        source.add("github", "eve", "other").unwrap();
        app.transfer_kind = Some(TransferKind::Import);
        app.start_ingest(source.entries().to_vec(), false);
        app.begin_conflict_rename();
        assert_eq!(app.form.service, "github-1");
        app.form.service = "mail".into();
        app.submit_form().unwrap();
        assert_eq!(app.mode, Mode::TransferRename);
        assert!(app.vault.get("github").unwrap().username == "ada");
        assert!(app.vault.get("mail").unwrap().username == "ada");
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
    fn idle_tick_locks_and_unlock_restores() {
        let (mut app, _dir) = app_with_entries();
        app.expire_idle_for_test();
        app.tick();
        assert_eq!(app.mode, Mode::Locked);
        assert!(app.vault.is_locked());
        assert!(app.vault.entries().is_empty());

        app.form.password = "wrong".into();
        app.submit_form().unwrap();
        assert_eq!(app.mode, Mode::Locked);

        app.form.password = "master".into();
        app.submit_form().unwrap();
        assert_eq!(app.mode, Mode::List);
        assert!(app.vault.get("github").is_some());
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
