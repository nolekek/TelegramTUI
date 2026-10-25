#![allow(dead_code)]

#[path = "../crates/tg-tui/src/text/mod.rs"]
pub mod text;

#[path = "../crates/tg-tui/src/ui/mod.rs"]
pub mod ui;

#[path = "../crates/tg-tui/src/app.rs"]
pub mod app;

#[path = "../crates/tg-tui/src/event.rs"]
pub mod event;

use std::path::{Path, PathBuf};
use std::time::Duration;

use app::{ActivePanel, App};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tg_core::backend::tdlib::raw::TdJsonRaw;
use tg_core::{
    AuthState, ChatId, Config, CoreError, CoreEvent, CoreHandle, Engine,
    FileTransferStatus, MessageContent, MessageStatus, ProxyConfig, ProxyType, Sender, UserId,
};
use ui::chat_list::ChatFilterTab;
use ui::modals::ActiveModal;
use ui::qr::render_qr_widget;

// ============================================================================
// Asynchronous Deterministic Polling Helpers
// ============================================================================

async fn wait_for_condition<F>(timeout_dur: Duration, mut predicate: F) -> bool
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout_dur {
        if predicate() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    predicate()
}

async fn wait_for_auth_state<F>(
    handle: &CoreHandle,
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
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    handle.read_state().auth_state.clone()
}

fn drain_events_to_app(app: &mut App, event_rx: &mut tokio::sync::broadcast::Receiver<CoreEvent>) {
    while let Ok(event) = event_rx.try_recv() {
        app.handle_core_event(event);
    }
}

