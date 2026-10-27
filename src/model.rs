//! Core Data Models and State Enums for TgTui
//! Matches schema.sql and architecture.md clean-room specs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChatId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStatus {
    Online,
    Offline,
    Recently,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub is_self: bool,
    pub is_bot: bool,
    pub is_premium: bool,
    pub status: UserStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatType {
    Private,
    Group,
    Supergroup,
    Channel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chat {
    pub id: ChatId,
    pub title: String,
    pub chat_type: ChatType,
    pub unread_count: u32,
    pub unread_mentions: u32,
    pub is_pinned: bool,
    pub is_muted: bool,
    pub folder_id: Option<u32>,
    pub last_message_snippet: String,
    pub last_message_date: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryStatus {
    Pending,
    Sent,
    Read,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityType {
    Bold,
    Italic,
    Code,
    Pre,
    Spoiler,
    Quote,
    Link(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageEntity {
    pub entity_type: EntityType,
    pub offset: usize,
    pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaction {
    pub emoji: String,
    pub count: u32,
    pub chosen_by_me: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageMedia {
    Photo {
        filename: String,
        width: u32,
        height: u32,
        is_compressed: bool,
    },
    Voice {
        duration_sec: u32,
        waveform: String,
    },
    Document {
        filename: String,
        size_bytes: u64,
        upload_progress: Option<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyPreview {
    pub message_id: MessageId,
    pub sender_name: String,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub chat_id: ChatId,
    pub sender_id: UserId,
    pub sender_name: String,
    pub date: String,
    pub text: String,
    pub entities: Vec<MessageEntity>,
    pub reply_to: Option<ReplyPreview>,
    pub media: Option<MessageMedia>,
    pub reactions: Vec<Reaction>,
    pub is_outgoing: bool,
    pub status: DeliveryStatus,
    pub is_edited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatFolder {
    pub id: u32,
    pub title: String,
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthStep {
    PhoneInput { buffer: String, error: Option<String> },
    CodeInput { phone: String, buffer: String, error: Option<String> },
    Password2FA { buffer: String, error: Option<String> },
    Authenticated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Folders,
    Dialogs,
    Messages,
    Composer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveModal {
    None,
    ContextMenu { message_idx: usize },
    ChatProfile,
    Search { query: String },
    SettingsHelp,
    ReactionPicker { message_idx: usize },
    ForwardModal { message_idx: usize, selected_chat_idx: usize, hide_author: bool },
    Lightbox { title: String, dimensions: String, zoom_percent: u32 },
    AttachFile { path_buffer: String, is_photo: bool, compress: bool },
}
