//! Independent Number ADD property ordering, native records and full SDK goldens.
pub mod mixed;
pub mod repeated;

use super::{canonical_created_add_column, canonical_created_default_zero, put16, put32};

const PAGE: usize = 4096;
const DIRECTORY: usize = 14;
const PROPERTY_VARIABLE: usize = 8;
const MASK_BYTES: usize = 3;

#[derive(Clone, Copy, Debug)]
pub enum Source {
    /// Initial Table1(ID Increment PrimaryKey), no field policies.
    IdOnly,
    /// Canonical creator Table1(ID, Score Required false DefaultValue0).
    GuiRequiredZero,
    /// Existing Items(Id, Score DefaultValue0), Required still absent.
    DefaultOnly,
}

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn wide(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn number_group(name: &str, required: bool, dictionary: &[&str]) -> Vec<u8> {
    let mut values = Vec::new();
    for (index, key) in dictionary.iter().enumerate() {
        if *key == "Required" && required {
            values.extend([9, 0, 1, 1]);
            values.extend((index as u16).to_le_bytes());
            values.extend([1, 0, 0]);
        } else if *key == "DefaultValue" {
            values.extend([10, 0, 1, 12]);
            values.extend((index as u16).to_le_bytes());
            values.extend([2, 0, b'0', 0]);
        }
    }
    let name = wide(name);
    let mut bytes = ((12 + name.len() + values.len()) as u32)
        .to_le_bytes()
        .to_vec();
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(((6 + name.len()) as u32).to_le_bytes());
    bytes.extend((name.len() as u16).to_le_bytes());
    bytes.extend(name);
    bytes.extend(values);
    bytes
}

fn body(fields: &[(&str, bool)], required_first: bool) -> Vec<u8> {
    let dictionary = if required_first {
        ["Required", "DefaultValue"]
    } else {
        ["DefaultValue", "Required"]
    };
    let mut names = Vec::new();
    for name in dictionary {
        let name = wide(name);
        names.extend((name.len() as u16).to_le_bytes());
        names.extend(name);
    }
    let mut bytes = b"MR2\0".to_vec();
    bytes.extend(((6 + names.len()) as u32).to_le_bytes());
    bytes.extend(128_u16.to_le_bytes());
    bytes.extend(names);
    for (name, required) in fields {
        bytes.extend(number_group(name, *required, &dictionary));
    }
    bytes
}

/// Literal canonical producer bytes. The GUI-policy source is checked separately
/// against both the engine constructor and the runtime draft-save producer.
pub fn source(case: Source) -> Vec<u8> {
    match case {
        Source::IdOnly => {
            canonical_created_default_zero::source(canonical_created_default_zero::Source::IdOnly)
        }
        Source::DefaultOnly => canonical_created_add_column::default_zero_source(),
        Source::GuiRequiredZero => {
            let before = source(Source::IdOnly);
            let mut bytes = canonical_created_add_column::append(&before, "Score");
            let catalog = &mut bytes[4 * PAGE..5 * PAGE];
            let start = word(catalog, DIRECTORY + 2 * 7);
            let end = word(catalog, DIRECTORY + 2 * 6);
            let old = catalog[start..end].to_vec();
            let count_position = old.len() - MASK_BYTES - 2;
            let begin = word(&old, count_position - 2 * (PROPERTY_VARIABLE + 1));
            assert_eq!(begin, 44);
            let body = body(&[("Score", true)], true);
            let mut row = old[..begin].to_vec();
            row.extend((0x8000_0000 | body.len() as u32).to_le_bytes());
            row.extend([0; 8]);
            row.extend(body);
            let payload_end = row.len();
            row.extend(&old[begin..]);
            let count_position = row.len() - MASK_BYTES - 2;
            for index in PROPERTY_VARIABLE + 1..=11 {
                put16(
                    &mut row,
                    count_position - 2 * (index + 1),
                    payload_end as u16,
                );
            }
            let mask = row.len() - MASK_BYTES;
            row[mask + 1] |= 0x40;
            let start = end - row.len();
            catalog[start..end].copy_from_slice(&row);
            put16(catalog, DIRECTORY + 2 * 7, start as u16);
            put16(catalog, 2, (start - (DIRECTORY + 2 * 8)) as u16);
            bytes
        }
    }
}

/// Share only the existing raw TD/map/catalog forwarding envelope; replace the
/// full property body using this independently authored dictionary/group model.
/// SDK's first ADD creates DefaultValue before Required, even when its builder
/// receives Required first. An existing Required-first dictionary stays ordered.
pub fn append(before: &[u8], case: Source, name: &str) -> Vec<u8> {
    let mut after = canonical_created_default_zero::append(before, name);
    let required_first = matches!(case, Source::GuiRequiredZero);
    let fields = match case {
        Source::IdOnly => vec![(name, true)],
        Source::GuiRequiredZero => vec![("Score", true), (name, true)],
        Source::DefaultOnly => vec![("Score", false), (name, true)],
    };
    let body = body(&fields, required_first);
    let catalog = &mut after[4 * PAGE..5 * PAGE];
    let root = word(catalog, DIRECTORY + 2 * 7);
    let start = if root & 0x4000 != 0 {
        word(catalog, DIRECTORY + 2 * 8) & 0x1fff
    } else {
        root
    };
    let end = if root & 0x4000 != 0 {
        root & 0x1fff
    } else {
        word(catalog, DIRECTORY + 2 * 6)
    };
    let row = &mut catalog[start..end];
    let count_position = row.len() - MASK_BYTES - 2;
    let property = word(row, count_position - 2 * (PROPERTY_VARIABLE + 1));
    put32(row, property, 0x4000_0000 | body.len() as u32);

    let page = &mut after[10 * PAGE..11 * PAGE];
    page.fill(0);
    page[..2].copy_from_slice(&[1, 1]);
    put16(page, 2, (PAGE - 16 - body.len()) as u16);
    page[4..8].copy_from_slice(b"LVAL");
    put16(page, 12, 1);
    put16(page, 14, (PAGE - body.len()) as u16);
    page[PAGE - body.len()..].copy_from_slice(&body);
    after
}
