//! S04: Message Composer Widget
//! Renders active reply/edit banner, auto-sizing text input, typing cursor, and formatting helpers.

use crate::actors::coordinator::AppCoordinator;
use crate::model::FocusPane;
use crate::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render_composer_panel(f: &mut Frame, app: &AppCoordinator, theme: &Theme, area: Rect) {
    let has_banner = app.composer_reply_to.is_some() || app.composer_edit_id.is_some();

    let chunks = if has_banner {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Reply/Edit banner
                Constraint::Min(2),    // Input box
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(2)])
            .split(area)
    };

    let is_focused = app.focus == FocusPane::Composer;

    // Optional Reply / Edit Banner
    if has_banner {
        let banner_spans = if let Some(reply) = &app.composer_reply_to {
            vec![
                Span::styled("↩ Replying to ", theme.reply_bar()),
                Span::styled(reply.sender_name.as_str(), theme.message_author()),
                Span::raw(": "),
                Span::styled(reply.snippet.as_str(), theme.dialog_snippet()),
                Span::styled(" [Esc to cancel]", theme.danger()),
            ]
        } else if app.composer_edit_id.is_some() {
            vec![
                Span::styled("✎ Editing message ", theme.reply_bar()),
                Span::styled(" [Esc to cancel]", theme.danger()),
            ]
        } else {
            vec![]
        };

        let banner = Paragraph::new(Line::from(banner_spans));
        f.render_widget(banner, chunks[0]);
    }

    // Input Area
    let input_area = if has_banner { chunks[1] } else { chunks[0] };

    let placeholder = "Write a message... (Enter to send, Tab to navigate)";
    let text = if app.composer_buffer.is_empty() {
        if is_focused {
            vec![
                Span::styled("█", theme.border_focused()),
                Span::raw(" "),
                Span::styled(placeholder, theme.dialog_snippet()),
            ]
        } else {
            vec![Span::styled(placeholder, theme.dialog_snippet())]
        }
    } else {
        vec![
            Span::styled(app.composer_buffer.as_str(), theme.dialog_item(true)),
            if is_focused {
                Span::styled("█", theme.border_focused())
            } else {
                Span::raw("")
            },
        ]
    };

    let input_block = Block::default()
        .title(" Message Composer ")
        .borders(Borders::ALL)
        .border_style(if is_focused { theme.border_focused() } else { theme.border() })
        .border_type(theme.border_type)
        .style(theme.composer_input(is_focused));

    // Calculate horizontal scroll offset so typing long text scrolls smoothly instead of clipping
    let inner_width = input_area.width.saturating_sub(4) as usize;
    let char_count = app.composer_buffer.chars().count();
    let scroll_x = if char_count > inner_width {
        (char_count - inner_width) as u16
    } else {
        0
    };

    let input_widget = Paragraph::new(Line::from(text))
        .block(input_block)
        .scroll((0, scroll_x));
    f.render_widget(input_widget, input_area);
}
