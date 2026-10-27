//! End-to-End Automated Flow Tests for TgTui
//! Validates user flows F01 through F08 with happy paths, edge cases, and negative scenarios.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tg_tui::actors::coordinator::{AppCoordinator, UserAction};
use tg_tui::actors::tdlib::TdlibActor;
use tg_tui::model::{ActiveModal, AuthStep, DeliveryStatus, FocusPane};
use tg_tui::theme::Theme;
use tg_tui::ui::render_app;
use tokio::sync::mpsc;

// Helper to bootstrap coordinator with live TdlibActor
async fn setup_app() -> (AppCoordinator, mpsc::Receiver<tg_tui::actors::tdlib::TdlibEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let (event_tx, mut event_rx) = mpsc::channel(64);

    let tdlib = TdlibActor::new(cmd_rx, event_tx, true);
    tokio::spawn(async move {
        tdlib.run().await;
    });

    let mut app = AppCoordinator::new(cmd_tx);

    // Wait and drain initial seed events
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    while let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    (app, event_rx)
}

#[tokio::test]
async fn test_f01_auth_flow_happy_path() {
    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let (event_tx, mut event_rx) = mpsc::channel(64);

    // Start in unauthenticated state
    let tdlib = TdlibActor::new(cmd_rx, event_tx, false);
    tokio::spawn(async move {
        tdlib.run().await;
    });

    let mut app = AppCoordinator::new(cmd_tx);
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    // Step 1: Phone input
    assert!(matches!(app.auth_state, AuthStep::PhoneInput { .. }));
    for c in "+15550192834".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;

    // Receive transition to CodeInput
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    assert!(matches!(app.auth_state, AuthStep::CodeInput { .. }));

    // Step 2: Code input
    for c in "12345".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;

    // Receive transition to Password2FA
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    assert!(matches!(app.auth_state, AuthStep::Password2FA { .. }));

    // Step 3: 2FA Password input
    for c in "secret123".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;

    // Receive Authenticated
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    while let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    assert_eq!(app.auth_state, AuthStep::Authenticated);
    assert!(!app.chats.is_empty(), "Chats must be populated upon authentication");
}

#[tokio::test]
async fn test_f01_auth_negative_cases() {
    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let (event_tx, mut event_rx) = mpsc::channel(64);

    let tdlib = TdlibActor::new(cmd_rx, event_tx, false);
    tokio::spawn(async move {
        tdlib.run().await;
    });

    let mut app = AppCoordinator::new(cmd_tx);
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    // F01-N1: Too short phone number
    for c in "123".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    if let AuthStep::PhoneInput { error, .. } = &app.auth_state {
        assert!(error.is_some(), "Must report validation error for short phone");
    } else {
        panic!("Should remain on PhoneInput on error");
    }
}

#[tokio::test]
async fn test_f02_send_message_optimistic_and_ack() {
    let (mut app, mut event_rx) = setup_app().await;

    // Switch focus to composer and type message
    app.focus = FocusPane::Composer;
    let test_msg = "Hello from e2e test **bold**";
    for c in test_msg.chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }

    let initial_count = app.current_messages().len();

    // Submit message
    app.handle_user_action(UserAction::ComposerSubmit).await;
    assert!(app.composer_buffer.is_empty(), "Composer must clear after submission");

    // Receive optimistic message
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    let new_msgs = app.current_messages();
    assert_eq!(new_msgs.len(), initial_count + 1);
    let last_msg = new_msgs.last().unwrap();
    assert_eq!(last_msg.text, test_msg);
    assert_eq!(last_msg.status, DeliveryStatus::Pending, "Optimistic status must be Pending");

    // Await server ACK (transitions to Sent after 250ms)
    tokio::time::sleep(tokio::time::Duration::from_millis(270)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    let last_msg_ack = (*app.current_messages().last().unwrap()).clone();
    assert_eq!(last_msg_ack.status, DeliveryStatus::Sent);

    // Await read status (transitions to Read after 700ms)
    tokio::time::sleep(tokio::time::Duration::from_millis(750)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }
    let last_msg_read = (*app.current_messages().last().unwrap()).clone();
    assert_eq!(last_msg_read.status, DeliveryStatus::Read);
}

#[tokio::test]
async fn test_f03_reply_flow() {
    let (mut app, mut event_rx) = setup_app().await;

    // Select first message
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(0);

    // Trigger reply
    app.handle_user_action(UserAction::StartReply).await;
    assert!(app.composer_reply_to.is_some(), "Reply banner must be populated");
    assert_eq!(app.focus, FocusPane::Composer, "Focus must move to composer");

    // Compose reply
    for c in "I am replying to you!".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::ComposerSubmit).await;

    // Drain event
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    let last_msg = (*app.current_messages().last().unwrap()).clone();
    assert!(last_msg.reply_to.is_some(), "New message must retain reply_to reference");
}

#[tokio::test]
async fn test_f04_edit_and_delete_flow() {
    let (mut app, mut event_rx) = setup_app().await;

    // Message idx 2 is outgoing ("You")
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(2);

    // Start edit
    app.handle_user_action(UserAction::StartEdit).await;
    assert!(app.composer_edit_id.is_some(), "Edit ID must be set");
    assert!(!app.composer_buffer.is_empty(), "Buffer must contain existing text");

    // Modify text
    app.composer_buffer = "Edited text".into();
    app.handle_user_action(UserAction::ComposerSubmit).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    let edited_msg = (*app.current_messages().get(2).unwrap()).clone();
    assert_eq!(edited_msg.text, "Edited text");
    assert!(edited_msg.is_edited);

    // Test Delete
    let before_count = app.current_messages().len();
    app.handle_user_action(UserAction::DeleteSelected).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    assert_eq!(app.current_messages().len(), before_count - 1);
}

