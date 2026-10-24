#![allow(dead_code)]

use std::path::PathBuf;
use std::time::Duration;
use serde_json::json;

use tg_core::{
    AuthState, ChatId, Config, CoreEvent, CoreState, Engine,
    FileInfo, FileTransferStatus, MessageContent, TDLibBackend,
};

#[path = "../crates/tg-tui/src/text/mod.rs"]
pub mod text;

#[path = "../crates/tg-tui/src/ui/messages.rs"]
pub mod messages_ui;

use messages_ui::{format_bytes, format_duration, render_progress_bar};

// ============================================================================
// 1. TDLib JSON Multimedia Message Parsing Tests
// ============================================================================

#[test]
fn test_parse_photo_message() {
    let json = json!({
        "@type": "messagePhoto",
        "caption": {
            "@type": "formattedText",
            "text": "Stunning sunset in the mountains"
        },
        "photo": {
            "@type": "photo",
            "has_stickers": false,
            "minithumbnail": null,
            "sizes": [
                {
                    "@type": "photoSize",
                    "type": "s",
                    "photo": { "@type": "file", "id": 1001, "size": 15000 },
                    "width": 320,
                    "height": 180
                },
                {
                    "@type": "photoSize",
                    "type": "m",
                    "photo": { "@type": "file", "id": 1002, "size": 60000 },
                    "width": 640,
                    "height": 360
                },
                {
                    "@type": "photoSize",
                    "type": "x",
                    "photo": { "@type": "file", "id": 1003, "size": 250000 },
                    "width": 1280,
                    "height": 720
                }
            ]
        }
    });

    let content = TDLibBackend::parse_message_content(&json);
    match content {
        MessageContent::Photo {
            caption,
            file_id,
            width,
            height,
        } => {
            assert_eq!(caption, "Stunning sunset in the mountains");
            // Must pick largest/last photo size
            assert_eq!(file_id, 1003);
            assert_eq!(width, 1280);
            assert_eq!(height, 720);
        }
        other => panic!("Expected MessageContent::Photo, got {:?}", other),
    }
}

#[test]
fn test_parse_document_message() {
    let json = json!({
        "@type": "messageDocument",
        "caption": {
            "@type": "formattedText",
            "text": "Quarterly Financial Overview"
        },
        "document": {
            "@type": "document",
            "file_name": "quarterly_report.pdf",
            "mime_type": "application/pdf",
            "document": {
                "@type": "file",
                "id": 2001,
                "size": 1843200,
                "expected_size": 1843200
            }
        }
    });

    let content = TDLibBackend::parse_message_content(&json);
    match content {
        MessageContent::Document {
            caption,
            file_name,
            file_id,
            file_size,
        } => {
            assert_eq!(caption, "Quarterly Financial Overview");
            assert_eq!(file_name, "quarterly_report.pdf");
            assert_eq!(file_id, 2001);
            assert_eq!(file_size, 1843200);
        }
        other => panic!("Expected MessageContent::Document, got {:?}", other),
    }
}

#[test]
fn test_parse_voice_note_message() {
    let json = json!({
        "@type": "messageVoiceNote",
        "caption": {
            "@type": "formattedText",
            "text": "Important update from team lead"
        },
        "voice_note": {
            "@type": "voiceNote",
            "duration": 42,
            "waveform": "",
            "mime_type": "audio/ogg",
            "voice": {
                "@type": "file",
                "id": 3001,
                "size": 262144
            }
        }
    });

    let content = TDLibBackend::parse_message_content(&json);
    match content {
        MessageContent::Voice {
            caption,
            duration,
            file_id,
        } => {
            assert_eq!(caption, "Important update from team lead");
            assert_eq!(duration, 42);
            assert_eq!(file_id, 3001);
        }
        other => panic!("Expected MessageContent::Voice, got {:?}", other),
    }
}

