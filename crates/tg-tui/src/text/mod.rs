pub mod entities;

#[allow(unused_imports)]
pub use entities::{
    byte_index_to_utf16_offset, format_text_entities, parse_entity_type, style_for_entities,
    utf16_offset_to_byte_index, ParsedEntityType,
};
