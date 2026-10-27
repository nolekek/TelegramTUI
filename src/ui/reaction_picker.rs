//! S07-R: Interactive Reaction Picker Modal
//! Quick emoji reaction selection (1-6) on any message.

use crate::actors::coordinator::AppCoordinator;
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_reaction_picker(f: &mut Frame, _app: &AppCoordinator, theme: &Theme, area: Rect) {
    let width = 38.min(area.width);
    let height = 7.min(area.height);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Add Reaction (+) ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(theme.border_focused())
        .border_type(theme.border_type)
        .style(theme.surface());

    let lines = vec![
        Line::from(vec![
            Span::styled(" [1] ", theme.badge_unread(false)),
            Span::raw("👍   "),
            Span::styled("[2] ", theme.badge_unread(false)),
            Span::raw("❤️   "),
            Span::styled("[3] ", theme.badge_unread(false)),
            Span::raw("🔥"),
        ]),
        Line::from(vec![
            Span::styled(" [4] ", theme.badge_unread(false)),
            Span::raw("🎉   "),
            Span::styled("[5] ", theme.badge_unread(false)),
            Span::raw("👏   "),
            Span::styled("[6] ", theme.badge_unread(false)),
            Span::raw("🤔"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press 1-6 to toggle | [Esc] Close", theme.dialog_snippet()),
        ]),
    ];

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}
