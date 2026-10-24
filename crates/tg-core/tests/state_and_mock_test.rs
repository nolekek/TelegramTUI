use tg_core::{
    AuthState, Chat, ChatId, ChatPosition, ChatType, Config, Engine, MessageContent,
};

#[test]
fn test_core_state_chat_ordering() {
    let mut state = tg_core::CoreState::new();

    let chat1 = Chat {
        id: ChatId(1),
        title: "Regular Chat Low".to_string(),
        chat_type: ChatType::Private,
        unread_count: 0,
        last_read_inbox_message_id: 0,
        last_read_outbox_message_id: 0,
        last_message: None,
        position: Some(ChatPosition { order: 10, is_pinned: false }),
        is_muted: false,
        has_unread_mention: false,
        folder_ids: Vec::new(),
    };

    let chat2 = Chat {
        id: ChatId(2),
        title: "Regular Chat High".to_string(),
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

    let chat3 = Chat {
        id: ChatId(3),
        title: "Pinned Chat".to_string(),
        chat_type: ChatType::Private,
        unread_count: 0,
        last_read_inbox_message_id: 0,
        last_read_outbox_message_id: 0,
        last_message: None,
        position: Some(ChatPosition { order: 5, is_pinned: true }),
        is_muted: false,
        has_unread_mention: false,
        folder_ids: Vec::new(),
    };

    state.insert_or_update_chat(chat1);
    state.insert_or_update_chat(chat2);
    state.insert_or_update_chat(chat3);

    // Pinned chat must be first, then order 100, then order 10
    assert_eq!(state.ordered_chats, vec![ChatId(3), ChatId(2), ChatId(1)]);
}

#[tokio::test]
async fn test_mock_backend_auth_flow() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, _event_rx) = Engine::start(config).await.expect("Failed to start engine");

    // Give the background task a tiny moment to initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check initial state
    assert_eq!(handle.read_state().auth_state, AuthState::WaitPhoneNumber);

    // Send phone number
    handle.send_phone_number("+1234567890").await.expect("Failed to send phone");
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    match handle.read_state().auth_state {
        AuthState::WaitCode(ref info) => {
            assert_eq!(info.phone_number, "+1234567890");
        }
        ref other => panic!("Expected WaitCode, got {:?}", other),
    }

    // Send correct mock code "11111"
    handle.send_code("11111").await.expect("Failed to send code");
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    // Must be in Ready state
    assert_eq!(handle.read_state().auth_state, AuthState::Ready);
    assert!(!handle.read_state().ordered_chats.is_empty());

    // Send a message to the first chat
    let first_chat_id = handle.read_state().ordered_chats[0];
    handle.send_text_message(first_chat_id, "Hello from Rust test!", None)
        .await
        .expect("Failed to send text message");

    // Wait for the message + mock echo reply (mock replies after 1000ms)
    tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;

    let msgs = handle.read_state().get_messages(first_chat_id).cloned().unwrap();
    assert!(msgs.len() >= 2);
    let last_msg = msgs.last().unwrap();
    match &last_msg.content {
        MessageContent::Text { text, .. } => {
            assert!(text.contains("Mock auto-reply to: \"Hello from Rust test!\""));
        }
        _ => panic!("Expected text message"),
    }
}
