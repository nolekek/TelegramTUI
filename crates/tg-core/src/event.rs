use serde::{Deserialize, Serialize};
use crate::domain::{AuthState, Chat, ChatFolder, ChatId, ChatPosition, FileInfo, Message, MessageId, MessageStatus, User};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    WaitingForNetwork,
    ConnectingToProxy,
    Connecting,
    Updating,
    Ready,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoreEvent {
    AuthStateChanged(AuthState),
    ConnectionStateChanged(ConnectionState),
    ChatListUpdated(Vec<ChatId>),
    ChatUpdated(Box<Chat>),
    MessageReceived(Box<Message>),
    MessageUpdated(Box<Message>),
    UserUpdated(Box<User>),
    MeUpdated(Box<User>),
    LogMessage(String),
    ErrorOccurred(String),

    // File handling events (Milestone 3)
    FileUpdated(FileInfo),

    // Reactive state synchronization events (Milestone 1)
    ChatPositionChanged {
        chat_id: ChatId,
        position: Option<ChatPosition>,
    },
    ChatReadInboxChanged {
        chat_id: ChatId,
        last_read_inbox_message_id: i64,
        unread_count: i32,
    },
    ChatReadOutboxChanged {
        chat_id: ChatId,
        last_read_outbox_message_id: i64,
    },
    MessagesDeleted {
        chat_id: ChatId,
        message_ids: Vec<MessageId>,
    },
    MessageStatusChanged {
        chat_id: ChatId,
        message_id: MessageId,
        status: MessageStatus,
    },
    ChatFoldersUpdated(Vec<ChatFolder>),
    ChatFolderPositionsChanged {
        chat_id: ChatId,
        folder_ids: Vec<i32>,
    },
    HistoryLoaded {
        chat_id: ChatId,
        count: usize,
    },
}

