//! Central Application Coordinator for TgTui
//! Manages view-model state, active dialogs, input focus, modals, and routes commands.

use crate::actors::tdlib::{TdlibCommand, TdlibEvent};
use crate::model::*;
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum UserAction {
    CycleFocus,
    SetFocus(FocusPane),
    NextItem,
    PrevItem,
    SelectCurrent,
    InputChar(char),
    Backspace,
    ComposerSubmit,
    StartReply,
    StartEdit,
    DeleteSelected,
    ToggleVoicePlay,
    OpenContextMenu,
    OpenSearch,
    OpenProfile,
    OpenSettingsHelp,
    OpenReactionPicker,
    ToggleReaction(String),
    OpenForwardModal,
    ToggleForwardHideAuthor,
    SubmitForward,
    OpenAttachFile,
    ToggleAttachCompress,
    SubmitAttachFile,
    OpenLightbox,
    CloseModal,
    SelectFolder(usize),
    ToggleAuthDemo,
    Quit,
}

pub struct AppCoordinator {
    pub auth_state: AuthStep,
    pub folders: Vec<ChatFolder>,
    pub chats: Vec<Chat>,
    pub messages: HashMap<ChatId, Vec<Message>>,
    pub drafts: HashMap<ChatId, String>,
    pub active_folder_idx: usize,
    pub selected_chat_idx: usize,
    pub selected_msg_idx: Option<usize>,
    pub focus: FocusPane,
    pub active_modal: ActiveModal,
    pub composer_buffer: String,
    pub composer_reply_to: Option<ReplyPreview>,
    pub composer_edit_id: Option<MessageId>,
    pub status_notification: Option<(String, std::time::Instant)>,
    tdlib_cmd_tx: mpsc::Sender<TdlibCommand>,
}

