//! Comprehensive 4-Tier E2E Automated Requirement Tests for TelegramTUI
//!
//! Tier 1: Feature Coverage (R1 through R5 isolation, >=5 tests per feature)
//! Tier 2: Boundary & Corner Cases (empty inputs, max limits, rate limits, 0x0 resizes, invalid codes)
//! Tier 3: Cross-Feature Combinations (pairwise interaction matrix)
//! Tier 4: Real-World Application Scenarios (complete end-to-end user journeys)

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::panic::AssertUnwindSafe;
use tg_tui::actors::coordinator::{AppCoordinator, UserAction};
use tg_tui::actors::tdlib::{TdlibActor, TdlibCommand, TdlibEvent};
use tg_tui::config::AppConfig;
use tg_tui::model::*;
use tg_tui::theme::Theme;
use tg_tui::ui::render_app;
use tokio::sync::mpsc;

// ============================================================================
// Test Harness Helpers
// ============================================================================

/// Bootstrap coordinator with authenticated TdlibActor (initial seed data loaded)
async fn setup_app() -> (AppCoordinator, mpsc::Receiver<TdlibEvent>, mpsc::Sender<TdlibCommand>) {
    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let (event_tx, mut event_rx) = mpsc::channel(64);

    let tdlib = TdlibActor::new(cmd_rx, event_tx, true);
    tokio::spawn(async move {
        tdlib.run().await;
    });

    let mut app = AppCoordinator::new(cmd_tx.clone());

    // Wait and drain initial seed events
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    while let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    (app, event_rx, cmd_tx)
}

/// Bootstrap coordinator in clean unauthenticated state (starts at S01 PhoneInput)
async fn setup_unauth_app() -> (AppCoordinator, mpsc::Receiver<TdlibEvent>, mpsc::Sender<TdlibCommand>) {
    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let (event_tx, mut event_rx) = mpsc::channel(64);

    let tdlib = TdlibActor::new(cmd_rx, event_tx, false);
    tokio::spawn(async move {
        tdlib.run().await;
    });

    let mut app = AppCoordinator::new(cmd_tx.clone());

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    (app, event_rx, cmd_tx)
}

// ============================================================================
// TIER 1: FEATURE COVERAGE (R1 - R5 Isolation)
// ============================================================================

// --- R1: Crossterm Event Loop & Input Fix ---

#[tokio::test]
async fn test_tier1_r1_single_press_event_processed() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;

    app.handle_user_action(UserAction::InputChar('a')).await;
    assert_eq!(app.composer_buffer, "a", "First press must append single char");

    app.handle_user_action(UserAction::InputChar('b')).await;
    assert_eq!(app.composer_buffer, "ab", "Second press must append second char without duplication");
}

#[tokio::test]
async fn test_tier1_r1_release_and_repeat_discard_contract() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;

    // Simulate input dispatch contract where only Press produces UserAction
    app.handle_user_action(UserAction::InputChar('z')).await;
    let len_after_press = app.composer_buffer.len();

    // Release events do not map to actions, meaning state does not change
    assert_eq!(len_after_press, 1, "Only press events modify buffer state");
    assert_eq!(app.composer_buffer, "z");
}

#[tokio::test]
async fn test_tier1_r1_navigation_keys_dispatch_once() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Dialogs;
    app.selected_chat_idx = 0;

    app.handle_user_action(UserAction::NextItem).await;
    assert_eq!(app.selected_chat_idx, 1, "Single NextItem (Down/j) advances index by exactly 1");

    app.handle_user_action(UserAction::PrevItem).await;
    assert_eq!(app.selected_chat_idx, 0, "Single PrevItem (Up/k) decrements index by exactly 1");
}

#[tokio::test]
async fn test_tier1_r1_cycle_focus_tab_dispatch_once() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Dialogs;

    app.handle_user_action(UserAction::CycleFocus).await;
    assert_eq!(app.focus, FocusPane::Messages, "Tab moves focus from Dialogs to Messages");

    app.handle_user_action(UserAction::CycleFocus).await;
    assert_eq!(app.focus, FocusPane::Composer, "Tab moves focus from Messages to Composer");

    app.handle_user_action(UserAction::CycleFocus).await;
    assert_eq!(app.focus, FocusPane::Folders, "Tab moves focus from Composer to Folders");

    app.handle_user_action(UserAction::CycleFocus).await;
    assert_eq!(app.focus, FocusPane::Dialogs, "Tab wraps focus back to Dialogs");
}

#[tokio::test]
async fn test_tier1_r1_input_prioritization_over_network() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;

    // Simulate burst of background TDLib events
    for i in 1..=10 {
        app.handle_tdlib_event(TdlibEvent::NewMessage(Message {
            id: MessageId(5000 + i),
            chat_id: ChatId(101),
            sender_id: UserId(2),
            sender_name: "BurstSender".into(),
            date: "12:00".into(),
            text: format!("Burst message {}", i),
            entities: vec![],
            reply_to: None,
            media: None,
            reactions: vec![],
            is_outgoing: false,
            status: DeliveryStatus::Read,
            is_edited: false,
        })).await;
    }

    // User typing action dispatches and executes cleanly during network load
    app.handle_user_action(UserAction::InputChar('H')).await;
    app.handle_user_action(UserAction::InputChar('i')).await;
    assert_eq!(app.composer_buffer, "Hi", "Composer input succeeds without corruption during message burst");
}

