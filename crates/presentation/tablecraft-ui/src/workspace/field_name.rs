//! TextEdit cannot insert an invalid UTF-8/UTF-16 name, including undo or IME.
use egui::{TextBuffer, text::CharIndex};
use std::{any::TypeId, ops::Range};
use tablecraft_builders::table::{MAX_DRAFT_NAME_BYTES, MAX_DRAFT_NAME_UNITS};

pub(in crate::workspace) struct NameBuffer {
    text: String,
    before_delete: String,
    deletion_pending: bool,
}

impl NameBuffer {
    pub(in crate::workspace) fn new(original: &str) -> Option<Self> {
        if !bounded_name(original) {
            return None;
        }
        let mut text = String::new();
        text.try_reserve_exact(MAX_DRAFT_NAME_BYTES).ok()?;
        text.push_str(original);
        let mut before_delete = String::new();
        before_delete.try_reserve_exact(MAX_DRAFT_NAME_BYTES).ok()?;
        Some(Self {
            text,
            before_delete,
            deletion_pending: false,
        })
    }

    pub(in crate::workspace) fn into_string(self) -> String {
        self.text
    }

    fn byte(&self, index: CharIndex) -> usize {
        self.text
            .char_indices()
            .nth(usize::from(index))
            .map_or(self.text.len(), |(byte, _)| byte)
    }
}

impl TextBuffer for NameBuffer {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn is_mutable(&self) -> bool {
        true
    }

    fn as_str(&self) -> &str {
        &self.text
    }

    fn insert_text(&mut self, inserted: &str, index: CharIndex) -> usize {
        let bytes = self.text.len().checked_add(inserted.len());
        let valid = bytes.is_some_and(|bytes| bytes <= MAX_DRAFT_NAME_BYTES)
            && self.text.encode_utf16().count() + inserted.encode_utf16().count()
                <= tablecraft_builders::table::MAX_DRAFT_NAME_UNITS;
        if !valid {
            if self.deletion_pending {
                self.text.clear();
                self.text.push_str(&self.before_delete);
                self.deletion_pending = false;
            }
            return 0;
        }
        let byte = self.byte(index);
        self.text.insert_str(byte, inserted);
        self.deletion_pending = false;
        inserted.chars().count()
    }

    fn delete_char_range(&mut self, range: Range<CharIndex>) {
        self.before_delete.clear();
        self.before_delete.push_str(&self.text);
        self.deletion_pending = true;
        let start = self.byte(range.start);
        let end = self.byte(range.end).max(start);
        self.text.replace_range(start..end, "");
    }

    fn replace_with(&mut self, replacement: &str) {
        if bounded_name(replacement) {
            self.text.clear();
            self.text.push_str(replacement);
            self.deletion_pending = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_units_refuse_whole_insertion_after_selection_without_partial_deletion() {
        let mut buffer = NameBuffer::new("Score").unwrap();
        buffer.delete_char_range(CharIndex(0)..CharIndex(5));
        assert_eq!(buffer.insert_text(&"😀".repeat(33), CharIndex(0)), 0);
        assert_eq!(buffer.as_str(), "Score");
        buffer.delete_char_range(CharIndex(0)..CharIndex(5));
        assert_eq!(buffer.insert_text(&"😀".repeat(32), CharIndex(0)), 32);
        assert_eq!(buffer.as_str(), "😀".repeat(32));
        let capacity = buffer.text.capacity();
        assert_eq!(buffer.insert_text(&"x".repeat(1_000_000), CharIndex(32)), 0);
        assert_eq!(buffer.text.capacity(), capacity);
        assert_eq!(buffer.as_str(), "😀".repeat(32));
    }

    #[test]
    fn undo_replacement_and_ascii_capacity_preserve_the_same_name_limits() {
        let mut buffer = NameBuffer::new("Score").unwrap();
        buffer.replace_with(&"a".repeat(64));
        assert_eq!(buffer.as_str(), "a".repeat(64));
        buffer.replace_with(&"😀".repeat(33));
        assert_eq!(buffer.as_str(), "a".repeat(64));
        buffer.replace_with("Score");
        assert_eq!(buffer.as_str(), "Score");
    }
}

pub(in crate::workspace) fn bounded_name(name: &str) -> bool {
    name.len() <= MAX_DRAFT_NAME_BYTES
        && name.encode_utf16().take(MAX_DRAFT_NAME_UNITS + 1).count() <= MAX_DRAFT_NAME_UNITS
}
