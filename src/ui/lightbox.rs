//! S08: Full-Screen Media Viewer / Lightbox Widget
//! High-resolution photo inspection with zoom and external viewer launcher.

use crate::actors::coordinator::AppCoordinator;
use crate::model::ActiveModal;
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_lightbox(f: &mut Frame, app: &AppCoordinator, theme: &Theme, area: Rect) {
    let width = 64.min(area.width);
    let height = 16.min(area.height);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let (title, dimensions, zoom) = if let ActiveModal::Lightbox { ref title, ref dimensions, zoom_percent } = app.active_modal {
        (title.as_str(), dimensions.as_str(), zoom_percent)
    } else {
        ("Photo", "1920x1080", 100)
    };

    let block = Block::default()
        .title(format!(" Media Viewer Lightbox (S08) — {} ", title))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(theme.border_focused())
        .border_type(theme.border_type)
        .style(theme.surface());

    let lines = vec![
        Line::from(vec![
            Span::styled("Resolution: ", theme.dialog_snippet()),
            Span::styled(dimensions, theme.dialog_item(true)),
            Span::raw(" | "),
            Span::styled("Zoom: ", theme.dialog_snippet()),
            Span::styled(format!("{}%", zoom), theme.message_author()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("┌──────────────────────────────────────────────┐", theme.border()),
        ]),
        Line::from(vec![
            Span::styled("│                  [ IMAGE ]                   │", theme.border_focused()),
        ]),
        Line::from(vec![
            Span::styled("│           High-Resolution Preview            │", theme.message_author()),
        ]),
        Line::from(vec![
            Span::styled("│                                              │", theme.border()),
        ]),
        Line::from(vec![
            Span::styled("└──────────────────────────────────────────────┘", theme.border()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [o] ", theme.badge_unread(false)),
            Span::raw("Open in system default viewer / mpv"),
        ]),
        Line::from(vec![
            Span::styled(" [s] ", theme.badge_unread(false)),
            Span::raw("Save to Downloads"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Esc] or [q] Close lightbox", theme.dialog_snippet()),
        ]),
    ];

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}
