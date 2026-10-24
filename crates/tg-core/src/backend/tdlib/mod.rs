pub mod raw;
pub mod router;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use serde_json::{json, Value};

use crate::backend::{BoxFuture, TelegramBackend};
use crate::config::Config;
use crate::domain::*;
use crate::error::{CoreError, Result};
use crate::event::{ConnectionState, CoreEvent};
use raw::TdJsonRaw;
use router::{RequestRegistration, RequestRouter};

struct TDLibInner {
    raw: TdJsonRaw,
    client_id: i32,
    config: Config,
    is_running: AtomicBool,
    router: Arc<RequestRouter>,
}

impl TDLibInner {
    fn send_json(&self, value: Value) -> Result<()> {
        let req_str = serde_json::to_string(&value)?;
        self.raw.send(self.client_id, &req_str)
    }

    async fn send_request(&self, request: Value) -> Result<Value> {
        self.send_request_with_timeout(request, self.router.default_timeout()).await
    }

    async fn send_request_with_timeout(&self, mut request: Value, timeout_dur: Duration) -> Result<Value> {
        let (extra_id, rx) = self.router.register();
        request["@extra"] = Value::String(extra_id.clone());

        let reg = RequestRegistration::new(&self.router, extra_id.clone());
        let req_str = serde_json::to_string(&request)?;

        if let Err(e) = self.raw.send(self.client_id, &req_str) {
            return Err(e);
        }

        match tokio::time::timeout(timeout_dur, rx).await {
            Ok(Ok(val)) => {
                reg.complete();
                val
            }
            Ok(Err(_closed)) => {
                Err(CoreError::Internal(format!("Request {} channel closed", extra_id)))
            }
            Err(_elapsed) => {
                Err(CoreError::Internal(format!("Request {} timed out after {:?}", extra_id, timeout_dur)))
            }
        }
    }

    fn send_encryption_key(&self) -> Result<()> {
        let req = json!({
            "@type": "checkDatabaseEncryptionKey",
            "encryption_key": ""
        });
        self.send_json(req)
    }

    fn parse_user(&self, v: &Value) -> Option<User> {
        let id = UserId(v.get("id")?.as_i64()?);
        let first_name = v.get("first_name").and_then(|s| s.as_str()).unwrap_or("").to_string();
        let last_name = v.get("last_name").and_then(|s| s.as_str()).unwrap_or("").to_string();
        let username = v
            .get("usernames")
            .and_then(|u| u.get("editable_username"))
            .and_then(|s| s.as_str())
            .or_else(|| v.get("username").and_then(|s| s.as_str()))
            .map(|s| s.to_string());
        let phone_number = v.get("phone_number").and_then(|s| s.as_str()).map(|s| s.to_string());

        let status = if let Some(st) = v.get("status").and_then(|s| s.get("@type")).and_then(|t| t.as_str()) {
            match st {
                "userStatusOnline" => UserStatus::Online,
                "userStatusOffline" => {
                    let was_online = v.get("status").and_then(|s| s.get("was_online")).and_then(|w| w.as_i64()).unwrap_or(0);
                    UserStatus::Offline { was_online }
                }
                "userStatusRecently" => UserStatus::Recently,
                "userStatusLastWeek" => UserStatus::LastWeek,
                "userStatusLastMonth" => UserStatus::LastMonth,
                _ => UserStatus::Unknown,
            }
        } else {
            UserStatus::Unknown
        };

        let is_contact = v.get("is_contact").and_then(|b| b.as_bool()).unwrap_or(false);
        let is_bot = v.get("type").and_then(|t| t.get("@type")).and_then(|s| s.as_str()).map(|s| s.contains("Bot")).unwrap_or(false);
        let is_verified = v.get("is_verified").and_then(|b| b.as_bool()).unwrap_or(false);

        Some(User {
            id,
            first_name,
            last_name,
            username,
            phone_number,
            status,
            is_contact,
            is_bot,
            is_verified,
        })
    }

    fn handle_update(&self, update: Value, event_tx: &broadcast::Sender<CoreEvent>) {
        let update_type = match update.get("@type").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return,
        };

