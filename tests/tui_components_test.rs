#![allow(dead_code)]

#[path = "../crates/tg-tui/src/text/entities.rs"]
mod entities;

#[path = "../crates/tg-tui/src/ui/qr.rs"]
mod qr;

#[path = "../crates/tg-tui/src/ui/chat_list.rs"]
mod chat_list;

use chat_list::{filter_chats, tab_unread_count, ChatFilterTab};
use entities::{
    byte_index_to_utf16_offset, format_text_entities, parse_entity_type, utf16_offset_to_byte_index,
    ParsedEntityType,
};
use qr::{generate_qr_halfblock_lines, render_qr_widget};
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;
use tg_core::{Chat, ChatId, ChatPosition, ChatType, CoreState, TextEntity};

fn make_chat(
    id: i64,
    title: &str,
    chat_type: ChatType,
    unread_count: i32,
    order: u64,
    is_pinned: bool,
) -> Chat {
    Chat {
        id: ChatId(id),
        title: title.to_string(),
        chat_type,
        unread_count,
        last_read_inbox_message_id: 0,
        last_read_outbox_message_id: 0,
        last_message: None,
        position: Some(ChatPosition { order, is_pinned }),
        is_muted: false,
        has_unread_mention: false,
        folder_ids: Vec::new(),
    }
}

// =========================================================================
// 1. Chat Filtering Tabs & Unread Badge Tests (Feature 9)
// =========================================================================

#[test]
fn test_chat_filter_tab_matching() {
    let private_chat = make_chat(1, "Alice", ChatType::Private, 0, 100, false);
    let secret_chat = make_chat(2, "Bob (Secret)", ChatType::Secret, 0, 90, false);
    let basic_group = make_chat(3, "Family", ChatType::BasicGroup, 0, 80, false);
    let super_group = make_chat(4, "Rust Devs", ChatType::Supergroup, 0, 70, false);
    let channel = make_chat(5, "Rust News", ChatType::Channel, 0, 60, false);

    // Tab All matches everything
    assert!(ChatFilterTab::All.matches(&private_chat));
    assert!(ChatFilterTab::All.matches(&secret_chat));
    assert!(ChatFilterTab::All.matches(&basic_group));
    assert!(ChatFilterTab::All.matches(&super_group));
    assert!(ChatFilterTab::All.matches(&channel));

    // Tab Personal matches Private and Secret
    assert!(ChatFilterTab::Personal.matches(&private_chat));
    assert!(ChatFilterTab::Personal.matches(&secret_chat));
    assert!(!ChatFilterTab::Personal.matches(&basic_group));
    assert!(!ChatFilterTab::Personal.matches(&super_group));
    assert!(!ChatFilterTab::Personal.matches(&channel));

    // Tab Groups matches BasicGroup and Supergroup
    assert!(!ChatFilterTab::Groups.matches(&private_chat));
    assert!(!ChatFilterTab::Groups.matches(&secret_chat));
    assert!(ChatFilterTab::Groups.matches(&basic_group));
    assert!(ChatFilterTab::Groups.matches(&super_group));
    assert!(!ChatFilterTab::Groups.matches(&channel));

    // Tab Channels matches Channel
    assert!(!ChatFilterTab::Channels.matches(&private_chat));
    assert!(!ChatFilterTab::Channels.matches(&secret_chat));
    assert!(!ChatFilterTab::Channels.matches(&basic_group));
    assert!(!ChatFilterTab::Channels.matches(&super_group));
    assert!(ChatFilterTab::Channels.matches(&channel));
}

#[test]
fn test_tab_unread_badge_aggregation() {
    let mut state = CoreState::new();

    // Insert chats with varying unread counts
    state.insert_or_update_chat(make_chat(10, "Alice", ChatType::Private, 3, 100, false));
    state.insert_or_update_chat(make_chat(20, "Bob", ChatType::Secret, 2, 90, false));
    state.insert_or_update_chat(make_chat(30, "Team", ChatType::BasicGroup, 5, 80, false));
    state.insert_or_update_chat(make_chat(40, "Rustaceans", ChatType::Supergroup, 10, 70, false));
    state.insert_or_update_chat(make_chat(50, "Announcements", ChatType::Channel, 7, 60, false));

    // Personal: 3 + 2 = 5
    assert_eq!(tab_unread_count(&state, ChatFilterTab::Personal), 5);

    // Groups: 5 + 10 = 15
    assert_eq!(tab_unread_count(&state, ChatFilterTab::Groups), 15);

    // Channels: 7
    assert_eq!(tab_unread_count(&state, ChatFilterTab::Channels), 7);

    // All: 5 + 15 + 7 = 27
    assert_eq!(tab_unread_count(&state, ChatFilterTab::All), 27);
}

