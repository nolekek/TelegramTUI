use std::time::Duration;
use tg_core::{
    AuthState, Chat, ChatId, ChatPosition, ChatType, Config, CoreState, Engine,
    Message, MessageContent, MessageId, MessageStatus, Sender, UserId,
};

/// Helper function to create a test chat
fn make_test_chat(
    id: i64,
    title: &str,
    chat_type: ChatType,
    order: u64,
    is_pinned: bool,
    unread_count: i32,
    is_muted: bool,
) -> Chat {
    Chat {
        id: ChatId(id),
        title: title.to_string(),
        chat_type,
        unread_count,
        last_read_inbox_message_id: 0,
        last_read_outbox_message_id: 0,
        last_message: None,
        position: Some(ChatPosition { order, is_pinned }),
        is_muted,
        has_unread_mention: false,
        folder_ids: Vec::new(),
    }
}

/// Helper function to create a test message
fn make_test_message(id: i64, chat_id: i64, date: i64, text: &str) -> Message {
    Message {
        id: MessageId(id),
        chat_id: ChatId(chat_id),
        sender: Sender::User(UserId(100)),
        date,
        content: MessageContent::Text {
            text: text.to_string(),
            entities: Vec::new(),
        },
        is_outgoing: false,
        status: MessageStatus::Read,
        reply_to_message_id: None,
        can_be_edited: false,
        can_be_deleted: false,
    }
}

#[test]
fn test_chat_ordering_pin_precedence_and_order_descending() {
    let mut state = CoreState::new();

    // Setup a variety of chats with distinct orders:
    // Pinned: c3 (order 500) > c2 (order 100)
    // Unpinned: c4 (order 50,000) > c1 (order 10,000) > c5 (order 10) > c6 (position None -> order 0)
    let c1 = make_test_chat(1, "Unpinned High Order", ChatType::Private, 10_000, false, 0, false);
    let c2 = make_test_chat(2, "Pinned Low Order", ChatType::Private, 100, true, 0, false);
    let c3 = make_test_chat(3, "Pinned High Order", ChatType::Private, 500, true, 0, false);
    let c4 = make_test_chat(4, "Unpinned Max Order", ChatType::Private, 50_000, false, 0, false);
    let c5 = make_test_chat(5, "Unpinned Low Order", ChatType::Private, 10, false, 0, false);

    let mut c6 = make_test_chat(6, "No Position Chat", ChatType::Private, 0, false, 0, false);
    c6.position = None;

    state.insert_or_update_chat(c1);
    state.insert_or_update_chat(c2);
    state.insert_or_update_chat(c3);
    state.insert_or_update_chat(c4);
    state.insert_or_update_chat(c5);
    state.insert_or_update_chat(c6);

    // Expected order:
    // 1. Pinned high order: c3 (order 500)
    // 2. Pinned low order: c2 (order 100)
    // 3. Unpinned max order: c4 (order 50,000)
    // 4. Unpinned high order: c1 (order 10,000)
    // 5. Unpinned low order: c5 (order 10)
    // 6. No position chat: c6 (order 0, not pinned)
    assert_eq!(
        state.ordered_chats,
        vec![
            ChatId(3),
            ChatId(2),
            ChatId(4),
            ChatId(1),
            ChatId(5),
            ChatId(6),
        ]
    );
}


