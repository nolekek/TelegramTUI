//! TDLib Client and Simulation Actor for TgTui
//! Handles MTProto state, authentication, message sending, and update streams.

use crate::model::*;
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum TdlibCommand {
    SubmitPhone { phone: String },
    SubmitCode { code: String },
    SubmitPassword2FA { password: String },
    SendMessage { chat_id: ChatId, text: String, reply_to: Option<ReplyPreview> },
    SendMedia { chat_id: ChatId, filename: String, is_photo: bool, compress: bool, caption: String },
    ForwardMessage { from_chat: ChatId, to_chat: ChatId, message_id: MessageId, hide_author: bool },
    ToggleReaction { chat_id: ChatId, message_id: MessageId, emoji: String },
    EditMessage { chat_id: ChatId, message_id: MessageId, new_text: String },
    DeleteMessage { chat_id: ChatId, message_id: MessageId },
}

#[derive(Debug, Clone)]
pub enum TdlibEvent {
    AuthChanged(AuthStep),
    ChatsLoaded(Vec<ChatFolder>, Vec<Chat>),
    MessagesLoaded(ChatId, Vec<Message>),
    NewMessage(Message),
    MessageStatusUpdated { chat_id: ChatId, message_id: MessageId, status: DeliveryStatus },
    MessageEdited { chat_id: ChatId, message_id: MessageId, new_text: String },
    MessageDeleted { chat_id: ChatId, message_id: MessageId },
    ReactionUpdated { chat_id: ChatId, message_id: MessageId, reactions: Vec<Reaction> },
}

pub struct TdlibActor {
    cmd_rx: mpsc::Receiver<TdlibCommand>,
    event_tx: mpsc::Sender<TdlibEvent>,
    auth_state: AuthStep,
    chats: Vec<Chat>,
    folders: Vec<ChatFolder>,
    messages: HashMap<ChatId, Vec<Message>>,
    next_msg_id: i64,
}

impl TdlibActor {
    pub fn new(
        cmd_rx: mpsc::Receiver<TdlibCommand>,
        event_tx: mpsc::Sender<TdlibEvent>,
        skip_auth: bool,
    ) -> Self {
        let (initial_auth, folders, chats, messages) = Self::seed_data(skip_auth);

        Self {
            cmd_rx,
            event_tx,
            auth_state: initial_auth,
            chats,
            folders,
            messages,
            next_msg_id: 1000,
        }
    }

