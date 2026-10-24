use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::{broadcast, mpsc};

use crate::backend::mock::MockBackend;
use crate::backend::tdlib::TDLibBackend;
use crate::backend::TelegramBackend;
use crate::config::Config;
use crate::domain::ProxyConfig;
use crate::error::Result;
use crate::event::CoreEvent;
use crate::handle::{CoreCommand, CoreHandle};
use crate::state::CoreState;

pub struct Engine {
    state: Arc<RwLock<CoreState>>,
    event_tx: broadcast::Sender<CoreEvent>,
    command_rx: mpsc::Receiver<CoreCommand>,
    backend: Box<dyn TelegramBackend>,
    initial_proxy: Option<ProxyConfig>,
}

impl Engine {
    pub async fn start(config: Config) -> Result<(CoreHandle, broadcast::Receiver<CoreEvent>)> {
        let _ = config.ensure_directories();
        let (command_tx, command_rx) = mpsc::channel(128);
        let (event_tx, event_rx) = broadcast::channel(512);

        let state = Arc::new(RwLock::new(CoreState::new()));
        let initial_proxy = config.proxy.clone();

        let backend: Box<dyn TelegramBackend> = if config.use_mock {
            tracing::info!("Initializing engine with MockBackend (mock mode explicitly enabled)");
            Box::new(MockBackend::new())
        } else {
            let b = TDLibBackend::new(config.clone())?;
            tracing::info!("Initializing engine with TDLibBackend (real Telegram connection)");
            Box::new(b)
        };

        let handle = CoreHandle::new(command_tx, state.clone());

        let engine = Self {
            state,
            event_tx: event_tx.clone(),
            command_rx,
            backend,
            initial_proxy,
        };

        // Launch background event & command loop
        tokio::spawn(async move {
            engine.run().await;
        });

        Ok((handle, event_rx))
    }

    async fn run(mut self) {
        let mut event_sub = self.event_tx.subscribe();
        let state = self.state.clone();

        // Spawn state synchronizer loop before starting backend so initial events are not missed
        tokio::spawn(async move {
            loop {
                match event_sub.recv().await {
                    Ok(event) => {
                        let mut guard = state.write();
                        guard.apply_update(&event);
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(
                            "State synchronizer event stream lagged by {} events; continuing",
                            skipped
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });

        // Start backend
        if let Err(e) = self.backend.start(self.event_tx.clone()).await {
            tracing::error!("Backend start error: {}", e);
            let _ = self.event_tx.send(CoreEvent::ErrorOccurred(e.to_string()));
            return;
        }

        // Apply initial proxy configuration if present
        if let Some(proxy) = self.initial_proxy {
            tracing::info!(
                "Applying initial proxy from configuration: {}:{} (type: {:?}, enabled: {})",
                proxy.server,
                proxy.port,
                proxy.proxy_type,
                proxy.is_enabled
            );
            if let Err(e) = self.backend.set_proxy(Some(proxy)).await {
                tracing::error!("Failed to apply initial proxy: {}", e);
                let _ = self
                    .event_tx
                    .send(CoreEvent::ErrorOccurred(format!("Proxy startup failed: {}", e)));
            }
        }

        // Command handler loop
        while let Some(cmd) = self.command_rx.recv().await {
            let res = match cmd {
                CoreCommand::SendPhoneNumber(phone) => self.backend.send_phone_number(&phone).await,
                CoreCommand::SendCode(code) => self.backend.send_code(&code).await,
                CoreCommand::SendPassword(pwd) => self.backend.send_password(&pwd).await,
                CoreCommand::RequestQrCode => self.backend.request_qr_code().await,
                CoreCommand::SendTextMessage {
                    chat_id,
                    text,
                    reply_to,
                } => self.backend.send_text_message(chat_id, &text, reply_to).await,
                CoreCommand::EditMessageText {
                    chat_id,
                    message_id,
                    text,
                } => self.backend.edit_message_text(chat_id, message_id, &text).await,
                CoreCommand::DeleteMessages {
                    chat_id,
                    message_ids,
                    revoke,
                } => self.backend.delete_messages(chat_id, message_ids, revoke).await,
                CoreCommand::LoadHistory {
                    chat_id,
                    from_msg_id,
                    limit,
                } => match self.backend.load_history(chat_id, from_msg_id, limit).await {
                    Ok(msgs) => {
                        let count = msgs.len();
                        let mut guard = self.state.write();
                        guard.insert_history_messages(chat_id, msgs);
                        drop(guard);
                        let _ = self.event_tx.send(CoreEvent::HistoryLoaded { chat_id, count });
                        Ok(())
                    }
                    Err(e) => Err(e),
                },
                CoreCommand::DownloadFile { file_id, priority } => {
                    self.backend.download_file(file_id, priority).await
                }
                CoreCommand::CancelDownloadFile { file_id } => {
                    self.backend.cancel_download_file(file_id).await
                }
                CoreCommand::SetProxy(proxy) => self.backend.set_proxy(proxy).await,

                CoreCommand::LogOut => self.backend.log_out().await,
                CoreCommand::Stop => {
                    let _ = self.backend.stop().await;
                    break;
                }
            };

            if let Err(e) = res {
                tracing::error!("Command execution error: {}", e);
                let _ = self.event_tx.send(CoreEvent::ErrorOccurred(e.to_string()));
            }
        }
    }
}
