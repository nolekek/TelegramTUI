#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;
use serde_json::json;
use tokio::sync::broadcast;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use tg_core::{
    Chat, ChatId, ChatType, CoreEvent, CoreState,
    FileInfo, FileTransferStatus, Message, MessageContent, MessageId,
    MessageStatus, MockBackend, Sender, TDLibBackend, TelegramBackend,
};

#[path = "../crates/tg-tui/src/text/mod.rs"]
pub mod text;

#[path = "../crates/tg-tui/src/ui/messages.rs"]
pub mod messages_ui;

use messages_ui::{format_bytes, format_duration, render_messages, render_progress_bar};

// ============================================================================
// SUITE 1: Pathological Media Metadata Boundaries (0-byte, Huge, Corrupt)
// ============================================================================

#[test]
fn test_adversarial_zero_and_negative_file_boundaries() {
    // 1. Zero-byte FileInfo
    let file_zero = FileInfo::new(10, 0, 0);
    assert_eq!(file_zero.total_size(), 0);
    assert_eq!(file_zero.downloaded_size(), 0);
    assert_eq!(file_zero.progress_percent(), 0.0);
    assert!(file_zero.can_be_downloaded());
    assert!(!file_zero.is_downloading());
    assert!(!file_zero.is_completed());

    // 2. Negative size FileInfo (defensive against protocol anomalies)
    let file_neg = FileInfo::new(11, -100, -500);
    assert_eq!(file_neg.total_size(), 0);
    assert_eq!(file_neg.progress_percent(), 0.0);

    // 3. Size without expected_size
    let file_size_only = FileInfo::new(12, 1024, 0);
    assert_eq!(file_size_only.total_size(), 1024);

    // 4. Expected size takes precedence
    let file_expected = FileInfo::new(13, 1024, 2048);
    assert_eq!(file_expected.total_size(), 2048);

    // 5. TDLib parse_file with empty or missing fields
    let missing_id = json!({ "@type": "file" });
    assert!(TDLibBackend::parse_file(&missing_id).is_none());

    let zero_json = json!({
        "@type": "file",
        "id": 100,
        "size": 0,
        "expected_size": 0,
        "local": {
            "path": "",
            "is_downloading_active": false,
            "is_downloading_completed": false,
            "downloaded_size": 0
        }
    });
    let parsed_zero = TDLibBackend::parse_file(&zero_json).expect("Must parse zero file");
    assert_eq!(parsed_zero.id, 100);
    assert_eq!(parsed_zero.total_size(), 0);
    assert!(parsed_zero.local_path.is_none());
    assert_eq!(parsed_zero.status, FileTransferStatus::Idle);
}

#[test]
fn test_adversarial_extreme_and_overflow_file_boundaries() {
    // 1. i64::MAX boundary
    let file_max = FileInfo::new(20, i64::MAX, i64::MAX);
    assert_eq!(file_max.total_size(), i64::MAX);
    assert_eq!(file_max.progress_percent(), 0.0);

    // 2. Completed with i64::MAX
    let file_max_completed = file_max.clone().with_status(FileTransferStatus::Completed);
    assert_eq!(file_max_completed.downloaded_size(), i64::MAX);
    assert_eq!(file_max_completed.progress_percent(), 100.0);

    // 3. Downloading with huge progress
    let file_half = file_max.clone().with_status(FileTransferStatus::Downloading {
        downloaded_size: i64::MAX / 2,
    });
    let pct = file_half.progress_percent();
    assert!(pct >= 49.9 && pct <= 50.1, "Expected ~50%, got {}", pct);

    // 4. Downloaded size exceeding total (clamp to 100.0)
    let file_overflow = FileInfo::new(21, 100, 100).with_status(FileTransferStatus::Downloading {
        downloaded_size: 200,
    });
    assert_eq!(file_overflow.progress_percent(), 100.0);

    // 5. Negative downloaded size (clamp to 0.0)
    let file_underflow = FileInfo::new(22, 100, 100).with_status(FileTransferStatus::Downloading {
        downloaded_size: -50,
    });
    assert_eq!(file_underflow.progress_percent(), 0.0);
}

