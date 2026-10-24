#![allow(dead_code)]

#[path = "../crates/tg-tui/src/text/mod.rs"]
mod text;

#[path = "../crates/tg-tui/src/ui/mod.rs"]
mod ui;

#[path = "../crates/tg-tui/src/app.rs"]
mod app;

#[path = "../crates/tg-tui/src/event.rs"]
mod event;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Duration;
use app::{ActivePanel, App};
use event::{AppEvent, EventHandler};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tg_core::{
    AuthState, ChatId, Config, ConnectionState, CoreEvent, Engine,
    Message, MessageContent, MessageId, MessageStatus, Sender,
};
use tokio::sync::broadcast;
use ui::chat_list::ChatFilterTab;
use ui::modals::{ActiveModal, ProxyModalState, ProxyTypeChoice};

/// Helper to spin up a mock engine and App instance.
async fn create_test_app() -> App {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };
    let (handle, _event_rx) = Engine::start(config).await.expect("Failed to start engine");
    App::new(handle)
}

fn make_dummy_message(chat_id: ChatId, msg_id: i64, text: &str) -> Message {
    Message {
        id: MessageId(msg_id),
        chat_id,
        sender: Sender::User(tg_core::UserId(1)),
        is_outgoing: false,
        date: 1700000000,
        reply_to_message_id: None,
        can_be_edited: true,
        can_be_deleted: true,
        content: MessageContent::Text {
            text: text.to_string(),
            entities: Vec::new(),
        },
        status: MessageStatus::Read,
    }
}

// ============================================================================
// SUITE 1: Terminal Resizing & Micro/Zero-Dimension Layout Stress
// ============================================================================

#[tokio::test]
async fn test_terminal_resizing_unauthenticated_all_dimensions() {
    let mut app = create_test_app().await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let dimensions: Vec<(u16, u16)> = vec![
        (0, 0),
        (0, 1),
        (1, 0),
        (1, 1),
        (2, 2),
        (10, 5),
        (5, 10),
        (10, 1),
        (1, 10),
        (20, 4),
        (40, 6),
        (79, 20),
        (80, 24),
        (119, 30),
        (120, 30),
        (200, 60),
    ];

    let panels = vec![ActivePanel::ChatList, ActivePanel::Messages, ActivePanel::Input];
    let modals = vec![ActiveModal::None, ActiveModal::Settings, ActiveModal::Proxy];

    for (w, h) in dimensions {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).expect("Failed to create TestBackend");

        for &panel in &panels {
            app.active_panel = panel;
            for modal in &modals {
                app.active_modal = modal.clone();

                terminal
                    .draw(|f| ui::render(f, &mut app))
                    .unwrap_or_else(|e| panic!("Draw failed at {}x{}, panel {:?}, modal {:?}: {:?}", w, h, panel, modal, e));
            }
        }
    }
}

#[tokio::test]
async fn test_terminal_resizing_authenticated_diagnostics() {
    let mut app = create_test_app().await;

    // Login via mock
    app.handle.send_phone_number("+1234567890").await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    app.handle.send_code("11111").await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(app.handle.read_state().auth_state, AuthState::Ready);

    let test_cases: Vec<(u16, u16, ActivePanel, ActiveModal)> = vec![
        // Micro/Zero dimensions
        (0, 0, ActivePanel::ChatList, ActiveModal::None),
        (0, 0, ActivePanel::Messages, ActiveModal::None),
        (0, 0, ActivePanel::Input, ActiveModal::None),
        (0, 1, ActivePanel::Input, ActiveModal::None),
        (1, 0, ActivePanel::Input, ActiveModal::None),
        (5, 0, ActivePanel::Input, ActiveModal::None),
        (10, 0, ActivePanel::Input, ActiveModal::None),
        (10, 1, ActivePanel::Input, ActiveModal::None),
        (10, 2, ActivePanel::Input, ActiveModal::None),
        (1, 1, ActivePanel::Input, ActiveModal::None),
        (10, 5, ActivePanel::ChatList, ActiveModal::None),
        (10, 5, ActivePanel::Messages, ActiveModal::None),
        (10, 5, ActivePanel::Input, ActiveModal::None),
        (5, 10, ActivePanel::Input, ActiveModal::None),
        (80, 24, ActivePanel::Input, ActiveModal::None),
    ];

    let mut failures = Vec::new();

    for (w, h, panel, modal) in test_cases {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).expect("Failed to create TestBackend");

        app.active_panel = panel;
        app.active_modal = modal.clone();

        let res = catch_unwind(AssertUnwindSafe(|| {
            let _ = terminal.draw(|f| ui::render(f, &mut app));
        }));

        if let Err(err) = res {
            let msg = if let Some(s) = err.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = err.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic payload".to_string()
            };
            failures.push(format!("Panic at {}x{}, panel={:?}, modal={:?}: {}", w, h, panel, modal, msg));
        }
    }

    if !failures.is_empty() {
        eprintln!("\n=== EMPIRICAL LAYOUT RESIZING PANIC REPORT ===");
        for f in &failures {
            eprintln!("  [FAIL] {}", f);
        }
        eprintln!("===============================================\n");
        panic!("Terminal resizing triggered {} panics: {:?}", failures.len(), failures);
    }
}