    pub async fn run(mut self) {
        // Send initial state
        let _ = self.event_tx.send(TdlibEvent::AuthChanged(self.auth_state.clone())).await;

        if let AuthStep::Authenticated = self.auth_state {
            self.broadcast_initial_data().await;
        }

        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                TdlibCommand::SubmitPhone { phone } => {
                    if phone.trim().len() >= 5 {
                        self.auth_state = AuthStep::CodeInput {
                            phone: phone.clone(),
                            buffer: String::new(),
                            error: None,
                        };
                    } else {
                        self.auth_state = AuthStep::PhoneInput {
                            buffer: phone,
                            error: Some("Phone number too short (minimum 5 digits)".into()),
                        };
                    }
                    let _ = self.event_tx.send(TdlibEvent::AuthChanged(self.auth_state.clone())).await;
                }
                TdlibCommand::SubmitCode { code } => {
                    if code.trim() == "12345" || code.trim().len() == 5 {
                        self.auth_state = AuthStep::Password2FA {
                            buffer: String::new(),
                            error: None,
                        };
                    } else {
                        if let AuthStep::CodeInput { phone, .. } = &self.auth_state {
                            self.auth_state = AuthStep::CodeInput {
                                phone: phone.clone(),
                                buffer: code,
                                error: Some("Invalid code. (Hint: use 12345)".into()),
                            };
                        }
                    }
                    let _ = self.event_tx.send(TdlibEvent::AuthChanged(self.auth_state.clone())).await;
                }
                TdlibCommand::SubmitPassword2FA { password } => {
                    if password.trim().is_empty() {
                        self.auth_state = AuthStep::Password2FA {
                            buffer: password,
                            error: Some("Password cannot be empty".into()),
                        };
                        let _ = self.event_tx.send(TdlibEvent::AuthChanged(self.auth_state.clone())).await;
                    } else {
                        self.auth_state = AuthStep::Authenticated;
                        let _ = self.event_tx.send(TdlibEvent::AuthChanged(self.auth_state.clone())).await;
                        self.broadcast_initial_data().await;
                    }
                }
                TdlibCommand::SendMessage { chat_id, text, reply_to } => {
                    self.next_msg_id += 1;
                    let msg_id = MessageId(self.next_msg_id);

                    let msg = Message {
                        id: msg_id,
                        chat_id,
                        sender_id: UserId(1),
                        sender_name: "You".into(),
                        date: "Just now".into(),
                        text: text.clone(),
                        entities: Self::detect_entities(&text),
                        reply_to,
                        media: None,
                        reactions: Vec::new(),
                        is_outgoing: true,
                        status: DeliveryStatus::Pending,
                        is_edited: false,
                    };

                    // Optimistic update
                    let _ = self.event_tx.send(TdlibEvent::NewMessage(msg.clone())).await;

                    // Update last snippet in chats list
                    if let Some(c) = self.chats.iter_mut().find(|c| c.id == chat_id) {
                        c.last_message_snippet = text;
                        c.last_message_date = "Just now".into();
                    }

                    // Simulate server ACK after 250ms -> Sent
                    let tx_ack = self.event_tx.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
                        let _ = tx_ack.send(TdlibEvent::MessageStatusUpdated {
                            chat_id,
                            message_id: msg_id,
                            status: DeliveryStatus::Sent,
                        }).await;

                        // Simulate recipient read after 700ms -> Read
                        tokio::time::sleep(tokio::time::Duration::from_millis(700)).await;
                        let _ = tx_ack.send(TdlibEvent::MessageStatusUpdated {
                            chat_id,
                            message_id: msg_id,
                            status: DeliveryStatus::Read,
                        }).await;
                    });
                }
                TdlibCommand::SendMedia { chat_id, filename, is_photo, compress, caption } => {
                    self.next_msg_id += 1;
                    let msg_id = MessageId(self.next_msg_id);

                    let media = if is_photo {
                        MessageMedia::Photo {
                            filename: filename.clone(),
                            width: 1920,
                            height: 1080,
                            is_compressed: compress,
                        }
                    } else {
                        MessageMedia::Document {
                            filename: filename.clone(),
                            size_bytes: 4_500_000,
                            upload_progress: None,
                        }
                    };

                    let msg = Message {
                        id: msg_id,
                        chat_id,
                        sender_id: UserId(1),
                        sender_name: "You".into(),
                        date: "Just now".into(),
                        text: caption.clone(),
                        entities: vec![],
                        reply_to: None,
                        media: Some(media),
                        reactions: Vec::new(),
                        is_outgoing: true,
                        status: DeliveryStatus::Sent,
                        is_edited: false,
                    };

                    let _ = self.event_tx.send(TdlibEvent::NewMessage(msg)).await;
                }
                TdlibCommand::ForwardMessage { from_chat, to_chat, message_id, hide_author } => {
                    self.next_msg_id += 1;
                    let new_id = MessageId(self.next_msg_id);

                    let orig_text = if let Some(msgs) = self.messages.get(&from_chat) {
                        msgs.iter().find(|m| m.id == message_id).map(|m| (m.sender_name.clone(), m.text.clone()))
                    } else {
                        None
                    };

                    if let Some((author, text)) = orig_text {
                        let forward_header = if hide_author {
                            String::new()
                        } else {
                            format!("[Forwarded from {}]\n", author)
                        };

                        let msg = Message {
                            id: new_id,
                            chat_id: to_chat,
                            sender_id: UserId(1),
                            sender_name: "You".into(),
                            date: "Just now".into(),
                            text: format!("{}{}", forward_header, text),
                            entities: vec![],
                            reply_to: None,
                            media: None,
                            reactions: Vec::new(),
                            is_outgoing: true,
                            status: DeliveryStatus::Sent,
                            is_edited: false,
                        };

                        let _ = self.event_tx.send(TdlibEvent::NewMessage(msg)).await;
                    }
                }
                TdlibCommand::ToggleReaction { chat_id, message_id, emoji } => {
                    let mut updated_reactions = Vec::new();

                    if let Some(msgs) = self.messages.get_mut(&chat_id) {
                        if let Some(msg) = msgs.iter_mut().find(|m| m.id == message_id) {
                            if let Some(r) = msg.reactions.iter_mut().find(|r| r.emoji == emoji) {
                                if r.chosen_by_me {
                                    r.count = r.count.saturating_sub(1);
                                    r.chosen_by_me = false;
                                } else {
                                    r.count += 1;
                                    r.chosen_by_me = true;
                                }
                            } else {
                                msg.reactions.push(Reaction {
                                    emoji: emoji.clone(),
                                    count: 1,
                                    chosen_by_me: true,
                                });
                            }
                            msg.reactions.retain(|r| r.count > 0);
                            updated_reactions = msg.reactions.clone();
                        }
                    }

                    let _ = self.event_tx.send(TdlibEvent::ReactionUpdated {
                        chat_id,
                        message_id,
                        reactions: updated_reactions,
                    }).await;
                }
                TdlibCommand::EditMessage { chat_id, message_id, new_text } => {
                    let _ = self.event_tx.send(TdlibEvent::MessageEdited {
                        chat_id,
                        message_id,
                        new_text,
                    }).await;
                }
                TdlibCommand::DeleteMessage { chat_id, message_id } => {
                    let _ = self.event_tx.send(TdlibEvent::MessageDeleted {
                        chat_id,
                        message_id,
                    }).await;
                }
            }
        }
    }

    async fn broadcast_initial_data(&self) {
        let _ = self.event_tx.send(TdlibEvent::ChatsLoaded(self.folders.clone(), self.chats.clone())).await;
        for (chat_id, msgs) in &self.messages {
            let _ = self.event_tx.send(TdlibEvent::MessagesLoaded(*chat_id, msgs.clone())).await;
        }
    }

    fn detect_entities(text: &str) -> Vec<MessageEntity> {
        let mut entities = Vec::new();
        if text.contains("**") {
            entities.push(MessageEntity {
                entity_type: EntityType::Bold,
                offset: 0,
                length: text.len(),
            });
        } else if text.contains('`') {
            entities.push(MessageEntity {
                entity_type: EntityType::Code,
                offset: 0,
                length: text.len(),
            });
        }
        entities
    }

    fn seed_data(skip_auth: bool) -> (AuthStep, Vec<ChatFolder>, Vec<Chat>, HashMap<ChatId, Vec<Message>>) {
        let auth = if skip_auth {
            AuthStep::Authenticated
        } else {
            AuthStep::PhoneInput { buffer: String::new(), error: None }
        };

        let folders = vec![
            ChatFolder { id: 0, title: "All".into(), icon: "💬".into() },
            ChatFolder { id: 1, title: "Personal".into(), icon: "👤".into() },
            ChatFolder { id: 2, title: "Groups".into(), icon: "👥".into() },
            ChatFolder { id: 3, title: "Channels".into(), icon: "📢".into() },
        ];

        let chats = vec![
            Chat {
                id: ChatId(101),
                title: "Pavel Durov".into(),
                chat_type: ChatType::Private,
                unread_count: 2,
                unread_mentions: 0,
                is_pinned: true,
                is_muted: false,
                folder_id: Some(1),
                last_message_snippet: "Welcome to the TgTui terminal client!".into(),
                last_message_date: "12:45".into(),
            },
            Chat {
                id: ChatId(102),
                title: "Rust Community Group".into(),
                chat_type: ChatType::Supergroup,
                unread_count: 14,
                unread_mentions: 1,
                is_pinned: true,
                is_muted: false,
                folder_id: Some(2),
                last_message_snippet: "Alice: Check out Ratatui v0.28 features".into(),
                last_message_date: "12:30".into(),
            },
            Chat {
                id: ChatId(103),
                title: "Telegram Desktop News".into(),
                chat_type: ChatType::Channel,
                unread_count: 0,
                unread_mentions: 0,
                is_pinned: false,
                is_muted: true,
                folder_id: Some(3),
                last_message_snippet: "Major update released with topic threads".into(),
                last_message_date: "Yesterday".into(),
            },
            Chat {
                id: ChatId(104),
                title: "Saved Messages".into(),
                chat_type: ChatType::Private,
                unread_count: 0,
                unread_mentions: 0,
                is_pinned: true,
                is_muted: true,
                folder_id: Some(1),
                last_message_snippet: "Notes: investigate MTProto crypto keys".into(),
                last_message_date: "Mon".into(),
            },
        ];

        let mut messages = HashMap::new();

        messages.insert(ChatId(101), vec![
            Message {
                id: MessageId(1),
                chat_id: ChatId(101),
                sender_id: UserId(2),
                sender_name: "Pavel Durov".into(),
                date: "12:40".into(),
                text: "Hello! Welcome to the TgTui clean-room terminal client.".into(),
                entities: vec![],
                reply_to: None,
                media: None,
                reactions: vec![
                    Reaction { emoji: "👋".into(), count: 4, chosen_by_me: true },
                ],
                is_outgoing: false,
                status: DeliveryStatus::Read,
                is_edited: false,
            },
            Message {
                id: MessageId(2),
                chat_id: ChatId(101),
                sender_id: UserId(2),
                sender_name: "Pavel Durov".into(),
                date: "12:42".into(),
                text: "Here is a voice briefing on MTProto updates:".into(),
                entities: vec![],
                reply_to: None,
                media: Some(MessageMedia::Voice {
                    duration_sec: 18,
                    waveform: " ▂▃▅▆▇▆▅▃▂ ".into(),
                }),
                reactions: vec![],
                is_outgoing: false,
                status: DeliveryStatus::Read,
                is_edited: false,
            },
            Message {
                id: MessageId(3),
                chat_id: ChatId(101),
                sender_id: UserId(1),
                sender_name: "You".into(),
                date: "12:43".into(),
                text: "Thanks! We implemented rich formatting with **bold**, `code`, and WCAG 2.2 AA tokens.".into(),
                entities: vec![
                    MessageEntity { entity_type: EntityType::Bold, offset: 40, length: 8 },
                    MessageEntity { entity_type: EntityType::Code, offset: 50, length: 6 },
                ],
                reply_to: Some(ReplyPreview {
                    message_id: MessageId(1),
                    sender_name: "Pavel Durov".into(),
                    snippet: "Hello! Welcome to the TgTui clean-room...".into(),
                }),
                media: None,
                reactions: vec![
                    Reaction { emoji: "🔥".into(), count: 3, chosen_by_me: true },
                    Reaction { emoji: "👍".into(), count: 5, chosen_by_me: false },
                ],
                is_outgoing: true,
                status: DeliveryStatus::Read,
                is_edited: false,
            },
            Message {
                id: MessageId(4),
                chat_id: ChatId(101),
                sender_id: UserId(2),
                sender_name: "Pavel Durov".into(),
                date: "12:45".into(),
                text: "Amazing work! Try sending a reply or testing the context menu.".into(),
                entities: vec![],
                reply_to: None,
                media: None,
                reactions: vec![
                    Reaction { emoji: "❤️".into(), count: 2, chosen_by_me: false },
                ],
                is_outgoing: false,
                status: DeliveryStatus::Read,
                is_edited: false,
            },
            Message {
                id: MessageId(5),
                chat_id: ChatId(101),
                sender_id: UserId(2),
                sender_name: "Pavel Durov".into(),
                date: "12:46".into(),
                text: "Here is the architectural diagram of the system:".into(),
                entities: vec![],
                reply_to: None,
                media: Some(MessageMedia::Photo {
                    filename: "architecture_diagram.png".into(),
                    width: 1920,
                    height: 1080,
                    is_compressed: true,
                }),
                reactions: vec![
                    Reaction { emoji: "👍".into(), count: 6, chosen_by_me: true },
                ],
                is_outgoing: false,
                status: DeliveryStatus::Read,
                is_edited: false,
            },
        ]);

        messages.insert(ChatId(102), vec![
            Message {
                id: MessageId(11),
                chat_id: ChatId(102),
                sender_id: UserId(3),
                sender_name: "Alice".into(),
                date: "12:20".into(),
                text: "Check out Ratatui v0.28 features in our repo:".into(),
                entities: vec![],
                reply_to: None,
                media: Some(MessageMedia::Document {
                    filename: "ratatui-guide.pdf".into(),
                    size_bytes: 2_450_000,
                    upload_progress: None,
                }),
                reactions: vec![],
                is_outgoing: false,
                status: DeliveryStatus::Read,
                is_edited: false,
            },
            Message {
                id: MessageId(12),
                chat_id: ChatId(102),
                sender_id: UserId(4),
                sender_name: "Bob".into(),
                date: "12:30".into(),
                text: "The actor model makes TUI event loops blazing fast without terminal flicker!".into(),
                entities: vec![],
                reply_to: None,
                media: None,
                reactions: vec![
                    Reaction { emoji: "🚀".into(), count: 7, chosen_by_me: true },
                ],
                is_outgoing: false,
                status: DeliveryStatus::Read,
                is_edited: false,
            },
        ]);

        messages.insert(ChatId(103), vec![
            Message {
                id: MessageId(21),
                chat_id: ChatId(103),
                sender_id: UserId(5),
                sender_name: "Telegram Desktop News".into(),
                date: "Yesterday".into(),
                text: "Major release brings collapsible folder sidebars, improved voice waveforms, and speed optimizations.".into(),
                entities: vec![],
                reply_to: None,
                media: None,
                reactions: vec![],
                is_outgoing: false,
                status: DeliveryStatus::Read,
                is_edited: false,
            },
        ]);

        messages.insert(ChatId(104), vec![
            Message {
                id: MessageId(31),
                chat_id: ChatId(104),
                sender_id: UserId(1),
                sender_name: "You".into(),
                date: "Mon".into(),
                text: "Notes: investigate MTProto crypto keys and TDLib FFI bindings.".into(),
                entities: vec![],
                reply_to: None,
                media: None,
                reactions: vec![],
                is_outgoing: true,
                status: DeliveryStatus::Read,
                is_edited: false,
            },
        ]);

        (auth, folders, chats, messages)
    }
}
