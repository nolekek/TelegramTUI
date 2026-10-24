#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tokio::sync::broadcast;

use tg_core::{
    AuthState, Chat, ChatId, ChatType, Config, CoreEvent, CoreState, Engine,
    FileInfo, FileTransferStatus, Message, MessageContent, MessageId,
    MessageStatus, MockBackend, Sender, TelegramBackend, UserId,
};

#[path = "../crates/tg-tui/src/text/mod.rs"]
pub mod text;

#[path = "../crates/tg-tui/src/ui/messages.rs"]
pub mod messages_ui;

use messages_ui::{format_bytes, format_duration, render_messages, render_progress_bar};

// ============================================================================
// SUITE 1: Division by Zero & Extreme Arithmetic Boundary Tests
// ============================================================================

#[test]
fn test_adversarial_progress_division_by_zero_guards() {
    // 1. Both expected_size and size are 0
    let zero_file = FileInfo::new(1, 0, 0);
    assert_eq!(zero_file.total_size(), 0);
    assert_eq!(zero_file.downloaded_size(), 0);
    // Crucial: Must NEVER divide by zero or yield NaN; must return 0.0
    assert_eq!(zero_file.progress_percent(), 0.0);

    // 2. expected_size is 0, size is 0, but status claims 500 downloaded
    let zero_downloading = zero_file.clone().with_status(FileTransferStatus::Downloading {
        downloaded_size: 500,
    });
    assert_eq!(zero_downloading.total_size(), 0);
    assert_eq!(zero_downloading.downloaded_size(), 500);
    assert_eq!(zero_downloading.progress_percent(), 0.0);

    // 3. expected_size is 0, size is positive (fallback to size)
    let size_fallback = FileInfo::new(2, 2048, 0).with_status(FileTransferStatus::Downloading {
        downloaded_size: 1024,
    });
    assert_eq!(size_fallback.total_size(), 2048);
    assert_eq!(size_fallback.progress_percent(), 50.0);

    // 4. expected_size is positive, size is 0
    let expected_only = FileInfo::new(3, 0, 4096).with_status(FileTransferStatus::Downloading {
        downloaded_size: 1024,
    });
    assert_eq!(expected_only.total_size(), 4096);
    assert_eq!(expected_only.progress_percent(), 25.0);

    // 5. Completed status always yields 100.0, even if size is 0
    let completed_zero = FileInfo::new(4, 0, 0).with_status(FileTransferStatus::Completed);
    assert_eq!(completed_zero.progress_percent(), 100.0);
    assert_eq!(completed_zero.downloaded_size(), 0);
}

#[test]
fn test_adversarial_progress_negative_values_and_underflow() {
    // 1. Negative expected_size and size (protocol corruption / unexpected negative integers)
    let neg_file = FileInfo::new(10, -500, -1000);
    assert_eq!(neg_file.total_size(), 0, "Negative sizes must not be treated as positive totals");
    assert_eq!(neg_file.progress_percent(), 0.0);

    // 2. Negative downloaded size
    let neg_downloaded = FileInfo::new(11, 1000, 1000).with_status(FileTransferStatus::Downloading {
        downloaded_size: -500,
    });
    assert_eq!(neg_downloaded.downloaded_size(), -500);
    // Must clamp to 0.0 and never produce negative percentage
    assert_eq!(neg_downloaded.progress_percent(), 0.0);

    // 3. Negative sizes with Completed status
    let neg_completed = FileInfo::new(12, -100, -200).with_status(FileTransferStatus::Completed);
    assert_eq!(neg_completed.progress_percent(), 100.0);

    // 4. Uploading status returns 0 for downloaded_size
    let uploading_file = FileInfo::new(13, 1000, 1000).with_status(FileTransferStatus::Uploading {
        uploaded_size: 500,
    });
    assert_eq!(uploading_file.downloaded_size(), 0);
    assert_eq!(uploading_file.progress_percent(), 0.0);
    assert!(uploading_file.is_uploading());
    assert!(!uploading_file.is_downloading());
}

