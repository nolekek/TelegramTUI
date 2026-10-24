use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use serde_json::json;

use tg_core::backend::tdlib::router::{RequestRegistration, RequestRouter};
use tg_core::backend::TelegramBackend;
use tg_core::domain::{
    Chat, ChatId, ChatPosition, ChatType, Message, MessageContent, MessageId, MessageStatus,
    Sender, UserId,
};
use tg_core::error::CoreError;
use tg_core::state::CoreState;
use tg_core::backend::mock::MockBackend;

// ============================================================================
// SUITE 1: RequestRouter Concurrency, High Volume, Timeouts & Cancellations
// ============================================================================

/// Stress-test RequestRouter with 10,000 requests across 16 concurrent workers.
/// Dispatches responses out of order and verifies 100% correlation fidelity.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_router_high_volume_concurrency() {
    let router = Arc::new(RequestRouter::new());
    let num_requests: usize = 10_000;
    let concurrency: usize = 16;
    let per_worker = num_requests / concurrency;

    let mut handles = Vec::new();
    let (tx_dispatch, rx_dispatch) = tokio::sync::mpsc::channel::<(String, usize)>(num_requests);

    // Spawn 16 registering tasks
    for worker_id in 0..concurrency {
        let router_clone = router.clone();
        let tx = tx_dispatch.clone();
        handles.push(tokio::spawn(async move {
            let mut receivers = Vec::with_capacity(per_worker);
            for i in 0..per_worker {
                let req_index = worker_id * per_worker + i;
                let (extra_id, rx) = router_clone.register();
                tx.send((extra_id, req_index)).await.unwrap();
                receivers.push((rx, req_index));
            }

            drop(tx);

            // Await all our receivers
            for (rx, req_index) in receivers {
                let res = rx.await.expect("Receiver dropped prematurely");
                assert!(res.is_ok(), "Request returned error: {:?}", res);
                let val = res.unwrap();
                assert_eq!(
                    val.get("index").and_then(|v| v.as_u64()),
                    Some(req_index as u64),
                    "Response index mismatch for worker {}",
                    worker_id
                );
            }
        }));
    }
    drop(tx_dispatch); // Close sender so dispatcher can finish

    // Spawn dispatcher task to simulate TDLib responses received out of order
    let router_dispatch = router.clone();
    let dispatch_handle = tokio::spawn(async move {
        let mut all_requests = Vec::with_capacity(num_requests);
        let mut rx = rx_dispatch;
        while let Some(item) = rx.recv().await {
            all_requests.push(item);
        }

        // Shuffle / reverse dispatch order to test out-of-order correlation
        all_requests.reverse();

        for (extra_id, req_index) in all_requests {
            let response = json!({
                "@type": "ok",
                "@extra": extra_id,
                "index": req_index
            });
            let unhandled = router_dispatch.dispatch(response);
            assert!(unhandled.is_none(), "Expected dispatch to consume @extra response");
        }
    });

    for h in handles {
        h.await.expect("Registering task panicked");
    }
    dispatch_handle.await.expect("Dispatcher task panicked");

    assert_eq!(
        router.pending_count(),
        0,
        "All requests should have completed and pending map must be empty"
    );
}

/// Stress-test RAII drop guard cancellations: half of the 5,000 requests are dropped
/// mid-flight, and responses arrive for both active and cancelled requests.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_router_cancellations_and_drop_guards_under_load() {
    let router = Arc::new(RequestRouter::new());
    let total = 5_000;

    let mut active_receivers = Vec::new();
    let mut to_dispatch = Vec::new();

    for i in 0..total {
        let (extra_id, rx) = router.register();
        let guard = RequestRegistration::new(&router, extra_id.clone());

        if i % 2 == 0 {
            // Cancelled request: guard dropped without complete()
            drop(guard);
            drop(rx);
            to_dispatch.push((extra_id, false));
        } else {
            // Active request: guard completed
            guard.complete();
            active_receivers.push((rx, extra_id.clone()));
            to_dispatch.push((extra_id, true));
        }
    }

    assert_eq!(
        router.pending_count(),
        total / 2,
        "Pending count should exactly match non-cancelled requests"
    );

    // Concurrently dispatch responses for ALL requests (both cancelled and active)
    for (extra_id, _is_active) in to_dispatch {
        let response = json!({
            "@type": "ok",
            "@extra": extra_id,
            "data": "payload"
        });
        let unhandled = router.dispatch(response);
        assert!(unhandled.is_none());
    }

    // Verify all active receivers got their response
    for (rx, extra_id) in active_receivers {
        let res = rx.await.expect("Active request receiver failed");
        assert!(res.is_ok(), "Expected Ok response for extra_id {}", extra_id);
    }

    assert_eq!(router.pending_count(), 0, "Pending count must be 0 after all dispatches");
}