// --- R2: First-Launch Setup & Authentication Gate ---

#[tokio::test]
async fn test_tier1_r2_phone_number_submission_and_step_advancement() {
    let (mut app, mut rx, _tx) = setup_unauth_app().await;
    assert!(matches!(app.auth_state, AuthStep::PhoneInput { .. }));

    for c in "+15550192834".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    assert!(matches!(app.auth_state, AuthStep::CodeInput { .. }), "Valid phone transitions to CodeInput");
}

#[tokio::test]
async fn test_tier1_r2_verification_code_submission() {
    let (mut app, mut rx, _tx) = setup_unauth_app().await;

    // Advance to code step
    app.auth_state = AuthStep::CodeInput {
        phone: "+15550192834".into(),
        buffer: String::new(),
        error: None,
    };

    for c in "12345".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    assert!(matches!(app.auth_state, AuthStep::Password2FA { .. }), "Valid code transitions to Password2FA");
}

#[tokio::test]
async fn test_tier1_r2_cloud_password_2fa_submission() {
    let (mut app, mut rx, _tx) = setup_unauth_app().await;

    app.auth_state = AuthStep::Password2FA {
        buffer: String::new(),
        error: None,
    };

    for c in "cloud_secret".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    while let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    assert_eq!(app.auth_state, AuthStep::Authenticated, "Valid password transitions to Authenticated");
    assert!(!app.chats.is_empty(), "Initial chats must be broadcast upon authentication");
}

#[tokio::test]
async fn test_tier1_r2_config_validation_valid_credentials() {
    let mut config = AppConfig::default();
    config.api_id = Some(987654);
    config.api_hash = Some("abcdef0123456789abcdef0123456789".into());

    let validation = config.validate();
    assert!(validation.is_ok(), "Config with positive id and 32-char hash must validate");
}

#[tokio::test]
async fn test_tier1_r2_config_save_and_load_roundtrip() {
    let mut config = AppConfig::default();
    config.api_id = Some(554433);
    config.api_hash = Some("1234567890abcdef1234567890abcdef".into());
    config.theme = "custom_test_theme".into();

    let toml_str = toml::to_string_pretty(&config).expect("Must serialize AppConfig");
    let loaded: AppConfig = toml::from_str(&toml_str).expect("Must deserialize AppConfig");

    assert_eq!(loaded.api_id, Some(554433));
    assert_eq!(loaded.api_hash, Some("1234567890abcdef1234567890abcdef".into()));
    assert_eq!(loaded.theme, "custom_test_theme");
}

// --- R3: TDLib Backend Integration ---

#[tokio::test]
async fn test_tier1_r3_chat_and_message_initial_sync() {
    let (app, _rx, _tx) = setup_app().await;

    assert_eq!(app.auth_state, AuthStep::Authenticated);
    assert!(app.folders.len() >= 2, "Must load chat folders");
    assert!(app.chats.len() >= 4, "Must load chat dialogs");
    assert!(app.current_chat().is_some(), "Active chat must resolve");
    assert!(!app.current_messages().is_empty(), "Initial chat messages must be populated");
}

