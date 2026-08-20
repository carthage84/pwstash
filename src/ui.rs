use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, Paragraph, Wrap};

/// Width of labels like `Service:  ` / `Path:     ` / `New name: `.
const FIELD_LABEL_WIDTH: u16 = 10;
const LOCKED_LABEL_WIDTH: u16 = 8;

use crate::app::{App, Mode};

const ACCENT: Color = Color::Rgb(218, 110, 63);

pub fn render(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(app, frame, chunks[0]);
    render_body(app, frame, chunks[1]);
    render_footer(app, frame, chunks[2]);

    match app.mode {
        Mode::Add | Mode::Edit => render_form(app, frame, area),
        Mode::ChangeMaster => render_change_master(app, frame, area),
        Mode::Rename | Mode::TransferRename => render_rename(app, frame, area),
        Mode::ConfirmDelete => render_confirm(app, frame, area),
        Mode::Help => render_help(frame, area),
        Mode::Locked => render_locked(app, frame, area),
        Mode::TransferSetup => render_transfer_setup(app, frame, area),
        Mode::TransferConflict => render_transfer_conflict(app, frame, area),
        Mode::List | Mode::Search => {}
    }
}

fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let title = format!(" pwstash — {} ", app.vault.path().display());
    let filter = if app.filter.is_empty() && app.mode != Mode::Search {
        String::new()
    } else {
        format!("  /{}", app.filter)
    };
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            title.clone(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw(filter.clone()),
    ]))
    .block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Vault")
            .title_alignment(Alignment::Left),
    );
    frame.render_widget(header, area);
    if app.mode == Mode::Search {
        set_header_cursor(frame, area, &title, &filter);
    }
}

fn render_body(app: &mut App, frame: &mut Frame, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let indices = app.filtered_indices();
    let items: Vec<ListItem> = indices
        .iter()
        .map(|&i| {
            let entry = &app.vault.entries()[i];
            let mark = if app.is_marked(&entry.service) {
                "*"
            } else {
                " "
            };
            ListItem::new(format!("{mark} {}  ({})", entry.service, entry.username))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title("Entries"),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, columns[0], &mut app.list_state);

    let detail = match app.selected_entry() {
        Some(entry) => {
            let password = if app.revealed {
                entry.password.clone()
            } else {
                "********".to_string()
            };
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Service:  ", Style::default().fg(ACCENT)),
                    Span::raw(entry.service.clone()),
                ]),
                Line::from(vec![
                    Span::styled("Username: ", Style::default().fg(ACCENT)),
                    Span::raw(entry.username.clone()),
                ]),
                Line::from(vec![
                    Span::styled("Password: ", Style::default().fg(ACCENT)),
                    Span::raw(password),
                ]),
            ];
            if !entry.url.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("URL:      ", Style::default().fg(ACCENT)),
                    Span::raw(entry.url.clone()),
                ]));
            }
            if !entry.notes.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("Notes:    ", Style::default().fg(ACCENT)),
                    Span::raw(entry.notes.clone()),
                ]));
            }
            Paragraph::new(lines)
        }
        None => Paragraph::new("No entry selected.\nPress a to add one."),
    }
    .block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Details"),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(detail, columns[1]);
}

fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let help = match app.mode {
        Mode::List => "[j/k] move  [/] search  [p] reveal  [c]opy  [P] master  [?] help  [q]uit",
        Mode::Search => "type to filter  Enter keep  Esc clear",
        Mode::Add => "Tab next  Ctrl-G generate  Enter save  Esc cancel",
        Mode::Edit => "Tab next  Ctrl-G generate  Enter save  Esc cancel",
        Mode::ChangeMaster => "Tab next  Enter save  Esc cancel",
        Mode::Rename | Mode::TransferRename => "Enter save  Esc cancel",
        Mode::ConfirmDelete => "[y] delete  [n] cancel",
        Mode::Help => "Esc or ? close help",
        Mode::Locked => "Enter unlock  q quit",
        Mode::TransferSetup => "Tab next  Enter continue  Esc cancel",
        Mode::TransferConflict => "[s]kip  [o]verwrite  [r]ename  [a]bort",
    };
    let text = if let Some((msg, _)) = &app.status {
        Line::from(vec![
            Span::styled(msg.clone(), Style::default().fg(ACCENT)),
            Span::raw("  |  "),
            Span::raw(help),
        ])
    } else {
        Line::from(help)
    };
    let footer = Paragraph::new(text).block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Keys"),
    );
    frame.render_widget(footer, area);
}