/// Stress-test timeouts and delayed response handling.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_router_timeouts_and_delayed_responses() {
    let router = Arc::new(RequestRouter::new());
    let count = 1_000;

    let mut fast_requests = Vec::new();
    let mut slow_requests = Vec::new();

    for i in 0..count {
        let (extra_id, rx) = router.register();
        let reg = RequestRegistration::new(&router, extra_id.clone());
        if i % 2 == 0 {
            fast_requests.push((extra_id, rx, reg));
        } else {
            slow_requests.push((extra_id, rx, reg));
        }
    }

    // Fast requests: dispatch immediately
    for (extra_id, _, _) in &fast_requests {
        let res = json!({ "@type": "ok", "@extra": extra_id });
        assert!(router.dispatch(res).is_none());
    }

    for (_, rx, reg) in fast_requests {
        let result = tokio::time::timeout(Duration::from_millis(50), rx).await;
        assert!(result.is_ok(), "Fast request should not time out");
        let recv_res = result.unwrap();
        assert!(recv_res.is_ok(), "Channel should not be closed");
        reg.complete();
    }

    // Slow requests: let them time out by awaiting with 1ms
    let mut timed_out_ids = Vec::new();
    for (extra_id, rx, reg) in slow_requests {
        let result = tokio::time::timeout(Duration::from_millis(1), rx).await;
        assert!(result.is_err(), "Expected timeout");
        timed_out_ids.push(extra_id);
        drop(reg); // Drop without complete() -> cancels from router
    }

    assert_eq!(router.pending_count(), 0, "All timed out requests must be cancelled");

    // Now late responses arrive from TDLib for the timed-out requests
    for extra_id in timed_out_ids {
        let late_res = json!({ "@type": "ok", "@extra": extra_id });
        let unhandled = router.dispatch(late_res);
        assert!(unhandled.is_none(), "Late response with @extra must be consumed without panic");
    }

    assert_eq!(router.pending_count(), 0);
}

/// Adversarial inputs to RequestRouter::dispatch:
/// Malformed @extra, non-string/non-number types, missing fields, duplicate dispatch.
#[tokio::test]
async fn test_router_adversarial_dispatch_inputs() {
    let router = RequestRouter::new();

    // 1. Spontaneous update without @extra -> must be passed through
    let spontaneous = json!({
        "@type": "updateNewMessage",
        "message": { "id": 100 }
    });
    assert_eq!(router.dispatch(spontaneous.clone()), Some(spontaneous));

    // 2. @extra as null, bool, array, object -> must be passed through as spontaneous
    let null_extra = json!({ "@type": "updateTest", "@extra": null });
    assert_eq!(router.dispatch(null_extra.clone()), Some(null_extra));

    let bool_extra = json!({ "@type": "updateTest", "@extra": true });
    assert_eq!(router.dispatch(bool_extra.clone()), Some(bool_extra));

    let array_extra = json!({ "@type": "updateTest", "@extra": [1, 2, 3] });
    assert_eq!(router.dispatch(array_extra.clone()), Some(array_extra));

    let obj_extra = json!({ "@type": "updateTest", "@extra": { "foo": "bar" } });
    assert_eq!(router.dispatch(obj_extra.clone()), Some(obj_extra));

    // 3. Register a request
    let (extra_id, mut rx) = router.register();

    // 4. Dispatch malformed error object (missing code and missing message)
    let bad_err = json!({
        "@type": "error",
        "@extra": extra_id
    });
    assert!(router.dispatch(bad_err).is_none());

    let res = rx.try_recv().expect("Expected response");
    assert!(res.is_err());
    match res.unwrap_err() {
        CoreError::RpcError { code, message } => {
            assert_eq!(code, 0, "Missing code should default to 0");
            assert_eq!(message, "Unknown TDLib error", "Missing message should default");
        }
        other => panic!("Expected RpcError, got {:?}", other),
    }

    // 5. Duplicate dispatch for the same extra_id
    let duplicate_dispatch = json!({
        "@type": "ok",
        "@extra": extra_id
    });
    assert!(router.dispatch(duplicate_dispatch).is_none(), "Duplicate dispatch must be safely consumed");

    // 6. Large payload dispatch
    let (extra_large, mut rx_large) = router.register();
    let large_str = "x".repeat(1_000_000); // 1MB string
    let large_val = json!({
        "@type": "largeResult",
        "@extra": extra_large,
        "data": large_str
    });
    assert!(router.dispatch(large_val).is_none());
    let recv_large = rx_large.try_recv().unwrap().unwrap();
    assert_eq!(
        recv_large.get("data").and_then(|d| d.as_str()).map(|s| s.len()),
        Some(1_000_000)
    );
}

