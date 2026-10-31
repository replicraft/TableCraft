//! Borrowed plans for text carriers decoded with native compression markers.
use super::CreateDatabaseError;
use crate::header::check;
use tablecraft_core::limits::ResourceBudget;

const MARKER: [u8; 2] = [0xff, 0xfe];
const WIDE_SWITCH: u8 = 0;
const UNIT_BYTES: usize = size_of::<u16>();
const ESCAPE_BYTES: usize = MARKER.len() + size_of::<u8>();

#[derive(Clone, Copy)]
pub(super) struct Plan<'a> {
    text: &'a str,
    escaped: bool,
    bytes: usize,
}

impl<'a> Plan<'a> {
    pub fn capture(
        text: &'a str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        let escaped = text.starts_with('\u{feff}');
        let mut units = 0_usize;
        for unit in text.encode_utf16() {
            if escaped {
                check(budget, cancelled)?;
                if unit.to_le_bytes()[0] == 0 {
                    return Err(CreateDatabaseError::TextEncoding);
                }
            }
            units += 1;
        }
        let bytes = units
            .checked_mul(UNIT_BYTES)
            .and_then(|bytes| bytes.checked_add(if escaped { ESCAPE_BYTES } else { 0 }))
            .ok_or(CreateDatabaseError::Layout)?;
        Ok(Self {
            text,
            escaped,
            bytes,
        })
    }

    /// Only the authored ASCII system names use this allocation-free constant plan.
    pub const fn ascii_literal(text: &'static str) -> Self {
        assert!(text.is_ascii());
        Self {
            text,
            escaped: false,
            bytes: text.len() * UNIT_BYTES,
        }
    }

    pub const fn byte_len(self) -> usize {
        self.bytes
    }

    pub fn write(
        self,
        destination: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CreateDatabaseError> {
        if destination.len() != self.bytes {
            return Err(CreateDatabaseError::Layout);
        }
        let mut position = 0;
        if self.escaped {
            destination[..MARKER.len()].copy_from_slice(&MARKER);
            destination[MARKER.len()] = WIDE_SWITCH;
            position = ESCAPE_BYTES;
        }
        for unit in self.text.encode_utf16() {
            check(budget, cancelled)?;
            destination[position..position + UNIT_BYTES].copy_from_slice(&unit.to_le_bytes());
            position += UNIT_BYTES;
        }
        if position != destination.len() {
            return Err(CreateDatabaseError::Layout);
        }
        Ok(())
    }
}
