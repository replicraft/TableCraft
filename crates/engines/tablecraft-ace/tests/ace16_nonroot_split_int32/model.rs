//! Literal two-leaf source, independent of production page encoding.
use super::{base_model, support};

pub use base_model::{PAGE, ROOT, assert_checked, bitmap_offset};
pub const FIRST: usize = 16;
pub const LAST: usize = 17;
pub const ADDED: usize = 18;
pub const FIRST_COUNT: usize = 40;
pub const ROWS: usize = 500;
const ENTRY_START: usize = 480;
const MASK_START: usize = 27;

pub fn source(code: u8, descending: bool) -> Vec<u8> {
    let mut bytes = base_model::source(code, descending, true);
    bytes.resize(ADDED * PAGE, 0xa7);
    let keys = base_model::keys(descending, None);
    bytes[FIRST * PAGE..(FIRST + 1) * PAGE].copy_from_slice(&page(
        4,
        0,
        0,
        LAST as u32,
        0,
        3,
        &keys[..FIRST_COUNT],
    ));
    bytes[LAST * PAGE..(LAST + 1) * PAGE].copy_from_slice(&page(
        4,
        0,
        FIRST as u32,
        0,
        0,
        3,
        &keys[FIRST_COUNT..],
    ));
    let mut bound = keys[FIRST_COUNT - 1].clone();
    bound.extend((FIRST as u32).to_be_bytes());
    bytes[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&page(
        3,
        1,
        0,
        0,
        LAST as u32,
        0,
        &[bound],
    ));
    let owned = bitmap_offset(&bytes, 14, FIRST);
    bytes[owned] |= 3;
    let global = bitmap_offset(&bytes, 1, FIRST);
    bytes[global] &= !3;
    bytes
}

pub fn expected(descending: bool) -> Vec<Vec<u8>> {
    let selected = if descending { 0 } else { ROWS - 1 };
    let replacement = if descending { i32::MIN } else { i32::MAX };
    let mut values = (0..ROWS)
        .map(|row| {
            (
                if row == selected {
                    replacement
                } else {
                    row as i32 + 1
                },
                row,
            )
        })
        .collect::<Vec<_>>();
    values.sort_by(|(a, row_a), (b, row_b)| {
        (if descending { b.cmp(a) } else { a.cmp(b) }).then(row_a.cmp(row_b))
    });
    values
        .into_iter()
        .map(|(value, row)| {
            let mut wire = vec![if descending { 0x80 } else { 0x7f }];
            wire.extend(
                ((value as u32) ^ 0x80000000)
                    .to_be_bytes()
                    .map(|byte| if descending { !byte } else { byte }),
            );
            let page = if row < 250 { 12_u32 } else { 15_u32 };
            wire.extend(((page << 8) | (row % 250) as u32).to_be_bytes());
            wire
        })
        .collect()
}

fn page(
    kind: u8,
    height: u8,
    previous: u32,
    next: u32,
    tail: u32,
    prefix: usize,
    entries: &[Vec<u8>],
) -> Vec<u8> {
    let mut bytes = vec![0xa7; PAGE];
    bytes[..ENTRY_START].fill(0);
    bytes[..2].copy_from_slice(&[kind, 1]);
    support::put32(&mut bytes, 4, 10);
    support::put32(&mut bytes, 12, previous);
    support::put32(&mut bytes, 16, next);
    support::put32(&mut bytes, 20, tail);
    support::put16(&mut bytes, 24, prefix as u16);
    bytes[26] = height;
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        let shared = if position == 0 { 0 } else { prefix };
        bytes[ENTRY_START + end..ENTRY_START + end + entry.len() - shared]
            .copy_from_slice(&entry[shared..]);
        end += entry.len() - shared;
        bytes[MASK_START + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut bytes, 2, (PAGE - ENTRY_START - end) as u16);
    bytes
}
