//! Complete short fixed records retain ID/Score while later fields stay absent.
use super::{COLUMNS, Column, RECORD_BYTES, Scalar, declaration, payload};
use crate::fixtures::access::{
    generated_record_insert::placement::{self, Pages},
    raw_forwarding_chain, raw_record_placement,
};

/// Append a literal historical record whose complete fixed prefix contains only
/// ID and Score. The current declaration still has all eight native fields.
pub fn append(before: &[u8], definition: u32, score: i32) -> Vec<u8> {
    declaration(before, definition);
    let pages = Pages {
        definition: definition as usize,
        data: definition as usize + 1,
        index: definition as usize + 2,
    };
    let id = placement::next_id(before, pages);
    let mut record = vec![2, 0];
    record.extend(id.to_le_bytes());
    record.extend(score.to_le_bytes());
    record.push(3);
    assert_eq!(record.len(), 11);
    placement::append(before, pages, id, &record)
}

/// A supplied later scalar expands the complete fixed frame without applying
/// defaults to the other absent fields. An absent NULL no-op keeps short bytes.
pub fn after(
    before: &[u8],
    definition: u32,
    root: usize,
    column: Column,
    value: Option<Scalar>,
) -> Vec<u8> {
    declaration(before, definition);
    let original = raw_forwarding_chain::record(before, definition + 1, root);
    assert_eq!(original.len(), 11);
    assert_eq!(&original[..2], &[2, 0]);
    assert_eq!(original[10], 3);
    let Some(value) = value else {
        return before.to_vec();
    };
    let selected = &COLUMNS[column.number()];
    let bytes = payload(column, Some(value));
    assert_eq!(bytes.len(), usize::from(selected.width));
    let mut record = vec![0; RECORD_BYTES];
    record[..2].copy_from_slice(&8_u16.to_le_bytes());
    record[2..10].copy_from_slice(&original[2..10]);
    let start = 2 + usize::from(selected.fixed_offset.unwrap());
    record[start..start + bytes.len()].copy_from_slice(&bytes);
    record[RECORD_BYTES - 1] = 3 | (1 << column.number());
    raw_record_placement::place(before, definition + 1, root, &record)
}
