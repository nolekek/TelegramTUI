//! S12: Settings & Keyboard Cheatsheet Modal

use crate::actors::coordinator::AppCoordinator;
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_settings_help_modal(f: &mut Frame, _app: &AppCoordinator, theme: &Theme, area: Rect) {
    let width = 56.min(area.width);
    let height = 18.min(area.height);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" TgTui Settings & Cheatsheet (S12) ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(theme.border_focused())
        .border_type(theme.border_type)
        .style(theme.surface());

    let lines = vec![
        Line::from(Span::styled("--- Navigation ---", theme.border_focused())),
        Line::from(vec![
            Span::styled("  Tab          ", theme.badge_unread(false)),
            Span::raw("Switch active pane (Folders -> Chats -> Messages -> Composer)"),
        ]),
        Line::from(vec![
            Span::styled("  j / k / ↑ / ↓", theme.badge_unread(false)),
            Span::raw("Navigate items in active pane"),
        ]),
        Line::from(vec![
            Span::styled("  1 .. 4       ", theme.badge_unread(false)),
            Span::raw("Switch chat folders (All, Personal, Groups, Channels)"),
        ]),
        Line::from(""),
        Line::from(Span::styled("--- Messaging ---", theme.border_focused())),
        Line::from(vec![
            Span::styled("  Enter        ", theme.badge_unread(false)),
            Span::raw("Send message / Select item"),
        ]),
        Line::from(vec![
            Span::styled("  r            ", theme.badge_unread(false)),
            Span::raw("Reply to selected message"),
        ]),
        Line::from(vec![
            Span::styled("  e            ", theme.badge_unread(false)),
            Span::raw("Edit selected message (own)"),
        ]),
        Line::from(vec![
            Span::styled("  d            ", theme.danger()),
            Span::raw("Delete selected message"),
        ]),
        Line::from(vec![
            Span::styled("  m            ", theme.badge_unread(false)),
            Span::raw("Open message context menu (S07)"),
        ]),
        Line::from(vec![
            Span::styled("  /            ", theme.badge_unread(false)),
            Span::raw("In-chat search (S10)"),
        ]),
        Line::from(vec![
            Span::styled("  i            ", theme.badge_unread(false)),
            Span::raw("Chat profile & shared media (S05/S06)"),
        ]),
        Line::from(vec![
            Span::styled("  F1           ", theme.dialog_snippet()),
            Span::raw("Toggle Auth Gate Demo (S01)"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press [Esc] or [q] to close", theme.dialog_snippet()),
        ]),
    ];

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}
