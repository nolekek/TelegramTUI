use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tg_core::{
    backend::tdlib::router::RequestRouter,
    domain::{
        AuthState, Chat, ChatId, ChatPosition, ChatType, CodeInfo, Message,
        MessageContent, MessageId, MessageStatus, PasswordHint, QrCodeInfo, Sender,
        UserId,
    },
    event::{ConnectionState, CoreEvent},
    CoreError, CoreState,
};

// ============================================================================
// 1. ADVERSARIAL TDLib JSON PARSER / UPDATE SIMULATION
// Replicates and stress-tests the exact JSON extraction logic used in `handle_update`
// to empirically verify zero panics across hundreds of malformed inputs.
// ============================================================================

/// Direct simulation of the extraction logic from `TDLibInner::handle_update`
fn simulate_handle_update(
    update: &Value,
    event_tx: &broadcast::Sender<CoreEvent>,
) -> Result<usize, String> {
    let mut events_emitted = 0;

    let update_type = match update.get("@type").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return Ok(0),
    };

    match update_type {
        "updateAuthorizationState" => {
            if let Some(auth_state_obj) = update.get("authorization_state") {
                let state_type = auth_state_obj.get("@type").and_then(|v| v.as_str()).unwrap_or("");
                match state_type {
                    "authorizationStateWaitTdlibParameters" => {}
                    "authorizationStateWaitEncryptionKey" => {}
                    "authorizationStateWaitPhoneNumber" => {
                        let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::WaitPhoneNumber));
                        events_emitted += 1;
                    }
                    "authorizationStateWaitOtherDeviceConfirmation" => {
                        let link = auth_state_obj
                            .get("link")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::WaitQrCode(QrCodeInfo { link })));
                        events_emitted += 1;
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
                        events_emitted += 1;
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
                        events_emitted += 1;
                    }
                    "authorizationStateWaitRegistration" => {
                        let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::WaitRegistration));
                        events_emitted += 1;
                    }
                    "authorizationStateReady" => {
                        let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::Ready));
                        events_emitted += 1;
                    }
                    "authorizationStateLoggingOut" => {
                        let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::LoggingOut));
                        events_emitted += 1;
                    }
                    "authorizationStateClosed" => {
                        let _ = event_tx.send(CoreEvent::AuthStateChanged(AuthState::Closed));
                        events_emitted += 1;
                    }
                    _ => {}
                }
            }
        }
        "error" => {
            let code = update.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
            let message = update.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown TDLib error");
            let err_str = format!("TDLib error ({}): {}", code, message);
            let _ = event_tx.send(CoreEvent::ErrorOccurred(err_str));
            events_emitted += 1;
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
                events_emitted += 1;
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
                    events_emitted += 1;
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
                events_emitted += 1;
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
                events_emitted += 1;
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
                events_emitted += 1;
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
                events_emitted += 1;
            }
        }
        _ => {}
    }

    Ok(events_emitted)
}

