//! S07: Message Context Menu Modal
//! Quick actions: Reply, Edit, Copy, Delete, Forward.

use crate::actors::coordinator::AppCoordinator;
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_context_menu(f: &mut Frame, _app: &AppCoordinator, theme: &Theme, area: Rect) {
    let width = 36.min(area.width);
    let height = 10.min(area.height);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Message Actions (S07) ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(theme.border_focused())
        .border_type(theme.border_type)
        .style(theme.surface());

    let lines = vec![
        Line::from(vec![
            Span::styled(" [r] ", theme.badge_unread(false)),
            Span::raw(" Reply to message"),
        ]),
        Line::from(vec![
            Span::styled(" [e] ", theme.dialog_snippet()),
            Span::raw(" Edit message (own only)"),
        ]),
        Line::from(vec![
            Span::styled(" [f] ", theme.dialog_snippet()),
            Span::raw(" Forward with privacy"),
        ]),
        Line::from(vec![
            Span::styled(" [d] ", theme.danger()),
            Span::raw(" Delete message"),
        ]),
        Line::from(vec![
            Span::styled(" [p] ", theme.waveform_active()),
            Span::raw(" Play voice note"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [Esc] ", theme.dialog_snippet()),
            Span::raw(" Close menu"),
        ]),
    ];

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}
