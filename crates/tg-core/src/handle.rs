use std::sync::Arc;
use parking_lot::{RwLock, RwLockReadGuard};
use tokio::sync::mpsc;

use crate::domain::*;
use crate::error::{CoreError, Result};
use crate::state::CoreState;

#[derive(Debug)]
pub enum CoreCommand {
    SendPhoneNumber(String),
    SendCode(String),
    SendPassword(String),
    RequestQrCode,
    SendTextMessage {
        chat_id: ChatId,
        text: String,
        reply_to: Option<MessageId>,
    },
    EditMessageText {
        chat_id: ChatId,
        message_id: MessageId,
        text: String,
    },
    DeleteMessages {
        chat_id: ChatId,
        message_ids: Vec<MessageId>,
        revoke: bool,
    },
    LoadHistory {
        chat_id: ChatId,
        from_msg_id: Option<MessageId>,
        limit: i32,
    },
    DownloadFile {
        file_id: i32,
        priority: i32,
    },
    CancelDownloadFile {
        file_id: i32,
    },
    SetProxy(Option<ProxyConfig>),
    LogOut,
    Stop,
}


#[derive(Clone)]
pub struct CoreHandle {
    command_tx: mpsc::Sender<CoreCommand>,
    state: Arc<RwLock<CoreState>>,
}

impl CoreHandle {
    pub(crate) fn new(command_tx: mpsc::Sender<CoreCommand>, state: Arc<RwLock<CoreState>>) -> Self {
        Self { command_tx, state }
    }

    /// Read-only snapshot of current normalized state without async overhead
    pub fn read_state(&self) -> RwLockReadGuard<'_, CoreState> {
        self.state.read()
    }

    pub fn clone_state(&self) -> CoreState {
        self.state.read().clone()
    }

    pub async fn send_phone_number(&self, phone: impl Into<String>) -> Result<()> {
        self.send(CoreCommand::SendPhoneNumber(phone.into())).await
    }

    pub async fn send_code(&self, code: impl Into<String>) -> Result<()> {
        self.send(CoreCommand::SendCode(code.into())).await
    }

    pub async fn send_password(&self, password: impl Into<String>) -> Result<()> {
        self.send(CoreCommand::SendPassword(password.into())).await
    }

    pub async fn request_qr_code(&self) -> Result<()> {
        self.send(CoreCommand::RequestQrCode).await
    }

    pub async fn send_text_message(
        &self,
        chat_id: ChatId,
        text: impl Into<String>,
        reply_to: Option<MessageId>,
    ) -> Result<()> {
        self.send(CoreCommand::SendTextMessage {
            chat_id,
            text: text.into(),
            reply_to,
        })
        .await
    }

    pub async fn edit_message_text(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        text: impl Into<String>,
    ) -> Result<()> {
        self.send(CoreCommand::EditMessageText {
            chat_id,
            message_id,
            text: text.into(),
        })
        .await
    }

    pub async fn delete_messages(
        &self,
        chat_id: ChatId,
        message_ids: Vec<MessageId>,
        revoke: bool,
    ) -> Result<()> {
        self.send(CoreCommand::DeleteMessages {
            chat_id,
            message_ids,
            revoke,
        })
        .await
    }

    pub async fn load_history(
        &self,
        chat_id: ChatId,
        from_msg_id: Option<MessageId>,
        limit: i32,
    ) -> Result<()> {
        self.send(CoreCommand::LoadHistory {
            chat_id,
            from_msg_id,
            limit,
        })
        .await
    }

    pub async fn load_history_older(
        &self,
        chat_id: ChatId,
        from_msg_id: Option<MessageId>,
        limit: i32,
    ) -> Result<()> {
        self.send(CoreCommand::LoadHistory {
            chat_id,
            from_msg_id,
            limit: limit.abs(),
        })
        .await
    }

    pub async fn load_history_newer(
        &self,
        chat_id: ChatId,
        from_msg_id: MessageId,
        limit: i32,
    ) -> Result<()> {
        self.send(CoreCommand::LoadHistory {
            chat_id,
            from_msg_id: Some(from_msg_id),
            limit: -limit.abs(),
        })
        .await
    }

    pub async fn download_file(&self, file_id: i32, priority: i32) -> Result<()> {
        self.send(CoreCommand::DownloadFile { file_id, priority }).await
    }

    pub async fn cancel_download_file(&self, file_id: i32) -> Result<()> {
        self.send(CoreCommand::CancelDownloadFile { file_id }).await
    }

    pub async fn set_proxy(&self, proxy: Option<ProxyConfig>) -> Result<()> {
        self.send(CoreCommand::SetProxy(proxy)).await
    }


    pub async fn log_out(&self) -> Result<()> {
        self.send(CoreCommand::LogOut).await
    }

    pub async fn stop(&self) -> Result<()> {
        self.send(CoreCommand::Stop).await
    }

    async fn send(&self, cmd: CoreCommand) -> Result<()> {
        self.command_tx
            .send(cmd)
            .await
            .map_err(|_| CoreError::Internal("Core worker thread channel closed".to_string()))
    }
}