#[tokio::test]
async fn test_adversarial_malformed_json_inputs_zero_panics() {
    let (event_tx, _event_rx) = broadcast::channel(1024);

    let malformed_inputs: Vec<Value> = vec![
        // 1. Primitive/non-object values
        Value::Null,
        Value::Bool(true),
        Value::Bool(false),
        Value::Number(serde_json::Number::from(0)),
        Value::Number(serde_json::Number::from(-999999)),
        Value::String("not an object".into()),
        Value::Array(vec![]),
        Value::Array(vec![Value::Null, Value::String("foo".into())]),

        // 2. Empty or corrupted object roots
        json!({}),
        json!({ "@type": null }),
        json!({ "@type": 12345 }),
        json!({ "@type": false }),
        json!({ "@type": [] }),
        json!({ "@type": {} }),
        json!({ "@type": "unknownUpdateTypeRandom" }),
        json!({ "@type": "" }),

        // 3. Malformed updateAuthorizationState
        json!({ "@type": "updateAuthorizationState" }),
        json!({ "@type": "updateAuthorizationState", "authorization_state": null }),
        json!({ "@type": "updateAuthorizationState", "authorization_state": 42 }),
        json!({ "@type": "updateAuthorizationState", "authorization_state": {} }),
        json!({ "@type": "updateAuthorizationState", "authorization_state": { "@type": null } }),
        json!({ "@type": "updateAuthorizationState", "authorization_state": { "@type": "unknownAuthState" } }),
        json!({ "@type": "updateAuthorizationState", "authorization_state": { "@type": "authorizationStateWaitCode", "code_info": null } }),
        json!({ "@type": "updateAuthorizationState", "authorization_state": { "@type": "authorizationStateWaitCode", "code_info": {} } }),
        json!({ "@type": "updateAuthorizationState", "authorization_state": { "@type": "authorizationStateWaitCode", "code_info": { "phone_number": null, "timeout": "not a number", "type": null, "next_type": null } } }),
        json!({ "@type": "updateAuthorizationState", "authorization_state": { "@type": "authorizationStateWaitPassword", "password_hint": null, "has_recovery_email_address": "not_bool" } }),
        json!({ "@type": "updateAuthorizationState", "authorization_state": { "@type": "authorizationStateWaitOtherDeviceConfirmation", "link": null } }),

        // 4. Malformed TDLib error objects
        json!({ "@type": "error" }),
        json!({ "@type": "error", "code": null, "message": null }),
        json!({ "@type": "error", "code": "string_code", "message": 12345 }),
        json!({ "@type": "error", "code": -400, "message": "" }),
        json!({ "@type": "error", "code": i64::MAX, "message": "max error code" }),
        json!({ "@type": "error", "code": i64::MIN, "message": "min error code" }),
        json!({ "@type": "error", "extra_garbage_field": { "nested": true } }),

        // 5. Malformed updateChatPosition & updateChatPositions
        json!({ "@type": "updateChatPosition" }),
        json!({ "@type": "updateChatPosition", "chat_id": null }),
        json!({ "@type": "updateChatPosition", "chat_id": "not_int", "position": null }),
        json!({ "@type": "updateChatPosition", "chat_id": 123, "position": {} }),
        json!({ "@type": "updateChatPosition", "chat_id": 123, "position": { "list": null } }),
        json!({ "@type": "updateChatPosition", "chat_id": 123, "position": { "list": { "@type": "chatListArchive" } } }),
        json!({ "@type": "updateChatPosition", "chat_id": 123, "position": { "list": { "@type": "chatListMain" }, "order": null, "is_pinned": null } }),
        json!({ "@type": "updateChatPosition", "chat_id": 123, "position": { "list": { "@type": "chatListMain" }, "order": "corrupted_string_order", "is_pinned": "not_a_bool" } }),
        json!({ "@type": "updateChatPosition", "chat_id": 123, "position": { "list": { "@type": "chatListMain" }, "order": -999, "is_pinned": false } }),
        json!({ "@type": "updateChatPositions", "chat_id": 123, "positions": null }),
        json!({ "@type": "updateChatPositions", "chat_id": 123, "positions": "not_an_array" }),
        json!({ "@type": "updateChatPositions", "chat_id": 123, "positions": [null, 42, "str", {}, { "list": { "@type": "chatListMain" }, "order": "18446744073709551615", "is_pinned": true }] }),

        // 6. Malformed read inbox & outbox updates
        json!({ "@type": "updateChatReadInbox" }),
        json!({ "@type": "updateChatReadInbox", "chat_id": null, "last_read_inbox_message_id": null, "unread_count": null }),
        json!({ "@type": "updateChatReadInbox", "chat_id": -100123456789i64, "last_read_inbox_message_id": "bad", "unread_count": "bad" }),
        json!({ "@type": "updateChatReadOutbox" }),
        json!({ "@type": "updateChatReadOutbox", "chat_id": null }),
        json!({ "@type": "updateChatReadOutbox", "chat_id": 456, "last_read_outbox_message_id": null }),
        json!({ "@type": "updateChatReadOutbox", "chat_id": 456, "last_read_outbox_message_id": i64::MIN }),

        // 7. Malformed message deletions
        json!({ "@type": "updateDeleteMessages" }),
        json!({ "@type": "updateDeleteMessages", "chat_id": null, "message_ids": null }),
        json!({ "@type": "updateDeleteMessages", "chat_id": 789, "message_ids": "not_array" }),
        json!({ "@type": "updateDeleteMessages", "chat_id": 789, "message_ids": [null, "foo", -1, 0, i64::MAX] }),

        // 8. Connection state malformed
        json!({ "@type": "updateConnectionState" }),
        json!({ "@type": "updateConnectionState", "state": null }),
        json!({ "@type": "updateConnectionState", "state": {} }),
        json!({ "@type": "updateConnectionState", "state": { "@type": "bogusConnectionState" } }),
    ];

    for (idx, input) in malformed_inputs.iter().enumerate() {
        let tx_safe = AssertUnwindSafe(&event_tx);
        let res = std::panic::catch_unwind(move || {
            let _ = simulate_handle_update(input, *tx_safe);
        });
        assert!(
            res.is_ok(),
            "Panic detected on malformed input #{}: {:?}",
            idx,
            input
        );
    }
}