// ============================================================================
// TEST 1: Full Client Lifecycle in Mock Mode (R1, R2, R3, Functional AC)
// ============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_e2e_full_client_lifecycle_mock_mode() {
    // 1. Config initialization and portable directory creation
    let config = Config {
        use_mock: true,
        ..Default::default()
    };
    config
        .ensure_directories()
        .expect("Failed to ensure .data directories");

    let tdlib_files_dir = PathBuf::from(".data").join("tdlib_files");
    assert!(
        tdlib_files_dir.exists(),
        ".data/tdlib_files directory must exist"
    );

    // 2. Mock engine startup
    let (handle, mut event_rx) = Engine::start(config)
        .await
        .expect("Engine failed to start in mock mode");

    let mut app = App::new(handle.clone());

    // 3. Authentication pipeline
    let initial_auth = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitPhoneNumber),
        Duration::from_secs(3),
    )
    .await;
    assert!(
        matches!(initial_auth, AuthState::WaitPhoneNumber),
        "Initial state should be WaitPhoneNumber"
    );

    drain_events_to_app(&mut app, &mut event_rx);

    // Submit phone number through App submit logic
    app.auth_modal_state.input_buffer = "+1234567890".to_string();
    app.handle_submit().await;

    let wait_code = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitCode(_)),
        Duration::from_secs(3),
    )
    .await;
    assert!(
        matches!(wait_code, AuthState::WaitCode(_)),
        "Expected WaitCode after phone submission"
    );

    drain_events_to_app(&mut app, &mut event_rx);

    // Submit confirmation code through App submit logic
    app.auth_modal_state.input_buffer = "11111".to_string();
    app.handle_submit().await;

    let ready_state = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::Ready),
        Duration::from_secs(3),
    )
    .await;
    assert!(
        matches!(ready_state, AuthState::Ready),
        "Expected Ready after SMS code verification"
    );

    drain_events_to_app(&mut app, &mut event_rx);
    assert_eq!(app.active_modal, ActiveModal::None);

    // 4. Offline navigation & Chat list queries
    let chats_ready = wait_for_condition(Duration::from_secs(3), || {
        handle.read_state().ordered_chats().len() >= 4
    })
    .await;
    assert!(
        chats_ready,
        "Mock chats should be populated in CoreState upon Ready"
    );

    drain_events_to_app(&mut app, &mut event_rx);

    {
        let state = handle.read_state();
        let chats = state.ordered_chats();
        assert_eq!(chats.len(), 4, "Should have 4 mock chats");

        // Verify pinned chat precedence
        let first_chat = state.get_chat(chats[0]).expect("First chat must exist");
        assert_eq!(first_chat.id, ChatId(1));
        assert_eq!(first_chat.title, "Saved Messages");
        assert!(
            first_chat
                .position
                .as_ref()
                .map(|p| p.is_pinned)
                .unwrap_or(false),
            "Saved Messages must be pinned"
        );
    }

    // Test TUI App chat navigation
    assert_eq!(app.selected_chat_index, 0);
    assert_eq!(app.active_chat_id(), Some(ChatId(1)));

    app.next_chat();
    assert_eq!(app.selected_chat_index, 1);
    assert_eq!(app.active_chat_id(), Some(ChatId(2)));

    app.previous_chat();
    assert_eq!(app.selected_chat_index, 0);
    assert_eq!(app.active_chat_id(), Some(ChatId(1)));

    // Test Chat Filtering Tabs
    app.switch_tab(ChatFilterTab::Personal);
    {
        let state = handle.read_state();
        let personal_chats = ui::chat_list::filter_chats(&state, ChatFilterTab::Personal);
        assert_eq!(personal_chats.len(), 2); // Saved Messages & Pavel Durov
    }

    app.switch_tab(ChatFilterTab::Groups);
    {
        let state = handle.read_state();
        let groups = ui::chat_list::filter_chats(&state, ChatFilterTab::Groups);
        assert_eq!(groups.len(), 1); // Rust Community
        assert_eq!(groups[0], ChatId(2));
    }

    app.switch_tab(ChatFilterTab::Channels);
    {
        let state = handle.read_state();
        let channels = ui::chat_list::filter_chats(&state, ChatFilterTab::Channels);
        assert_eq!(channels.len(), 1); // Telegram News
        assert_eq!(channels[0], ChatId(3));
    }

    app.switch_tab(ChatFilterTab::All);
    assert_eq!(app.active_chat_id(), Some(ChatId(1)));

    // Test Panel Switching
    assert_eq!(app.active_panel, ActivePanel::ChatList);
    app.cycle_panel_forward();
    assert_eq!(app.active_panel, ActivePanel::Messages);
    app.cycle_panel_forward();
    assert_eq!(app.active_panel, ActivePanel::Input);
    app.cycle_panel_forward();
    assert_eq!(app.active_panel, ActivePanel::ChatList);

    // 5. Interactive Message Sending & Simulated Incoming Echo Auto-Replies
    app.input_text = "E2E integration test message".to_string();
    app.cursor_pos = app.input_text.len();
    app.handle_submit().await;

    assert!(
        app.input_text.is_empty(),
        "Input buffer must be cleared on submit"
    );
    assert_eq!(app.status_message, "Message sent");

    // Wait for outgoing message to appear in state
    let sent_msg_id = {
        let sent_found = wait_for_condition(Duration::from_secs(3), || {
            let state = handle.read_state();
            let msgs = state.messages_for_chat(ChatId(1));
            msgs.iter().any(|m| {
                m.is_outgoing
                    && matches!(&m.content, MessageContent::Text { text, .. } if text == "E2E integration test message")
            })
        })
        .await;
        assert!(sent_found, "Outgoing message should be recorded in state");

        let state = handle.read_state();
        let msgs = state.messages_for_chat(ChatId(1));
        let outgoing = msgs
            .iter()
            .find(|m| {
                m.is_outgoing
                    && matches!(&m.content, MessageContent::Text { text, .. } if text == "E2E integration test message")
            })
            .expect("Sent message must exist");
        outgoing.id
    };

    // Wait for mock auto-reply (MockBackend emits after 300ms)
    let auto_reply_received = wait_for_condition(Duration::from_secs(4), || {
        let state = handle.read_state();
        let msgs = state.messages_for_chat(ChatId(1));
        msgs.iter().any(|m| {
            !m.is_outgoing
                && m.sender == Sender::User(UserId(999))
                && matches!(&m.content, MessageContent::Text { text, .. } if text.contains("Mock auto-reply to: \"E2E integration test message\""))
        })
    })
    .await;
    assert!(
        auto_reply_received,
        "Mock auto-reply should arrive and be recorded in state"
    );

    drain_events_to_app(&mut app, &mut event_rx);

    // Verify auto-reply properties
    {
        let state = handle.read_state();
        let msgs = state.messages_for_chat(ChatId(1));
        let reply = msgs
            .iter()
            .find(|m| {
                !m.is_outgoing
                    && m.sender == Sender::User(UserId(999))
                    && matches!(&m.content, MessageContent::Text { text, .. } if text.contains("Mock auto-reply to:"))
            })
            .expect("Auto-reply message should exist");

        assert_eq!(reply.reply_to_message_id, Some(sent_msg_id));
        assert_eq!(reply.status, MessageStatus::Read);
    }

    // 6. Progressive File Downloading via App
    // Chat 1 has Document file 101 ("architecture_specification.pdf") in history
    app.selected_chat_index = 0; // Focus Chat 1
    assert_eq!(app.active_chat_id(), Some(ChatId(1)));

    // Load message history for active chat into CoreState
    handle
        .load_history(ChatId(1), None, 20)
        .await
        .expect("load_history failed");

    let history_loaded = wait_for_condition(Duration::from_secs(3), || {
        let state = handle.read_state();
        let msgs = state.messages_for_chat(ChatId(1));
        msgs.iter().any(|m| m.content.file_id() == Some(101))
    })
    .await;
    assert!(
        history_loaded,
        "Document file 101 must be loaded into chat message history"
    );

    app.download_focused_media()
        .await
        .expect("download_focused_media failed");
    assert_eq!(
        app.status_message, "Started download for file #101",
        "Status should announce started download"
    );

    // Poll until file completes download
    let download_completed = wait_for_condition(Duration::from_secs(5), || {
        drain_events_to_app(&mut app, &mut event_rx);
        let state = handle.read_state();
        state
            .get_file(101)
            .map(|f| f.is_completed())
            .unwrap_or(false)
    })
    .await;
    assert!(download_completed, "File 101 download must complete");

    drain_events_to_app(&mut app, &mut event_rx);
    assert!(
        app.status_message.contains("Downloaded:") || app.status_message.contains("Загружено"),
        "Status should indicate file download completion, got: {}",
        app.status_message
    );

    // Verify actual file exists on disk with authentic contents
    let local_path = {
        let state = handle.read_state();
        let file = state.get_file(101).expect("File 101 must exist");
        assert!(file.is_completed());
        file.local_path.clone().expect("local_path must be Some")
    };
    assert!(
        local_path.exists(),
        "Downloaded file must physically exist at {:?}",
        local_path
    );
    let file_content = std::fs::read_to_string(&local_path).expect("Failed to read downloaded file");
    assert!(
        file_content.contains("file_id=101"),
        "File should contain simulated payload"
    );

    // 7. Full Ratatui Terminal UI Render Verification
    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).expect("Failed to init ratatui TestBackend");
    terminal
        .draw(|f| ui::render(f, &mut app))
        .expect("TUI render failed");

    let buffer = terminal.backend().buffer().clone();
    let rendered_text: String = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<Vec<_>>()
        .join("");

    assert!(
        rendered_text.contains("Saved Messages"),
        "Buffer should contain chat title 'Saved Messages'"
    );
    assert!(
        rendered_text.contains("Rust Community"),
        "Buffer should contain chat title 'Rust Community'"
    );
    assert!(
        rendered_text.contains("All"),
        "Buffer should contain Tab 'All'"
    );

    // 8. Graceful Clean Shutdown
    handle.stop().await.expect("Failed to cleanly stop engine");
}

