use std::time::Duration;
use tg_core::{AuthState, Config, ConnectionState, CoreEvent, Engine, UserId};

/// Helper to poll handle's auth_state until condition is met or timeout expires.
async fn wait_for_auth_state<F>(
    handle: &tg_core::CoreHandle,
    predicate: F,
    timeout_dur: Duration,
) -> AuthState
where
    F: Fn(&AuthState) -> bool,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout_dur {
        let current = handle.read_state().auth_state.clone();
        if predicate(&current) {
            return current;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    handle.read_state().auth_state.clone()
}

#[tokio::test]
async fn test_auth_phone_and_sms_code_flow() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, _event_rx) = Engine::start(config).await.expect("Failed to start engine");

    // 1. Initial state should be WaitPhoneNumber
    let state = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitPhoneNumber),
        Duration::from_secs(2),
    )
    .await;
    assert_eq!(state, AuthState::WaitPhoneNumber);

    // 2. Submit phone number
    handle
        .send_phone_number("+1234567890")
        .await
        .expect("Failed to send phone number");

    // 3. Verify transition to WaitCode with correct metadata
    let state = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitCode(_)),
        Duration::from_secs(2),
    )
    .await;
    match state {
        AuthState::WaitCode(info) => {
            assert_eq!(info.phone_number, "+1234567890");
            assert_eq!(info.timeout, 60);
            assert_eq!(info.code_type, "telegram_message");
            assert_eq!(info.next_type, Some("sms".to_string()));
        }
        other => panic!("Expected WaitCode, got {:?}", other),
    }

    // 4. Submit standard verification code
    handle
        .send_code("11111")
        .await
        .expect("Failed to send code");

    // 5. Verify transition to Ready
    let state = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::Ready),
        Duration::from_secs(2),
    )
    .await;
    assert_eq!(state, AuthState::Ready);

    // 6. Verify user session and mock chats initialized
    let current_state = handle.clone_state();
    assert_eq!(current_state.connection_state, ConnectionState::Ready);
    let me = current_state.me.expect("Authenticated user 'me' must be set");
    assert_eq!(me.id, UserId(777000));
    assert_eq!(me.first_name, "Rust");
    assert_eq!(me.username, Some("rust_dev".to_string()));
    assert_eq!(current_state.ordered_chats.len(), 4);
    assert_eq!(current_state.ordered_chats[0].0, 1); // Saved Messages is pinned
}

#[tokio::test]
async fn test_auth_2fa_cloud_password_flow() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, _event_rx) = Engine::start(config).await.expect("Failed to start engine");

    // 1. Initial WaitPhoneNumber
    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitPhoneNumber),
        Duration::from_secs(2),
    )
    .await;

    // 2. Submit phone number
    handle
        .send_phone_number("+1987654321")
        .await
        .expect("Failed to send phone number");

    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitCode(_)),
        Duration::from_secs(2),
    )
    .await;

    // 3. Submit 2FA-triggering code ("22222")
    handle
        .send_code("22222")
        .await
        .expect("Failed to send code");

    // 4. Verify transition to WaitPassword
    let state = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitPassword(_)),
        Duration::from_secs(2),
    )
    .await;
    match state {
        AuthState::WaitPassword(hint) => {
            assert!(
                hint.hint
                    .as_deref()
                    .unwrap_or_default()
                    .contains("default password is 'rust'"),
                "Password hint missing expected content"
            );
            assert!(hint.has_recovery_email);
        }
        other => panic!("Expected WaitPassword, got {:?}", other),
    }

    // 5. Submit 2FA password
    handle
        .send_password("rust")
        .await
        .expect("Failed to send password");

    // 6. Verify transition to Ready
    let state = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::Ready),
        Duration::from_secs(2),
    )
    .await;
    assert_eq!(state, AuthState::Ready);
    assert!(!handle.read_state().ordered_chats.is_empty());
}

