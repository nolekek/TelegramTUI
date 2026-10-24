use std::time::Duration;
use tg_core::{
    AuthState, Chat, ChatId, ChatPosition, ChatType, Config, ConnectionState, CoreEvent,
    CoreState, Engine, Message, MessageContent, MessageId, MessageStatus, ProxyConfig,
    ProxyType, Sender, UserId,
};

/// Helper to wait for a condition on CoreState
async fn wait_for_state<F>(handle: &tg_core::CoreHandle, predicate: F, timeout_dur: Duration) -> bool
where
    F: Fn(&CoreState) -> bool,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout_dur {
        if predicate(&handle.read_state()) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    predicate(&handle.read_state())
}

#[tokio::test]
async fn test_outgoing_message_edit_and_delete() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, mut event_rx) = Engine::start(config).await.expect("Failed to start engine");

    // Authenticate
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.send_phone_number("+1234567890").await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.send_code("11111").await.unwrap();

    let ready = wait_for_state(&handle, |s| s.auth_state == AuthState::Ready, Duration::from_secs(2)).await;
    assert!(ready, "Engine failed to transition to Ready");

    let chat_id = ChatId(1);

    // 1. Send an outgoing message
    handle
        .send_text_message(chat_id, "Original text message", None)
        .await
        .expect("Failed to send message");

    let msg_sent = wait_for_state(
        &handle,
        |s| {
            s.get_messages(chat_id)
                .map(|ms| ms.iter().any(|m| match &m.content {
                    MessageContent::Text { text, .. } => text == "Original text message",
                    _ => false,
                }))
                .unwrap_or(false)
        },
        Duration::from_secs(2),
    )
    .await;
    assert!(msg_sent, "Message not found in state");

    let msg_id = {
        let state = handle.read_state();
        let msgs = state.get_messages(chat_id).unwrap();
        msgs.iter()
            .find(|m| match &m.content {
                MessageContent::Text { text, .. } => text == "Original text message",
                _ => false,
            })
            .unwrap()
            .id
    };

    // 2. Edit message text
    handle
        .edit_message_text(chat_id, msg_id, "Edited text message")
        .await
        .expect("Failed to edit message");

    let msg_edited = wait_for_state(
        &handle,
        |s| {
            s.get_messages(chat_id)
                .and_then(|ms| ms.iter().find(|m| m.id == msg_id))
                .map(|m| match &m.content {
                    MessageContent::Text { text, .. } => text == "Edited text message",
                    _ => false,
                })
                .unwrap_or(false)
        },
        Duration::from_secs(2),
    )
    .await;
    assert!(msg_edited, "Message content was not updated to edited text");

    // 3. Delete message
    handle
        .delete_messages(chat_id, vec![msg_id], true)
        .await
        .expect("Failed to delete messages");

    let msg_deleted = wait_for_state(
        &handle,
        |s| {
            s.get_messages(chat_id)
                .map(|ms| !ms.iter().any(|m| m.id == msg_id))
                .unwrap_or(true)
        },
        Duration::from_secs(2),
    )
    .await;
    assert!(msg_deleted, "Message was not removed from state after deletion");

    handle.stop().await.expect("Failed to stop engine");
    let _ = event_rx.try_recv();
}