        match update_type {
            "updateAuthorizationState" => {
                if let Some(auth_state_obj) = update.get("authorization_state") {
                    let state_type = auth_state_obj.get("@type").and_then(|v| v.as_str()).unwrap_or("");
                    match state_type {
                        "authorizationStateWaitTdlibParameters" => {
                            let _ = self.set_parameters();
                        }
                        "authorizationStateWaitEncryptionKey" => {
                            if let Err(e) = self.send_encryption_key() {
                                tracing::error!("Failed to send database encryption key: {}", e);
                                let _ = event_tx.send(CoreEvent::ErrorOccurred(format!(
                                    "Database encryption error: {}",
                                    e
                                )));
                            }
                        }
                        "authorizationStateWaitPhoneNumber" => {
                            let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::WaitPhoneNumber));
                        }
                        "authorizationStateWaitOtherDeviceConfirmation" => {
                            let link = auth_state_obj
                                .get("link")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::WaitQrCode(QrCodeInfo { link })));
                        }
                        "authorizationStateWaitCode" => {
                            let code_info = auth_state_obj.get("code_info");
                            let phone = code_info
                                .and_then(|c| c.get("phone_number"))
                                .and_then(|p| p.as_str())
                                .unwrap_or("")
                                .to_string();
                            let timeout = code_info
                                .and_then(|c| c.get("timeout"))
                                .and_then(|t| t.as_i64())
                                .unwrap_or(60) as i32;
                            let code_type = code_info
                                .and_then(|c| c.get("type"))
                                .and_then(|t| t.get("@type"))
                                .and_then(|s| s.as_str())
                                .unwrap_or("authenticationCodeTypeTelegramMessage")
                                .to_string();
                            let next_type = code_info
                                .and_then(|c| c.get("next_type"))
                                .and_then(|t| t.get("@type"))
                                .and_then(|s| s.as_str())
                                .map(|s| s.to_string());

                            let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::WaitCode(CodeInfo {
                                phone_number: phone,
                                timeout,
                                code_type,
                                next_type,
                            })));
                        }
                        "authorizationStateWaitPassword" => {
                            let hint = auth_state_obj
                                .get("password_hint")
                                .and_then(|h| h.as_str())
                                .map(|s| s.to_string());
                            let has_email = auth_state_obj
                                .get("has_recovery_email_address")
                                .or_else(|| auth_state_obj.get("has_recovery_email"))
                                .and_then(|b| b.as_bool())
                                .unwrap_or(false);
                            let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::WaitPassword(PasswordHint {
                                hint,
                                has_recovery_email: has_email,
                            })));
                        }
                        "authorizationStateWaitRegistration" => {
                            let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::WaitRegistration));
                        }
                        "authorizationStateReady" => {
                            let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::Ready));

                            // Query current user profile and initial chats
                            let _ = self.send_json(json!({
                                "@type": "getMe",
                                "@extra": "get_me_request"
                            }));
                            let _ = self.send_json(json!({
                                "@type": "loadChats",
                                "chat_list": { "@type": "chatListMain" },
                                "limit": 50
                            }));
                        }
                        "authorizationStateLoggingOut" => {
                            let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::LoggingOut));
                        }
                        "authorizationStateClosed" => {
                            let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::Closed));
                        }
                        _ => {}
                    }
                }
            }
            "updateOption" => {
                if let (Some(name), Some(val)) = (update.get("name").and_then(|n| n.as_str()), update.get("value")) {
                    if name == "my_id" {
                        let uid_opt = val.get("value").and_then(|v| match v {
                            Value::Number(n) => n.as_i64(),
                            Value::String(s) => s.parse::<i64>().ok(),
                            _ => None,
                        });
                        if let Some(uid) = uid_opt {
                            let _ = self.send_json(json!({
                                "@type": "getUser",
                                "user_id": uid,
                                "@extra": "get_me_request"
                            }));
                        }
                    }
                }
            }
            "updateUser" => {
                if let Some(user_obj) = update.get("user") {
                    if let Some(user) = self.parse_user(user_obj) {
                        let is_me = update.get("@extra").and_then(|e| e.as_str()) == Some("get_me_request");
                        if is_me {
                            let _ = event_tx.send(CoreEvent::MeUpdated(Box::new(user)));
                        } else {
                            let _ = event_tx.send(CoreEvent::UserUpdated(Box::new(user)));
                        }
                    }
                }
            }
            "user" => {
                if let Some(user) = self.parse_user(&update) {
                    let is_me = update.get("@extra").and_then(|e| e.as_str()) == Some("get_me_request");
                    if is_me {
                        let _ = event_tx.send(CoreEvent::MeUpdated(Box::new(user)));
                    } else {
                        let _ = event_tx.send(CoreEvent::UserUpdated(Box::new(user)));
                    }
                }
            }
            "error" => {
                let code = update.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
                let message = update.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown TDLib error");
                let err_str = format!("TDLib error ({}): {}", code, message);
                tracing::error!("{}", err_str);
                let _ = event_tx.send(CoreEvent::ErrorOccurred(err_str));
            }
            "updateConnectionState" => {
                if let Some(state_obj) = update.get("state") {
                    let state_type = state_obj.get("@type").and_then(|v| v.as_str()).unwrap_or("");
                    let state = match state_type {
                        "connectionStateWaitingForNetwork" => ConnectionState::WaitingForNetwork,
                        "connectionStateConnectingToProxy" => ConnectionState::ConnectingToProxy,
                        "connectionStateConnecting" => ConnectionState::Connecting,
                        "connectionStateUpdating" => ConnectionState::Updating,
                        "connectionStateReady" => ConnectionState::Ready,
                        _ => ConnectionState::Connecting,
                    };
                    let _ = event_tx.send(CoreEvent::ConnectionStateChanged(state));
                }
            }
            "updateNewChat" => {
                if let Some(chat_obj) = update.get("chat") {
                    if let Some(chat) = self.parse_chat(chat_obj) {
                        let _ = event_tx.send(CoreEvent::ChatUpdated(Box::new(chat)));
                    }
                }
            }
            "updateChatPosition" => {
                if let (Some(chat_id), Some(pos_obj)) = (
                    update.get("chat_id").and_then(|c| c.as_i64()),
                    update.get("position"),
                ) {
                    let is_main = pos_obj
                        .get("list")
                        .and_then(|l| l.get("@type"))
                        .and_then(|t| t.as_str())
                        == Some("chatListMain");
                    if is_main {
                        let order = pos_obj
                            .get("order")
                            .and_then(|o| match o {
                                Value::String(s) => s.parse::<u64>().ok(),
                                Value::Number(n) => n.as_u64(),
                                _ => None,
                            })
                            .unwrap_or(0);
                        let is_pinned = pos_obj.get("is_pinned").and_then(|p| p.as_bool()).unwrap_or(false);
                        let _ = event_tx.send(CoreEvent::ChatPositionChanged {
                            chat_id: ChatId(chat_id),
                            position: Some(ChatPosition { order, is_pinned }),
                        });
                    }
                }
            }
            "updateChatPositions" => {
                if let (Some(chat_id), Some(positions)) = (
                    update.get("chat_id").and_then(|c| c.as_i64()),
                    update.get("positions").and_then(|p| p.as_array()),
                ) {
                    let main_pos = positions.iter().find_map(|pos| {
                        let is_main = pos
                            .get("list")
                            .and_then(|l| l.get("@type"))
                            .and_then(|t| t.as_str())
                            == Some("chatListMain");
                        if is_main {
                            let order = pos
                                .get("order")
                                .and_then(|o| match o {
                                    Value::String(s) => s.parse::<u64>().ok(),
                                    Value::Number(n) => n.as_u64(),
                                    _ => None,
                                })
                                .unwrap_or(0);
                            let is_pinned = pos.get("is_pinned").and_then(|p| p.as_bool()).unwrap_or(false);
                            Some(ChatPosition { order, is_pinned })
                        } else {
                            None
                        }
                    });
                    let _ = event_tx.send(CoreEvent::ChatPositionChanged {
                        chat_id: ChatId(chat_id),
                        position: main_pos,
                    });

                    let folder_ids: Vec<i32> = positions
                        .iter()
                        .filter_map(|pos| {
                            let is_folder = pos
                                .get("list")
                                .and_then(|l| l.get("@type"))
                                .and_then(|t| t.as_str())
                                == Some("chatListFolder");
                            if is_folder {
                                pos.get("list")
                                    .and_then(|l| l.get("chat_folder_id"))
                                    .and_then(|id| id.as_i64())
                                    .map(|id| id as i32)
                            } else {
                                None
                            }
                        })
                        .collect();
                    let _ = event_tx.send(CoreEvent::ChatFolderPositionsChanged {
                        chat_id: ChatId(chat_id),
                        folder_ids,
                    });
                }
            }
            "updateChatFolders" => {
                if let Some(folders_arr) = update.get("chat_folders").and_then(|f| f.as_array()) {
                    let mut folders = Vec::new();
                    for f in folders_arr {
                        let id = f.get("id").and_then(|i| i.as_i64()).unwrap_or(0) as i32;
                        let title = f.get("title").and_then(|t| t.as_str()).unwrap_or("Folder").to_string();
                        let icon = f.get("icon_name").and_then(|i| i.as_str()).map(|s| s.to_string());
                        folders.push(ChatFolder { id, title, icon_name: icon });

                        // Proactively load chats from this folder
                        let _ = self.send_json(json!({
                            "@type": "loadChats",
                            "chat_list": {
                                "@type": "chatListFolder",
                                "chat_folder_id": id
                            },
                            "limit": 50
                        }));
                    }
                    let _ = event_tx.send(CoreEvent::ChatFoldersUpdated(folders));
                }
            }
            "updateChatLastMessage" => {
                if let Some(chat_id) = update.get("chat_id").and_then(|c| c.as_i64()) {
                    if let Some(msg_obj) = update.get("last_message") {
                        if let Some(msg) = self.parse_message(msg_obj) {
                            let _ = event_tx.send(CoreEvent::MessageReceived(Box::new(msg)));
                        }
                    }
                    if let Some(positions) = update.get("positions").and_then(|p| p.as_array()) {
                        let main_pos = positions.iter().find_map(|pos| {
                            let is_main = pos
                                .get("list")
                                .and_then(|l| l.get("@type"))
                                .and_then(|t| t.as_str())
                                == Some("chatListMain");
                            if is_main {
                                let order = pos
                                    .get("order")
                                    .and_then(|o| match o {
                                        Value::String(s) => s.parse::<u64>().ok(),
                                        Value::Number(n) => n.as_u64(),
                                        _ => None,
                                    })
                                    .unwrap_or(0);
                                let is_pinned = pos.get("is_pinned").and_then(|p| p.as_bool()).unwrap_or(false);
                                Some(ChatPosition { order, is_pinned })
                            } else {
                                None
                            }
                        });
                        let _ = event_tx.send(CoreEvent::ChatPositionChanged {
                            chat_id: ChatId(chat_id),
                            position: main_pos,
                        });
                    }
                }
            }
            "updateChatReadInbox" => {
                if let Some(chat_id) = update.get("chat_id").and_then(|c| c.as_i64()) {
                    let last_read = update
                        .get("last_read_inbox_message_id")
                        .and_then(|m| m.as_i64())
                        .unwrap_or(0);
                    let unread = update.get("unread_count").and_then(|u| u.as_i64()).unwrap_or(0) as i32;
                    let _ = event_tx.send(CoreEvent::ChatReadInboxChanged {
                        chat_id: ChatId(chat_id),
                        last_read_inbox_message_id: last_read,
                        unread_count: unread,
                    });
                }
            }
            "updateChatReadOutbox" => {
                if let Some(chat_id) = update.get("chat_id").and_then(|c| c.as_i64()) {
                    let last_read = update
                        .get("last_read_outbox_message_id")
                        .and_then(|m| m.as_i64())
                        .unwrap_or(0);
                    let _ = event_tx.send(CoreEvent::ChatReadOutboxChanged {
                        chat_id: ChatId(chat_id),
                        last_read_outbox_message_id: last_read,
                    });
                }
            }
            "updateNewMessage" => {
                if let Some(msg_obj) = update.get("message") {
                    if let Some(msg) = self.parse_message(msg_obj) {
                        let _ = event_tx.send(CoreEvent::MessageReceived(Box::new(msg)));
                    }
                }
            }
            "updateMessageSendSucceeded" => {
                if let (Some(chat_id), Some(msg_obj)) = (
                    update.get("message").and_then(|m| m.get("chat_id")).and_then(|c| c.as_i64()),
                    update.get("message"),
                ) {
                    if let Some(msg) = self.parse_message(msg_obj) {
                        let _ = event_tx.send(CoreEvent::MessageStatusChanged {
                            chat_id: ChatId(chat_id),
                            message_id: msg.id,
                            status: MessageStatus::Sent,
                        });
                        let _ = event_tx.send(CoreEvent::MessageReceived(Box::new(msg)));
                    }
                }
            }
            "updateMessageSendFailed" => {
                if let (Some(chat_id), Some(old_id)) = (
                    update.get("message").and_then(|m| m.get("chat_id")).and_then(|c| c.as_i64()),
                    update.get("old_message_id").and_then(|o| o.as_i64()),
                ) {
                    let _ = event_tx.send(CoreEvent::MessageStatusChanged {
                        chat_id: ChatId(chat_id),
                        message_id: MessageId(old_id),
                        status: MessageStatus::Failed,
                    });
                }
            }
            "updateMessageContent" => {
                if let (Some(chat_id), Some(msg_id)) = (
                    update.get("chat_id").and_then(|c| c.as_i64()),
                    update.get("message_id").and_then(|m| m.as_i64()),
                ) {
                    let content = if let Some(new_content) = update.get("new_content") {
                        Self::parse_message_content(new_content)
                    } else {
                        MessageContent::Unsupported {
                            raw_type: "unknown".to_string(),
                        }
                    };
                    let msg = Message {
                        id: MessageId(msg_id),
                        chat_id: ChatId(chat_id),
                        sender: Sender::Unknown,
                        date: 0,
                        content,
                        is_outgoing: false,
                        status: MessageStatus::Read,
                        reply_to_message_id: None,
                        can_be_edited: false,
                        can_be_deleted: false,
                    };
                    let _ = event_tx.send(CoreEvent::MessageUpdated(Box::new(msg)));
                }
            }
            "updateDeleteMessages" => {
                if let (Some(chat_id), Some(msg_ids)) = (
                    update.get("chat_id").and_then(|c| c.as_i64()),
                    update.get("message_ids").and_then(|m| m.as_array()),
                ) {
                    let ids: Vec<MessageId> =
                        msg_ids.iter().filter_map(|v| v.as_i64().map(MessageId)).collect();
                    let _ = event_tx.send(CoreEvent::MessagesDeleted {
                        chat_id: ChatId(chat_id),
                        message_ids: ids,
                    });
                }
            }
            "updateFile" => {
                if let Some(file_obj) = update.get("file") {
                    if let Some(file_info) = Self::parse_file(file_obj) {
                        let _ = event_tx.send(CoreEvent::FileUpdated(file_info));
                    }
                }
            }
            _ => {}
        }
    }


    fn set_parameters(&self) -> Result<()> {
        let db_dir = self.config.tdlib_database_path();
        let files_dir = self.config.tdlib_files_path();
        let _ = std::fs::create_dir_all(&db_dir);
        let _ = std::fs::create_dir_all(&files_dir);

        let db_str = db_dir.to_string_lossy().replace('\\', "/");
        let files_str = files_dir.to_string_lossy().replace('\\', "/");

        let params = json!({
            "@type": "setTdlibParameters",
            "database_directory": db_str,
            "files_directory": files_str,
            "api_id": self.config.api_id,
            "api_hash": self.config.api_hash,
            "system_language_code": "en",
            "device_model": "Desktop",
            "system_version": "Cross-Platform",
            "application_version": "0.1.0",
            "enable_storage_optimizer": true,
            "use_test_dc": self.config.use_test_dc,
            "use_message_database": true,
            "use_chat_info_database": true,
            "use_file_database": true,
            "use_secret_chats": true
        });
        self.send_json(params)
    }

    fn parse_chat(&self, v: &Value) -> Option<Chat> {
        let id = ChatId(v.get("id")?.as_i64()?);
        let title = v.get("title").and_then(|s| s.as_str()).unwrap_or("Unknown").to_string();

        let chat_type = match v
            .get("type")
            .and_then(|t| t.get("@type"))
            .and_then(|s| s.as_str())
            .unwrap_or("")
        {
            "chatTypePrivate" => ChatType::Private,
            "chatTypeSecret" => ChatType::Secret,
            "chatTypeBasicGroup" => ChatType::BasicGroup,
            "chatTypeSupergroup" => {
                let is_channel = v
                    .get("type")
                    .and_then(|t| t.get("is_channel"))
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);
                if is_channel {
                    ChatType::Channel
                } else {
                    ChatType::Supergroup
                }
            }
            _ => ChatType::Private,
        };

        let unread_count = v.get("unread_count").and_then(|u| u.as_i64()).unwrap_or(0) as i32;
        let last_read_inbox_message_id = v
            .get("last_read_inbox_message_id")
            .and_then(|u| u.as_i64())
            .unwrap_or(0);
        let last_read_outbox_message_id = v
            .get("last_read_outbox_message_id")
            .and_then(|u| u.as_i64())
            .unwrap_or(0);
        let last_message = v.get("last_message").and_then(|m| self.parse_message(m));

        let position = if let Some(positions) = v.get("positions").and_then(|p| p.as_array()) {
            positions.iter().find_map(|pos| {
                let is_main = pos
                    .get("list")
                    .and_then(|l| l.get("@type"))
                    .and_then(|t| t.as_str())
                    == Some("chatListMain");
                if is_main {
                    let order = pos
                        .get("order")
                        .and_then(|o| match o {
                            Value::String(s) => s.parse::<u64>().ok(),
                            Value::Number(n) => n.as_u64(),
                            _ => None,
                        })
                        .unwrap_or(0);
                    let is_pinned = pos.get("is_pinned").and_then(|p| p.as_bool()).unwrap_or(false);
                    Some(ChatPosition { order, is_pinned })
                } else {
                    None
                }
            })
        } else if let Some(pos) = v.get("position") {
            let is_main = pos
                .get("list")
                .and_then(|l| l.get("@type"))
                .and_then(|t| t.as_str())
                == Some("chatListMain");
            if is_main {
                let order = pos
                    .get("order")
                    .and_then(|o| match o {
                        Value::String(s) => s.parse::<u64>().ok(),
                        Value::Number(n) => n.as_u64(),
                        _ => None,
                    })
                    .unwrap_or(0);
                let is_pinned = pos.get("is_pinned").and_then(|p| p.as_bool()).unwrap_or(false);
                Some(ChatPosition { order, is_pinned })
            } else {
                None
            }
        } else {
            None
        };

        let is_muted = v
            .get("notification_settings")
            .and_then(|n| n.get("mute_for"))
            .and_then(|m| m.as_i64())
            .map(|m| m > 0)
            .unwrap_or(false);

        let has_unread_mention = v
            .get("unread_mention_count")
            .and_then(|u| u.as_i64())
            .map(|u| u > 0)
            .unwrap_or(false);

        let folder_ids: Vec<i32> = if let Some(positions) = v.get("positions").and_then(|p| p.as_array()) {
            positions
                .iter()
                .filter_map(|pos| {
                    let is_folder = pos
                        .get("list")
                        .and_then(|l| l.get("@type"))
                        .and_then(|t| t.as_str())
                        == Some("chatListFolder");
                    if is_folder {
                        pos.get("list")
                            .and_then(|l| l.get("chat_folder_id"))
                            .and_then(|id| id.as_i64())
                            .map(|id| id as i32)
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        Some(Chat {
            id,
            title,
            chat_type,
            unread_count,
            last_read_inbox_message_id,
            last_read_outbox_message_id,
            last_message,
            position,
            is_muted,
            has_unread_mention,
            folder_ids,
        })
    }

    fn parse_message(&self, v: &Value) -> Option<Message> {
        let id = MessageId(v.get("id")?.as_i64()?);
        let chat_id = ChatId(v.get("chat_id")?.as_i64()?);
        let date = v.get("date").and_then(|d| d.as_i64()).unwrap_or(0);
        let is_outgoing = v.get("is_outgoing").and_then(|b| b.as_bool()).unwrap_or(false);

        let sender = if let Some(sender_id) = v.get("sender_id") {
            let s_type = sender_id.get("@type").and_then(|t| t.as_str()).unwrap_or("");
            if s_type == "messageSenderUser" {
                sender_id
                    .get("user_id")
                    .and_then(|u| u.as_i64())
                    .map(|id| Sender::User(UserId(id)))
                    .unwrap_or(Sender::Unknown)
            } else if s_type == "messageSenderChat" {
                sender_id
                    .get("chat_id")
                    .and_then(|c| c.as_i64())
                    .map(|id| Sender::Chat(ChatId(id)))
                    .unwrap_or(Sender::Unknown)
            } else {
                Sender::Unknown
            }
        } else {
            Sender::Unknown
        };

        let status = if let Some(sending_state) = v.get("sending_state") {
            let st = sending_state.get("@type").and_then(|s| s.as_str()).unwrap_or("");
            if st == "messageSendingStateFailed" {
                MessageStatus::Failed
            } else {
                MessageStatus::Sending
            }
        } else if is_outgoing {
            MessageStatus::Sent
        } else {
            MessageStatus::Read
        };

        let content = if let Some(content_obj) = v.get("content") {
            Self::parse_message_content(content_obj)
        } else {
            MessageContent::Unsupported {
                raw_type: "unknown".to_string(),
            }
        };

        let reply_to_message_id = if let Some(reply_to) = v.get("reply_to") {
            reply_to.get("message_id").and_then(|m| m.as_i64()).map(MessageId)
        } else {
            v.get("reply_to_message_id").and_then(|m| m.as_i64()).map(MessageId)
        };

        let can_be_edited = v.get("can_be_edited").and_then(|b| b.as_bool()).unwrap_or(false);
        let can_be_deleted = v
            .get("can_be_deleted_only_for_self")
            .or_else(|| v.get("can_be_deleted_for_all_users"))
            .and_then(|b| b.as_bool())
            .unwrap_or(false);

        Some(Message {
            id,
            chat_id,
            sender,
            date,
            content,
            is_outgoing,
            status,
            reply_to_message_id,
            can_be_edited,
            can_be_deleted,
        })
    }

    pub(crate) fn parse_file(v: &Value) -> Option<FileInfo> {
        let id = v.get("id")?.as_i64()? as i32;
        let size = v.get("size").and_then(|s| s.as_i64()).unwrap_or(0);
        let expected_size = v.get("expected_size").and_then(|s| s.as_i64()).unwrap_or(size);

        let local = v.get("local");
        let local_path = local
            .and_then(|l| l.get("path"))
            .and_then(|p| p.as_str())
            .filter(|p| !p.is_empty())
            .map(std::path::PathBuf::from);

        let is_downloading_completed = local
            .and_then(|l| l.get("is_downloading_completed"))
            .and_then(|b| b.as_bool())
            .unwrap_or(false);

        let is_downloading_active = local
            .and_then(|l| l.get("is_downloading_active"))
            .and_then(|b| b.as_bool())
            .unwrap_or(false);

        let downloaded_size = local
            .and_then(|l| l.get("downloaded_size"))
            .and_then(|s| s.as_i64())
            .unwrap_or(0);

        let remote = v.get("remote");
        let is_uploading_active = remote
            .and_then(|r| r.get("is_uploading_active"))
            .and_then(|b| b.as_bool())
            .unwrap_or(false);

        let uploaded_size = remote
            .and_then(|r| r.get("uploaded_size"))
            .and_then(|s| s.as_i64())
            .unwrap_or(0);

        let status = if is_downloading_completed {
            FileTransferStatus::Completed
        } else if is_downloading_active {
            FileTransferStatus::Downloading { downloaded_size }
        } else if is_uploading_active {
            FileTransferStatus::Uploading { uploaded_size }
        } else {
            FileTransferStatus::Idle
        };

        Some(FileInfo {
            id,
            size,
            expected_size,
            local_path,
            status,
        })
    }

    pub(crate) fn parse_text_entities(text_obj: Option<&Value>) -> Vec<TextEntity> {
        let mut result = Vec::new();
        if let Some(entities_val) = text_obj.and_then(|t| t.get("entities")).and_then(|e| e.as_array()) {
            for entity in entities_val {
                if let (Some(offset), Some(length), Some(entity_type_obj)) = (
                    entity.get("offset").and_then(|o| o.as_i64()),
                    entity.get("length").and_then(|l| l.as_i64()),
                    entity.get("type"),
                ) {
                    let entity_type = entity_type_obj
                        .get("@type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    result.push(TextEntity {
                        offset: offset as i32,
                        length: length as i32,
                        entity_type,
                    });
                }
            }
        }
        result
    }

    pub(crate) fn parse_message_content(content_obj: &Value) -> MessageContent {
        let content_type = content_obj.get("@type").and_then(|t| t.as_str()).unwrap_or("");
        match content_type {
            "messageText" => {
                let text = content_obj
                    .get("text")
                    .and_then(|t| t.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let entities = Self::parse_text_entities(content_obj.get("text"));
                MessageContent::Text { text, entities }
            }
            "messagePhoto" => {
                let caption = content_obj
                    .get("caption")
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let photo = content_obj.get("photo");
                let sizes = photo.and_then(|p| p.get("sizes")).and_then(|s| s.as_array());
                let last_size = sizes.and_then(|s| s.last());
                let file_id = last_size
                    .and_then(|s| s.get("photo"))
                    .and_then(|p| p.get("id"))
                    .and_then(|id| id.as_i64())
                    .unwrap_or(0) as i32;
                let width = last_size
                    .and_then(|s| s.get("width"))
                    .and_then(|w| w.as_i64())
                    .unwrap_or(0) as i32;
                let height = last_size
                    .and_then(|s| s.get("height"))
                    .and_then(|h| h.as_i64())
                    .unwrap_or(0) as i32;
                MessageContent::Photo {
                    caption,
                    file_id,
                    width,
                    height,
                }
            }
            "messageDocument" => {
                let caption = content_obj
                    .get("caption")
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let doc = content_obj.get("document");
                let file_name = doc
                    .and_then(|d| d.get("file_name"))
                    .and_then(|f| f.as_str())
                    .unwrap_or("document")
                    .to_string();
                let doc_file = doc.and_then(|d| d.get("document"));
                let file_id = doc_file
                    .and_then(|f| f.get("id"))
                    .and_then(|id| id.as_i64())
                    .unwrap_or(0) as i32;
                let file_size = doc_file
                    .and_then(|f| f.get("size"))
                    .and_then(|s| s.as_i64())
                    .unwrap_or(0) as u64;
                MessageContent::Document {
                    caption,
                    file_name,
                    file_id,
                    file_size,
                }
            }
            "messageVoiceNote" => {
                let caption = content_obj
                    .get("caption")
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let voice_note = content_obj.get("voice_note");
                let duration = voice_note
                    .and_then(|v| v.get("duration"))
                    .and_then(|d| d.as_i64())
                    .unwrap_or(0) as i32;
                let voice_file = voice_note.and_then(|v| v.get("voice"));
                let file_id = voice_file
                    .and_then(|v| v.get("id"))
                    .and_then(|id| id.as_i64())
                    .unwrap_or(0) as i32;
                MessageContent::Voice {
                    caption,
                    duration,
                    file_id,
                }
            }
            "messageAudio" => {
                let audio = content_obj.get("audio");
                let duration = audio
                    .and_then(|a| a.get("duration"))
                    .and_then(|d| d.as_i64())
                    .unwrap_or(0) as i32;
                let title = audio
                    .and_then(|a| a.get("title"))
                    .and_then(|t| t.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                let performer = audio
                    .and_then(|a| a.get("performer"))
                    .and_then(|p| p.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                let audio_file = audio.and_then(|a| a.get("audio"));
                let file_id = audio_file
                    .and_then(|f| f.get("id"))
                    .and_then(|id| id.as_i64())
                    .unwrap_or(0) as i32;
                let file_size = audio_file
                    .and_then(|f| f.get("size"))
                    .and_then(|s| s.as_i64())
                    .unwrap_or(0);
                MessageContent::Audio {
                    file_id,
                    title,
                    performer,
                    duration,
                    file_size,
                }
            }
            "messageSticker" => {
                let sticker = content_obj.get("sticker");
                let emoji = sticker
                    .and_then(|s| s.get("emoji"))
                    .and_then(|e| e.as_str())
                    .unwrap_or("")
                    .to_string();
                let sticker_file = sticker.and_then(|s| s.get("sticker"));
                let file_id = sticker_file
                    .and_then(|f| f.get("id"))
                    .and_then(|id| id.as_i64())
                    .unwrap_or(0) as i32;
                MessageContent::Sticker { emoji, file_id }
            }
            _ => {
                if let Some(text_obj) = content_obj.get("text") {
                    let text = text_obj.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
                    let entities = Self::parse_text_entities(Some(text_obj));
                    MessageContent::Text { text, entities }
                } else {
                    MessageContent::Unsupported {
                        raw_type: content_type.to_string(),
                    }
                }
            }
        }
    }
}

pub struct TDLibBackend {
    inner: Arc<TDLibInner>,
    event_tx: Arc<Mutex<Option<broadcast::Sender<CoreEvent>>>>,
}

impl TDLibBackend {
    pub fn parse_file(v: &Value) -> Option<FileInfo> {
        TDLibInner::parse_file(v)
    }

    pub fn parse_message_content(v: &Value) -> MessageContent {
        TDLibInner::parse_message_content(v)
    }

    pub fn new(config: Config) -> Result<Self> {
        let raw = TdJsonRaw::load_auto(None)?;
        let client_id = raw.create_client_id();

        // Suppress noisy TDLib debug outputs on stderr
        raw.execute(r#"{"@type":"setLogVerbosityLevel","new_verbosity_level":1}"#);

        Ok(Self {
            inner: Arc::new(TDLibInner {
                raw,
                client_id,
                config,
                is_running: AtomicBool::new(false),
                router: Arc::new(RequestRouter::new()),
            }),
            event_tx: Arc::new(Mutex::new(None)),
        })
    }
}


impl TelegramBackend for TDLibBackend {
    fn start(&self, event_tx: broadcast::Sender<CoreEvent>) -> BoxFuture<'_, ()> {
        self.inner.is_running.store(true, Ordering::SeqCst);
        let inner = self.inner.clone();
        let tx = event_tx.clone();

        // Spawn OS thread for polling td_receive
        std::thread::spawn(move || {
            while inner.is_running.load(Ordering::SeqCst) {
                if let Some(res) = inner.raw.receive(1.0) {
                    if let Ok(val) = serde_json::from_str::<Value>(&res) {
                        // 1. Multiplex through RequestRouter
                        if let Some(unhandled_update) = inner.router.dispatch(val) {
                            // 2. Dispatch spontaneous updates
                            inner.handle_update(unhandled_update, &tx);
                        }
                    }
                }
            }
        });

        Box::pin(async move {
            *self.event_tx.lock().await = Some(event_tx);
            let _ = self.inner.send_json(json!({ "@type": "getAuthorizationState" }));
            Ok(())
        })
    }

    fn send_phone_number(&self, phone: &str) -> BoxFuture<'_, ()> {
        let req = json!({
            "@type": "setAuthenticationPhoneNumber",
            "phone_number": phone
        });
        let inner = self.inner.clone();
        Box::pin(async move { inner.send_json(req) })
    }

    fn send_code(&self, code: &str) -> BoxFuture<'_, ()> {
        let req = json!({
            "@type": "checkAuthenticationCode",
            "code": code
        });
        let inner = self.inner.clone();
        Box::pin(async move { inner.send_json(req) })
    }

    fn send_password(&self, password: &str) -> BoxFuture<'_, ()> {
        let req = json!({
            "@type": "checkAuthenticationPassword",
            "password": password
        });
        let inner = self.inner.clone();
        Box::pin(async move { inner.send_json(req) })
    }

    fn request_qr_code(&self) -> BoxFuture<'_, ()> {
        let req = json!({
            "@type": "requestQrCodeAuthentication",
            "other_user_ids": []
        });
        let inner = self.inner.clone();
        Box::pin(async move { inner.send_json(req) })
    }

    fn send_text_message(&self, chat_id: ChatId, text: &str, reply_to: Option<MessageId>) -> BoxFuture<'_, ()> {
        let mut req = json!({
            "@type": "sendMessage",
            "chat_id": chat_id.0,
            "input_message_content": {
                "@type": "inputMessageText",
                "text": {
                    "@type": "formattedText",
                    "text": text
                }
            }
        });

        if let Some(reply) = reply_to {
            req["reply_to"] = json!({
                "@type": "inputMessageReplyToMessage",
                "message_id": reply.0
            });
        }

        let inner = self.inner.clone();
        Box::pin(async move { inner.send_json(req) })
    }

    fn edit_message_text(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        text: &str,
    ) -> BoxFuture<'_, ()> {
        let req = json!({
            "@type": "editMessageText",
            "chat_id": chat_id.0,
            "message_id": message_id.0,
            "input_message_content": {
                "@type": "inputMessageText",
                "text": {
                    "@type": "formattedText",
                    "text": text
                }
            }
        });
        let inner = self.inner.clone();
        Box::pin(async move { inner.send_request(req).await.map(|_| ()) })
    }

    fn delete_messages(
        &self,
        chat_id: ChatId,
        message_ids: Vec<MessageId>,
        revoke: bool,
    ) -> BoxFuture<'_, ()> {
        let ids: Vec<i64> = message_ids.into_iter().map(|m| m.0).collect();
        let req = json!({
            "@type": "deleteMessages",
            "chat_id": chat_id.0,
            "message_ids": ids,
            "revoke": revoke
        });
        let inner = self.inner.clone();
        Box::pin(async move { inner.send_request(req).await.map(|_| ()) })
    }

    fn load_history(&self, chat_id: ChatId, from_msg_id: Option<MessageId>, limit: i32) -> BoxFuture<'_, Vec<Message>> {
        let offset = if limit < 0 { limit } else { 0 };
        let req_limit = limit.abs();
        let from_id = from_msg_id.map(|m| m.0).unwrap_or(0);
        let req = json!({
            "@type": "getChatHistory",
            "chat_id": chat_id.0,
            "from_message_id": from_id,
            "offset": offset,
            "limit": req_limit,
            "only_local": false
        });
        let inner = self.inner.clone();
        Box::pin(async move {
            let res = inner.send_request(req).await?;
            let mut messages = Vec::new();
            if let Some(msg_array) = res.get("messages").and_then(|m| m.as_array()) {
                for m in msg_array {
                    if let Some(parsed) = inner.parse_message(m) {
                        messages.push(parsed);
                    }
                }
            }
            Ok(messages)
        })
    }

    fn download_file(&self, file_id: i32, priority: i32) -> BoxFuture<'_, ()> {
        let req = json!({
            "@type": "downloadFile",
            "file_id": file_id,
            "priority": priority,
            "offset": 0,
            "limit": 0,
            "synchronous": false
        });
        let inner = self.inner.clone();
        Box::pin(async move {
            inner.send_request(req).await.map(|_| ())
        })
    }

    fn cancel_download_file(&self, file_id: i32) -> BoxFuture<'_, ()> {
        let req = json!({
            "@type": "cancelDownloadFile",
            "file_id": file_id,
            "only_if_pending": false
        });
        let inner = self.inner.clone();
        Box::pin(async move {
            inner.send_request(req).await.map(|_| ())
        })
    }

    fn set_proxy(&self, proxy: Option<ProxyConfig>) -> BoxFuture<'_, ()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            match proxy {
                Some(p) => {
                    let proxy_type = match p.proxy_type {
                        ProxyType::Socks5 => json!({
                            "@type": "proxyTypeSocks5",
                            "username": p.username.unwrap_or_default(),
                            "password": p.password.unwrap_or_default()
                        }),
                        ProxyType::Http => json!({
                            "@type": "proxyTypeHttp",
                            "username": p.username.unwrap_or_default(),
                            "password": p.password.unwrap_or_default(),
                            "http_only": false
                        }),
                        ProxyType::Mtproto { secret } => json!({
                            "@type": "proxyTypeMtproto",
                            "secret": secret
                        }),
                    };
                    let add_req = json!({
                        "@type": "addProxy",
                        "server": p.server,
                        "port": p.port,
                        "enable": p.is_enabled,
                        "type": proxy_type
                    });
                    inner.send_request(add_req).await.map(|_| ())?;
                    if !p.is_enabled {
                        inner.send_request(json!({ "@type": "disableProxy" })).await.map(|_| ())?;
                    }
                    Ok(())
                }
                None => {
                    inner.send_request(json!({ "@type": "disableProxy" })).await.map(|_| ())
                }
            }
        })
    }

    fn log_out(&self) -> BoxFuture<'_, ()> {
        let req = json!({ "@type": "logOut" });
        let inner = self.inner.clone();
        Box::pin(async move { inner.send_json(req) })
    }

    fn stop(&self) -> BoxFuture<'_, ()> {
        self.inner.is_running.store(false, Ordering::SeqCst);
        self.inner.router.abort_all("TDLibBackend stopped");
        let req = json!({ "@type": "close" });
        let inner = self.inner.clone();
        Box::pin(async move { inner.send_json(req) })
    }
}
