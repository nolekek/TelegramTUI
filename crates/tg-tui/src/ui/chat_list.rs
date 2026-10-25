use chrono::DateTime;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::symbols;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs};
use ratatui::Frame;
use tg_core::{Chat, ChatId, ChatType, CoreState, MessageContent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatFilterTab {
    All,
    Personal,
    Groups,
    Channels,
    Folder(i32, String),
}

impl Default for ChatFilterTab {
    fn default() -> Self {
        Self::All
    }
}

impl ChatFilterTab {
    pub const ALL: [Self; 4] = [Self::All, Self::Personal, Self::Groups, Self::Channels];

    pub fn title(&self) -> &str {
        match self {
            Self::All => "All",
            Self::Personal => "Personal",
            Self::Groups => "Groups",
            Self::Channels => "Channels",
            Self::Folder(_, title) => title.as_str(),
        }
    }

    pub fn matches(&self, chat: &Chat) -> bool {
        match self {
            Self::All => true,
            Self::Personal => matches!(chat.chat_type, ChatType::Private | ChatType::Secret),
            Self::Groups => matches!(chat.chat_type, ChatType::BasicGroup | ChatType::Supergroup),
            Self::Channels => matches!(chat.chat_type, ChatType::Channel),
            Self::Folder(folder_id, _) => chat.folder_ids.contains(folder_id),
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::All => Self::Personal,
            Self::Personal => Self::Groups,
            Self::Groups => Self::Channels,
            Self::Channels => Self::All,
            Self::Folder(..) => Self::All,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::All => Self::Channels,
            Self::Personal => Self::All,
            Self::Groups => Self::Personal,
            Self::Channels => Self::Groups,
            Self::Folder(..) => Self::Channels,
        }
    }

    pub fn next_with_state(&self, state: &CoreState) -> Self {
        let tabs = available_tabs(state);
        if let Some(pos) = tabs.iter().position(|t| t == self) {
            let next_pos = (pos + 1) % tabs.len();
            tabs[next_pos].clone()
        } else {
            Self::All
        }
    }

    pub fn prev_with_state(&self, state: &CoreState) -> Self {
        let tabs = available_tabs(state);
        if let Some(pos) = tabs.iter().position(|t| t == self) {
            let prev_pos = if pos == 0 { tabs.len() - 1 } else { pos - 1 };
            tabs[prev_pos].clone()
        } else {
            Self::All
        }
    }
}

/// Returns all available tabs including base filters and custom Telegram folders.
pub fn available_tabs(state: &CoreState) -> Vec<ChatFilterTab> {
    let mut tabs = vec![
        ChatFilterTab::All,
        ChatFilterTab::Personal,
        ChatFilterTab::Groups,
        ChatFilterTab::Channels,
    ];
    for folder in &state.chat_folders {
        tabs.push(ChatFilterTab::Folder(folder.id, folder.title.clone()));
    }
    tabs
}

/// Compute live unread badge count for a tab.
pub fn tab_unread_count<T: std::borrow::Borrow<ChatFilterTab>>(state: &CoreState, tab: T) -> i32 {
    let tab = tab.borrow();
    state
        .chats
        .values()
        .filter(|c| tab.matches(c))
        .map(|c| c.unread_count)
        .sum()
}

/// Filter `ordered_chats` from `state` by the given tab.
pub fn filter_chats<T: std::borrow::Borrow<ChatFilterTab>>(state: &CoreState, tab: T) -> Vec<ChatId> {
    let tab = tab.borrow();
    state
        .ordered_chats
        .iter()
        .copied()
        .filter(|id| {
            if let Some(chat) = state.get_chat(*id) {
                tab.matches(chat)
            } else {
                false
            }
        })
        .collect()
}

/// Filter `ordered_chats` from `state` by tab and search query.
pub fn filter_chats_query<T: std::borrow::Borrow<ChatFilterTab>>(state: &CoreState, tab: T, query: &str) -> Vec<ChatId> {
    let base = filter_chats(state, tab.borrow());
    if query.trim().is_empty() {
        return base;
    }
    let q = query.trim().to_lowercase();
    base.into_iter()
        .filter(|id| {
            if let Some(c) = state.get_chat(*id) {
                c.title.to_lowercase().contains(&q)
            } else {
                false
            }
        })
        .collect()
}

/// Render the chat filtering tabs and chat list widget with search support.
pub fn render_chat_list(
    frame: &mut Frame,
    state: &CoreState,
    active_tab: ChatFilterTab,
    search_query: &str,
    is_searching: bool,
    selected_index: usize,
    list_state: &mut ListState,
    is_focused: bool,
    area: Rect,
) {
    let has_search = is_searching || !search_query.is_empty();
    let (tab_chunk, search_chunk, list_chunk) = if has_search {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tab bar
                Constraint::Length(3), // Search bar
                Constraint::Min(3),    // Chat list items
            ])
            .split(area);
        (chunks[0], Some(chunks[1]), chunks[2])
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tab bar
                Constraint::Min(3),    // Chat list items
            ])
            .split(area);
        (chunks[0], None, chunks[1])
    };

    // 1. Tab Bar
    let all_tabs = available_tabs(state);
    let selected_tab_idx = all_tabs.iter().position(|t| t == &active_tab).unwrap_or(0);

    let tab_titles: Vec<Line> = all_tabs
        .iter()
        .enumerate()
        .map(|(idx, tab)| {
            let unread = tab_unread_count(state, tab);
            let key = idx + 1;
            let title = if unread > 0 {
                format!("{}:{} ({})", key, tab.title(), unread)
            } else {
                format!("{}:{}", key, tab.title())
            };
            Line::from(title)
        })
        .collect();

    let tab_bar_title = if all_tabs.len() > 4 {
        format!(" Folders [ [ / ] or 1-{} ] ", all_tabs.len())
    } else {
        " Folders [ [ / ] or 1-4 ] ".to_string()
    };

    let tabs_widget = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(symbols::border::ROUNDED)
                .title(tab_bar_title),
        )
        .select(selected_tab_idx)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .divider("|");

    frame.render_widget(tabs_widget, tab_chunk);

    // 2. Search Bar (if searching or query present)
    if let Some(s_chunk) = search_chunk {
        let s_title = if is_searching {
            " Search [/] (Type to filter, Down/Up browse, Enter open, Esc cancel) "
        } else {
            " Filter Active (Esc to clear) "
        };
        let s_style = if is_searching {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow)
        };
        let s_text = format!("/ {}{}", search_query, if is_searching { "█" } else { "" });
        let search_widget = Paragraph::new(s_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(symbols::border::ROUNDED)
                .title(s_title)
                .border_style(s_style),
        );
        frame.render_widget(search_widget, s_chunk);
    }

    // 3. Chat List Items
    let filtered = filter_chats_query(state, &active_tab, search_query);

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(idx, id)| {
            let chat = state.get_chat(*id);
            let title = chat.map(|c| c.title.as_str()).unwrap_or("Unknown");
            let unread = chat.map(|c| c.unread_count).unwrap_or(0);
            let is_pinned = chat
                .and_then(|c| c.position.as_ref())
                .map(|p| p.is_pinned)
                .unwrap_or(false);
            let is_muted = chat.map(|c| c.is_muted).unwrap_or(false);
            let has_mention = chat.map(|c| c.has_unread_mention).unwrap_or(false);

            // Line 1: Title and indicators
            let mut line1_spans = Vec::new();
            if is_pinned {
                line1_spans.push(Span::styled("📌 ", Style::default().fg(Color::Yellow)));
            }

            let title_style = if unread > 0 {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            line1_spans.push(Span::styled(title.to_string(), title_style));

            if is_muted {
                line1_spans.push(Span::styled(" 🔕", Style::default().fg(Color::DarkGray)));
            }

            if has_mention {
                line1_spans.push(Span::styled(
                    " @",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            if unread > 0 {
                let badge_style = if is_muted {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                };
                line1_spans.push(Span::styled(format!(" [{}]", unread), badge_style));
            }

            // Line 2: Snippet and timestamp
            let mut line2_spans = Vec::new();
            if let Some(c) = chat {
                if let Some(ref last_msg) = c.last_message {
                    if last_msg.is_outgoing {
                        line2_spans.push(Span::styled("You: ", Style::default().fg(Color::Cyan)));
                    }
                    let snippet = match &last_msg.content {
                        MessageContent::Text { text, .. } => {
                            let clean = text.replace('\n', " ");
                            if clean.chars().count() > 24 {
                                format!("{}...", clean.chars().take(22).collect::<String>())
                            } else {
                                clean
                            }
                        }
                        MessageContent::Photo { .. } => "[Photo]".to_string(),
                        MessageContent::Video { .. } => "[Video]".to_string(),
                        MessageContent::VideoNote { .. } => "[Video Note]".to_string(),
                        MessageContent::Document { file_name, .. } => {
                            format!("[Doc: {}]", file_name)
                        }
                        MessageContent::Voice { .. } => "[Voice]".to_string(),
                        MessageContent::Audio { title, performer, .. } => {
                            let t = title.as_deref().unwrap_or("Audio");
                            if let Some(perf) = performer {
                                format!("[Audio: {} - {}]", perf, t)
                            } else {
                                format!("[Audio: {}]", t)
                            }
                        }
                        MessageContent::Sticker { emoji, .. } => format!("[Sticker {}]", emoji),
                        MessageContent::Unsupported { .. } => "[Unsupported]".to_string(),

                    };
                    line2_spans.push(Span::styled(snippet, Style::default().fg(Color::DarkGray)));

                    // Right-align or append timestamp
                    if let Some(utc_dt) = DateTime::from_timestamp(last_msg.date, 0) {
                        let time_str = utc_dt.format("%H:%M").to_string();
                        line2_spans.push(Span::styled(
                            format!("  {}", time_str),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                }
            }

            let mut lines = vec![Line::from(line1_spans)];
            if !line2_spans.is_empty() {
                lines.push(Line::from(line2_spans));
            }

            let _ = idx;
            ListItem::new(lines)
        })
        .collect();

    let border_style = if is_focused {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = format!(" Chats ({}) ", filtered.len());
    let list_widget = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(symbols::border::ROUNDED)
                .title(title)
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(30, 48, 80))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▎ ");

    if !filtered.is_empty() {
        list_state.select(Some(selected_index.min(filtered.len() - 1)));
    } else {
        list_state.select(None);
    }

    frame.render_stateful_widget(list_widget, list_chunk, list_state);
}

/// Render a full-screen dedicated Chat Search & Picker view (Telescope / fzf style).
pub fn render_chat_search(
    frame: &mut Frame,
    state: &CoreState,
    active_tab: ChatFilterTab,
    search_query: &str,
    selected_index: usize,
    list_state: &mut ListState,
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Folder Tabs
            Constraint::Length(3), // Search Input Box
            Constraint::Min(4),    // Matched Chats List
        ])
        .split(area);

    // 1. Tab Bar
    let all_tabs = available_tabs(state);
    let selected_tab_idx = all_tabs.iter().position(|t| t == &active_tab).unwrap_or(0);
    let tab_titles: Vec<Line> = all_tabs
        .iter()
        .enumerate()
        .map(|(idx, tab)| {
            let unread = tab_unread_count(state, tab);
            let key = idx + 1;
            let title = if unread > 0 {
                format!("{}:{} ({})", key, tab.title(), unread)
            } else {
                format!("{}:{}", key, tab.title())
            };
            Line::from(title)
        })
        .collect();

    let tab_bar_title = if all_tabs.len() > 4 {
        format!(" Папки [ [ / ] или 1-{} ] ", all_tabs.len())
    } else {
        " Папки [ [ / ] или 1-4 ] ".to_string()
    };

    let tabs_widget = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(symbols::border::ROUNDED)
                .title(tab_bar_title)
                .border_style(Style::default().fg(Color::Rgb(70, 80, 110))),
        )
        .select(selected_tab_idx)
        .style(Style::default().fg(Color::Rgb(108, 112, 134)))
        .highlight_style(
            Style::default()
                .fg(Color::Rgb(125, 207, 255))
                .add_modifier(Modifier::BOLD),
        )
        .divider(" │ ");

    frame.render_widget(tabs_widget, chunks[0]);

    // 2. Full-width Prominent Search Input Box
    let search_title = " 🔍 Поиск чатов [ Enter — Открыть │ Up/Down или Ctrl+j/k — Выбрать │ Esc — Назад ] ";
    let mut search_spans = vec![
        Span::styled(" ❯ ", Style::default().fg(Color::Rgb(88, 166, 255)).add_modifier(Modifier::BOLD)),
    ];
    if search_query.is_empty() {
        search_spans.push(Span::styled("введите название чата или пользователя...", Style::default().fg(Color::Rgb(108, 112, 134)).add_modifier(Modifier::ITALIC)));
    } else {
        search_spans.push(Span::styled(search_query, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
    }
    search_spans.push(Span::styled("█", Style::default().fg(Color::Rgb(187, 154, 247))));

    let search_box = Paragraph::new(Line::from(search_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .title(search_title)
            .border_style(Style::default().fg(Color::Rgb(88, 166, 255)).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(search_box, chunks[1]);

    // 3. Matched Chats List
    let filtered = filter_chats_query(state, &active_tab, search_query);
    if filtered.is_empty() {
        let empty_msg = vec![
            Line::default(),
            Line::from(vec![
                Span::styled(
                    if search_query.is_empty() {
                        "  Чаты не найдены. Ожидание синхронизации с Telegram..."
                    } else {
                        "  Чаты с таким названием не найдены. Попробуйте другой запрос."
                    },
                    Style::default().fg(Color::Rgb(108, 112, 134)),
                ),
            ]),
        ];
        let p = Paragraph::new(empty_msg).block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(symbols::border::ROUNDED)
                .title(" Результаты поиска (0) ")
                .border_style(Style::default().fg(Color::Rgb(70, 80, 110))),
        );
        frame.render_widget(p, chunks[2]);
        list_state.select(None);
        return;
    }

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(idx, id)| {
            let chat = state.get_chat(*id);
            let title = chat.map(|c| c.title.as_str()).unwrap_or("Неизвестный чат");
            let unread = chat.map(|c| c.unread_count).unwrap_or(0);
            let is_pinned = chat.and_then(|c| c.position.map(|p| p.is_pinned)).unwrap_or(false);
            let chat_type_badge = match chat.map(|c| &c.chat_type) {
                Some(ChatType::Private | ChatType::Secret) => (" [Личный] ", Color::Rgb(125, 207, 255)),
                Some(ChatType::BasicGroup | ChatType::Supergroup) => (" [Группа] ", Color::Rgb(115, 218, 154)),
                Some(ChatType::Channel) => (" [Канал] ", Color::Rgb(187, 154, 247)),
                None => (" [Чат] ", Color::Rgb(108, 112, 134)),
            };

            let is_cur = idx == selected_index;
            let mut line_spans = Vec::new();
            if is_cur {
                line_spans.push(Span::styled("▎ ", Style::default().fg(Color::Rgb(125, 207, 255)).add_modifier(Modifier::BOLD)));
            } else {
                line_spans.push(Span::raw("  "));
            }

            if is_pinned {
                line_spans.push(Span::styled("📌 ", Style::default().fg(Color::Rgb(250, 200, 100))));
            }

            line_spans.push(Span::styled(
                chat_type_badge.0,
                Style::default().fg(chat_type_badge.1).add_modifier(Modifier::DIM),
            ));

            let title_style = if is_cur {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(200, 205, 220))
            };
            line_spans.push(Span::styled(title, title_style));

            if unread > 0 {
                line_spans.push(Span::styled(
                    format!("  {} ", unread),
                    Style::default().bg(Color::Rgb(247, 118, 142)).fg(Color::Black).add_modifier(Modifier::BOLD),
                ));
            }

            // Show last message snippet
            if let Some(c) = chat {
                if let Some(ref last_msg) = c.last_message {
                    let preview = match &last_msg.content {
                        MessageContent::Text { text, .. } => {
                            let clean = text.replace('\n', " ");
                            if clean.chars().count() > 40 {
                                format!(" — {}...", clean.chars().take(40).collect::<String>())
                            } else {
                                format!(" — {}", clean)
                            }
                        }
                        MessageContent::Photo { .. } => " — 📷 [Фото]".to_string(),
                        MessageContent::Video { .. } => " — 🎬 [Видео]".to_string(),
                        MessageContent::VideoNote { .. } => " — 🎥 [Видеосообщение]".to_string(),
                        MessageContent::Voice { .. } => " — 🎤 [Голосовое]".to_string(),
                        MessageContent::Audio { .. } => " — 🎵 [Аудио]".to_string(),
                        MessageContent::Document { file_name, .. } => format!(" — 📄 [{}]", file_name),
                        MessageContent::Sticker { emoji, .. } => format!(" — 🎨 [{}]", emoji),
                        _ => " — [Медиа]".to_string(),
                    };
                    line_spans.push(Span::styled(preview, Style::default().fg(Color::Rgb(140, 145, 165))));

                    if let Some(utc_dt) = DateTime::from_timestamp(last_msg.date, 0) {
                        let time_str = utc_dt.format(" %H:%M").to_string();
                        line_spans.push(Span::styled(time_str, Style::default().fg(Color::Rgb(108, 112, 134))));
                    }
                }
            }

            ListItem::new(vec![Line::from(line_spans)])
        })
        .collect();

    let title = format!(" Найдено чатов: {} ", filtered.len());
    let list_widget = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(symbols::border::ROUNDED)
                .title(title)
                .border_style(Style::default().fg(Color::Rgb(80, 100, 130))),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(25, 45, 75))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

    list_state.select(Some(selected_index.min(filtered.len().saturating_sub(1))));
    frame.render_stateful_widget(list_widget, chunks[2], list_state);
}