// ============================================================================
// TEST 2: TDLib Dynamic Binary Loading Mode and Fallback (R1, Functional AC)
// ============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_e2e_tdlib_dynamic_loading_and_fallback() {
    // 1. Verify dynamic library loader discovers binary or fails gracefully with typed CoreError
    let custom_nonexistent = Path::new("nonexistent_directory/tdjson_missing.dll");
    let load_res = TdJsonRaw::load_auto(Some(custom_nonexistent));
    assert!(
        load_res.is_err(),
        "Loading nonexistent TDLib binary must fail"
    );
    match load_res.err().unwrap() {
        CoreError::LibraryNotFound(msg) => {
            assert!(msg.contains("Could not find tdjson dynamic library"));
        }
        other => panic!("Expected LibraryNotFound error, got {:?}", other),
    }

    // 2. Verify invalid shared library format surfaces typed LibraryLoadError
    let invalid_file = Path::new("Cargo.toml");
    let load_invalid_res = TdJsonRaw::load(invalid_file);
    assert!(
        load_invalid_res.is_err(),
        "Loading non-binary file as shared library must fail"
    );
    match load_invalid_res.err().unwrap() {
        CoreError::LibraryLoadError(msg) => {
            assert!(msg.contains("Failed to load"));
        }
        other => panic!("Expected LibraryLoadError, got {:?}", other),
    }

    // 3. Verify TDLib mode (use_mock = false):
    // When tdjson dynamic library is present, verify it connects to Telegram and reaches WaitPhoneNumber.
    // When tdjson is not installed, verify graceful typed LibraryNotFound error.
    let tdjson_present = TdJsonRaw::load_auto(None).is_ok();
    let temp_data = std::env::temp_dir().join(format!("tg_tdlib_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let config = Config {
        use_mock: false,
        data_dir: temp_data,
        ..Default::default()
    };

    if tdjson_present {
        let (handle, _event_rx) = Engine::start(config)
            .await
            .expect("Engine::start with use_mock=false must successfully start when tdjson is present");

        // Verify engine is fully functional and responsive
        let initial_auth = wait_for_auth_state(
            &handle,
            |s| matches!(s, AuthState::WaitPhoneNumber | AuthState::WaitQrCode(_)),
            Duration::from_secs(5),
        )
        .await;
        println!("test_e2e_tdlib_dynamic_loading_and_fallback initial_auth: {:?}", initial_auth);
        assert!(
            matches!(initial_auth, AuthState::WaitPhoneNumber | AuthState::WaitQrCode(_)),
            "Engine should reach WaitPhoneNumber or WaitQrCode, but got: {:?}",
            initial_auth
        );

        // Clean termination
        handle.stop().await.expect("Failed to stop engine");
    } else {
        // If tdjson dynamic library is absent, Engine::start must return typed LibraryNotFound error
        let start_res = Engine::start(config).await;
        assert!(start_res.is_err(), "Engine::start without mock must fail if tdjson is absent");
        match start_res.err().unwrap() {
            CoreError::LibraryNotFound(_) => {}
            other => panic!("Expected LibraryNotFound error, got {:?}", other),
        }
    }
}