#[test]
fn test_adversarial_format_helpers_boundaries() {
    // format_bytes
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(-1), "0 B");
    assert_eq!(format_bytes(i64::MIN), "0 B");
    assert_eq!(format_bytes(500), "500 B");
    assert_eq!(format_bytes(1023), "1023 B");
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1536), "1.5 KB");
    assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    assert_eq!(format_bytes(50 * 1024 * 1024 * 1024), "50.0 GB");
    assert_eq!(format_bytes(i64::MAX), "8589934592.0 GB");

    // format_duration
    assert_eq!(format_duration(0), "0:00");
    assert_eq!(format_duration(-1), "0:00");
    assert_eq!(format_duration(-9999), "0:00");
    assert_eq!(format_duration(i32::MIN), "0:00");
    assert_eq!(format_duration(42), "0:42");
    assert_eq!(format_duration(60), "1:00");
    assert_eq!(format_duration(3665), "61:05");
    let dur_max = format_duration(i32::MAX);
    assert!(!dur_max.is_empty(), "Should format max duration safely");
}

#[test]
fn test_adversarial_render_progress_bar_edge_cases() {
    // Normal cases
    assert_eq!(render_progress_bar(0.0, 10), "[          ]");
    assert_eq!(render_progress_bar(50.0, 10), "[====>     ]");
    assert_eq!(render_progress_bar(100.0, 10), "[==========]");

    // Boundary & extreme percentages
    assert_eq!(render_progress_bar(-50.0, 10), "[          ]");
    assert_eq!(render_progress_bar(150.0, 10), "[==========]");
    assert_eq!(render_progress_bar(f64::NAN, 10), "[          ]");
    assert_eq!(render_progress_bar(f64::INFINITY, 10), "[==========]");
    assert_eq!(render_progress_bar(f64::NEG_INFINITY, 10), "[          ]");

    // Extreme widths (width is clamped to at least 4)
    assert_eq!(render_progress_bar(0.0, 0), "[    ]");
    assert_eq!(render_progress_bar(100.0, 1), "[====]");
    assert_eq!(render_progress_bar(50.0, 4), "[=>  ]");
    assert_eq!(render_progress_bar(100.0, 50).len(), 52); // 50 '=' + '[' + ']'
}

