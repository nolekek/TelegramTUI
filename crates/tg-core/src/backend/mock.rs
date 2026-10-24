use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::backend::{BoxFuture, TelegramBackend};
use crate::domain::*;
use crate::event::{ConnectionState, CoreEvent};

pub struct MockBackend {
    event_tx: Arc<Mutex<Option<broadcast::Sender<CoreEvent>>>>,
    next_msg_id: Arc<AtomicI64>,
    chats: Arc<Mutex<HashMap<ChatId, Chat>>>,
    messages: Arc<Mutex<HashMap<ChatId, Vec<Message>>>>,
    files: Arc<Mutex<HashMap<i32, FileInfo>>>,
    download_tasks: Arc<Mutex<HashMap<i32, tokio::sync::oneshot::Sender<()>>>>,
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            event_tx: Arc::new(Mutex::new(None)),
            next_msg_id: Arc::new(AtomicI64::new(1000)),
            chats: Arc::new(Mutex::new(HashMap::new())),
            messages: Arc::new(Mutex::new(HashMap::new())),
            files: Arc::new(Mutex::new(HashMap::new())),
            download_tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_file(&self, file_id: i32) -> Option<FileInfo> {
        self.files.lock().get(&file_id).cloned()
    }


    fn emit(&self, event: CoreEvent) {
        if let Some(ref sender) = *self.event_tx.lock() {
            let _ = sender.send(event);
        }
    }

