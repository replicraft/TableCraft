//! Independent append-only second-table bytes, without production creators or plans.
use super::{
    canonical_creation, canonical_initial_table_creation as initial, canonical_native17_creation,
    data_page, put16, put32,
};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const OPAQUE_AT: usize = 3500;
const GAP_AT: usize = 512;
const MAP_GAP: &[u8; 16] = b"map-gap-sentinel";
const CATALOG_GAP: &[u8; 16] = b"cat-gap-sentinel";
const OPAQUE: &[u8; 16] = b"retained-opaque!";

pub const FIRST_TABLE: &str = "Items";
pub const NEW_TABLE: &str = "More";

/// Original populated table and literal empty second-table outcome.
pub struct Source {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub original_row: Vec<Value>,
}

/// Retain Id=1/Score=17, opaque definition bytes and map/catalog gaps while adding
/// More(Id Increment primary, Name Short Text 20), without defaults or policies.
pub fn source() -> Source {
    let original = initial::inserts::first_insert(initial::inserts::InsertCase::LongInteger);
    let mut before = original.after;
    assert_eq!(before.len(), 10 * PAGE);
    before[7 * PAGE + OPAQUE_AT..7 * PAGE + OPAQUE_AT + OPAQUE.len()].copy_from_slice(OPAQUE);
    before[3 * PAGE + GAP_AT..3 * PAGE + GAP_AT + MAP_GAP.len()].copy_from_slice(MAP_GAP);
    before[4 * PAGE + GAP_AT..4 * PAGE + GAP_AT + CATALOG_GAP.len()].copy_from_slice(CATALOG_GAP);
    let mut after = before.clone();

    // Existing objects, row identities, primary history and all opaque bytes stay put.
    put32(&mut after, 2 * PAGE + 16, 9);
    assert_eq!(records(&before[3 * PAGE..4 * PAGE]).len(), 23);
    for member in [11_u8, 11, 12] {
        let mut map = vec![0; 13];
        map[5 + usize::from(member) / 8] = 1 << (member % 8);
        append(&mut after[3 * PAGE..4 * PAGE], &map);
    }
    assert_eq!(records(&before[4 * PAGE..5 * PAGE]).len(), 8);
    let raw = canonical_creation::catalog_record(10, 0x0f000001, NEW_TABLE, 1, 0);
    append(
        &mut after[4 * PAGE..5 * PAGE],
        &canonical_native17_creation::catalog_row(&raw),
    );

    // Independent initial-table fixture owns this declaration spelling. Patch
    // physical references and the explicitly smaller Short Text capacity only.
    let short_text = initial::source(initial::Case::ShortText).bytes;
    let mut definition = short_text[7 * PAGE..8 * PAGE].to_vec();
    put32(&mut definition, 55, (3 << 8) | 23);
    put32(&mut definition, 59, (3 << 8) | 24);
    put16(&mut definition, 75 + 25 + 23, 40);
    // Two field headers and their length-prefixed UTF-16 names precede the
    // single physical index: Id is six bytes, Name ten bytes.
    let physical_index = 75 + 2 * 25 + 6 + 10;
    put32(&mut definition, physical_index + 34, (3 << 8) | 25);
    put32(&mut definition, physical_index + 38, 12);
    after.extend(definition);
    after.extend(packed(10, Vec::new()));

    let mut primary = vec![0; PAGE];
    primary[..2].copy_from_slice(&[4, 1]);
    put16(&mut primary, 2, 3616);
    put32(&mut primary, 4, 10);
    after.extend(primary);

    assert_eq!(after.len(), 13 * PAGE);
    assert_eq!(&after[7 * PAGE..10 * PAGE], &before[7 * PAGE..10 * PAGE]);
    assert_eq!(&after[3 * PAGE + GAP_AT..3 * PAGE + GAP_AT + 16], MAP_GAP);
    assert_eq!(
        &after[4 * PAGE + GAP_AT..4 * PAGE + GAP_AT + 16],
        CATALOG_GAP
    );
    Source {
        before,
        after,
        original_row: original.row,
    }
}

/// A preexisting owned map claims an append destination beyond current EOF.
/// The source must refuse; the planner cannot treat this live claim as free.
pub fn claimed_append_destination(index: bool) -> Vec<u8> {
    let mut before = source().before;
    let page = &mut before[3 * PAGE..4 * PAGE];
    let member = if index { 12_u8 } else { 11_u8 };
    let at = 14 + 2 * 3;
    let start = u16::from_le_bytes(page[at..at + 2].try_into().unwrap()) as usize;
    page[start + 5 + usize::from(member) / 8] |= 1 << (member % 8);
    before
}

/// Append within independently checked packed geometry. Existing record bodies,
/// offsets and unconsumed free-gap bytes are never rebuilt or cleared.
fn append(page: &mut [u8], row: &[u8]) {
    let old = records(page);
    let count = old.len();
    let directory_end = 14 + 2 * count;
    let free = u16::from_le_bytes(page[2..4].try_into().unwrap()) as usize;
    let end = directory_end + free;
    let start = end.checked_sub(row.len()).unwrap();
    assert!(start >= directory_end + 2);
    assert!(
        page[directory_end..directory_end + 2]
            .iter()
            .all(|byte| *byte == 0)
    );
    assert!(page[start..end].iter().all(|byte| *byte == 0));

    page[start..end].copy_from_slice(row);
    put16(page, directory_end, start as u16);
    put16(page, 12, (count + 1) as u16);
    put16(page, 2, (free - row.len() - 2) as u16);

    let updated = records(page);
    assert_eq!(&updated[..count], old.as_slice());
    assert_eq!(updated[count].as_slice(), row);
}

fn records(page: &[u8]) -> Vec<Vec<u8>> {
    assert_eq!(&page[..2], &[1, 1]);
    let count = u16::from_le_bytes(page[12..14].try_into().unwrap()) as usize;
    let mut end = PAGE;
    let mut rows = Vec::new();
    for slot in 0..count {
        let at = 14 + 2 * slot;
        let start = u16::from_le_bytes(page[at..at + 2].try_into().unwrap()) as usize;
        assert!(start >= 14 + 2 * count && start <= end);
        rows.push(page[start..end].to_vec());
        end = start;
    }
    assert_eq!(
        u16::from_le_bytes(page[2..4].try_into().unwrap()) as usize,
        end - 14 - 2 * count,
    );
    rows
}

fn packed(owner: u32, records: Vec<Vec<u8>>) -> Vec<u8> {
    let used = 14 + 2 * records.len() + records.iter().map(Vec::len).sum::<usize>();
    assert!(used <= PAGE);
    let rows = records.into_iter().map(|row| (0, row)).collect::<Vec<_>>();
    let mut page = data_page(owner, &rows);
    put16(&mut page, 2, (PAGE - used) as u16);
    page
}
