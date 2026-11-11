//! Exact GUID/Decimal declarations and independent retained row transformations.
use super::desktop;
use crate::fixtures::access::{
    canonical_initial_table_creation::Column,
    generated_record_insert::placement::{self, Pages},
    raw_forwarding_chain, raw_record_update,
};
use tablecraft_core::values::{Decimal, Guid, Value};

const PAGE: usize = 4096;
const FIXED_END: usize = 26;
const PRECISION_BOUND: u128 = 10_u128.pow(28);
const COLUMNS: [Column; 4] = [
    Column {
        name: "ID",
        native_type: 4,
        fixed_offset: Some(0),
        width: 4,
        variable_index: 0,
        precision: 0,
        scale: 0,
    },
    Column {
        name: "Score",
        native_type: 4,
        fixed_offset: Some(4),
        width: 4,
        variable_index: 0,
        precision: 0,
        scale: 0,
    },
    Column {
        name: "Token",
        native_type: 15,
        fixed_offset: Some(8),
        width: 16,
        variable_index: 0,
        precision: 0,
        scale: 0,
    },
    Column {
        name: "Amount",
        native_type: 16,
        fixed_offset: None,
        width: 17,
        variable_index: 0,
        precision: 28,
        scale: 4,
    },
];

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn name(text: &str) -> Vec<u8> {
    let payload = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut bytes = (payload.len() as u16).to_le_bytes().to_vec();
    bytes.extend(payload);
    bytes
}