/// Stress-test abort_all during concurrent request registration and dispatch.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_router_abort_all_concurrency() {
    let router = Arc::new(RequestRouter::new());
    let running = Arc::new(AtomicBool::new(true));

    let mut reg_handles = Vec::new();
    for _ in 0..4 {
        let router_c = router.clone();
        let running_c = running.clone();
        reg_handles.push(tokio::spawn(async move {
            let mut count = 0;
            while running_c.load(Ordering::Relaxed) && count < 1000 {
                let (_id, rx) = router_c.register();
                tokio::task::yield_now().await;
                let _ = rx.await;
                count += 1;
            }
        }));
    }

    tokio::time::sleep(Duration::from_millis(5)).await;
    router.abort_all("Simulated connection termination");
    running.store(false, Ordering::Relaxed);

    for h in reg_handles {
        let _ = h.await;
    }

    // Secondary abort must be idempotent
    router.abort_all("Secondary call");
    assert_eq!(router.pending_count(), 0);
}

// ============================================================================
// SUITE 2: CoreState Chat Ordering, Conflicting Pins, Negative/Zero Orders
// ============================================================================

fn make_chat(id: i64, order: u64, is_pinned: bool) -> Chat {
    Chat {
        id: ChatId(id),
        title: format!("Chat {}", id),
        chat_type: ChatType::Private,
        unread_count: 0,
        last_read_inbox_message_id: 0,
        last_read_outbox_message_id: 0,
        last_message: None,
        position: Some(ChatPosition { order, is_pinned }),
        is_muted: false,
        has_unread_mention: false,
        folder_ids: Vec::new(),
    }
}

/// Verify ordering with zero, max u64, negative ChatIds, and position None.
#[test]
fn test_chat_ordering_extreme_boundary_orders() {
    let mut state = CoreState::new();

    // Pinned chat with order 0 -> MUST be included and at the top!
    let c_pinned_zero = make_chat(1, 0, true);
    // Pinned chat with max order
    let c_pinned_max = make_chat(2, u64::MAX, true);
    // Unpinned chat with order 0 -> MUST BE EXCLUDED!
    let c_unpinned_zero = make_chat(3, 0, false);
    // Unpinned chat with order 1 (lowest non-zero)
    let c_unpinned_one = make_chat(4, 1, false);
    // Unpinned chat with max order
    let c_unpinned_max = make_chat(5, u64::MAX, false);
    // Chat with position None -> MUST be included at the bottom (order 0 unpinned)
    let mut c_none = make_chat(6, 0, false);
    c_none.position = None;

    state.insert_or_update_chat(c_pinned_zero);
    state.insert_or_update_chat(c_pinned_max);
    state.insert_or_update_chat(c_unpinned_zero);
    state.insert_or_update_chat(c_unpinned_one);
    state.insert_or_update_chat(c_unpinned_max);
    state.insert_or_update_chat(c_none);

    // Expected order:
    // 1. Pinned max (order u64::MAX): Chat 2
    // 2. Pinned zero (order 0): Chat 1
    // 3. Unpinned max (order u64::MAX): Chat 5
    // 4. Unpinned one (order 1): Chat 4
    // 5. None position chat (order 0): Chat 6
    // EXCLUDED: Chat 3 (order 0, unpinned)
    assert_eq!(
        state.ordered_chats,
        vec![ChatId(2), ChatId(1), ChatId(5), ChatId(4), ChatId(6)]
    );
    assert!(
        !state.ordered_chats.contains(&ChatId(3)),
        "Chat with order 0 and is_pinned=false MUST NOT be in ordered_chats"
    );
}