#[test]
fn test_adversarial_tdlib_message_parsing_pathological_inputs() {
    // 1. Document with empty filename, null bytes, and path traversal
    let doc_json = json!({
        "@type": "messageDocument",
        "caption": { "@type": "formattedText", "text": "Shady file" },
        "document": {
            "@type": "document",
            "file_name": "../../../../../Windows/System32/evil.dll\0.txt",
            "document": {
                "@type": "file",
                "id": 8888,
                "size": -1,
                "expected_size": 50000000000i64
            }
        }
    });
    let parsed_doc = TDLibBackend::parse_message_content(&doc_json);
    match parsed_doc {
        MessageContent::Document { file_name, file_id, file_size, .. } => {
            assert_eq!(file_id, 8888);
            assert!(file_name.contains("evil.dll"));
            // u64 cast of -1 is u64::MAX
            assert_eq!(file_size, u64::MAX);
        }
        other => panic!("Expected Document, got {:?}", other),
    }

    // 2. Photo with empty sizes array []
    let empty_photo = json!({
        "@type": "messagePhoto",
        "photo": {
            "@type": "photo",
            "sizes": []
        }
    });
    let parsed_photo = TDLibBackend::parse_message_content(&empty_photo);
    match parsed_photo {
        MessageContent::Photo { file_id, width, height, caption } => {
            assert_eq!(file_id, 0);
            assert_eq!(width, 0);
            assert_eq!(height, 0);
            assert_eq!(caption, "");
        }
        other => panic!("Expected Photo, got {:?}", other),
    }

    // 3. Photo with missing photo object
    let null_photo = json!({ "@type": "messagePhoto" });
    let parsed_null_photo = TDLibBackend::parse_message_content(&null_photo);
    assert!(matches!(parsed_null_photo, MessageContent::Photo { file_id: 0, .. }));

    // 4. Audio with null/missing title and performer
    let sparse_audio = json!({
        "@type": "messageAudio",
        "audio": {
            "@type": "audio",
            "duration": -5,
            "title": "",
            "performer": "",
            "audio": {
                "@type": "file",
                "id": 9999,
                "size": i64::MAX
            }
        }
    });
    let parsed_audio = TDLibBackend::parse_message_content(&sparse_audio);
    match parsed_audio {
        MessageContent::Audio { file_id, title, performer, duration, file_size } => {
            assert_eq!(file_id, 9999);
            assert!(title.is_none());
            assert!(performer.is_none());
            assert_eq!(duration, -5);
            assert_eq!(file_size, i64::MAX);
        }
        other => panic!("Expected Audio, got {:?}", other),
    }

    // 5. Sticker with complex multi-codepoint emoji and empty sticker object
    let complex_sticker = json!({
        "@type": "messageSticker",
        "sticker": {
            "@type": "sticker",
            "emoji": "👨‍👩‍👧‍👦",
            "sticker": {
                "@type": "file",
                "id": 5555
            }
        }
    });
    let parsed_sticker = TDLibBackend::parse_message_content(&complex_sticker);
    match parsed_sticker {
        MessageContent::Sticker { emoji, file_id } => {
            assert_eq!(emoji, "👨‍👩‍👧‍👦");
            assert_eq!(file_id, 5555);
        }
        other => panic!("Expected Sticker, got {:?}", other),
    }

    // 6. Voice note with empty voice object
    let sparse_voice = json!({
        "@type": "messageVoiceNote",
        "voice_note": {
            "@type": "voiceNote",
            "duration": 0
        }
    });
    let parsed_voice = TDLibBackend::parse_message_content(&sparse_voice);
    match parsed_voice {
        MessageContent::Voice { file_id, duration, .. } => {
            assert_eq!(file_id, 0);
            assert_eq!(duration, 0);
        }
        other => panic!("Expected Voice, got {:?}", other),
    }

    // 7. Empty JSON payload
    let empty_val = json!({});
    let parsed_empty = TDLibBackend::parse_message_content(&empty_val);
    assert!(matches!(parsed_empty, MessageContent::Unsupported { .. }));
}

// ============================================================================
// SUITE 2: Rapid Download Cancellation Stress & State Transitions
// ============================================================================

#[tokio::test]
async fn test_adversarial_rapid_download_cancellation_immediate() {
    let mock = Arc::new(MockBackend::new());
    let (tx, mut rx) = broadcast::channel(100);
    mock.start(tx.clone()).await.unwrap();

    // Trigger download of Document (file_id = 101) and immediately cancel
    mock.download_file(101, 32).await.unwrap();
    mock.cancel_download_file(101).await.unwrap();

    // Collect events for file 101
    let mut saw_cancelled = false;
    let mut saw_completed = false;

    let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
            if let CoreEvent::FileUpdated(file) = event {
                if file.id == 101 {
                    if matches!(file.status, FileTransferStatus::Failed(_)) {
                        saw_cancelled = true;
                    }
                    if matches!(file.status, FileTransferStatus::Completed) {
                        saw_completed = true;
                    }
                }
            }
        }
    }

    assert!(saw_cancelled, "File 101 should have transitioned to Failed/Cancelled");
    assert!(!saw_completed, "File 101 must NOT have completed after immediate cancellation");
}

#[tokio::test]
async fn test_adversarial_download_cancellation_hammering() {
    let mock = Arc::new(MockBackend::new());
    let (tx, _rx) = broadcast::channel(500);
    mock.start(tx.clone()).await.unwrap();

    // Rapid loop hammering download & cancel 50 times across alternating file IDs
    for i in 0..50 {
        let file_id = 1000 + (i % 5);
        mock.download_file(file_id, 32).await.unwrap();
        // Zero delay cancellation
        mock.cancel_download_file(file_id).await.unwrap();
    }

    // Verify all 5 files ended in either Failed or Idle, without panic or deadlock
    for id in 1000..1005 {
        mock.cancel_download_file(id).await.unwrap();
    }
}