fn render_form(app: &App, frame: &mut Frame, area: Rect) {
    let popup = centered_rect(70, 15, area);
    frame.render_widget(Clear, popup);

    let title = match app.mode {
        Mode::Add => "Add entry",
        Mode::Edit => "Edit entry",
        _ => "Form",
    };

    let password_display: String = "*".repeat(app.form.password.chars().count());
    let mut lines = Vec::new();
    if app.mode == Mode::Add {
        lines.push(Line::from(vec![
            Span::raw("Service:  "),
            Span::styled(app.form.service.clone(), field_style(app, 0)),
        ]));
        lines.push(Line::from(vec![
            Span::raw("Username: "),
            Span::styled(app.form.username.clone(), field_style(app, 1)),
        ]));
        lines.push(Line::from(vec![
            Span::raw("Password: "),
            Span::styled(password_display, field_style(app, 2)),
        ]));
        lines.push(Line::from(vec![
            Span::raw("URL:      "),
            Span::styled(app.form.url.clone(), field_style(app, 3)),
        ]));
        lines.push(Line::from(vec![
            Span::raw("Notes:    "),
            Span::styled(app.form.notes.clone(), field_style(app, 4)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("Service:  "),
            Span::raw(app.form.service.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::raw("Username: "),
            Span::styled(app.form.username.clone(), field_style(app, 0)),
        ]));
        lines.push(Line::from(vec![
            Span::raw("Password: "),
            Span::styled(
                if app.form.password.is_empty() {
                    "(unchanged)".to_string()
                } else {
                    password_display
                },
                field_style(app, 1),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("URL:      "),
            Span::styled(app.form.url.clone(), field_style(app, 2)),
        ]));
        lines.push(Line::from(vec![
            Span::raw("Notes:    "),
            Span::styled(app.form.notes.clone(), field_style(app, 3)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(if app.mode == Mode::Edit {
        "Blank password keeps the current one. Ctrl-G generates. Enter saves."
    } else {
        "Ctrl-G generates a password. Enter saves, Esc cancels."
    }));

    let form = Paragraph::new(lines).block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(title)
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(Color::White).bg(Color::Black)),
    );
    frame.render_widget(form, popup);
    let (line, value) = add_edit_cursor_value(app);
    set_labeled_cursor(frame, popup, line, FIELD_LABEL_WIDTH, &value);
}

fn add_edit_cursor_value(app: &App) -> (u16, String) {
    if app.mode == Mode::Add {
        let value = match app.form.field {
            1 => app.form.username.clone(),
            2 => "*".repeat(app.form.password.chars().count()),
            3 => app.form.url.clone(),
            4 => app.form.notes.clone(),
            _ => app.form.service.clone(),
        };
        (app.form.field as u16, value)
    } else {
        match app.form.field {
            1 => (
                2,
                if app.form.password.is_empty() {
                    String::new()
                } else {
                    "*".repeat(app.form.password.chars().count())
                },
            ),
            2 => (3, app.form.url.clone()),
            3 => (4, app.form.notes.clone()),
            _ => (1, app.form.username.clone()),
        }
    }
}

fn field_style(app: &App, index: usize) -> Style {
    if app.form.field == index {
        Style::default()
            .fg(Color::Black)
            .bg(ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

fn render_transfer_setup(app: &App, frame: &mut Frame, area: Rect) {
    let popup = centered_rect(70, 12, area);
    frame.render_widget(Clear, popup);
    let title = match app.transfer_kind {
        Some(crate::app::TransferKind::Import) => "Import from vault",
        Some(crate::app::TransferKind::ExportMove) => "Move entries to vault",
        _ => "Copy entries to vault",
    };
    let password_display: String = "*".repeat(app.form.password.chars().count());
    let confirm_display: String = "*".repeat(app.form.confirm.chars().count());
    let mut lines = vec![
        Line::from(vec![
            Span::raw("Path:     "),
            Span::styled(app.form.service.clone(), field_style(app, 0)),
        ]),
        Line::from(vec![
            Span::raw("Master:   "),
            Span::styled(password_display, field_style(app, 1)),
        ]),
    ];
    if !matches!(app.transfer_kind, Some(crate::app::TransferKind::Import)) {
        lines.push(Line::from(vec![
            Span::raw("Confirm:  "),
            Span::styled(confirm_display, field_style(app, 2)),
        ]));
    }
    lines.push(Line::from(""));
    if matches!(app.transfer_kind, Some(crate::app::TransferKind::Import)) {
        lines.push(Line::from(
            "Enter the source vault path and its master password.",
        ));
    } else {
        lines.push(Line::from(
            "Confirm is required only when creating a new vault.",
        ));
    }
    let form = Paragraph::new(lines).block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(title)
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(Color::White).bg(Color::Black)),
    );
    frame.render_widget(form, popup);
    let (line, value) = match app.form.field {
        1 => (1, "*".repeat(app.form.password.chars().count())),
        2 => (2, "*".repeat(app.form.confirm.chars().count())),
        _ => (0, app.form.service.clone()),
    };
    set_labeled_cursor(frame, popup, line, FIELD_LABEL_WIDTH, &value);
}

fn render_transfer_conflict(app: &App, frame: &mut Frame, area: Rect) {
    let popup = centered_rect(70, 14, area);
    frame.render_widget(Clear, popup);
    let mut lines = Vec::new();
    if let (Some(existing), Some(incoming)) = (&app.conflict_existing, &app.conflict_incoming) {
        lines.push(Line::from(format!("Conflict: {}", incoming.service)));
        lines.push(Line::from(""));
        for line in existing.diff_lines(incoming) {
            lines.push(Line::from(line));
        }
        if existing.diff_lines(incoming).is_empty() {
            lines.push(Line::from("Entries look the same."));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from("s skip   o overwrite   r rename   a abort"));
    let text = Paragraph::new(lines).block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Conflict")
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(Color::White).bg(Color::Black)),
    );
    frame.render_widget(text, popup);
}

fn render_rename(app: &App, frame: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 8, area);
    frame.render_widget(Clear, popup);
    let form = Paragraph::new(vec![
        Line::from(vec![
            Span::raw("New name: "),
            Span::styled(app.form.service.clone(), field_style(app, 0)),
        ]),
        Line::from(""),
        Line::from("Enter saves, Esc cancels."),
    ])
    .block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(if app.mode == Mode::TransferRename {
                "Import as"
            } else {
                "Rename"
            })
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(Color::White).bg(Color::Black)),
    );
    frame.render_widget(form, popup);
    set_labeled_cursor(frame, popup, 0, FIELD_LABEL_WIDTH, &app.form.service);
}

fn render_change_master(app: &App, frame: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 10, area);
    frame.render_widget(Clear, popup);
    let new_style = field_style(app, 0);
    let confirm_style = field_style(app, 1);
    let new_display: String = "*".repeat(app.form.password.chars().count());
    let confirm_display: String = "*".repeat(app.form.confirm.chars().count());
    let form = Paragraph::new(vec![
        Line::from(vec![
            Span::raw("New:      "),
            Span::styled(new_display, new_style),
        ]),
        Line::from(vec![
            Span::raw("Confirm:  "),
            Span::styled(confirm_display, confirm_style),
        ]),
        Line::from(""),
        Line::from("Enter saves, Esc cancels."),
    ])
    .block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Change master password")
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(Color::White).bg(Color::Black)),
    );
    frame.render_widget(form, popup);
    let (line, value) = if app.form.field == 1 {
        (1, "*".repeat(app.form.confirm.chars().count()))
    } else {
        (0, "*".repeat(app.form.password.chars().count()))
    };
    set_labeled_cursor(frame, popup, line, FIELD_LABEL_WIDTH, &value);
}

