use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, Paragraph, Wrap};

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
        Mode::ConfirmDelete => render_confirm(app, frame, area),
        Mode::Help => render_help(frame, area),
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
            title,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw(filter),
    ]))
    .block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Vault")
            .title_alignment(Alignment::Left),
    );
    frame.render_widget(header, area);
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
            ListItem::new(format!("{}  ({})", entry.service, entry.username))
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
            Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("Service:  ", Style::default().fg(ACCENT)),
                    Span::raw(&entry.service),
                ]),
                Line::from(vec![
                    Span::styled("Username: ", Style::default().fg(ACCENT)),
                    Span::raw(&entry.username),
                ]),
                Line::from(vec![
                    Span::styled("Password: ", Style::default().fg(ACCENT)),
                    Span::raw(password),
                ]),
            ])
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
        Mode::ConfirmDelete => "[y] delete  [n] cancel",
        Mode::Help => "Esc or ? close help",
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
    let popup = centered_rect(60, 11, area);
    frame.render_widget(Clear, popup);

    let title = match app.mode {
        Mode::Add => "Add entry",
        Mode::Edit => "Edit entry",
        _ => "Form",
    };

    let service_style = field_style(app, 0);
    let user_style = field_style(app, add_field_index(app.mode, 1));
    let pass_style = field_style(app, add_field_index(app.mode, 2));

    let password_display: String = "*".repeat(app.form.password.chars().count());
    let mut lines = Vec::new();
    if app.mode == Mode::Add {
        lines.push(Line::from(vec![
            Span::raw("Service:  "),
            Span::styled(app.form.service.clone(), service_style),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("Service:  "),
            Span::raw(app.form.service.clone()),
        ]));
    }
    lines.push(Line::from(vec![
        Span::raw("Username: "),
        Span::styled(app.form.username.clone(), user_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw("Password: "),
        Span::styled(password_display, pass_style),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(
        "Ctrl-G generates a password. Enter saves, Esc cancels.",
    ));

    let form = Paragraph::new(lines).block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(title)
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(Color::White).bg(Color::Black)),
    );
    frame.render_widget(form, popup);
}

fn add_field_index(mode: Mode, add_index: usize) -> usize {
    match mode {
        Mode::Add => add_index,
        Mode::Edit => add_index.saturating_sub(1),
        _ => add_index,
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
        Line::from("a / e / d       add / edit / delete"),
        Line::from("P               change master password"),
        Line::from("Ctrl-G          generate in a form"),
        Line::from("q / Ctrl-C      quit"),
        Line::from(""),
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
        vault.add("github", "ada", "secret").unwrap();
        vault.add("mail", "ada", "secret2").unwrap();
        let mut app = App::new(vault);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(&mut app, f)).unwrap();
        assert!(buffer_has(terminal.backend().buffer(), "github"));
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
    }
}
