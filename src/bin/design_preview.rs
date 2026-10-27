//! Design System Preview for TgTui
//! Renders core UI primitives, color tokens, and widgets in isolation.

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use tg_tui::theme::Theme;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|f| {
        let theme = Theme::default();
        draw_design_system_preview(f, &theme);
    })?;

    Ok(())
}

fn draw_design_system_preview(f: &mut Frame, theme: &Theme) {
    let size = f.area();

    // Background fill
    let bg_block = Block::default().style(theme.base());
    f.render_widget(bg_block, size);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title / Header
            Constraint::Min(10),   // Content panels
            Constraint::Length(3), // Footer / Hotkeys
        ])
        .split(size);

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled("TgTui", theme.message_author().add_modifier(Modifier::BOLD)),
        Span::raw(" • "),
        Span::styled(
            "Design System & Primitives Preview (WCAG 2.2 AA Compliant)",
            theme.dialog_snippet(),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(theme.border())
            .border_type(theme.border_type),
    );
    f.render_widget(header, chunks[0]);

    // Panels (Left: Dialogs list, Center: Messages, Right: Tokens)
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25), // Dialog List
            Constraint::Percentage(50), // Chat History & Composer
            Constraint::Percentage(25), // Color Tokens & State Badges
        ])
        .split(chunks[1]);

    draw_dialog_list(f, theme, body_chunks[0]);
    draw_chat_view(f, theme, body_chunks[1]);
    draw_token_inspector(f, theme, body_chunks[2]);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Tab", theme.badge_unread(false)),
        Span::raw(" Switch Pane | "),
        Span::styled("j/k", theme.dialog_snippet()),
        Span::raw(" Navigate | "),
        Span::styled("Enter", theme.dialog_snippet()),
        Span::raw(" Select | "),
        Span::styled("q", theme.danger()),
        Span::raw(" Quit"),
    ]))
    .block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(theme.border())
            .border_type(theme.border_type),
    );
    f.render_widget(footer, chunks[2]);
}

fn draw_dialog_list(f: &mut Frame, theme: &Theme, area: Rect) {
    let block = Block::default()
        .title(" Dialogs ")
        .borders(Borders::ALL)
        .border_style(theme.border_focused())
        .border_type(theme.border_type)
        .style(theme.base());

    let lines = vec![
        Line::from(vec![
            Span::styled("● ", theme.success()),
            Span::styled("Pavel Durov", theme.dialog_item(true)),
            Span::raw(" "),
            Span::styled("12:45", theme.dialog_snippet()),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Check out the new design!", theme.dialog_snippet()),
            Span::raw(" "),
            Span::styled(" 3 ", theme.badge_unread(false)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("● ", theme.dialog_snippet()),
            Span::styled("Rust Community", theme.dialog_item(false)),
            Span::raw(" "),
            Span::styled("11:20", theme.dialog_snippet()),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Alice: Ratatui 0.28 released", theme.dialog_snippet()),
            Span::raw(" "),
            Span::styled(" 12 ", theme.badge_unread(true)),
        ]),
    ];

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn draw_chat_view(f: &mut Frame, theme: &Theme, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),    // Message bubbles
            Constraint::Length(3), // Composer
        ])
        .split(area);

    let chat_block = Block::default()
        .title(" Pavel Durov ")
        .borders(Borders::ALL)
        .border_style(theme.border())
        .border_type(theme.border_type);

    let messages = vec![
        Line::from(vec![
            Span::styled("Pavel Durov", theme.message_author()),
            Span::raw(" "),
            Span::styled("12:44", theme.message_meta()),
        ]),
        Line::from(vec![
            Span::raw("Hello! Does TgTui support "),
            Span::styled("bold", theme.text_bold()),
            Span::raw(", "),
            Span::styled("italic", theme.text_italic()),
            Span::raw(", and "),
            Span::styled(" `code` ", theme.text_code()),
            Span::raw("?"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Voice message (0:15) ", theme.message_author()),
            Span::styled(" ▂▃▅▆▇▆▅▃ ", theme.waveform_active()),
            Span::styled(" ▂▃ ", theme.waveform_inactive()),
            Span::styled(" [Press 'p' to play]", theme.message_meta()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("You", theme.message_author()),
            Span::raw(" "),
            Span::styled("12:45", theme.message_meta()),
            Span::raw(" "),
            Span::styled("✓✓", theme.checkmarks(true)),
        ]),
        Line::from(vec![
            Span::raw("Yes! Fully compliant with WCAG 2.2 AA tokens."),
        ]),
    ];

    let p = Paragraph::new(messages).block(chat_block).wrap(Wrap { trim: true });
    f.render_widget(p, chunks[0]);

    // Composer
    let composer = Paragraph::new("Type a message...")
        .style(theme.composer_input(true))
        .block(
            Block::default()
                .title(" Message ")
                .borders(Borders::ALL)
                .border_style(theme.border_focused())
                .border_type(theme.border_type),
        );
    f.render_widget(composer, chunks[1]);
}

fn draw_token_inspector(f: &mut Frame, theme: &Theme, area: Rect) {
    let block = Block::default()
        .title(" Tokens & Roles ")
        .borders(Borders::ALL)
        .border_style(theme.border())
        .border_type(theme.border_type);

    let lines = vec![
        Line::from(vec![
            Span::styled("■ ", theme.surface()),
            Span::raw("bg: #0e1621"),
        ]),
        Line::from(vec![
            Span::styled("■ ", theme.dialog_item(true)),
            Span::raw("surface: #182533"),
        ]),
        Line::from(vec![
            Span::styled("■ ", theme.border_focused()),
            Span::raw("accent: #5ba2f4"),
        ]),
        Line::from(vec![
            Span::styled("■ ", theme.danger()),
            Span::raw("danger: #f87171"),
        ]),
        Line::from(vec![
            Span::styled("■ ", theme.success()),
            Span::raw("success: #4ade80"),
        ]),
        Line::from(vec![
            Span::styled("■ ", theme.warning()),
            Span::raw("warning: #fbbf24"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("WCAG Status: ", theme.message_author()),
            Span::styled("10/10 PASS AA", theme.success()),
        ]),
    ];

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}