/// Conflicting pins and identical orders:
/// Verifies total, deterministic ordering and permutation invariance.
#[test]
fn test_chat_ordering_conflicting_pins_and_identical_orders() {
    let mut state = CoreState::new();

    // 10 chats with identical order=1000 and is_pinned=true
    for id in 1..=10 {
        state.insert_or_update_chat(make_chat(id, 1000, true));
    }
    // 10 chats with identical order=1000 and is_pinned=false
    for id in 11..=20 {
        state.insert_or_update_chat(make_chat(id, 1000, false));
    }

    assert_eq!(state.ordered_chats.len(), 20);

    // Pinned chats must come first, tie-broken by ChatId descending: 10 down to 1
    for i in 0..10 {
        assert_eq!(
            state.ordered_chats[i],
            ChatId(10 - i as i64),
            "Pinned chats must be ordered by ChatId descending on equal order"
        );
    }

    // Unpinned chats come second, tie-broken by ChatId descending: 20 down to 11
    for i in 0..10 {
        assert_eq!(
            state.ordered_chats[10 + i],
            ChatId(20 - i as i64),
            "Unpinned chats must be ordered by ChatId descending on equal order"
        );
    }

    // Permutation invariance: insert in completely reversed order and verify identical result
    let mut state2 = CoreState::new();
    for id in (11..=20).rev() {
        state2.insert_or_update_chat(make_chat(id, 1000, false));
    }
    for id in (1..=10).rev() {
        state2.insert_or_update_chat(make_chat(id, 1000, true));
    }
    assert_eq!(state.ordered_chats, state2.ordered_chats);
}

/// Negative ChatIds: Telegram channels and supergroups have negative IDs.
/// Verifies negative IDs sort correctly and tie-break deterministically.
#[test]
fn test_chat_ordering_negative_chat_ids() {
    let mut state = CoreState::new();

    // Supergroups with negative IDs
    let c1 = make_chat(-1001234567890, 500, false);
    let c2 = make_chat(-1009876543210, 500, false);
    // User chat with positive ID
    let c3 = make_chat(12345, 500, false);

    state.insert_or_update_chat(c1);
    state.insert_or_update_chat(c2);
    state.insert_or_update_chat(c3);

    // All have order 500, unpinned.
    // Descending ChatId comparison:
    // 12345 > -1001234567890 > -1009876543210
    assert_eq!(
        state.ordered_chats,
        vec![
            ChatId(12345),
            ChatId(-1001234567890),
            ChatId(-1009876543210)
        ]
    );
}

/// Rapid simulated chat position flapping:
/// 1,000 rapid updates across 30 chats toggling pins, orders, order 0 removal,
/// and ensuring the list has zero duplicates and maintains all ordering invariants.
#[test]
fn test_chat_ordering_rapid_position_flapping() {
    let mut state = CoreState::new();

    for id in 1..=30 {
        state.insert_or_update_chat(make_chat(id, (id as u64) * 10, false));
    }

    assert_eq!(state.ordered_chats.len(), 30);

    // Apply 1,000 chaotic position updates
    for step in 0..1000 {
        let chat_id = ChatId((step % 30) + 1);
        let is_pinned = (step % 5) == 0;
        let order = if (step % 7) == 0 {
            0 // Order 0 -> should remove if unpinned!
        } else {
            ((step * 17) % 50_000) as u64 + 1
        };

        state.update_chat_position(chat_id, Some(ChatPosition { order, is_pinned }));

        // Invariant 1: No duplicates in ordered_chats
        let mut set = HashSet::new();
        for &id in &state.ordered_chats {
            assert!(set.insert(id), "Duplicate ChatId {:?} detected in ordered_chats!", id);
        }

        // Invariant 2: No unpinned chat with order == 0 in ordered_chats
        for &id in &state.ordered_chats {
            let chat = state.get_chat(id).expect("Chat in ordered_chats must exist");
            if let Some(pos) = chat.position {
                if pos.order == 0 {
                    assert!(pos.is_pinned, "Chat {:?} has order 0 but is not pinned!", id);
                }
            }
        }

        // Invariant 3: All pinned chats come before all unpinned chats
        let mut seen_unpinned = false;
        for &id in &state.ordered_chats {
            let chat = state.get_chat(id).unwrap();
            let is_p = chat.position.map(|p| p.is_pinned).unwrap_or(false);
            if !is_p {
                seen_unpinned = true;
            } else {
                assert!(!seen_unpinned, "Pinned chat {:?} appeared after unpinned chats!", id);
            }
        }
    }
}