#[test]
fn test_chat_filter_list_ordering_and_clamping() {
    let mut state = CoreState::new();

    // Insert chats in specific order
    state.insert_or_update_chat(make_chat(1, "Pinned Channel", ChatType::Channel, 0, 50, true));
    state.insert_or_update_chat(make_chat(2, "Alice", ChatType::Private, 0, 100, false));
    state.insert_or_update_chat(make_chat(3, "Bob", ChatType::Private, 0, 90, false));
    state.insert_or_update_chat(make_chat(4, "Rust Group", ChatType::Supergroup, 0, 80, false));

    // In All tab: pinned first, then by order descending
    let all_filtered = filter_chats(&state, ChatFilterTab::All);
    assert_eq!(all_filtered, vec![ChatId(1), ChatId(2), ChatId(3), ChatId(4)]);

    // In Personal tab: only Alice and Bob
    let personal_filtered = filter_chats(&state, ChatFilterTab::Personal);
    assert_eq!(personal_filtered, vec![ChatId(2), ChatId(3)]);

    // In Groups tab: only Rust Group
    let groups_filtered = filter_chats(&state, ChatFilterTab::Groups);
    assert_eq!(groups_filtered, vec![ChatId(4)]);

    // In Channels tab: only Pinned Channel
    let channels_filtered = filter_chats(&state, ChatFilterTab::Channels);
    assert_eq!(channels_filtered, vec![ChatId(1)]);
}

#[test]
fn test_tab_cycle_navigation() {
    let tab = ChatFilterTab::All;
    assert_eq!(tab.next(), ChatFilterTab::Personal);
    assert_eq!(tab.next().next(), ChatFilterTab::Groups);
    assert_eq!(tab.next().next().next(), ChatFilterTab::Channels);
    assert_eq!(tab.next().next().next().next(), ChatFilterTab::All);

    assert_eq!(tab.prev(), ChatFilterTab::Channels);
    assert_eq!(tab.prev().prev(), ChatFilterTab::Groups);
}

// =========================================================================
// 2. Rich Text Formatting & UTF-16 Code Unit Safety Tests (Feature 12)
// =========================================================================

#[test]
fn test_utf16_offset_conversion_ascii() {
    let text = "Hello, world!";
    assert_eq!(utf16_offset_to_byte_index(text, 0), 0);
    assert_eq!(utf16_offset_to_byte_index(text, 5), 5);
    assert_eq!(utf16_offset_to_byte_index(text, 13), 13);
    assert_eq!(utf16_offset_to_byte_index(text, 100), 13); // Beyond length

    assert_eq!(byte_index_to_utf16_offset(text, 0), 0);
    assert_eq!(byte_index_to_utf16_offset(text, 5), 5);
    assert_eq!(byte_index_to_utf16_offset(text, 13), 13);
}

#[test]
fn test_utf16_offset_conversion_multibyte_and_emoji() {
    // "Hi 👋! Rust 🦀"
    // "Hi " -> 3 bytes, 3 code units
    // "👋" -> 4 bytes, 2 UTF-16 code units (surrogate pair)
    // "! Rust " -> 7 bytes, 7 code units
    // "🦀" -> 4 bytes, 2 UTF-16 code units
    let text = "Hi 👋! Rust 🦀";
    assert_eq!(text.len(), 18); // 3 + 4 + 7 + 4 = 18 bytes

    // UTF-16 offset 3 should be start of 👋 (byte index 3)
    assert_eq!(utf16_offset_to_byte_index(text, 3), 3);

    // UTF-16 offset 5 (3 + 2 code units) should be start of "!" (byte index 7)
    assert_eq!(utf16_offset_to_byte_index(text, 5), 7);
    assert_eq!(&text[3..7], "👋");

    // UTF-16 offset 12 (5 + 7 code units) should be start of 🦀 (byte index 14)
    assert_eq!(utf16_offset_to_byte_index(text, 12), 14);

    // UTF-16 offset 14 (12 + 2 code units) should be end of string (byte index 18)
    assert_eq!(utf16_offset_to_byte_index(text, 14), 18);
    assert_eq!(&text[14..18], "🦀");
}