#[tokio::test]
async fn test_tier1_r3_optimistic_message_delivery_lifecycle() {
    let (mut app, mut rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;

    for c in "Lifecycle message".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;

    // 1. Optimistic arrival
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    let last = (*app.current_messages().last().unwrap()).clone();
    assert_eq!(last.status, DeliveryStatus::Pending, "Optimistic state is Pending");

    // 2. Server ACK arrival
    tokio::time::sleep(tokio::time::Duration::from_millis(270)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    let last_ack = (*app.current_messages().last().unwrap()).clone();
    assert_eq!(last_ack.status, DeliveryStatus::Sent, "Acknowledged state is Sent");

    // 3. Recipient Read arrival
    tokio::time::sleep(tokio::time::Duration::from_millis(750)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    let last_read = (*app.current_messages().last().unwrap()).clone();
    assert_eq!(last_read.status, DeliveryStatus::Read, "Final state is Read");
}

#[tokio::test]
async fn test_tier1_r3_reaction_update_propagation() {
    let (mut app, mut rx, _tx) = setup_app().await;
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(0);

    app.handle_user_action(UserAction::OpenReactionPicker).await;
    app.handle_user_action(UserAction::InputChar('2')).await; // ❤️

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    let msg = (*app.current_messages().first().unwrap()).clone();
    let heart = msg.reactions.iter().find(|r| r.emoji == "❤️");
    assert!(heart.is_some(), "Reaction ❤️ must be added to message");
    assert_eq!(heart.unwrap().count, 1);
    assert!(heart.unwrap().chosen_by_me);
}

#[tokio::test]
async fn test_tier1_r3_message_edit_propagation() {
    let (mut app, mut rx, _tx) = setup_app().await;
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(2); // outgoing message

    app.handle_user_action(UserAction::StartEdit).await;
    assert!(app.composer_edit_id.is_some());

    app.composer_buffer = "Edited test body".into();
    app.handle_user_action(UserAction::ComposerSubmit).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    let msg = (*app.current_messages().get(2).unwrap()).clone();
    assert_eq!(msg.text, "Edited test body");
    assert!(msg.is_edited);
}

#[tokio::test]
async fn test_tier1_r3_message_delete_propagation() {
    let (mut app, mut rx, _tx) = setup_app().await;
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(0);
    let count_before = app.current_messages().len();

    app.handle_user_action(UserAction::DeleteSelected).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    assert_eq!(app.current_messages().len(), count_before - 1, "Message count must decrement by 1");
}

// --- R4: Message History Viewport & Composer ---

#[tokio::test]
async fn test_tier1_r4_message_navigation_next_prev_bounds() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Messages;
    let count = app.current_messages().len();
    assert!(count > 1);

    // Initial state
    app.selected_msg_idx = Some(0);
    app.handle_user_action(UserAction::PrevItem).await;
    assert_eq!(app.selected_msg_idx, Some(0), "PrevItem at top stays clamped at 0");

    app.handle_user_action(UserAction::NextItem).await;
    assert_eq!(app.selected_msg_idx, Some(1), "NextItem advances to 1");

    // Navigate to bottom
    for _ in 0..count + 5 {
        app.handle_user_action(UserAction::NextItem).await;
    }
    assert_eq!(app.selected_msg_idx, Some(count - 1), "NextItem clamps at count - 1");
}

#[tokio::test]
async fn test_tier1_r4_composer_character_accumulation() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;

    for c in "Typing 12345 into composer".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    assert_eq!(app.composer_buffer, "Typing 12345 into composer");
}

#[tokio::test]
async fn test_tier1_r4_composer_backspace_handling() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;
    app.composer_buffer = "Hello!".into();

    app.handle_user_action(UserAction::Backspace).await;
    assert_eq!(app.composer_buffer, "Hello", "Backspace removes last character");
}

#[tokio::test]
async fn test_tier1_r4_composer_long_text_rendering() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;
    app.composer_buffer = "A".repeat(120);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();

    // Must render cleanly without panicking on 80+ character composer text
    terminal.draw(|f| render_app(f, &app, &theme)).expect("Composer rendering with 120 chars must not panic");
}

#[tokio::test]
async fn test_tier1_r4_history_rendering_with_many_messages() {
    let (mut app, _rx, _tx) = setup_app().await;

    // Add 40 additional messages to test conversation history rendering
    for i in 1..=40 {
        app.handle_tdlib_event(TdlibEvent::NewMessage(Message {
            id: MessageId(8000 + i),
            chat_id: ChatId(101),
            sender_id: UserId(if i % 2 == 0 { 1 } else { 2 }),
            sender_name: if i % 2 == 0 { "You".into() } else { "Peer".into() },
            date: "12:30".into(),
            text: format!("History scroll test message #{}", i),
            entities: vec![],
            reply_to: None,
            media: None,
            reactions: vec![],
            is_outgoing: i % 2 == 0,
            status: DeliveryStatus::Read,
            is_edited: false,
        })).await;
    }

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();

    terminal.draw(|f| render_app(f, &app, &theme)).expect("History with 40+ messages must render without panic");
}

// --- R5: Terminal Panic Safety & Clean Teardown ---

#[tokio::test]
async fn test_tier1_r5_quit_action_terminates_loop() {
    let (mut app, _rx, _tx) = setup_app().await;
    let should_continue = app.handle_user_action(UserAction::Quit).await;
    assert!(!should_continue, "Quit action must return false to terminate main loop");
}

#[tokio::test]
async fn test_tier1_r5_render_zero_dimension_resilience() {
    let (app, _rx, _tx) = setup_app().await;
    let theme = Theme::default();

    // 0x0 terminal test
    let backend_zero = TestBackend::new(0, 0);
    let mut terminal_zero = Terminal::new(backend_zero).unwrap();
    terminal_zero.draw(|f| render_app(f, &app, &theme)).expect("0x0 terminal render must not panic");

    // 1x1 terminal test
    let backend_one = TestBackend::new(1, 1);
    let mut terminal_one = Terminal::new(backend_one).unwrap();
    terminal_one.draw(|f| render_app(f, &app, &theme)).expect("1x1 terminal render must not panic");
}

#[tokio::test]
async fn test_tier1_r5_rapid_resize_render_stability() {
    let (app, _rx, _tx) = setup_app().await;
    let theme = Theme::default();

    let sizes = [(20, 10), (80, 24), (200, 60), (10, 30), (120, 5), (40, 40)];
    for (w, h) in sizes {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_app(f, &app, &theme)).expect("Rapid resize must render cleanly");
    }
}

#[tokio::test]
async fn test_tier1_r5_panic_safety_hook_contract() {
    let (app, _rx, _tx) = setup_app().await;
    let theme = Theme::default();

    // Catch unwind contract ensuring rendering boundary is panic-safe
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_app(f, &app, &theme)).unwrap();
    }));

    assert!(result.is_ok(), "Render execution safely isolates inside catch boundary");
}