// ============================================================================
// SUITE 2: Modal Transitions & State Navigation Stress
// ============================================================================

#[tokio::test]
async fn test_modal_transitions_rapid_switching() {
    let mut app = create_test_app().await;

    // 1. Rapidly switch modal states
    for i in 0..1000 {
        match i % 3 {
            0 => app.active_modal = ActiveModal::None,
            1 => app.active_modal = ActiveModal::Settings,
            _ => app.active_modal = ActiveModal::Proxy,
        }
    }

    // 2. Stress ProxyModalState
    let mut proxy_state = ProxyModalState::default();
    assert_eq!(proxy_state.selected_field, 0);

    // Rapid field cycling forward and backward
    for _ in 0..500 {
        proxy_state.selected_field = (proxy_state.selected_field + 1) % 7;
    }
    for _ in 0..500 {
        proxy_state.selected_field = if proxy_state.selected_field == 0 {
            6
        } else {
            proxy_state.selected_field - 1
        };
    }

    // Rapid protocol type cycling
    for _ in 0..300 {
        proxy_state.proxy_type = match proxy_state.proxy_type {
            ProxyTypeChoice::Socks5 => ProxyTypeChoice::Http,
            ProxyTypeChoice::Http => ProxyTypeChoice::Mtproto,
            ProxyTypeChoice::Mtproto => ProxyTypeChoice::Socks5,
        };
    }

    // Extreme port inputs
    proxy_state.port = "99999999999999999999".to_string();
    let cfg = proxy_state.to_proxy_config();
    assert!(cfg.is_some(), "to_proxy_config should gracefully fallback on invalid port");
    assert_eq!(cfg.unwrap().port, 1080, "Fallback port should be 1080");

    proxy_state.port = "-100".to_string();
    assert_eq!(proxy_state.to_proxy_config().unwrap().port, 1080);

    proxy_state.port = "".to_string();
    assert_eq!(proxy_state.to_proxy_config().unwrap().port, 1080);

    // Pathological large inputs
    proxy_state.server = "a".repeat(10_000);
    proxy_state.username = "user_🦀_test".repeat(100);
    proxy_state.password = "secret_🌟_123".repeat(100);
    proxy_state.secret = "mtproto_secret_hex".repeat(50);
    let large_cfg = proxy_state.to_proxy_config().unwrap();
    assert_eq!(large_cfg.server.len(), 10_000);
    assert!(large_cfg.username.is_some());
}

// ============================================================================
// SUITE 3: Tab Switching, Selection Clamping & Scroll Stress
// ============================================================================

#[tokio::test]
async fn test_tab_switching_selection_clamping_and_scrolling() {
    let mut app = create_test_app().await;

    // Switch tabs on empty state
    for _ in 0..100 {
        app.switch_tab(ChatFilterTab::All);
        app.switch_tab(ChatFilterTab::Personal);
        app.switch_tab(ChatFilterTab::Groups);
        app.switch_tab(ChatFilterTab::Channels);
    }
    assert_eq!(app.selected_chat_index, 0);

    // Panel cycling forward and backward
    for _ in 0..300 {
        app.cycle_panel_forward();
    }
    for _ in 0..300 {
        app.cycle_panel_backward();
    }

    // Scroll operations
    app.scroll_messages_up(10);
    assert_eq!(app.message_scroll, 10);
    app.scroll_messages_up(100);
    assert_eq!(app.message_scroll, 110);
    app.scroll_messages_down(50);
    assert_eq!(app.message_scroll, 60);
    app.scroll_messages_down(1000); // saturating sub
    assert_eq!(app.message_scroll, 0);

    // Extreme scroll delta
    app.scroll_messages_up(usize::MAX / 2);
    app.scroll_messages_up(usize::MAX / 2); // saturating add prevents overflow
    app.scroll_messages_down(usize::MAX);
    assert_eq!(app.message_scroll, 0);
}