#[test]
fn test_dynamic_pin_unpin_and_reorder() {
    let mut state = CoreState::new();

    let ca = make_test_chat(10, "Chat A", ChatType::Private, 100, true, 0, false);
    let cb = make_test_chat(20, "Chat B", ChatType::Supergroup, 200, false, 0, false);
    let cc = make_test_chat(30, "Chat C", ChatType::Channel, 50, false, 0, false);

    state.insert_or_update_chat(ca);
    state.insert_or_update_chat(cb);
    state.insert_or_update_chat(cc);

    // Initial: A is pinned (100), B is unpinned (200), C is unpinned (50)
    assert_eq!(state.ordered_chats, vec![ChatId(10), ChatId(20), ChatId(30)]);

    // Step 1: Pin Chat B as well (order 200 > Chat A order 100)
    let mut cb_updated = state.get_chat(ChatId(20)).unwrap().clone();
    cb_updated.position = Some(ChatPosition { order: 200, is_pinned: true });
    state.insert_or_update_chat(cb_updated);

    // Order: Both pinned -> B (200) > A (100), then C (50 unpinned)
    assert_eq!(state.ordered_chats, vec![ChatId(20), ChatId(10), ChatId(30)]);

    // Step 2: Unpin Chat A
    let mut ca_updated = state.get_chat(ChatId(10)).unwrap().clone();
    ca_updated.position = Some(ChatPosition { order: 100, is_pinned: false });
    state.insert_or_update_chat(ca_updated);

    // Order: B is sole pinned chat -> B, then unpinned by order: A (100) > C (50)
    assert_eq!(state.ordered_chats, vec![ChatId(20), ChatId(10), ChatId(30)]);

    // Step 3: Increase Chat C order to 300
    let mut cc_updated = state.get_chat(ChatId(30)).unwrap().clone();
    cc_updated.position = Some(ChatPosition { order: 300, is_pinned: false });
    state.insert_or_update_chat(cc_updated);

    // Order: B (pinned), then C (300) > A (100)
    assert_eq!(state.ordered_chats, vec![ChatId(20), ChatId(30), ChatId(10)]);
}

#[test]
fn test_unread_badge_math_and_tab_filtering() {
    let mut state = CoreState::new();

    // Create diverse chats across tabs
    // Tab "Personal": Private & Secret
    let c_priv1 = make_test_chat(1, "Alice", ChatType::Private, 100, false, 3, false);
    let c_priv2 = make_test_chat(2, "Bob", ChatType::Private, 90, false, 0, false);
    let c_secret = make_test_chat(3, "Secret Alice", ChatType::Secret, 80, false, 1, false);

    // Tab "Groups": BasicGroup & Supergroup
    let c_group = make_test_chat(4, "Rust Devs", ChatType::BasicGroup, 70, false, 5, false);
    let c_super = make_test_chat(5, "Rust Global", ChatType::Supergroup, 60, false, 10, true); // Muted

    // Tab "Channels": Channel
    let c_chan = make_test_chat(6, "Rust Announcements", ChatType::Channel, 50, false, 7, false);

    state.insert_or_update_chat(c_priv1);
    state.insert_or_update_chat(c_priv2);
    state.insert_or_update_chat(c_secret);
    state.insert_or_update_chat(c_group);
    state.insert_or_update_chat(c_super);
    state.insert_or_update_chat(c_chan);

    // 1. Total unread badge math (All tab)
    let total_unreads: i32 = state.chats.values().map(|c| c.unread_count).sum();
    assert_eq!(total_unreads, 3 + 0 + 1 + 5 + 10 + 7); // 26 total

    // 2. Personal tab badge aggregation (Private + Secret)
    let personal_unreads: i32 = state
        .chats
        .values()
        .filter(|c| matches!(c.chat_type, ChatType::Private | ChatType::Secret))
        .map(|c| c.unread_count)
        .sum();
    assert_eq!(personal_unreads, 3 + 1); // 4

    // 3. Groups tab badge aggregation (BasicGroup + Supergroup)
    let group_unreads: i32 = state
        .chats
        .values()
        .filter(|c| matches!(c.chat_type, ChatType::BasicGroup | ChatType::Supergroup))
        .map(|c| c.unread_count)
        .sum();
    assert_eq!(group_unreads, 5 + 10); // 15

    // 4. Channels tab badge aggregation (Channel)
    let channel_unreads: i32 = state
        .chats
        .values()
        .filter(|c| matches!(c.chat_type, ChatType::Channel))
        .map(|c| c.unread_count)
        .sum();
    assert_eq!(channel_unreads, 7);

    // 5. Unmuted unread count (practical notification badge math)
    let unmuted_unreads: i32 = state
        .chats
        .values()
        .filter(|c| !c.is_muted)
        .map(|c| c.unread_count)
        .sum();
    assert_eq!(unmuted_unreads, 26 - 10); // 16 (excluding c_super which has 10 and is muted)
}

