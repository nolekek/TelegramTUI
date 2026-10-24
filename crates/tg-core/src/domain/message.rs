use serde::{Deserialize, Serialize};
use super::chat::ChatId;
use super::user::UserId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MessageId(pub i64);

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sender {
    User(UserId),
    Chat(ChatId),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    Sending,
    Sent,
    Read,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEntity {
    pub offset: i32,
    pub length: i32,
    pub entity_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageContent {
    Text {
        text: String,
        entities: Vec<TextEntity>,
    },
    Photo {
        caption: String,
        file_id: i32,
        width: i32,
        height: i32,
    },
    Document {
        caption: String,
        file_name: String,
        file_id: i32,
        file_size: u64,
    },
    Voice {
        caption: String,
        duration: i32,
        file_id: i32,
    },
    Sticker {
        emoji: String,
        file_id: i32,
    },
    Audio {
        file_id: i32,
        title: Option<String>,
        performer: Option<String>,
        duration: i32,
        file_size: i64,
    },
    Unsupported {
        raw_type: String,
    },
}

impl MessageContent {
    pub fn file_id(&self) -> Option<i32> {
        match self {
            MessageContent::Photo { file_id, .. } => Some(*file_id),
            MessageContent::Document { file_id, .. } => Some(*file_id),
            MessageContent::Voice { file_id, .. } => Some(*file_id),
            MessageContent::Audio { file_id, .. } => Some(*file_id),
            MessageContent::Sticker { file_id, .. } => Some(*file_id),
            _ => None,
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub chat_id: ChatId,
    pub sender: Sender,
    pub date: i64,
    pub content: MessageContent,
    pub is_outgoing: bool,
    pub status: MessageStatus,
    pub reply_to_message_id: Option<MessageId>,
    pub can_be_edited: bool,
    pub can_be_deleted: bool,
}