fn render_locked(app: &App, frame: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 9, area);
    frame.render_widget(Clear, popup);
    let stars: String = "*".repeat(app.form.password.chars().count());
    let text = Paragraph::new(vec![
        Line::from("Idle lock. Vault is wiped from memory."),
        Line::from(""),
        Line::from(vec![
            Span::raw("Master: "),
            Span::styled(
                stars.clone(),
                Style::default()
                    .fg(Color::Black)
                    .bg(ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from("Enter unlocks. q quits."),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Locked")
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(Color::White).bg(Color::Black)),
    );
    frame.render_widget(text, popup);
    set_centered_labeled_cursor(frame, popup, 2, LOCKED_LABEL_WIDTH, &stars);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(70, 16, area);
    frame.render_widget(Clear, popup);
    let text = Paragraph::new(vec![
        Line::from("j/k or arrows   move"),
        Line::from("/               search"),
        Line::from("p or Enter      reveal password"),
        Line::from("c               copy password (30s)"),
        Line::from("y               copy username (30s)"),
        Line::from("g               add with generated password"),
        Line::from("a / e / r / d   add / edit / rename / delete"),
        Line::from("space           mark for export"),
        Line::from("E / M / I       export copy / export move / import"),
        Line::from("s / o / r / a   skip / overwrite / rename / abort on clash"),
        Line::from("P               change master password"),
        Line::from("Ctrl-G          generate in a form"),
        Line::from("q / Ctrl-C      quit"),
        Line::from(""),
        Line::from("Idle 2 min       lock vault"),
        Line::from("Esc or ? closes this help."),
    ])
    .block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Keys")
            .title_alignment(Alignment::Center),
    );
    frame.render_widget(text, popup);
}

fn render_confirm(app: &App, frame: &mut Frame, area: Rect) {
    let popup = centered_rect(50, 7, area);
    frame.render_widget(Clear, popup);
    let service = app
        .selected_service()
        .unwrap_or_else(|| "(unknown)".to_string());
    let text = Paragraph::new(vec![
        Line::from(format!("Delete {service}?")),
        Line::from("Press y to confirm, n to cancel."),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Confirm")
            .title_alignment(Alignment::Center),
    );
    frame.render_widget(text, popup);
}

fn display_width(s: &str) -> u16 {
    u16::try_from(s.chars().count()).unwrap_or(u16::MAX)
}

fn inner_block(area: Rect) -> Rect {
    area.inner(Margin::new(1, 1))
}

fn clamp_cursor(inner: Rect, x: u16, y: u16) -> Option<Position> {
    if inner.width == 0 || inner.height == 0 {
        return None;
    }
    let max_x = inner.x.saturating_add(inner.width.saturating_sub(1));
    let max_y = inner.y.saturating_add(inner.height.saturating_sub(1));
    Some(Position::new(
        x.clamp(inner.x, max_x),
        y.clamp(inner.y, max_y),
    ))
}

fn labeled_cursor(popup: Rect, line: u16, label_width: u16, value: &str) -> Option<Position> {
    let inner = inner_block(popup);
    let x = inner
        .x
        .saturating_add(label_width)
        .saturating_add(display_width(value));
    let y = inner.y.saturating_add(line);
    clamp_cursor(inner, x, y)
}

fn set_labeled_cursor(frame: &mut Frame, popup: Rect, line: u16, label_width: u16, value: &str) {
    if let Some(pos) = labeled_cursor(popup, line, label_width, value) {
        frame.set_cursor_position(pos);
    }
}

fn set_centered_labeled_cursor(
    frame: &mut Frame,
    popup: Rect,
    line: u16,
    label_width: u16,
    value: &str,
) {
    let inner = inner_block(popup);
    let content_width = label_width.saturating_add(display_width(value));
    let start = inner
        .x
        .saturating_add(inner.width.saturating_sub(content_width) / 2);
    let x = start
        .saturating_add(label_width)
        .saturating_add(display_width(value));
    let y = inner.y.saturating_add(line);
    if let Some(pos) = clamp_cursor(inner, x, y) {
        frame.set_cursor_position(pos);
    }
}

fn set_header_cursor(frame: &mut Frame, area: Rect, title: &str, filter: &str) {
    let inner = inner_block(area);
    let x = inner
        .x
        .saturating_add(display_width(title))
        .saturating_add(display_width(filter));
    if let Some(pos) = clamp_cursor(inner, x, inner.y) {
        frame.set_cursor_position(pos);
    }
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tempfile::TempDir;

    fn buffer_has(buffer: &ratatui::buffer::Buffer, needle: &str) -> bool {
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        text.contains(needle)
    }

    #[test]
    fn renders_entries_and_confirm_dialog() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("ui.stash");
        let mut vault = Vault::create(&path, "master").unwrap();
        vault
            .add_full("github", "ada", "secret", "https://github.com", "work")
            .unwrap();
        vault.add("mail", "ada", "secret2").unwrap();
        let mut app = App::new(vault);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(&mut app, f)).unwrap();
        assert!(buffer_has(terminal.backend().buffer(), "github"));
        assert!(buffer_has(terminal.backend().buffer(), "github.com"));
        assert!(buffer_has(terminal.backend().buffer(), "mail"));

        app.begin_delete();
        terminal.draw(|f| render(&mut app, f)).unwrap();
        assert!(buffer_has(terminal.backend().buffer(), "Delete github?"));

        app.cancel_mode();
        app.begin_help();
        terminal.draw(|f| render(&mut app, f)).unwrap();
        assert!(buffer_has(terminal.backend().buffer(), "copy password"));
        assert!(buffer_has(
            terminal.backend().buffer(),
            "change master password"
        ));

        app.cancel_mode();
        app.begin_import();
        terminal.draw(|f| render(&mut app, f)).unwrap();
        assert!(buffer_has(terminal.backend().buffer(), "source vault path"));
        assert!(!buffer_has(
            terminal.backend().buffer(),
            "Confirm is required"
        ));
        app.cancel_mode();

        app.begin_export(false);
        terminal.draw(|f| render(&mut app, f)).unwrap();
        assert!(buffer_has(
            terminal.backend().buffer(),
            "Confirm is required"
        ));
        app.cancel_mode();

        let source_path = dir.path().join("src.stash");
        let mut source = Vault::create(&source_path, "src-secret").unwrap();
        source.add("github", "eve", "other").unwrap();
        app.transfer_kind = Some(crate::app::TransferKind::Import);
        app.start_ingest(source.entries().to_vec(), false);
        terminal.draw(|f| render(&mut app, f)).unwrap();
        assert!(buffer_has(terminal.backend().buffer(), "r rename"));
        app.begin_conflict_rename();
        terminal.draw(|f| render(&mut app, f)).unwrap();
        assert!(buffer_has(terminal.backend().buffer(), "github-1"));
        assert!(buffer_has(terminal.backend().buffer(), "Import as"));

        app.lock();
        terminal.draw(|f| render(&mut app, f)).unwrap();
        assert!(buffer_has(terminal.backend().buffer(), "Locked"));
        assert!(!buffer_has(terminal.backend().buffer(), "github"));
    }

    #[test]
    fn labeled_cursor_sits_after_the_value() {
        let popup = Rect::new(10, 5, 40, 10);
        let pos = labeled_cursor(popup, 0, 10, "github").unwrap();
        assert_eq!(pos, Position::new(27, 6));
    }

    #[test]
    fn caret_follows_the_active_form_field() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("ui.stash");
        let vault = Vault::create(&path, "master").unwrap();
        let mut app = App::new(vault);
        let area = Rect::new(0, 0, 80, 24);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        app.begin_add();
        app.form.service = "gh".into();
        terminal.draw(|f| render(&mut app, f)).unwrap();
        let popup = centered_rect(70, 15, area);
        assert_eq!(
            terminal.get_cursor_position().unwrap(),
            labeled_cursor(popup, 0, FIELD_LABEL_WIDTH, "gh").unwrap()
        );

        app.form.next_field(app.mode, app.transfer_kind);
        app.form.username = "ada".into();
        terminal.draw(|f| render(&mut app, f)).unwrap();
        assert_eq!(
            terminal.get_cursor_position().unwrap(),
            labeled_cursor(popup, 1, FIELD_LABEL_WIDTH, "ada").unwrap()
        );

        app.begin_search();
        app.filter = "git".into();
        terminal.draw(|f| render(&mut app, f)).unwrap();
        let header = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area)[0];
        let title = format!(" pwstash — {} ", app.vault.path().display());
        let filter = format!("  /{}", app.filter);
        let inner = inner_block(header);
        let expected = clamp_cursor(
            inner,
            inner
                .x
                .saturating_add(display_width(&title))
                .saturating_add(display_width(&filter)),
            inner.y,
        )
        .unwrap();
        assert_eq!(terminal.get_cursor_position().unwrap(), expected);
    }
}
