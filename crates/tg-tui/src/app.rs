use std::collections::HashSet;
use ratatui::widgets::ListState;
use tg_core::{AuthState, ChatId, CoreEvent, CoreHandle, Message, MessageContent, MessageId};

#[path = "audio.rs"]
pub mod audio;
use audio::AudioPlayer;
use crate::ui::chat_list::{filter_chats, ChatFilterTab};
use crate::ui::image::open_file_in_os;
use crate::ui::modals::image_viewer::ImageViewerState;
use crate::ui::modals::{ActiveModal, AuthModalState, ProxyModalState, SettingsModalState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    ChatList,
    Messages,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurrentScreen {
    #[default]
    ChatSearch,
    ChatView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigationMode {
    #[default]
    Vim,
    Classic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VimMode {
    #[default]
    Normal,
    Insert,
    Search,
    Command,
}

pub struct App {
    pub handle: CoreHandle,
    pub should_quit: bool,
    pub current_screen: CurrentScreen,
    pub active_panel: ActivePanel,
    pub active_tab: ChatFilterTab,
    pub active_modal: ActiveModal,
    pub auth_modal_state: AuthModalState,
    pub proxy_modal_state: ProxyModalState,
    pub settings_modal_state: SettingsModalState,
    pub image_viewer_state: Option<ImageViewerState>,
    pub audio_player: AudioPlayer,

    // Navigation and Vim mode system
    pub navigation_mode: NavigationMode,
    pub vim_mode: VimMode,
    pub search_query: String,
    pub chat_list_state: ListState,
    pub selected_chat_index: usize,

    // Conversation pane state
    pub selected_message_index: usize,
    pub message_scroll: usize,

    // In-chat action states
    pub reply_target: Option<(MessageId, String)>,
    pub edit_target: Option<(MessageId, String)>,
    pub cmd_buffer: String,

    // Text Input for typing messages
    pub input_text: String,
    pub cursor_pos: usize,

    // Status bar & feedback
    pub status_message: String,
    pub is_status_error: bool,

    // Media & History states
    pub is_fetching_older_history: bool,
    pub reached_history_start: HashSet<ChatId>,
    pub pending_preview_file_id: Option<i32>,
    pub pending_save_file_id: Option<i32>,
    pub config: tg_core::Config,
}

impl App {
    pub fn new(handle: CoreHandle) -> Self {
        Self::with_config(handle, tg_core::Config::default())
    }

    pub fn with_config(handle: CoreHandle, config: tg_core::Config) -> Self {
        let mut chat_list_state = ListState::default();
        chat_list_state.select(Some(0));

        Self {
            handle,
            should_quit: false,
            current_screen: CurrentScreen::ChatSearch,
            active_panel: ActivePanel::ChatList,
            active_tab: ChatFilterTab::All,
            active_modal: ActiveModal::None,
            auth_modal_state: AuthModalState::default(),
            proxy_modal_state: ProxyModalState::default(),
            settings_modal_state: SettingsModalState::default(),
            image_viewer_state: None,
            audio_player: AudioPlayer::new(),
            navigation_mode: NavigationMode::Vim,
            vim_mode: VimMode::Normal,
            search_query: String::new(),
            chat_list_state,
            selected_chat_index: 0,
            selected_message_index: 0,
            message_scroll: 0,
            reply_target: None,
            edit_target: None,
            cmd_buffer: String::new(),
            input_text: String::new(),
            cursor_pos: 0,
            status_message: "Начните вводить название чата... [Enter] Открыть  [Up/Down] Выбор  [Esc] Выход".to_string(),
            is_status_error: false,
            is_fetching_older_history: false,
            reached_history_start: HashSet::new(),
            pending_preview_file_id: None,
            pending_save_file_id: None,
            config,
        }
    }

    /// Transition to full-screen view for the currently selected chat.
    pub fn open_selected_chat(&mut self) {
        self.current_screen = CurrentScreen::ChatView;
        self.active_panel = ActivePanel::Messages;
        self.vim_mode = VimMode::Normal;
        self.sync_message_selection_to_newest();
        let title = self.active_chat_id()
            .and_then(|id| self.handle.read_state().get_chat(id).map(|c| c.title.clone()))
            .unwrap_or_else(|| "Чат".to_string());
        self.status_message = format!("Чат: {}. [i] Написать  [Esc] Поиск чатов  [j/k] Скролл  [:] Команды", title);
    }

    /// Return to full-screen chat search / buffer switcher.
    pub fn exit_to_chat_search(&mut self) {
        self.current_screen = CurrentScreen::ChatSearch;
        self.active_panel = ActivePanel::ChatList;
        self.vim_mode = VimMode::Normal;
        self.search_query.clear();
        self.selected_chat_index = 0;
        self.chat_list_state.select(Some(0));
        self.status_message = "Поиск чатов: начните вводить название... [Enter] Открыть  [Up/Down/Ctrl+j/k] Выбор  [Esc] Назад".to_string();
    }

    /// Toggle between Vim Modal navigation and Classic Desktop navigation.
    pub fn toggle_navigation_mode(&mut self) {
        self.navigation_mode = match self.navigation_mode {
            NavigationMode::Vim => {
                self.vim_mode = VimMode::Normal;
                self.status_message = "Классический режим: Стрелки, прямой ввод сообщения, Tab переключение".to_string();
                NavigationMode::Classic
            }
            NavigationMode::Classic => {
                self.vim_mode = VimMode::Normal;
                self.status_message = "Vim режим: j/k навигация, / поиск, i ввод, r ответ, e правка, dd удаление".to_string();
                NavigationMode::Vim
            }
        };
    }

    /// Return the list of ChatIds matching active tab and optional search query
    pub fn current_chat_list(&self) -> Vec<ChatId> {
        let state = self.handle.read_state();
        let base = filter_chats(&state, &self.active_tab);
        if self.search_query.trim().is_empty() {
            return base;
        }
        let q = self.search_query.trim().to_lowercase();
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

    /// Return the currently selected ChatId based on search filter or active tab.
    pub fn active_chat_id(&self) -> Option<ChatId> {
        let list = self.current_chat_list();
        list.get(self.selected_chat_index).copied()
    }

    /// Move to the next chat in the filtered list.
    pub fn next_chat(&mut self) {
        let count = self.current_chat_list().len();
        if count == 0 {
            return;
        }
        self.selected_chat_index = (self.selected_chat_index + 1) % count;
        self.chat_list_state.select(Some(self.selected_chat_index));
        self.message_scroll = 0;
        self.sync_message_selection_to_newest();
    }

    /// Move to the previous chat in the filtered list.
    pub fn previous_chat(&mut self) {
        let count = self.current_chat_list().len();
        if count == 0 {
            return;
        }
        if self.selected_chat_index == 0 {
            self.selected_chat_index = count - 1;
        } else {
            self.selected_chat_index -= 1;
        }
        self.chat_list_state.select(Some(self.selected_chat_index));
        self.message_scroll = 0;
        self.sync_message_selection_to_newest();
    }

    /// Switch active chat filter tab and clamp selection bounds.
    pub fn switch_tab(&mut self, tab: ChatFilterTab) {
        self.active_tab = tab;
        self.search_query.clear();
        let count = self.current_chat_list().len();
        if count == 0 {
            self.selected_chat_index = 0;
            self.chat_list_state.select(None);
        } else if self.selected_chat_index >= count {
            self.selected_chat_index = count - 1;
            self.chat_list_state.select(Some(self.selected_chat_index));
        } else {
            self.chat_list_state.select(Some(self.selected_chat_index));
        }
        self.message_scroll = 0;
        self.sync_message_selection_to_newest();
    }

    /// Cycle to the next available chat folder / tab.
    pub fn next_tab(&mut self) {
        let next = {
            let state = self.handle.read_state();
            self.active_tab.next_with_state(&state)
        };
        self.switch_tab(next);
    }

    /// Cycle to the previous available chat folder / tab.
    pub fn prev_tab(&mut self) {
        let prev = {
            let state = self.handle.read_state();
            self.active_tab.prev_with_state(&state)
        };
        self.switch_tab(prev);
    }

    /// Sync selected message index to the newest message in active chat.
    pub fn sync_message_selection_to_newest(&mut self) {
        if let Some(chat_id) = self.active_chat_id() {
            let state = self.handle.read_state();
            let count = state.messages_for_chat(chat_id).len();
            if count > 0 {
                self.selected_message_index = count - 1;
            } else {
                self.selected_message_index = 0;
            }
        }
    }

    /// Move message cursor towards newer messages (down).
    pub fn next_message(&mut self) {
        let count = if let Some(chat_id) = self.active_chat_id() {
            let state = self.handle.read_state();
            state.messages_for_chat(chat_id).len()
        } else {
            0
        };
        if count > 0 && self.selected_message_index + 1 < count {
            self.selected_message_index += 1;
            self.scroll_messages_down(1);
        }
    }

    /// Move message cursor towards older messages (up).
    pub fn previous_message(&mut self) {
        if self.selected_message_index > 0 {
            self.selected_message_index -= 1;
            self.scroll_messages_up(1);
        }
    }

    /// Jump to bottom / newest message.
    pub fn jump_to_newest_message(&mut self) {
        self.sync_message_selection_to_newest();
        self.message_scroll = 0;
    }

    /// Jump to oldest message and fetch older history.
    pub async fn jump_to_oldest_and_fetch(&mut self) {
        self.selected_message_index = 0;
        self.fetch_older_history().await;
    }

    /// Get current selected message in active chat.
    pub fn get_selected_message(&self) -> Option<Message> {
        if let Some(chat_id) = self.active_chat_id() {
            let state = self.handle.read_state();
            let msgs = state.messages_for_chat(chat_id);
            msgs.get(self.selected_message_index).cloned()
        } else {
            None
        }
    }

    /// Automatically load full recent message history for the active chat if not yet loaded.
    pub async fn ensure_active_chat_history(&self) {
        if let Some(chat_id) = self.active_chat_id() {
            let state = self.handle.read_state();
            let count = state.messages_for_chat(chat_id).len();
            if count <= 1 {
                let _ = self.handle.load_history(chat_id, None, 50).await;
            }
        }
    }

    /// Fetch older messages when scrolling upwards.
    pub async fn fetch_older_history(&mut self) {
        if self.is_fetching_older_history {
            return;
        }
        if let Some(chat_id) = self.active_chat_id() {
            if self.reached_history_start.contains(&chat_id) {
                return;
            }
            let state = self.handle.read_state();
            let msgs = state.messages_for_chat(chat_id);
            if let Some(oldest) = msgs.first() {
                self.is_fetching_older_history = true;
                let _ = self.handle.load_history(chat_id, Some(oldest.id), 50).await;
                self.status_message = "Загрузка более старых сообщений...".to_string();
            }
        }
    }

    /// Proactively request download for media (no-op by default in CLI mode to save bandwidth).
    pub async fn auto_download_active_chat_media(&self) {
        // In CLI mode, downloads are on-demand when requested by user.
    }

    /// On-demand download and preview for the currently selected media.
    pub async fn trigger_selected_media_action(&mut self) {
        if let Some(msg) = self.get_selected_message() {
            match msg.content {
                MessageContent::Photo { caption, file_id, width, height } => {
                    let state = self.handle.read_state();
                    if let Some(file) = state.get_file(file_id) {
                        if let Some(ref path) = file.local_path {
                            if path.exists() {
                                self.image_viewer_state = Some(ImageViewerState::new(
                                    path.clone(),
                                    caption,
                                    width,
                                    height,
                                    file.total_size() as u64,
                                ));
                                self.active_modal = ActiveModal::ImageViewer;
                                return;
                            }
                        }
                    }
                    self.pending_preview_file_id = Some(file_id);
                    let _ = self.handle.download_file(file_id, 32).await;
                    self.status_message = "Загрузка фото для просмотра...".to_string();
                }
                MessageContent::Video { file_id, .. } | MessageContent::VideoNote { file_id, .. } => {
                    self.open_selected_video(file_id).await;
                }
                MessageContent::Voice { .. } | MessageContent::Audio { .. } => {
                    self.play_selected_audio();
                }
                MessageContent::Document { file_id, file_name, .. } => {
                    let state = self.handle.read_state();
                    if let Some(file) = state.get_file(file_id) {
                        if let Some(ref path) = file.local_path {
                            if path.exists() {
                                if let Err(e) = open_file_in_os(path) {
                                    self.status_message = format!("Ошибка открытия документа: {}", e);
                                    self.is_status_error = true;
                                } else {
                                    self.status_message = format!("▶ Открытие документа '{}'...", file_name);
                                    self.is_status_error = false;
                                }
                                return;
                            }
                        }
                    }
                    self.pending_preview_file_id = Some(file_id);
                    let _ = self.handle.download_file(file_id, 32).await;
                    self.status_message = format!("Загрузка документа '{}'...", file_name);
                }
                _ => {}
            }
        }
    }

    /// Open or launch video playback in OS default media player.
    pub async fn open_selected_video(&mut self, file_id: i32) {
        let state = self.handle.read_state();
        if let Some(file) = state.get_file(file_id) {
            if let Some(ref path) = file.local_path {
                if path.exists() {
                    if let Err(e) = open_file_in_os(path) {
                        self.status_message = format!("Ошибка открытия видео: {}", e);
                        self.is_status_error = true;
                    } else {
                        self.status_message = "▶ Запуск видео в системном плеере...".to_string();
                        self.is_status_error = false;
                    }
                    return;
                }
            }
        }
        self.pending_preview_file_id = Some(file_id);
        let _ = self.handle.download_file(file_id, 32).await;
        self.status_message = "Загрузка видео... Файл откроется в системном плеере после загрузки.".to_string();
    }

    /// Open full-screen Image Viewer for the currently selected photo message.
    pub fn open_selected_media(&mut self) {
        if let Some(msg) = self.get_selected_message() {
            match msg.content {
                MessageContent::Photo { caption, file_id, width, height } => {
                    let state = self.handle.read_state();
                    if let Some(file) = state.get_file(file_id) {
                        if let Some(ref path) = file.local_path {
                            if path.exists() {
                                self.image_viewer_state = Some(ImageViewerState::new(
                                    path.clone(),
                                    caption,
                                    width,
                                    height,
                                    file.total_size() as u64,
                                ));
                                self.active_modal = ActiveModal::ImageViewer;
                                return;
                            }
                        }
                    }
                    self.status_message = "Фото загружается...".to_string();
                }
                MessageContent::Video { file_id, .. } | MessageContent::VideoNote { file_id, .. } => {
                    let state = self.handle.read_state();
                    if let Some(file) = state.get_file(file_id) {
                        if let Some(ref path) = file.local_path {
                            if path.exists() {
                                let _ = open_file_in_os(path);
                                self.status_message = "▶ Запуск видео в системном плеере...".to_string();
                                return;
                            }
                        }
                    }
                }
                MessageContent::Voice { .. } | MessageContent::Audio { .. } => {
                    self.play_selected_audio();
                }
                _ => {}
            }
        }
    }

    /// Start audio playback for the currently selected voice note or audio track.
    pub fn play_selected_audio(&mut self) {
        if let Some(msg) = self.get_selected_message() {
            let (file_id, title, duration) = match msg.content {
                MessageContent::Voice { file_id, caption, duration } => {
                    let title = if caption.is_empty() { "Voice Note".to_string() } else { caption };
                    (file_id, title, duration)
                }
                MessageContent::Audio { file_id, title, performer, duration, .. } => {
                    let track = match (performer, title) {
                        (Some(p), Some(t)) => format!("{} - {}", p, t),
                        (None, Some(t)) => t,
                        (Some(p), None) => p,
                        (None, None) => "Audio Track".to_string(),
                    };
                    (file_id, track, duration)
                }
                _ => {
                    self.status_message = "Selected message has no audio.".to_string();
                    return;
                }
            };

            let state = self.handle.read_state();
            if let Some(file) = state.get_file(file_id) {
                if let Some(ref path) = file.local_path {
                    if path.exists() {
                        if let Err(e) = self.audio_player.play(path, title, duration) {
                            self.status_message = format!("Audio play error: {}", e);
                            self.is_status_error = true;
                        } else {
                            self.status_message = "▶ Playing audio... (Press [s] to stop)".to_string();
                        }
                        return;
                    }
                }
            }
            self.status_message = "Voice note is downloading... Try again in a moment.".to_string();
        }
    }

    /// Stop currently playing audio.
    pub fn stop_audio(&mut self) {
        self.audio_player.stop();
        self.status_message = "⏹ Audio stopped.".to_string();
    }

    /// Initiate reply to selected message.
    pub fn start_reply(&mut self) {
        if let Some(msg) = self.get_selected_message() {
            let snippet = match &msg.content {
                MessageContent::Text { text, .. } => {
                    let s: String = text.chars().take(30).collect();
                    if text.chars().count() > 30 {
                        format!("{}...", s)
                    } else {
                        s
                    }
                }
                _ => "[Media]".to_string(),
            };
            self.reply_target = Some((msg.id, snippet));
            self.edit_target = None;
            self.vim_mode = VimMode::Insert;
            self.active_panel = ActivePanel::Input;
            self.status_message = "Replying to message. Press Enter to send, Esc to cancel.".to_string();
        }
    }

    /// Initiate edit of selected message (if outgoing).
    pub fn start_edit(&mut self) {
        if let Some(msg) = self.get_selected_message() {
            if !msg.is_outgoing {
                self.status_message = "Cannot edit someone else's message.".to_string();
                self.is_status_error = true;
                return;
            }
            if let MessageContent::Text { ref text, .. } = msg.content {
                self.input_text = text.clone();
                self.cursor_pos = self.input_text.chars().count();
                self.edit_target = Some((msg.id, text.clone()));
                self.reply_target = None;
                self.vim_mode = VimMode::Insert;
                self.active_panel = ActivePanel::Input;
                self.status_message = "Editing message. Press Enter to save, Esc to cancel.".to_string();
                self.is_status_error = false;
            } else {
                self.status_message = "Can only edit text messages.".to_string();
                self.is_status_error = true;
            }
        }
    }

    /// Delete selected message.
    pub async fn delete_selected_message(&mut self) {
        if let Some(msg) = self.get_selected_message() {
            if let Some(chat_id) = self.active_chat_id() {
                if let Err(e) = self.handle.delete_messages(chat_id, vec![msg.id], true).await {
                    self.status_message = format!("Delete error: {}", e);
                    self.is_status_error = true;
                } else {
                    self.status_message = format!("Message #{} deleted.", msg.id.0);
                    self.is_status_error = false;
                }
            }
        }
    }

    /// Download media attached to selected message or most recent media in chat.
    pub async fn download_selected_message_media(&mut self) {
        if let Some(chat_id) = self.active_chat_id() {
            let state = self.handle.clone_state();
            let msgs = state.messages_for_chat(chat_id);
            if msgs.is_empty() {
                self.status_message = "No messages in chat to download".to_string();
                self.is_status_error = false;
                return;
            }

            let target_msg = self.get_selected_message()
                .filter(|m| m.content.file_id().is_some())
                .or_else(|| msgs.iter().rev().find(|m| m.content.file_id().is_some()).cloned());

            if let Some(msg) = target_msg {
                if let Some(file_id) = msg.content.file_id() {
                    if let Err(e) = self.handle.download_file(file_id, 1).await {
                        self.status_message = format!("Failed to download file #{}: {}", file_id, e);
                        self.is_status_error = true;
                    } else {
                        self.status_message = format!("Started download for file #{}", file_id);
                        self.is_status_error = false;
                    }
                }
            } else {
                self.status_message = "No downloadable media in this chat".to_string();
                self.is_status_error = false;
            }
        } else {
            self.status_message = "No message selected.".to_string();
        }
    }

    // --- Search Mode Operations ---

    pub fn enter_search_mode(&mut self) {
        self.vim_mode = VimMode::Search;
        self.active_panel = ActivePanel::ChatList;
        self.search_query.clear();
        self.selected_chat_index = 0;
        self.chat_list_state.select(Some(0));
        self.status_message = "SEARCH: Type chat name. [Down/Up] Navigate, [Enter] Open, [Esc] Cancel.".to_string();
        self.is_status_error = false;
    }

    pub fn handle_search_char(&mut self, c: char) {
        self.search_query.push(c);
        self.selected_chat_index = 0;
        self.chat_list_state.select(Some(0));
    }

    pub fn handle_search_backspace(&mut self) {
        if self.search_query.pop().is_none() {
            self.cancel_search();
        } else {
            self.selected_chat_index = 0;
            self.chat_list_state.select(Some(0));
        }
    }

    pub fn confirm_search(&mut self) {
        self.vim_mode = VimMode::Normal;
        self.active_panel = ActivePanel::Messages;
        self.sync_message_selection_to_newest();
        let title = self.active_chat_id()
            .and_then(|id| self.handle.read_state().get_chat(id).map(|c| c.title.clone()))
            .unwrap_or_else(|| "chat".to_string());
        self.status_message = format!("Opened: {}. [i] Write  [r] Reply  [e] Edit  [j/k] Messages", title);
        self.search_query.clear();
    }

    pub fn cancel_search(&mut self) {
        self.vim_mode = VimMode::Normal;
        self.search_query.clear();
        self.status_message = "Search cancelled.".to_string();
    }

    /// Cycle to next active panel.
    pub fn cycle_panel_forward(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::ChatList => ActivePanel::Messages,
            ActivePanel::Messages => ActivePanel::Input,
            ActivePanel::Input => ActivePanel::ChatList,
        };
    }

    /// Cycle to previous active panel.
    pub fn cycle_panel_backward(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::ChatList => ActivePanel::Input,
            ActivePanel::Messages => ActivePanel::ChatList,
            ActivePanel::Input => ActivePanel::Messages,
        };
    }

    /// Scroll messages upward (towards older messages).
    pub fn scroll_messages_up(&mut self, delta: usize) {
        self.message_scroll = self.message_scroll.saturating_add(delta).min(u16::MAX as usize);
    }

    /// Scroll messages downward (towards newer messages).
    pub fn scroll_messages_down(&mut self, delta: usize) {
        self.message_scroll = self.message_scroll.saturating_sub(delta);
    }

    // --- Text Input Cursor Operations ---

    pub fn insert_char(&mut self, c: char) {
        let byte_offset = self
            .input_text
            .char_indices()
            .nth(self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.input_text.len());
        self.input_text.insert(byte_offset, c);
        self.cursor_pos += 1;
    }

    pub fn delete_char_before_cursor(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            let byte_offset = self
                .input_text
                .char_indices()
                .nth(self.cursor_pos)
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input_text.remove(byte_offset);
        }
    }

    pub fn delete_char_at_cursor(&mut self) {
        let char_count = self.input_text.chars().count();
        if self.cursor_pos < char_count {
            let byte_offset = self
                .input_text
                .char_indices()
                .nth(self.cursor_pos)
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input_text.remove(byte_offset);
        }
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_sub(1);
    }

    pub fn move_cursor_right(&mut self) {
        let max_pos = self.input_text.chars().count();
        if self.cursor_pos < max_pos {
            self.cursor_pos += 1;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.input_text.chars().count();
    }

    pub fn delete_word_backward(&mut self) {
        while self.cursor_pos > 0 {
            self.delete_char_before_cursor();
            if self.cursor_pos == 0 {
                break;
            }
            let prev_char = self
                .input_text
                .chars()
                .nth(self.cursor_pos - 1)
                .unwrap_or(' ');
            if prev_char == ' ' {
                break;
            }
        }
    }

    pub fn clear_input(&mut self) {
        self.input_text.clear();
        self.cursor_pos = 0;
    }

    // --- Submission Logic ---

    pub async fn handle_submit(&mut self) {
        let state = self.handle.clone_state();

        match &state.auth_state {
            AuthState::WaitPhoneNumber => {
                let phone = self.auth_modal_state.input_buffer.trim().to_string();
                if !phone.is_empty() {
                    let _ = self.handle.send_phone_number(&phone).await;
                    self.status_message = format!("Sent phone {}. Waiting for code...", phone);
                    self.is_status_error = false;
                    self.auth_modal_state.input_buffer.clear();
                }
            }
            AuthState::WaitCode(_) => {
                let code = self.auth_modal_state.input_buffer.trim().to_string();
                if !code.is_empty() {
                    let _ = self.handle.send_code(&code).await;
                    self.status_message = "Sent code. Verifying...".to_string();
                    self.is_status_error = false;
                    self.auth_modal_state.input_buffer.clear();
                }
            }
            AuthState::WaitPassword(_) => {
                let pwd = self.auth_modal_state.input_buffer.clone();
                if !pwd.is_empty() {
                    let _ = self.handle.send_password(&pwd).await;
                    self.status_message = "Sent 2FA password. Verifying...".to_string();
                    self.is_status_error = false;
                    self.auth_modal_state.input_buffer.clear();
                }
            }
            AuthState::Ready => {
                let text = self.input_text.trim().to_string();
                if text.is_empty() {
                    self.vim_mode = VimMode::Normal;
                    self.active_panel = ActivePanel::Messages;
                    self.reply_target = None;
                    self.edit_target = None;
                    return;
                }

                if let Some(chat_id) = self.active_chat_id() {
                    if let Some((msg_id, _)) = self.edit_target.take() {
                        if let Err(e) = self.handle.edit_message_text(chat_id, msg_id, &text).await {
                            self.status_message = format!("Edit error: {}", e);
                            self.is_status_error = true;
                        } else {
                            self.status_message = "Message edited successfully".to_string();
                            self.is_status_error = false;
                        }
                    } else if let Some((reply_id, _)) = self.reply_target.take() {
                        if let Err(e) = self.handle.send_text_message(chat_id, &text, Some(reply_id)).await {
                            self.status_message = format!("Reply error: {}", e);
                            self.is_status_error = true;
                        } else {
                            self.status_message = "Reply sent".to_string();
                            self.is_status_error = false;
                            self.message_scroll = 0;
                        }
                    } else {
                        if let Err(e) = self.handle.send_text_message(chat_id, &text, None).await {
                            self.status_message = format!("Send error: {}", e);
                            self.is_status_error = true;
                        } else {
                            self.status_message = "Message sent".to_string();
                            self.is_status_error = false;
                            self.message_scroll = 0;
                        }
                    }
                }

                self.clear_input();
                self.vim_mode = VimMode::Normal;
                self.active_panel = ActivePanel::Messages;
                self.sync_message_selection_to_newest();
            }
            _ => {}
        }
    }

    /// Reactive event handler for CoreEvent updates from the core engine.
    pub fn handle_core_event(&mut self, event: CoreEvent) {
        match event {
            CoreEvent::AuthStateChanged(auth) => {
                tracing::info!("App: AuthState changed to {:?}", auth);
                match &auth {
                    AuthState::Ready => {
                        self.status_message = "Telegram connection ready. Vim Normal Mode active.".to_string();
                        self.is_status_error = false;
                        self.active_modal = ActiveModal::None;
                        self.auth_modal_state.input_buffer.clear();
                    }
                    AuthState::WaitQrCode(_) => {
                        self.status_message = "QR Code login active. Scan with mobile app.".to_string();
                        self.is_status_error = false;
                    }
                    AuthState::WaitPhoneNumber => {
                        self.status_message = "Enter phone number to log in.".to_string();
                    }
                    AuthState::WaitCode(_) => {
                        self.status_message = "Enter confirmation code.".to_string();
                    }
                    AuthState::WaitPassword(_) => {
                        self.status_message = "Enter 2FA cloud password.".to_string();
                    }
                    AuthState::LoggingOut => {
                        self.status_message = "Logging out from Telegram session...".to_string();
                        self.is_status_error = false;
                        self.active_modal = ActiveModal::None;
                    }
                    AuthState::Closed => {
                        self.status_message = "Logged out successfully. Ready for new login.".to_string();
                        self.is_status_error = false;
                        self.active_modal = ActiveModal::None;
                        self.selected_chat_index = 0;
                        self.selected_message_index = 0;
                        self.search_query.clear();
                        self.input_text.clear();
                    }
                    _ => {}
                }
            }
            CoreEvent::ConnectionStateChanged(conn) => {
                tracing::info!("App: ConnectionState changed to {:?}", conn);
            }
            CoreEvent::ErrorOccurred(err) => {
                tracing::error!("App: Core error: {}", err);
                self.status_message = err;
                self.is_status_error = true;
            }
            CoreEvent::MessageReceived(msg) | CoreEvent::MessageUpdated(msg) => {
                if Some(msg.chat_id) == self.active_chat_id() {
                    self.message_scroll = 0;
                    self.sync_message_selection_to_newest();
                }
            }
            CoreEvent::ChatListUpdated(_) => {
                let count = self.current_chat_list().len();
                if count > 0 && self.selected_chat_index >= count {
                    self.selected_chat_index = count - 1;
                    self.chat_list_state.select(Some(self.selected_chat_index));
                }
            }
            CoreEvent::HistoryLoaded { chat_id, count } => {
                self.is_fetching_older_history = false;
                if count <= 1 {
                    self.reached_history_start.insert(chat_id);
                    self.status_message = "Достигнуто начало истории чата.".to_string();
                } else if Some(chat_id) == self.active_chat_id() {
                    let added = count.saturating_sub(1);
                    self.selected_message_index = self.selected_message_index.saturating_add(added);
                    self.message_scroll = self.message_scroll.saturating_add(added);
                    self.status_message = format!("Загружено сообщений: +{}", added);
                }
            }
            CoreEvent::FileUpdated(file) => {
                if file.is_completed() {
                    let path_name = file
                        .local_path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| format!("file_{}", file.id));
                    self.status_message = format!("Загружено (Downloaded): {}", path_name);
                    self.is_status_error = false;

                    if self.pending_preview_file_id == Some(file.id) {
                        self.pending_preview_file_id = None;
                        if let Some(msg) = self.get_selected_message() {
                            if msg.content.file_id() == Some(file.id) {
                                match msg.content {
                                    MessageContent::Photo { .. } => {
                                        self.open_selected_media();
                                    }
                                    MessageContent::Video { .. } | MessageContent::VideoNote { .. } => {
                                        if let Some(ref path) = file.local_path {
                                            let _ = open_file_in_os(path);
                                            self.status_message = "▶ Запуск видео в системном плеере...".to_string();
                                        }
                                    }
                                    MessageContent::Document { .. } => {
                                        if let Some(ref path) = file.local_path {
                                            let _ = open_file_in_os(path);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    if self.pending_save_file_id == Some(file.id) {
                        self.pending_save_file_id = None;
                        if let Some(ref path) = file.local_path {
                            let downloads_dir = self.config.downloads_path();
                            let _ = std::fs::create_dir_all(&downloads_dir);
                            let file_name = path.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| format!("file_{}.bin", file.id));
                            let dest = downloads_dir.join(&file_name);
                            if let Ok(_) = std::fs::copy(path, &dest) {
                                self.status_message = format!("✔ Файл сохранён: {}", dest.display());
                                self.is_status_error = false;
                            }
                        }
                    }
                } else if file.is_downloading() {
                    self.status_message = format!(
                        "Загрузка файла #{}: {:.0}%",
                        file.id,
                        file.progress_percent()
                    );
                    self.is_status_error = false;
                }
            }
            _ => {}
        }
    }

    /// Permanently save media from selected message into persistent downloads directory (Ctrl+S / D).
    pub async fn save_selected_media_permanently(&mut self) {
        if let Some(msg) = self.get_selected_message() {
            if let Some(file_id) = msg.content.file_id() {
                let state = self.handle.read_state();
                if let Some(file) = state.get_file(file_id) {
                    if let Some(ref path) = file.local_path {
                        if path.exists() {
                            let downloads_dir = self.config.downloads_path();
                            let _ = std::fs::create_dir_all(&downloads_dir);
                            let file_name = match &msg.content {
                                MessageContent::Document { file_name, .. } => file_name.clone(),
                                MessageContent::Video { file_name, .. } => file_name.clone(),
                                _ => path.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| format!("file_{}.bin", file_id)),
                            };
                            let dest = downloads_dir.join(&file_name);
                            match std::fs::copy(path, &dest) {
                                Ok(_) => {
                                    self.status_message = format!("✔ Файл сохранён: {}", dest.display());
                                    self.is_status_error = false;
                                }
                                Err(e) => {
                                    self.status_message = format!("Ошибка сохранения: {}", e);
                                    self.is_status_error = true;
                                }
                            }
                            return;
                        }
                    }
                }

                // File not yet downloaded, request download with priority and mark for saving
                self.pending_save_file_id = Some(file_id);
                if let Err(e) = self.handle.download_file(file_id, 32).await {
                    self.status_message = format!("Ошибка скачивания #{}: {}", file_id, e);
                    self.is_status_error = true;
                } else {
                    self.status_message = "Скачивание для постоянного сохранения на диск...".to_string();
                    self.is_status_error = false;
                }
            } else {
                self.status_message = "В выбранном сообщении нет медиафайлов.".to_string();
            }
        }
    }

    /// Clean temporary media cache folder on exit
    pub fn cleanup_cache(&self) {
        let _ = self.config.clean_media_cache();
    }

    /// Trigger download of the media file in the currently focused chat/message.
    pub async fn download_focused_media(&mut self) -> tg_core::Result<()> {
        self.download_selected_message_media().await;
        Ok(())
    }
}