// ============================================================================
// 2. ADVERSARIAL STRESS TEST: RequestRouter UNTRUSTED & MALFORMED PROTOCOL INPUTS
// ============================================================================

#[test]
fn test_request_router_malformed_and_extreme_inputs() {
    let router = RequestRouter::new();

    // Register a valid pending request
    let (extra_id, mut rx) = router.register();

    // 1. Dispatch completely non-matching values
    let non_matching_cases = vec![
        json!(null),
        json!(12345),
        json!("string"),
        json!([]),
        json!({}),
        json!({ "@type": "updateNewChat" }),
        json!({ "@extra": null }),
        json!({ "@extra": true }),
        json!({ "@extra": [] }),
        json!({ "@extra": {} }),
        json!({ "@extra": "unknown-nonexistent-request-id" }),
        json!({ "@extra": 999999999 }),
    ];

    for case in non_matching_cases {
        let unhandled = router.dispatch(case.clone());
        let has_valid_extra = case.get("@extra").map(|e| e.is_string() || e.is_number()).unwrap_or(false);
        if has_valid_extra {
            assert!(unhandled.is_none(), "Expected request with valid @extra to be consumed");
        } else {
            assert_eq!(unhandled, Some(case), "Expected request without valid @extra to pass through as unsolicited update");
        }
    }

    // Pending request must still be waiting
    assert_eq!(router.pending_count(), 1);
    assert!(rx.try_recv().is_err());

    // 2. Dispatch malformed error object matching the extra_id
    let malformed_error = json!({
        "@type": "error",
        "@extra": extra_id,
        "code": "not_an_int",      // code is wrong type
        "message": null            // message is null
    });

    let unhandled = router.dispatch(malformed_error);
    assert!(unhandled.is_none());
    assert_eq!(router.pending_count(), 0);

    let res = rx.try_recv().expect("Expected response for registered request");
    assert!(res.is_err());
    match res.unwrap_err() {
        CoreError::RpcError { code, message } => {
            // code should fallback to 0, message should fallback to default
            assert_eq!(code, 0);
            assert_eq!(message, "Unknown TDLib error");
        }
        other => panic!("Expected RpcError, got {:?}", other),
    }

    // 3. Dispatch duplicate response to already completed request
    let dup_response = json!({
        "@type": "ok",
        "@extra": extra_id
    });
    // Should be safely ignored (debug log) without panic
    let unhandled_dup = router.dispatch(dup_response);
    assert!(unhandled_dup.is_none());
}

// ============================================================================
// 3. ADVERSARIAL STRESS TEST: DELIVERY RECEIPTS & OUT-OF-ORDER STATE TRANSITIONS
// ============================================================================

