use serde::{Deserialize, Serialize};
use super::message::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChatId(pub i64);

impl std::fmt::Display for ChatId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatType {
    Private,
    BasicGroup,
    Supergroup,
    Channel,
    Secret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChatPosition {
    pub order: u64,
    pub is_pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatFolder {
    pub id: i32,
    pub title: String,
    pub icon_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chat {
    pub id: ChatId,
    pub title: String,
    pub chat_type: ChatType,
    pub unread_count: i32,
    pub last_read_inbox_message_id: i64,
    pub last_read_outbox_message_id: i64,
    pub last_message: Option<Message>,
    pub position: Option<ChatPosition>,
    pub is_muted: bool,
    pub has_unread_mention: bool,
    #[serde(default)]
    pub folder_ids: Vec<i32>,
}