// ============================================================================
// TEST 3: 2FA Cloud Password and QR Code Auth Pipeline in App (R1, R2)
// ============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_e2e_2fa_and_qr_auth_flows_in_app() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };
    let (handle, mut event_rx) = Engine::start(config).await.expect("Engine start failed");
    let mut app = App::new(handle.clone());

    // Wait for initial phone prompt
    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitPhoneNumber),
        Duration::from_secs(3),
    )
    .await;
    drain_events_to_app(&mut app, &mut event_rx);

    // Submit phone number
    app.auth_modal_state.input_buffer = "+18885551234".to_string();
    app.handle_submit().await;

    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitCode(_)),
        Duration::from_secs(3),
    )
    .await;
    drain_events_to_app(&mut app, &mut event_rx);

    // Submit special code "22222" which activates 2FA Password requirement
    app.auth_modal_state.input_buffer = "22222".to_string();
    app.handle_submit().await;

    let wait_pwd = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitPassword(_)),
        Duration::from_secs(3),
    )
    .await;
    assert!(
        matches!(wait_pwd, AuthState::WaitPassword(_)),
        "Expected WaitPassword after submitting 2FA trigger code '22222'"
    );

    drain_events_to_app(&mut app, &mut event_rx);
    assert_eq!(app.status_message, "Enter 2FA cloud password.");

    // Submit 2FA cloud password "rust"
    app.auth_modal_state.input_buffer = "rust".to_string();
    app.handle_submit().await;

    let ready = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::Ready),
        Duration::from_secs(3),
    )
    .await;
    assert!(
        matches!(ready, AuthState::Ready),
        "Expected Ready after entering valid 2FA password"
    );

    drain_events_to_app(&mut app, &mut event_rx);
    assert_eq!(app.active_modal, ActiveModal::None);

    // Test QR Code flow
    handle.request_qr_code().await.expect("request_qr_code failed");
    let wait_qr = wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitQrCode(_)),
        Duration::from_secs(3),
    )
    .await;

    if let AuthState::WaitQrCode(info) = wait_qr {
        assert!(
            info.link.starts_with("tg://login?token="),
            "QR link must have Telegram login URI scheme"
        );

        // Test in-terminal QR rendering into ratatui buffer
        let backend = TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).expect("Terminal init failed");
        terminal
            .draw(|f| {
                render_qr_widget(f, &info.link, f.area());
            })
            .expect("render_qr_widget failed");

        let buffer = terminal.backend().buffer().clone();
        let rendered: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<Vec<_>>()
            .join("");

        // Half-block character used in QR generator
        assert!(
            rendered.contains('▀') || rendered.contains('▄'),
            "Buffer should contain rendered QR half-blocks"
        );
    } else {
        panic!("Expected AuthState::WaitQrCode");
    }

    handle.stop().await.expect("Failed to stop engine");
}