#[test]
fn test_parse_entity_type_normalization() {
    assert_eq!(parse_entity_type("bold"), Some(ParsedEntityType::Bold));
    assert_eq!(parse_entity_type("textEntityTypeBold"), Some(ParsedEntityType::Bold));
    assert_eq!(parse_entity_type("textEntityTypeItalic"), Some(ParsedEntityType::Italic));
    assert_eq!(parse_entity_type("code"), Some(ParsedEntityType::Code));
    assert_eq!(parse_entity_type("textEntityTypeCode"), Some(ParsedEntityType::Code));
    assert_eq!(parse_entity_type("pre"), Some(ParsedEntityType::Pre));
    assert_eq!(parse_entity_type("textEntityTypePre"), Some(ParsedEntityType::Pre));
    assert_eq!(parse_entity_type("textEntityTypeTextUrl"), Some(ParsedEntityType::TextUrl));
    assert_eq!(parse_entity_type("url"), Some(ParsedEntityType::TextUrl));
    assert_eq!(parse_entity_type("mention"), Some(ParsedEntityType::Mention));
    assert_eq!(parse_entity_type("spoiler"), Some(ParsedEntityType::Spoiler));
    assert_eq!(parse_entity_type("unknownCustomType"), None);
}

#[test]
fn test_rich_text_single_entity_formatting() {
    let text = "Check this bold text!";
    // "bold" is at offset 11, length 4 ("bold")
    let entities = vec![TextEntity {
        offset: 11,
        length: 4,
        entity_type: "bold".to_string(),
    }];

    let lines = format_text_entities(text, &entities);
    assert_eq!(lines.len(), 1);
    let spans = &lines[0].spans;
    assert_eq!(spans.len(), 3);

    // 1: "Check this "
    assert_eq!(spans[0].content, "Check this ");
    assert!(!spans[0].style.add_modifier.contains(Modifier::BOLD));

    // 2: "bold"
    assert_eq!(spans[1].content, "bold");
    assert!(spans[1].style.add_modifier.contains(Modifier::BOLD));

    // 3: " text!"
    assert_eq!(spans[2].content, " text!");
    assert!(!spans[2].style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_rich_text_nested_entities_interval_slicing() {
    // "This is bold and italic combined"
    // Bold covers "bold and italic" (offset 8, length 15)
    // Italic covers "italic" (offset 17, length 6)
    let text = "This is bold and italic combined";
    let entities = vec![
        TextEntity {
            offset: 8,
            length: 15,
            entity_type: "bold".to_string(),
        },
        TextEntity {
            offset: 17,
            length: 6,
            entity_type: "italic".to_string(),
        },
    ];

    let lines = format_text_entities(text, &entities);
    assert_eq!(lines.len(), 1);
    let spans = &lines[0].spans;

    // Span 0: "This is " (plain)
    assert_eq!(spans[0].content, "This is ");
    assert!(!spans[0].style.add_modifier.contains(Modifier::BOLD));

    // Span 1: "bold and " (bold only)
    assert_eq!(spans[1].content, "bold and ");
    assert!(spans[1].style.add_modifier.contains(Modifier::BOLD));
    assert!(!spans[1].style.add_modifier.contains(Modifier::ITALIC));

    // Span 2: "italic" (bold AND italic)
    assert_eq!(spans[2].content, "italic");
    assert!(spans[2].style.add_modifier.contains(Modifier::BOLD));
    assert!(spans[2].style.add_modifier.contains(Modifier::ITALIC));

    // Span 3: " combined" (plain)
    assert_eq!(spans[3].content, " combined");
    assert!(!spans[3].style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_rich_text_multiline_entities() {
    let text = "Line 1\nLine 2 is code\nLine 3";
    let entities = vec![TextEntity {
        offset: 7,
        length: 14, // covers "Line 2 is code"
        entity_type: "code".to_string(),
    }];

    let lines = format_text_entities(text, &entities);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].spans[0].content, "Line 1");
    assert_eq!(lines[1].spans[0].content, "Line 2 is code");
    assert_eq!(lines[1].spans[0].style.fg, Some(Color::Yellow));
    assert_eq!(lines[2].spans[0].content, "Line 3");
}

#[test]
fn test_rich_text_edge_cases_no_panics() {
    // Empty text
    let empty_lines = format_text_entities("", &[]);
    assert_eq!(empty_lines.len(), 1);

    // Negative offset or length
    let bad_entities = vec![
        TextEntity {
            offset: -5,
            length: 10,
            entity_type: "bold".to_string(),
        },
        TextEntity {
            offset: 0,
            length: 0,
            entity_type: "italic".to_string(),
        },
        TextEntity {
            offset: 100, // way past length
            length: 5,
            entity_type: "code".to_string(),
        },
    ];
    let safe_lines = format_text_entities("Just plain text", &bad_entities);
    assert_eq!(safe_lines.len(), 1);
    assert_eq!(safe_lines[0].spans[0].content, "Just plain text");
}

// =========================================================================
// 3. In-Terminal QR Code Generator & Renderer Tests (Feature 10)
// =========================================================================

#[test]
fn test_qr_generator_halfblock_valid_matrix() {
    let link = "tg://login?token=AQAAAI_MOCK_TEST_TOKEN_123456789";
    let res = generate_qr_halfblock_lines(link);
    assert!(res.is_ok(), "QR generation should succeed for valid link");

    let lines = res.unwrap();
    assert!(!lines.is_empty(), "QR lines should not be empty");

    let first_line_width = lines[0].spans.len();
    assert!(first_line_width > 20, "QR code matrix width should be substantial");

    // All lines must have the exact same number of horizontal spans (square matrix)
    for line in &lines {
        assert_eq!(line.spans.len(), first_line_width, "All QR lines must have uniform width");
    }

    // Check that half-block characters are used
    let mut saw_halfblock = false;
    for line in &lines {
        for span in &line.spans {
            if span.content == "▀" || span.content == "▄" || span.content == "█" {
                saw_halfblock = true;
                break;
            }
        }
        if saw_halfblock {
            break;
        }
    }
    assert!(saw_halfblock, "QR rendering should utilize Unicode half-blocks");
}

#[test]
fn test_qr_generator_aspect_ratio_packing() {
    let link = "tg://login?token=DEMO";
    let lines = generate_qr_halfblock_lines(link).unwrap();

    let width = lines[0].spans.len();
    let height = lines.len();

    // Because each terminal character cell packs 2 vertical modules:
    // height should be roughly half of width (width / 2 +/- 2)
    let ratio = (width as f64) / (height as f64);
    assert!(
        ratio >= 1.5 && ratio <= 2.5,
        "Aspect ratio packing ratio {} should be approximately 2.0 (width: {}, height: {})",
        ratio,
        width,
        height
    );
}

// =========================================================================
// 4. Adversarial UTF-16 Code Unit & Emoji Edge Cases (Milestone 2 Challenge)
// =========================================================================

#[test]
fn test_adversarial_utf16_surrogate_split_offsets() {
    // "A🦀B🚀C"
    // 'A' -> 1 byte, 1 code unit (byte 0)
    // '🦀' -> 4 bytes, 2 code units (byte 1..5)
    // 'B' -> 1 byte, 1 code unit (byte 5)
    // '🚀' -> 4 bytes, 2 code units (byte 6..10)
    // 'C' -> 1 byte, 1 code unit (byte 10)
    // Total len: 11 bytes, 7 UTF-16 code units
    let text = "A🦀B🚀C";
    assert_eq!(text.len(), 11);

    // Verify all possible offsets return valid UTF-8 char boundaries
    for u16_offset in 0..=20 {
        let byte_idx = utf16_offset_to_byte_index(text, u16_offset);
        assert!(
            text.is_char_boundary(byte_idx),
            "Byte index {} at u16 offset {} must be on a char boundary",
            byte_idx,
            u16_offset
        );
        // Slicing must never panic
        let _ = &text[..byte_idx];
        let _ = &text[byte_idx..];
    }

    // Mid-surrogate offset: offset 1 is start of 🦀 (byte 1), offset 2 is inside 🦀 surrogate pair!
    // Since '🦀' takes 2 code units (offsets 1 and 2), target_utf16 = 2 lands in the middle of 🦀.
    // utf16_offset_to_byte_index should safely advance to byte 5 (the next char 'B').
    assert_eq!(utf16_offset_to_byte_index(text, 1), 1);
    assert_eq!(utf16_offset_to_byte_index(text, 2), 5); // next valid char boundary!
    assert_eq!(utf16_offset_to_byte_index(text, 3), 5); // start of 'B'
    assert_eq!(utf16_offset_to_byte_index(text, 4), 6); // start of '🚀'
    assert_eq!(utf16_offset_to_byte_index(text, 5), 10); // mid-surrogate of '🚀', advances to 'C'
    assert_eq!(utf16_offset_to_byte_index(text, 6), 10); // start of 'C'
    assert_eq!(utf16_offset_to_byte_index(text, 7), 11); // end of string
    assert_eq!(utf16_offset_to_byte_index(text, 100), 11); // beyond end

    // Test byte_index_to_utf16_offset
    assert_eq!(byte_index_to_utf16_offset(text, 0), 0);
    assert_eq!(byte_index_to_utf16_offset(text, 1), 1);
    assert_eq!(byte_index_to_utf16_offset(text, 5), 3);
    assert_eq!(byte_index_to_utf16_offset(text, 6), 4);
    assert_eq!(byte_index_to_utf16_offset(text, 10), 6);
    assert_eq!(byte_index_to_utf16_offset(text, 11), 7);
    assert_eq!(byte_index_to_utf16_offset(text, 100), 7);
}

#[test]
fn test_adversarial_utf16_complex_emoji_sequences() {
    // 1. ZWJ Family: 👨 (2) + ZWJ (1) + 👩 (2) + ZWJ (1) + 👧 (2) + ZWJ (1) + 👦 (2) = 11 u16 units, 25 UTF-8 bytes
    let family = "👨‍👩‍👧‍👦";
    assert_eq!(family.len(), 25);
    for u16_offset in 0..=15 {
        let b = utf16_offset_to_byte_index(family, u16_offset);
        assert!(family.is_char_boundary(b));
    }

    // 2. Flags: 🇺 (2) + 🇸 (2) = 4 u16 units, 8 UTF-8 bytes
    let flag = "🇺🇸";
    assert_eq!(flag.len(), 8);
    for u16_offset in 0..=6 {
        let b = utf16_offset_to_byte_index(flag, u16_offset);
        assert!(flag.is_char_boundary(b));
    }

    // 3. Skin Tone Modifier: 👍 (2) + 🏿 (2) = 4 u16 units, 8 UTF-8 bytes
    let skin_tone = "👍🏿";
    assert_eq!(skin_tone.len(), 8);
    for u16_offset in 0..=6 {
        let b = utf16_offset_to_byte_index(skin_tone, u16_offset);
        assert!(skin_tone.is_char_boundary(b));
    }

    // 4. Mixed scripts: CJK + Arabic (RTL) + Cyrillic + Math astral symbols
    let mixed = "Привет! 你好 🌍 𝕏 مرحبا";
    for u16_offset in 0..=(mixed.encode_utf16().count() + 5) {
        let b = utf16_offset_to_byte_index(mixed, u16_offset);
        assert!(mixed.is_char_boundary(b));
    }
}

#[test]
fn test_adversarial_utf16_extreme_and_overflow_values() {
    let text = "Extreme boundaries test 🦀";
    let len = text.len();

    // Max values
    assert_eq!(utf16_offset_to_byte_index(text, usize::MAX), len);
    assert_eq!(utf16_offset_to_byte_index(text, usize::MAX / 2), len);
    assert_eq!(utf16_offset_to_byte_index(text, i32::MAX as usize), len);

    assert_eq!(byte_index_to_utf16_offset(text, usize::MAX), text.encode_utf16().count());

    // Empty text
    assert_eq!(utf16_offset_to_byte_index("", 0), 0);
    assert_eq!(utf16_offset_to_byte_index("", 10), 0);
    assert_eq!(utf16_offset_to_byte_index("", usize::MAX), 0);
    assert_eq!(byte_index_to_utf16_offset("", 0), 0);
    assert_eq!(byte_index_to_utf16_offset("", 10), 0);
}

#[test]
fn test_adversarial_rich_text_pathological_offsets() {
    let text = "Telegram Client 🚀";

    // Pathological entities: negative offsets, negative lengths, zero lengths, enormous offsets
    let entities = vec![
        TextEntity { offset: -1, length: 5, entity_type: "bold".to_string() },
        TextEntity { offset: -99999, length: -99999, entity_type: "italic".to_string() },
        TextEntity { offset: 0, length: 0, entity_type: "underline".to_string() },
        TextEntity { offset: 5, length: -10, entity_type: "code".to_string() },
        TextEntity { offset: 1000, length: 50, entity_type: "bold".to_string() },
        TextEntity { offset: i32::MAX, length: 1, entity_type: "italic".to_string() },
        TextEntity { offset: 0, length: i32::MAX, entity_type: "spoiler".to_string() }, // spans entire text and clamps
        TextEntity { offset: 9, length: 6, entity_type: "bold".to_string() }, // "Client"
    ];

    let lines = format_text_entities(text, &entities);
    assert_eq!(lines.len(), 1);

    // Whole text is covered by spoiler, and "Client" is bold + spoiler
    let full_content: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(full_content, text);

    // Verify spans that overlap "Client" contain bold
    let mut saw_bold = false;
    for span in &lines[0].spans {
        if span.content.contains("Client") {
            assert!(span.style.add_modifier.contains(Modifier::BOLD));
            saw_bold = true;
        }
        // All spans should have spoiler bg/fg
        assert_eq!(span.style.fg, Some(Color::DarkGray));
        assert_eq!(span.style.bg, Some(Color::DarkGray));
    }
    assert!(saw_bold);
}

#[test]
fn test_adversarial_rich_text_multi_level_overlapping_entities() {
    // Text: "The quick brown fox jumps over the lazy dog"
    // Entity 1: [0..15] Bold ("The quick brown")
    // Entity 2: [10..25] Italic ("brown fox jumps ov")
    // Entity 3: [20..35] Underline ("mps over the laz")
    // Entity 4: [12..22] Code ("own fox ju")
    // Entity 5: [0..43] TextUrl (all)
    let text = "The quick brown fox jumps over the lazy dog";
    let entities = vec![
        TextEntity { offset: 0, length: 15, entity_type: "bold".to_string() },
        TextEntity { offset: 10, length: 15, entity_type: "italic".to_string() },
        TextEntity { offset: 20, length: 15, entity_type: "underline".to_string() },
        TextEntity { offset: 12, length: 10, entity_type: "code".to_string() },
        TextEntity { offset: 0, length: 43, entity_type: "text_url".to_string() },
    ];

    let lines = format_text_entities(text, &entities);
    assert_eq!(lines.len(), 1);

    // Verify text integrity: concatenated spans must exactly equal input text
    let concatenated: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(concatenated, text);

    // Verify disjoint intervals are sliced properly:
    // Span at [12..15] ("own") must have: Bold + Italic + Code + TextUrl
    for span in &lines[0].spans {
        if span.content == "own" {
            assert!(span.style.add_modifier.contains(Modifier::BOLD));
            assert!(span.style.add_modifier.contains(Modifier::ITALIC));
            assert!(span.style.add_modifier.contains(Modifier::UNDERLINED)); // from text_url
        }
    }
}

#[test]
fn test_adversarial_rich_text_emojis_surrogate_entity_boundaries() {
    // Text: "Hi 🦀 Rust 🚀 World"
    // 🦀 is at UTF-16 offset 3..5
    // 🚀 is at UTF-16 offset 11..13
    let text = "Hi 🦀 Rust 🚀 World";

    // Entity that starts right on first surrogate half of 🦀 (offset 3) and ends on second half (offset 4)
    // Entity that starts inside 🚀 (offset 12) and ends after (offset 15)
    let entities = vec![
        TextEntity { offset: 3, length: 1, entity_type: "bold".to_string() }, // lands inside 🦀
        TextEntity { offset: 12, length: 3, entity_type: "code".to_string() }, // starts inside 🚀
    ];

    let lines = format_text_entities(text, &entities);
    assert_eq!(lines.len(), 1);

    let concatenated: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(concatenated, text, "Text content must remain completely intact without panicking");
}

#[test]
fn test_adversarial_rich_text_consecutive_and_trailing_newlines() {
    let text = "\n\nHello\n\nWorld\n\n";
    let entities = vec![
        TextEntity { offset: 2, length: 5, entity_type: "bold".to_string() }, // "Hello"
        TextEntity { offset: 9, length: 5, entity_type: "italic".to_string() }, // "World"
    ];

    let lines = format_text_entities(text, &entities);
    // Split by '\n' should yield 7 lines: ["", "", "Hello", "", "World", "", ""]
    assert_eq!(lines.len(), 7, "Newline count must accurately preserve empty lines");

    // Line 2 is "Hello"
    assert_eq!(lines[2].spans[0].content, "Hello");
    assert!(lines[2].spans[0].style.add_modifier.contains(Modifier::BOLD));

    // Line 4 is "World"
    assert_eq!(lines[4].spans[0].content, "World");
    assert!(lines[4].spans[0].style.add_modifier.contains(Modifier::ITALIC));
}

#[test]
fn test_adversarial_rich_text_duplicate_and_unknown_entities() {
    let text = "Robustness check";
    let entities = vec![
        TextEntity { offset: 0, length: 10, entity_type: "bold".to_string() },
        TextEntity { offset: 0, length: 10, entity_type: "bold".to_string() }, // duplicate
        TextEntity { offset: 0, length: 10, entity_type: "UNKNOWN_ENTITY_XXX".to_string() }, // unrecognized
        TextEntity { offset: 0, length: 10, entity_type: "   ".to_string() }, // whitespace
        TextEntity { offset: 0, length: 10, entity_type: "textEntityTypeBold".to_string() }, // alternate format
    ];

    let lines = format_text_entities(text, &entities);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].content, "Robustness");
    assert!(lines[0].spans[0].style.add_modifier.contains(Modifier::BOLD));
    assert_eq!(lines[0].spans[1].content, " check");
}