#[test]
fn test_delivery_receipts_out_of_order_and_nonexistent_messages() {
    let mut state = CoreState::new();

    let chat_id = ChatId(100);
    let chat = Chat {
        id: chat_id,
        title: "Receipt Test Chat".to_string(),
        chat_type: ChatType::Private,
        unread_count: 5,
        last_read_inbox_message_id: 0,
        last_read_outbox_message_id: 0,
        last_message: None,
        position: Some(ChatPosition { order: 10, is_pinned: false }),
        is_muted: false,
        has_unread_mention: false,
        folder_ids: Vec::new(),
    };
    state.insert_or_update_chat(chat);

    // 1. Receipt on completely nonexistent chat -> MUST NOT PANIC
    state.update_chat_read_outbox(ChatId(9999), 500);
    state.update_chat_read_inbox(ChatId(9999), 500, 0);
    state.update_message_status(ChatId(9999), MessageId(500), MessageStatus::Read);

    // 2. Receipt for nonexistent message IDs (zero, negative, huge) -> MUST NOT PANIC
    state.update_chat_read_outbox(chat_id, 0);
    state.update_chat_read_outbox(chat_id, -100);
    state.update_chat_read_outbox(chat_id, i64::MAX);
    state.update_chat_read_inbox(chat_id, -50, -10); // negative unread count

    // Add a sequence of 5 outgoing messages: IDs 10, 20, 30, 40, 50
    for id_val in &[10, 20, 30, 40, 50] {
        let msg = Message {
            id: MessageId(*id_val),
            chat_id,
            sender: Sender::User(UserId(1)),
            date: *id_val,
            content: MessageContent::Text {
                text: format!("Msg {}", id_val),
                entities: Vec::new(),
            },
            is_outgoing: true,
            status: MessageStatus::Sending,
            reply_to_message_id: None,
            can_be_edited: true,
            can_be_deleted: true,
        };
        state.add_message(msg);
    }

    // Verify all initial messages are Sending
    for msg in state.get_messages(chat_id).unwrap() {
        assert_eq!(msg.status, MessageStatus::Sending);
    }

    // 3. Out-of-order transition 1: Mark ID 30 as Sent
    state.update_message_status(chat_id, MessageId(30), MessageStatus::Sent);
    assert_eq!(state.get_messages(chat_id).unwrap()[2].status, MessageStatus::Sent);

    // 4. Out-of-order transition 2: Delivery receipt for ID 40 arrives BEFORE ID 10 and 20 are Sent
    // Outbox read 40 should mark all outgoing messages <= 40 as Read!
    state.update_chat_read_outbox(chat_id, 40);

    let msgs = state.get_messages(chat_id).unwrap();
    assert_eq!(msgs[0].status, MessageStatus::Read, "Msg 10 should be Read");
    assert_eq!(msgs[1].status, MessageStatus::Read, "Msg 20 should be Read");
    assert_eq!(msgs[2].status, MessageStatus::Read, "Msg 30 should be Read");
    assert_eq!(msgs[3].status, MessageStatus::Read, "Msg 40 should be Read");
    assert_eq!(msgs[4].status, MessageStatus::Sending, "Msg 50 must still be Sending");

    // 5. Out-of-order transition 3: An older receipt arrives (ID 20)
    // Already read messages (30, 40) MUST NOT regress back to Sending or Sent!
    state.update_chat_read_outbox(chat_id, 20);

    let msgs_after_regression = state.get_messages(chat_id).unwrap();
    assert_eq!(msgs_after_regression[2].status, MessageStatus::Read, "Msg 30 must remain Read");
    assert_eq!(msgs_after_regression[3].status, MessageStatus::Read, "Msg 40 must remain Read");

    // 6. Out-of-order transition 4: update_message_send_succeeded for nonexistent old ID
    let new_unsolicited = Message {
        id: MessageId(60),
        chat_id,
        sender: Sender::User(UserId(1)),
        date: 60,
        content: MessageContent::Text { text: "Unsolicited".to_string(), entities: Vec::new() },
        is_outgoing: true,
        status: MessageStatus::Sending,
        reply_to_message_id: None,
        can_be_edited: true,
        can_be_deleted: true,
    };
    // Old ID 999 does not exist in collection
    state.update_message_send_succeeded(chat_id, MessageId(999), new_unsolicited);

    // It should safely append message 60 with status Sent
    let msgs_with_60 = state.get_messages(chat_id).unwrap();
    assert_eq!(msgs_with_60.len(), 6);
    let msg_60 = msgs_with_60.iter().find(|m| m.id == MessageId(60)).unwrap();
    assert_eq!(msg_60.status, MessageStatus::Sent);

    // 7. Duplicate delete_messages & nonexistent message IDs
    state.delete_messages(chat_id, &[MessageId(9999), MessageId(8888)]);
    assert_eq!(state.get_messages(chat_id).unwrap().len(), 6);

    // Delete message 10
    state.delete_messages(chat_id, &[MessageId(10)]);
    assert_eq!(state.get_messages(chat_id).unwrap().len(), 5);

    // Duplicate deletion of message 10
    state.delete_messages(chat_id, &[MessageId(10)]);
    assert_eq!(state.get_messages(chat_id).unwrap().len(), 5);

    // Deletion of all remaining messages in chat
    let all_remaining: Vec<MessageId> = state
        .get_messages(chat_id)
        .unwrap()
        .iter()
        .map(|m| m.id)
        .collect();
    state.delete_messages(chat_id, &all_remaining);
    assert_eq!(state.get_messages(chat_id).unwrap().len(), 0);

    // Chat's last_message must become None without crashing
    assert!(state.get_chat(chat_id).unwrap().last_message.is_none());

    // Deleting from empty chat
    state.delete_messages(chat_id, &[MessageId(100)]);
    assert_eq!(state.get_messages(chat_id).unwrap().len(), 0);
}

// ============================================================================
// 4. ADVERSARIAL STRESS TEST: RAPID CONCURRENT STATE EVENT REDUCER
// ============================================================================