#[tokio::test]
async fn test_mock_chat_order_reactive_synchronization() {
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

    // Wait until Ready state
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if handle.read_state().auth_state == AuthState::Ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Give state synchronizer a brief moment to process initial chats
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Initial mock chats order:
    // Chat 1 (Saved Messages): pinned, order 1000
    // Chat 2 (Rust Community): order 900
    // Chat 3 (Telegram News): order 800
    // Chat 4 (Pavel Durov): order 700
    {
        let state = handle.read_state();
        assert_eq!(
            state.ordered_chats,
            vec![ChatId(1), ChatId(2), ChatId(3), ChatId(4)]
        );
        let chat1 = state.get_chat(ChatId(1)).unwrap();
        assert_eq!(chat1.title, "Saved Messages");
        assert_eq!(chat1.unread_count, 0);

        let chat2 = state.get_chat(ChatId(2)).unwrap();
        assert_eq!(chat2.title, "Rust Community");
        assert_eq!(chat2.unread_count, 2);
    }

    // Now simulate an update: Pin Chat 4 ("Pavel Durov") with order 2000 > 1000
    let mut updated_chat4 = handle.read_state().get_chat(ChatId(4)).unwrap().clone();
    updated_chat4.position = Some(ChatPosition {
        order: 2000,
        is_pinned: true,
    });
    updated_chat4.unread_count = 5;

    // Broadcast ChatUpdated event through the event channel
    // We can simulate an update directly through the state or verify event-driven updates
    // Because CoreState synchronizer listens to CoreEvent::ChatUpdated, let's test CoreState reducer:
    let mut state_clone = handle.clone_state();
    state_clone.insert_or_update_chat(updated_chat4);

    // Chat 4 should now be first, followed by Chat 1, then Chat 2, then Chat 3
    assert_eq!(
        state_clone.ordered_chats,
        vec![ChatId(4), ChatId(1), ChatId(2), ChatId(3)]
    );
    assert_eq!(
        state_clone.get_chat(ChatId(4)).unwrap().unread_count,
        5
    );

    // Drain events to verify clean broadcast shutdown
    handle.stop().await.expect("Failed to stop engine");
    let _ = event_rx.try_recv();
}

#[test]
fn test_chat_message_history_and_last_message_update() {
    let mut state = CoreState::new();

    let chat = make_test_chat(1, "Chat with Messages", ChatType::Private, 100, false, 0, false);
    state.insert_or_update_chat(chat);

    assert!(state.get_chat(ChatId(1)).unwrap().last_message.is_none());

    // Add first message (timestamp 1000)
    let msg1 = make_test_message(101, 1, 1000, "First message");
    state.add_message(msg1.clone());

    assert_eq!(
        state.get_chat(ChatId(1)).unwrap().last_message.as_ref().unwrap().id,
        MessageId(101)
    );
    assert_eq!(state.get_messages(ChatId(1)).unwrap().len(), 1);

    // Add second message (timestamp 2000)
    let msg2 = make_test_message(102, 1, 2000, "Second message");
    state.add_message(msg2.clone());

    assert_eq!(
        state.get_chat(ChatId(1)).unwrap().last_message.as_ref().unwrap().id,
        MessageId(102)
    );
    assert_eq!(state.get_messages(ChatId(1)).unwrap().len(), 2);

    // Add older message arrived out-of-order (timestamp 500)
    let msg0 = make_test_message(100, 1, 500, "Older message");
    state.add_message(msg0.clone());

    // Messages must be chronologically sorted by date: msg0 (500), msg1 (1000), msg2 (2000)
    let msgs = state.get_messages(ChatId(1)).unwrap();
    assert_eq!(msgs.len(), 3);
    assert_eq!(msgs[0].id, MessageId(100));
    assert_eq!(msgs[1].id, MessageId(101));
    assert_eq!(msgs[2].id, MessageId(102));

    // Update existing message (edit msg1 text)
    let msg1_edited = make_test_message(101, 1, 1000, "First message (edited)");
    state.add_message(msg1_edited);

    // Message list length must remain 3 (no duplicates)
    let msgs = state.get_messages(ChatId(1)).unwrap();
    assert_eq!(msgs.len(), 3);
    match &msgs[1].content {
        MessageContent::Text { text, .. } => assert_eq!(text, "First message (edited)"),
        _ => panic!("Expected text message"),
    }
}