#[test]
fn test_adversarial_progress_integer_overflow_and_overshoot() {
    // 1. i64::MAX boundary
    let max_file = FileInfo::new(20, i64::MAX, i64::MAX);
    assert_eq!(max_file.total_size(), i64::MAX);
    assert_eq!(max_file.progress_percent(), 0.0);

    // 2. Exactly i64::MAX downloaded
    let max_downloaded = max_file.clone().with_status(FileTransferStatus::Downloading {
        downloaded_size: i64::MAX,
    });
    assert_eq!(max_downloaded.progress_percent(), 100.0);

    // 3. Downloaded size greatly exceeding total (overshoot clamp)
    let overshoot = FileInfo::new(21, 100, 100).with_status(FileTransferStatus::Downloading {
        downloaded_size: 50_000,
    });
    assert_eq!(overshoot.progress_percent(), 100.0, "Overshoot must clamp to 100.0");

    // 4. i64::MAX downloaded on a 1-byte file
    let extreme_overshoot = FileInfo::new(22, 1, 1).with_status(FileTransferStatus::Downloading {
        downloaded_size: i64::MAX,
    });
    assert_eq!(extreme_overshoot.progress_percent(), 100.0);

    // 5. 1 byte downloaded on an i64::MAX file (tiny fraction, no underflow panic)
    let tiny_fraction = FileInfo::new(23, i64::MAX, i64::MAX).with_status(FileTransferStatus::Downloading {
        downloaded_size: 1,
    });
    let pct = tiny_fraction.progress_percent();
    assert!(pct >= 0.0 && pct <= 1.0);

    // 6. i64::MIN boundaries
    let min_file = FileInfo::new(24, i64::MIN, i64::MIN).with_status(FileTransferStatus::Downloading {
        downloaded_size: i64::MIN,
    });
    assert_eq!(min_file.total_size(), 0);
    assert_eq!(min_file.progress_percent(), 0.0);
}

// ============================================================================
// SUITE 2: UI Progress Bar, Byte & Duration Formatting Fuzzing
// ============================================================================

#[test]
fn test_adversarial_render_progress_bar_fuzzing() {
    let test_percents = [
        -1000.0, -100.0, -1.0, -0.0001, 0.0, 0.0001, 1.0, 25.0, 50.0, 75.0,
        99.99, 100.0, 100.001, 150.0, 1000.0, 1e18,
        f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MIN_POSITIVE,
    ];

    let test_widths = [0, 1, 2, 3, 4, 5, 8, 10, 16, 20, 50, 80, 120];

    for &pct in &test_percents {
        for &w in &test_widths {
            let bar = render_progress_bar(pct, w);
            // Must always start with '[' and end with ']'
            assert!(bar.starts_with('['), "Bar must start with [: {}", bar);
            assert!(bar.ends_with(']'), "Bar must end with ]: {}", bar);

            // Width is clamped to at least 4 inside render_progress_bar, so length is w.max(4) + 2
            let expected_len = w.max(4) + 2;
            assert_eq!(
                bar.len(),
                expected_len,
                "Bar length mismatch for pct={} w={}: got len {} vs expected {}",
                pct,
                w,
                bar.len(),
                expected_len
            );
        }
    }
}

#[test]
fn test_adversarial_format_bytes_fuzzing() {
    let test_bytes = [
        i64::MIN, -100_000_000, -1, 0, 1, 100, 1023, 1024, 1025, 1536,
        1_048_575, 1_048_576, 1_048_577,
        1_073_741_823, 1_073_741_824, 1_073_741_825,
        100_000_000_000, 1_000_000_000_000, i64::MAX,
    ];

    for &b in &test_bytes {
        let formatted = format_bytes(b);
        assert!(!formatted.is_empty(), "format_bytes must never return empty string");
        if b <= 0 {
            assert_eq!(formatted, "0 B");
        } else {
            assert!(
                formatted.ends_with(" B")
                    || formatted.ends_with(" KB")
                    || formatted.ends_with(" MB")
                    || formatted.ends_with(" GB"),
                "Invalid unit suffix in formatted bytes: {}",
                formatted
            );
        }
    }
}