#[tokio::test]
async fn test_bidirectional_history_pagination() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, _rx) = Engine::start(config).await.expect("Failed to start engine");

    // Authenticate
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.send_phone_number("+1234567890").await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.send_code("11111").await.unwrap();

    wait_for_state(&handle, |s| s.auth_state == AuthState::Ready, Duration::from_secs(2)).await;

    let chat_id = ChatId(1);

    // Initial message count is 1 (the chat's last_message)
    let initial_count = handle
        .read_state()
        .get_messages(chat_id)
        .map(|m| m.len())
        .unwrap_or(0);
    assert_eq!(initial_count, 1, "Expected 1 initial message from last_message");

    // 1. Older history pagination (backward: loads 5 older messages)
    handle
        .load_history_older(chat_id, None, 5)
        .await
        .expect("Failed to load older history");

    let loaded_older = wait_for_state(
        &handle,
        |s| s.get_messages(chat_id).map(|ms| ms.len()).unwrap_or(0) > 1,
        Duration::from_secs(2),
    )
    .await;
    assert!(loaded_older, "Older history was not populated into state");

    let msgs = handle.read_state().get_messages(chat_id).cloned().unwrap();
    assert!(msgs.len() >= 5);
    // Messages must stay chronologically sorted
    for i in 1..msgs.len() {
        assert!(msgs[i].date >= msgs[i - 1].date);
    }

    // 2. Newer history pagination (forward: negative limit via helper)
    let first_msg_id = msgs[0].id;
    handle
        .load_history_newer(chat_id, first_msg_id, 3)
        .await
        .expect("Failed to load newer history");

    tokio::time::sleep(Duration::from_millis(50)).await;
    let msgs_after = handle.read_state().get_messages(chat_id).cloned().unwrap();
    assert!(msgs_after.len() >= msgs.len());

    handle.stop().await.expect("Failed to stop engine");
}

#[tokio::test]
async fn test_proxy_startup_and_toggling() {
    let proxy = ProxyConfig {
        server: "127.0.0.1".to_string(),
        port: 9050,
        proxy_type: ProxyType::Socks5,
        username: None,
        password: None,
        is_enabled: true,
    };

    let config = Config {
        use_mock: true,
        proxy: Some(proxy),
        ..Default::default()
    };

    let (handle, mut event_rx) = Engine::start(config).await.expect("Failed to start engine");

    // Monitor events for proxy connection states
    let mut saw_proxy_connecting = false;
    let mut saw_proxy_ready = false;

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        if let Ok(event) = event_rx.recv().await {
            match event {
                CoreEvent::ConnectionStateChanged(ConnectionState::ConnectingToProxy) => {
                    saw_proxy_connecting = true;
                }
                CoreEvent::ConnectionStateChanged(ConnectionState::Ready) => {
                    saw_proxy_ready = true;
                }
                _ => {}
            }
            if saw_proxy_connecting && saw_proxy_ready {
                break;
            }
        }
    }

    assert!(saw_proxy_connecting, "ConnectingToProxy event was not observed on startup");
    assert!(saw_proxy_ready, "Ready connection state was not observed after proxy connect");

    // Dynamically disable proxy
    handle.set_proxy(None).await.expect("Failed to clear proxy");
    tokio::time::sleep(Duration::from_millis(50)).await;

    handle.stop().await.expect("Failed to stop engine");
}

#[tokio::test]
async fn test_auth_error_reporting() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, mut event_rx) = Engine::start(config).await.expect("Failed to start engine");

    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.send_phone_number("+1234567890").await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send error-triggering invalid code "00000"
    handle.send_code("00000").await.unwrap();

    let mut saw_code_error = false;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Ok(event) = event_rx.recv().await {
            if let CoreEvent::ErrorOccurred(err) = event {
                if err.contains("PHONE_CODE_INVALID") {
                    saw_code_error = true;
                    break;
                }
            }
        }
    }
    assert!(saw_code_error, "ErrorOccurred event for PHONE_CODE_INVALID was not received");

    // Now test 2FA cloud password error
    handle.send_code("22222").await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.send_password("wrong").await.unwrap();

    let mut saw_pwd_error = false;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Ok(event) = event_rx.recv().await {
            if let CoreEvent::ErrorOccurred(err) = event {
                if err.contains("PASSWORD_HASH_INVALID") {
                    saw_pwd_error = true;
                    break;
                }
            }
        }
    }
    assert!(saw_pwd_error, "ErrorOccurred event for PASSWORD_HASH_INVALID was not received");

    handle.stop().await.expect("Failed to stop engine");
}

