use std::collections::HashMap;
use crate::domain::{
    AuthState, Chat, ChatFolder, ChatId, ChatPosition, ChatType, FileInfo, Message, MessageContent, MessageId,
    MessageStatus, User, UserId,
};
use crate::event::{ConnectionState, CoreEvent};

#[derive(Debug, Clone)]
pub struct CoreState {
    pub auth_state: AuthState,
    pub connection_state: ConnectionState,
    pub me: Option<User>,
    pub chats: HashMap<ChatId, Chat>,
    pub ordered_chats: Vec<ChatId>,
    pub chat_folders: Vec<ChatFolder>,
    pub messages: HashMap<ChatId, Vec<Message>>,
    pub users: HashMap<UserId, User>,
    pub files: HashMap<i32, FileInfo>,
}

impl Default for CoreState {
    fn default() -> Self {
        Self {
            auth_state: AuthState::Closed,
            connection_state: ConnectionState::WaitingForNetwork,
            me: None,
            chats: HashMap::new(),
            ordered_chats: Vec::new(),
            chat_folders: Vec::new(),
            messages: HashMap::new(),
            users: HashMap::new(),
            files: HashMap::new(),
        }
    }
}


impl CoreState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_auth_state(&mut self, state: AuthState) {
        self.auth_state = state;
    }

    pub fn update_connection_state(&mut self, state: ConnectionState) {
        self.connection_state = state;
    }

    pub fn set_me(&mut self, user: User) {
        self.users.insert(user.id, user.clone());
        self.me = Some(user);
    }

    pub fn update_user(&mut self, user: User) {
        let is_me = self.me.as_ref().map(|m| m.id == user.id).unwrap_or(false);
        if is_me {
            self.me = Some(user.clone());
        }
        self.users.insert(user.id, user);
    }

    pub fn insert_or_update_chat(&mut self, mut chat: Chat) {
        if let Some(existing) = self.chats.get(&chat.id) {
            if chat.folder_ids.is_empty() && !existing.folder_ids.is_empty() {
                chat.folder_ids = existing.folder_ids.clone();
            }
        }
        self.chats.insert(chat.id, chat);
        self.rebuild_chat_order();
    }

    /// Recompute the deterministic ordering of chats for the main chat list.
    /// Pinned chats come first, followed by descending order key, followed by descending ChatId.
    /// Chats with order == 0 (and not pinned) are excluded from the main list.
    pub fn rebuild_chat_order(&mut self) {
        let mut list: Vec<(ChatId, u64, bool)> = self
            .chats
            .values()
            .filter_map(|c| match c.position {
                Some(pos) if pos.order > 0 || pos.is_pinned => {
                    Some((c.id, pos.order, pos.is_pinned))
                }
                Some(pos) if pos.order == 0 && !pos.is_pinned => None,
                None => Some((c.id, 0, false)),
                _ => None,
            })
            .collect();

        // Sort: pinned first, then order descending, then ChatId descending
        list.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| b.0.cmp(&a.0))
        });

        self.ordered_chats = list.into_iter().map(|(id, _, _)| id).collect();
    }

    pub fn update_chat_position(&mut self, chat_id: ChatId, position: Option<ChatPosition>) {
        if let Some(chat) = self.chats.get_mut(&chat_id) {
            chat.position = position;
            self.rebuild_chat_order();
        }
    }

    pub fn update_chat_read_inbox(
        &mut self,
        chat_id: ChatId,
        last_read_inbox_message_id: i64,
        unread_count: i32,
    ) {
        if let Some(chat) = self.chats.get_mut(&chat_id) {
            chat.last_read_inbox_message_id = last_read_inbox_message_id;
            chat.unread_count = unread_count;
        }
    }

    pub fn update_chat_read_outbox(&mut self, chat_id: ChatId, last_read_outbox_message_id: i64) {
        if let Some(chat) = self.chats.get_mut(&chat_id) {
            chat.last_read_outbox_message_id = last_read_outbox_message_id;
            if let Some(ref mut last_msg) = chat.last_message {
                if last_msg.is_outgoing && last_msg.id.0 <= last_read_outbox_message_id {
                    last_msg.status = MessageStatus::Read;
                }
            }
        }
        if let Some(msgs) = self.messages.get_mut(&chat_id) {
            for msg in msgs.iter_mut() {
                if msg.is_outgoing
                    && msg.id.0 <= last_read_outbox_message_id
                    && msg.status != MessageStatus::Read
                {
                    msg.status = MessageStatus::Read;
                }
            }
        }
    }

    pub fn update_message_content(
        &mut self,
        chat_id: ChatId,
        message_id: MessageId,
        content: MessageContent,
    ) {
        if let Some(msgs) = self.messages.get_mut(&chat_id) {
            if let Some(msg) = msgs.iter_mut().find(|m| m.id == message_id) {
                msg.content = content.clone();
            }
        }
        if let Some(chat) = self.chats.get_mut(&chat_id) {
            if let Some(ref mut last_msg) = chat.last_message {
                if last_msg.id == message_id {
                    last_msg.content = content;
                }
            }
        }
    }

    pub fn update_message_status(
        &mut self,
        chat_id: ChatId,
        message_id: MessageId,
        status: MessageStatus,
    ) {
        if let Some(msgs) = self.messages.get_mut(&chat_id) {
            if let Some(msg) = msgs.iter_mut().find(|m| m.id == message_id) {
                msg.status = status.clone();
            }
        }
        if let Some(chat) = self.chats.get_mut(&chat_id) {
            if let Some(ref mut last_msg) = chat.last_message {
                if last_msg.id == message_id {
                    last_msg.status = status;
                }
            }
        }
    }

    pub fn update_message_send_succeeded(
        &mut self,
        chat_id: ChatId,
        old_message_id: MessageId,
        mut message: Message,
    ) {
        message.status = MessageStatus::Sent;
        if let Some(msgs) = self.messages.get_mut(&chat_id) {
            if let Some(pos) = msgs.iter().position(|m| m.id == old_message_id) {
                msgs[pos] = message.clone();
            } else if !msgs.iter().any(|m| m.id == message.id) {
                msgs.push(message.clone());
                msgs.sort_by_key(|m| (m.date, m.id.0));
            }
        }
        if let Some(chat) = self.chats.get_mut(&chat_id) {
            if let Some(ref last_msg) = chat.last_message {
                if last_msg.id == old_message_id || last_msg.id == message.id {
                    chat.last_message = Some(message);
                }
            }
        }
    }

    pub fn delete_messages(&mut self, chat_id: ChatId, message_ids: &[MessageId]) {
        if let Some(msgs) = self.messages.get_mut(&chat_id) {
            msgs.retain(|m| !message_ids.contains(&m.id));
        }
        if let Some(chat) = self.chats.get_mut(&chat_id) {
            if let Some(ref last_msg) = chat.last_message {
                if message_ids.contains(&last_msg.id) {
                    chat.last_message = self
                        .messages
                        .get(&chat_id)
                        .and_then(|ms| ms.last())
                        .cloned();
                }
            }
        }
    }

    pub fn insert_history_messages(&mut self, chat_id: ChatId, history_messages: Vec<Message>) {
        let msgs = self.messages.entry(chat_id).or_default();
        for msg in history_messages {
            if let Some(pos) = msgs.iter().position(|m| m.id == msg.id) {
                msgs[pos] = msg;
            } else {
                msgs.push(msg);
            }
        }
        msgs.sort_by_key(|m| (m.date, m.id.0));
        if let Some(chat) = self.chats.get_mut(&chat_id) {
            if chat.last_message.is_none() {
                chat.last_message = msgs.last().cloned();
            }
        }
    }

    pub fn add_message(&mut self, message: Message) {
        let chat_id = message.chat_id;
        let msgs = self.messages.entry(chat_id).or_default();

        if let Some(pos) = msgs.iter().position(|m| m.id == message.id) {
            msgs[pos] = message.clone();
        } else {
            msgs.push(message.clone());
            msgs.sort_by_key(|m| (m.date, m.id.0));
        }

        if let Some(chat) = self.chats.get_mut(&chat_id) {
            let should_update = match &chat.last_message {
                Some(last) => message.date >= last.date || message.id == last.id,
                None => true,
            };
            if should_update {
                chat.last_message = Some(message);
            }
        }
    }

    pub fn ordered_chats(&self) -> &[ChatId] {
        &self.ordered_chats
    }

    pub fn get_chat(&self, id: ChatId) -> Option<&Chat> {
        self.chats.get(&id)
    }

    pub fn get_messages(&self, chat_id: ChatId) -> Option<&Vec<Message>> {
        self.messages.get(&chat_id)
    }

    pub fn messages_for_chat(&self, chat_id: ChatId) -> &[Message] {
        self.messages.get(&chat_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn get_user(&self, user_id: UserId) -> Option<&User> {
        self.users.get(&user_id)
    }

    pub fn unread_count_for_type(&self, chat_type: &ChatType) -> i32 {
        self.chats
            .values()
            .filter(|c| &c.chat_type == chat_type)
            .map(|c| c.unread_count)
            .sum()
    }

    pub fn total_unread_count(&self) -> i32 {
        self.chats.values().map(|c| c.unread_count).sum()
    }

    pub fn update_file(&mut self, file: FileInfo) {
        self.files.insert(file.id, file);
    }

    pub fn get_file(&self, file_id: i32) -> Option<&FileInfo> {
        self.files.get(&file_id)
    }

    pub fn apply_update(&mut self, event: &CoreEvent) {
        match event {
            CoreEvent::AuthStateChanged(auth) => self.update_auth_state(auth.clone()),
            CoreEvent::ConnectionStateChanged(conn) => self.update_connection_state(conn.clone()),
            CoreEvent::MeUpdated(user) => self.set_me(*user.clone()),
            CoreEvent::UserUpdated(user) => self.update_user(*user.clone()),
            CoreEvent::ChatUpdated(chat) => self.insert_or_update_chat(*chat.clone()),
            CoreEvent::ChatListUpdated(_) => self.rebuild_chat_order(),
            CoreEvent::ChatPositionChanged { chat_id, position } => {
                self.update_chat_position(*chat_id, *position)
            }
            CoreEvent::ChatReadInboxChanged {
                chat_id,
                last_read_inbox_message_id,
                unread_count,
            } => self.update_chat_read_inbox(
                *chat_id,
                *last_read_inbox_message_id,
                *unread_count,
            ),
            CoreEvent::ChatReadOutboxChanged {
                chat_id,
                last_read_outbox_message_id,
            } => self.update_chat_read_outbox(*chat_id, *last_read_outbox_message_id),
            CoreEvent::MessageReceived(msg) | CoreEvent::MessageUpdated(msg) => {
                self.add_message(*msg.clone())
            }
            CoreEvent::MessagesDeleted {
                chat_id,
                message_ids,
            } => self.delete_messages(*chat_id, message_ids),
            CoreEvent::MessageStatusChanged {
                chat_id,
                message_id,
                status,
            } => self.update_message_status(*chat_id, *message_id, status.clone()),
            CoreEvent::FileUpdated(file) => self.update_file(file.clone()),
            CoreEvent::ChatFoldersUpdated(folders) => {
                self.chat_folders = folders.clone();
            }
            CoreEvent::ChatFolderPositionsChanged { chat_id, folder_ids } => {
                if let Some(chat) = self.chats.get_mut(chat_id) {
                    chat.folder_ids = folder_ids.clone();
                }
            }
            _ => {}
        }
    }
}