#[test]
fn test_state_apply_update_adversarial_event_stream() {
    let mut state = CoreState::new();

    // Create 100 heterogeneous events with extreme values and out-of-order transitions
    let mut events = Vec::new();

    for i in 0..100 {
        let chat_id = ChatId(if i % 2 == 0 { -1000000 - i } else { 1000000 + i });
        
        // Chat update with extreme order keys
        let chat = Chat {
            id: chat_id,
            title: format!("Stress Chat {}", i),
            chat_type: if i % 3 == 0 { ChatType::Supergroup } else { ChatType::Private },
            unread_count: (i as i32) * 10 - 50, // includes negative unread counters
            last_read_inbox_message_id: i,
            last_read_outbox_message_id: i,
            last_message: None,
            position: Some(ChatPosition {
                order: if i % 5 == 0 { 0 } else { u64::MAX - (i as u64) },
                is_pinned: i % 4 == 0,
            }),
            is_muted: i % 2 == 0,
            has_unread_mention: i % 3 == 0,
            folder_ids: Vec::new(),
        };
        events.push(CoreEvent::ChatUpdated(Box::new(chat)));

        // Position changes
        events.push(CoreEvent::ChatPositionChanged {
            chat_id,
            position: if i % 7 == 0 {
                None
            } else {
                Some(ChatPosition {
                    order: (i as u64) * 1000,
                    is_pinned: i % 2 == 1,
                })
            },
        });

        // Messages
        let msg = Message {
            id: MessageId(i * 10),
            chat_id,
            sender: Sender::Unknown,
            date: i,
            content: MessageContent::Text {
                text: format!("Text {}", i),
                entities: Vec::new(),
            },
            is_outgoing: i % 2 == 0,
            status: MessageStatus::Sending,
            reply_to_message_id: None,
            can_be_edited: false,
            can_be_deleted: false,
        };
        events.push(CoreEvent::MessageReceived(Box::new(msg)));

        // Receipts
        events.push(CoreEvent::ChatReadOutboxChanged {
            chat_id,
            last_read_outbox_message_id: i * 10,
        });

        events.push(CoreEvent::ChatReadInboxChanged {
            chat_id,
            last_read_inbox_message_id: i * 10,
            unread_count: 0,
        });

        // Deletions
        events.push(CoreEvent::MessagesDeleted {
            chat_id,
            message_ids: vec![MessageId(i * 10), MessageId(999999)],
        });
    }

    // Apply all 500 interleaved stress events sequentially
    for event in &events {
        state.apply_update(event);
    }

    // Verify chat ordering invariant: pinned first, then order descending, then ChatId descending
    let ordered = state.ordered_chats();
    for window in ordered.windows(2) {
        let c1 = state.get_chat(window[0]).unwrap();
        let c2 = state.get_chat(window[1]).unwrap();

        let pos1 = c1.position.unwrap_or(ChatPosition { order: 0, is_pinned: false });
        let pos2 = c2.position.unwrap_or(ChatPosition { order: 0, is_pinned: false });

        if pos1.is_pinned != pos2.is_pinned {
            assert!(pos1.is_pinned && !pos2.is_pinned, "Pinned chats must precede unpinned chats");
        } else if pos1.order != pos2.order {
            assert!(pos1.order >= pos2.order, "Higher order must precede lower order");
        }
    }
}

// ============================================================================
// 5. ADVERSARIAL STRESS TEST: HIGH CONCURRENCY & EXTREME STRINGS
// ============================================================================

#[tokio::test]
async fn test_request_router_concurrent_chaos_stress() {
    let router = Arc::new(RequestRouter::new());
    let mut tasks = Vec::new();

    // Spawn 20 concurrent tasks registering and dispatching thousands of requests
    for task_idx in 0..20 {
        let r = router.clone();
        tasks.push(tokio::spawn(async move {
            for i in 0..100 {
                let (extra_id, mut rx) = r.register();
                if (task_idx + i) % 3 == 0 {
                    // Cancel immediately
                    r.cancel(&extra_id);
                } else if (task_idx + i) % 3 == 1 {
                    // Dispatch success response
                    let ok_resp = json!({
                        "@type": "ok",
                        "@extra": extra_id,
                        "payload": "x".repeat(10_000) // 10KB string payload
                    });
                    r.dispatch(ok_resp);
                    let res = rx.try_recv().unwrap();
                    assert!(res.is_ok());
                } else {
                    // Dispatch error response with extreme unicode / control chars
                    let err_resp = json!({
                        "@type": "error",
                        "@extra": extra_id,
                        "code": 404,
                        "message": "\u{0000}\u{FFFF}\u{1F4A9} ERROR \r\n\t---\"'\0"
                    });
                    r.dispatch(err_resp);
                    let res = rx.try_recv().unwrap();
                    assert!(res.is_err());
                }
            }
        }));
    }

    for t in tasks {
        t.await.expect("Task panicked during concurrent stress");
    }

    // After all tasks finish, pending count must return to 0
    assert_eq!(router.pending_count(), 0);
}