    fn init_mock_chats(&self) {
        let my_id = UserId(777000);
        let my_user = User {
            id: my_id,
            first_name: "Rust".to_string(),
            last_name: "Developer".to_string(),
            username: Some("rust_dev".to_string()),
            phone_number: Some("+1234567890".to_string()),
            status: UserStatus::Online,
            is_contact: true,
            is_bot: false,
            is_verified: true,
        };
        self.emit(CoreEvent::UserUpdated(Box::new(my_user.clone())));
        self.emit(CoreEvent::MeUpdated(Box::new(my_user)));

        let chats_data = vec![
            (
                ChatId(1),
                "Saved Messages",
                ChatType::Private,
                "Welcome to your Rust Telegram client!",
                true,
            ),
            (
                ChatId(2),
                "Rust Community",
                ChatType::Supergroup,
                "Rust 1.99 released with incredible performance!",
                false,
            ),
            (
                ChatId(3),
                "Telegram News",
                ChatType::Channel,
                "Major update: TUI client launched on GitHub",
                false,
            ),
            (
                ChatId(4),
                "Pavel Durov",
                ChatType::Private,
                "Freedom of speech and privacy matters.",
                false,
            ),
        ];

        let mut chat_ids = Vec::new();
        let mut chats_map = self.chats.lock();
        let mut messages_map = self.messages.lock();

        for (i, (chat_id, title, chat_type, text, is_pinned)) in chats_data.into_iter().enumerate() {
            chat_ids.push(chat_id);
            let msg = Message {
                id: MessageId(i as i64 + 1),
                chat_id,
                sender: Sender::User(UserId(100 + i as i64)),
                date: chrono::Utc::now().timestamp() - (i as i64 * 300),
                content: MessageContent::Text {
                    text: text.to_string(),
                    entities: Vec::new(),
                },
                is_outgoing: false,
                status: MessageStatus::Read,
                reply_to_message_id: None,
                can_be_edited: false,
                can_be_deleted: false,
            };

            let chat = Chat {
                id: chat_id,
                title: title.to_string(),
                chat_type,
                unread_count: if is_pinned { 0 } else { 2 },
                last_read_inbox_message_id: 0,
                last_read_outbox_message_id: 0,
                last_message: Some(msg.clone()),
                position: Some(ChatPosition {
                    order: 1000 - (i as u64 * 100),
                    is_pinned,
                }),
                is_muted: false,
                has_unread_mention: false,
                folder_ids: Vec::new(),
            };

            // Seed initial message history for pagination tests and multimedia fixtures
            let mut hist = Vec::new();
            for m_idx in 1..=10 {
                let content = if chat_id == ChatId(1) && m_idx == 5 {
                    MessageContent::Document {
                        caption: "Project Architecture Specification".to_string(),
                        file_name: "architecture_specification.pdf".to_string(),
                        file_id: 101,
                        file_size: 1_843_200,
                    }
                } else if chat_id == ChatId(2) && m_idx == 4 {
                    MessageContent::Audio {
                        file_id: 201,
                        title: Some("Never Gonna Give You Up".to_string()),
                        performer: Some("Rick Astley".to_string()),
                        duration: 212,
                        file_size: 4_718_592,
                    }
                } else if chat_id == ChatId(2) && m_idx == 7 {
                    MessageContent::Sticker {
                        emoji: "🦀".to_string(),
                        file_id: 501,
                    }
                } else if chat_id == ChatId(3) && m_idx == 5 {
                    MessageContent::Photo {
                        caption: "TUI client preview banner".to_string(),
                        file_id: 301,
                        width: 1280,
                        height: 720,
                    }
                } else if chat_id == ChatId(4) && m_idx == 5 {
                    MessageContent::Voice {
                        caption: "Message from Pavel".to_string(),
                        duration: 42,
                        file_id: 401,
                    }
                } else {
                    MessageContent::Text {
                        text: format!("Historical message {} in {}", m_idx, title),
                        entities: Vec::new(),
                    }
                };

                hist.push(Message {
                    id: MessageId((i as i64 * 100) + m_idx),
                    chat_id,
                    sender: Sender::User(UserId(100 + i as i64)),
                    date: chrono::Utc::now().timestamp() - (3600 - m_idx * 60),
                    content,
                    is_outgoing: m_idx % 2 == 0,
                    status: MessageStatus::Read,
                    reply_to_message_id: None,
                    can_be_edited: m_idx % 2 == 0,
                    can_be_deleted: true,
                });
            }
            hist.push(msg.clone());

            chats_map.insert(chat_id, chat.clone());
            messages_map.insert(chat_id, hist);

            self.emit(CoreEvent::ChatUpdated(Box::new(chat)));
            self.emit(CoreEvent::MessageReceived(Box::new(msg)));
        }

        // Register initial FileInfo objects for mock media files
        let mut files_map = self.files.lock();

        let mock_files = vec![
            FileInfo::new(101, 1_843_200, 1_843_200), // Document in Chat 1
            FileInfo::new(201, 4_718_592, 4_718_592), // Audio in Chat 2
            FileInfo::new(301, 2_400_000, 2_400_000), // Photo in Chat 3
            FileInfo::new(401, 262_144, 262_144),     // Voice in Chat 4
            FileInfo::new(501, 65_536, 65_536),       // Sticker in Chat 2
        ];

        for file in mock_files {
            files_map.insert(file.id, file.clone());
            self.emit(CoreEvent::FileUpdated(file));
        }

        self.emit(CoreEvent::ChatListUpdated(chat_ids));
    }
}



