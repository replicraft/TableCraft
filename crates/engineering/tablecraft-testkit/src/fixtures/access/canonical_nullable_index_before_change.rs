//! Separate original/request-only NULL-index compatibility derivative.
use super::canonical_mixed_before_change::{self as mixed, Update};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;

/// Separate nullable-index compatibility original and literal expected image.
pub struct Case {
    /// Complete original with an ordinary include-NULL Id index.
    pub before: Vec<u8>,
    /// Complete outcome with only Score payload changes.
    pub expected: Vec<u8>,
    /// Complete original scalar rows, including the actual NULL Id.
    pub rows: Vec<Vec<Value>>,
    /// Original user request before native BeforeChange execution.
    pub update: Update,
}

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn row(bytes: &[u8], slot: usize) -> (usize, usize) {
    let page = &bytes[8 * PAGE..9 * PAGE];
    assert_eq!(&page[12..14], &3_u16.to_le_bytes());
    let at = 14 + 2 * slot;
    let word = u16::from_le_bytes(page[at..at + 2].try_into().unwrap());
    assert_eq!(word & 0xe000, 0);
    let start = 8 * PAGE + usize::from(word);
    let end = if slot == 0 {
        9 * PAGE
    } else {
        8 * PAGE + usize::from(u16::from_le_bytes(page[at - 2..at].try_into().unwrap()))
    };
    assert_eq!(&bytes[start..start + 2], &6_u16.to_le_bytes());
    (start, end)
}

/// Derive only an original envelope and literal unindexed Score expectation.
pub fn source(header: u8) -> Case {
    let original = mixed::source(header, mixed::Recipe::CounterPrimary);
    let mut before = original.before;
    assert_eq!(before.len(), 10 * PAGE);

    let td = &mut before[7 * PAGE..8 * PAGE];
    assert_eq!(&td[47..51], &1_u32.to_le_bytes());
    assert_eq!(&td[51..55], &1_u32.to_le_bytes());
    let columns = 63 + 12;
    assert_eq!(td[columns + 15], 7);
    td[columns + 15] = 3;
    td[20..24].fill(0);
    let mut names_end = columns + 6 * 25;
    for _ in 0..6 {
        let width = usize::from(u16::from_le_bytes(
            td[names_end..names_end + 2].try_into().unwrap(),
        ));
        names_end += 2 + width;
    }
    let physical = names_end;
    let logical = physical + 52;
    assert_eq!(td[physical + 46], 0x89);
    assert_eq!(td[logical + 23], 1);
    td[physical + 46] = 0x80;
    td[logical + 23] = 0;

    let (third, end) = row(&before, 2);
    assert_eq!(&before[third + 2..third + 6], &3_i32.to_le_bytes());
    assert_eq!(before[end - 1] & 1, 1);
    before[end - 1] &= !1;
    assert_eq!(&before[third + 2..third + 6], &3_i32.to_le_bytes());

    let leaf = &mut before[9 * PAGE..10 * PAGE];
    leaf[..480].fill(0);
    leaf[..2].copy_from_slice(&[4, 1]);
    put16(leaf, 2, 3593);
    leaf[4..8].copy_from_slice(&7_u32.to_le_bytes());
    let entries: [&[u8]; 3] = [
        &[0, 0, 0, 8, 2],
        &[0x7f, 0x80, 0, 0, 1, 0, 0, 8, 0],
        &[0x7f, 0x80, 0, 0, 2, 0, 0, 8, 1],
    ];
    let mut active = 0;
    for entry in entries {
        leaf[480 + active..480 + active + entry.len()].copy_from_slice(entry);
        active += entry.len();
        leaf[27 + active / 8] |= 1 << (active % 8);
    }
    assert_eq!(active, 23);

    let mut expected = before.clone();
    let (first, _) = row(&before, 0);
    assert_eq!(&before[first + 6..first + 10], &1000_i32.to_le_bytes());
    expected[first + 6..first + 10].copy_from_slice(&1018_i32.to_le_bytes());
    let mut rows = original.rows;
    rows[2][0] = Value::Null;
    Case {
        before,
        expected,
        rows,
        update: Update {
            ordinal: 0,
            expected: 1000,
            replacement: 17,
        },
    }
}
