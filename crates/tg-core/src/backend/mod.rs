use std::future::Future;
use std::pin::Pin;
use tokio::sync::broadcast;

use crate::domain::*;
use crate::error::Result;
use crate::event::CoreEvent;

pub mod mock;
pub mod tdlib;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// Abstraction layer for interacting with Telegram API.
/// Object-safe so it can be dynamically swapped (e.g. MockBackend vs TDLibBackend).
pub trait TelegramBackend: Send + Sync {
    fn start(&self, event_tx: broadcast::Sender<CoreEvent>) -> BoxFuture<'_, ()>;
    fn send_phone_number(&self, phone: &str) -> BoxFuture<'_, ()>;
    fn send_code(&self, code: &str) -> BoxFuture<'_, ()>;
    fn send_password(&self, password: &str) -> BoxFuture<'_, ()>;
    fn request_qr_code(&self) -> BoxFuture<'_, ()>;
    fn send_text_message(&self, chat_id: ChatId, text: &str, reply_to: Option<MessageId>) -> BoxFuture<'_, ()>;
    fn edit_message_text(&self, chat_id: ChatId, message_id: MessageId, text: &str) -> BoxFuture<'_, ()>;
    fn delete_messages(&self, chat_id: ChatId, message_ids: Vec<MessageId>, revoke: bool) -> BoxFuture<'_, ()>;
    fn load_history(&self, chat_id: ChatId, from_msg_id: Option<MessageId>, limit: i32) -> BoxFuture<'_, Vec<Message>>;
    fn download_file(&self, file_id: i32, priority: i32) -> BoxFuture<'_, ()>;
    fn cancel_download_file(&self, file_id: i32) -> BoxFuture<'_, ()>;
    fn set_proxy(&self, proxy: Option<ProxyConfig>) -> BoxFuture<'_, ()>;
    fn log_out(&self) -> BoxFuture<'_, ()>;
    fn stop(&self) -> BoxFuture<'_, ()>;
}