#[test]
fn test_adversarial_rich_text_randomized_stress_fuzz() {
    // Deterministic pseudo-random generation to stress test 500 combinations
    let char_pool = [
        'a', 'B', '3', ' ', '\n', '🦀', '🚀', '中', '文', 'д', 'ع', '👨', '\u{200D}', '👩', '!', '?',
    ];

    let entity_types = [
        "bold", "italic", "underline", "strikethrough", "code", "pre", "text_url", "mention", "spoiler",
        "invalid_type", "",
    ];

    let mut seed: u64 = 0xDEADBEEFCAFE;
    let mut next_rand = || -> u64 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        seed
    };

    for _ in 0..500 {
        // Generate random string
        let str_len = (next_rand() % 40) as usize;
        let mut text = String::new();
        for _ in 0..str_len {
            let ch_idx = (next_rand() % (char_pool.len() as u64)) as usize;
            text.push(char_pool[ch_idx]);
        }

        // Generate 0 to 15 random entities
        let entity_count = (next_rand() % 15) as usize;
        let mut entities = Vec::new();
        for _ in 0..entity_count {
            let offset = ((next_rand() % 60) as i32) - 10; // Can be negative or out of bounds
            let length = ((next_rand() % 40) as i32) - 5;  // Can be negative or zero
            let type_idx = (next_rand() % (entity_types.len() as u64)) as usize;
            entities.push(TextEntity {
                offset,
                length,
                entity_type: entity_types[type_idx].to_string(),
            });
        }

        // Run format_text_entities: MUST NEVER PANIC
        let lines = format_text_entities(&text, &entities);
        assert!(!lines.is_empty(), "Result lines should never be empty");

        // Verify that reconstructing the text from lines matches original text
        let mut reconstructed = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                reconstructed.push('\n');
            }
            for span in &line.spans {
                reconstructed.push_str(&span.content);
            }
        }
        assert_eq!(
            reconstructed, text,
            "Reconstructed text must perfectly match input text during randomized fuzzing"
        );
    }
}