impl AppCoordinator {
    pub fn new(tdlib_cmd_tx: mpsc::Sender<TdlibCommand>) -> Self {
        let is_authenticated = std::path::Path::new("session.json").exists();
        let initial_auth = if is_authenticated {
            AuthStep::Authenticated
        } else {
            AuthStep::PhoneInput { buffer: String::new(), error: None }
        };

        Self {
            auth_state: initial_auth,
            folders: vec![],
            chats: vec![],
            messages: HashMap::new(),
            drafts: HashMap::new(),
            active_folder_idx: 0,
            selected_chat_idx: 0,
            selected_msg_idx: None,
            focus: FocusPane::Dialogs,
            active_modal: ActiveModal::None,
            composer_buffer: String::new(),
            composer_reply_to: None,
            composer_edit_id: None,
            status_notification: None,
            tdlib_cmd_tx,
        }
    }

    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status_notification = Some((text.into(), std::time::Instant::now()));
    }

    pub fn current_chat(&self) -> Option<&Chat> {
        let filtered = self.filtered_chats();
        filtered.get(self.selected_chat_idx).copied()
    }

    pub fn filtered_chats(&self) -> Vec<&Chat> {
        let folder_id = self.folders.get(self.active_folder_idx).map(|f| f.id).unwrap_or(0);
        if folder_id == 0 {
            self.chats.iter().collect()
        } else {
            self.chats.iter().filter(|c| c.folder_id == Some(folder_id)).collect()
        }
    }

    pub fn current_messages(&self) -> Vec<&Message> {
        if let Some(chat) = self.current_chat() {
            if let Some(list) = self.messages.get(&chat.id) {
                return list.iter().collect();
            }
        }
        vec![]
    }

    pub async fn handle_tdlib_event(&mut self, event: TdlibEvent) {
        match event {
            TdlibEvent::AuthChanged(auth) => {
                if let AuthStep::Authenticated = &auth {
                    let _ = std::fs::write("session.json", "{\"authenticated\": true}");
                }
                self.auth_state = auth;
            }
            TdlibEvent::ChatsLoaded(folders, chats) => {
                self.folders = folders;
                self.chats = chats;
            }
            TdlibEvent::MessagesLoaded(chat_id, msgs) => {
                self.messages.insert(chat_id, msgs);
            }
            TdlibEvent::NewMessage(msg) => {
                let chat_id = msg.chat_id;
                self.messages.entry(chat_id).or_default().push(msg);
            }
            TdlibEvent::MessageStatusUpdated { chat_id, message_id, status } => {
                if let Some(list) = self.messages.get_mut(&chat_id) {
                    if let Some(m) = list.iter_mut().find(|m| m.id == message_id) {
                        m.status = status;
                    }
                }
            }
            TdlibEvent::MessageEdited { chat_id, message_id, new_text } => {
                if let Some(list) = self.messages.get_mut(&chat_id) {
                    if let Some(m) = list.iter_mut().find(|m| m.id == message_id) {
                        m.text = new_text;
                        m.is_edited = true;
                    }
                }
            }
            TdlibEvent::MessageDeleted { chat_id, message_id } => {
                if let Some(list) = self.messages.get_mut(&chat_id) {
                    list.retain(|m| m.id != message_id);
                }
            }
            TdlibEvent::ReactionUpdated { chat_id, message_id, reactions } => {
                if let Some(list) = self.messages.get_mut(&chat_id) {
                    if let Some(m) = list.iter_mut().find(|m| m.id == message_id) {
                        m.reactions = reactions;
                    }
                }
            }
        }
    }

    pub async fn handle_user_action(&mut self, action: UserAction) -> bool {
        // Handle Modal intercepts first
        if self.active_modal != ActiveModal::None {
            match action {
                UserAction::CloseModal | UserAction::Quit => {
                    self.active_modal = ActiveModal::None;
                    return true;
                }
                UserAction::InputChar(c) => match &mut self.active_modal {
                    ActiveModal::Search { query } => {
                        query.push(c);
                        return true;
                    }
                    ActiveModal::AttachFile { path_buffer, .. } => {
                        path_buffer.push(c);
                        return true;
                    }
                    ActiveModal::ReactionPicker { message_idx } => {
                        let idx = *message_idx;
                        let emoji = match c {
                            '1' => Some("👍"),
                            '2' => Some("❤️"),
                            '3' => Some("🔥"),
                            '4' => Some("🎉"),
                            '5' => Some("👏"),
                            '6' => Some("🤔"),
                            _ => None,
                        };
                        if let Some(e) = emoji {
                            self.toggle_reaction_on_index(idx, e).await;
                            self.active_modal = ActiveModal::None;
                            return true;
                        }
                    }
                    _ => {}
                },
                UserAction::Backspace => match &mut self.active_modal {
                    ActiveModal::Search { query } => {
                        query.pop();
                        return true;
                    }
                    ActiveModal::AttachFile { path_buffer, .. } => {
                        path_buffer.pop();
                        return true;
                    }
                    _ => {}
                },
                UserAction::ToggleForwardHideAuthor => {
                    if let ActiveModal::ForwardModal { hide_author, .. } = &mut self.active_modal {
                        *hide_author = !*hide_author;
                        return true;
                    }
                }
                UserAction::ToggleAttachCompress => {
                    if let ActiveModal::AttachFile { compress, .. } = &mut self.active_modal {
                        *compress = !*compress;
                        return true;
                    }
                }
                UserAction::SubmitForward => {
                    self.submit_forward_modal().await;
                    self.active_modal = ActiveModal::None;
                    return true;
                }
                UserAction::SubmitAttachFile => {
                    self.submit_attach_modal().await;
                    self.active_modal = ActiveModal::None;
                    return true;
                }
                UserAction::NextItem => {
                    if let ActiveModal::ForwardModal { selected_chat_idx, .. } = &mut self.active_modal {
                        if *selected_chat_idx + 1 < self.chats.len() {
                            *selected_chat_idx += 1;
                        }
                        return true;
                    }
                }
                UserAction::PrevItem => {
                    if let ActiveModal::ForwardModal { selected_chat_idx, .. } = &mut self.active_modal {
                        if *selected_chat_idx > 0 {
                            *selected_chat_idx -= 1;
                        }
                        return true;
                    }
                }
                _ => {}
            }
        }

        // Handle Auth Screen inputs
        if self.auth_state != AuthStep::Authenticated {
            match action {
                UserAction::InputChar(c) => {
                    match &mut self.auth_state {
                        AuthStep::PhoneInput { buffer, .. } => buffer.push(c),
                        AuthStep::CodeInput { buffer, .. } => buffer.push(c),
                        AuthStep::Password2FA { buffer, .. } => buffer.push(c),
                        _ => {}
                    }
                }
                UserAction::Backspace => {
                    match &mut self.auth_state {
                        AuthStep::PhoneInput { buffer, .. } => { buffer.pop(); },
                        AuthStep::CodeInput { buffer, .. } => { buffer.pop(); },
                        AuthStep::Password2FA { buffer, .. } => { buffer.pop(); },
                        _ => {}
                    }
                }
                UserAction::ComposerSubmit | UserAction::SelectCurrent => {
                    match &self.auth_state {
                        AuthStep::PhoneInput { buffer, .. } => {
                            let _ = self.tdlib_cmd_tx.send(TdlibCommand::SubmitPhone { phone: buffer.clone() }).await;
                        }
                        AuthStep::CodeInput { buffer, .. } => {
                            let _ = self.tdlib_cmd_tx.send(TdlibCommand::SubmitCode { code: buffer.clone() }).await;
                        }
                        AuthStep::Password2FA { buffer, .. } => {
                            let _ = self.tdlib_cmd_tx.send(TdlibCommand::SubmitPassword2FA { password: buffer.clone() }).await;
                        }
                        _ => {}
                    }
                }
                UserAction::Quit => return false,
                _ => {}
            }
            return true;
        }

        // Handle Normal Navigation & Actions
        match action {
            UserAction::Quit => return false,
            UserAction::CycleFocus => {
                self.focus = match self.focus {
                    FocusPane::Folders => FocusPane::Dialogs,
                    FocusPane::Dialogs => FocusPane::Messages,
                    FocusPane::Messages => FocusPane::Composer,
                    FocusPane::Composer => FocusPane::Folders,
                };
            }
            UserAction::SetFocus(pane) => {
                self.focus = pane;
            }
            UserAction::SelectFolder(idx) => {
                if idx < self.folders.len() {
                    self.save_current_draft();
                    self.active_folder_idx = idx;
                    self.selected_chat_idx = 0;
                    self.load_current_draft();
                    self.set_status(format!("Switched to folder: {}", self.folders[idx].title));
                }
            }
            UserAction::NextItem => match self.focus {
                FocusPane::Folders => {
                    if self.active_folder_idx + 1 < self.folders.len() {
                        self.save_current_draft();
                        self.active_folder_idx += 1;
                        self.selected_chat_idx = 0;
                        self.load_current_draft();
                    }
                }
                FocusPane::Dialogs => {
                    let count = self.filtered_chats().len();
                    if count > 0 && self.selected_chat_idx + 1 < count {
                        self.save_current_draft();
                        self.selected_chat_idx += 1;
                        self.selected_msg_idx = None;
                        self.load_current_draft();
                    }
                }
                FocusPane::Messages => {
                    let count = self.current_messages().len();
                    if count > 0 {
                        let next = self.selected_msg_idx.map(|i| (i + 1).min(count - 1)).unwrap_or(0);
                        self.selected_msg_idx = Some(next);
                    }
                }
                FocusPane::Composer => {}
            },
            UserAction::PrevItem => match self.focus {
                FocusPane::Folders => {
                    if self.active_folder_idx > 0 {
                        self.save_current_draft();
                        self.active_folder_idx -= 1;
                        self.selected_chat_idx = 0;
                        self.load_current_draft();
                    }
                }
                FocusPane::Dialogs => {
                    if self.selected_chat_idx > 0 {
                        self.save_current_draft();
                        self.selected_chat_idx -= 1;
                        self.selected_msg_idx = None;
                        self.load_current_draft();
                    }
                }
                FocusPane::Messages => {
                    let count = self.current_messages().len();
                    if count > 0 {
                        let prev = self.selected_msg_idx.map(|i| i.saturating_sub(1)).unwrap_or(count.saturating_sub(1));
                        self.selected_msg_idx = Some(prev);
                    }
                }
                FocusPane::Composer => {}
            },
            UserAction::SelectCurrent => {
                if self.focus == FocusPane::Dialogs {
                    self.focus = FocusPane::Composer;
                } else if self.focus == FocusPane::Messages {
                    if let Some(idx) = self.selected_msg_idx {
                        if let Some(m) = self.current_messages().get(idx) {
                            if let Some(MessageMedia::Photo { filename, width, height, .. }) = &m.media {
                                self.active_modal = ActiveModal::Lightbox {
                                    title: filename.clone(),
                                    dimensions: format!("{}x{}", width, height),
                                    zoom_percent: 100,
                                };
                            }
                        }
                    }
                }
            }
            UserAction::InputChar(c) => {
                if self.focus == FocusPane::Composer {
                    self.composer_buffer.push(c);
                }
            }
            UserAction::Backspace => {
                if self.focus == FocusPane::Composer {
                    self.composer_buffer.pop();
                }
            }
            UserAction::ComposerSubmit => {
                if !self.composer_buffer.trim().is_empty() {
                    if let Some(chat) = self.current_chat() {
                        let chat_id = chat.id;
                        let text = self.composer_buffer.clone();
                        let reply_to = self.composer_reply_to.take();

                        if let Some(edit_id) = self.composer_edit_id.take() {
                            let _ = self.tdlib_cmd_tx.send(TdlibCommand::EditMessage {
                                chat_id,
                                message_id: edit_id,
                                new_text: text,
                            }).await;
                            self.set_status("Message updated");
                        } else {
                            let _ = self.tdlib_cmd_tx.send(TdlibCommand::SendMessage {
                                chat_id,
                                text,
                                reply_to,
                            }).await;
                        }

                        self.composer_buffer.clear();
                    }
                }
            }
            UserAction::StartReply => {
                let reply_data = self.selected_msg_idx.and_then(|idx| {
                    self.current_messages().get(idx).map(|m| (m.id, m.sender_name.clone(), m.text.chars().take(30).collect::<String>()))
                });
                if let Some((id, sender_name, snippet)) = reply_data {
                    self.composer_reply_to = Some(ReplyPreview {
                        message_id: id,
                        sender_name: sender_name.clone(),
                        snippet,
                    });
                    self.focus = FocusPane::Composer;
                    self.set_status(format!("Replying to {}", sender_name));
                }
            }
            UserAction::StartEdit => {
                let edit_data = self.selected_msg_idx.and_then(|idx| {
                    self.current_messages().get(idx).map(|m| (m.id, m.is_outgoing, m.text.clone()))
                });
                if let Some((id, is_outgoing, text)) = edit_data {
                    if is_outgoing {
                        self.composer_edit_id = Some(id);
                        self.composer_buffer = text;
                        self.focus = FocusPane::Composer;
                        self.set_status("Editing message");
                    } else {
                        self.set_status("Can only edit your own messages");
                    }
                }
            }
            UserAction::DeleteSelected => {
                let to_delete = self.selected_msg_idx.and_then(|idx| {
                    self.current_messages().get(idx).map(|m| m.id)
                });
                if let Some(msg_id) = to_delete {
                    if let Some(chat) = self.current_chat() {
                        let chat_id = chat.id;
                        let _ = self.tdlib_cmd_tx.send(TdlibCommand::DeleteMessage {
                            chat_id,
                            message_id: msg_id,
                        }).await;
                        self.set_status("Message deleted");
                        self.selected_msg_idx = None;
                    }
                }
            }
            UserAction::ToggleVoicePlay => {
                self.set_status("Voice note audio playback started via mpv / external audio");
            }
            UserAction::OpenContextMenu => {
                if let Some(idx) = self.selected_msg_idx {
                    self.active_modal = ActiveModal::ContextMenu { message_idx: idx };
                } else {
                    self.set_status("Select a message first using Up/Down in Messages pane");
                }
            }
            UserAction::OpenSearch => {
                self.active_modal = ActiveModal::Search { query: String::new() };
            }
            UserAction::OpenProfile => {
                self.active_modal = ActiveModal::ChatProfile;
            }
            UserAction::OpenSettingsHelp => {
                self.active_modal = ActiveModal::SettingsHelp;
            }
            UserAction::OpenReactionPicker => {
                if let Some(idx) = self.selected_msg_idx {
                    self.active_modal = ActiveModal::ReactionPicker { message_idx: idx };
                } else {
                    self.set_status("Select a message first to add a reaction (+)");
                }
            }
            UserAction::ToggleReaction(emoji) => {
                if let Some(idx) = self.selected_msg_idx {
                    self.toggle_reaction_on_index(idx, &emoji).await;
                }
            }
            UserAction::OpenForwardModal => {
                if let Some(idx) = self.selected_msg_idx {
                    self.active_modal = ActiveModal::ForwardModal {
                        message_idx: idx,
                        selected_chat_idx: 0,
                        hide_author: false,
                    };
                } else {
                    self.set_status("Select a message first to forward (f)");
                }
            }
            UserAction::ToggleForwardHideAuthor => {
                if let ActiveModal::ForwardModal { hide_author, .. } = &mut self.active_modal {
                    *hide_author = !*hide_author;
                }
            }
            UserAction::SubmitForward => {
                self.submit_forward_modal().await;
                self.active_modal = ActiveModal::None;
            }
            UserAction::OpenAttachFile => {
                self.active_modal = ActiveModal::AttachFile {
                    path_buffer: String::new(),
                    is_photo: true,
                    compress: true,
                };
            }
            UserAction::ToggleAttachCompress => {
                if let ActiveModal::AttachFile { compress, .. } = &mut self.active_modal {
                    *compress = !*compress;
                }
            }
            UserAction::SubmitAttachFile => {
                self.submit_attach_modal().await;
                self.active_modal = ActiveModal::None;
            }
            UserAction::OpenLightbox => {
                if let Some(idx) = self.selected_msg_idx {
                    if let Some(m) = self.current_messages().get(idx) {
                        if let Some(MessageMedia::Photo { filename, width, height, .. }) = &m.media {
                            self.active_modal = ActiveModal::Lightbox {
                                title: filename.clone(),
                                dimensions: format!("{}x{}", width, height),
                                zoom_percent: 100,
                            };
                        }
                    }
                }
            }
            UserAction::CloseModal => {
                self.active_modal = ActiveModal::None;
                self.composer_reply_to = None;
                self.composer_edit_id = None;
            }
            UserAction::ToggleAuthDemo => {
                if self.auth_state == AuthStep::Authenticated {
                    let _ = std::fs::remove_file("session.json");
                    self.auth_state = AuthStep::PhoneInput { buffer: String::new(), error: None };
                    self.set_status("Switched to Auth Gate (S01)");
                } else {
                    let _ = std::fs::write("session.json", "{\"authenticated\": true}");
                    self.auth_state = AuthStep::Authenticated;
                    self.set_status("Returned to Main Shell (S02)");
                }
            }
        }

        true
    }

    fn save_current_draft(&mut self) {
        if let Some(chat) = self.current_chat() {
            let chat_id = chat.id;
            if self.composer_buffer.is_empty() {
                self.drafts.remove(&chat_id);
            } else {
                self.drafts.insert(chat_id, self.composer_buffer.clone());
            }
        }
    }

    fn load_current_draft(&mut self) {
        if let Some(chat) = self.current_chat() {
            let chat_id = chat.id;
            self.composer_buffer = self.drafts.get(&chat_id).cloned().unwrap_or_default();
        }
    }

    async fn toggle_reaction_on_index(&mut self, idx: usize, emoji: &str) {
        if let Some(chat) = self.current_chat() {
            let chat_id = chat.id;
            if let Some(m) = self.current_messages().get(idx) {
                let message_id = m.id;
                let _ = self.tdlib_cmd_tx.send(TdlibCommand::ToggleReaction {
                    chat_id,
                    message_id,
                    emoji: emoji.to_string(),
                }).await;
                self.set_status(format!("Toggled reaction {}", emoji));
            }
        }
    }

    async fn submit_forward_modal(&mut self) {
        if let ActiveModal::ForwardModal { message_idx, selected_chat_idx, hide_author } = self.active_modal {
            if let Some(from_chat) = self.current_chat() {
                let from_chat_id = from_chat.id;
                if let Some(m) = self.current_messages().get(message_idx) {
                    let msg_id = m.id;
                    if let Some(to_chat) = self.chats.get(selected_chat_idx) {
                        let to_chat_id = to_chat.id;
                        let _ = self.tdlib_cmd_tx.send(TdlibCommand::ForwardMessage {
                            from_chat: from_chat_id,
                            to_chat: to_chat_id,
                            message_id: msg_id,
                            hide_author,
                        }).await;
                        self.set_status(format!("Forwarded message to {}", to_chat.title));
                    }
                }
            }
        }
    }

    async fn submit_attach_modal(&mut self) {
        if let ActiveModal::AttachFile { path_buffer, is_photo, compress } = &self.active_modal {
            if !path_buffer.trim().is_empty() {
                if let Some(chat) = self.current_chat() {
                    let chat_id = chat.id;
                    let filename = path_buffer.trim().to_string();
                    let caption = self.composer_buffer.clone();
                    self.composer_buffer.clear();

                    let _ = self.tdlib_cmd_tx.send(TdlibCommand::SendMedia {
                        chat_id,
                        filename: filename.clone(),
                        is_photo: *is_photo,
                        compress: *compress,
                        caption,
                    }).await;
                    self.set_status(format!("Sent attachment: {}", filename));
                }
            }
        }
    }
}
