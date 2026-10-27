//! Attachment Dialog: Send Photos & Documents with Compression Choice

use crate::actors::coordinator::AppCoordinator;
use crate::model::ActiveModal;
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_attach_modal(f: &mut Frame, app: &AppCoordinator, theme: &Theme, area: Rect) {
    let width = 56.min(area.width);
    let height = 12.min(area.height);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let (path_buffer, is_photo, compress) = if let ActiveModal::AttachFile { ref path_buffer, is_photo, compress } = app.active_modal {
        (path_buffer.as_str(), is_photo, compress)
    } else {
        ("", true, true)
    };

    let block = Block::default()
        .title(" Send File / Photo Attachment ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(theme.border_focused())
        .border_type(theme.border_type)
        .style(theme.surface());

    let compress_label = if compress { "[X] Compress photo (fast preview)" } else { "[ ] Send uncompressed file (full quality)" };

    let lines = vec![
        Line::from(vec![
            Span::styled("Enter file path on local disk:", theme.message_author()),
        ]),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(if path_buffer.is_empty() { "C:/Users/.../photo.jpg" } else { path_buffer }, theme.dialog_item(true)),
            Span::styled("█", theme.border_focused()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(compress_label, theme.badge_unread(false)),
            Span::raw(" (Press Tab to toggle)"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Type: ", theme.dialog_snippet()),
            Span::raw(if is_photo { "Photo (JPG/PNG)" } else { "Document (up to 2GB)" }),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Enter] Send Attachment | [Esc] Cancel", theme.dialog_snippet()),
        ]),
    ];

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}