#[test]
fn test_adversarial_format_duration_fuzzing() {
    let test_seconds = [
        i32::MIN, -1000, -1, 0, 1, 9, 10, 59, 60, 61, 119, 120,
        3599, 3600, 3661, 86400, 999999, i32::MAX,
    ];

    for &s in &test_seconds {
        let formatted = format_duration(s);
        assert!(!formatted.is_empty(), "format_duration must never be empty");
        let parts: Vec<&str> = formatted.split(':').collect();
        assert_eq!(parts.len(), 2, "format_duration must be M:SS format: {}", formatted);
        assert_eq!(parts[1].len(), 2, "Seconds part must always be 2 digits: {}", formatted);
        let sec: u32 = parts[1].parse().expect("Seconds part must be valid integer");
        assert!(sec < 60, "Seconds part must be < 60, got {}", sec);
    }
}

// ============================================================================
// SUITE 3: High Volume Concurrent File Downloads Stress
// ============================================================================

#[tokio::test]
async fn test_adversarial_high_volume_concurrent_downloads_30_tasks() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };
    let (handle, mut event_rx) = Engine::start(config).await.expect("Failed to start engine");

    // Authenticate
    handle.send_phone_number("+1234567890").await.unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    handle.send_code("11111").await.unwrap();

    let auth_start = std::time::Instant::now();
    while auth_start.elapsed() < Duration::from_secs(2) {
        if handle.read_state().auth_state == AuthState::Ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(handle.read_state().auth_state, AuthState::Ready);

    // Concurrently trigger 30 file downloads (combination of pre-seeded IDs and dynamically allocated IDs)
    let file_ids: Vec<i32> = (1001..=1030).collect();
    let mut download_futures = Vec::new();

    for &id in &file_ids {
        let h = handle.clone();
        download_futures.push(tokio::spawn(async move {
            h.download_file(id, 1).await
        }));
    }

    // Await all download initiation calls
    for f in download_futures {
        let res = f.await.expect("Tokio join error");
        assert!(res.is_ok(), "Download request must succeed");
    }

    // Monitor for completion events across all 30 files
    let mut completed_files = std::collections::HashSet::new();
    let timeout_start = std::time::Instant::now();

    while timeout_start.elapsed() < Duration::from_secs(5) {
        if let Ok(event) = event_rx.recv().await {
            if let CoreEvent::FileUpdated(file) = event {
                if file.is_completed() && file_ids.contains(&file.id) {
                    completed_files.insert(file.id);
                    if completed_files.len() == file_ids.len() {
                        break;
                    }
                }
            }
        }
    }

    assert_eq!(
        completed_files.len(),
        file_ids.len(),
        "All 30 concurrent file downloads must complete without deadlock or drops"
    );

    // Verify all mock file payloads exist on disk
    for &id in &file_ids {
        let path = PathBuf::from(format!(".data/tdlib_files/mock_file_{}.bin", id));
        assert!(path.exists(), "Payload for file {} must exist at {:?}", id, path);
    }

    handle.stop().await.unwrap();
}

#[tokio::test]
async fn test_adversarial_duplicate_concurrent_downloads_same_file() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };
    let (handle, mut event_rx) = Engine::start(config).await.expect("Failed to start engine");

    handle.send_phone_number("+1234567890").await.unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    handle.send_code("11111").await.unwrap();

    let auth_start = std::time::Instant::now();
    while auth_start.elapsed() < Duration::from_secs(2) {
        if handle.read_state().auth_state == AuthState::Ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Spawn 15 simultaneous tasks requesting the exact same file (101)
    let mut duplicate_tasks = Vec::new();
    for _ in 0..15 {
        let h = handle.clone();
        duplicate_tasks.push(tokio::spawn(async move {
            h.download_file(101, 1).await
        }));
    }

    for t in duplicate_tasks {
        let res = t.await.expect("Join error");
        assert!(res.is_ok(), "Duplicate download call should not return Err");
    }

    // Wait for file 101 to complete or fail
    let mut completed = false;
    let mut failed = false;
    let timeout_start = std::time::Instant::now();
    while timeout_start.elapsed() < Duration::from_secs(3) {
        match tokio::time::timeout(Duration::from_millis(50), event_rx.recv()).await {
            Ok(Ok(CoreEvent::FileUpdated(file))) => {
                if file.id == 101 {
                    if file.is_completed() {
                        completed = true;
                        break;
                    } else if matches!(file.status, FileTransferStatus::Failed(_)) {
                        failed = true;
                    }
                }
            }
            Ok(Err(_)) => break,
            _ => {} // Timeout or other event
        }
    }

    println!("Duplicate download outcome for file 101: completed={}, failed={}", completed, failed);
    assert!(
        completed,
        "CONCURRENCY RACE DEFECT: Concurrent duplicate calls to download_file(101) resulted in spurious cancellation (MockBackend oneshot sender drop)"
    );
    handle.stop().await.unwrap();
}