// =========================================================================
// 5. Adversarial QR Code Generator & Renderer Stress Suite (Milestone 2 Challenge)
// =========================================================================

#[test]
fn test_adversarial_qr_empty_payload() {
    let res = generate_qr_halfblock_lines("");
    match res {
        Ok(lines) => {
            assert!(!lines.is_empty());
            assert_eq!(lines[0].spans.len(), 21 + 4);
        }
        Err(err) => {
            assert!(!err.is_empty());
        }
    }
}

#[test]
fn test_adversarial_qr_minimal_payloads() {
    let payloads = ["1", "a", "!", " ", "\n", "\t"];
    for p in payloads {
        let res = generate_qr_halfblock_lines(p);
        assert!(res.is_ok(), "Minimal payload {:?} must produce valid QR lines", p);
        let lines = res.unwrap();
        assert!(!lines.is_empty());
        let expected_width = lines[0].spans.len();
        for line in &lines {
            assert_eq!(line.spans.len(), expected_width);
            for span in &line.spans {
                assert!(
                    span.content == "▀" || span.content == "▄" || span.content == "█" || span.content == " ",
                    "Span content must only be half-blocks or space, got {:?}",
                    span.content
                );
            }
        }
    }
}

#[test]
fn test_adversarial_qr_varying_input_sizes() {
    let sizes = [10, 32, 64, 128, 256, 512, 1024, 1500, 2000];

    for &size in &sizes {
        let payload: String = (0..size).map(|i| (b'A' + (i % 26) as u8) as char).collect();
        let res = generate_qr_halfblock_lines(&payload);
        assert!(res.is_ok(), "Payload of size {} should succeed", size);

        let lines = res.unwrap();
        assert!(!lines.is_empty());
        let width = lines[0].spans.len();
        let height = lines.len();

        for line in &lines {
            assert_eq!(line.spans.len(), width);
        }

        let expected_height = (width + 1) / 2;
        assert_eq!(
            height, expected_height,
            "Height {} should equal ceil(width / 2) = {} for width {}",
            height, expected_height, width
        );
    }
}