#[tokio::test]
async fn test_auth_qr_code_flow() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, _event_rx) = Engine::start(config).await.expect("Failed to start engine");

    // 1. Initial WaitPhoneNumber
    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitPhoneNumber),
        Duration::from_secs(2),
    )
    .await;

    // 2. Request QR code login
    handle.request_qr_code().await.expect("Failed to request QR code");

    // 3. Verify transition to WaitQrCode with valid tg:// link
    let state = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitQrCode(_)),
        Duration::from_secs(2),
    )
    .await;
    match state {
        AuthState::WaitQrCode(qr) => {
            assert!(
                qr.link.starts_with("tg://login?token="),
                "QR link must start with tg://login?token=, got: {}",
                qr.link
            );
            assert_eq!(qr.link, "tg://login?token=MOCK_QR_TOKEN_DEMO");
        }
        other => panic!("Expected WaitQrCode, got {:?}", other),
    }
}

#[tokio::test]
async fn test_auth_logout_and_session_termination() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, _event_rx) = Engine::start(config).await.expect("Failed to start engine");

    // Fast login
    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitPhoneNumber),
        Duration::from_secs(2),
    )
    .await;
    handle.send_phone_number("+1111111111").await.unwrap();
    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitCode(_)),
        Duration::from_secs(2),
    )
    .await;
    handle.send_code("11111").await.unwrap();
    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::Ready),
        Duration::from_secs(2),
    )
    .await;

    // Log out
    handle.log_out().await.expect("Failed to log out");

    // Verify Closed state
    let state = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::Closed),
        Duration::from_secs(2),
    )
    .await;
    assert_eq!(state, AuthState::Closed);

    // Stop engine gracefully
    handle.stop().await.expect("Failed to stop engine");
}

#[tokio::test]
async fn test_auth_events_broadcast_stream() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, mut event_rx) = Engine::start(config).await.expect("Failed to start engine");

    // Track observed event variants in real-time
    let mut saw_connection_ready = false;
    let mut saw_wait_phone = false;
    let mut saw_wait_code = false;
    let mut saw_auth_ready = false;
    let mut saw_user_updated = false;
    let mut saw_chat_list = false;

    // Spawn a monitor loop over the broadcast receiver
    let monitor_handle = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match event {
                CoreEvent::ConnectionStateChanged(ConnectionState::Ready) => {
                    saw_connection_ready = true;
                }
                CoreEvent::AuthStateChanged(AuthState::WaitPhoneNumber) => {
                    saw_wait_phone = true;
                }
                CoreEvent::AuthStateChanged(AuthState::WaitCode(_)) => {
                    saw_wait_code = true;
                }
                CoreEvent::AuthStateChanged(AuthState::Ready) => {
                    saw_auth_ready = true;
                }
                CoreEvent::UserUpdated(_) => {
                    saw_user_updated = true;
                }
                CoreEvent::ChatListUpdated(_) => {
                    saw_chat_list = true;
                }
                _ => {}
            }
            if saw_connection_ready
                && saw_wait_phone
                && saw_wait_code
                && saw_auth_ready
                && saw_user_updated
                && saw_chat_list
            {
                break;
            }
        }
        (
            saw_connection_ready,
            saw_wait_phone,
            saw_wait_code,
            saw_auth_ready,
            saw_user_updated,
            saw_chat_list,
        )
    });

    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitPhoneNumber),
        Duration::from_secs(2),
    )
    .await;
    handle.send_phone_number("+1000000000").await.unwrap();

    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitCode(_)),
        Duration::from_secs(2),
    )
    .await;
    handle.send_code("11111").await.unwrap();

    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::Ready),
        Duration::from_secs(2),
    )
    .await;

    let (conn_ok, phone_ok, code_ok, ready_ok, user_ok, chat_list_ok) =
        tokio::time::timeout(Duration::from_secs(3), monitor_handle)
            .await
            .expect("Event monitor timed out")
            .expect("Task join error");

    assert!(conn_ok, "Did not receive ConnectionState::Ready event");
    assert!(phone_ok, "Did not receive AuthState::WaitPhoneNumber event");
    assert!(code_ok, "Did not receive AuthState::WaitCode event");
    assert!(ready_ok, "Did not receive AuthState::Ready event");
    assert!(user_ok, "Did not receive UserUpdated event");
    assert!(chat_list_ok, "Did not receive ChatListUpdated event");
}