#[tokio::test]
async fn test_adversarial_rapid_concurrent_cancellations_and_downloads_churn() {
    let mock = Arc::new(MockBackend::new());
    let (tx, mut rx) = broadcast::channel(500);
    mock.start(tx.clone()).await.unwrap();

    // 20 tasks rapidly interleaving download and cancel calls
    let mut join_handles = Vec::new();

    for i in 0..20 {
        let m = mock.clone();
        let file_id = 5000 + (i % 5);
        join_handles.push(tokio::spawn(async move {
            m.download_file(file_id, 32).await.unwrap();
            // Variable micro-delay before cancellation
            if i % 2 == 0 {
                tokio::time::sleep(Duration::from_millis(i as u64)).await;
                let _ = m.cancel_download_file(file_id).await;
            }
        }));
    }

    for h in join_handles {
        h.await.expect("Join error");
    }

    // Drain events for 300ms to ensure no panic or task leak
    let drain_start = tokio::time::Instant::now();
    while tokio::time::Instant::now() < drain_start + Duration::from_millis(300) {
        let _ = tokio::time::timeout(Duration::from_millis(20), rx.recv()).await;
    }

    // Verify system remains operational and can download a fresh file
    mock.download_file(9999, 32).await.unwrap();
    let mut saw_9999 = false;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(400);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
            if let CoreEvent::FileUpdated(file) = event {
                if file.id == 9999 && file.is_completed() {
                    saw_9999 = true;
                    break;
                }
            }
        }
    }
    assert!(saw_9999, "Backend must remain operational after churn");
}

// ============================================================================
// SUITE 4: Rapid UI Rendering Under Active Download Pressure
// ============================================================================

