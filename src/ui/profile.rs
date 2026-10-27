//! S05: Chat Profile & Shared Media Panel Modal
//! Displays participant info, notification toggles, and shared media preview.

use crate::actors::coordinator::AppCoordinator;
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_profile_modal(f: &mut Frame, app: &AppCoordinator, theme: &Theme, area: Rect) {
    let width = 50.min(area.width);
    let height = 16.min(area.height);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let title = app.current_chat().map(|c| c.title.as_str()).unwrap_or("Chat Info");
    let block = Block::default()
        .title(format!(" {} (S05) ", title))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(theme.border_focused())
        .border_type(theme.border_type)
        .style(theme.surface());

    let lines = vec![
        Line::from(vec![
            Span::styled("● Status: ", theme.dialog_snippet()),
            Span::styled("Online", theme.success()),
        ]),
        Line::from(vec![
            Span::styled("Username: ", theme.dialog_snippet()),
            Span::styled("@durov", theme.message_author()),
        ]),
        Line::from(vec![
            Span::styled("Bio: ", theme.dialog_snippet()),
            Span::raw("Building Telegram and decentralized tech."),
        ]),
        Line::from(""),
        Line::from(Span::styled("--- Shared Media (S06) ---", theme.border_focused())),
        Line::from(vec![
            Span::styled("🖼 Photos: ", theme.dialog_snippet()),
            Span::raw("14 items"),
        ]),
        Line::from(vec![
            Span::styled("📄 Files: ", theme.dialog_snippet()),
            Span::raw("3 documents (ratatui-guide.pdf)"),
        ]),
        Line::from(vec![
            Span::styled("🎤 Voice: ", theme.dialog_snippet()),
            Span::raw("6 voice messages"),
        ]),
        Line::from(vec![
            Span::styled("🔗 Links: ", theme.dialog_snippet()),
            Span::raw("22 shared links"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press [Esc] or [q] to close", theme.dialog_snippet()),
        ]),
    ];

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}