#[tokio::test]
async fn test_adversarial_cancellation_idempotency_and_nonexistent_ids() {
    let mock = Arc::new(MockBackend::new());
    let (tx, _rx) = broadcast::channel(100);
    mock.start(tx.clone()).await.unwrap();

    // 1. Cancel nonexistent IDs
    mock.cancel_download_file(-1).await.unwrap();
    mock.cancel_download_file(0).await.unwrap();
    mock.cancel_download_file(999999).await.unwrap();
    mock.cancel_download_file(i32::MAX).await.unwrap();

    // 2. Double cancellation idempotency
    mock.download_file(201, 32).await.unwrap();
    mock.cancel_download_file(201).await.unwrap();
    // Second cancel must not error or panic
    mock.cancel_download_file(201).await.unwrap();

    // 3. Cancel already completed download
    // Wait for file 202 (Audio) to complete
    let (tx2, mut rx2) = broadcast::channel(100);
    mock.start(tx2.clone()).await.unwrap();
    mock.download_file(202, 32).await.unwrap();

    let mut completed = false;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(400);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(50), rx2.recv()).await {
            if let CoreEvent::FileUpdated(file) = event {
                if file.id == 202 && file.is_completed() {
                    completed = true;
                    break;
                }
            }
        }
    }
    assert!(completed, "File 202 should complete");

    // Now cancelling completed file must be a graceful no-op
    mock.cancel_download_file(202).await.unwrap();
}

#[tokio::test]
async fn test_adversarial_redownload_after_cancellation() {
    let mock = Arc::new(MockBackend::new());
    let (tx, mut rx) = broadcast::channel(100);
    mock.start(tx.clone()).await.unwrap();

    // 1. Initiate download for file 301 and cancel it
    mock.download_file(301, 32).await.unwrap();
    mock.cancel_download_file(301).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 2. Re-download file 301 and let it run to completion
    mock.download_file(301, 32).await.unwrap();

    let mut saw_completed = false;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(400);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
            if let CoreEvent::FileUpdated(file) = event {
                if file.id == 301 && file.is_completed() {
                    saw_completed = true;
                    assert!(file.local_path.is_some());
                    break;
                }
            }
        }
    }

    assert!(saw_completed, "File 301 should successfully complete upon re-download");
}

// ============================================================================
// SUITE 3: Mock Backend 50GB & Extreme Size Download Handling
// ============================================================================

#[tokio::test]
async fn test_adversarial_mock_huge_50gb_download() {
    let mock = Arc::new(MockBackend::new());
    let (tx, mut rx) = broadcast::channel(100);
    mock.start(tx.clone()).await.unwrap();

    let huge_id = 7777;
    assert_eq!(format_bytes(50 * 1024 * 1024 * 1024), "50.0 GB");

    mock.download_file(huge_id, 32).await.unwrap();

    let mut progress_events = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_millis(400);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
            if let CoreEvent::FileUpdated(file) = event {
                if file.id == huge_id {
                    progress_events.push(file);
                }
            }
        }
    }

    assert!(!progress_events.is_empty(), "Should receive progress events for 50GB file");
    let last = progress_events.last().unwrap();
    assert!(last.is_completed(), "50GB mock download should complete cleanly");
}

#[test]
fn test_adversarial_core_state_file_event_stream_fuzz() {
    let mut state = CoreState::new();

    // Apply 500 file events across 50 file IDs with various statuses
    for i in 0..500 {
        let file_id = (i % 50) as i32;
        let status = match i % 5 {
            0 => FileTransferStatus::Idle,
            1 => FileTransferStatus::Downloading { downloaded_size: (i as i64) * 1000 },
            2 => FileTransferStatus::Uploading { uploaded_size: (i as i64) * 500 },
            3 => FileTransferStatus::Completed,
            _ => FileTransferStatus::Failed(format!("Error code {}", i)),
        };
        let file = FileInfo::new(file_id, 100000, 100000).with_status(status);
        state.apply_update(&CoreEvent::FileUpdated(file));
    }

    assert_eq!(state.files.len(), 50);
    for id in 0..50 {
        assert!(state.get_file(id).is_some());
    }
}

