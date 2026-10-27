//! S03: Active Chat History Widget
//! Renders chat header, chronological message bubbles, rich text, voice waveforms, and delivery ticks.

use crate::actors::coordinator::AppCoordinator;
use crate::model::{DeliveryStatus, FocusPane, MessageMedia};
use crate::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render_messages_panel(f: &mut Frame, app: &AppCoordinator, theme: &Theme, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Chat Header
            Constraint::Min(6),    // Messages History
        ])
        .split(area);

    let is_focused = app.focus == FocusPane::Messages;

    // 1. Header
    let current_chat = app.current_chat();
    let header_text = if let Some(chat) = current_chat {
        vec![
            Line::from(vec![
                Span::styled(chat.title.as_str(), theme.message_author().add_modifier(Modifier::BOLD)),
                Span::raw(" • "),
                Span::styled("Online", theme.success()),
                Span::raw(" | "),
                Span::styled("[i] Info", theme.dialog_snippet()),
                Span::raw(" | "),
                Span::styled("[/] Search", theme.dialog_snippet()),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled("Select a chat to start messaging", theme.dialog_snippet()))]
    };

    let header_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(theme.border())
        .border_type(theme.border_type);

    let header = Paragraph::new(header_text).block(header_block);
    f.render_widget(header, chunks[0]);

    // 2. Message History
    let history_block = Block::default()
        .title(" Conversation History ")
        .borders(Borders::ALL)
        .border_style(if is_focused { theme.border_focused() } else { theme.border() })
        .border_type(theme.border_type)
        .style(theme.base());

    let msgs = app.current_messages();
    let mut lines = Vec::new();

    if msgs.is_empty() {
        lines.push(Line::from(Span::styled("No messages yet. Say hello!", theme.dialog_snippet())));
    } else {
        for (i, msg) in msgs.iter().enumerate() {
            let is_msg_selected = app.selected_msg_idx == Some(i);

            // Selection indicator
            let sel_prefix = if is_msg_selected { "▶ " } else { "  " };

            // Status tick for outgoing
            let status_span = if msg.is_outgoing {
                match msg.status {
                    DeliveryStatus::Pending => Span::styled(" ⏱", theme.dialog_snippet()),
                    DeliveryStatus::Sent => Span::styled(" ✓", theme.checkmarks(false)),
                    DeliveryStatus::Read => Span::styled(" ✓✓", theme.checkmarks(true)),
                    DeliveryStatus::Failed => Span::styled(" ⚠", theme.danger()),
                }
            } else {
                Span::raw("")
            };

            // Sender & Date line
            let sender_style = if msg.is_outgoing {
                theme.dialog_item(true)
            } else {
                theme.message_author()
            };

            lines.push(Line::from(vec![
                Span::styled(sel_prefix, theme.border_focused()),
                Span::styled(msg.sender_name.as_str(), sender_style),
                Span::raw(" "),
                Span::styled(msg.date.as_str(), theme.message_meta()),
                status_span,
                if msg.is_edited {
                    Span::styled(" (edited)", theme.message_meta())
                } else {
                    Span::raw("")
                },
            ]));

            // Reply banner if replying to another message
            if let Some(reply) = &msg.reply_to {
                lines.push(Line::from(vec![
                    Span::raw("    ┌─ "),
                    Span::styled(reply.sender_name.as_str(), theme.message_author()),
                    Span::raw(": "),
                    Span::styled(reply.snippet.as_str(), theme.dialog_snippet()),
                ]));
            }

            // Message Body (with media and formatting)
            if let Some(media) = &msg.media {
                match media {
                    MessageMedia::Voice { duration_sec, waveform } => {
                        lines.push(Line::from(vec![
                            Span::raw("    🎤 Voice (0:"),
                            Span::raw(format!("{:02}) ", duration_sec)),
                            Span::styled(waveform.as_str(), theme.waveform_active()),
                            Span::styled(" [p: play]", theme.message_meta()),
                        ]));
                    }
                    MessageMedia::Document { filename, size_bytes, upload_progress } => {
                        let size_kb = size_bytes / 1024;
                        let progress_str = if let Some(p) = upload_progress {
                            format!(" [Upload: {}%]", p)
                        } else {
                            String::new()
                        };
                        lines.push(Line::from(vec![
                            Span::raw("    📄 Document: "),
                            Span::styled(filename.as_str(), theme.dialog_item(true)),
                            Span::styled(format!(" ({} KB){}", size_kb, progress_str), theme.message_meta()),
                            Span::styled(" [o: open]", theme.message_meta()),
                        ]));
                    }
                    MessageMedia::Photo { filename, width, height, is_compressed } => {
                        let comp_str = if *is_compressed { " (compressed)" } else { " (uncompressed)" };
                        lines.push(Line::from(vec![
                            Span::raw("    🖼 Photo: "),
                            Span::styled(filename.as_str(), theme.dialog_item(true)),
                            Span::styled(format!(" [{}x{}]{}", width, height, comp_str), theme.message_meta()),
                            Span::styled(" [Enter: View Lightbox]", theme.badge_unread(false)),
                        ]));
                    }
                }
            }

            // Text content
            let bubble_style = if is_msg_selected {
                theme.dialog_item(true)
            } else if msg.is_outgoing {
                theme.message_outgoing()
            } else {
                theme.message_incoming()
            };

            if !msg.text.is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(msg.text.as_str(), bubble_style),
                ]));
            }

            // Reactions bar
            if !msg.reactions.is_empty() || is_msg_selected {
                let mut reaction_spans = vec![Span::raw("    ")];
                for r in &msg.reactions {
                    let pill_style = if r.chosen_by_me {
                        theme.badge_unread(false)
                    } else {
                        theme.dialog_snippet()
                    };
                    reaction_spans.push(Span::styled(format!(" [{}: {}] ", r.emoji, r.count), pill_style));
                }
                if is_msg_selected {
                    reaction_spans.push(Span::styled("[+ react] ", theme.dialog_snippet()));
                    reaction_spans.push(Span::styled("[f: forward] ", theme.dialog_snippet()));
                }
                lines.push(Line::from(reaction_spans));
            }

            lines.push(Line::from("")); // Space between bubbles
        }
    }

    let history = Paragraph::new(lines).block(history_block).wrap(Wrap { trim: false });
    f.render_widget(history, chunks[1]);
}
