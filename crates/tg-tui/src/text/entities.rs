use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use tg_core::TextEntity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedEntityType {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Code,
    Pre,
    TextUrl,
    Mention,
    Spoiler,
}

/// Convert a UTF-16 code unit offset into a UTF-8 byte offset in `text`.
/// If `target_utf16` exceeds the length in code units, returns `text.len()`.
pub fn utf16_offset_to_byte_index(text: &str, target_utf16: usize) -> usize {
    let mut current_utf16 = 0;
    for (byte_idx, ch) in text.char_indices() {
        if current_utf16 >= target_utf16 {
            return byte_idx;
        }
        current_utf16 += ch.len_utf16();
    }
    text.len()
}

/// Convert a UTF-8 byte offset into a UTF-16 code unit offset in `text`.
pub fn byte_index_to_utf16_offset(text: &str, target_byte: usize) -> usize {
    let mut current_utf16 = 0;
    for (byte_idx, ch) in text.char_indices() {
        if byte_idx >= target_byte {
            return current_utf16;
        }
        current_utf16 += ch.len_utf16();
    }
    current_utf16
}

/// Parse a raw TDLib or shorthand entity type string into a normalized `ParsedEntityType`.
pub fn parse_entity_type(raw_type: &str) -> Option<ParsedEntityType> {
    let normalized = raw_type
        .trim()
        .to_lowercase()
        .replace('_', "");
    let s = normalized
        .strip_prefix("textentitytype")
        .unwrap_or(&normalized);

    match s {
        "bold" => Some(ParsedEntityType::Bold),
        "italic" => Some(ParsedEntityType::Italic),
        "underline" => Some(ParsedEntityType::Underline),
        "strikethrough" => Some(ParsedEntityType::Strikethrough),
        "code" => Some(ParsedEntityType::Code),
        "pre" | "precode" => Some(ParsedEntityType::Pre),
        "texturl" | "url" => Some(ParsedEntityType::TextUrl),
        "mention" | "mentionname" => Some(ParsedEntityType::Mention),
        "spoiler" => Some(ParsedEntityType::Spoiler),
        _ => None,
    }
}

/// Map a slice of parsed entities to a merged Ratatui `Style`.
pub fn style_for_entities(types: &[ParsedEntityType]) -> Style {
    let mut style = Style::default();
    for &t in types {
        match t {
            ParsedEntityType::Bold => {
                style = style.add_modifier(Modifier::BOLD);
            }
            ParsedEntityType::Italic => {
                style = style.add_modifier(Modifier::ITALIC);
            }
            ParsedEntityType::Underline => {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            ParsedEntityType::Strikethrough => {
                style = style.add_modifier(Modifier::CROSSED_OUT);
            }
            ParsedEntityType::Code => {
                style = style
                    .fg(Color::Yellow)
                    .bg(Color::Rgb(40, 44, 52));
            }
            ParsedEntityType::Pre => {
                style = style
                    .fg(Color::LightGreen)
                    .bg(Color::Rgb(30, 30, 30));
            }
            ParsedEntityType::TextUrl => {
                style = style
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::UNDERLINED);
            }
            ParsedEntityType::Mention => {
                style = style.fg(Color::LightCyan);
            }
            ParsedEntityType::Spoiler => {
                style = style
                    .fg(Color::DarkGray)
                    .bg(Color::DarkGray);
            }
        }
    }
    style
}

/// Format `text` with Telegram `TextEntity`s into a list of Ratatui `Line`s,
/// ensuring safety across multi-byte UTF-8 sequences and UTF-16 code unit offsets.
pub fn format_text_entities(text: &str, entities: &[TextEntity]) -> Vec<Line<'static>> {
    if text.is_empty() {
        return vec![Line::default()];
    }

    if entities.is_empty() {
        return text
            .split('\n')
            .map(|l| Line::from(l.to_string()))
            .collect();
    }

    // Convert entities into byte ranges with parsed types
    let mut resolved_entities: Vec<(usize, usize, ParsedEntityType)> = Vec::new();
    let mut boundaries = vec![0, text.len()];

    for entity in entities {
        if entity.offset < 0 || entity.length <= 0 {
            continue;
        }
        let parsed = match parse_entity_type(&entity.entity_type) {
            Some(p) => p,
            None => continue,
        };

        let start_u16 = entity.offset as usize;
        let end_u16 = start_u16 + entity.length as usize;

        let start_byte = utf16_offset_to_byte_index(text, start_u16);
        let end_byte = utf16_offset_to_byte_index(text, end_u16).min(text.len());

        if start_byte < end_byte {
            boundaries.push(start_byte);
            boundaries.push(end_byte);
            resolved_entities.push((start_byte, end_byte, parsed));
        }
    }

    boundaries.sort_unstable();
    boundaries.dedup();

    // Build slices between adjacent boundaries
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();

    for i in 0..boundaries.len().saturating_sub(1) {
        let start = boundaries[i];
        let end = boundaries[i + 1];
        if start >= end || start >= text.len() {
            continue;
        }

        let slice = &text[start..end];
        let active_types: Vec<ParsedEntityType> = resolved_entities
            .iter()
            .filter(|(s, e, _)| *s <= start && *e >= end)
            .map(|(_, _, t)| *t)
            .collect();

        let style = style_for_entities(&active_types);

        // A slice might contain newlines; split by '\n' to build lines
        let parts: Vec<&str> = slice.split('\n').collect();
        for (idx, part) in parts.iter().enumerate() {
            if idx > 0 {
                lines.push(Line::from(std::mem::take(&mut current_spans)));
            }
            if !part.is_empty() {
                current_spans.push(Span::styled((*part).to_string(), style));
            }
        }
    }

    lines.push(Line::from(current_spans));
    lines
}
