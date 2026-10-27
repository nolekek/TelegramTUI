//! S10: In-Chat and Global Search Widget

use crate::actors::coordinator::AppCoordinator;
use crate::model::ActiveModal;
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_search_modal(f: &mut Frame, app: &AppCoordinator, theme: &Theme, area: Rect) {
    let width = 48.min(area.width);
    let height = 8.min(area.height);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + 4,
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let query = if let ActiveModal::Search { query } = &app.active_modal {
        query.as_str()
    } else {
        ""
    };

    let block = Block::default()
        .title(" Search Messages (S10) ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(theme.border_focused())
        .border_type(theme.border_type)
        .style(theme.surface());

    let lines = vec![
        Line::from(vec![
            Span::raw("Query: "),
            Span::styled(if query.is_empty() { "(type to search...)" } else { query }, theme.dialog_item(true)),
            Span::styled("█", theme.border_focused()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Filter: ", theme.dialog_snippet()),
            Span::raw("All | Media | Files | Links"),
        ]),
        Line::from(vec![
            Span::styled("[Esc] Close | [Enter] Jump to message", theme.dialog_snippet()),
        ]),
    ];

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}