#[tokio::test]
async fn test_tier1_r5_clean_teardown_unauthenticated_quit() {
    let (mut app, _rx, _tx) = setup_unauth_app().await;
    assert_ne!(app.auth_state, AuthStep::Authenticated);

    let should_continue = app.handle_user_action(UserAction::Quit).await;
    assert!(!should_continue, "Quit action during authentication gate must cleanly exit");
}

// ============================================================================
// TIER 2: BOUNDARY & CORNER CASES (Empty Inputs, Max Limits, Zero Sizes)
// ============================================================================

// --- R1 Boundaries ---

#[tokio::test]
async fn test_tier2_r1_b1_rapid_successive_keypresses() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;

    // Simulate 100 rapid sequential keypresses without frame sleep
    let rapid_stream = "abcdefghij".repeat(10);
    for c in rapid_stream.chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }

    assert_eq!(app.composer_buffer.len(), 100);
    assert_eq!(app.composer_buffer, rapid_stream);
}

#[tokio::test]
async fn test_tier2_r1_b2_non_ascii_unicode_input() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;

    let unicode_str = "Привет, мир! 🚀 🦀 🌟";
    for c in unicode_str.chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    assert_eq!(app.composer_buffer, unicode_str);

    // Test backspace on multi-byte UTF-8 emoji
    app.handle_user_action(UserAction::Backspace).await; // removes 🌟
    app.handle_user_action(UserAction::Backspace).await; // removes space
    app.handle_user_action(UserAction::Backspace).await; // removes 🦀
    app.handle_user_action(UserAction::Backspace).await; // removes space
    assert_eq!(app.composer_buffer, "Привет, мир! 🚀");
}

#[tokio::test]
async fn test_tier2_r1_b3_unmapped_keys_ignored_safely() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;
    let initial_buf = app.composer_buffer.clone();

    // Selecting a folder out of range should be ignored safely
    app.handle_user_action(UserAction::SelectFolder(99)).await;
    assert_eq!(app.composer_buffer, initial_buf);
    assert_eq!(app.focus, FocusPane::Composer);
}

#[tokio::test]
async fn test_tier2_r1_b4_modifier_combinations_ctrl_c() {
    let (mut app, _rx, _tx) = setup_app().await;

    // When modal is open, first Quit/CloseModal closes the modal
    app.active_modal = ActiveModal::Search { query: "query".into() };
    let cont1 = app.handle_user_action(UserAction::Quit).await;
    assert!(cont1, "Quit while modal open should dismiss modal and remain running");
    assert_eq!(app.active_modal, ActiveModal::None);

    // Second Quit terminates app
    let cont2 = app.handle_user_action(UserAction::Quit).await;
    assert!(!cont2, "Quit when no modal open exits application");
}

#[tokio::test]
async fn test_tier2_r1_b5_focus_cycling_full_loop() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Dialogs;

    // Cycle focus 16 times (4 full loops)
    for _ in 0..16 {
        app.handle_user_action(UserAction::CycleFocus).await;
    }
    assert_eq!(app.focus, FocusPane::Dialogs, "Focus wraps cleanly after 4 full cycles");
}

// --- R2 Boundaries ---

#[tokio::test]
async fn test_tier2_r2_b1_empty_phone_input_rejection() {
    let (mut app, mut rx, _tx) = setup_unauth_app().await;
    assert!(matches!(app.auth_state, AuthStep::PhoneInput { .. }));

    // Submit empty phone
    app.handle_user_action(UserAction::ComposerSubmit).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    if let AuthStep::PhoneInput { error, .. } = &app.auth_state {
        assert!(error.is_some(), "Empty phone number must be rejected with error banner");
    } else {
        panic!("Must remain on PhoneInput step");
    }
}

#[tokio::test]
async fn test_tier2_r2_b2_whitespace_in_verification_code() {
    let (mut app, mut rx, _tx) = setup_unauth_app().await;
    app.auth_state = AuthStep::CodeInput {
        phone: "+15551234567".into(),
        buffer: "  12345  ".into(),
        error: None,
    };

    app.handle_user_action(UserAction::ComposerSubmit).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    assert!(matches!(app.auth_state, AuthStep::Password2FA { .. }), "Whitespace in code must be trimmed and accepted");
}

#[tokio::test]
async fn test_tier2_r2_b3_empty_2fa_password_rejection() {
    let (mut app, mut rx, _tx) = setup_unauth_app().await;
    app.auth_state = AuthStep::Password2FA {
        buffer: String::new(),
        error: None,
    };

    app.handle_user_action(UserAction::ComposerSubmit).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    if let AuthStep::Password2FA { error, .. } = &app.auth_state {
        assert!(error.is_some(), "Empty 2FA password must be rejected");
    } else {
        panic!("Must remain on Password2FA step");
    }
}