#[test]
fn test_parse_audio_message() {
    let json = json!({
        "@type": "messageAudio",
        "caption": {
            "@type": "formattedText",
            "text": ""
        },
        "audio": {
            "@type": "audio",
            "duration": 212,
            "title": "Never Gonna Give You Up",
            "performer": "Rick Astley",
            "file_name": "never_gonna_give_you_up.mp3",
            "mime_type": "audio/mpeg",
            "audio": {
                "@type": "file",
                "id": 4001,
                "size": 4718592
            }
        }
    });

    let content = TDLibBackend::parse_message_content(&json);
    match content {
        MessageContent::Audio {
            file_id,
            title,
            performer,
            duration,
            file_size,
        } => {
            assert_eq!(file_id, 4001);
            assert_eq!(title.as_deref(), Some("Never Gonna Give You Up"));
            assert_eq!(performer.as_deref(), Some("Rick Astley"));
            assert_eq!(duration, 212);
            assert_eq!(file_size, 4718592);
        }
        other => panic!("Expected MessageContent::Audio, got {:?}", other),
    }
}

#[test]
fn test_parse_sticker_message() {
    let json = json!({
        "@type": "messageSticker",
        "sticker": {
            "@type": "sticker",
            "set_id": 987654321,
            "width": 512,
            "height": 512,
            "emoji": "🦀",
            "sticker": {
                "@type": "file",
                "id": 5001,
                "size": 65536
            }
        }
    });

    let content = TDLibBackend::parse_message_content(&json);
    match content {
        MessageContent::Sticker { emoji, file_id } => {
            assert_eq!(emoji, "🦀");
            assert_eq!(file_id, 5001);
        }
        other => panic!("Expected MessageContent::Sticker, got {:?}", other),
    }
}

#[test]
fn test_parse_unsupported_content_fallback() {
    let json = json!({
        "@type": "messageVideoNote",
        "video_note": {
            "duration": 10
        }
    });

    let content = TDLibBackend::parse_message_content(&json);
    match content {
        MessageContent::Unsupported { raw_type } => {
            assert_eq!(raw_type, "messageVideoNote");
        }
        other => panic!("Expected MessageContent::Unsupported, got {:?}", other),
    }
}

#[test]
fn test_parse_text_with_formatting_entities() {
    let json = json!({
        "@type": "messageText",
        "text": {
            "@type": "formattedText",
            "text": "Hello bold world",
            "entities": [
                {
                    "@type": "textEntity",
                    "offset": 6,
                    "length": 4,
                    "type": {
                        "@type": "textEntityTypeBold"
                    }
                }
            ]
        }
    });

    let content = TDLibBackend::parse_message_content(&json);
    match content {
        MessageContent::Text { text, entities } => {
            assert_eq!(text, "Hello bold world");
            assert_eq!(entities.len(), 1);
            assert_eq!(entities[0].offset, 6);
            assert_eq!(entities[0].length, 4);
            assert_eq!(entities[0].entity_type, "textEntityTypeBold");
        }
        other => panic!("Expected MessageContent::Text, got {:?}", other),
    }
}

// ============================================================================
// 2. TDLib updateFile JSON Parsing Tests
// ============================================================================

#[test]
fn test_parse_update_file_downloading() {
    let json = json!({
        "@type": "file",
        "id": 12345,
        "size": 1000000,
        "expected_size": 1000000,
        "local": {
            "@type": "localFile",
            "path": "",
            "can_be_downloaded": true,
            "can_be_deleted": false,
            "is_downloading_active": true,
            "is_downloading_completed": false,
            "downloaded_size": 450000
        },
        "remote": {
            "@type": "remoteFile",
            "id": "rem_123",
            "is_uploading_active": false,
            "is_uploading_completed": true,
            "uploaded_size": 1000000
        }
    });

    let file_info = TDLibBackend::parse_file(&json).expect("Failed to parse FileInfo");
    assert_eq!(file_info.id, 12345);
    assert_eq!(file_info.size, 1000000);
    assert_eq!(file_info.local_path, None);
    assert!(file_info.is_downloading());
    assert!(!file_info.is_completed());
    assert_eq!(file_info.downloaded_size(), 450000);
    assert!((file_info.progress_percent() - 45.0).abs() < 0.01);
}

