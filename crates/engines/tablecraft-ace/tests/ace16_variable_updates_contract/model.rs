use super::{page_model, support};
use tablecraft_ace::{Ace16WritePreview, EditError, SlotScalarUpdate};
use tablecraft_core::values::Value;

pub const PAGE: usize = 4096;
pub const ROOT: usize = 12;
pub const ALTERNATE: usize = 13;
pub const EOF: usize = 14;
pub const SLOT: usize = 14;
pub const HIDDEN: u16 = 0x8000;
pub const FORWARDED: u16 = 0x4000;
pub const OFFSET: u16 = 0x1fff;

pub fn source(alternate: bool, highest_fits: bool) -> Vec<u8> {
    let rows = [(0, page_model::record(b"A")), (HIDDEN, vec![0xa6; 4000])];
    let mut bytes = page_model::image(&rows, 1, true);
    if alternate {
        let rows = if highest_fits {
            vec![(HIDDEN, vec![0xc7; 8])]
        } else {
            vec![(HIDDEN, vec![0xc7; 4060])]
        };
        bytes[ALTERNATE * PAGE..(ALTERNATE + 1) * PAGE].copy_from_slice(&page_model::page(&rows));
    }
    maps(
        &mut bytes,
        if alternate { 0x30 } else { 0x10 },
        if alternate { 0x30 } else { 0x10 },
    );
    bytes
}

pub fn maps(bytes: &mut [u8], owned: u8, hint: u8) {
    // Two represented bitmap bytes cover pages 0..15. EOF14 and15
    // therefore share the byte containing the existing data pages12/13.
    let owned = vec![0, 0, 0, 0, 0, 0, owned];
    let hint = vec![0, 0, 0, 0, 0, 0, hint];
    bytes[11 * PAGE..12 * PAGE].copy_from_slice(&support::data_page(0, &[(0, owned), (0, hint)]));
}

pub fn update(bytes: &[u8], value: &[u8]) -> Result<Ace16WritePreview, EditError> {
    super::capture(bytes).preview_ace16_variable_updates(
        "Items",
        "Score",
        &[SlotScalarUpdate::new(
            0,
            Value::Binary(b"A".to_vec()),
            Value::Binary(value.to_vec()),
        )],
        &mut super::budget(),
        &|| false,
    )
}

pub fn word(bytes: &[u8], page: usize, slot: usize) -> u16 {
    let start = page * PAGE + SLOT + 2 * slot;
    u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap())
}

pub fn pointer(bytes: &[u8]) -> u32 {
    let start = ROOT * PAGE + usize::from(word(bytes, ROOT, 0) & OFFSET);
    u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap())
}

pub fn map_byte(bytes: &[u8], page: usize, row: usize, byte: usize) -> u8 {
    let start = usize::from(word(bytes, page, row) & OFFSET);
    bytes[page * PAGE + start + 5 + byte]
}
