//! S02: Left Column — Folders Sidebar and Dialogs List Widget
//! Renders active folders, unread badges, pin indicators, and search trigger.

use crate::actors::coordinator::AppCoordinator;
use crate::model::{ChatType, FocusPane};
use crate::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render_dialogs_panel(f: &mut Frame, app: &AppCoordinator, theme: &Theme, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Folders Header / Bar
            Constraint::Min(5),    // Dialog List
        ])
        .split(area);

    // 1. Folders Bar
    let is_folders_focused = app.focus == FocusPane::Folders;
    let mut folder_spans = Vec::new();

    for (i, folder) in app.folders.iter().enumerate() {
        let is_active = i == app.active_folder_idx;
        let prefix = if is_active { "[" } else { " " };
        let suffix = if is_active { "]" } else { " " };

        let style = if is_active {
            theme.badge_unread(false)
        } else {
            theme.dialog_snippet()
        };

        folder_spans.push(Span::styled(format!("{}{}{} ", prefix, folder.title, suffix), style));
    }

    let folders_block = Block::default()
        .title(" Folders (1-4) ")
        .borders(Borders::ALL)
        .border_style(if is_folders_focused { theme.border_focused() } else { theme.border() })
        .border_type(theme.border_type);

    let folders_widget = Paragraph::new(Line::from(folder_spans)).block(folders_block);
    f.render_widget(folders_widget, chunks[0]);

    // 2. Dialogs List
    let is_dialogs_focused = app.focus == FocusPane::Dialogs;
    let dialogs_block = Block::default()
        .title(" Chats ")
        .borders(Borders::ALL)
        .border_style(if is_dialogs_focused { theme.border_focused() } else { theme.border() })
        .border_type(theme.border_type)
        .style(theme.base());

    let filtered = app.filtered_chats();
    let mut lines = Vec::new();

    if filtered.is_empty() {
        lines.push(Line::from(Span::styled("No chats in this folder", theme.dialog_snippet())));
    } else {
        for (i, chat) in filtered.iter().enumerate() {
            let is_selected = i == app.selected_chat_idx;

            // Row 1: Pin + Type icon + Title + Timestamp
            let type_icon = match chat.chat_type {
                ChatType::Private => "👤",
                ChatType::Group => "👥",
                ChatType::Supergroup => "🌐",
                ChatType::Channel => "📢",
            };

            let pin_icon = if chat.is_pinned { "📌 " } else { "" };
            let mute_icon = if chat.is_muted { "🔇 " } else { "" };

            let title_style = if is_selected {
                theme.dialog_item(true)
            } else {
                theme.dialog_item(false)
            };

            let mut row1 = vec![
                Span::raw(pin_icon),
                Span::raw(mute_icon),
                Span::raw(format!("{} ", type_icon)),
                Span::styled(chat.title.as_str(), title_style),
                Span::raw(" "),
                Span::styled(chat.last_message_date.as_str(), theme.dialog_snippet()),
            ];

            // Row 2: Snippet + Unread badge
            let mut row2 = vec![
                Span::raw("   "),
                Span::styled(
                    if chat.last_message_snippet.chars().count() > 22 {
                        let short: String = chat.last_message_snippet.chars().take(20).collect();
                        format!("{}...", short)
                    } else {
                        chat.last_message_snippet.clone()
                    },
                    theme.dialog_snippet(),
                ),
            ];

            if chat.unread_count > 0 {
                row2.push(Span::raw(" "));
                row2.push(Span::styled(
                    format!(" {} ", chat.unread_count),
                    theme.badge_unread(chat.is_muted),
                ));
            }

            if is_selected {
                row1.insert(0, Span::styled("▶ ", theme.border_focused()));
            } else {
                row1.insert(0, Span::raw("  "));
            }

            lines.push(Line::from(row1));
            lines.push(Line::from(row2));
            lines.push(Line::from("")); // Separator line
        }
    }

    let dialogs_widget = Paragraph::new(lines).block(dialogs_block);
    f.render_widget(dialogs_widget, chunks[1]);
}