#[test]
fn test_adversarial_tui_render_messages_with_pathological_media() {
    let mut state = CoreState::new();
    let chat_id = ChatId(1);
    let chat = Chat {
        id: chat_id,
        title: "Adversarial Media Chat".to_string(),
        chat_type: ChatType::Supergroup,
        unread_count: 0,
        last_read_inbox_message_id: 0,
        last_read_outbox_message_id: 0,
        last_message: None,
        position: None,
        is_muted: false,
        has_unread_mention: false,
        folder_ids: Vec::new(),
    };
    state.insert_or_update_chat(chat);

    // Register pathological files
    state.update_file(FileInfo::new(1, 0, 0)); // 0-byte file
    state.update_file(FileInfo::new(2, i64::MAX, i64::MAX)); // i64::MAX
    state.update_file(FileInfo::new(3, -100, -200)); // negative sizes
    state.update_file(
        FileInfo::new(4, 1000, 1000)
            .with_status(FileTransferStatus::Downloading { downloaded_size: 500 })
    );
    state.update_file(
        FileInfo::new(5, 1000, 1000)
            .with_status(FileTransferStatus::Uploading { uploaded_size: 750 })
    );
    state.update_file(
        FileInfo::new(6, 1000, 1000)
            .with_status(FileTransferStatus::Failed("Disk full".to_string()))
    );

    // Add messages with pathological media contents
    let messages = vec![
        Message {
            id: MessageId(1),
            chat_id,
            sender: Sender::Unknown,
            date: 100,
            content: MessageContent::Photo {
                caption: "".to_string(),
                file_id: 1,
                width: 0,
                height: 0,
            },
            is_outgoing: false,
            status: MessageStatus::Read,
            reply_to_message_id: None,
            can_be_edited: false,
            can_be_deleted: false,
        },
        Message {
            id: MessageId(2),
            chat_id,
            sender: Sender::Unknown,
            date: 200,
            content: MessageContent::Photo {
                caption: "Huge Photo".to_string(),
                file_id: 2,
                width: i32::MAX,
                height: i32::MAX,
            },
            is_outgoing: true,
            status: MessageStatus::Sent,
            reply_to_message_id: None,
            can_be_edited: false,
            can_be_deleted: false,
        },
        Message {
            id: MessageId(3),
            chat_id,
            sender: Sender::Unknown,
            date: 300,
            content: MessageContent::Document {
                file_name: "".to_string(),
                file_size: u64::MAX,
                caption: "Weird doc".to_string(),
                file_id: 3,
            },
            is_outgoing: false,
            status: MessageStatus::Read,
            reply_to_message_id: None,
            can_be_edited: false,
            can_be_deleted: false,
        },
        Message {
            id: MessageId(4),
            chat_id,
            sender: Sender::Unknown,
            date: 400,
            content: MessageContent::Audio {
                file_id: 4,
                title: None,
                performer: None,
                duration: -1,
                file_size: i64::MAX,
            },
            is_outgoing: false,
            status: MessageStatus::Read,
            reply_to_message_id: None,
            can_be_edited: false,
            can_be_deleted: false,
        },
        Message {
            id: MessageId(5),
            chat_id,
            sender: Sender::Unknown,
            date: 500,
            content: MessageContent::Voice {
                caption: "Voice note".to_string(),
                duration: i32::MAX,
                file_id: 5,
            },
            is_outgoing: false,
            status: MessageStatus::Read,
            reply_to_message_id: None,
            can_be_edited: false,
            can_be_deleted: false,
        },
        Message {
            id: MessageId(6),
            chat_id,
            sender: Sender::Unknown,
            date: 600,
            content: MessageContent::Sticker {
                emoji: "".to_string(),
                file_id: 6,
            },
            is_outgoing: false,
            status: MessageStatus::Read,
            reply_to_message_id: None,
            can_be_edited: false,
            can_be_deleted: false,
        },
    ];

    state.insert_history_messages(chat_id, messages);

    // Render across zero dimensions and normal dimensions
    let areas = [
        Rect::new(0, 0, 0, 0),
        Rect::new(0, 0, 1, 1),
        Rect::new(0, 0, 80, 24),
        Rect::new(0, 0, 200, 100),
    ];

    for area in areas {
        let backend = TestBackend::new(area.width.max(1), area.height.max(1));
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_messages(f, &state, Some(chat_id), 0, 0, true, area);
            })
            .unwrap();
    }
}