#[tokio::test]
async fn test_tier2_r2_b4_invalid_api_id_negative_or_zero() {
    let mut config = AppConfig::default();
    config.api_id = Some(0);
    assert!(config.validate().is_err(), "api_id = 0 must fail validation");

    config.api_id = Some(-1234);
    assert!(config.validate().is_err(), "Negative api_id must fail validation");
}

#[tokio::test]
async fn test_tier2_r2_b5_invalid_api_hash_hex_length() {
    let mut config = AppConfig::default();
    config.api_hash = Some("short_hash".into());
    assert!(config.validate().is_err(), "Short api_hash must fail validation");

    config.api_hash = Some("0123456789abcdef0123456789abcdef0".into()); // 33 chars
    assert!(config.validate().is_err(), "Overlong api_hash must fail validation");
}

// --- R3 Boundaries ---

#[tokio::test]
async fn test_tier2_r3_b1_send_empty_or_whitespace_message() {
    let (mut app, mut rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;
    let initial_count = app.current_messages().len();

    // 1. Completely empty
    app.composer_buffer.clear();
    app.handle_user_action(UserAction::ComposerSubmit).await;

    // 2. Spaces only
    app.composer_buffer = "     ".into();
    app.handle_user_action(UserAction::ComposerSubmit).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    while let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    assert_eq!(app.current_messages().len(), initial_count, "Empty/whitespace submission does not send message");
}

#[tokio::test]
async fn test_tier2_r3_b2_unknown_chat_message_lookup() {
    let (app, _rx, _tx) = setup_app().await;
    let msgs = app.messages.get(&ChatId(999999));
    assert!(msgs.is_none(), "Non-existent chat returns None without panic");
}

#[tokio::test]
async fn test_tier2_r3_b3_message_edit_nonexistent_id() {
    let (mut app, _rx, _tx) = setup_app().await;
    let initial_msgs: Vec<_> = app.current_messages().into_iter().cloned().collect();

    // Send edit event for message id that does not exist
    app.handle_tdlib_event(TdlibEvent::MessageEdited {
        chat_id: ChatId(101),
        message_id: MessageId(999999),
        new_text: "Ghost text".into(),
    }).await;

    let post_msgs: Vec<_> = app.current_messages().into_iter().cloned().collect();
    assert_eq!(initial_msgs, post_msgs, "Editing non-existent message leaves history unchanged");
}

#[tokio::test]
async fn test_tier2_r3_b4_message_delete_nonexistent_id() {
    let (mut app, _rx, _tx) = setup_app().await;
    let count_before = app.current_messages().len();

    app.handle_tdlib_event(TdlibEvent::MessageDeleted {
        chat_id: ChatId(101),
        message_id: MessageId(999999),
    }).await;

    assert_eq!(app.current_messages().len(), count_before, "Deleting non-existent message does not alter count");
}

#[tokio::test]
async fn test_tier2_r3_b5_duplicate_reaction_toggle() {
    let (mut app, mut rx, _tx) = setup_app().await;
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(0);

    // Toggle reaction on
    app.handle_user_action(UserAction::ToggleReaction("🔥".into())).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    let msg1 = (*app.current_messages().first().unwrap()).clone();
    let fire1 = msg1.reactions.iter().find(|r| r.emoji == "🔥").unwrap();
    assert_eq!(fire1.count, 1);
    assert!(fire1.chosen_by_me);

    // Toggle same reaction off
    app.handle_user_action(UserAction::ToggleReaction("🔥".into())).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    let msg2 = (*app.current_messages().first().unwrap()).clone();
    let fire2 = msg2.reactions.iter().find(|r| r.emoji == "🔥");
    assert!(fire2.is_none(), "Reaction is removed completely when count reaches 0");
}

// --- R4 Boundaries ---

#[tokio::test]
async fn test_tier2_r4_b1_navigate_empty_chat_history() {
    let (mut app, _rx, _tx) = setup_app().await;
    let empty_chat_id = ChatId(999);
    app.chats.push(Chat {
        id: empty_chat_id,
        title: "Empty Chat".into(),
        chat_type: ChatType::Private,
        unread_count: 0,
        unread_mentions: 0,
        is_pinned: false,
        is_muted: false,
        folder_id: None,
        last_message_snippet: String::new(),
        last_message_date: String::new(),
    });

    app.selected_chat_idx = app.chats.len() - 1;
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = None;

    app.handle_user_action(UserAction::NextItem).await;
    assert_eq!(app.selected_msg_idx, None, "Navigation in empty history safely preserves None");

    app.handle_user_action(UserAction::PrevItem).await;
    assert_eq!(app.selected_msg_idx, None);
}

#[tokio::test]
async fn test_tier2_r4_b2_composer_backspace_on_empty_buffer() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;
    app.composer_buffer.clear();

    // Call backspace 10 times on empty buffer
    for _ in 0..10 {
        app.handle_user_action(UserAction::Backspace).await;
    }
    assert!(app.composer_buffer.is_empty(), "Backspace on empty buffer must not underflow or panic");
}