#[test]
fn test_parse_update_file_completed() {
    let json = json!({
        "@type": "file",
        "id": 54321,
        "size": 204800,
        "expected_size": 204800,
        "local": {
            "@type": "localFile",
            "path": ".data/tdlib_files/documents/sample.pdf",
            "can_be_downloaded": false,
            "can_be_deleted": true,
            "is_downloading_active": false,
            "is_downloading_completed": true,
            "downloaded_size": 204800
        }
    });

    let file_info = TDLibBackend::parse_file(&json).expect("Failed to parse FileInfo");
    assert_eq!(file_info.id, 54321);
    assert!(file_info.is_completed());
    assert!(!file_info.is_downloading());
    assert_eq!(file_info.local_path, Some(PathBuf::from(".data/tdlib_files/documents/sample.pdf")));
    assert_eq!(file_info.progress_percent(), 100.0);
}

#[test]
fn test_parse_update_file_uploading() {
    let json = json!({
        "@type": "file",
        "id": 9999,
        "size": 500000,
        "expected_size": 500000,
        "local": {
            "@type": "localFile",
            "path": "/local/file.mp4",
            "is_downloading_active": false,
            "is_downloading_completed": false,
            "downloaded_size": 0
        },
        "remote": {
            "@type": "remoteFile",
            "is_uploading_active": true,
            "is_uploading_completed": false,
            "uploaded_size": 250000
        }
    });

    let file_info = TDLibBackend::parse_file(&json).expect("Failed to parse FileInfo");
    assert!(file_info.is_uploading());
    assert_eq!(file_info.status, FileTransferStatus::Uploading { uploaded_size: 250000 });
}

#[test]
fn test_parse_update_file_idle() {
    let json = json!({
        "@type": "file",
        "id": 7777,
        "size": 1024,
        "expected_size": 1024,
        "local": {
            "@type": "localFile",
            "path": "",
            "is_downloading_active": false,
            "is_downloading_completed": false,
            "downloaded_size": 0
        }
    });

    let file_info = TDLibBackend::parse_file(&json).expect("Failed to parse FileInfo");
    assert_eq!(file_info.status, FileTransferStatus::Idle);
    assert!(file_info.can_be_downloaded());
}

// ============================================================================
// 3. FileInfo Domain & Progress Edge Cases
// ============================================================================

#[test]
fn test_file_info_progress_edge_cases() {
    // 1. Zero size
    let f_zero = FileInfo::new(1, 0, 0);
    assert_eq!(f_zero.progress_percent(), 0.0);

    // 2. Expected size fallback
    let mut f_exp = FileInfo::new(2, 0, 1000);
    f_exp.status = FileTransferStatus::Downloading { downloaded_size: 500 };
    assert_eq!(f_exp.progress_percent(), 50.0);

    // 3. Completed always 100%
    let mut f_done = FileInfo::new(3, 1000, 1000);
    f_done.status = FileTransferStatus::Completed;
    assert_eq!(f_done.progress_percent(), 100.0);

    // 4. Downloaded exceeds total clamped to 100%
    let mut f_overflow = FileInfo::new(4, 100, 100);
    f_overflow.status = FileTransferStatus::Downloading { downloaded_size: 200 };
    assert_eq!(f_overflow.progress_percent(), 100.0);
}

// ============================================================================
// 4. CoreState File Cache & Event Reduction Tests
// ============================================================================

#[test]
fn test_core_state_file_cache() {
    let mut state = CoreState::new();

    let file1 = FileInfo::new(101, 50000, 50000);
    state.apply_update(&CoreEvent::FileUpdated(file1.clone()));

    assert_eq!(state.get_file(101), Some(&file1));
    assert_eq!(state.get_file(999), None);

    // Progressive update
    let file1_updated = file1.with_status(FileTransferStatus::Downloading { downloaded_size: 25000 });
    state.apply_update(&CoreEvent::FileUpdated(file1_updated.clone()));

    let cached = state.get_file(101).unwrap();
    assert_eq!(cached.downloaded_size(), 25000);
    assert_eq!(cached.progress_percent(), 50.0);
}

// ============================================================================
// 5. Mock Backend Media Initialization & History Loading Tests
// ============================================================================