#[test]
fn test_order_zero_removal_and_delivery_receipts() {
    let mut state = CoreState::new();

    let c1 = Chat {
        id: ChatId(1),
        title: "Chat 1".to_string(),
        chat_type: ChatType::Private,
        unread_count: 5,
        last_read_inbox_message_id: 10,
        last_read_outbox_message_id: 10,
        last_message: None,
        position: Some(ChatPosition { order: 500, is_pinned: false }),
        is_muted: false,
        has_unread_mention: false,
        folder_ids: Vec::new(),
    };
    let c2 = Chat {
        id: ChatId(2),
        title: "Chat 2".to_string(),
        chat_type: ChatType::Private,
        unread_count: 0,
        last_read_inbox_message_id: 0,
        last_read_outbox_message_id: 0,
        last_message: None,
        position: Some(ChatPosition { order: 100, is_pinned: false }),
        is_muted: false,
        has_unread_mention: false,
        folder_ids: Vec::new(),
    };

    state.insert_or_update_chat(c1);
    state.insert_or_update_chat(c2);
    assert_eq!(state.ordered_chats, vec![ChatId(1), ChatId(2)]);

    // 1. Set Chat 1 order to 0 with is_pinned = false -> Chat 1 MUST be removed
    state.update_chat_position(ChatId(1), Some(ChatPosition { order: 0, is_pinned: false }));
    assert_eq!(state.ordered_chats, vec![ChatId(2)], "Chat with order 0 unpinned must be excluded");

    // 2. Set Chat 1 order to 0 with is_pinned = true -> Chat 1 MUST be restored as pinned!
    state.update_chat_position(ChatId(1), Some(ChatPosition { order: 0, is_pinned: true }));
    assert_eq!(state.ordered_chats, vec![ChatId(1), ChatId(2)], "Pinned chat with order 0 must remain in list");

    // 3. Test Message Delivery Receipts
    let outgoing_msg = Message {
        id: MessageId(100),
        chat_id: ChatId(2),
        sender: Sender::User(UserId(777000)),
        date: 1000,
        content: MessageContent::Text { text: "Hello".to_string(), entities: Vec::new() },
        is_outgoing: true,
        status: MessageStatus::Sending,
        reply_to_message_id: None,
        can_be_edited: true,
        can_be_deleted: true,
    };
    state.add_message(outgoing_msg);

    // Message is Sending
    assert_eq!(state.get_messages(ChatId(2)).unwrap()[0].status, MessageStatus::Sending);

    // Send succeeded: replace temporary id with permanent id 105
    let confirmed_msg = Message {
        id: MessageId(105),
        chat_id: ChatId(2),
        sender: Sender::User(UserId(777000)),
        date: 1000,
        content: MessageContent::Text { text: "Hello".to_string(), entities: Vec::new() },
        is_outgoing: true,
        status: MessageStatus::Sending,
        reply_to_message_id: None,
        can_be_edited: true,
        can_be_deleted: true,
    };
    state.update_message_send_succeeded(ChatId(2), MessageId(100), confirmed_msg);
    assert_eq!(state.get_messages(ChatId(2)).unwrap()[0].id, MessageId(105));
    assert_eq!(state.get_messages(ChatId(2)).unwrap()[0].status, MessageStatus::Sent);

    // Outbox read: mark up to message 105 as Read
    state.update_chat_read_outbox(ChatId(2), 105);
    assert_eq!(state.get_messages(ChatId(2)).unwrap()[0].status, MessageStatus::Read);

    // Inbox read: update last_read_inbox and decrease unread count
    state.update_chat_read_inbox(ChatId(1), 20, 1);
    assert_eq!(state.get_chat(ChatId(1)).unwrap().unread_count, 1);
    assert_eq!(state.get_chat(ChatId(1)).unwrap().last_read_inbox_message_id, 20);
}