impl TelegramBackend for MockBackend {
    fn start(&self, event_tx: broadcast::Sender<CoreEvent>) -> BoxFuture<'_, ()> {
        *self.event_tx.lock() = Some(event_tx);
        self.emit(CoreEvent::ConnectionStateChanged(ConnectionState::Ready));
        self.emit(CoreEvent::AuthStateChanged(AuthState::WaitPhoneNumber));
        Box::pin(async move { Ok(()) })
    }

    fn send_phone_number(&self, phone: &str) -> BoxFuture<'_, ()> {
        if phone.trim().is_empty() {
            self.emit(CoreEvent::AuthStateChanged(AuthState::WaitPhoneNumber));
            return Box::pin(async move { Ok(()) });
        }
        let phone = phone.to_string();
        self.emit(CoreEvent::AuthStateChanged(AuthState::WaitCode(CodeInfo {
            phone_number: phone,
            timeout: 60,
            code_type: "telegram_message".to_string(),
            next_type: Some("sms".to_string()),
        })));
        Box::pin(async move { Ok(()) })
    }

    fn send_code(&self, code: &str) -> BoxFuture<'_, ()> {
        let code = code.to_string();
        if code == "00000" {
            self.emit(CoreEvent::ErrorOccurred(
                "TDLib error (400): PHONE_CODE_INVALID".to_string(),
            ));
        } else if code == "22222" {
            self.emit(CoreEvent::AuthStateChanged(AuthState::WaitPassword(PasswordHint {
                hint: Some("Hint: default password is 'rust'".to_string()),
                has_recovery_email: true,
            })));
        } else {
            self.emit(CoreEvent::AuthStateChanged(AuthState::Ready));
            self.init_mock_chats();
        }
        Box::pin(async move { Ok(()) })
    }

    fn send_password(&self, password: &str) -> BoxFuture<'_, ()> {
        let pwd = password.to_string();
        if pwd == "wrong" {
            self.emit(CoreEvent::ErrorOccurred(
                "TDLib error (400): PASSWORD_HASH_INVALID".to_string(),
            ));
        } else {
            self.emit(CoreEvent::AuthStateChanged(AuthState::Ready));
            self.init_mock_chats();
        }
        Box::pin(async move { Ok(()) })
    }

    fn request_qr_code(&self) -> BoxFuture<'_, ()> {
        self.emit(CoreEvent::AuthStateChanged(AuthState::WaitQrCode(QrCodeInfo {
            link: "tg://login?token=MOCK_QR_TOKEN_DEMO".to_string(),
        })));
        Box::pin(async move { Ok(()) })
    }

    fn send_text_message(
        &self,
        chat_id: ChatId,
        text: &str,
        reply_to: Option<MessageId>,
    ) -> BoxFuture<'_, ()> {
        let text = text.to_string();
        let msg_id = MessageId(self.next_msg_id.fetch_add(1, Ordering::SeqCst));
        let outgoing = Message {
            id: msg_id,
            chat_id,
            sender: Sender::User(UserId(777000)),
            date: chrono::Utc::now().timestamp(),
            content: MessageContent::Text {
                text: text.clone(),
                entities: Vec::new(),
            },
            is_outgoing: true,
            status: MessageStatus::Sent,
            reply_to_message_id: reply_to,
            can_be_edited: true,
            can_be_deleted: true,
        };

        {
            let mut messages = self.messages.lock();
            messages.entry(chat_id).or_default().push(outgoing.clone());
        }
        {
            let mut chats = self.chats.lock();
            if let Some(chat) = chats.get_mut(&chat_id) {
                chat.last_message = Some(outgoing.clone());
            }
        }

        self.emit(CoreEvent::MessageReceived(Box::new(outgoing)));

        let tx_opt = self.event_tx.lock().clone();
        let next_id = self.next_msg_id.clone();
        let messages_arc = self.messages.clone();
        let chats_arc = self.chats.clone();

        Box::pin(async move {
            // Simulate automatic reply after 300ms
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                let reply_id = MessageId(next_id.fetch_add(1, Ordering::SeqCst));
                let echo_msg = Message {
                    id: reply_id,
                    chat_id,
                    sender: Sender::User(UserId(999)),
                    date: chrono::Utc::now().timestamp(),
                    content: MessageContent::Text {
                        text: format!("Mock auto-reply to: \"{}\"", text),
                        entities: Vec::new(),
                    },
                    is_outgoing: false,
                    status: MessageStatus::Read,
                    reply_to_message_id: Some(msg_id),
                    can_be_edited: false,
                    can_be_deleted: false,
                };

                {
                    let mut msgs = messages_arc.lock();
                    msgs.entry(chat_id).or_default().push(echo_msg.clone());
                }
                {
                    let mut cts = chats_arc.lock();
                    if let Some(c) = cts.get_mut(&chat_id) {
                        c.last_message = Some(echo_msg.clone());
                    }
                }

                if let Some(ref sender) = tx_opt {
                    let _ = sender.send(CoreEvent::MessageReceived(Box::new(echo_msg)));
                }
            });

            Ok(())
        })
    }

    fn edit_message_text(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        text: &str,
    ) -> BoxFuture<'_, ()> {
        let text = text.to_string();
        let mut edited_msg = None;

        {
            let mut messages = self.messages.lock();
            if let Some(msgs) = messages.get_mut(&chat_id) {
                if let Some(msg) = msgs.iter_mut().find(|m| m.id == message_id) {
                    msg.content = MessageContent::Text {
                        text: text.clone(),
                        entities: Vec::new(),
                    };
                    edited_msg = Some(msg.clone());
                }
            }
        }

        if let Some(ref msg) = edited_msg {
            let mut chats = self.chats.lock();
            if let Some(chat) = chats.get_mut(&chat_id) {
                if let Some(ref last) = chat.last_message {
                    if last.id == message_id {
                        chat.last_message = Some(msg.clone());
                    }
                }
            }
            self.emit(CoreEvent::MessageUpdated(Box::new(msg.clone())));
        }

        Box::pin(async move { Ok(()) })
    }

    fn delete_messages(
        &self,
        chat_id: ChatId,
        message_ids: Vec<MessageId>,
        _revoke: bool,
    ) -> BoxFuture<'_, ()> {
        {
            let mut messages = self.messages.lock();
            if let Some(msgs) = messages.get_mut(&chat_id) {
                msgs.retain(|m| !message_ids.contains(&m.id));
            }
        }
        {
            let mut chats = self.chats.lock();
            if let Some(chat) = chats.get_mut(&chat_id) {
                if let Some(ref last) = chat.last_message {
                    if message_ids.contains(&last.id) {
                        let messages = self.messages.lock();
                        chat.last_message = messages.get(&chat_id).and_then(|ms| ms.last()).cloned();
                    }
                }
            }
        }

        self.emit(CoreEvent::MessagesDeleted {
            chat_id,
            message_ids,
        });

        Box::pin(async move { Ok(()) })
    }

    fn load_history(
        &self,
        chat_id: ChatId,
        from_msg_id: Option<MessageId>,
        limit: i32,
    ) -> BoxFuture<'_, Vec<Message>> {
        let messages_guard = self.messages.lock();
        let msgs = messages_guard.get(&chat_id).cloned().unwrap_or_default();

        let result = if limit > 0 {
            // Backward pagination: messages older than from_msg_id
            let filtered: Vec<Message> = if let Some(from_id) = from_msg_id {
                if from_id.0 <= 0 {
                    msgs
                } else {
                    msgs.into_iter().filter(|m| m.id.0 < from_id.0).collect()
                }
            } else {
                msgs
            };
            let count = limit as usize;
            if filtered.len() > count {
                filtered[filtered.len() - count..].to_vec()
            } else {
                filtered
            }
        } else if limit < 0 {
            // Forward pagination: messages newer than from_msg_id
            let count = limit.unsigned_abs() as usize;
            let filtered: Vec<Message> = if let Some(from_id) = from_msg_id {
                msgs.into_iter().filter(|m| m.id.0 > from_id.0).collect()
            } else {
                msgs
            };
            if filtered.len() > count {
                filtered[..count].to_vec()
            } else {
                filtered
            }
        } else {
            Vec::new()
        };

        Box::pin(async move { Ok(result) })
    }

    fn download_file(&self, file_id: i32, _priority: i32) -> BoxFuture<'_, ()> {
        let (file_info, tx_opt, cancel_rx) = {
            let mut tasks = self.download_tasks.lock();
            let mut files = self.files.lock();
            let file = files.entry(file_id).or_insert_with(|| {
                FileInfo::new(file_id, 1_000_000, 1_000_000)
            });

            if file.is_completed() || file.is_downloading() || tasks.contains_key(&file_id) {
                return Box::pin(async move { Ok(()) });
            }

            let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
            tasks.insert(file_id, cancel_tx);
            (file.clone(), self.event_tx.lock().clone(), cancel_rx)
        };

        let files_arc = self.files.clone();
        let download_tasks_arc = self.download_tasks.clone();

        Box::pin(async move {
            tokio::spawn(async move {
                let mut cancel_rx = cancel_rx;
                let total = file_info.total_size().max(1024);

                // Step 1: Start downloading (33%)
                let p1 = total / 3;
                let mut current = file_info.clone();
                current.status = FileTransferStatus::Downloading { downloaded_size: p1 };
                {
                    let mut files = files_arc.lock();
                    files.insert(file_id, current.clone());
                }
                if let Some(ref tx) = tx_opt {
                    let _ = tx.send(CoreEvent::FileUpdated(current.clone()));
                }

                tokio::select! {
                    res = &mut cancel_rx => {
                        if res.is_ok() {
                            let mut cancelled = current;
                            cancelled.status = FileTransferStatus::Failed("Download cancelled".to_string());
                            files_arc.lock().insert(file_id, cancelled.clone());
                            if let Some(ref tx) = tx_opt {
                                let _ = tx.send(CoreEvent::FileUpdated(cancelled));
                            }
                            download_tasks_arc.lock().remove(&file_id);
                            return;
                        }
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(60)) => {}
                }

                // Step 2: Intermediate progress (66%)
                let p2 = (total * 2) / 3;
                current.status = FileTransferStatus::Downloading { downloaded_size: p2 };
                {
                    let mut files = files_arc.lock();
                    files.insert(file_id, current.clone());
                }
                if let Some(ref tx) = tx_opt {
                    let _ = tx.send(CoreEvent::FileUpdated(current.clone()));
                }

                tokio::select! {
                    res = &mut cancel_rx => {
                        if res.is_ok() {
                            let mut cancelled = current;
                            cancelled.status = FileTransferStatus::Failed("Download cancelled".to_string());
                            files_arc.lock().insert(file_id, cancelled.clone());
                            if let Some(ref tx) = tx_opt {
                                let _ = tx.send(CoreEvent::FileUpdated(cancelled));
                            }
                            download_tasks_arc.lock().remove(&file_id);
                            return;
                        }
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(60)) => {}
                }

                // Step 3: Complete download and write dummy file to .data/tdlib_files
                let dir = std::path::PathBuf::from(".data").join("tdlib_files");
                let _ = std::fs::create_dir_all(&dir);
                let target_path = dir.join(format!("mock_file_{}.bin", file_id));
                let dummy_bytes = format!("Simulated file payload for file_id={} total_size={}\n", file_id, total);
                let _ = std::fs::write(&target_path, dummy_bytes.as_bytes());

                current.status = FileTransferStatus::Completed;
                current.local_path = Some(target_path);
                {
                    let mut files = files_arc.lock();
                    files.insert(file_id, current.clone());
                }
                if let Some(ref tx) = tx_opt {
                    let _ = tx.send(CoreEvent::FileUpdated(current));
                }

                download_tasks_arc.lock().remove(&file_id);
            });

            Ok(())
        })
    }

    fn cancel_download_file(&self, file_id: i32) -> BoxFuture<'_, ()> {
        let mut tasks = self.download_tasks.lock();
        if let Some(sender) = tasks.remove(&file_id) {
            let _ = sender.send(());
        } else {
            let mut files = self.files.lock();
            if let Some(file) = files.get_mut(&file_id) {
                if file.is_downloading() {
                    file.status = FileTransferStatus::Failed("Download cancelled".to_string());
                    self.emit(CoreEvent::FileUpdated(file.clone()));
                }
            }
        }
        Box::pin(async move { Ok(()) })
    }

    fn set_proxy(&self, proxy: Option<ProxyConfig>) -> BoxFuture<'_, ()> {
        let tx_opt = self.event_tx.lock().clone();
        Box::pin(async move {
            match proxy {
                Some(p) => {
                    if p.is_enabled {
                        if let Some(ref tx) = tx_opt {
                            let _ = tx.send(CoreEvent::ConnectionStateChanged(
                                ConnectionState::ConnectingToProxy,
                            ));
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        if let Some(ref tx) = tx_opt {
                            let _ = tx.send(CoreEvent::ConnectionStateChanged(ConnectionState::Ready));
                        }
                    } else if let Some(ref tx) = tx_opt {
                        let _ = tx.send(CoreEvent::ConnectionStateChanged(ConnectionState::Ready));
                    }
                }
                None => {
                    if let Some(ref tx) = tx_opt {
                        let _ = tx.send(CoreEvent::ConnectionStateChanged(ConnectionState::Ready));
                    }
                }
            }
            Ok(())
        })
    }

    fn log_out(&self) -> BoxFuture<'_, ()> {
        self.emit(CoreEvent::AuthStateChanged(AuthState::Closed));
        Box::pin(async move { Ok(()) })
    }

    fn stop(&self) -> BoxFuture<'_, ()> {
        *self.event_tx.lock() = None;
        Box::pin(async move { Ok(()) })
    }
}