#[tokio::test]
async fn test_mock_backend_media_initialization_and_history() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, _rx) = Engine::start(config).await.expect("Failed to start engine");

    // Authenticate
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

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify initial FileInfo entries are registered in state
    let state = handle.read_state();
    assert!(state.get_file(101).is_some(), "Document file 101 must be in state");
    assert!(state.get_file(201).is_some(), "Audio file 201 must be in state");
    assert!(state.get_file(301).is_some(), "Photo file 301 must be in state");
    assert!(state.get_file(401).is_some(), "Voice file 401 must be in state");
    assert!(state.get_file(501).is_some(), "Sticker file 501 must be in state");
    drop(state);

    // Verify history of Chat 1 contains Document
    handle.load_history(ChatId(1), None, 10).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let msgs_c1 = handle.read_state().get_messages(ChatId(1)).cloned().unwrap();
    let has_doc = msgs_c1.iter().any(|m| matches!(&m.content, MessageContent::Document { file_id: 101, .. }));
    assert!(has_doc, "Chat 1 must contain document message 101");

    // Verify history of Chat 2 contains Audio and Sticker
    handle.load_history(ChatId(2), None, 10).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let msgs_c2 = handle.read_state().get_messages(ChatId(2)).cloned().unwrap();
    let has_audio = msgs_c2.iter().any(|m| matches!(&m.content, MessageContent::Audio { file_id: 201, .. }));
    let has_sticker = msgs_c2.iter().any(|m| matches!(&m.content, MessageContent::Sticker { file_id: 501, .. }));
    assert!(has_audio, "Chat 2 must contain audio message 201");
    assert!(has_sticker, "Chat 2 must contain sticker message 501");

    // Verify history of Chat 3 contains Photo
    handle.load_history(ChatId(3), None, 10).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let msgs_c3 = handle.read_state().get_messages(ChatId(3)).cloned().unwrap();
    let has_photo = msgs_c3.iter().any(|m| matches!(&m.content, MessageContent::Photo { file_id: 301, .. }));
    assert!(has_photo, "Chat 3 must contain photo message 301");

    // Verify history of Chat 4 contains Voice
    handle.load_history(ChatId(4), None, 10).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let msgs_c4 = handle.read_state().get_messages(ChatId(4)).cloned().unwrap();
    let has_voice = msgs_c4.iter().any(|m| matches!(&m.content, MessageContent::Voice { file_id: 401, .. }));
    assert!(has_voice, "Chat 4 must contain voice note message 401");

    handle.stop().await.unwrap();
}

// ============================================================================
// 6. Progressive File Download Simulation & Disk Verification Tests
// ============================================================================

#[tokio::test]
async fn test_mock_backend_progressive_file_download() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, mut event_rx) = Engine::start(config).await.expect("Failed to start engine");

    // Authenticate
    handle.send_phone_number("+1234567890").await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.send_code("11111").await.unwrap();

    // Wait until ready
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if handle.read_state().auth_state == AuthState::Ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Trigger download for document file 101
    handle.download_file(101, 1).await.expect("Failed to initiate download");

    let mut saw_downloading = false;
    let mut saw_completed = false;
    let mut downloaded_path: Option<PathBuf> = None;

    let timeout_start = std::time::Instant::now();
    while timeout_start.elapsed() < Duration::from_secs(3) {
        if let Ok(event) = event_rx.recv().await {
            if let CoreEvent::FileUpdated(file) = event {
                if file.id == 101 {
                    if file.is_downloading() {
                        saw_downloading = true;
                    }
                    if file.is_completed() {
                        saw_completed = true;
                        downloaded_path = file.local_path;
                        break;
                    }
                }
            }
        }
    }

    assert!(saw_downloading, "Must emit progressive Downloading state");
    assert!(saw_completed, "Must emit Completed state");

    // Verify local storage in .data/tdlib_files
    let path = downloaded_path.expect("Completed file must provide local_path");
    assert!(path.exists(), "Downloaded simulated file must physically exist on disk at {:?}", path);

    // Verify file content is genuine simulated payload
    let content = std::fs::read_to_string(&path).expect("Failed to read simulated file payload");
    assert!(content.contains("file_id=101"), "Simulated payload must identify file_id");

    // Verify state cache is updated
    let state = handle.read_state();
    let cached = state.get_file(101).expect("File 101 must exist in state");
    assert!(cached.is_completed());
    assert_eq!(cached.progress_percent(), 100.0);

    handle.stop().await.unwrap();
}