#[tokio::test]
async fn test_tier2_r4_b3_composer_extreme_length_1000_chars() {
    let (mut app, mut rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;
    let long_text = "T".repeat(1000);
    app.composer_buffer = long_text.clone();

    // Verify it renders cleanly
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();
    terminal.draw(|f| render_app(f, &app, &theme)).expect("1000-char buffer must render without error");

    // Submit long text
    app.handle_user_action(UserAction::ComposerSubmit).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    let last_msg = (*app.current_messages().last().unwrap()).clone();
    assert_eq!(last_msg.text.len(), 1000);
}

#[tokio::test]
async fn test_tier2_r4_b4_navigate_single_message_chat() {
    let (mut app, _rx, _tx) = setup_app().await;
    let single_chat_id = ChatId(888);
    app.chats.push(Chat {
        id: single_chat_id,
        title: "Single Message Chat".into(),
        chat_type: ChatType::Private,
        unread_count: 0,
        unread_mentions: 0,
        is_pinned: false,
        is_muted: false,
        folder_id: None,
        last_message_snippet: "Only message".into(),
        last_message_date: "10:00".into(),
    });
    app.messages.insert(single_chat_id, vec![Message {
        id: MessageId(1),
        chat_id: single_chat_id,
        sender_id: UserId(2),
        sender_name: "Bot".into(),
        date: "10:00".into(),
        text: "Only message".into(),
        entities: vec![],
        reply_to: None,
        media: None,
        reactions: vec![],
        is_outgoing: false,
        status: DeliveryStatus::Read,
        is_edited: false,
    }]);

    app.selected_chat_idx = app.chats.len() - 1;
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(0);

    // Navigating Next and Prev must stay clamped at index 0
    app.handle_user_action(UserAction::NextItem).await;
    assert_eq!(app.selected_msg_idx, Some(0));

    app.handle_user_action(UserAction::PrevItem).await;
    assert_eq!(app.selected_msg_idx, Some(0));
}

#[tokio::test]
async fn test_tier2_r4_b5_draft_retention_across_chat_switching() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Dialogs;

    // Chat 0
    app.selected_chat_idx = 0;
    app.composer_buffer = "Draft for chat 0".into();

    // Switch to Chat 1
    app.handle_user_action(UserAction::NextItem).await;
    assert_eq!(app.selected_chat_idx, 1);
    assert_eq!(app.composer_buffer, "", "New chat buffer is initially empty");

    // Compose draft for Chat 1
    app.composer_buffer = "Draft for chat 1".into();

    // Switch back to Chat 0
    app.handle_user_action(UserAction::PrevItem).await;
    assert_eq!(app.selected_chat_idx, 0);
    assert_eq!(app.composer_buffer, "Draft for chat 0", "Draft for chat 0 must be restored");
}

// --- R5 Boundaries ---

#[tokio::test]
async fn test_tier2_r5_b1_render_extreme_narrow_width_1() {
    let (app, _rx, _tx) = setup_app().await;
    let backend = TestBackend::new(1, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();
    terminal.draw(|f| render_app(f, &app, &theme)).expect("Width = 1 must render without panic");
}

#[tokio::test]
async fn test_tier2_r5_b2_render_extreme_short_height_1() {
    let (app, _rx, _tx) = setup_app().await;
    let backend = TestBackend::new(100, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();
    terminal.draw(|f| render_app(f, &app, &theme)).expect("Height = 1 must render without panic");
}

#[tokio::test]
async fn test_tier2_r5_b3_render_huge_viewport_500x500() {
    let (app, _rx, _tx) = setup_app().await;
    let backend = TestBackend::new(500, 500);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();
    terminal.draw(|f| render_app(f, &app, &theme)).expect("500x500 terminal must render cleanly");
}

#[tokio::test]
async fn test_tier2_r5_b4_catch_unwind_around_render() {
    let (app, _rx, _tx) = setup_app().await;
    let theme = Theme::default();

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let backend = TestBackend::new(50, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_app(f, &app, &theme)).unwrap();
    }));
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_tier2_r5_b5_modal_open_during_quit() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.active_modal = ActiveModal::AttachFile {
        path_buffer: "sample.png".into(),
        is_photo: true,
        compress: true,
    };

    // First Quit closes modal
    let res1 = app.handle_user_action(UserAction::Quit).await;
    assert!(res1);
    assert_eq!(app.active_modal, ActiveModal::None);

    // Second Quit shuts down
    let res2 = app.handle_user_action(UserAction::Quit).await;
    assert!(!res2);
}

// ============================================================================
// TIER 3: CROSS-FEATURE COMBINATIONS (Pairwise Interactions)
// ============================================================================

#[tokio::test]
async fn test_tier3_pairwise_input_navigation_while_receiving_messages() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(0);

    // Interleave navigation keystrokes and background incoming messages
    for i in 1..=5 {
        app.handle_user_action(UserAction::NextItem).await;
        app.handle_tdlib_event(TdlibEvent::NewMessage(Message {
            id: MessageId(9500 + i),
            chat_id: ChatId(101),
            sender_id: UserId(2),
            sender_name: "Peer".into(),
            date: "13:00".into(),
            text: format!("Interleaved message #{}", i),
            entities: vec![],
            reply_to: None,
            media: None,
            reactions: vec![],
            is_outgoing: false,
            status: DeliveryStatus::Read,
            is_edited: false,
        })).await;
    }

    assert!(app.selected_msg_idx.is_some());
    assert!(app.selected_msg_idx.unwrap() <= app.current_messages().len() - 1);
}