// ============================================================================
// TEST 4: Outgoing Message Edit, Delete and Last Message Tracking (R1, R2)
// ============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_e2e_outgoing_message_edit_delete_lifecycle() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };
    let (handle, _event_rx) = Engine::start(config).await.expect("Engine start failed");

    // Authenticate
    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitPhoneNumber),
        Duration::from_secs(3),
    )
    .await;
    handle.send_phone_number("+10001112222").await.unwrap();

    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitCode(_)),
        Duration::from_secs(3),
    )
    .await;
    handle.send_code("11111").await.unwrap();

    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::Ready),
        Duration::from_secs(3),
    )
    .await;

    wait_for_condition(Duration::from_secs(3), || {
        handle.read_state().ordered_chats().len() >= 4
    })
    .await;

    // Send outgoing message in Chat 2 (Rust Community)
    let chat_id = ChatId(2);
    handle
        .send_text_message(chat_id, "Original text message", None)
        .await
        .expect("send_text_message failed");

    // Find sent message ID
    let sent_found = wait_for_condition(Duration::from_secs(3), || {
        let state = handle.read_state();
        let msgs = state.messages_for_chat(chat_id);
        msgs.iter().any(|m| {
            m.is_outgoing
                && matches!(&m.content, MessageContent::Text { text, .. } if text == "Original text message")
        })
    })
    .await;
    assert!(sent_found);

    let msg_id = {
        let state = handle.read_state();
        let msgs = state.messages_for_chat(chat_id);
        msgs.iter()
            .find(|m| {
                m.is_outgoing
                    && matches!(&m.content, MessageContent::Text { text, .. } if text == "Original text message")
            })
            .unwrap()
            .id
    };

    // Edit message text
    handle
        .edit_message_text(chat_id, msg_id, "Edited text v2.0")
        .await
        .expect("edit_message_text failed");

    let edit_applied = wait_for_condition(Duration::from_secs(3), || {
        let state = handle.read_state();
        let msgs = state.messages_for_chat(chat_id);
        msgs.iter().any(|m| {
            m.id == msg_id
                && matches!(&m.content, MessageContent::Text { text, .. } if text == "Edited text v2.0")
        })
    })
    .await;
    assert!(edit_applied, "Edited message text must update in CoreState");

    // Delete message
    handle
        .delete_messages(chat_id, vec![msg_id], true)
        .await
        .expect("delete_messages failed");

    let delete_applied = wait_for_condition(Duration::from_secs(3), || {
        let state = handle.read_state();
        let msgs = state.messages_for_chat(chat_id);
        !msgs.iter().any(|m| m.id == msg_id)
    })
    .await;
    assert!(
        delete_applied,
        "Deleted message must be removed from CoreState"
    );

    handle.stop().await.expect("Failed to stop engine");
}

