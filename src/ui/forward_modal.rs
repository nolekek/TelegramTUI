//! S11: Forward Message Modal with Privacy Toggle
//! Target chat selector with [x] Hide Author option.

use crate::actors::coordinator::AppCoordinator;
use crate::model::ActiveModal;
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_forward_modal(f: &mut Frame, app: &AppCoordinator, theme: &Theme, area: Rect) {
    let width = 50.min(area.width);
    let height = 15.min(area.height);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let (selected_idx, hide_author) = if let ActiveModal::ForwardModal { selected_chat_idx, hide_author, .. } = app.active_modal {
        (selected_chat_idx, hide_author)
    } else {
        (0, false)
    };

    let block = Block::default()
        .title(" Forward Message (S11) ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(theme.border_focused())
        .border_type(theme.border_type)
        .style(theme.surface());

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Select destination chat:", theme.message_author()),
        ]),
        Line::from(""),
    ];

    for (i, chat) in app.chats.iter().enumerate() {
        let is_selected = i == selected_idx;
        let prefix = if is_selected { "▶ " } else { "  " };
        let style = if is_selected { theme.dialog_item(true) } else { theme.dialog_item(false) };

        lines.push(Line::from(vec![
            Span::styled(prefix, theme.border_focused()),
            Span::styled(chat.title.as_str(), style),
        ]));
    }

    lines.push(Line::from(""));
    let checkbox = if hide_author { "[X] Hide sender name" } else { "[ ] Hide sender name" };
    lines.push(Line::from(vec![
        Span::styled(checkbox, theme.badge_unread(false)),
        Span::raw(" (Press Space to toggle)"),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("[Enter] Send | [↑/↓] Select Chat | [Esc] Cancel", theme.dialog_snippet()),
    ]));

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}