#[tokio::test]
async fn test_adversarial_rapid_ui_rendering_under_concurrent_downloads() {
    let mut state = CoreState::new();
    let chat_id = ChatId(42);
    let chat = Chat {
        id: chat_id,
        title: "Active Media Download Channel".to_string(),
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

    // Register 10 media messages covering all supported multimedia variants
    let mut initial_messages = Vec::new();
    for i in 1..=10 {
        let content = match i % 5 {
            0 => MessageContent::Photo {
                caption: format!("Photo caption {}", i),
                file_id: i,
                width: 1920,
                height: 1080,
            },
            1 => MessageContent::Document {
                file_name: format!("document_{}.pdf", i),
                file_size: 5_000_000,
                caption: format!("Doc caption {}", i),
                file_id: i,
            },
            2 => MessageContent::Audio {
                file_id: i,
                title: Some(format!("Track {}", i)),
                performer: Some("Artist".to_string()),
                duration: 180 + i,
                file_size: 8_000_000,
            },
            3 => MessageContent::Voice {
                caption: format!("Voice {}", i),
                duration: 30 + i,
                file_id: i,
            },
            _ => MessageContent::Sticker {
                emoji: "🚀".to_string(),
                file_id: i,
            },
        };

        initial_messages.push(Message {
            id: MessageId(i as i64),
            chat_id,
            sender: Sender::User(UserId(100 + i as i64)),
            date: 1700000000 + (i as i64 * 60),
            content,
            is_outgoing: i % 2 == 0,
            status: MessageStatus::Read,
            reply_to_message_id: None,
            can_be_edited: false,
            can_be_deleted: false,
        });

        // Initialize file in downloading status
        state.update_file(
            FileInfo::new(i, 10_000_000, 10_000_000).with_status(FileTransferStatus::Downloading {
                downloaded_size: (i as i64) * 500_000,
            }),
        );
    }
    state.insert_history_messages(chat_id, initial_messages);

    // Setup diverse terminal rendering viewports
    let viewports = [
        Rect::new(0, 0, 0, 0),       // 0x0 collapsed
        Rect::new(0, 0, 1, 1),       // 1x1 minimal
        Rect::new(0, 0, 10, 4),      // tiny window
        Rect::new(0, 0, 40, 15),     // compact
        Rect::new(0, 0, 80, 24),     // standard terminal
        Rect::new(0, 0, 120, 40),    // wide terminal
        Rect::new(0, 0, 250, 80),    // massive terminal
    ];

    let scroll_offsets = [0, 1, 5, 20, 100, 500, 1000];

    // Rapid rendering stress loop (150 iterations) while files mutate
    for iteration in 0..150 {
        // Mutate files actively to simulate concurrent background download updates
        for id in 1..=10 {
            let pct = (iteration * 7 + (id as usize * 13)) % 105;
            let status = if pct >= 100 {
                FileTransferStatus::Completed
            } else {
                FileTransferStatus::Downloading {
                    downloaded_size: (pct as i64) * 100_000,
                }
            };
            state.update_file(FileInfo::new(id, 10_000_000, 10_000_000).with_status(status));
        }

        let area = viewports[iteration % viewports.len()];
        let scroll = scroll_offsets[iteration % scroll_offsets.len()];
        let is_focused = iteration % 2 == 0;

        let backend = TestBackend::new(area.width.max(1), area.height.max(1));
        let mut terminal = Terminal::new(backend).expect("Failed to create TestBackend");

        terminal
            .draw(|f| {
                render_messages(f, &state, Some(chat_id), 0, scroll, is_focused, area);
            })
            .unwrap_or_else(|e| {
                panic!(
                    "render_messages panicked on iteration {} (area: {:?}, scroll: {}): {:?}",
                    iteration, area, scroll, e
                )
            });
    }
}

/// Adversarial stress test demonstrating the integer overflow vulnerability in `render_messages`
/// when `scroll_offset` is large (e.g. u16::MAX or usize::MAX / 2).
/// In `render_messages`: `scroll((scroll_offset as u16, 0))` is passed without clamping against
/// `area.height` or total lines, which triggers `ratatui`'s debug integer overflow:
/// "attempt to add with overflow at ratatui-0.29.0\src\widgets\paragraph.rs:483:21".
#[test]
fn test_adversarial_render_messages_scroll_overflow_vulnerability() {
    let mut state = CoreState::new();
    let chat_id = ChatId(1);
    let chat = Chat {
        id: chat_id,
        title: "Overflow Test Chat".to_string(),
        chat_type: ChatType::Private,
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

    let msg = Message {
        id: MessageId(1),
        chat_id,
        sender: Sender::User(UserId(1)),
        date: 1700000000,
        content: MessageContent::Photo {
            caption: "Test photo".to_string(),
            file_id: 1,
            width: 800,
            height: 600,
        },
        is_outgoing: false,
        status: MessageStatus::Read,
        reply_to_message_id: None,
        can_be_edited: false,
        can_be_deleted: false,
    };
    state.insert_history_messages(chat_id, vec![msg]);
    state.update_file(FileInfo::new(1, 1000, 1000).with_status(FileTransferStatus::Completed));

    let area = Rect::new(0, 0, 80, 24);
    let backend = TestBackend::new(area.width, area.height);
    let mut terminal = Terminal::new(backend).unwrap();

    // 1. Normal scroll offset must render without panic
    terminal
        .draw(|f| {
            render_messages(f, &state, Some(chat_id), 0, 10, true, area);
        })
        .expect("Normal scroll must succeed");

    // 2. Unbounded scroll offset (u16::MAX or usize::MAX / 2) triggers integer overflow in Ratatui
    // Catch the panic empirically
    let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = terminal.draw(|f| {
            render_messages(f, &state, Some(chat_id), 0, usize::MAX / 2, true, area);
        });
    }));

    assert!(
        panic_result.is_ok(),
        "CRITICAL PANIC VULNERABILITY: render_messages panics with 'attempt to add with overflow' at ratatui-0.29.0\\src\\widgets\\paragraph.rs:483:21 when scroll_offset is large"
    );
}