fn block(kind: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((6 + payload.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend(payload);
    bytes
}

fn properties() -> Vec<u8> {
    let mut dictionary = name("Required");
    dictionary.extend(name("DefaultValue"));
    let mut body = b"MR2\0".to_vec();
    body.extend(block(128, &dictionary));
    for (field, default) in [
        ("Score", Some("0")),
        ("Token", None),
        ("Amount", Some("0.0000")),
    ] {
        let label = name(field);
        let mut group = ((4 + label.len()) as u32).to_le_bytes().to_vec();
        group.extend(label);
        group.extend([9, 0, 1, 1, 0, 0, 1, 0, 0]);
        if let Some(text) = default {
            let encoded = text
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            group.extend(((8 + encoded.len()) as u16).to_le_bytes());
            group.extend([1, 12, 1, 0]);
            group.extend((encoded.len() as u16).to_le_bytes());
            group.extend(encoded);
        }
        body.extend(block(1, &group));
    }
    body
}

/// Desktop ID/PrimaryKey plus nullable Score, Token GUID and Amount Decimal28,4.
/// Only Score and Amount have literal defaults; Token is not generated.
pub fn append_table(before: &[u8], table: &str) -> Vec<u8> {
    let template = desktop::initial::table(&COLUMNS, &properties());
    super::append_template(before, table, &template, Some(&properties()))
}

fn declaration(before: &[u8], definition: u32) {
    let page = &before[definition as usize * PAGE..(definition as usize + 1) * PAGE];
    assert_eq!(&page[41..47], &[4, 0, 1, 0, 4, 0]);
    for (number, column) in COLUMNS.iter().enumerate() {
        let at = 75 + number * 25;
        assert_eq!(page[at], column.native_type);
        assert_eq!(word(page, at + 5), number);
        assert_eq!(word(page, at + 23), usize::from(column.width));
        assert_eq!(
            page[at + 15],
            if number == 0 {
                7
            } else if number == 3 {
                2
            } else {
                3
            }
        );
        if number == 3 {
            assert_eq!(&page[at + 11..at + 13], &[28, 4]);
        } else {
            assert_eq!(
                word(page, at + 21),
                usize::from(column.fixed_offset.unwrap())
            );
        }
    }
}

// Canonical text groups reverse their first 4/2/2 bytes in native field storage.
// Repeating this transformation restores the canonical sequence.
fn guid_bytes(mut bytes: [u8; 16]) -> [u8; 16] {
    bytes[..4].reverse();
    bytes[4..6].reverse();
    bytes[6..8].reverse();
    bytes
}

fn decimal_bytes(coefficient: i128) -> [u8; 17] {
    let magnitude = coefficient.unsigned_abs();
    assert!(magnitude < PRECISION_BOUND);
    let mut bytes = [0; 17];
    bytes[0] = if coefficient < 0 { 128 } else { 0 };
    // Most-significant 32-bit limb first, little-endian bytes within each limb.
    for (position, shift) in [96, 64, 32, 0].into_iter().enumerate() {
        let word = (magnitude >> shift) as u32;
        bytes[1 + position * 4..5 + position * 4].copy_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn decimal_value(bytes: &[u8]) -> i128 {
    assert_eq!(bytes.len(), 17);
    assert!(matches!(bytes[0], 0 | 128));
    let mut magnitude = 0_u128;
    for limb in bytes[1..].chunks_exact(4) {
        magnitude = (magnitude << 32) | u128::from(u32::from_le_bytes(limb.try_into().unwrap()));
    }
    assert!(magnitude < PRECISION_BOUND);
    if bytes[0] == 128 {
        -(magnitude as i128)
    } else {
        magnitude as i128
    }
}

/// Literal semantic outcome. Decimal coefficients are exact at declared scale4.
pub fn row(token: Option<[u8; 16]>, amount: Option<i128>, id: i32) -> Vec<Value> {
    vec![
        Value::Int32(id),
        Value::Int32(0),
        token.map_or(Value::Null, |bytes| Value::Guid(Guid::from_bytes(bytes))),
        amount.map_or(Value::Null, |value| {
            Value::Decimal(Decimal::new(value, 4).unwrap())
        }),
    ]
}

/// Append exact supplied values; None is explicit NULL, not an omitted default.
pub fn append(
    before: &[u8],
    definition: u32,
    token: Option<[u8; 16]>,
    amount: Option<i128>,
) -> Vec<u8> {
    declaration(before, definition);
    let pages = Pages {
        definition: definition as usize,
        data: definition as usize + 1,
        index: definition as usize + 2,
    };
    let id = placement::next_id(before, pages);
    let mut record = vec![4, 0];
    record.extend(id.to_le_bytes());
    record.extend(0_i32.to_le_bytes());
    record.extend(token.map_or([0; 16], guid_bytes));
    assert_eq!(record.len(), FIXED_END);
    if let Some(value) = amount {
        record.extend(decimal_bytes(value));
    }
    let end = record.len();
    record.extend((end as u16).to_le_bytes());
    record.extend((FIXED_END as u16).to_le_bytes());
    record.extend(1_u16.to_le_bytes());
    let mut mask = 3;
    if token.is_some() {
        mask |= 4;
    }
    if amount.is_some() {
        mask |= 8;
    }
    record.push(mask);
    placement::append(before, pages, id, &record)
}

/// Actual omitted companions resolve to TokenNULL and Amount0.0000.
pub fn append_omitted(before: &[u8], definition: u32) -> Vec<u8> {
    append(before, definition, None, Some(0))
}

/// Replace only the sixteen native GUID bytes and selected presence bit.
/// NULL retains ignored payload; an exact semantic no-op preserves all bytes.
pub fn update_guid(
    before: &[u8],
    definition: u32,
    root: usize,
    token: Option<[u8; 16]>,
) -> Vec<u8> {
    declaration(before, definition);
    let old = raw_forwarding_chain::record(before, definition + 1, root);
    assert_eq!(&old[..2], &[4, 0]);
    let original =
        (old[old.len() - 1] & 4 != 0).then(|| guid_bytes(old[10..26].try_into().unwrap()));
    if original == token {
        return before.to_vec();
    }
    let mut record = old.to_vec();
    let mask = record.len() - 1;
    if let Some(token) = token {
        record[10..26].copy_from_slice(&guid_bytes(token));
        record[mask] |= 4;
    } else {
        record[mask] &= !4;
    }
    raw_forwarding_chain::replace_terminal(before, definition + 1, root, &record)
}

/// Rebuild the one actual Decimal slot and footer, preserving siblings and old
/// padding. Shrink retains the original footprint; refill retains that padding.
pub fn update_decimal(
    before: &[u8],
    definition: u32,
    root: usize,
    amount: Option<i128>,
) -> Vec<u8> {
    declaration(before, definition);
    let old = raw_forwarding_chain::record(before, definition + 1, root);
    assert_eq!(&old[..2], &[4, 0]);
    let mask = old.len() - 1;
    assert_eq!(word(old, mask - 2), 1);
    let start = word(old, mask - 4);
    let stop = word(old, mask - 6);
    assert_eq!(start, FIXED_END);
    let original = (old[mask] & 8 != 0).then(|| decimal_value(&old[start..stop]));
    if original == amount {
        return before.to_vec();
    }
    let data_end = mask - 6;
    let mut record = old[..start].to_vec();
    if let Some(value) = amount {
        record.extend(decimal_bytes(value));
    }
    let end = record.len();
    record.extend_from_slice(&old[stop..data_end]);
    record.resize(record.len().max(data_end), 0);
    record.extend((end as u16).to_le_bytes());
    record.extend((start as u16).to_le_bytes());
    record.extend(1_u16.to_le_bytes());
    record.push((old[mask] & !8) | if amount.is_some() { 8 } else { 0 });
    raw_forwarding_chain::place_terminal(before, definition + 1, root, &record)
}

/// Existing LONG17 control shares this same table without changing wide values.
pub fn update_score(before: &[u8], definition: u32, root: usize, score: Option<i32>) -> Vec<u8> {
    declaration(before, definition);
    raw_record_update::number(
        before,
        definition + 1,
        root,
        definition as usize * PAGE + 100,
        1,
        score,
    )
}