/// Stress-test message history, content editing, deletion, and outbox read receipts.
#[test]
fn test_state_message_management_adversarial_stress() {
    let mut state = CoreState::new();
    let chat_id = ChatId(1);
    state.insert_or_update_chat(make_chat(1, 100, false));

    // Add 100 messages out of date order
    for i in (1..=100).rev() {
        let msg = Message {
            id: MessageId(i),
            chat_id,
            sender: Sender::User(UserId(999)),
            date: i * 10,
            content: MessageContent::Text {
                text: format!("Msg {}", i),
                entities: Vec::new(),
            },
            is_outgoing: i % 2 == 0,
            status: MessageStatus::Sending,
            reply_to_message_id: None,
            can_be_edited: true,
            can_be_deleted: true,
        };
        state.add_message(msg);
    }

    let msgs = state.get_messages(chat_id).unwrap();
    assert_eq!(msgs.len(), 100);
    // Verify messages are sorted by date ascending
    for i in 0..99 {
        assert!(msgs[i].date <= msgs[i + 1].date);
    }

    // Read outbox up to message 50
    state.update_chat_read_outbox(chat_id, 50);
    let msgs_after_read = state.get_messages(chat_id).unwrap();
    for m in msgs_after_read {
        if m.is_outgoing && m.id.0 <= 50 {
            assert_eq!(m.status, MessageStatus::Read, "Message {} should be Read", m.id.0);
        } else if m.is_outgoing && m.id.0 > 50 {
            assert_eq!(m.status, MessageStatus::Sending, "Message {} should still be Sending", m.id.0);
        }
    }

    // Delete 30 messages (including some non-existent IDs like 9999, -5)
    let to_delete = vec![
        MessageId(10),
        MessageId(20),
        MessageId(100), // Current last message!
        MessageId(9999), // Non-existent
        MessageId(-5),   // Non-existent negative
    ];
    state.delete_messages(chat_id, &to_delete);

    let msgs_after_del = state.get_messages(chat_id).unwrap();
    assert_eq!(msgs_after_del.len(), 97);
    assert!(!msgs_after_del.iter().any(|m| m.id == MessageId(100)));
    assert!(!msgs_after_del.iter().any(|m| m.id == MessageId(10)));

    // Verify last_message on chat updated to message 99!
    let chat = state.get_chat(chat_id).unwrap();
    assert_eq!(
        chat.last_message.as_ref().map(|m| m.id),
        Some(MessageId(99)),
        "Chat's last_message must fall back to the new latest message"
    );

    // Delete ALL remaining messages
    let remaining_ids: Vec<MessageId> = msgs_after_del.iter().map(|m| m.id).collect();
    state.delete_messages(chat_id, &remaining_ids);

    let msgs_empty = state.get_messages(chat_id).unwrap();
    assert_eq!(msgs_empty.len(), 0);
    let chat_empty = state.get_chat(chat_id).unwrap();
    assert!(
        chat_empty.last_message.is_none(),
        "Chat last_message should be None when all messages deleted"
    );
}

/// Stress-test MockBackend pagination edge cases:
/// limit = 0, limit > available, limit < -available, from_msg_id not found, etc.
#[tokio::test]
async fn test_mock_backend_pagination_edge_cases() {
    let mock = MockBackend::new();
    let (tx, _rx) = tokio::sync::broadcast::channel(100);
    mock.start(tx).await.unwrap();

    // Auth to seed mock chats
    mock.send_phone_number("+1234567890").await.unwrap();
    mock.send_code("11111").await.unwrap();

    let chat_id = ChatId(1);

    // 1. limit == 0 -> should return empty vec
    let p_zero = mock.load_history(chat_id, None, 0).await.unwrap();
    assert!(p_zero.is_empty(), "Limit 0 must return empty vector");

    // 2. limit > total messages in chat (mock has 11 messages)
    let p_large = mock.load_history(chat_id, None, 100).await.unwrap();
    assert_eq!(p_large.len(), 11, "Should return all available messages when limit exceeds count");

    // 3. from_msg_id does not exist (e.g. MessageId(999999))
    let p_nonexistent = mock.load_history(chat_id, Some(MessageId(999999)), 5).await.unwrap();
    assert_eq!(p_nonexistent.len(), 5, "Should return 5 messages older than non-existent high ID");

    // 4. from_msg_id is 0 or negative
    let p_zero_from = mock.load_history(chat_id, Some(MessageId(0)), 5).await.unwrap();
    assert_eq!(p_zero_from.len(), 5, "From ID 0 should return latest 5 messages");

    // 5. Forward pagination with extreme negative limit
    let p_fwd_large = mock.load_history(chat_id, Some(MessageId(1)), -100).await.unwrap();
    assert!(!p_fwd_large.is_empty(), "Forward pagination should return newer messages");
    assert!(p_fwd_large.len() <= 11);
}
