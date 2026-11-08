//! First-party raw inline-Memo forwarding oracles; no production codec is called.
use super::{put16, put32, saved_queries::fitting_select};

const PAGE: usize = 4096;

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

/// Complete nine-page independent source, with one five-fragment local SELECT.
pub fn source() -> Vec<u8> {
    fitting_select::storage_pair()[0].clone()
}

/// Independent SDK-observed row policy applied to one explicit synthetic root.
/// The synthetic source intentionally lacks SDK system indexes, and supplies no
/// imported-source authority. Literal logical ROW8:3 is independent of ACE code.
pub fn replace(before: &[u8], expression: &str) -> Vec<u8> {
    replace_at(before, 3, expression)
}

/// Independently replace one specified logical slot on the synthetic DATA page.
pub fn replace_at(before: &[u8], slot: usize, expression: &str) -> Vec<u8> {
    let mut after = before.to_vec();
    let page = &before[8 * PAGE..9 * PAGE];
    let root_word = word(page, 14 + 2 * slot);
    let root_start = root_word & 0x1fff;
    let root_end = if slot == 0 {
        PAGE
    } else {
        word(page, 12 + 2 * slot) & 0x1fff
    };
    let (physical_start, physical_end) = if root_word & 0x4000 == 0 {
        (root_start, root_end)
    } else {
        let pointer = u32::from_le_bytes(page[root_start..root_start + 4].try_into().unwrap());
        assert_eq!(pointer >> 8, 8);
        let slot = (pointer & 255) as usize;
        (
            word(page, 14 + 2 * slot) & 0x1fff,
            word(page, 12 + 2 * slot) & 0x1fff,
        )
    };
    let old = &page[physical_start..physical_end];
    let start = word(old, old.len() - 11);
    let utf16 = expression
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let data_end = start + 12 + utf16.len();
    let length = old.len().max(data_end + 13);
    let mut record = vec![0; length];
    record[..start].copy_from_slice(&old[..start]);
    put32(&mut record, start, 0x8000_0000 | utf16.len() as u32);
    record[start + 12..data_end].copy_from_slice(&utf16);
    record[length - 13..].copy_from_slice(&old[old.len() - 13..]);
    put16(&mut record, length - 13, data_end as u16);
    let page = &mut after[8 * PAGE..9 * PAGE];
    if length == old.len() {
        page[physical_start..physical_end].copy_from_slice(&record);
    } else {
        let count = word(page, 12);
        let frontier = word(page, 12 + 2 * count) & 0x1fff;
        let target = frontier - length;
        assert!(target >= 16 + 2 * count);
        if root_word & 0x4000 == 0 {
            page[root_start..root_end].fill(0);
        }
        put32(page, root_start, (8 << 8) | count as u32);
        put16(page, 14 + 2 * slot, root_word as u16 | 0x4000);
        put16(page, 14 + 2 * count, target as u16 | 0x8000);
        let free = word(page, 2);
        put16(page, 2, (free - length - 2) as u16);
        put16(page, 12, (count + 1) as u16);
        page[target..frontier].copy_from_slice(&record);
    }
    after
}

/// Two different saved-query objects share Column/Order1 without sharing a key.
/// Existing inert raw builder supplies both object catalog identities. This is
/// selection-only input, not the closed write-source ownership profile.
pub fn multiple_objects() -> Vec<u8> {
    use super::saved_queries::{Fragment, database_with};
    database_with(&[
        Fragment {
            object: Some(-2147483524),
            attribute: Some(6),
            order: Some(&[0, 0, 0, 1]),
            expression: Some("99"),
            ..Default::default()
        },
        Fragment {
            object: Some(-2147483525),
            attribute: Some(6),
            order: Some(&[0, 0, 0, 1]),
            expression: Some("42"),
            ..Default::default()
        },
    ])
}

/// Six independently authored fragments with two same-page projection roots.
/// Logical slot3 is Order1/Answer42; slot4 is Order2/Second99.
pub fn two_projections() -> Vec<u8> {
    let mut before = source();
    let page = &before[8 * PAGE..9 * PAGE];
    assert_eq!(word(page, 12), 5);
    let mut rows = Vec::new();
    let mut end = PAGE;
    for slot in 0..5 {
        let start = word(page, 14 + 2 * slot);
        rows.push(page[start..end].to_vec());
        end = start;
    }

    let mut second = rows[3].clone();
    assert_eq!(&second[13..17], &[0, 0, 0, 1]);
    assert_eq!(
        &second[17..29],
        &[65, 0, 110, 0, 115, 0, 119, 0, 101, 0, 114, 0]
    );
    assert_eq!(&second[41..45], &[52, 0, 50, 0]);
    second[13..17].copy_from_slice(&[0, 0, 0, 2]);
    second[17..29].copy_from_slice(&[83, 0, 101, 0, 99, 0, 111, 0, 110, 0, 100, 0]);
    second[41..45].copy_from_slice(&[57, 0, 57, 0]);
    rows.insert(4, second);

    before[8 * PAGE..9 * PAGE]
        .copy_from_slice(&super::canonical_native17_creation::packed(7, rows));
    put32(&mut before, 7 * PAGE + 16, 6);
    before
}