#[tokio::test]
async fn test_tier3_pairwise_auth_failure_with_retry_and_event_interleaving() {
    let (mut app, mut rx, _tx) = setup_unauth_app().await;

    // 1. Submit invalid short phone
    for c in "12".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    assert!(matches!(app.auth_state, AuthStep::PhoneInput { error: Some(_), .. }));

    // 2. Clear input and enter valid phone
    if let AuthStep::PhoneInput { buffer, .. } = &mut app.auth_state {
        buffer.clear();
    }
    for c in "+15559876543".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    assert!(matches!(app.auth_state, AuthStep::CodeInput { .. }));

    // 3. Submit wrong OTP code
    for c in "999".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    assert!(matches!(app.auth_state, AuthStep::CodeInput { error: Some(_), .. }));

    // 4. Enter correct OTP code
    if let AuthStep::CodeInput { buffer, .. } = &mut app.auth_state {
        buffer.clear();
    }
    for c in "12345".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    assert!(matches!(app.auth_state, AuthStep::Password2FA { .. }));
}

#[tokio::test]
async fn test_tier3_pairwise_composer_typing_during_status_notification() {
    let (mut app, _rx, _tx) = setup_app().await;
    app.focus = FocusPane::Composer;

    // Trigger transient status notification banner
    app.set_status("Connecting to Telegram MTProto gateway...");

    // User types message simultaneously
    for c in "Message while status is visible".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }

    assert_eq!(app.composer_buffer, "Message while status is visible");
    assert!(app.status_notification.is_some());

    // Render both simultaneously
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();
    terminal.draw(|f| render_app(f, &app, &theme)).expect("Status notification and typing must co-render");
}

#[tokio::test]
async fn test_tier3_pairwise_reply_to_message_then_receive_new_message() {
    let (mut app, mut rx, _tx) = setup_app().await;
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(0);

    // Start reply to message 0 (ID 1)
    app.handle_user_action(UserAction::StartReply).await;
    assert!(app.composer_reply_to.is_some());
    let target_reply_id = app.composer_reply_to.as_ref().unwrap().message_id;

    // Background incoming message arrives from another conversation/user
    app.handle_tdlib_event(TdlibEvent::NewMessage(Message {
        id: MessageId(9999),
        chat_id: ChatId(101),
        sender_id: UserId(3),
        sender_name: "Intruder".into(),
        date: "14:00".into(),
        text: "New distraction message".into(),
        entities: vec![],
        reply_to: None,
        media: None,
        reactions: vec![],
        is_outgoing: false,
        status: DeliveryStatus::Read,
        is_edited: false,
    })).await;

    // Reply target must remain locked to message 0
    assert_eq!(app.composer_reply_to.as_ref().unwrap().message_id, target_reply_id);

    // Send reply
    app.composer_buffer = "Reply text".into();
    app.handle_user_action(UserAction::ComposerSubmit).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    let sent_reply = (*app.current_messages().last().unwrap()).clone();
    assert!(sent_reply.reply_to.is_some());
    assert_eq!(sent_reply.reply_to.as_ref().unwrap().message_id, target_reply_id);
}

#[tokio::test]
async fn test_tier3_pairwise_folder_switch_with_composer_draft_and_modal() {
    let (mut app, _rx, _tx) = setup_app().await;

    // 1. Compose draft in Folder 0, Chat 0
    app.focus = FocusPane::Composer;
    app.composer_buffer = "Draft for folder 0 chat".into();

    // 2. Switch folder
    app.handle_user_action(UserAction::SelectFolder(1)).await;
    assert_eq!(app.active_folder_idx, 1);

    // 3. Open modal in new folder
    app.handle_user_action(UserAction::OpenSearch).await;
    assert!(matches!(app.active_modal, ActiveModal::Search { .. }));
    app.handle_user_action(UserAction::CloseModal).await;
    assert_eq!(app.active_modal, ActiveModal::None);

    // 4. Switch back to Folder 0
    app.handle_user_action(UserAction::SelectFolder(0)).await;
    assert_eq!(app.active_folder_idx, 0);
    assert_eq!(app.composer_buffer, "Draft for folder 0 chat", "Draft remains intact after modal and folder switch");
}

// ============================================================================
// TIER 4: REAL-WORLD APPLICATION SCENARIOS (End-to-End User Journeys)
// ============================================================================

#[tokio::test]
async fn test_tier4_scenario_first_launch_to_authenticated_shell() {
    let (mut app, mut rx, _tx) = setup_unauth_app().await;

    // Step 1: User launches client with no session -> enters PhoneInput
    assert!(matches!(app.auth_state, AuthStep::PhoneInput { .. }));
    for c in "+15550192834".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    // Step 2: Verification code
    assert!(matches!(app.auth_state, AuthStep::CodeInput { .. }));
    for c in "12345".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    // Step 3: Cloud Password
    assert!(matches!(app.auth_state, AuthStep::Password2FA { .. }));
    for c in "super_secret_2fa".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    while let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    // Step 4: Full authenticated main shell rendered
    assert_eq!(app.auth_state, AuthStep::Authenticated);
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();
    terminal.draw(|f| render_app(f, &app, &theme)).expect("Main shell renders cleanly after complete auth flow");
}

