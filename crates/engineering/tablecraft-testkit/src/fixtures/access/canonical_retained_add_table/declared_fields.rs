//! Literal unadorned Currency descriptor insertion; no property snapshot is authored.
use crate::fixtures::access::{put16, put32, raw_catalog_properties};

const PAGE: usize = 4096;

/// Independent populated keyless target with Score at native ordinal one.
pub struct Source {
    /// Complete original retained image.
    pub bytes: Vec<u8>,
    /// Actual target definition page.
    pub definition: u32,
    /// Actual target catalog row slot.
    pub catalog_root: usize,
}

/// Build Items with one ordinary Score field and retained two-field high water.
pub fn source() -> Source {
    let seed = super::fields::source(super::fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / PAGE) as u32;
    let catalog_root = word(&seed, 4 * PAGE + 12);
    let full = super::field_changes::keyless_first_field(&seed, "Items");
    let bytes = super::field_changes::drop_field(&full, definition, catalog_root, "Id");
    Source {
        bytes,
        definition,
        catalog_root,
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

/// Append one ordinary Currency declaration while preserving all catalog/property bytes.
pub fn after(before: &[u8], definition: u32, name: &str) -> Vec<u8> {
    assert!(name.is_ascii() && !name.is_empty() && name.len() <= 64);
    let base = definition as usize * PAGE;
    let old = &before[base..base + PAGE];
    assert_eq!(&old[47..55], &[0; 8]);
    let count = word(old, 45);
    let high = word(old, 41);
    let names = 63 + count * 25;
    let mut tail = names;
    for _ in 0..count {
        tail += 2 + word(old, tail);
    }
    let length = u32::from_le_bytes(old[8..12].try_into().unwrap()) as usize;
    let mut fixed = 0;
    let mut variable = 0;
    let mut creation = 0;
    for start in (63..names).step_by(25) {
        creation = creation.max(word(old, start + 9) + 1);
        if old[start + 15] & 1 != 0 {
            if old[start] != 1 {
                fixed = fixed.max(word(old, start + 21) + word(old, start + 23));
            }
        } else {
            variable = variable.max(word(old, start + 7) + 1);
        }
    }
    let mut descriptor = [0; 25];
    descriptor[0] = 5;
    put32(&mut descriptor, 1, 1625);
    put16(&mut descriptor, 5, high as u16);
    put16(&mut descriptor, 7, variable as u16);
    put16(&mut descriptor, 9, creation as u16);
    descriptor[15] = 3;
    put16(&mut descriptor, 21, fixed as u16);
    put16(&mut descriptor, 23, 8);
    let mut page = old[..names].to_vec();
    page.extend(descriptor);
    page.extend_from_slice(&old[names..tail]);
    page.extend(((name.len() * 2) as u16).to_le_bytes());
    page.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
    page.extend_from_slice(&old[tail..length]);
    let new_length = page.len();
    assert!(new_length <= PAGE - 8);
    put16(&mut page, 2, (PAGE - 8 - new_length) as u16);
    put32(&mut page, 8, new_length as u32);
    put16(&mut page, 41, (high + 1) as u16);
    put16(&mut page, 45, (count + 1) as u16);
    page.extend_from_slice(&old[new_length..]);
    let mut after = before.to_vec();
    after[base..base + PAGE].copy_from_slice(&page);
    after
}

/// Add an unused dictionary token; it is not a column property group.
pub fn with_unused_dictionary_name(before: &[u8], catalog_root: usize, name: &str) -> Vec<u8> {
    let original = raw_catalog_properties::body(before, catalog_root);
    assert_eq!(&original[..4], b"MR2\0");
    assert_eq!(&original[8..10], &[128, 0]);
    let end = 4 + u32::from_le_bytes(original[4..8].try_into().unwrap()) as usize;
    let mut body = original[..end].to_vec();
    let wide = name
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    body.extend((wide.len() as u16).to_le_bytes());
    body.extend(wide);
    let dictionary = body.len() - 4;
    put32(&mut body, 4, dictionary as u32);
    body.extend_from_slice(&original[end..]);
    with_body(before, catalog_root, &body)
}

/// Add an actual inactive Required(false) column group, without a default entry.
pub fn with_retired_required(before: &[u8], catalog_root: usize, name: &str) -> Vec<u8> {
    let mut body = raw_catalog_properties::body(before, catalog_root);
    assert_eq!(&body[..4], b"MR2\0");
    let end = 4 + u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
    assert_eq!(
        end,
        body.len(),
        "this recipe starts with no live property groups"
    );
    assert_eq!(
        &body[10..28],
        &[
            16, 0, b'R', 0, b'e', 0, b'q', 0, b'u', 0, b'i', 0, b'r', 0, b'e', 0, b'd', 0
        ]
    );
    let wide = name
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let length = 12 + wide.len() + 9;
    body.extend((length as u32).to_le_bytes());
    body.extend(1_u16.to_le_bytes());
    body.extend(((6 + wide.len()) as u32).to_le_bytes());
    body.extend((wide.len() as u16).to_le_bytes());
    body.extend(wide);
    body.extend([9, 0, 1, 1, 0, 0, 1, 0, 0]);
    with_body(before, catalog_root, &body)
}

fn with_body(before: &[u8], catalog_root: usize, body: &[u8]) -> Vec<u8> {
    let candidate = raw_catalog_properties::candidate(before, body.len());
    // NO_CLONE: The independent mutated SOURCE must coexist with the original
    // input; this raw fixture owns its complete replacement separately.
    raw_catalog_properties::append(before, before.to_vec(), catalog_root, body, candidate)
}
