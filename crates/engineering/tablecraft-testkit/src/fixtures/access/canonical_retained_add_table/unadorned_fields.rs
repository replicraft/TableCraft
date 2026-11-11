//! Independent descriptor-only scalar ADD without gallery property snapshots.
pub mod rows;
use super::typed_fields::{self, Kind};
use crate::fixtures::access::generated_record_insert::placement::{self, Pages};

/// Append one explicit declaration, preserving every catalog/property/user byte.
/// Text sort bytes are literal source expectations, not a production-reader call.
pub fn after(
    before: &[u8],
    definition: u32,
    name: &str,
    kind: Kind,
    text_order: [u8; 4],
) -> Vec<u8> {
    assert!(!matches!(
        kind,
        Kind::ShortText {
            compression: true,
            ..
        }
    ));
    typed_fields::descriptor_after(before, definition, name, kind, text_order)
}

/// Append an independently resolved generated-ID/Score/Currency record.
/// Only ID generation comes from the actual retained LAST/primary fixture law;
/// callers explicitly supply resolved companion values or NULL, never inferred
/// gallery defaults for the new unadorned declaration.
pub fn append_currency(
    before: &[u8],
    definition: u32,
    score: Option<i32>,
    amount: Option<i64>,
) -> Vec<u8> {
    let base = definition as usize * 4096;
    assert_eq!(&before[base + 41..base + 47], &[3, 0, 0, 0, 3, 0]);
    assert_eq!(&before[base + 47..base + 55], &[1, 0, 0, 0, 1, 0, 0, 0]);
    for (at, kind, number, flags, offset, width) in [
        (75, 4, 0_u16, 7_u8, 0_u16, 4_u16),
        (100, 4, 1_u16, 3_u8, 4_u16, 4_u16),
        (125, 5, 2_u16, 3_u8, 8_u16, 8_u16),
    ] {
        assert_eq!(before[base + at], kind);
        assert_eq!(&before[base + at + 5..base + at + 7], &number.to_le_bytes());
        assert_eq!(&before[base + at + 15..base + at + 17], &[flags, 0]);
        assert_eq!(
            &before[base + at + 21..base + at + 23],
            &offset.to_le_bytes()
        );
        assert_eq!(
            &before[base + at + 23..base + at + 25],
            &width.to_le_bytes()
        );
    }
    let pages = Pages {
        definition: definition as usize,
        data: definition as usize + 1,
        index: definition as usize + 2,
    };
    let id = placement::next_id(before, pages);
    let mut row = vec![3, 0];
    row.extend(id.to_le_bytes());
    row.extend(score.unwrap_or(0).to_le_bytes());
    row.extend(amount.unwrap_or(0).to_le_bytes());
    row.push(1 | if score.is_some() { 2 } else { 0 } | if amount.is_some() { 4 } else { 0 });
    placement::append(before, pages, id, &row)
}