// ============================================================================
// TEST 5: Concurrent File Downloads and Cancellation (R3)
// ============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_e2e_concurrent_media_downloads_and_cancellation() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };
    let (handle, _event_rx) = Engine::start(config).await.expect("Engine start failed");

    // Authenticate
    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitPhoneNumber),
        Duration::from_secs(3),
    )
    .await;
    handle.send_phone_number("+15556667777").await.unwrap();

    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::WaitCode(_)),
        Duration::from_secs(3),
    )
    .await;
    handle.send_code("11111").await.unwrap();

    wait_for_auth_state(
        &handle,
        |s| matches!(s, AuthState::Ready),
        Duration::from_secs(3),
    )
    .await;

    // Start download of file 201 (Audio in Chat 2) and file 301 (Photo in Chat 3)
    handle.download_file(201, 1).await.unwrap();
    handle.download_file(301, 1).await.unwrap();

    // Cancel file 201 immediately
    handle.cancel_download_file(201).await.unwrap();

    // Verify file 201 transitions to Failed("Download cancelled")
    let cancel_verified = wait_for_condition(Duration::from_secs(3), || {
        let state = handle.read_state();
        state
            .get_file(201)
            .map(|f| matches!(f.status, FileTransferStatus::Failed(_)))
            .unwrap_or(false)
    })
    .await;
    assert!(cancel_verified, "File 201 must transition to Failed status");

    // Verify file 301 completes successfully
    let completed_verified = wait_for_condition(Duration::from_secs(5), || {
        let state = handle.read_state();
        state
            .get_file(301)
            .map(|f| f.is_completed())
            .unwrap_or(false)
    })
    .await;
    assert!(completed_verified, "File 301 must complete download");

    let file301_path = {
        let state = handle.read_state();
        state
            .get_file(301)
            .and_then(|f| f.local_path.clone())
            .expect("File 301 local path must exist")
    };
    assert!(file301_path.exists());

    handle.stop().await.expect("Failed to stop engine");
}

// ============================================================================
// TEST 6: Dynamic Network Proxy Configuration (R1)
// ============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_e2e_proxy_configuration_lifecycle() {
    let proxy_initial = ProxyConfig {
        server: "127.0.0.1".to_string(),
        port: 1080,
        proxy_type: ProxyType::Socks5,
        username: Some("user".to_string()),
        password: Some("pass".to_string()),
        is_enabled: true,
    };

    let config = Config {
        use_mock: true,
        proxy: Some(proxy_initial),
        ..Default::default()
    };

    let (handle, _event_rx) = Engine::start(config).await.expect("Engine start failed");

    // Remove proxy dynamically
    handle.set_proxy(None).await.expect("set_proxy(None) failed");

    // Reconfigure to HTTP proxy dynamically
    let proxy_http = ProxyConfig {
        server: "proxy.internal".to_string(),
        port: 8080,
        proxy_type: ProxyType::Http,
        username: None,
        password: None,
        is_enabled: true,
    };
    handle
        .set_proxy(Some(proxy_http))
        .await
        .expect("set_proxy(Some(Http)) failed");

    handle.stop().await.expect("Failed to stop engine");
}

// ============================================================================
// TEST 7: Adversarial Offline Navigation & Terminal Resize Stress (R2)
// ============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_e2e_adversarial_offline_navigation_and_input() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };
    let (handle, _event_rx) = Engine::start(config).await.expect("Engine start failed");
    let mut app = App::new(handle.clone());

    // Rapid navigation cycles without auth
    for _ in 0..100 {
        app.next_chat();
        app.previous_chat();
        app.cycle_panel_forward();
        app.cycle_panel_backward();
    }

    // Text input stress
    app.insert_char('H');
    app.insert_char('e');
    app.insert_char('l');
    app.insert_char('l');
    app.insert_char('o');
    app.insert_char(' ');
    app.insert_char('🦀'); // Multi-byte emoji
    app.insert_char('🚀');
    assert_eq!(app.input_text, "Hello 🦀🚀");

    app.delete_char_before_cursor();
    assert_eq!(app.input_text, "Hello 🦀");

    app.delete_word_backward();
    assert_eq!(app.input_text, "Hello ");

    app.clear_input();
    assert!(app.input_text.is_empty());

    // Stress terminal drawing across diverse extreme dimensions
    let test_dimensions = [
        (1, 1),
        (5, 5),
        (20, 10),
        (80, 24),
        (120, 40),
        (200, 60),
    ];

    for (width, height) in test_dimensions {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("Terminal init failed");
        let draw_res = terminal.draw(|f| ui::render(f, &mut app));
        assert!(
            draw_res.is_ok(),
            "Render must not panic at dimension {}x{}",
            width,
            height
        );
    }

    handle.stop().await.expect("Failed to stop engine");
}