// ============================================================================
// SUITE 4: Text Input Cursor Operations & Pathological Unicode Stress
// ============================================================================

#[tokio::test]
async fn test_input_box_cursor_stress_and_unicode() {
    let mut app = create_test_app().await;

    // 1. Empty input operations
    app.delete_char_before_cursor();
    app.delete_char_at_cursor();
    app.delete_word_backward();
    app.move_cursor_left();
    app.move_cursor_right();
    assert_eq!(app.cursor_pos, 0);
    assert!(app.input_text.is_empty());

    // 2. Typing ASCII
    for c in "Hello World".chars() {
        app.insert_char(c);
    }
    assert_eq!(app.input_text, "Hello World");
    assert_eq!(app.cursor_pos, 11);

    // Move home and insert at start
    app.move_cursor_home();
    assert_eq!(app.cursor_pos, 0);
    app.insert_char('>');
    assert_eq!(app.input_text, ">Hello World");
    assert_eq!(app.cursor_pos, 1);

    // Move end and delete word backward
    app.move_cursor_end();
    assert_eq!(app.cursor_pos, 12);
    app.delete_word_backward(); // deletes "World"
    assert_eq!(app.input_text, ">Hello ");
    app.delete_word_backward(); // deletes ">Hello "
    assert_eq!(app.input_text, "");

    // 3. Multibyte Unicode & Emojis
    app.clear_input();
    let complex_text = "Привет 🦀 世界 🚀 123";
    for c in complex_text.chars() {
        app.insert_char(c);
    }
    assert_eq!(app.input_text, complex_text);
    assert_eq!(app.cursor_pos, complex_text.chars().count());

    // Step backwards through multi-byte chars without UTF-8 slicing panics
    while app.cursor_pos > 0 {
        app.delete_char_before_cursor();
    }
    assert!(app.input_text.is_empty());
    assert_eq!(app.cursor_pos, 0);
}

// ============================================================================
// SUITE 5: Event Multiplexer & High-Burst CoreEvent Stress
// ============================================================================

#[tokio::test]
async fn test_event_multiplexer_high_load_bursts() {
    let mut app = create_test_app().await;

    // Send a flood of diverse CoreEvents to verify zero crashes
    for i in 0..2000 {
        let event = match i % 5 {
            0 => CoreEvent::ConnectionStateChanged(ConnectionState::Ready),
            1 => CoreEvent::ErrorOccurred(format!("Simulated transient error #{}", i)),
            2 => CoreEvent::ChatListUpdated(vec![ChatId(1), ChatId(2)]),
            3 => CoreEvent::MessageReceived(Box::new(make_dummy_message(ChatId(1), i as i64, "Burst msg"))),
            _ => CoreEvent::MessageUpdated(Box::new(make_dummy_message(ChatId(1), i as i64, "Edited msg"))),
        };
        app.handle_core_event(event);
    }

    // Last error event was at i = 1996 (1996 % 5 == 1)
    assert_eq!(app.status_message, "Simulated transient error #1996");
    assert!(app.is_status_error);
}

#[tokio::test]
async fn test_event_handler_tokio_broadcast_lag_resilience() {
    let (tx, rx) = broadcast::channel(16);
    let mut handler = EventHandler::new_headless(rx, Duration::from_millis(10));

    // Flood the channel with 100 events while handler is not polling
    for i in 0..100 {
        let _ = tx.send(CoreEvent::ErrorOccurred(format!("Flooding event #{}", i)));
    }

    // Now poll the event handler: it should catch RecvError::Lagged and yield Tick rather than dying
    let mut received_tick_or_event = false;
    for _ in 0..10 {
        if let Some(event) = handler.next().await {
            match event {
                AppEvent::Tick | AppEvent::Core(_) => {
                    received_tick_or_event = true;
                    break;
                }
                _ => {}
            }
        }
    }
    assert!(received_tick_or_event, "EventHandler should recover from broadcast channel lag");
}