#[tokio::test]
async fn test_f07_search_modal() {
    let (mut app, _event_rx) = setup_app().await;

    app.handle_user_action(UserAction::OpenSearch).await;
    assert!(matches!(app.active_modal, ActiveModal::Search { .. }));

    for c in "Pavel".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }

    if let ActiveModal::Search { query } = &app.active_modal {
        assert_eq!(query, "Pavel");
    }

    app.handle_user_action(UserAction::CloseModal).await;
    assert_eq!(app.active_modal, ActiveModal::None);
}

#[tokio::test]
async fn test_f08_folder_switching() {
    let (mut app, _event_rx) = setup_app().await;

    // Select Folder 1 (Personal)
    app.handle_user_action(UserAction::SelectFolder(1)).await;
    assert_eq!(app.active_folder_idx, 1);
    let personal_chats = app.filtered_chats();
    assert!(!personal_chats.is_empty());
    for chat in personal_chats {
        assert_eq!(chat.folder_id, Some(1));
    }

    // Select Folder 0 (All)
    app.handle_user_action(UserAction::SelectFolder(0)).await;
    assert_eq!(app.filtered_chats().len(), app.chats.len());
}

#[tokio::test]
async fn test_f05_send_attachment_and_lightbox() {
    let (mut app, mut event_rx) = setup_app().await;

    // Open attach modal
    app.handle_user_action(UserAction::OpenAttachFile).await;
    assert!(matches!(app.active_modal, ActiveModal::AttachFile { .. }));

    // Input path
    for c in "vacation_photo.png".chars() {
        app.handle_user_action(UserAction::InputChar(c)).await;
    }
    app.handle_user_action(UserAction::SubmitAttachFile).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    let last_msg = (*app.current_messages().last().unwrap()).clone();
    assert!(matches!(last_msg.media, Some(tg_tui::model::MessageMedia::Photo { .. })));

    // Open Lightbox on Photo message
    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(app.current_messages().len() - 1);
    app.handle_user_action(UserAction::SelectCurrent).await;

    assert!(matches!(app.active_modal, ActiveModal::Lightbox { .. }));
}

#[tokio::test]
async fn test_f06_forward_message_with_privacy() {
    let (mut app, mut event_rx) = setup_app().await;

    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(0);

    // Open forward modal
    app.handle_user_action(UserAction::OpenForwardModal).await;
    assert!(matches!(app.active_modal, ActiveModal::ForwardModal { .. }));

    // Toggle hide author
    app.handle_user_action(UserAction::ToggleForwardHideAuthor).await;
    if let ActiveModal::ForwardModal { hide_author, .. } = &app.active_modal {
        assert!(*hide_author);
    }

    // Submit forward
    app.handle_user_action(UserAction::SubmitForward).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    // Message forwarded to chat 101 (target index 0)
    let last_msg = (*app.current_messages().last().unwrap()).clone();
    assert!(!last_msg.text.contains("[Forwarded from"), "Author name should be hidden");
}

#[tokio::test]
async fn test_reactions_toggle() {
    let (mut app, mut event_rx) = setup_app().await;

    app.focus = FocusPane::Messages;
    app.selected_msg_idx = Some(0);

    // Toggle reaction via OpenReactionPicker and key 1 (👍)
    app.handle_user_action(UserAction::OpenReactionPicker).await;
    app.handle_user_action(UserAction::InputChar('1')).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    if let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    let msg = (*app.current_messages().get(0).unwrap()).clone();
    let thumb = msg.reactions.iter().find(|r| r.emoji == "👍");
    assert!(thumb.is_some(), "Reaction 👍 must be added to message");
    assert_eq!(thumb.unwrap().count, 1);
    assert!(thumb.unwrap().chosen_by_me);
}

#[test]
fn test_render_all_screens_no_panic() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();
    let (tx, _rx) = mpsc::channel(1);
    let mut app = AppCoordinator::new(tx);

    // 1. Render Auth Screen
    app.auth_state = AuthStep::PhoneInput { buffer: "+12345".into(), error: None };
    terminal.draw(|f| render_app(f, &app, &theme)).unwrap();

    // 2. Render Shell
    app.auth_state = AuthStep::Authenticated;
    terminal.draw(|f| render_app(f, &app, &theme)).unwrap();

    // 3. Render Modals
    app.active_modal = ActiveModal::ContextMenu { message_idx: 0 };
    terminal.draw(|f| render_app(f, &app, &theme)).unwrap();

    app.active_modal = ActiveModal::ChatProfile;
    terminal.draw(|f| render_app(f, &app, &theme)).unwrap();

    app.active_modal = ActiveModal::SettingsHelp;
    terminal.draw(|f| render_app(f, &app, &theme)).unwrap();

    app.active_modal = ActiveModal::ReactionPicker { message_idx: 0 };
    terminal.draw(|f| render_app(f, &app, &theme)).unwrap();

    app.active_modal = ActiveModal::ForwardModal { message_idx: 0, selected_chat_idx: 0, hide_author: true };
    terminal.draw(|f| render_app(f, &app, &theme)).unwrap();

    app.active_modal = ActiveModal::Lightbox { title: "photo.jpg".into(), dimensions: "1920x1080".into(), zoom_percent: 100 };
    terminal.draw(|f| render_app(f, &app, &theme)).unwrap();

    app.active_modal = ActiveModal::AttachFile { path_buffer: "test.png".into(), is_photo: true, compress: true };
    terminal.draw(|f| render_app(f, &app, &theme)).unwrap();
}