// ============================================================================
// 7. Mock Backend Download Cancellation Tests
// ============================================================================

#[tokio::test]
async fn test_mock_backend_download_cancellation() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, mut event_rx) = Engine::start(config).await.expect("Failed to start engine");

    handle.send_phone_number("+1234567890").await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.send_code("11111").await.unwrap();

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if handle.read_state().auth_state == AuthState::Ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Start download for audio file 201
    handle.download_file(201, 1).await.unwrap();

    // Immediately cancel download
    handle.cancel_download_file(201).await.unwrap();

    let mut saw_cancellation = false;
    let timeout_start = std::time::Instant::now();
    while timeout_start.elapsed() < Duration::from_secs(2) {
        if let Ok(event) = event_rx.recv().await {
            if let CoreEvent::FileUpdated(file) = event {
                if file.id == 201 {
                    if let FileTransferStatus::Failed(reason) = file.status {
                        assert!(reason.contains("cancelled"));
                        saw_cancellation = true;
                        break;
                    }
                }
            }
        }
    }

    assert!(saw_cancellation, "Cancellation must result in FileTransferStatus::Failed");
    handle.stop().await.unwrap();
}

// ============================================================================
// 8. TUI Formatting Functions & Visual Badges Tests
// ============================================================================

#[test]
fn test_tui_format_bytes() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(500), "500 B");
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1536), "1.5 KB");
    assert_eq!(format_bytes(1048576), "1.0 MB");
    assert_eq!(format_bytes(1843200), "1.8 MB");
    assert_eq!(format_bytes(4718592), "4.5 MB");
    assert_eq!(format_bytes(1073741824), "1.0 GB");
}

#[test]
fn test_tui_format_duration() {
    assert_eq!(format_duration(0), "0:00");
    assert_eq!(format_duration(9), "0:09");
    assert_eq!(format_duration(42), "0:42");
    assert_eq!(format_duration(60), "1:00");
    assert_eq!(format_duration(212), "3:32");
    assert_eq!(format_duration(3605), "60:05");
}

#[test]
fn test_tui_render_progress_bar() {
    assert_eq!(render_progress_bar(0.0, 10), "[          ]");
    assert_eq!(render_progress_bar(50.0, 10), "[====>     ]");
    assert_eq!(render_progress_bar(100.0, 10), "[==========]");
}

// ============================================================================
// 9. Concurrent Media Downloads Stress Test
// ============================================================================

#[tokio::test]
async fn test_concurrent_media_downloads() {
    let config = Config {
        use_mock: true,
        ..Default::default()
    };

    let (handle, mut event_rx) = Engine::start(config).await.expect("Failed to start engine");

    handle.send_phone_number("+1234567890").await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.send_code("11111").await.unwrap();

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if handle.read_state().auth_state == AuthState::Ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Download both photo (301) and voice note (401) concurrently
    tokio::try_join!(
        handle.download_file(301, 1),
        handle.download_file(401, 1)
    ).expect("Concurrent download requests failed");

    let mut completed_301 = false;
    let mut completed_401 = false;

    let timeout_start = std::time::Instant::now();
    while timeout_start.elapsed() < Duration::from_secs(3) {
        if let Ok(event) = event_rx.recv().await {
            if let CoreEvent::FileUpdated(file) = event {
                if file.id == 301 && file.is_completed() {
                    completed_301 = true;
                }
                if file.id == 401 && file.is_completed() {
                    completed_401 = true;
                }
                if completed_301 && completed_401 {
                    break;
                }
            }
        }
    }

    assert!(completed_301, "File 301 must complete download concurrently");
    assert!(completed_401, "File 401 must complete download concurrently");

    // Both files exist on disk
    let file_301_path = PathBuf::from(".data/tdlib_files/mock_file_301.bin");
    let file_401_path = PathBuf::from(".data/tdlib_files/mock_file_401.bin");
    assert!(file_301_path.exists());
    assert!(file_401_path.exists());

    handle.stop().await.unwrap();
}
