//! Ordinary Short Text metadata, without source or write qualification.
use crate::{
    NativeColumn,
    format::{
        data_type::{MAX_SHORT_TEXT_UNITS, SHORT_TEXT},
        layout::{UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG},
        text::UTF16_UNIT_BYTES,
    },
};

impl NativeColumn {
    /// Whether this declaration uses the supported ordinary Short Text storage
    /// profile. This metadata-only predicate does not qualify values, indexes,
    /// dependencies, preserved companions, constraints or write permission.
    pub fn is_ordinary_short_text(&self) -> bool {
        self.native_type() == SHORT_TEXT
            && !self.fixed()
            && self.flags() == UPDATABLE_FLAG
            && self.extended_flags() & !UNICODE_COMPRESSION_FLAG == 0
            && self.storage_length().is_multiple_of(UTF16_UNIT_BYTES)
            && self.storage_length() / UTF16_UNIT_BYTES <= MAX_SHORT_TEXT_UNITS
    }
}
