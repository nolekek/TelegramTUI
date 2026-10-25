use chrono::DateTime;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use tg_core::{ChatId, CoreState, FileInfo, FileTransferStatus, MessageContent, MessageStatus, Sender};

#[path = "image.rs"]
pub mod image;

use crate::text::entities::format_text_entities;

/// Render the conversation history viewport for the selected chat.
pub fn render_messages(
    frame: &mut Frame,
    state: &CoreState,
    active_chat_id: Option<ChatId>,
    selected_message_index: usize,
    scroll_offset: usize,
    is_focused: bool,
    area: Rect,
) {
    let border_style = if is_focused {
        Style::default()
            .fg(Color::Rgb(88, 166, 255))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Rgb(70, 80, 110))
    };

    let chat = active_chat_id.and_then(|id| state.get_chat(id));
    let title = if let Some(c) = chat {
        let badge = match c.chat_type {
            tg_core::ChatType::Private | tg_core::ChatType::Secret => " [Личный]",
            tg_core::ChatType::BasicGroup | tg_core::ChatType::Supergroup => " [Группа]",
            tg_core::ChatType::Channel => " [Канал]",
        };
        format!(" 💬 {}{} ", c.title, badge)
    } else {
        " 💬 Переписка ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .title(title)
        .border_style(border_style);

    let chat_id = match active_chat_id {
        Some(id) => id,
        None => {
            let empty_text = vec![
                Line::default(),
                Line::from(vec![
                    Span::styled(
                        "Чат не выбран. Нажмите [/] или [Esc] для перехода к поиску чатов.",
                        Style::default().fg(Color::Rgb(108, 112, 134)),
                    ),
                ]),
            ];
            let paragraph = Paragraph::new(empty_text).block(block);
            frame.render_widget(paragraph, area);
            return;
        }
    };

    let messages = state.messages_for_chat(chat_id);
    if messages.is_empty() {
        let empty_text = vec![
            Line::default(),
            Line::from(vec![
                Span::styled(
                    "В этом чате пока нет сообщений. Нажмите 'i' или 'Enter' для ввода сообщения.",
                    Style::default().fg(Color::Rgb(108, 112, 134)),
                ),
            ]),
        ];
        let paragraph = Paragraph::new(empty_text).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    let mut rendered_lines: Vec<Line<'static>> = Vec::new();

    for (idx, msg) in messages.iter().enumerate() {
        let is_selected = is_focused && idx == selected_message_index;

        // Prefix cursor
        let cursor_span = if is_selected {
            Span::styled("▌ ", Style::default().fg(Color::Rgb(125, 207, 255)).add_modifier(Modifier::BOLD))
        } else {
            Span::raw("  ")
        };

        // Timestamp [HH:MM]
        let time_str = if let Some(utc_dt) = DateTime::from_timestamp(msg.date, 0) {
            utc_dt.format("[%H:%M]").to_string()
        } else {
            "[--:--]".to_string()
        };
        let time_span = Span::styled(time_str, Style::default().fg(Color::Rgb(108, 112, 134)));

        // Sender Name
        let (sender_span, prefix_color) = if msg.is_outgoing {
            (
                Span::styled("Вы", Style::default().fg(Color::Rgb(115, 218, 154)).add_modifier(Modifier::BOLD)),
                Color::Rgb(115, 218, 154),
            )
        } else {
            let name = match msg.sender {
                Sender::User(uid) => state
                    .get_user(uid)
                    .map(|u| u.full_name())
                    .unwrap_or_else(|| format!("User #{}", uid.0)),
                Sender::Chat(cid) => state
                    .get_chat(cid)
                    .map(|c| c.title.clone())
                    .unwrap_or_else(|| format!("Chat #{}", cid.0)),
                Sender::Unknown => "Unknown".to_string(),
            };
            (
                Span::styled(name, Style::default().fg(Color::Rgb(125, 207, 255)).add_modifier(Modifier::BOLD)),
                Color::Rgb(125, 207, 255),
            )
        };

        // Outgoing receipt
        let receipt_span = if msg.is_outgoing {
            match msg.status {
                MessageStatus::Sending => Some(Span::styled(" ⏳", Style::default().fg(Color::Rgb(250, 200, 100)))),
                MessageStatus::Sent => Some(Span::styled(" ✓", Style::default().fg(Color::Rgb(108, 112, 134)))),
                MessageStatus::Read => Some(Span::styled(" ✓✓", Style::default().fg(Color::Rgb(115, 218, 154)))),
                MessageStatus::Failed => Some(Span::styled(" ✗", Style::default().fg(Color::Rgb(247, 118, 142)))),
            }
        } else {
            None
        };

        // Reply tag
        let reply_span = msg.reply_to_message_id.map(|reply_id| {
            Span::styled(format!(" [↪ #{}]", reply_id.0), Style::default().fg(Color::Rgb(187, 154, 247)))
        });

        // Assemble the header spans for the first line
        let mut first_line_spans = vec![
            cursor_span,
            time_span,
            Span::raw(" "),
            sender_span,
        ];
        if let Some(r) = receipt_span {
            first_line_spans.push(r);
        }
        if let Some(rep) = reply_span {
            first_line_spans.push(rep);
        }
        first_line_spans.push(Span::styled(": ", Style::default().fg(prefix_color)));

        // Continuation indent for multiline messages
        let cont_indent = if is_selected {
            Span::styled("  │ ", Style::default().fg(Color::Rgb(125, 207, 255)))
        } else {
            Span::styled("    ", Style::default().fg(Color::Rgb(70, 80, 110)))
        };

        match &msg.content {
            MessageContent::Text { text, entities } => {
                let formatted = format_text_entities(text, entities);
                if formatted.is_empty() {
                    rendered_lines.push(Line::from(first_line_spans));
                } else {
                    for (i, line) in formatted.into_iter().enumerate() {
                        if i == 0 {
                            let mut full_first = first_line_spans.clone();
                            full_first.extend(line.spans);
                            rendered_lines.push(Line::from(full_first));
                        } else {
                            let mut next_line = vec![cont_indent.clone()];
                            next_line.extend(line.spans);
                            rendered_lines.push(Line::from(next_line));
                        }
                    }
                }
            }
            MessageContent::Photo {
                caption,
                file_id,
                width,
                height,
            } => {
                let file = state.get_file(*file_id);
                let size_str = file
                    .map(|f| format!(" • {}", format_bytes(f.total_size())))
                    .unwrap_or_default();
                let dim_str = if *width > 0 && *height > 0 {
                    format!(" {}x{}", width, height)
                } else {
                    String::new()
                };
                let status_hint = match file {
                    Some(f) if f.is_completed() => " [Enter/o: View  Ctrl+S: Save]",
                    Some(f) if f.is_downloading() => " [Downloading...]",
                    _ => " [Enter/o: Download & View  Ctrl+S: Save]",
                };
                let badge = format!("📷 [Photo{}{}{}]", dim_str, size_str, status_hint);
                let body_str = if caption.is_empty() {
                    badge
                } else {
                    format!("{} {}", badge, caption)
                };

                let mut spans = first_line_spans;
                spans.push(Span::styled(body_str, Style::default().fg(Color::LightBlue)));
                rendered_lines.push(Line::from(spans));
                if let Some(status_spans) = file_status_spans(file) {
                    let mut s = vec![cont_indent.clone()];
                    s.extend(status_spans);
                    rendered_lines.push(Line::from(s));
                }
            }
            MessageContent::Video {
                caption,
                file_name,
                duration,
                width,
                height,
                file_id,
                file_size,
                ..
            } => {
                let file = state.get_file(*file_id);
                let size_bytes = if *file_size > 0 {
                    *file_size as i64
                } else {
                    file.map(|f| f.total_size()).unwrap_or(0)
                };
                let dim_str = if *width > 0 && *height > 0 {
                    format!(" {}x{}", width, height)
                } else {
                    String::new()
                };
                let status_hint = match file {
                    Some(f) if f.is_completed() => " [Enter/o: Play  Ctrl+S: Save]",
                    Some(f) if f.is_downloading() => " [Downloading...]",
                    _ => " [Enter/o: Download & Play  Ctrl+S: Save]",
                };
                let badge = format!(
                    "🎬 [Video: {}{} • {} • {}{}]",
                    file_name,
                    dim_str,
                    format_duration(*duration),
                    format_bytes(size_bytes),
                    status_hint
                );
                let body_str = if caption.is_empty() {
                    badge
                } else {
                    format!("{} {}", badge, caption)
                };

                let mut spans = first_line_spans;
                spans.push(Span::styled(body_str, Style::default().fg(Color::LightGreen)));
                rendered_lines.push(Line::from(spans));
                if let Some(status_spans) = file_status_spans(file) {
                    let mut s = vec![cont_indent.clone()];
                    s.extend(status_spans);
                    rendered_lines.push(Line::from(s));
                }
            }
            MessageContent::VideoNote {
                duration,
                length,
                file_id,
                file_size,
            } => {
                let file = state.get_file(*file_id);
                let size_bytes = if *file_size > 0 {
                    *file_size as i64
                } else {
                    file.map(|f| f.total_size()).unwrap_or(0)
                };
                let status_hint = match file {
                    Some(f) if f.is_completed() => " [Enter/o: Play  Ctrl+S: Save]",
                    Some(f) if f.is_downloading() => " [Downloading...]",
                    _ => " [Enter/o: Download & Play  Ctrl+S: Save]",
                };
                let badge = format!(
                    "🎥 [Video Note (Round): {} • {} • {}px{}]",
                    format_duration(*duration),
                    format_bytes(size_bytes),
                    length,
                    status_hint
                );

                let mut spans = first_line_spans;
                spans.push(Span::styled(badge, Style::default().fg(Color::LightGreen)));
                rendered_lines.push(Line::from(spans));
                if let Some(status_spans) = file_status_spans(file) {
                    let mut s = vec![cont_indent.clone()];
                    s.extend(status_spans);
                    rendered_lines.push(Line::from(s));
                }
            }
            MessageContent::Document {
                file_name,
                file_size,
                caption,
                file_id,
            } => {
                let file = state.get_file(*file_id);
                let size_bytes = if *file_size > 0 {
                    *file_size as i64
                } else {
                    file.map(|f| f.total_size()).unwrap_or(0)
                };
                let badge = format!("📄 [Document: {} • {}]", file_name, format_bytes(size_bytes));
                let body_str = if caption.is_empty() {
                    badge
                } else {
                    format!("{} {}", badge, caption)
                };

                let mut spans = first_line_spans;
                spans.push(Span::styled(body_str, Style::default().fg(Color::LightMagenta)));
                rendered_lines.push(Line::from(spans));
                if let Some(status_spans) = file_status_spans(file) {
                    let mut s = vec![cont_indent.clone()];
                    s.extend(status_spans);
                    rendered_lines.push(Line::from(s));
                }
            }
            MessageContent::Audio {
                file_id,
                title,
                performer,
                duration,
                file_size,
            } => {
                let file = state.get_file(*file_id);
                let size_bytes = if *file_size > 0 {
                    *file_size
                } else {
                    file.map(|f| f.total_size()).unwrap_or(0)
                };
                let track_str = match (performer, title) {
                    (Some(p), Some(t)) => format!("{} - {}", p, t),
                    (None, Some(t)) => t.clone(),
                    (Some(p), None) => p.clone(),
                    (None, None) => "Unknown Track".to_string(),
                };
                let badge = format!(
                    "🎵 [Audio: {} • {} • {}] [Enter/o: Play  Ctrl+S: Save]",
                    track_str,
                    format_duration(*duration),
                    format_bytes(size_bytes)
                );

                let mut spans = first_line_spans;
                spans.push(Span::styled(badge, Style::default().fg(Color::LightCyan)));
                rendered_lines.push(Line::from(spans));
                if let Some(status_spans) = file_status_spans(file) {
                    let mut s = vec![cont_indent.clone()];
                    s.extend(status_spans);
                    rendered_lines.push(Line::from(s));
                }
            }
            MessageContent::Voice {
                caption,
                duration,
                file_id,
            } => {
                let file = state.get_file(*file_id);
                let size_str = file
                    .map(|f| format!(" • {}", format_bytes(f.total_size())))
                    .unwrap_or_default();
                let badge = format!("🎤 [Voice Note: {}{} • [Enter/o: Play  Ctrl+S: Save]]", format_duration(*duration), size_str);
                let body_str = if caption.is_empty() {
                    badge
                } else {
                    format!("{} {}", badge, caption)
                };

                let mut spans = first_line_spans;
                spans.push(Span::styled(body_str, Style::default().fg(Color::Yellow)));
                rendered_lines.push(Line::from(spans));
                if let Some(status_spans) = file_status_spans(file) {
                    let mut s = vec![cont_indent.clone()];
                    s.extend(status_spans);
                    rendered_lines.push(Line::from(s));
                }
            }
            MessageContent::Sticker { emoji, .. } => {
                let text = if emoji.is_empty() {
                    "🎨 [Sticker: 512x512]".to_string()
                } else {
                    format!("🎨 [Sticker: {}]", emoji)
                };

                let mut spans = first_line_spans;
                spans.push(Span::styled(text, Style::default().fg(Color::LightYellow)));
                rendered_lines.push(Line::from(spans));
            }
            MessageContent::Unsupported { raw_type } => {
                let text = format!("[Unsupported: {}]", raw_type);
                let mut spans = first_line_spans;
                spans.push(Span::styled(text, Style::default().fg(Color::DarkGray)));
                rendered_lines.push(Line::from(spans));
            }
        }
    }

    let total_lines = rendered_lines.len().min(u16::MAX as usize) as u16;
    let visible_height = area.height.saturating_sub(2);
    let max_scroll = total_lines.saturating_sub(visible_height);
    // Anchor to bottom by default (newest messages). Scrolling up reduces scroll_y towards 0 (oldest).
    let scroll_y = max_scroll.saturating_sub(scroll_offset as u16);

    let paragraph = Paragraph::new(rendered_lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    frame.render_widget(paragraph, area);
}

pub fn format_bytes(bytes: i64) -> String {
    if bytes <= 0 {
        return "0 B".to_string();
    }
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{} B", bytes)
    } else if b < MB {
        format!("{:.1} KB", b / KB)
    } else if b < GB {
        format!("{:.1} MB", b / MB)
    } else {
        format!("{:.1} GB", b / GB)
    }
}

pub fn format_duration(seconds: i32) -> String {
    let s = seconds.max(0);
    let m = s / 60;
    let sec = s % 60;
    format!("{}:{:02}", m, sec)
}

pub fn render_progress_bar(percent: f64, width: usize) -> String {
    let w = width.max(4);
    let filled = ((percent / 100.0) * w as f64).round() as usize;
    let filled = filled.min(w);
    let (lead, arrow, trail) = if filled >= w {
        ("=".repeat(w), "", "".to_string())
    } else if filled > 0 {
        ("=".repeat(filled - 1), ">", " ".repeat(w - filled))
    } else {
        ("".to_string(), "", " ".repeat(w))
    };
    format!("[{}{}{}]", lead, arrow, trail)
}

fn file_status_spans(file: Option<&FileInfo>) -> Option<Vec<Span<'static>>> {
    let file = file?;
    match &file.status {
        FileTransferStatus::Downloading { downloaded_size } => {
            let pct = file.progress_percent();
            let bar = render_progress_bar(pct, 10);
            let text = format!(
                "{} {:.0}% ({} / {})",
                bar,
                pct,
                format_bytes(*downloaded_size),
                format_bytes(file.total_size())
            );
            Some(vec![
                Span::raw("    "),
                Span::styled(text, Style::default().fg(Color::Yellow)),
            ])
        }
        FileTransferStatus::Completed => {
            let path_str = file
                .local_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "кэш".to_string());
            Some(vec![
                Span::raw("    "),
                Span::styled(
                    format!("[Готово к просмотру ({} • Ctrl+S чтобы сохранить)]", path_str),
                    Style::default().fg(Color::Green),
                ),
            ])
        }
        FileTransferStatus::Uploading { uploaded_size } => {
            Some(vec![
                Span::raw("    "),
                Span::styled(
                    format!(
                        "[Отправка: {} / {}]",
                        format_bytes(*uploaded_size),
                        format_bytes(file.total_size())
                    ),
                    Style::default().fg(Color::Cyan),
                ),
            ])
        }
        FileTransferStatus::Failed(err) => {
            Some(vec![
                Span::raw("    "),
                Span::styled(
                    format!("[Ошибка загрузки: {}]", err),
                    Style::default().fg(Color::Red),
                ),
            ])
        }
        FileTransferStatus::Idle => {
            Some(vec![
                Span::raw("    "),
                Span::styled("[Enter/o: Просмотр  Ctrl+S: Сохранить на диск]", Style::default().fg(Color::DarkGray)),
            ])
        }
    }
}