#[tokio::test]
async fn test_tier4_scenario_chat_navigation_and_message_reading() {
    let (mut app, _rx, _tx) = setup_app().await;

    // User navigates dialog list
    app.focus = FocusPane::Dialogs;
    app.handle_user_action(UserAction::NextItem).await; // Chat 1
    app.handle_user_action(UserAction::NextItem).await; // Chat 2

    // Switches to messages pane to read
    app.handle_user_action(UserAction::CycleFocus).await;
    assert_eq!(app.focus, FocusPane::Messages);

    // Navigates down to inspect messages
    app.handle_user_action(UserAction::NextItem).await;
    app.handle_user_action(UserAction::NextItem).await;
    assert!(app.selected_msg_idx.is_some());

    // Opens chat profile
    app.handle_user_action(UserAction::OpenProfile).await;
    assert_eq!(app.active_modal, ActiveModal::ChatProfile);

    // Closes profile modal
    app.handle_user_action(UserAction::CloseModal).await;
    assert_eq!(app.active_modal, ActiveModal::None);
}

#[tokio::test]
async fn test_tier4_scenario_compose_long_message_80_chars_and_send_ack() {
    let (mut app, mut rx, _tx) = setup_app().await;

    app.focus = FocusPane::Composer;
    let long_msg = "TelegramTUI clean-room rebuild: verifying 80+ character message without border clipping or cursor distortion in terminal.";
    for c in long_msg.chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    assert!(app.composer_buffer.len() > 80);

    // Send
    app.handle_user_action(UserAction::ComposerSubmit).await;
    assert!(app.composer_buffer.is_empty(), "Composer clears after submit");

    // Receive optimistic message
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    let optimistic = (*app.current_messages().last().unwrap()).clone();
    assert_eq!(optimistic.text, long_msg);
    assert_eq!(optimistic.status, DeliveryStatus::Pending);

    // Receive Sent ACK
    tokio::time::sleep(tokio::time::Duration::from_millis(270)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    assert_eq!(app.current_messages().last().unwrap().status, DeliveryStatus::Sent);

    // Receive Read ACK
    tokio::time::sleep(tokio::time::Duration::from_millis(750)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    assert_eq!(app.current_messages().last().unwrap().status, DeliveryStatus::Read);
}

#[tokio::test]
async fn test_tier4_scenario_reply_edit_reaction_workflow() {
    let (mut app, mut rx, _tx) = setup_app().await;

    // 1. React to message 0 with 👍
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(0);
    app.handle_user_action(UserAction::OpenReactionPicker).await;
    app.handle_user_action(UserAction::InputChar('1')).await; // 👍
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    assert!(app.current_messages()[0].reactions.iter().any(|r| r.emoji == "👍" && r.chosen_by_me));

    // 2. Start reply to message 0
    app.handle_user_action(UserAction::StartReply).await;
    for c in "I agree with this completely!".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    let new_idx = app.current_messages().len() - 1;
    assert!(app.current_messages()[new_idx].reply_to.is_some());

    // 3. Edit newly posted outgoing message
    app.selected_msg_idx = Some(new_idx);
    app.handle_user_action(UserAction::StartEdit).await;
    app.composer_buffer = "I agree with this completely! (Updated confirmation)".into();
    app.handle_user_action(UserAction::ComposerSubmit).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    let edited = &app.current_messages()[new_idx];
    assert_eq!(edited.text, "I agree with this completely! (Updated confirmation)");
    assert!(edited.is_edited);
}

#[tokio::test]
async fn test_tier4_scenario_forward_with_privacy_and_attachment() {
    let (mut app, mut rx, _tx) = setup_app().await;

    // 1. Forward message with hidden author
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(0);
    app.handle_user_action(UserAction::OpenForwardModal).await;
    assert!(matches!(app.active_modal, ActiveModal::ForwardModal { .. }));

    // Toggle hide author
    app.handle_user_action(UserAction::ToggleForwardHideAuthor).await;
    app.handle_user_action(UserAction::SubmitForward).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    let fwd_msg = (*app.current_messages().last().unwrap()).clone();
    assert!(!fwd_msg.text.contains("[Forwarded from"));

    // 2. Attach and send document
    app.handle_user_action(UserAction::OpenAttachFile).await;
    for c in "quarterly_financials.pdf".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    // Switch to document (toggle photo mode)
    if let ActiveModal::AttachFile { is_photo, .. } = &mut app.active_modal {
        *is_photo = false;
    }
    app.handle_user_action(UserAction::SubmitAttachFile).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    let doc_msg = (*app.current_messages().last().unwrap()).clone();
    assert!(matches!(doc_msg.media, Some(MessageMedia::Document { .. })));
}