#[test]
fn test_adversarial_qr_oversized_payload_graceful_error() {
    let huge_payload = "X".repeat(5000);
    let res = generate_qr_halfblock_lines(&huge_payload);
    assert!(res.is_err(), "Payload of 5000 bytes must exceed QR capacity");
    let err_msg = res.unwrap_err();
    assert!(err_msg.contains("Failed to generate QR"));

    let colossal_payload = "A".repeat(20000);
    let res2 = generate_qr_halfblock_lines(&colossal_payload);
    assert!(res2.is_err(), "Payload of 20000 bytes must exceed QR capacity");
}

#[test]
fn test_adversarial_qr_non_ascii_and_special_payloads() {
    let complex_payloads = [
        "https://t.me/joinchat/AAAAAFM_MOCK_TEST_12345",
        "tg://login?token=AQAAAI_MOCK_TEST_TOKEN&lang=ru&ref=123",
        "Привет мир! 🌍 🦀 日本語 שלום",
        "data:text/plain;charset=utf-8;base64,SGVsbG8gV29ybGQh",
        "payload\x00with\x00null\x00bytes",
        "https://example.com/path?param=value&foo=bar#section",
    ];

    for payload in complex_payloads {
        let res = generate_qr_halfblock_lines(payload);
        assert!(res.is_ok(), "Complex payload {:?} must succeed", payload);
        let lines = res.unwrap();
        assert!(!lines.is_empty());
    }
}

#[test]
fn test_adversarial_qr_widget_rendering_boundary_areas() {
    let link = "tg://login?token=MOCK_AUTH_TOKEN_TEST";
    let oversized = "Oversized".repeat(600);

    // Test across various terminal viewport dimensions (from minimal/degenerate to large)
    let dimensions = [
        (0, 0),       // Zero area
        (1, 1),       // 1x1 minimal area
        (10, 5),      // Sub-QR area
        (35, 15),     // Narrow area
        (80, 35),     // Standard terminal
        (200, 100),   // Large viewport
    ];

    for &(w, h) in &dimensions {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        // Normal QR rendering
        terminal.draw(|f| {
            render_qr_widget(f, link, f.area());
        }).expect("Draw must succeed without panic on valid link");

        // Error path QR rendering
        terminal.draw(|f| {
            render_qr_widget(f, &oversized, f.area());
        }).expect("Draw must succeed without panic on oversized error link");

        // Empty string link rendering
        terminal.draw(|f| {
            render_qr_widget(f, "", f.area());
        }).expect("Draw must succeed without panic on empty link");
    }
}

